//! Omarchy theme integration (Linux only).
//!
//! Renders the active Omarchy theme's `colors.toml` into the themes directory
//! as a theme named `Omarchy` (Ghostty format, same mapping as Omarchy's own
//! `ghostty.conf.tpl`), and re-renders on every `omarchy theme set` so the
//! caller can reload the catalog. Selecting the theme stays with the user via
//! the theme picker; this module only keeps the file current.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use notify::{RecursiveMode, Watcher};

pub(crate) const THEME_NAME: &str = "Omarchy";
// Settings/theme lookup is case-sensitive; the picker shows this name.

pub(crate) struct Omarchy {
    _watcher: notify::RecommendedWatcher,
}

/// Starts the integration when Omarchy state exists; returns `None` otherwise.
///
/// Renders immediately, then re-renders on every change under Omarchy's
/// `current/` state directory. Generation can't order the events: theme-set
/// bumps it before the theme name and staged files settle, so the content
/// no-op check in `write_theme` is the debounce — a `changed` notification
/// goes out only when the rendered theme actually differs.
pub(crate) fn start(themes_dir: PathBuf, changed: async_channel::Sender<()>) -> Option<Omarchy> {
    let current = state_dir()?.join("current");
    if !current.join("theme.name").is_file() {
        return None;
    }
    render(&themes_dir);

    let mut watcher = notify::RecommendedWatcher::new(
        move |event: notify::Result<notify::Event>| {
            if event.is_err() {
                return;
            }
            if render(&themes_dir) {
                let _ = changed.send_blocking(());
            }
        },
        notify::Config::default(),
    )
    .ok()?;
    watcher.watch(&current, RecursiveMode::NonRecursive).ok()?;
    Some(Omarchy { _watcher: watcher })
}

fn state_dir() -> Option<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
        })?;
    let dir = state.join("omarchy");
    dir.is_dir().then_some(dir)
}

fn read_generation(current: &Path) -> String {
    fs::read_to_string(current.join("theme.generation")).unwrap_or_default()
}

/// Renders the active theme into `themes_dir/Omarchy`, atomically.
/// Returns `true` when the theme file's content changed.
///
/// Seqlock: the generation is re-read after the colors, and the pass restarts
/// if a switch landed in between. Colors are read from the named theme's
/// directory rather than staged files, which settle later.
fn render(themes_dir: &Path) -> bool {
    for _ in 0..3 {
        let Some(current) = state_dir().map(|dir| dir.join("current")) else {
            return false;
        };
        let generation = read_generation(&current);
        let rendered = theme_name_slug(&current)
            .and_then(|slug| read_colors(&slug))
            .and_then(|colors| render_scheme(&colors));
        if generation == read_generation(&current)
            && let Some(scheme) = rendered
        {
            return write_theme(themes_dir, &scheme);
        }
    }
    false
}

fn theme_name_slug(current: &Path) -> Option<String> {
    let name = fs::read_to_string(current.join("theme.name")).ok()?;
    let slug = name.trim().to_lowercase().replace(' ', "-");
    (!slug.is_empty()).then_some(slug)
}

fn read_colors(slug: &str) -> Option<HashMap<String, String>> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let user = home.join(".config/omarchy/themes").join(slug);
    let stock = Path::new("/usr/share/omarchy/themes").join(slug);
    let path = [user, stock].into_iter().find(|dir| dir.is_dir())?;
    let source = fs::read_to_string(path.join("colors.toml")).ok()?;
    let mut colors = HashMap::new();
    for line in source.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"');
        if value.starts_with('#') {
            colors.insert(key.trim().to_owned(), value.to_owned());
        }
    }
    Some(colors)
}

fn render_scheme(colors: &HashMap<String, String>) -> Option<String> {
    let get = |key: &str| colors.get(key).map(String::as_str);
    let or = |key: &str, fallback: &str| get(key).or_else(|| get(fallback)).or(get("foreground"));
    let (background, foreground) = (get("background")?, get("foreground")?);
    let bright_foreground = or("bright_foreground", "foreground")?;
    let selection = or("selection", "lighter_background").unwrap_or(background);
    let mut out = format!(
        "background = {background}\n\
         foreground = {foreground}\n\
         cursor-color = {bright_foreground}\n\
         selection-background = {selection}\n\
         selection-foreground = {foreground}\n\n\
         palette = 0={background}\n"
    );
    for (index, key, fallback) in [
        (1, "red", "foreground"),
        (2, "green", "foreground"),
        (3, "yellow", "foreground"),
        (4, "blue", "foreground"),
        (5, "magenta", "foreground"),
        (6, "cyan", "foreground"),
        (7, "foreground", "foreground"),
        (8, "muted", "foreground"),
        (9, "bright_red", "red"),
        (10, "bright_green", "green"),
        (11, "bright_yellow", "yellow"),
        (12, "bright_blue", "blue"),
        (13, "bright_magenta", "magenta"),
        (14, "bright_cyan", "cyan"),
        (15, "bright_foreground", "foreground"),
    ] {
        let _ = writeln!(out, "palette = {index}={}", or(key, fallback)?);
    }
    Some(out)
}

fn write_theme(themes_dir: &Path, scheme: &str) -> bool {
    let target = themes_dir.join(THEME_NAME);
    if fs::read_to_string(&target).ok().as_deref() == Some(scheme) {
        return false;
    }
    let temp = themes_dir.join(".omarchy.tmp");
    if fs::write(&temp, scheme).is_ok() && fs::rename(&temp, &target).is_ok() {
        return true;
    }
    let _ = fs::remove_file(&temp);
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colors(source: &str) -> HashMap<String, String> {
        let mut parsed = HashMap::new();
        for line in source.lines() {
            if let Some((key, value)) = line.split_once('=') {
                let value = value.trim().trim_matches('"');
                if value.starts_with('#') {
                    parsed.insert(key.trim().to_owned(), value.to_owned());
                }
            }
        }
        parsed
    }

    #[test]
    fn renders_ghostty_mapping() {
        let scheme = render_scheme(&colors(
            r##"
            mode = "dark"
            background = "#1a1b26"
            foreground = "#a9b1d6"
            bright_foreground = "#c0caf5"
            selection = "#292e42"
            red = "#f7768e"
            green = "#9ece6a"
            yellow = "#e0af68"
            blue = "#7aa2f7"
            magenta = "#ad8ee6"
            cyan = "#449dab"
            muted = "#414868"
            bright_red = "#ff7a93"
            bright_green = "#b9f27c"
            bright_yellow = "#ff9e64"
            bright_blue = "#7da6ff"
            bright_magenta = "#bb9af7"
            bright_cyan = "#0db9d7"
        "##,
        ))
        .unwrap();
        assert!(scheme.contains("background = #1a1b26"));
        assert!(scheme.contains("cursor-color = #c0caf5"));
        assert!(scheme.contains("selection-background = #292e42"));
        assert!(scheme.contains("palette = 0=#1a1b26"));
        assert!(scheme.contains("palette = 8=#414868"));
        assert!(scheme.contains("palette = 15=#c0caf5"));
    }

    #[test]
    fn falls_back_for_missing_brights() {
        let scheme = render_scheme(&colors(
            "background = \"#000000\"\nforeground = \"#ffffff\"\nred = \"#ff0000\"\n",
        ))
        .unwrap();
        assert!(scheme.contains("cursor-color = #ffffff"));
        assert!(scheme.contains("palette = 9=#ff0000"));
    }

    #[test]
    fn rejects_missing_background_or_foreground() {
        assert!(render_scheme(&colors("foreground = \"#ffffff\"")).is_none());
        assert!(render_scheme(&colors("background = \"#000000\"")).is_none());
    }
}
