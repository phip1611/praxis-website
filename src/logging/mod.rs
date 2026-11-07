// SPDX-License-Identifier: MIT OR Apache-2.0

//! Logging for the terminal and for the systemd journal.
//!
//! Under systemd, lines get a syslog priority prefix (e.g., `<4>` for
//! warnings) instead of a timestamp, as the journal records the time itself.
//! This makes `journalctl -p warning` and colored output work. The level can
//! be changed with `RUST_LOG`, e.g., `RUST_LOG=debug`.
//!
//! Client IP addresses and user agents are never logged.

mod journald;

use std::env;
use std::io::{self, IsTerminal};
use std::panic::{self, PanicHookInfo};
use tracing::error;
use tracing_subscriber::EnvFilter;

/// The environment variable that systemd sets to the `device:inode` of the
/// journal stream, see `systemd.exec(5)`.
const JOURNAL_STREAM: &str = "JOURNAL_STREAM";

/// The default filter in production.
const DEFAULT_FILTER: &str = "info";

/// The default filter in development, which includes the debug output of
/// this crate.
const DEV_FILTER: &str = "info,praxis_website=debug";

/// The output format of log lines.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LogFormat {
    /// For the systemd journal: syslog priority prefix, no timestamp.
    Journald,
    /// For humans: timestamp and level, colored on terminals.
    Terminal,
}

impl LogFormat {
    /// Picks the format from `JOURNAL_STREAM` and the `device:inode` of stdout.
    ///
    /// Child processes inherit `JOURNAL_STREAM`, e.g., a shell in a terminal
    /// that systemd started. So the variable alone doesn't prove that stdout
    /// is the journal, it has to match stdout, as `systemd.exec(5)` says.
    fn detect(journal_stream: Option<&str>, stdout: Option<&str>) -> Self {
        match (journal_stream, stdout) {
            (Some(journal), Some(stdout)) if journal == stdout => Self::Journald,
            _ => Self::Terminal,
        }
    }
}

/// Returns the `device:inode` of stdout, in the format of `JOURNAL_STREAM`.
#[cfg(unix)]
fn stdout_identity() -> Option<String> {
    use std::fs::File;
    use std::os::fd::AsFd;
    use std::os::unix::fs::MetadataExt;

    let fd = io::stdout().as_fd().try_clone_to_owned().ok()?;
    let metadata = File::from(fd).metadata().ok()?;
    Some(format!("{}:{}", metadata.dev(), metadata.ino()))
}

/// Returns `None`, as there is no journal on this platform.
#[cfg(not(unix))]
const fn stdout_identity() -> Option<String> {
    None
}

/// Installs the global logger and routes panics into it.
///
/// Must be called once, before any log output.
pub(crate) fn init(dev: bool) {
    let journal_stream = env::var(JOURNAL_STREAM).ok();
    let format = LogFormat::detect(journal_stream.as_deref(), stdout_identity().as_deref());
    // Unlike `try_from_default_env`, reports invalid directives on stderr
    // instead of silently using the default.
    let default = if dev { DEV_FILTER } else { DEFAULT_FILTER };
    let directives = env::var(EnvFilter::DEFAULT_ENV).unwrap_or_else(|_| default.to_string());
    let filter = EnvFilter::builder().parse_lossy(directives);

    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    match format {
        LogFormat::Journald => builder
            .with_ansi(false)
            .event_format(journald::JournaldFormat)
            .init(),
        LogFormat::Terminal => builder
            .with_ansi(io::stdout().is_terminal())
            .with_target(false)
            .init(),
    }

    panic::set_hook(Box::new(log_panic));
}

/// Logs a panic as a single error line, so that it gets the right priority in
/// the journal and isn't split into several entries.
fn log_panic(info: &PanicHookInfo<'_>) {
    let location = info
        .location()
        .map_or_else(String::new, |location| format!(" at {location}"));
    let message = info.payload_as_str().unwrap_or("unknown cause");
    error!("panic{location}: {}", message.replace('\n', " "));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The journal format is only used if stdout is the journal stream.
    #[test]
    fn detect_requires_matching_stream() {
        assert_eq!(
            LogFormat::detect(Some("8:123"), Some("8:123")),
            LogFormat::Journald
        );
        assert_eq!(
            LogFormat::detect(Some("8:123"), Some("8:456")),
            LogFormat::Terminal
        );
        assert_eq!(LogFormat::detect(None, Some("8:123")), LogFormat::Terminal);
        assert_eq!(LogFormat::detect(Some("8:123"), None), LogFormat::Terminal);
    }

    /// The identity has the `device:inode` format of `JOURNAL_STREAM`.
    #[cfg(unix)]
    #[test]
    fn stdout_identity_has_journal_stream_format() {
        let identity = stdout_identity().expect("stdout should exist in tests");
        let (device, inode) = identity.split_once(':').expect("should contain ':'");

        device
            .parse::<u64>()
            .expect("the device should be a number");
        inode.parse::<u64>().expect("the inode should be a number");
    }
}
