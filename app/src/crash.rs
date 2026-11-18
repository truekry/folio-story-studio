//! What Folio leaves behind when something goes wrong.
//!
//! A release build has no console, so a panic would close the window without a
//! word. This module installs a panic hook that appends the message, the
//! location and a backtrace to `folio-crash.log` in the data folder, plus the
//! last few actions that led there. No settings, no keys and no chapter text
//! are written.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// The most recent actions, oldest first. A panic is usually the end of a
/// short chain; ten lines are enough to see which one it was.
const BREADCRUMBS: usize = 10;

static RECENT: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Remember one action for a later crash report. Cheap enough to call on any
/// user-visible change.
pub fn note(action: impl AsRef<str>) {
    let stamp = timestamp();
    let line = format!("{stamp}  {}", action.as_ref());
    if let Ok(mut recent) = RECENT.lock() {
        if recent.len() >= BREADCRUMBS {
            recent.remove(0);
        }
        recent.push(line);
    }
}

/// Install the panic hook. Call once, before the window opens. The previous
/// hook still runs, so a debug build keeps printing to stderr.
pub fn install(data_dir: &Path) {
    let log = data_dir.join("folio-crash.log");
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let report = report(info);
        let appended = append(&log, &report);
        if let Err(error) = appended {
            eprintln!("could not write {}: {error}", log.display());
        }
        previous(info);
    }));
}

fn report(info: &std::panic::PanicHookInfo<'_>) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "\n===== Folio panic =====");
    let _ = writeln!(out, "when:   {}", timestamp());
    let thread = std::thread::current();
    let name = thread.name().unwrap_or("unnamed");
    let _ = writeln!(out, "thread: {name}");
    let location = info
        .location()
        .map(|at| format!("{}:{}", at.file(), at.line()))
        .unwrap_or_else(|| "unknown".to_string());
    let _ = writeln!(out, "where:  {location}");
    let message = payload(info);
    let _ = writeln!(out, "message: {message}");
    let _ = writeln!(
        out,
        "backtrace:\n{}",
        std::backtrace::Backtrace::force_capture()
    );
    let _ = writeln!(out, "recent actions:");
    match RECENT.lock() {
        Ok(recent) if !recent.is_empty() => {
            for line in recent.iter() {
                let _ = writeln!(out, "  {line}");
            }
        }
        Ok(_) => {
            let _ = writeln!(out, "  (none recorded)");
        }
        Err(_) => {
            let _ = writeln!(out, "  (unavailable)");
        }
    }
    let _ = writeln!(out, "=======================");
    out
}

fn payload(info: &std::panic::PanicHookInfo<'_>) -> String {
    if let Some(message) = info.payload().downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = info.payload().downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

fn append(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(text.as_bytes())
}

fn timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let millis = now.subsec_millis();
    // UTC, to avoid a dependency just for a local-time conversion.
    let days = secs / 86_400;
    let day_secs = secs % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    let hour = day_secs / 3_600;
    let minute = (day_secs % 3_600) / 60;
    let second = day_secs % 60;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

/// Howard Hinnant's `civil_from_days`, the usual way to turn a Unix day count
/// into a date without pulling in a calendar crate.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_calendar_conversion_lands_on_known_days() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_000), (2022, 1, 8));
        assert_eq!(civil_from_days(20_000), (2024, 10, 4));
    }

    #[test]
    fn a_breadcrumb_keeps_only_the_last_few_lines() {
        // The global list is shared by other tests in this binary; fill it and
        // check the bound rather than the exact contents.
        for index in 0..(BREADCRUMBS + 5) {
            note(format!("test action {index}"));
        }
        let recent = RECENT.lock().expect("the list");
        assert!(recent.len() <= BREADCRUMBS);
        assert!(recent
            .last()
            .is_some_and(|line| line.contains("test action")));
    }
}
