//! mimir-api (`ro-ai-bridge`) and `monitor` refuse to start when JWT_SECRET is
//! missing, empty, or a value published in this public repository.
//!
//! Each case runs the real binary with a cleared environment in an empty temp
//! directory (no `.env`). No DATABASE_URL is set, so a binary that gets past the
//! JWT check stops at `init_db` ("DATABASE_URL must be set") without touching
//! any database or network.

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const API: &str = env!("CARGO_BIN_EXE_ro-ai-bridge");
const MONITOR: &str = env!("CARGO_BIN_EXE_monitor");

fn empty_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mimir-jwt-refuse-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    assert!(!dir.join(".env").exists());
    dir
}

/// Run `bin` with only `vars` set; kill it if it is still running after 30 s.
fn run(bin: &str, tag: &str, vars: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(bin);
    cmd.env_clear()
        .current_dir(empty_dir(tag))
        .env("RUST_LOG", "info")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in vars {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().expect("spawn binary");
    let deadline = Instant::now() + Duration::from_secs(30);
    while child.try_wait().expect("poll child").is_none() {
        if Instant::now() > deadline {
            child.kill().ok();
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    child.wait_with_output().expect("collect output")
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn assert_refused(out: &Output, why: &str) {
    let all = text(out);
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1, got {:?}\n{all}",
        out.status
    );
    assert!(all.contains("Refusing to start"), "{all}");
    assert!(all.contains(why), "expected {why:?} in:\n{all}");
    assert!(
        !all.contains("DATABASE_URL must be set"),
        "must refuse before touching the database:\n{all}"
    );
}

fn assert_passed_jwt_check(out: &Output) {
    let all = text(out);
    assert!(!all.contains("Refusing to start"), "{all}");
    assert!(
        all.contains("DATABASE_URL must be set"),
        "expected to get past the JWT check and stop at init_db:\n{all}"
    );
}

#[test]
fn api_refuses_a_missing_jwt_secret() {
    assert_refused(&run(API, "api-missing", &[]), "JWT_SECRET is not set");
}

#[test]
fn api_refuses_an_empty_jwt_secret() {
    assert_refused(
        &run(API, "api-empty", &[("JWT_SECRET", "")]),
        "JWT_SECRET is not set",
    );
}

#[test]
fn api_refuses_the_old_default() {
    assert_refused(
        &run(API, "api-default", &[("JWT_SECRET", "dev_secret_key")]),
        "public value",
    );
}

#[test]
fn api_starts_past_the_check_with_a_private_secret() {
    assert_passed_jwt_check(&run(
        API,
        "api-private",
        &[("JWT_SECRET", "test-only-private-6c1f0e2b9a")],
    ));
}

#[test]
fn api_allows_the_default_only_with_the_dev_opt_in() {
    let out = run(API, "api-optin", &[("MIMIR_ALLOW_INSECURE_DEV_JWT", "1")]);
    assert_passed_jwt_check(&out);
    assert!(
        text(&out).contains("insecure_jwt_secret_default"),
        "{}",
        text(&out)
    );
}

#[test]
fn monitor_refuses_a_missing_jwt_secret() {
    assert_refused(&run(MONITOR, "mon-missing", &[]), "JWT_SECRET is not set");
}

#[test]
fn monitor_refuses_the_old_default() {
    assert_refused(
        &run(MONITOR, "mon-default", &[("JWT_SECRET", "dev_secret_key")]),
        "public value",
    );
}

#[test]
fn monitor_starts_past_the_check_with_a_private_secret() {
    assert_passed_jwt_check(&run(
        MONITOR,
        "mon-private",
        &[("JWT_SECRET", "test-only-private-6c1f0e2b9a")],
    ));
}
