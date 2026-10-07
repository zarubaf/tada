//! The settings of `tada serve` (ADR 0036).

use std::io::Write;
use std::process::{Child, Command};
use std::time::Duration;

use tada_store_pg::testing::TestDatabase;
use tempfile::NamedTempFile;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[test]
fn serve_without_the_rate_limit_key_stops_with_exit_code_2() {
    let mut secret = NamedTempFile::new().unwrap();
    secret.write_all(b"development only").unwrap();
    let secret = secret.path().to_str().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tada"))
        .arg("serve")
        .env_clear()
        .envs([
            ("TADA_DATABASE_URL", "postgres://tada@127.0.0.1:1/tada"),
            ("TADA_DATABASE_PASSWORD_FILE", secret),
            ("TADA_S3_ENDPOINT", "http://127.0.0.1:1"),
            ("TADA_S3_REGION", "garage"),
            ("TADA_S3_BUCKET", "tada"),
            ("TADA_S3_ACCESS_KEY_ID_FILE", secret),
            ("TADA_S3_SECRET_ACCESS_KEY_FILE", secret),
            ("TADA_PUBLIC_URL", "https://tada.example.org"),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let errors = String::from_utf8(output.stderr).unwrap();
    assert!(errors.contains("TADA_RATE_LIMIT_KEY_FILE"), "{errors}");
}

/// Stops the process of the test, also when the test fails.
struct Serve(Child);

impl Drop for Serve {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The whole response to a `GET` request without a cookie, or `None` if nothing listens yet.
async fn get(port: u16, path: &str) -> Option<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.ok()?;
    let request = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).await.ok()?;
    Some(response)
}

#[tokio::test]
async fn the_serve_process_rejects_a_request_without_a_session() {
    let test = TestDatabase::start().await;
    let port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let mut password = NamedTempFile::new().unwrap();
    password.write_all(b"postgres").unwrap();
    let mut secret = NamedTempFile::new().unwrap();
    secret.write_all(b"development only").unwrap();
    let password = password.path().to_str().unwrap();
    let secret = secret.path().to_str().unwrap();
    let port_text = port.to_string();
    let _serve = Serve(
        Command::new(env!("CARGO_BIN_EXE_tada"))
            .arg("serve")
            .env_clear()
            .envs([
                ("TADA_DATABASE_URL", test.url()),
                ("TADA_DATABASE_PASSWORD_FILE", password),
                ("TADA_PORT", &port_text),
                ("TADA_S3_ENDPOINT", "http://127.0.0.1:1"),
                ("TADA_S3_REGION", "garage"),
                ("TADA_S3_BUCKET", "tada"),
                ("TADA_S3_ACCESS_KEY_ID_FILE", secret),
                ("TADA_S3_SECRET_ACCESS_KEY_FILE", secret),
                ("TADA_PUBLIC_URL", "https://tada.example.org"),
                ("TADA_RATE_LIMIT_KEY_FILE", secret),
            ])
            .spawn()
            .unwrap(),
    );

    let mut response = None;
    for _ in 0..100 {
        response = get(port, "/api/v1/events").await;
        if response.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let response = response.expect("serve did not start");
    assert!(response.starts_with("HTTP/1.1 401"), "{response}");
    assert!(response.contains("unauthenticated"), "{response}");
}
