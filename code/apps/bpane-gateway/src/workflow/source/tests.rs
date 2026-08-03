use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command as StdCommand;

use tempfile::tempdir;
use zip::ZipArchive;

use super::{
    WorkflowGitSource, WorkflowSource, WorkflowSourceError, WorkflowSourcePolicy,
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

fn default_resolver() -> WorkflowSourceResolver {
    WorkflowSourceResolver::with_policy(PathBuf::from("git"), WorkflowSourcePolicy::default())
}

fn resolver_trusting_temp() -> WorkflowSourceResolver {
    WorkflowSourceResolver::with_policy(
        PathBuf::from("git"),
        WorkflowSourcePolicy::default().with_trusted_local_roots([std::env::temp_dir()]),
    )
}

fn resolver_trusting_temp_with_limits(
    max_files: usize,
    max_file_bytes: u64,
    max_total_bytes: u64,
) -> WorkflowSourceResolver {
    WorkflowSourceResolver::with_policy(
        PathBuf::from("git"),
        WorkflowSourcePolicy::default()
            .with_trusted_local_roots([std::env::temp_dir()])
            .with_collection_limits(max_files, max_file_bytes, max_total_bytes),
    )
}

#[tokio::test]
async fn preserves_explicit_resolved_commit_without_git_lookup() {
    let resolver = default_resolver();
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

    let resolver = resolver_trusting_temp();
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

#[cfg(unix)]
#[tokio::test]
async fn resolves_local_repository_with_scoped_dubious_ownership_trust() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempdir().unwrap();
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).unwrap();
    git(&["init", "--initial-branch=main"], &repo);
    git(&["config", "user.email", "workflow@test.local"], &repo);
    git(&["config", "user.name", "Workflow Test"], &repo);
    fs::write(repo.join("README.md"), "hello\n").unwrap();
    git(&["add", "README.md"], &repo);
    git(&["commit", "-m", "init"], &repo);
    let expected = git_head(&repo);

    let git_wrapper = temp.path().join("git-assume-different-owner");
    fs::write(
        &git_wrapper,
        r#"#!/bin/sh
if [ "$1" = "config" ]; then
  exec git "$@"
fi
if [ -z "$GIT_CONFIG_GLOBAL" ]; then
  echo "fatal: detected dubious ownership in repository" >&2
  exit 128
fi
after_separator=0
repository=""
for argument in "$@"; do
  if [ "$after_separator" = "1" ]; then
    repository="$argument"
    break
  fi
  if [ "$argument" = "--" ]; then
    after_separator=1
  fi
done
safe_directories=$(git config --file "$GIT_CONFIG_GLOBAL" --get-all safe.directory)
case "$safe_directories" in
  *"$repository/.git"*) ;;
  *)
    echo "fatal: protected config does not trust the exact repository" >&2
    exit 128
    ;;
esac
case "$safe_directories" in
  *'*'*)
    echo "fatal: protected config contains wildcard trust" >&2
    exit 128
    ;;
esac
exec git "$@"
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&git_wrapper).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&git_wrapper, permissions).unwrap();

    let direct = StdCommand::new(&git_wrapper)
        .env_remove("GIT_CONFIG_GLOBAL")
        .args(["ls-remote", "--"])
        .arg(&repo)
        .arg("HEAD")
        .output()
        .unwrap();
    assert!(!direct.status.success());
    assert!(String::from_utf8_lossy(&direct.stderr).contains("dubious ownership"));

    let resolver = WorkflowSourceResolver::with_policy(
        git_wrapper.clone(),
        WorkflowSourcePolicy::default().with_trusted_local_roots([temp.path().to_path_buf()]),
    );
    let resolved = resolver
        .resolve(Some(WorkflowSource::Git(WorkflowGitSource {
            repository_url: repo.to_string_lossy().into_owned(),
            r#ref: Some("HEAD".to_string()),
            resolved_commit: None,
            root_path: None,
        })))
        .await
        .unwrap();
    assert_eq!(
        resolved,
        Some(WorkflowSource::Git(WorkflowGitSource {
            repository_url: repo.to_string_lossy().into_owned(),
            r#ref: Some("HEAD".to_string()),
            resolved_commit: Some(expected),
            root_path: None,
        }))
    );

    let after = StdCommand::new(&git_wrapper)
        .env_remove("GIT_CONFIG_GLOBAL")
        .args(["ls-remote", "--"])
        .arg(&repo)
        .arg("HEAD")
        .output()
        .unwrap();
    assert!(!after.status.success());
    assert!(String::from_utf8_lossy(&after.stderr).contains("dubious ownership"));
}

#[tokio::test]
async fn rejects_git_source_without_ref_or_commit() {
    let resolver = default_resolver();
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

#[tokio::test]
async fn rejects_unsafe_git_repository_urls_before_git_lookup() {
    let resolver = default_resolver();
    for repository_url in [
        "ext::sh -c touch /tmp/pwned",
        "file:///workspace/dev",
        "git@github.com:ITmedes/browserpane.git",
        "http://example.com/repo.git",
        "-uhttps://example.com/repo.git",
    ] {
        let error = resolver
            .resolve(Some(WorkflowSource::Git(WorkflowGitSource {
                repository_url: repository_url.to_string(),
                r#ref: Some("HEAD".to_string()),
                resolved_commit: None,
                root_path: None,
            })))
            .await
            .unwrap_err();
        assert!(
            matches!(error, WorkflowSourceError::Invalid(_)),
            "{repository_url} produced {error:?}"
        );
    }
}

#[tokio::test]
async fn rejects_local_repository_outside_trusted_roots() {
    let temp = tempdir().unwrap();
    git(&["init", "--initial-branch=main"], temp.path());

    let resolver = default_resolver();
    let error = resolver
        .resolve(Some(WorkflowSource::Git(WorkflowGitSource {
            repository_url: temp.path().to_string_lossy().into_owned(),
            r#ref: Some("HEAD".to_string()),
            resolved_commit: None,
            root_path: None,
        })))
        .await
        .unwrap_err();
    assert!(matches!(error, WorkflowSourceError::Invalid(_)));
}

#[test]
fn rejects_entrypoint_outside_workflow_root_path() {
    let error = default_resolver()
        .validate_entrypoint(
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

    let resolver = resolver_trusting_temp();
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

#[cfg(unix)]
#[tokio::test]
async fn rejects_source_preview_symlink_file() {
    use std::os::unix::fs as unix_fs;

    let temp = tempdir().unwrap();
    git(&["init", "--initial-branch=main"], temp.path());
    git(
        &["config", "user.email", "workflow@test.local"],
        temp.path(),
    );
    git(&["config", "user.name", "Workflow Test"], temp.path());
    fs::create_dir_all(temp.path().join("workflows")).unwrap();
    fs::write(temp.path().join("README.md"), "outside workflow root\n").unwrap();
    fs::write(temp.path().join("workflows/run.ts"), "export default 1;\n").unwrap();
    unix_fs::symlink("../README.md", temp.path().join("workflows/link.ts")).unwrap();
    git(&["add", "."], temp.path());
    git(&["commit", "-m", "init"], temp.path());
    let head = git_head(temp.path());

    let resolver = resolver_trusting_temp();
    let error = resolver
        .materialize_source_file_preview(
            &WorkflowSource::Git(WorkflowGitSource {
                repository_url: temp.path().to_string_lossy().into_owned(),
                r#ref: None,
                resolved_commit: Some(head),
                root_path: Some("workflows".to_string()),
            }),
            "workflows/run.ts",
            "workflows/link.ts",
            1024,
        )
        .await
        .unwrap_err();

    assert!(
        matches!(&error, WorkflowSourceError::Materialize(message) if message.contains("symlinks are not supported")),
        "{error:?}"
    );
}

#[tokio::test]
async fn rejects_source_listing_when_file_count_limit_is_exceeded() {
    let temp = tempdir().unwrap();
    git(&["init", "--initial-branch=main"], temp.path());
    git(
        &["config", "user.email", "workflow@test.local"],
        temp.path(),
    );
    git(&["config", "user.name", "Workflow Test"], temp.path());
    fs::create_dir_all(temp.path().join("workflows")).unwrap();
    fs::write(temp.path().join("workflows/run.ts"), "export default 1;\n").unwrap();
    fs::write(
        temp.path().join("workflows/extra.ts"),
        "export default 2;\n",
    )
    .unwrap();
    git(&["add", "."], temp.path());
    git(&["commit", "-m", "init"], temp.path());
    let head = git_head(temp.path());

    let resolver = resolver_trusting_temp_with_limits(1, 1024, 2048);
    let error = resolver
        .materialize_source_files(
            &WorkflowSource::Git(WorkflowGitSource {
                repository_url: temp.path().to_string_lossy().into_owned(),
                r#ref: None,
                resolved_commit: Some(head),
                root_path: Some("workflows".to_string()),
            }),
            "workflows/run.ts",
        )
        .await
        .unwrap_err();

    assert!(
        matches!(&error, WorkflowSourceError::Snapshot(message) if message.contains("maximum file count")),
        "{error:?}"
    );
}
