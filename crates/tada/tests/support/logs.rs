//! The log guard of the integration tests (ADR 0035): no log line holds a direct identifier.
//!
//! `install` starts one global subscriber with the JSON format of `serve`, which writes into a buffer.
//! `assert_clean` scans the lines of the whole test process, so a leak in any task or thread fails.
//!
//! The buffer is global. `nextest` runs each test in its own process. Under plain `cargo test` the tests of
//! one file share the buffer: a leak of another test then fails too, and `assert_route_logged` can match a
//! line of another test.

use std::io;
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::Value;
use tada::logging::layer_for;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, Layer, Registry};

/// Strings that no line may hold, whatever the test: loopback and the test client (`super::PEER`).
const ALWAYS_FORBIDDEN: [&str; 2] = ["127.0.0.1", "192.0.2.10"];

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

/// The code of tada at `trace`, because ADR 0035 forbids identifiers at each level and an operator can set
/// `TADA_LOG`. Other crates stay at `info`: hyper logs the address of the fake Bot API at `debug`.
const FILTER: &str = "info,tada=trace,tada_api=trace,tada_app=trace,tada_adapters=trace,tada_domain=trace,tada_store_pg=trace,tada_telegram=trace";

static BUFFER: OnceLock<Buffer> = OnceLock::new();

/// Starts the capture once per process. Call it before the code under test runs.
pub fn install() {
    BUFFER.get_or_init(|| {
        let buffer = Buffer::default();
        let subscriber = Registry::default()
            .with(layer_for("test", buffer.clone()).with_filter(EnvFilter::new(FILTER)));
        tracing::subscriber::set_global_default(subscriber).unwrap();
        buffer
    });
}

/// The captured lines.
fn lines() -> Vec<String> {
    let buffer = BUFFER.get().expect("call `logs::install()` first");
    let bytes = buffer.0.lock().unwrap().clone();
    String::from_utf8(bytes)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Fails if a captured line holds one of `forbidden`, for example the emails, names and tokens of the
/// test, or if it holds a client IP address.
pub fn assert_clean(forbidden: &[&str]) {
    let lines = lines();
    assert!(!lines.is_empty(), "the log capture saw no line at all");
    let always = ALWAYS_FORBIDDEN.iter().copied();
    for needle in forbidden.iter().copied().chain(always) {
        assert!(!needle.is_empty(), "an empty needle matches each line");
        // A line is JSON: it holds a quote, a backslash or a non-ASCII character in the escaped form.
        let escaped = serde_json::to_string(needle).unwrap();
        let escaped = &escaped[1..escaped.len() - 1];
        let found = lines
            .iter()
            .find(|line| line.contains(needle) || line.contains(escaped));
        if let Some(line) = found {
            panic!(
                "a log line holds a direct identifier ({} bytes): {line}",
                needle.len()
            );
        }
    }
}

/// Fails unless a request log line has the route template `route`, for example
/// `/api/v1/events/{event_id}`: the template, never the path.
pub fn assert_route_logged(route: &str) {
    let routes: Vec<String> = lines()
        .iter()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|line| line["message"] == "request completed")
        .map(|line| line["route"].as_str().unwrap().to_owned())
        .collect();
    assert!(
        routes.iter().any(|logged| logged == route),
        "no request line has the route {route}: {routes:?}"
    );
}
