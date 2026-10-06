//! JSON logs on standard output, one event per line (ADR 0035).

use json_subscriber::JsonLayer;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::fmt::time::SystemTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// Starts the logs of a process. `filter` is a valid `TADA_LOG` value.
pub fn init(process_role: &'static str, filter: &str) {
    tracing_subscriber::registry()
        .with(layer(process_role, std::io::stdout).with_filter(EnvFilter::new(filter)))
        .init();
}

/// The fields of each line: `timestamp`, `level`, `target`, the event fields with `message`, the fields
/// of all spans, for example `request_id`, and `process_role`. All fields are at the top level.
///
/// `process_role` is a static field, not a span field, so that it is also in the lines of spawned tasks.
fn layer<S, W>(process_role: &'static str, writer: W) -> JsonLayer<S, W>
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

    /// The `json-subscriber` check of the walking skeleton (ADR 0035): all fields at the top level.
    #[test]
    fn writes_one_flat_json_object_per_event() {
        let buffer = Buffer::default();
        let subscriber =
            tracing_subscriber::registry().with(layer::<Registry, _>("serve", buffer.clone()));
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
        assert!(line["target"].as_str().unwrap().starts_with("tada"));
        let timestamp = line["timestamp"].as_str().unwrap();
        assert!(timestamp.ends_with('Z'), "not UTC: {timestamp}");
    }
}
