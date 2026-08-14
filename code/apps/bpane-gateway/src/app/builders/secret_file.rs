use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::{anyhow, bail};

const MAX_SECRET_BYTES: u64 = 64 * 1024;

#[derive(Clone, Copy)]
pub(in crate::app) enum SecretFilePermissions {
    Compatibility,
    OwnerOnly,
}

pub(in crate::app) fn resolve_optional_secret(
    inline_value: Option<&str>,
    file_path: Option<&Path>,
    inline_flag: &str,
    file_flag: &str,
    permissions: SecretFilePermissions,
) -> anyhow::Result<Option<String>> {
    match (inline_value, file_path) {
        (Some(_), Some(_)) => bail!("{inline_flag} conflicts with {file_flag}"),
        (Some(value), None) => {
            let value = value.trim();
            if value.is_empty() {
                bail!("{inline_flag} must not be empty");
            }
            Ok(Some(value.to_string()))
        }
        (None, Some(path)) => load_secret_file(path, file_flag, permissions).map(Some),
        (None, None) => Ok(None),
    }
}

pub(in crate::app) fn load_secret_file(
    path: &Path,
    flag: &str,
    permissions: SecretFilePermissions,
) -> anyhow::Result<String> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| anyhow!("failed to inspect {flag}"))?;
    if metadata.file_type().is_symlink() {
        bail!("{flag} must not reference a symlink");
    }
    if !metadata.is_file() {
        bail!("{flag} must reference a regular file");
    }
    if metadata.len() > MAX_SECRET_BYTES {
        bail!("{flag} exceeds the maximum supported size");
    }
    validate_permissions(&metadata, flag, permissions)?;

    let file = File::open(path).map_err(|_| anyhow!("failed to read {flag}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_SECRET_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| anyhow!("failed to read {flag}"))?;
    if bytes.len() as u64 > MAX_SECRET_BYTES {
        bail!("{flag} exceeds the maximum supported size");
    }
    let value = String::from_utf8(bytes).map_err(|_| anyhow!("{flag} must contain UTF-8"))?;
    let value = value.trim();
    if value.is_empty() {
        bail!("{flag} must not be empty");
    }
    Ok(value.to_string())
}

#[cfg(unix)]
fn validate_permissions(
    metadata: &std::fs::Metadata,
    flag: &str,
    permissions: SecretFilePermissions,
) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    if matches!(permissions, SecretFilePermissions::OwnerOnly)
        && metadata.permissions().mode() & 0o077 != 0
    {
        bail!("{flag} must not be accessible by group or other users");
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_permissions(
    _metadata: &std::fs::Metadata,
    _flag: &str,
    _permissions: SecretFilePermissions,
) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn write_secret(path: &Path, value: &str) {
        fs::write(path, value).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        }
    }

    #[test]
    fn owner_only_file_is_trimmed_and_loaded() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("secret");
        write_secret(&path, "  value-with-whitespace  \n");

        let value =
            load_secret_file(&path, "--secret-file", SecretFilePermissions::OwnerOnly).unwrap();

        assert_eq!(value, "value-with-whitespace");
    }

    #[test]
    fn inline_and_file_values_are_ambiguous() {
        let error = resolve_optional_secret(
            Some("inline"),
            Some(Path::new("secret")),
            "--secret",
            "--secret-file",
            SecretFilePermissions::OwnerOnly,
        )
        .unwrap_err();

        assert_eq!(error.to_string(), "--secret conflicts with --secret-file");
    }

    #[test]
    fn empty_file_is_rejected_without_exposing_content() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("secret");
        write_secret(&path, " \n");

        let error =
            load_secret_file(&path, "--secret-file", SecretFilePermissions::OwnerOnly).unwrap_err();

        assert_eq!(error.to_string(), "--secret-file must not be empty");
    }

    #[test]
    fn directory_is_rejected() {
        let directory = tempfile::tempdir().unwrap();

        let error = load_secret_file(
            directory.path(),
            "--secret-file",
            SecretFilePermissions::OwnerOnly,
        )
        .unwrap_err();

        assert_eq!(
            error.to_string(),
            "--secret-file must reference a regular file"
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_is_rejected() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target");
        let link = directory.path().join("link");
        write_secret(&target, "value");
        symlink(&target, &link).unwrap();

        let error =
            load_secret_file(&link, "--secret-file", SecretFilePermissions::OwnerOnly).unwrap_err();

        assert_eq!(
            error.to_string(),
            "--secret-file must not reference a symlink"
        );
    }

    #[cfg(unix)]
    #[test]
    fn owner_only_policy_rejects_broad_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("secret");
        write_secret(&path, "value");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();

        let error =
            load_secret_file(&path, "--secret-file", SecretFilePermissions::OwnerOnly).unwrap_err();

        assert_eq!(
            error.to_string(),
            "--secret-file must not be accessible by group or other users"
        );
    }

    #[cfg(unix)]
    #[test]
    fn compatibility_policy_accepts_local_fixture_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("secret");
        write_secret(&path, "value");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        let value =
            load_secret_file(&path, "--secret-file", SecretFilePermissions::Compatibility).unwrap();

        assert_eq!(value, "value");
    }
}
