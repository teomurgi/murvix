use super::{Result, error};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub(super) fn stop(directory: &Path) -> Result<()> {
    let directory = directory.canonicalize().map_err(error)?;
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let processes: Vec<_> = directories()?
            .into_iter()
            .filter(|(_, cwd)| cwd.starts_with(&directory))
            .collect();
        if processes.is_empty() {
            return Ok(());
        }
        if processes
            .iter()
            .any(|(pid, _)| *pid <= 1 || *pid == std::process::id())
        {
            return Err(error(
                "The server is running from this worktree; files were preserved",
            ));
        }
        if Instant::now() >= deadline {
            return Err(error(
                "Processes are still using this worktree; files were preserved",
            ));
        }
        for (pid, _) in processes {
            let Some(before) = identity(pid) else {
                continue;
            };
            for force in [false, true] {
                let still_here = directories()?
                    .iter()
                    .any(|(current, cwd)| *current == pid && cwd.starts_with(&directory));
                if !still_here || identity(pid).as_ref() != Some(&before) {
                    break;
                }
                if let Err(e) = signal(pid, force)
                    && identity(pid).as_ref() == Some(&before)
                {
                    return Err(e);
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

fn signal(pid: u32, force: bool) -> Result<()> {
    let signal = if force {
        rustix::process::Signal::KILL
    } else {
        rustix::process::Signal::TERM
    };
    let Some(pid) = rustix::process::Pid::from_raw(
        i32::try_from(pid).map_err(|_| error("Invalid process id"))?,
    ) else {
        return Ok(());
    };
    match rustix::process::kill_process(pid, signal) {
        // Exited between the directory scan and the signal.
        Ok(()) | Err(rustix::io::Errno::SRCH) => Ok(()),
        Err(other) => Err(error(other)),
    }
}

#[cfg(target_os = "macos")]
fn directories() -> Result<Vec<(u32, PathBuf)>> {
    use libproc::processes::{self, ProcFilter};
    use std::os::unix::ffi::OsStringExt as _;

    let pids = processes::pids_by_type(ProcFilter::All).map_err(error)?;
    let mut result = Vec::new();
    for raw in pids {
        let Ok(pid) = i32::try_from(raw) else {
            continue;
        };
        if pid <= 0 {
            continue;
        }
        let Some(cwd) = crate::session::metadata::process_directory(pid) else {
            continue;
        };
        result.push((raw, PathBuf::from(std::ffi::OsString::from_vec(cwd.0))));
    }
    Ok(result)
}

#[cfg(target_os = "macos")]
fn identity(pid: u32) -> Option<String> {
    use libproc::libproc::{bsd_info::BSDInfo, proc_pid::pidinfo};
    let info = pidinfo::<BSDInfo>(i32::try_from(pid).ok()?, 0).ok()?;
    Some(format!(
        "{}:{}",
        info.pbi_start_tvsec, info.pbi_start_tvusec
    ))
}

#[cfg(target_os = "linux")]
fn directories() -> Result<Vec<(u32, PathBuf)>> {
    Ok(std::fs::read_dir("/proc")
        .map_err(error)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let pid = entry.file_name().to_str()?.parse().ok()?;
            Some((pid, std::fs::read_link(entry.path().join("cwd")).ok()?))
        })
        .collect())
}

#[cfg(target_os = "linux")]
fn identity(pid: u32) -> Option<String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    stat.rsplit_once(") ")?
        .1
        .split_whitespace()
        .nth(19)
        .map(str::to_owned)
}
