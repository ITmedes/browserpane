use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command as StdCommand;

use tempfile::tempdir;
use zip::ZipArchive;

use super::{
    validate_workflow_source_entrypoint, WorkflowGitSource, WorkflowSource, WorkflowSourceError,
    WorkflowSourceResolver,
};

fn git(args: &[&str], cwd: &Path) {
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

fn git_head(cwd: &Path) -> String {
    let head = StdCommand::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(head.status.success());
    String::from_utf8_lossy(&head.stdout)
        .trim()
        .to_ascii_lowercase()
}

#[tokio::test]
async fn preserves_explicit_resolved_commit_without_git_lookup() {
    let resolver = WorkflowSourceResolver::new(PathBuf::from("git"));
    let source = WorkflowSource::Git(WorkflowGitSource {
        repository_url: "https://example.com/repo.git".to_string(),
        r#ref: None,
        resolved_commit: Some("0123456789abcdef0123456789abcdef01234567".to_string()),
        root_path: Some("workflows".to_string()),
    });

    let resolved = resolver.resolve(Some(source.clone())).await.unwrap();
    assert_eq!(resolved, Some(source));
}

#[tokio::test]
async fn resolves_git_source_from_local_repository_ref() {
    let temp = tempdir().unwrap();
    git(&["init", "--initial-branch=main"], temp.path());
    git(
        &["config", "user.email", "workflow@test.local"],
        temp.path(),
    );
    git(&["config", "user.name", "Workflow Test"], temp.path());
    fs::write(temp.path().join("README.md"), "hello\n").unwrap();
    git(&["add", "README.md"], temp.path());
    git(&["commit", "-m", "init"], temp.path());
    let expected = git_head(temp.path());

    let resolver = WorkflowSourceResolver::new(PathBuf::from("git"));
    let resolved = resolver
        .resolve(Some(WorkflowSource::Git(WorkflowGitSource {
            repository_url: temp.path().to_string_lossy().into_owned(),
            r#ref: Some("HEAD".to_string()),
            resolved_commit: None,
            root_path: Some("workflows".to_string()),
        })))
        .await
        .unwrap();

    assert_eq!(
        resolved,
        Some(WorkflowSource::Git(WorkflowGitSource {
            repository_url: temp.path().to_string_lossy().into_owned(),
            r#ref: Some("HEAD".to_string()),
            resolved_commit: Some(expected),
            root_path: Some("workflows".to_string()),
        }))
    );
}

#[tokio::test]
async fn rejects_git_source_without_ref_or_commit() {
    let resolver = WorkflowSourceResolver::new(PathBuf::from("git"));
    let error = resolver
        .resolve(Some(WorkflowSource::Git(WorkflowGitSource {
            repository_url: "https://example.com/repo.git".to_string(),
            r#ref: None,
            resolved_commit: None,
            root_path: None,
        })))
        .await
        .unwrap_err();
    assert!(matches!(error, WorkflowSourceError::Invalid(_)));
}

#[test]
fn rejects_entrypoint_outside_workflow_root_path() {
    let error = validate_workflow_source_entrypoint(
        Some(&WorkflowSource::Git(WorkflowGitSource {
            repository_url: "https://example.com/repo.git".to_string(),
            r#ref: Some("refs/heads/main".to_string()),
            resolved_commit: Some("0123456789abcdef0123456789abcdef01234567".to_string()),
            root_path: Some("workflows".to_string()),
        })),
        "scripts/export.ts",
    )
    .unwrap_err();
    assert!(matches!(error, WorkflowSourceError::Invalid(_)));
}

#[tokio::test]
async fn materializes_git_source_archive_from_local_repository() {
    let temp = tempdir().unwrap();
    git(&["init", "--initial-branch=main"], temp.path());
    git(
        &["config", "user.email", "workflow@test.local"],
        temp.path(),
    );
    git(&["config", "user.name", "Workflow Test"], temp.path());
    fs::create_dir_all(temp.path().join("workflows/smoke")).unwrap();
    fs::write(temp.path().join("README.md"), "hello\n").unwrap();
    fs::write(
        temp.path().join("workflows/smoke/export.ts"),
        "export default 1;\n",
    )
    .unwrap();
    fs::write(temp.path().join("workflows/notes.txt"), "notes\n").unwrap();
    git(&["add", "."], temp.path());
    git(&["commit", "-m", "init"], temp.path());
    let head = git_head(temp.path());

    let resolver = WorkflowSourceResolver::new(PathBuf::from("git"));
    let archive = resolver
        .materialize_archive(
            &WorkflowSource::Git(WorkflowGitSource {
                repository_url: temp.path().to_string_lossy().into_owned(),
                r#ref: None,
                resolved_commit: Some(head.clone()),
                root_path: Some("workflows".to_string()),
            }),
            "workflows/smoke/export.ts",
        )
        .await
        .unwrap();

    assert_eq!(
        archive.source,
        WorkflowSource::Git(WorkflowGitSource {
            repository_url: temp.path().to_string_lossy().into_owned(),
            r#ref: None,
            resolved_commit: Some(head),
            root_path: Some("workflows".to_string()),
        })
    );
    assert_eq!(archive.media_type, "application/zip");
    assert!(archive.file_name.ends_with(".zip"));

    let mut zip = ZipArchive::new(Cursor::new(archive.bytes)).unwrap();
    let names = (0..zip.len())
        .map(|index| zip.by_index(index).unwrap().name().to_string())
        .collect::<Vec<_>>();
    assert!(names.contains(&"workflows/smoke/export.ts".to_string()));
    assert!(names.contains(&"workflows/notes.txt".to_string()));
    assert!(!names.contains(&"README.md".to_string()));
}
