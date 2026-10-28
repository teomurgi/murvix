//! Completed native installs only: OS locks protect writers, and rename publishes
//! an install and its exact input manifest together. No Cargo output path is keyed.
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct Inputs(Vec<u8>);

impl Inputs {
    pub fn add(&mut self, name: &str, value: &[u8]) {
        for part in [name.as_bytes(), value] {
            self.0.extend_from_slice(&(part.len() as u64).to_le_bytes());
            self.0.extend_from_slice(part);
        }
    }

    pub fn file(&mut self, path: &Path) -> io::Result<()> {
        self.add(&path.to_string_lossy(), &fs::read(path)?);
        Ok(())
    }
}

const HASH_SEED: u64 = 0xcbf29ce484222325;

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}

pub fn install(
    root: &Path,
    inputs: &Inputs,
    valid: impl Fn(&Path) -> bool,
    build: impl FnOnce(&Path) -> PathBuf,
) -> io::Result<PathBuf> {
    fs::create_dir_all(root)?;
    let root = fs::canonicalize(root)?;
    let mut hash = HASH_SEED;
    hash_bytes(&mut hash, &inputs.0);
    let key = format!("{hash:016x}");
    let completed = root.join(&key);
    let partial = root.join(format!("{key}.partial"));
    // The lock lives outside the renamed directory and is never unlinked.
    // OS locks also release automatically if a build is killed or panics.
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(format!("{key}.lock")))?;
    lock.lock()?;
    if let Ok(manifest) = fs::read(completed.join("inputs")) {
        // Directory hashes only select buckets; exact equality authorizes reuse.
        if manifest != inputs.0 {
            return Err(io::Error::other(
                "native cache key collision; choose a different cache directory",
            ));
        }
        let install = completed.join("install");
        if valid(&install) {
            println!("cargo:warning=libghostty-vt native cache hit: {key}");
            return Ok(install);
        }
    }
    println!("cargo:warning=libghostty-vt native cache miss: {key}");
    for directory in [&partial, &completed] {
        if directory.exists() {
            fs::remove_dir_all(directory)?;
        }
    }
    fs::create_dir(&partial)?;
    let work = partial.join("work");
    fs::create_dir(&work)?;
    let built = build(&work);
    if !valid(&built) {
        return Err(io::Error::other(
            "native build produced an incomplete install",
        ));
    }
    fs::rename(&built, partial.join("install"))?;
    fs::remove_dir_all(&work)?;
    // Writing the manifest is the last step before atomic publication. A failed
    // or interrupted build has no completed path, and can never be a hit.
    fs::write(partial.join("inputs"), &inputs.0)?;
    fs::rename(&partial, &completed)?;
    Ok(completed.join("install"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
    };

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "ghostty-cache-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn inputs(value: &[u8]) -> Inputs {
        let mut inputs = Inputs::default();
        inputs.add("semantic-input", value);
        inputs
    }
    fn valid(path: &Path) -> bool {
        matches!(fs::read(path.join("library")), Ok(bytes) if bytes == b"complete")
            && path.join("header").is_file()
    }
    fn build(work: &Path) -> PathBuf {
        let prefix = work.join("prefix");
        fs::create_dir(&prefix).unwrap();
        fs::write(prefix.join("library"), b"complete").unwrap();
        fs::write(prefix.join("header"), b"header").unwrap();
        prefix
    }

    #[test]
    fn complete_hits_and_semantic_changes_miss() {
        let root = Scratch::new();
        let first = install(&root.0, &inputs(b"first"), valid, build).unwrap();
        assert_eq!(
            install(&root.0, &inputs(b"first"), valid, |_| panic!("cache miss")).unwrap(),
            first
        );
        assert_ne!(
            install(&root.0, &inputs(b"second"), valid, build).unwrap(),
            first
        );
        fs::remove_file(first.join("header")).unwrap();
        install(&root.0, &inputs(b"first"), valid, build).unwrap();
        assert!(valid(&first));
    }

    #[test]
    fn interrupted_and_invalid_builds_cannot_publish() {
        let root = Scratch::new();
        assert!(
            std::panic::catch_unwind(|| {
                let _ = install(&root.0, &inputs(b"same"), valid, |work| {
                    build(work);
                    panic!("interrupted build");
                });
            })
            .is_err()
        );
        assert!(
            install(&root.0, &inputs(b"same"), valid, |work| {
                let prefix = build(work);
                fs::remove_file(prefix.join("header")).unwrap();
                prefix
            })
            .is_err()
        );
        let completed = install(&root.0, &inputs(b"same"), valid, build).unwrap();
        assert!(valid(&completed));
        assert!(root.0.read_dir().unwrap().all(|entry| {
            !entry
                .unwrap()
                .path()
                .to_string_lossy()
                .ends_with(".partial")
        }));
    }

    #[test]
    fn missing_manifest_and_corrupt_library_are_misses() {
        let root = Scratch::new();
        let prefix = install(&root.0, &inputs(b"same"), valid, build).unwrap();
        fs::remove_file(prefix.parent().unwrap().join("inputs")).unwrap();
        install(&root.0, &inputs(b"same"), valid, build).unwrap();
        fs::write(prefix.join("library"), b"").unwrap();
        install(&root.0, &inputs(b"same"), valid, build).unwrap();
        assert!(valid(&prefix));
    }

    #[test]
    fn mismatched_manifest_never_reuses_or_removes_another_install() {
        let root = Scratch::new();
        let prefix = install(&root.0, &inputs(b"same"), valid, build).unwrap();
        fs::write(prefix.parent().unwrap().join("inputs"), b"different inputs").unwrap();
        assert!(
            install(&root.0, &inputs(b"same"), valid, |_| panic!(
                "must not build"
            ))
            .is_err()
        );
        assert!(valid(&prefix));
    }

    #[test]
    fn concurrent_builders_publish_once() {
        let root = Scratch::new();
        let barrier = Arc::new(Barrier::new(8));
        let builds = Arc::new(AtomicUsize::new(0));
        let mut threads = Vec::new();
        for _ in 0..8 {
            let root = root.0.clone();
            let barrier = Arc::clone(&barrier);
            let builds = Arc::clone(&builds);
            threads.push(std::thread::spawn(move || {
                barrier.wait();
                install(&root, &inputs(b"same"), valid, |work| {
                    builds.fetch_add(1, Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(30));
                    build(work)
                })
                .unwrap()
            }));
        }
        let paths: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect();
        assert!(paths.iter().all(|path| path == &paths[0] && valid(path)));
        assert_eq!(builds.load(Ordering::SeqCst), 1);
    }

    fn child(root: &Path, mode: &str) -> std::process::Child {
        std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "tests::producer_child",
                "--nocapture",
            ])
            .env("GHOSTTY_CACHE_TEST_ROOT", root)
            .env("GHOSTTY_CACHE_TEST_MODE", mode)
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap()
    }

    #[test]
    #[ignore = "subprocess fixture invoked by producer tests"]
    fn producer_child() {
        use std::io::Write;
        let root = PathBuf::from(
            std::env::var_os("GHOSTTY_CACHE_TEST_ROOT").expect("producer fixture root"),
        );
        install(&root, &inputs(b"processes"), valid, |work| {
            let prefix = build(work);
            if std::env::var("GHOSTTY_CACHE_TEST_MODE").unwrap() == "kill" {
                fs::write(root.join("ready"), b"locked").unwrap();
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(root.join("producers"))
                .unwrap()
                .write_all(b"producer\n")
                .unwrap();
            std::thread::sleep(std::time::Duration::from_millis(50));
            prefix
        })
        .unwrap();
    }

    #[test]
    fn killed_producer_releases_lock_without_publishing() {
        let root = Scratch::new();
        let mut producer = child(&root.0, "kill");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !root.0.join("ready").exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let ready = root.0.join("ready").exists();
        producer.kill().unwrap();
        producer.wait().unwrap();
        assert!(ready, "producer did not acquire its lock");
        let prefix = install(&root.0, &inputs(b"processes"), valid, build).unwrap();
        assert!(valid(&prefix));
    }

    #[test]
    fn independent_processes_publish_once() {
        let root = Scratch::new();
        let mut producers: Vec<_> = (0..4).map(|_| child(&root.0, "build")).collect();
        for producer in &mut producers {
            assert!(producer.wait().unwrap().success());
        }
        assert_eq!(fs::read(root.0.join("producers")).unwrap(), b"producer\n");
        install(&root.0, &inputs(b"processes"), valid, |_| {
            panic!("not published")
        })
        .unwrap();
    }

    #[test]
    fn changed_patch_contents_cannot_reuse_previous_install() {
        let root = Scratch::new();
        let patch = root.0.join("source.patch");
        fs::write(&patch, b"first patch").unwrap();
        let mut first = Inputs::default();
        first.file(&patch).unwrap();
        let previous = install(&root.0, &first, valid, build).unwrap();
        fs::write(&patch, b"changed patch").unwrap();
        let mut second = Inputs::default();
        second.file(&patch).unwrap();
        let updated = install(&root.0, &second, valid, build).unwrap();
        assert_ne!(previous, updated);
        assert!(valid(&previous) && valid(&updated));
    }

    #[test]
    fn field_boundaries_are_unambiguous() {
        let root = Scratch::new();
        let mut first = Inputs::default();
        first.add("a", b"bc");
        let mut second = Inputs::default();
        second.add("ab", b"c");
        let previous = install(&root.0, &first, valid, build).unwrap();
        assert_ne!(install(&root.0, &second, valid, build).unwrap(), previous);
    }
}
