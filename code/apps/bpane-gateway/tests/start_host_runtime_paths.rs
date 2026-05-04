use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("gateway crate should live under code/apps/bpane-gateway")
        .to_path_buf()
}

fn validation_command() -> Command {
    let mut command = Command::new("bash");
    command.arg(repo_root().join("deploy/start-host.sh"));
    command.env("BPANE_VALIDATE_RUNTIME_PATHS_ONLY", "1");
    command.env("BPANE_SESSION_ID", "test-session");
    command
}

#[test]
fn docker_runtime_path_validation_accepts_session_data_paths() {
    let temp = TempDir::new().expect("create temp dir");
    let root = temp.path().join("session-data");
    let output = validation_command()
        .env("BPANE_SESSION_DATA_DIR", &root)
        .env("BPANE_PROFILE_DIR", root.join("chromium"))
        .env("BPANE_UPLOAD_DIR", root.join("uploads"))
        .env("BPANE_DOWNLOAD_DIR", root.join("downloads"))
        .output()
        .expect("run validation");

    assert!(
        output.status.success(),
        "expected validation success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn docker_runtime_path_validation_rejects_escaping_profile_dir() {
    let temp = TempDir::new().expect("create temp dir");
    let root = temp.path().join("session-data");
    let output = validation_command()
        .env("BPANE_SESSION_DATA_DIR", &root)
        .env(
            "BPANE_PROFILE_DIR",
            temp.path().join("other-session/chromium"),
        )
        .env("BPANE_UPLOAD_DIR", root.join("uploads"))
        .env("BPANE_DOWNLOAD_DIR", root.join("downloads"))
        .output()
        .expect("run validation");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("escapes BPANE_SESSION_DATA_DIR"),
        "stderr should explain containment failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn docker_runtime_path_validation_rejects_symlink_escape() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().expect("create temp dir");
    let root = temp.path().join("session-data");
    let outside = temp.path().join("outside");
    let link = root.join("linked-outside");
    std::fs::create_dir_all(&root).expect("create session root");
    std::fs::create_dir_all(&outside).expect("create outside dir");
    symlink(&outside, &link).expect("create symlink");

    let output = validation_command()
        .env("BPANE_SESSION_DATA_DIR", &root)
        .env("BPANE_PROFILE_DIR", link.join("chromium"))
        .env("BPANE_UPLOAD_DIR", root.join("uploads"))
        .env("BPANE_DOWNLOAD_DIR", root.join("downloads"))
        .output()
        .expect("run validation");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("escapes BPANE_SESSION_DATA_DIR"),
        "stderr should explain containment failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
