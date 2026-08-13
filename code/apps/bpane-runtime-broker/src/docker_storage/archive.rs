use std::io::Read;
use std::path::Component;

use flate2::read::GzDecoder;

use super::StorageRuntimeDockerConfig;

pub(super) fn validate_context_archive(
    bytes: &[u8],
    config: &StorageRuntimeDockerConfig,
) -> Result<(), &'static str> {
    if bytes.is_empty() || bytes.len() > config.max_payload_bytes {
        return Err("context archive size is invalid");
    }
    let decoder = GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|_| "context archive is invalid")?;
    let mut entry_count = 0_usize;
    let mut extracted_bytes = 0_u64;
    for entry in entries {
        let mut entry = entry.map_err(|_| "context archive entry is invalid")?;
        entry_count = entry_count
            .checked_add(1)
            .ok_or("context archive has too many entries")?;
        if entry_count > config.max_archive_entries {
            return Err("context archive has too many entries");
        }
        let kind = entry.header().entry_type();
        if !kind.is_file() && !kind.is_dir() {
            return Err("context archive entry type is not allowed");
        }
        let path = entry
            .path()
            .map_err(|_| "context archive path is invalid")?;
        let path = path.to_str().ok_or("context archive path must be UTF-8")?;
        if path.is_empty()
            || path.len() > config.max_archive_path_bytes
            || std::path::Path::new(path).components().any(|component| {
                matches!(
                    component,
                    Component::RootDir | Component::ParentDir | Component::Prefix(_)
                )
            })
        {
            return Err("context archive path is invalid");
        }
        if kind.is_file() {
            let mut sink = std::io::sink();
            let copied = std::io::copy(
                &mut entry
                    .by_ref()
                    .take(config.max_archive_uncompressed_bytes.saturating_add(1)),
                &mut sink,
            )
            .map_err(|_| "context archive payload is invalid")?;
            extracted_bytes = extracted_bytes
                .checked_add(copied)
                .ok_or("context archive payload is too large")?;
            if extracted_bytes > config.max_archive_uncompressed_bytes {
                return Err("context archive payload is too large");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use bpane_runtime_contract::ResourceLimits;
    use flate2::{write::GzEncoder, Compression};

    use super::*;

    fn config() -> StorageRuntimeDockerConfig {
        StorageRuntimeDockerConfig {
            image: "registry.example/browser@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            session_data_volume_prefix: "bpane-session-data".to_string(),
            browser_context_volume_prefix: "bpane-browser-context".to_string(),
            container_name_prefix: "bpane-storage-helper".to_string(),
            seccomp_profile: "default".to_string(),
            resources: ResourceLimits {
                memory_bytes: 256 * 1024 * 1024,
                cpu_millis: 1_000,
                pids: 64,
                shm_bytes: 16 * 1024 * 1024,
                timeout_secs: 30,
                output_limit_bytes: 1024 * 1024,
            },
            max_payload_bytes: 1024 * 1024,
            max_archive_entries: 10,
            max_archive_path_bytes: 256,
            max_archive_uncompressed_bytes: 1024,
        }
    }

    fn archive(path: &str, body: &[u8], entry_type: tar::EntryType) -> Vec<u8> {
        let encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut builder = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(entry_type);
        header.set_size(body.len() as u64);
        header.set_mode(0o600);
        header.set_cksum();
        builder.append_data(&mut header, path, body).unwrap();
        let encoder = builder.into_inner().unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn accepts_regular_bounded_archive() {
        let bytes = archive("Default/Cookies", b"cookies", tar::EntryType::Regular);
        validate_context_archive(&bytes, &config()).unwrap();
    }

    #[test]
    fn rejects_link_and_oversized_content() {
        let link = archive("Default/link", b"target", tar::EntryType::Symlink);
        assert!(validate_context_archive(&link, &config()).is_err());
        let large = archive("Default/Cookies", &[0_u8; 1_025], tar::EntryType::Regular);
        assert!(validate_context_archive(&large, &config()).is_err());
    }

    #[test]
    fn rejects_traversal_path() {
        let encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut builder = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.as_mut_bytes()[..9].copy_from_slice(b"../escape");
        header.set_size(1);
        header.set_mode(0o600);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        builder.append(&header, &b"x"[..]).unwrap();
        let bytes = builder.into_inner().unwrap().finish().unwrap();
        assert!(validate_context_archive(&bytes, &config()).is_err());
    }
}
