use std::io::{self, Cursor, Read, Write};
use std::num::NonZeroUsize;
use std::path::Component;
use std::sync::Arc;

use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use tar::Archive;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, TryAcquireError};
use zip::write::SimpleFileOptions;

use crate::session_control::{
    BrowserContextPersistenceMode, BrowserContextResource, BrowserContextState,
};

const BROWSER_CONTEXT_EXPORT_MANIFEST_PATH: &str = "manifest.json";
pub(super) const BROWSER_CONTEXT_PROFILE_ARCHIVE_PATH: &str = "profile.tar.gz";
const PROFILE_EXPANSION_LIMIT_ERROR: &str =
    "browser context profile archive exceeds the uncompressed size limit";

#[derive(Debug, thiserror::Error)]
pub(super) enum BrowserContextImportArchiveError {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    LimitExceeded(String),
}

impl BrowserContextImportArchiveError {
    pub(super) fn is_limit_exceeded(&self) -> bool {
        matches!(self, Self::LimitExceeded(_))
    }
}

type ImportArchiveResult<T> = Result<T, BrowserContextImportArchiveError>;

#[derive(Debug, Clone, Copy)]
pub(crate) struct BrowserContextImportArchiveLimits {
    pub(crate) max_archive_bytes: u64,
    pub(crate) max_manifest_bytes: u64,
    pub(crate) max_profile_archive_bytes: u64,
    pub(crate) max_profile_uncompressed_bytes: u64,
    pub(crate) max_profile_entries: usize,
    pub(crate) max_profile_path_bytes: usize,
}

impl Default for BrowserContextImportArchiveLimits {
    fn default() -> Self {
        Self {
            max_archive_bytes: 512 * 1024 * 1024,
            max_manifest_bytes: 128 * 1024,
            max_profile_archive_bytes: 512 * 1024 * 1024,
            max_profile_uncompressed_bytes: 2 * 1024 * 1024 * 1024,
            max_profile_entries: 100_000,
            max_profile_path_bytes: 4096,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BrowserContextImportService {
    limits: BrowserContextImportArchiveLimits,
    permits: Arc<Semaphore>,
}

impl BrowserContextImportService {
    pub(crate) fn new(
        limits: BrowserContextImportArchiveLimits,
        max_concurrent_imports: NonZeroUsize,
    ) -> Self {
        Self {
            limits,
            permits: Arc::new(Semaphore::new(max_concurrent_imports.get())),
        }
    }

    pub(crate) fn limits(&self) -> BrowserContextImportArchiveLimits {
        self.limits
    }

    pub(super) fn try_acquire(&self) -> Result<OwnedSemaphorePermit, TryAcquireError> {
        self.permits.clone().try_acquire_owned()
    }
}

impl Default for BrowserContextImportService {
    fn default() -> Self {
        Self::new(
            BrowserContextImportArchiveLimits::default(),
            NonZeroUsize::new(2).expect("default browser context import capacity must be non-zero"),
        )
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct BrowserContextExportManifest {
    pub format_version: u32,
    pub archive_type: String,
    pub exported_at: chrono::DateTime<chrono::Utc>,
    pub source_context: BrowserContextResource,
    pub profile_archive_path: Option<String>,
}

#[derive(Debug)]
pub(super) struct ParsedBrowserContextImportArchive {
    pub manifest: BrowserContextExportManifest,
    pub profile_archive: Option<Vec<u8>>,
}

pub(super) fn build_browser_context_export_archive(
    manifest: &BrowserContextExportManifest,
    profile_archive: Option<&[u8]>,
) -> Result<Vec<u8>, String> {
    let manifest_json = serde_json::to_vec_pretty(manifest).map_err(|error| error.to_string())?;
    let cursor = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cursor);
    let file_options =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file(BROWSER_CONTEXT_EXPORT_MANIFEST_PATH, file_options)
        .map_err(|error| error.to_string())?;
    zip.write_all(&manifest_json)
        .map_err(|error| error.to_string())?;
    if let Some(profile_archive) = profile_archive {
        zip.start_file(BROWSER_CONTEXT_PROFILE_ARCHIVE_PATH, file_options)
            .map_err(|error| error.to_string())?;
        zip.write_all(profile_archive)
            .map_err(|error| error.to_string())?;
    }

    let cursor = zip.finish().map_err(|error| error.to_string())?;
    Ok(cursor.into_inner())
}

pub(super) fn parse_browser_context_import_archive(
    bytes: &[u8],
    limits: BrowserContextImportArchiveLimits,
) -> ImportArchiveResult<ParsedBrowserContextImportArchive> {
    validate_outer_archive_size(bytes, limits.max_archive_bytes)?;
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|error| {
        invalid(format!(
            "browser context import archive must be a valid zip: {error}"
        ))
    })?;
    let profile_count = validate_outer_archive_entries(&mut zip)?;

    let manifest_bytes = {
        let manifest_file = zip
            .by_name(BROWSER_CONTEXT_EXPORT_MANIFEST_PATH)
            .map_err(|error| {
                invalid(format!(
                    "browser context import archive is missing manifest.json: {error}"
                ))
            })?;
        read_limited(
            manifest_file,
            limits.max_manifest_bytes,
            "browser context import manifest is too large",
        )?
    };
    let manifest = serde_json::from_slice::<BrowserContextExportManifest>(&manifest_bytes)
        .map_err(|error| {
            invalid(format!(
                "browser context import manifest is invalid JSON: {error}"
            ))
        })?;
    validate_browser_context_import_manifest(&manifest, profile_count > 0)?;

    let profile_archive = if manifest.profile_archive_path.is_some() {
        let profile_file = zip
            .by_name(BROWSER_CONTEXT_PROFILE_ARCHIVE_PATH)
            .map_err(|error| {
                invalid(format!(
                    "browser context import archive is missing profile.tar.gz: {error}"
                ))
            })?;
        let profile_bytes = read_limited(
            profile_file,
            limits.max_profile_archive_bytes,
            "browser context profile archive exceeds the compressed size limit",
        )?;
        if profile_bytes.is_empty() {
            return Err(invalid("browser context profile archive must not be empty"));
        }
        validate_profile_archive(&profile_bytes, limits)?;
        Some(profile_bytes)
    } else {
        None
    };

    Ok(ParsedBrowserContextImportArchive {
        manifest,
        profile_archive,
    })
}

fn validate_outer_archive_size(bytes: &[u8], max_bytes: u64) -> ImportArchiveResult<()> {
    if bytes.is_empty() {
        return Err(invalid("browser context import archive must not be empty"));
    }
    if bytes.len() as u64 > max_bytes {
        return Err(limit(
            "browser context import archive exceeds the compressed size limit",
        ));
    }
    Ok(())
}

fn validate_outer_archive_entries<R: Read + io::Seek>(
    zip: &mut zip::ZipArchive<R>,
) -> ImportArchiveResult<u32> {
    let mut manifest_count = 0_u32;
    let mut profile_count = 0_u32;
    for index in 0..zip.len() {
        let file = zip.by_index(index).map_err(|error| {
            invalid(format!(
                "failed to read browser context archive entry: {error}"
            ))
        })?;
        if file.is_dir() {
            return Err(invalid(format!(
                "browser context import archive contains unsupported directory entry {}",
                file.name()
            )));
        }
        match file.name() {
            BROWSER_CONTEXT_EXPORT_MANIFEST_PATH => manifest_count += 1,
            BROWSER_CONTEXT_PROFILE_ARCHIVE_PATH => profile_count += 1,
            other => {
                return Err(invalid(format!(
                    "browser context import archive contains unsupported entry {other}"
                )));
            }
        }
    }
    if manifest_count != 1 {
        return Err(invalid(
            "browser context import archive must contain exactly one manifest.json",
        ));
    }
    if profile_count > 1 {
        return Err(invalid(
            "browser context import archive must contain at most one profile.tar.gz",
        ));
    }
    Ok(profile_count)
}

fn read_limited(
    reader: impl Read,
    max_bytes: u64,
    limit_error: &str,
) -> ImportArchiveResult<Vec<u8>> {
    let read_limit = max_bytes.saturating_add(1);
    let mut bytes = Vec::new();
    reader
        .take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|error| invalid(error.to_string()))?;
    if bytes.len() as u64 > max_bytes {
        return Err(limit(limit_error));
    }
    Ok(bytes)
}

fn validate_profile_archive(
    bytes: &[u8],
    limits: BrowserContextImportArchiveLimits,
) -> ImportArchiveResult<()> {
    let decoder = GzDecoder::new(bytes);
    let limited_decoder = ByteLimitReader::new(decoder, limits.max_profile_uncompressed_bytes);
    let mut archive = Archive::new(limited_decoder);
    let entries = archive.entries().map_err(profile_archive_error)?;
    let mut entry_count = 0_usize;

    for entry in entries {
        entry_count = entry_count.saturating_add(1);
        if entry_count > limits.max_profile_entries {
            return Err(limit(
                "browser context profile archive contains too many entries",
            ));
        }
        let mut entry = entry.map_err(profile_archive_error)?;
        validate_profile_entry(&entry, limits.max_profile_path_bytes)?;
        io::copy(&mut entry, &mut io::sink()).map_err(profile_archive_error)?;
    }
    Ok(())
}

fn validate_profile_entry<R: Read>(
    entry: &tar::Entry<'_, R>,
    max_path_bytes: usize,
) -> ImportArchiveResult<()> {
    let path = entry.path().map_err(profile_archive_error)?;
    if path.as_os_str().as_encoded_bytes().len() > max_path_bytes {
        return Err(limit(
            "browser context profile archive entry path is too long",
        ));
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(invalid(format!(
            "browser context profile archive contains unsafe path {}",
            path.display()
        )));
    }

    let entry_type = entry.header().entry_type();
    if !entry_type.is_file() && !entry_type.is_dir() {
        return Err(invalid(format!(
            "browser context profile archive contains unsupported entry type for {}",
            path.display()
        )));
    }
    Ok(())
}

fn validate_browser_context_import_manifest(
    manifest: &BrowserContextExportManifest,
    archive_contains_profile: bool,
) -> ImportArchiveResult<()> {
    if manifest.format_version != 1 {
        return Err(invalid(format!(
            "unsupported browser context export format version {}",
            manifest.format_version
        )));
    }
    if manifest.archive_type != "browser_context_export" {
        return Err(invalid(format!(
            "unsupported browser context archive type {}",
            manifest.archive_type
        )));
    }
    if manifest.source_context.persistence_mode != BrowserContextPersistenceMode::Reusable {
        return Err(invalid("browser context import source must be reusable"));
    }
    if manifest.source_context.state != BrowserContextState::Ready {
        return Err(invalid("browser context import source must be ready"));
    }
    match manifest.profile_archive_path.as_deref() {
        Some(BROWSER_CONTEXT_PROFILE_ARCHIVE_PATH) if archive_contains_profile => Ok(()),
        Some(BROWSER_CONTEXT_PROFILE_ARCHIVE_PATH) => Err(invalid(
            "browser context import manifest references profile.tar.gz but the archive is missing it"
        )),
        Some(path) => Err(invalid(format!(
            "unsupported browser context profile archive path {path}"
        ))),
        None if archive_contains_profile => Err(invalid(
            "browser context import archive contains profile.tar.gz but the manifest does not reference it"
        )),
        None => Ok(()),
    }
}

fn profile_archive_error(error: io::Error) -> BrowserContextImportArchiveError {
    if error.to_string().contains(PROFILE_EXPANSION_LIMIT_ERROR) {
        limit(PROFILE_EXPANSION_LIMIT_ERROR)
    } else {
        invalid(format!(
            "browser context profile archive is invalid: {error}"
        ))
    }
}

fn invalid(message: impl Into<String>) -> BrowserContextImportArchiveError {
    BrowserContextImportArchiveError::Invalid(message.into())
}

fn limit(message: impl Into<String>) -> BrowserContextImportArchiveError {
    BrowserContextImportArchiveError::LimitExceeded(message.into())
}

struct ByteLimitReader<R> {
    inner: R,
    remaining: u64,
}

impl<R> ByteLimitReader<R> {
    fn new(inner: R, max_bytes: u64) -> Self {
        Self {
            inner,
            remaining: max_bytes,
        }
    }
}

impl<R: Read> Read for ByteLimitReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            let mut probe = [0_u8; 1];
            return match self.inner.read(&mut probe)? {
                0 => Ok(0),
                _ => Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    PROFILE_EXPANSION_LIMIT_ERROR,
                )),
            };
        }
        let allowed = usize::try_from(self.remaining)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let read = self.inner.read(&mut buffer[..allowed])?;
        self.remaining = self.remaining.saturating_sub(read as u64);
        Ok(read)
    }
}

#[cfg(test)]
mod tests;
