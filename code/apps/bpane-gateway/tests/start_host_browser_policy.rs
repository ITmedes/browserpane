use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("gateway crate should live under code/apps/bpane-gateway")
        .to_path_buf()
}

fn managed_policy() -> Value {
    let policy_path = repo_root().join("deploy/chromium-policies/managed/bpane.json");
    let policy = std::fs::read_to_string(&policy_path).expect("read managed Chromium policy");
    serde_json::from_str(&policy).expect("managed Chromium policy should be valid JSON")
}

fn policy_validation_command() -> Command {
    let mut command = Command::new("bash");
    command.arg(repo_root().join("deploy/start-host.sh"));
    command.env("BPANE_VALIDATE_BROWSER_POLICY_ONLY", "1");
    command.env(
        "BPANE_CHROMIUM_POLICY_FILE",
        repo_root().join("deploy/chromium-policies/managed/bpane.json"),
    );
    command
}

#[test]
fn managed_policy_blocks_file_url_navigation_by_default() {
    let policy = managed_policy();
    let blocklist = policy
        .get("URLBlocklist")
        .and_then(Value::as_array)
        .expect("URLBlocklist should be configured");

    assert!(
        blocklist.iter().any(|value| value == "file:///*"),
        "URLBlocklist should block file:///*"
    );
}

#[test]
fn managed_policy_denies_file_system_access_api_by_default() {
    let policy = managed_policy();

    assert_eq!(
        policy
            .get("DefaultFileSystemReadGuardSetting")
            .and_then(Value::as_i64),
        Some(2),
        "File System Access API read prompts should be blocked"
    );
    assert_eq!(
        policy
            .get("DefaultFileSystemWriteGuardSetting")
            .and_then(Value::as_i64),
        Some(2),
        "File System Access API write prompts should be blocked"
    );
}

#[test]
fn managed_policy_preserves_extension_policy() {
    let policy = managed_policy();

    let force_list = policy
        .get("ExtensionInstallForcelist")
        .and_then(Value::as_array)
        .expect("ExtensionInstallForcelist should be configured");
    assert!(
        force_list.iter().any(|value| value
            .as_str()
            .is_some_and(|item| item.contains("gighmmpiobklfepjocnamgkkbiglidom"))),
        "existing AdBlock extension policy should remain configured"
    );

    assert!(
        policy
            .pointer("/3rdparty/extensions/gighmmpiobklfepjocnamgkkbiglidom")
            .is_some(),
        "existing extension settings should remain configured"
    );
}

#[test]
fn start_host_reports_deny_all_local_file_policy() {
    let output = policy_validation_command()
        .output()
        .expect("run policy validation");

    assert!(
        output.status.success(),
        "expected policy validation success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Browser local file access policy: mode=deny_all"),
        "stderr should report deny_all mode: {stderr}"
    );
    assert!(
        stderr.contains("file_url_navigation=blocked"),
        "stderr should report blocked file URL navigation: {stderr}"
    );
    assert!(
        stderr.contains("file_system_read=blocked"),
        "stderr should report blocked File System API reads: {stderr}"
    );
    assert!(
        stderr.contains("file_system_write=blocked"),
        "stderr should report blocked File System API writes: {stderr}"
    );
}
