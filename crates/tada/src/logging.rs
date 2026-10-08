//! JSON logs on standard output, one event per line (ADR 0035).

use json_subscriber::JsonLayer;
use tracing::{Level, Subscriber};
use tracing_subscriber::filter::{FilterExt, filter_fn};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::fmt::time::SystemTime;
use tracing_subscriber::layer::Filter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// Starts the logs of a process. `filter` is a valid `TADA_LOG` value.
pub fn init(process_role: &'static str, filter: &str) {
    tracing_subscriber::registry()
        .with(layer_for(process_role, std::io::stdout).with_filter(self::filter(filter)))
        .init();
}

/// The filter of `TADA_LOG`, with a fixed cap for crates that log content at lower levels (ADR 0035).
///
/// rmcp logs each MCP request and result at `debug`, and a rejected tool input with the serde message
/// at `warn`. Both can hold the words of members, for example a search query or a quote. Only the
/// errors of rmcp pass. The cap is a second filter after `TADA_LOG`, so no directive of `TADA_LOG`,
/// also not a longer target such as `rmcp::service=trace`, can raise it.
pub fn filter<S: Subscriber>(directives: &str) -> impl Filter<S> + use<S> {
    EnvFilter::new(directives).and(filter_fn(|metadata| {
        !is_capped(metadata.target()) || *metadata.level() == Level::ERROR
    }))
}

/// True for the targets of rmcp: `rmcp` and its modules.
fn is_capped(target: &str) -> bool {
    target == "rmcp" || target.starts_with("rmcp::")
}

/// The layer of one line format, for any writer. Tests of other crates use it to read the lines.
///
/// The fields of each line: `timestamp`, `level`, `target`, the event fields with `message`, the fields
/// of all spans, for example `request_id`, and `process_role`. All fields are at the top level.
///
/// `process_role` is a static field, not a span field, so that it is also in the lines of spawned tasks.
/// `json-subscriber` also writes the name of the innermost span as `name`. An event field `name` would
/// repeat this key; ADR 0035 forbids names in logs anyway.
pub fn layer_for<S, W>(process_role: &'static str, writer: W) -> JsonLayer<S, W>
where
    S: tracing::Subscriber + for<'lookup> LookupSpan<'lookup>,
    W: for<'writer> MakeWriter<'writer> + 'static,
{
    let mut layer = JsonLayer::<S>::new(writer);
    layer
        .with_timer("timestamp", SystemTime)
        .with_level("level")
        .with_target("target")
        .with_flattened_event()
        .with_top_level_flattened_span_list();
    layer.add_static_field("process_role", process_role.into());
    layer
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::{Arc, Mutex};

    use serde_json::Value;
    use tracing_subscriber::Registry;

    use super::*;

    #[derive(Clone, Default)]
    struct Buffer(Arc<Mutex<Vec<u8>>>);

    impl io::Write for Buffer {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().write(bytes)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'writer> MakeWriter<'writer> for Buffer {
        type Writer = Self;

        fn make_writer(&'writer self) -> Self::Writer {
            self.clone()
        }
    }

    /// rmcp logs content below `error`, so no `TADA_LOG` value lets those lines through (ADR 0035).
    #[test]
    fn rmcp_logs_only_errors_whatever_the_filter_says() {
        for directives in [
            "info",
            "trace",
            "rmcp=trace",
            "trace,rmcp=trace",
            "rmcp=debug",
            "rmcp::service=trace",
            "rmcp::transport=debug",
            "info,rmcp::service::server=trace",
        ] {
            let buffer = Buffer::default();
            let subscriber = tracing_subscriber::registry().with(
                layer_for::<Registry, _>("serve", buffer.clone()).with_filter(filter(directives)),
            );
            tracing::subscriber::with_default(subscriber, || {
                tracing::debug!(target: "rmcp::service", query = "Flugfeld", "received request");
                tracing::warn!(target: "rmcp::service", error = "Flugfeld", "response error");
                tracing::trace!(target: "rmcp::service", query = "Flugfeld", "received request");
                tracing::debug!(target: "rmcp::transport::streamable_http_server", "Flugfeld");
                tracing::error!(target: "rmcp::service", "fail to close sink");
                // A crate whose name starts with `rmcp` but is another crate is not capped.
                tracing::error!(target: "rmcp_other", "not capped");
            });
            let output = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
            let rmcp_lines = output
                .lines()
                .filter(|line| line.contains("\"target\":\"rmcp::"))
                .count();
            // The error passes where `TADA_LOG` enables it; `rmcp::transport=debug` enables no `rmcp::service`.
            let expected = usize::from(directives != "rmcp::transport=debug");
            assert_eq!(rmcp_lines, expected, "{directives}: {output}");
            assert!(
                output
                    .lines()
                    .all(|line| !line.contains("\"target\":\"rmcp::")
                        || line.contains("\"level\":\"ERROR\"")),
                "{directives}: {output}"
            );
            assert!(!output.contains("Flugfeld"), "{directives}");
        }
    }

    /// The cap hides nothing of tada itself.
    #[test]
    fn the_logs_of_tada_pass_the_cap() {
        let buffer = Buffer::default();
        let subscriber = tracing_subscriber::registry().with(
            layer_for::<Registry, _>("serve", buffer.clone())
                .with_filter(filter("info,tada_mcp=debug")),
        );
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "tada_api::request_id", "request completed");
            tracing::debug!(target: "tada_mcp::tools", "tool called");
            tracing::debug!(target: "tada_api::request_id", "below info");
        });
        let output = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
        assert_eq!(output.lines().count(), 2, "{output}");
    }

    /// The `json-subscriber` check of the walking skeleton (ADR 0035): all fields at the top level.
    #[test]
    fn writes_one_flat_json_object_per_event() {
        let buffer = Buffer::default();
        let subscriber =
            tracing_subscriber::registry().with(layer_for::<Registry, _>("serve", buffer.clone()));
        tracing::subscriber::with_default(subscriber, || {
            let request = tracing::info_span!(
                "request",
                request_id = "01a1114f-9428-7111-8a10-be3f0112e5e0"
            );
            let _entered = request.enter();
            let job = tracing::info_span!("job", job_id = 7);
            let _entered = job.enter();
            tracing::info!(organization_id = "org", "event created");
        });

        let output = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 1);
        let line: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(line["message"], "event created");
        assert_eq!(line["level"], "INFO");
        assert_eq!(line["process_role"], "serve");
        assert_eq!(line["request_id"], "01a1114f-9428-7111-8a10-be3f0112e5e0");
        assert_eq!(line["job_id"], 7);
        assert_eq!(line["organization_id"], "org");
        assert_eq!(line["name"], "job", "the name of the innermost span");
        assert!(line["target"].as_str().unwrap().starts_with("tada"));
        let timestamp = line["timestamp"].as_str().unwrap();
        assert!(timestamp.ends_with('Z'), "not UTC: {timestamp}");
    }
}
