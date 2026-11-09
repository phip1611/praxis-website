// SPDX-License-Identifier: MIT OR Apache-2.0

//! The log line format for the systemd journal.

use std::fmt;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;

/// The target prefix of this crate's own events.
const CRATE_TARGET: &str = env!("CARGO_CRATE_NAME");

/// Formats events as `<priority>message key=value`.
///
/// The journal parses the prefix as syslog priority (see `SyslogLevelPrefix=`
/// in `systemd.exec(5)`). Events of other crates also show their target, so
/// that their origin is clear.
#[derive(Clone, Copy, Debug)]
pub(super) struct JournaldFormat;

impl<S, N> FormatEvent<S, N> for JournaldFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    /// Writes one event as one line.
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let metadata = event.metadata();
        write!(writer, "<{}>", syslog_priority(*metadata.level()))?;
        if !metadata.target().starts_with(CRATE_TARGET) {
            write!(writer, "{}: ", metadata.target())?;
        }
        ctx.format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}

/// Maps a log level to its syslog priority, as defined in RFC 5424.
const fn syslog_priority(level: Level) -> u8 {
    match level {
        Level::ERROR => 3,
        Level::WARN => 4,
        Level::INFO => 6,
        Level::DEBUG | Level::TRACE => 7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::sync::{Arc, Mutex};
    use tracing::{info, subscriber, warn};
    use tracing_subscriber::fmt::MakeWriter;

    /// Collects the log output in memory.
    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<u8>>>);

    impl io::Write for Capture {
        /// Appends to the buffer.
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0
                .lock()
                .expect("lock should not be poisoned")
                .write(buf)
        }

        /// Nothing to flush.
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for Capture {
        type Writer = Self;

        /// Hands out a clone that writes to the same buffer.
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    /// Runs `log` with the journald format and returns the output.
    fn capture(log: impl FnOnce()) -> String {
        let output = Capture::default();
        let logger = tracing_subscriber::fmt()
            .with_writer(output.clone())
            .with_ansi(false)
            .event_format(JournaldFormat)
            .finish();
        subscriber::with_default(logger, log);
        let bytes = output
            .0
            .lock()
            .expect("lock should not be poisoned")
            .clone();
        String::from_utf8(bytes).expect("log output should be UTF-8")
    }

    /// Each level gets its syslog priority, and fields follow the message.
    #[test]
    fn writes_priority_prefix_and_fields() {
        let output = capture(|| {
            info!(status = 200, "request served");
            warn!("disk almost full");
        });

        assert_eq!(
            output,
            "<6>request served status=200\n<4>disk almost full\n"
        );
    }

    /// Events of dependencies show where they come from.
    #[test]
    fn shows_target_of_foreign_events() {
        let output = capture(|| info!(target: "hyper::server", "connection reset"));

        assert_eq!(output, "<6>hyper::server: connection reset\n");
    }

    /// The mapping follows RFC 5424.
    #[test]
    fn maps_levels_to_syslog_priorities() {
        assert_eq!(syslog_priority(Level::ERROR), 3);
        assert_eq!(syslog_priority(Level::WARN), 4);
        assert_eq!(syslog_priority(Level::INFO), 6);
        assert_eq!(syslog_priority(Level::DEBUG), 7);
        assert_eq!(syslog_priority(Level::TRACE), 7);
    }
}
