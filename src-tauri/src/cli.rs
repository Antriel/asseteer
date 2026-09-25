//! Startup CLI arguments.
//!
//! Parsed once, lazily, from `std::env::args()`. They exist so the test harness can drive
//! the real app without touching the user's own library (`tests/CLAUDE.md`):
//!
//! - `--data-dir <path>` replaces the app data directory — the SQLite library, CLAP logs,
//!   uv. Without it a harness run would scan fixtures into the real `asseteer.db`.
//! - `--profile-dir <path>` puts the webview's browsing data (localStorage: settings, view
//!   state) somewhere other than the profile every build of Asseteer shares.
//! - `--offscreen` parks the window off the visible desktop, so an unattended run doesn't
//!   steal focus. CDP screenshots come from the renderer, so an offscreen window still
//!   paints (unlike a minimized one).
//! - `--window-size WxH` in logical pixels. Under CDP the viewport *is* the window.
//!
//! All take both forms: `--flag <value>` and `--flag=<value>`.

use std::sync::OnceLock;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CliArgs {
    pub data_dir: Option<String>,
    pub profile_dir: Option<String>,
    pub offscreen: bool,
    pub window_size: Option<(u32, u32)>,
}

impl CliArgs {
    /// True when the app runs under the harness. Anything that persists outside the
    /// isolated directories (the window-state plugin writes to the real config dir)
    /// must stay off in that case.
    pub fn isolated(&self) -> bool {
        self.data_dir.is_some()
    }
}

static ARGS: OnceLock<CliArgs> = OnceLock::new();

/// The parsed startup arguments. Parsed on first call, cached thereafter.
pub fn args() -> &'static CliArgs {
    ARGS.get_or_init(|| parse(std::env::args().skip(1)))
}

/// Parse a `WIDTHxHEIGHT` pair. Returns `None` on anything malformed.
fn parse_size(value: &str) -> Option<(u32, u32)> {
    let (w, h) = value.split_once(['x', 'X'])?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
}

fn parse(argv: impl Iterator<Item = String>) -> CliArgs {
    let mut out = CliArgs::default();
    let mut argv = argv.peekable();

    while let Some(arg) = argv.next() {
        if arg == "--offscreen" {
            out.offscreen = true;
            continue;
        }

        // Every other flag takes a value, as `--flag=value` or `--flag value`.
        let (flag, inline) = match arg.split_once('=') {
            Some((flag, value)) => (flag.to_string(), Some(value.to_string())),
            None => (arg, None),
        };
        if !matches!(
            flag.as_str(),
            "--data-dir" | "--profile-dir" | "--window-size"
        ) {
            continue; // Unknown arguments are ignored, not fatal.
        }
        let value = inline.or_else(|| argv.next_if(|next| !next.starts_with("--")));
        let Some(value) = value else {
            eprintln!("[asseteer] {flag} needs a value; ignoring it");
            continue;
        };

        match flag.as_str() {
            "--data-dir" => out.data_dir = Some(value),
            "--profile-dir" => out.profile_dir = Some(value),
            "--window-size" => out.window_size = parse_size(&value),
            _ => unreachable!(),
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> CliArgs {
        parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn empty() {
        assert_eq!(p(&[]), CliArgs::default());
        assert!(!p(&[]).isolated());
    }

    #[test]
    fn both_forms() {
        let a = p(&["--data-dir", "C:/x", "--profile-dir=C:/y", "--window-size", "1400x900"]);
        assert_eq!(a.data_dir.as_deref(), Some("C:/x"));
        assert_eq!(a.profile_dir.as_deref(), Some("C:/y"));
        assert_eq!(a.window_size, Some((1400, 900)));
        assert!(a.isolated());
    }

    #[test]
    fn offscreen_and_unknown() {
        let a = p(&["--whatever", "--offscreen", "--window-size=bad"]);
        assert!(a.offscreen);
        assert_eq!(a.window_size, None);
    }

    #[test]
    fn missing_value_does_not_eat_next_flag() {
        let a = p(&["--data-dir", "--offscreen"]);
        assert_eq!(a.data_dir, None);
        assert!(a.offscreen);
    }
}
