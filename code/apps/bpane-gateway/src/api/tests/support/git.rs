use super::super::*;

pub(crate) fn create_sleep_workflow_worker_script(
    dir: &tempfile::TempDir,
    capture_file: &std::path::Path,
    sleep_seconds: f32,
) -> std::path::PathBuf {
    let script_path = dir.path().join("workflow-worker-test.sh");
    fs::write(
        &script_path,
        format!(
            r#"#!/bin/sh
printf '%s\n' "$@" >> "{}"
sleep {}
"#,
            capture_file.display(),
            sleep_seconds,
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(&script_path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions).unwrap();
    script_path
}

pub(crate) fn git(args: &[&str], cwd: &std::path::Path) {
    let output = StdCommand::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(crate) fn git_head(cwd: &std::path::Path) -> String {
    let output = StdCommand::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_ascii_lowercase()
}
