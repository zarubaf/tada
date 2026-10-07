//! The settings of `tada serve` (ADR 0036).

use std::io::Write;
use std::process::Command;

use tempfile::NamedTempFile;

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
