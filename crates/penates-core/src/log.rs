//! Penates's own log: warnings, errors and crashes (panics) from the manager, the app windows and the
//! command line, one line each, in `<data>/penates/logs/penates.log`. It stays on this computer; a problem
//! report (`crate::report`) shows its recent lines for the user to review and send themselves.
//! Writing is best effort: a log that cannot be written never stops Penates.

use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

/// When the log passes this size it becomes `penates.log.1` (replacing an older one) and a new one starts.
const MAX_BYTES: u64 = 256 * 1024;
/// Longest message kept; longer ones are cut.
const MAX_LINE: usize = 2000;

static SOURCE: OnceLock<String> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Warn,
    Error,
    Crash,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Level::Warn => "WARN",
            Level::Error => "ERROR",
            Level::Crash => "CRASH",
        }
    }
}

pub fn dir() -> PathBuf {
    crate::store::data_dir().join("logs")
}

pub fn path() -> PathBuf {
    dir().join("penates.log")
}

/// Name this process in its log lines: "manager", "app github", "cli". Set once, at start.
pub fn set_source(source: &str) {
    let _ = SOURCE.set(source.to_string());
}

/// Write one line to the log (not to the terminal).
pub fn write(level: Level, message: &str) {
    let mut msg: String = message.split_whitespace().collect::<Vec<_>>().join(" ");
    if msg.chars().count() > MAX_LINE {
        msg = msg.chars().take(MAX_LINE).collect::<String>() + "…";
    }
    let line = format!(
        "{} {} {} {}: {msg}\n",
        utc_now(),
        level.label(),
        crate::updates::BUILD_VERSION,
        SOURCE.get().map(String::as_str).unwrap_or("penates")
    );
    let _ = append(&path(), &line);
}

fn append(p: &std::path::Path, line: &str) -> std::io::Result<()> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    if std::fs::metadata(p).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = std::fs::rename(p, p.with_extension("log.1"));
    }
    // One write call per line in append mode, so lines from several Penates processes do not mix.
    std::fs::OpenOptions::new().create(true).append(true).open(p)?.write_all(line.as_bytes())
}

/// A warning: printed to the terminal (for anyone who started Penates from one) and logged.
pub fn warn(message: &str) {
    eprintln!("warning: {message}");
    write(Level::Warn, message);
}

/// An error: printed to the terminal and logged.
pub fn error(message: &str) {
    eprintln!("error: {message}");
    write(Level::Error, message);
}

/// Log crashes (panics) with where they happened, then let the default handler report them as usual.
pub fn log_panics() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let what = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic".into());
        let at = info.location().map(|l| format!(" at {}:{}", l.file(), l.line())).unwrap_or_default();
        write(Level::Crash, &format!("{what}{at}"));
        default(info);
    }));
}

/// The last `n` lines of the log (the older file first, if the log rotated recently), oldest first.
pub fn recent(n: usize) -> Vec<String> {
    recent_in(&path(), n)
}

fn recent_in(p: &std::path::Path, n: usize) -> Vec<String> {
    let mut lines: Vec<String> = vec![];
    for f in [p.with_extension("log.1"), p.to_path_buf()] {
        if let Ok(text) = std::fs::read_to_string(&f) {
            lines.extend(text.lines().filter(|l| !l.trim().is_empty()).map(String::from));
        }
    }
    let skip = lines.len().saturating_sub(n);
    lines.split_off(skip)
}

/// Delete the log (both files).
pub fn clear() -> std::io::Result<()> {
    let p = path();
    for f in [p.with_extension("log.1"), p] {
        match std::fs::remove_file(&f) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    Ok(())
}

/// The current time as `2026-11-16T07:12:03Z`.
fn utc_now() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    utc_string(secs)
}

fn utc_string(secs: u64) -> String {
    let (days, rest) = ((secs / 86_400) as i64, secs % 86_400);
    // Days since 1970-01-01 to a calendar date (Howard Hinnant's civil_from_days).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rest / 3600, rest % 3600 / 60, rest % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_utc_calendar_dates() {
        assert_eq!(utc_string(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc_string(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(utc_string(1_794_813_123), "2026-11-16T07:12:03Z");
    }

    #[test]
    fn the_log_rotates_and_recent_reads_across_both_files() {
        let dir = std::env::temp_dir().join(format!("penates-log-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = dir.join("penates.log");
        let big = "x".repeat(1000);
        for i in 0..600 {
            append(&p, &format!("{i} {big}\n")).unwrap();
        }
        assert!(p.with_extension("log.1").exists(), "rotated past {MAX_BYTES} bytes");
        assert!(std::fs::metadata(&p).unwrap().len() <= MAX_BYTES + 1100);
        let last = recent_in(&p, 3);
        assert_eq!(last.len(), 3);
        assert!(last[2].starts_with("599 ") && last[0].starts_with("597 "), "{:?}", &last[0][..8]);
        let all = recent_in(&p, 10_000);
        // Two rotations: the first file's lines are gone, the second's are read before the current one.
        assert!(all.len() > 256 && all.len() < 600, "{}", all.len());
        assert!(!all[0].starts_with("0 ") && all.windows(2).all(|w| w[0].split(' ').next().unwrap().parse::<u32>().unwrap() + 1 == w[1].split(' ').next().unwrap().parse::<u32>().unwrap()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
