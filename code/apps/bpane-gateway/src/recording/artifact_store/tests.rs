#[cfg(unix)]
use std::os::unix::fs::{symlink, PermissionsExt};
#[cfg(unix)]
use std::os::unix::net::UnixListener;

use tempfile::{tempdir, TempDir};

use super::*;

fn test_store(temp_dir: &TempDir) -> (RecordingArtifactStore, PathBuf, PathBuf) {
    let root = temp_dir.path().join("artifacts");
    let staging_root = temp_dir.path().join("staging");
    (
        RecordingArtifactStore::local_fs(root.clone(), staging_root.clone()),
        root,
        staging_root,
    )
}

fn staged_path(staging_root: &Path, session_id: Uuid, recording_id: Uuid) -> PathBuf {
    staging_root
        .join(session_id.to_string())
        .join(format!("{recording_id}.webm"))
}

fn write_staged_artifact(
    staging_root: &Path,
    session_id: Uuid,
    recording_id: Uuid,
    bytes: &[u8],
) -> PathBuf {
    let source = staged_path(staging_root, session_id, recording_id);
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    std::fs::write(&source, bytes).unwrap();
    source
}

fn finalize_request(
    session_id: Uuid,
    recording_id: Uuid,
    source_path: impl Into<String>,
    expected_bytes: Option<u64>,
) -> FinalizeRecordingArtifactRequest {
    FinalizeRecordingArtifactRequest {
        session_id,
        recording_id,
        format: SessionRecordingFormat::Webm,
        source_path: source_path.into(),
        expected_bytes,
    }
}

#[tokio::test]
async fn local_fs_store_moves_source_file_into_managed_root() {
    let temp_dir = tempdir().unwrap();
    let (store, root, staging_root) = test_store(&temp_dir);
    let session_id = Uuid::now_v7();
    let recording_id = Uuid::now_v7();
    let source = write_staged_artifact(&staging_root, session_id, recording_id, b"artifact");

    let artifact = store
        .finalize(finalize_request(
            session_id,
            recording_id,
            source.to_string_lossy(),
            Some(8),
        ))
        .await
        .unwrap();

    assert!(!source.exists());
    assert_eq!(artifact.bytes, 8);
    assert_eq!(
        artifact.artifact_ref,
        format!("{LOCAL_FS_REF_PREFIX}{session_id}/{recording_id}.webm")
    );
    let bytes = store.read(&artifact.artifact_ref).await.unwrap();
    assert_eq!(bytes.as_slice(), b"artifact");
    assert!(root
        .join(session_id.to_string())
        .join(format!("{recording_id}.webm"))
        .exists());
}

#[tokio::test]
async fn local_fs_store_rejects_invalid_references() {
    let temp_dir = tempdir().unwrap();
    let (store, _, _) = test_store(&temp_dir);
    let error = store.read("../../../etc/passwd").await.unwrap_err();
    assert!(matches!(
        error,
        RecordingArtifactStoreError::InvalidReference(_)
    ));
}

#[tokio::test]
async fn local_fs_store_readiness_creates_and_cleans_probe() {
    let temp_dir = tempdir().unwrap();
    let (store, root, staging_root) = test_store(&temp_dir);

    store.check_readiness().await.unwrap();

    assert!(root.exists());
    assert!(staging_root.exists());
    assert_eq!(std::fs::read_dir(root).unwrap().count(), 0);
    assert_eq!(std::fs::read_dir(staging_root).unwrap().count(), 0);
}

#[tokio::test]
async fn local_fs_store_readiness_rejects_relative_or_overlapping_roots() {
    let temp_dir = tempdir().unwrap();
    let root = temp_dir.path().join("artifacts");
    let invalid_stores = [
        RecordingArtifactStore::local_fs(root.clone(), PathBuf::from("relative-staging")),
        RecordingArtifactStore::local_fs(root.clone(), root.clone()),
        RecordingArtifactStore::local_fs(root.clone(), root.join("staging")),
        RecordingArtifactStore::local_fs(
            root.clone(),
            temp_dir.path().join("staging").join("..").join("aliased"),
        ),
    ];

    for store in invalid_stores {
        let error = store.check_readiness().await.unwrap_err();
        assert!(matches!(
            error,
            RecordingArtifactStoreError::InvalidConfiguration(_)
        ));
    }
}

#[tokio::test]
async fn local_fs_store_rejects_paths_outside_the_assigned_staging_path() {
    let temp_dir = tempdir().unwrap();
    let (store, _, staging_root) = test_store(&temp_dir);
    let session_id = Uuid::now_v7();
    let recording_id = Uuid::now_v7();
    let outside = temp_dir.path().join("outside.webm");
    std::fs::write(&outside, b"outside").unwrap();
    let alias = format!(
        "{}/./{session_id}/{recording_id}.webm",
        staging_root.display()
    );
    let invalid_paths = [
        "".to_string(),
        "relative.webm".to_string(),
        outside.to_string_lossy().to_string(),
        staged_path(&staging_root, Uuid::now_v7(), recording_id)
            .to_string_lossy()
            .to_string(),
        staged_path(&staging_root, session_id, Uuid::now_v7())
            .to_string_lossy()
            .to_string(),
        staging_root
            .join(session_id.to_string())
            .join(format!("{recording_id}.zip"))
            .to_string_lossy()
            .to_string(),
        alias,
    ];

    for source_path in invalid_paths {
        let error = store
            .finalize(finalize_request(
                session_id,
                recording_id,
                source_path,
                None,
            ))
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            RecordingArtifactStoreError::InvalidSourcePath(_)
        ));
    }
    assert!(outside.exists());
}

#[tokio::test]
async fn local_fs_store_rejects_missing_and_directory_sources() {
    let temp_dir = tempdir().unwrap();
    let (store, _, staging_root) = test_store(&temp_dir);
    let session_id = Uuid::now_v7();
    let recording_id = Uuid::now_v7();
    let source = staged_path(&staging_root, session_id, recording_id);

    let missing_error = store
        .finalize(finalize_request(
            session_id,
            recording_id,
            source.to_string_lossy(),
            None,
        ))
        .await
        .unwrap_err();
    assert!(matches!(
        missing_error,
        RecordingArtifactStoreError::InvalidSourcePath(_)
    ));

    std::fs::create_dir_all(&source).unwrap();
    let directory_error = store
        .finalize(finalize_request(
            session_id,
            recording_id,
            source.to_string_lossy(),
            None,
        ))
        .await
        .unwrap_err();
    assert!(matches!(
        directory_error,
        RecordingArtifactStoreError::InvalidSourcePath(_)
    ));
}

#[tokio::test]
async fn local_fs_store_uses_actual_bytes_and_rejects_declared_mismatch() {
    let temp_dir = tempdir().unwrap();
    let (store, _, staging_root) = test_store(&temp_dir);
    let session_id = Uuid::now_v7();
    let recording_id = Uuid::now_v7();
    let source = write_staged_artifact(&staging_root, session_id, recording_id, b"artifact");

    let error = store
        .finalize(finalize_request(
            session_id,
            recording_id,
            source.to_string_lossy(),
            Some(7),
        ))
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "recording artifact byte count mismatch: declared 7, actual 8"
    );
    assert!(source.exists());

    let artifact = store
        .finalize(finalize_request(
            session_id,
            recording_id,
            source.to_string_lossy(),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(artifact.bytes, 8);
}

#[cfg(unix)]
#[tokio::test]
async fn local_fs_store_rejects_symlink_file_and_parent() {
    let temp_dir = tempdir().unwrap();
    let (store, _, staging_root) = test_store(&temp_dir);
    let session_id = Uuid::now_v7();
    let recording_id = Uuid::now_v7();
    let source = staged_path(&staging_root, session_id, recording_id);
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    let target = temp_dir.path().join("target.webm");
    std::fs::write(&target, b"target").unwrap();
    symlink(&target, &source).unwrap();

    let file_error = store
        .finalize(finalize_request(
            session_id,
            recording_id,
            source.to_string_lossy(),
            None,
        ))
        .await
        .unwrap_err();
    assert!(matches!(
        file_error,
        RecordingArtifactStoreError::InvalidSourcePath(_)
    ));
    std::fs::remove_file(&source).unwrap();
    std::fs::remove_dir(source.parent().unwrap()).unwrap();

    let real_parent = temp_dir.path().join("real-parent");
    std::fs::create_dir_all(&real_parent).unwrap();
    std::fs::write(real_parent.join(format!("{recording_id}.webm")), b"target").unwrap();
    symlink(&real_parent, source.parent().unwrap()).unwrap();
    let parent_error = store
        .finalize(finalize_request(
            session_id,
            recording_id,
            source.to_string_lossy(),
            None,
        ))
        .await
        .unwrap_err();
    assert!(matches!(
        parent_error,
        RecordingArtifactStoreError::InvalidSourcePath(_)
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn local_fs_store_rejects_non_regular_sources() {
    let temp_dir = tempfile::Builder::new()
        .prefix("bp")
        .tempdir_in("/tmp")
        .unwrap();
    let (store, _, staging_root) = test_store(&temp_dir);
    let session_id = Uuid::now_v7();
    let recording_id = Uuid::now_v7();
    let source = staged_path(&staging_root, session_id, recording_id);
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    let _listener = UnixListener::bind(&source).unwrap();

    let error = store
        .finalize(finalize_request(
            session_id,
            recording_id,
            source.to_string_lossy(),
            None,
        ))
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        RecordingArtifactStoreError::InvalidSourcePath(_)
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn local_fs_store_copies_read_only_source_without_failing_finalize() {
    let temp_dir = tempdir().unwrap();
    let (store, root, staging_root) = test_store(&temp_dir);
    let session_id = Uuid::now_v7();
    let recording_id = Uuid::now_v7();
    let source = write_staged_artifact(&staging_root, session_id, recording_id, b"artifact");
    let source_dir = source.parent().unwrap();
    let mut permissions = std::fs::metadata(source_dir).unwrap().permissions();
    permissions.set_mode(0o555);
    std::fs::set_permissions(source_dir, permissions).unwrap();

    let artifact = store
        .finalize(finalize_request(
            session_id,
            recording_id,
            source.to_string_lossy(),
            Some(8),
        ))
        .await
        .unwrap();

    let mut restore_permissions = std::fs::metadata(source_dir).unwrap().permissions();
    restore_permissions.set_mode(0o755);
    std::fs::set_permissions(source_dir, restore_permissions).unwrap();

    assert_eq!(
        artifact.artifact_ref,
        format!("{LOCAL_FS_REF_PREFIX}{session_id}/{recording_id}.webm")
    );
    let bytes = store.read(&artifact.artifact_ref).await.unwrap();
    assert_eq!(bytes.as_slice(), b"artifact");
    assert!(root
        .join(session_id.to_string())
        .join(format!("{recording_id}.webm"))
        .exists());
}
