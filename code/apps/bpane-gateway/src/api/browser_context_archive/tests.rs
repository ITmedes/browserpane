use std::collections::HashMap;

use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use tar::{Builder, EntryType, Header};
use uuid::Uuid;

use super::*;
use crate::session_control::{BrowserContextResource, BrowserContextUsageResource};

#[test]
fn round_trips_manifest_only_export() {
    let manifest = test_manifest(false);
    let archive = build_browser_context_export_archive(&manifest, None).unwrap();

    let parsed =
        parse_browser_context_import_archive(&archive, test_limits()).expect("valid archive");

    assert_eq!(parsed.manifest.source_context.name, "test-context");
    assert!(parsed.profile_archive.is_none());
}

#[test]
fn accepts_regular_profile_files_and_directories() {
    let profile = profile_archive(&[("Default/Preferences", b"{}"), ("Cache/data", b"abc")]);
    let archive =
        build_browser_context_export_archive(&test_manifest(true), Some(&profile)).unwrap();

    let parsed =
        parse_browser_context_import_archive(&archive, test_limits()).expect("valid archive");

    assert_eq!(parsed.profile_archive.as_deref(), Some(profile.as_slice()));
}

#[tokio::test(flavor = "current_thread")]
async fn packages_large_profile_export_off_the_async_runtime() {
    let profile = vec![0x5a; 4 * 1024 * 1024];
    let export =
        build_browser_context_export_archive_off_thread(test_manifest(true), Some(profile.clone()));
    tokio::pin!(export);

    tokio::select! {
        biased;
        () = tokio::task::yield_now() => {}
        result = &mut export => panic!(
            "archive packaging completed before the async runtime regained control: {result:?}"
        ),
    }

    let archive = export.await.unwrap();

    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive)).unwrap();
    let mut packaged_profile = Vec::new();
    zip.by_name(BROWSER_CONTEXT_PROFILE_ARCHIVE_PATH)
        .unwrap()
        .read_to_end(&mut packaged_profile)
        .unwrap();
    assert_eq!(packaged_profile, profile);
}

#[test]
fn rejects_outer_archive_over_limit() {
    let archive = build_browser_context_export_archive(&test_manifest(false), None).unwrap();
    let limits = BrowserContextImportArchiveLimits {
        max_archive_bytes: archive.len() as u64 - 1,
        ..test_limits()
    };

    let error = parse_browser_context_import_archive(&archive, limits).unwrap_err();

    assert!(error.to_string().contains("compressed size limit"));
}

#[test]
fn rejects_compressed_profile_over_limit() {
    let profile = profile_archive(&[("Default/Preferences", b"{}")]);
    let archive =
        build_browser_context_export_archive(&test_manifest(true), Some(&profile)).unwrap();
    let limits = BrowserContextImportArchiveLimits {
        max_profile_archive_bytes: profile.len() as u64 - 1,
        ..test_limits()
    };

    let error = parse_browser_context_import_archive(&archive, limits).unwrap_err();

    assert!(error.to_string().contains("compressed size limit"));
}

#[test]
fn rejects_profile_expansion_over_limit() {
    let profile = profile_archive(&[("Default/Preferences", &[b'x'; 4096])]);
    let archive =
        build_browser_context_export_archive(&test_manifest(true), Some(&profile)).unwrap();
    let limits = BrowserContextImportArchiveLimits {
        max_profile_uncompressed_bytes: 1024,
        ..test_limits()
    };

    let error = parse_browser_context_import_archive(&archive, limits).unwrap_err();

    assert!(error.to_string().contains("uncompressed size limit"));
}

#[test]
fn rejects_profile_entry_count_over_limit() {
    let profile = profile_archive(&[("one", b"1"), ("two", b"2")]);
    let archive =
        build_browser_context_export_archive(&test_manifest(true), Some(&profile)).unwrap();
    let limits = BrowserContextImportArchiveLimits {
        max_profile_entries: 1,
        ..test_limits()
    };

    let error = parse_browser_context_import_archive(&archive, limits).unwrap_err();

    assert!(error.to_string().contains("too many entries"));
}

#[test]
fn rejects_profile_links() {
    let profile = profile_archive_with_symlink();
    let archive =
        build_browser_context_export_archive(&test_manifest(true), Some(&profile)).unwrap();

    let error = parse_browser_context_import_archive(&archive, test_limits()).unwrap_err();

    assert!(error.to_string().contains("unsupported entry type"));
}

#[test]
fn rejects_hardlinks_devices_and_fifos() {
    for entry_type in [
        EntryType::Link,
        EntryType::Char,
        EntryType::Block,
        EntryType::Fifo,
    ] {
        let profile = profile_archive_with_special_entry(entry_type);
        let archive =
            build_browser_context_export_archive(&test_manifest(true), Some(&profile)).unwrap();

        let error = parse_browser_context_import_archive(&archive, test_limits()).unwrap_err();

        assert!(error.to_string().contains("unsupported entry type"));
    }
}

#[test]
fn rejects_parent_directory_paths() {
    let profile = profile_archive_with_raw_path(b"../escape");
    let archive =
        build_browser_context_export_archive(&test_manifest(true), Some(&profile)).unwrap();

    let error = parse_browser_context_import_archive(&archive, test_limits()).unwrap_err();

    assert!(error.to_string().contains("unsafe path"));
}

#[test]
fn rejects_absolute_paths() {
    let profile = profile_archive_with_raw_path(b"/escape");
    let archive =
        build_browser_context_export_archive(&test_manifest(true), Some(&profile)).unwrap();

    let error = parse_browser_context_import_archive(&archive, test_limits()).unwrap_err();

    assert!(error.to_string().contains("unsafe path"));
}

#[test]
fn rejects_profile_paths_over_limit() {
    let profile = profile_archive(&[("Default/Preferences", b"{}")]);
    let archive =
        build_browser_context_export_archive(&test_manifest(true), Some(&profile)).unwrap();
    let limits = BrowserContextImportArchiveLimits {
        max_profile_path_bytes: 5,
        ..test_limits()
    };

    let error = parse_browser_context_import_archive(&archive, limits).unwrap_err();

    assert!(error.to_string().contains("path is too long"));
}

fn test_limits() -> BrowserContextImportArchiveLimits {
    BrowserContextImportArchiveLimits {
        max_archive_bytes: 1024 * 1024,
        max_manifest_bytes: 128 * 1024,
        max_profile_archive_bytes: 1024 * 1024,
        max_profile_uncompressed_bytes: 1024 * 1024,
        max_profile_entries: 100,
        max_profile_path_bytes: 1024,
    }
}

fn test_manifest(with_profile: bool) -> BrowserContextExportManifest {
    let now = Utc::now();
    BrowserContextExportManifest {
        format_version: 1,
        archive_type: "browser_context_export".to_string(),
        exported_at: now,
        source_context: BrowserContextResource {
            id: Uuid::now_v7(),
            project_id: None,
            project: None,
            name: "test-context".to_string(),
            description: Some("test context".to_string()),
            labels: HashMap::new(),
            persistence_mode: BrowserContextPersistenceMode::Reusable,
            retention_sec: None,
            retention_expires_at: None,
            max_profile_storage_bytes: None,
            state: BrowserContextState::Ready,
            usage: BrowserContextUsageResource::default(),
            created_at: now,
            updated_at: now,
            last_used_at: None,
            deleted_at: None,
        },
        profile_archive_path: with_profile
            .then(|| BROWSER_CONTEXT_PROFILE_ARCHIVE_PATH.to_string()),
    }
}

fn profile_archive(files: &[(&str, &[u8])]) -> Vec<u8> {
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = Builder::new(encoder);
    for (path, content) in files {
        let mut header = Header::new_gnu();
        header.set_entry_type(EntryType::Regular);
        header.set_mode(0o600);
        header.set_size(content.len() as u64);
        header.set_cksum();
        builder.append_data(&mut header, path, *content).unwrap();
    }
    let encoder = builder.into_inner().unwrap();
    encoder.finish().unwrap()
}

fn profile_archive_with_symlink() -> Vec<u8> {
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = Builder::new(encoder);
    let mut header = Header::new_gnu();
    header.set_entry_type(EntryType::Symlink);
    header.set_mode(0o777);
    header.set_size(0);
    header.set_link_name("/etc/passwd").unwrap();
    header.set_cksum();
    builder
        .append_data(&mut header, "Default/unsafe-link", std::io::empty())
        .unwrap();
    let encoder = builder.into_inner().unwrap();
    encoder.finish().unwrap()
}

fn profile_archive_with_special_entry(entry_type: EntryType) -> Vec<u8> {
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = Builder::new(encoder);
    let mut header = Header::new_gnu();
    header.set_entry_type(entry_type);
    header.set_mode(0o600);
    header.set_size(0);
    if entry_type.is_hard_link() {
        header.set_link_name("Default/source").unwrap();
    }
    if entry_type.is_character_special() || entry_type.is_block_special() {
        header.set_device_major(1).unwrap();
        header.set_device_minor(3).unwrap();
    }
    header.set_cksum();
    builder
        .append_data(&mut header, "Default/special", std::io::empty())
        .unwrap();
    builder.into_inner().unwrap().finish().unwrap()
}

fn profile_archive_with_raw_path(path: &[u8]) -> Vec<u8> {
    assert!(path.len() < 100);
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = Builder::new(encoder);
    let mut header = Header::new_gnu();
    header.set_entry_type(EntryType::Regular);
    header.set_mode(0o600);
    header.set_size(0);
    header.as_mut_bytes()[..100].fill(0);
    header.as_mut_bytes()[..path.len()].copy_from_slice(path);
    header.set_cksum();
    builder.append(&header, std::io::empty()).unwrap();
    let encoder = builder.into_inner().unwrap();
    encoder.finish().unwrap()
}
