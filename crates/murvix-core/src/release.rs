//! Which release a build is, read from its version.
//!
//! Releases are plain semver `X.Y.Z`. Any other version, such as an
//! unstamped checkout's `0.0.0`, is a local build.

/// This build's application name.
pub const APP_NAME: &str = "Murvix";

/// This build's bundle identifier (macOS) and application ID (Linux).
pub const BUNDLE_ID: &str = "com.murvix.app";

/// Parse a released `X.Y.Z` version; `None` for local builds.
pub fn parse(version: &str) -> Option<[u64; 3]> {
    let mut parts = [0; 3];
    let mut numbers = version.split('.');
    for part in &mut parts {
        *part = number(numbers.next()?)?;
    }
    if numbers.next().is_some() {
        return None;
    }
    Some(parts)
}

/// This build's release, or `None` for a local build.
pub fn current() -> Option<[u64; 3]> {
    parse(env!("CARGO_PKG_VERSION"))
}

/// Whether `target` is a newer release than `current`.
pub fn is_newer(target: &str, current: &str) -> bool {
    parse(target)
        .zip(parse(current))
        .is_some_and(|(target, current)| target > current)
}

fn number(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn releases_replace_only_older_releases() {
        assert!(is_newer("2.0.10", "2.0.9"));
        assert!(is_newer("3.0.0", "2.9.9"));
        assert!(!is_newer("2.0.0", "2.0.1"));
        assert!(!is_newer("2.0.0", "2.0.0"));
        assert!(parse("2.0.0-beta.10").is_none());
        assert!(parse("2.0-beta.99").is_none());
        assert!(parse("2.0.0.0").is_none());
        assert!(parse("2.0.0-beta-0").is_none());
        assert!(parse("").is_none());
    }
}
