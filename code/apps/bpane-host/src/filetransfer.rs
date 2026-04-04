use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::Context;
use tokio::fs::{self, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use bpane_protocol::channel::ChannelId;
use bpane_protocol::frame::Frame;
use bpane_protocol::FileMessage;

const FILE_CHUNK_SIZE: usize = 64 * 1024;
const DOWNLOAD_POLL_INTERVAL: Duration = Duration::from_millis(500);
const DOWNLOAD_STABLE_POLLS_REQUIRED: u8 = 2;

pub struct FileTransferState {
    upload_dir: PathBuf,
    download_dir: PathBuf,
    active_uploads: HashMap<u32, UploadState>,
}

struct UploadState {
    path: PathBuf,
    file: fs::File,
    filename: String,
    expected_size: u64,
    received: u64,
    next_seq: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileSignature {
    size: u64,
    modified_ms: u128,
}

#[derive(Clone, Debug)]
struct DownloadTracker {
    signature: FileSignature,
    stable_polls: u8,
    sent_signature: Option<FileSignature>,
}

impl FileTransferState {
    pub async fn from_env() -> anyhow::Result<Self> {
        let upload_base = transfer_root_from_env("BPANE_UPLOAD_DIR", "bpane-uploads");
        let session_upload_dir = upload_base.join(format!("session-{}", unix_time_ms_now()));
        let download_dir = transfer_root_from_env("BPANE_DOWNLOAD_DIR", "bpane-downloads");

        fs::create_dir_all(&session_upload_dir)
            .await
            .with_context(|| format!("create upload dir {}", session_upload_dir.display()))?;
        fs::create_dir_all(&download_dir)
            .await
            .with_context(|| format!("create download dir {}", download_dir.display()))?;

        info!(
            upload_dir = %session_upload_dir.display(),
            download_dir = %download_dir.display(),
            "file transfer directories ready"
        );

        Ok(Self {
            upload_dir: session_upload_dir,
            download_dir,
            active_uploads: HashMap::new(),
        })
    }

    pub fn upload_dir(&self) -> &Path {
        &self.upload_dir
    }

    pub fn download_dir(&self) -> &Path {
        &self.download_dir
    }

    pub async fn handle_upload_message(&mut self, message: FileMessage) -> anyhow::Result<()> {
        match message {
            FileMessage::FileHeader {
                id, filename, size, ..
            } => self.handle_upload_header(id, &filename, size).await,
            FileMessage::FileChunk { id, seq, data } => {
                self.handle_upload_chunk(id, seq, &data).await
            }
            FileMessage::FileComplete { id } => self.handle_upload_complete(id).await,
        }
    }

    async fn handle_upload_header(
        &mut self,
        id: u32,
        filename_buf: &[u8; 256],
        size: u64,
    ) -> anyhow::Result<()> {
        if let Some(existing) = self.active_uploads.remove(&id) {
            discard_upload(existing).await;
        }

        let requested_name = decode_fixed_string(filename_buf);
        let safe_name = sanitize_filename(&requested_name, &format!("upload-{id}"));
        let path = unique_upload_path(&self.upload_dir, &safe_name);
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .await
            .with_context(|| format!("create upload file {}", path.display()))?;

        self.active_uploads.insert(
            id,
            UploadState {
                path: path.clone(),
                file,
                filename: safe_name.clone(),
                expected_size: size,
                received: 0,
                next_seq: 0,
            },
        );

        debug!(
            id,
            filename = %safe_name,
            size,
            path = %path.display(),
            "started file upload"
        );
        Ok(())
    }

    async fn handle_upload_chunk(&mut self, id: u32, seq: u32, data: &[u8]) -> anyhow::Result<()> {
        let Some(state) = self.active_uploads.get_mut(&id) else {
            warn!(id, seq, "received upload chunk without header");
            return Ok(());
        };

        if seq != state.next_seq {
            warn!(
                id,
                expected_seq = state.next_seq,
                received_seq = seq,
                "upload chunk sequence mismatch; dropping partial file"
            );
            if let Some(state) = self.active_uploads.remove(&id) {
                discard_upload(state).await;
            }
            return Ok(());
        }

        state
            .file
            .write_all(data)
            .await
            .with_context(|| format!("write upload chunk to {}", state.path.display()))?;
        state.received = state.received.saturating_add(data.len() as u64);
        state.next_seq = state.next_seq.wrapping_add(1);
        Ok(())
    }

    async fn handle_upload_complete(&mut self, id: u32) -> anyhow::Result<()> {
        let Some(mut state) = self.active_uploads.remove(&id) else {
            warn!(id, "received upload completion without active file");
            return Ok(());
        };

        state
            .file
            .flush()
            .await
            .with_context(|| format!("flush upload {}", state.path.display()))?;

        if state.received != state.expected_size {
            warn!(
                id,
                filename = %state.filename,
                expected_size = state.expected_size,
                received_size = state.received,
                path = %state.path.display(),
                "upload completed with size mismatch"
            );
        } else {
            info!(
                id,
                filename = %state.filename,
                size = state.received,
                path = %state.path.display(),
                "upload completed"
            );
        }

        Ok(())
    }
}

pub fn spawn_download_watcher(
    download_dir: PathBuf,
    to_gateway: mpsc::Sender<Frame>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut trackers: HashMap<PathBuf, DownloadTracker> = HashMap::new();
        let mut next_download_id: u32 = 1;
        let session_started_at = SystemTime::now();
        let mut ticker = tokio::time::interval(DOWNLOAD_POLL_INTERVAL);

        loop {
            ticker.tick().await;

            let mut read_dir = match fs::read_dir(&download_dir).await {
                Ok(read_dir) => read_dir,
                Err(e) => {
                    warn!(
                        dir = %download_dir.display(),
                        error = %e,
                        "could not read download directory"
                    );
                    continue;
                }
            };

            let mut active_paths = Vec::new();

            loop {
                let entry = match read_dir.next_entry().await {
                    Ok(Some(entry)) => entry,
                    Ok(None) => break,
                    Err(e) => {
                        warn!(
                            dir = %download_dir.display(),
                            error = %e,
                            "could not iterate download directory"
                        );
                        break;
                    }
                };

                let path = entry.path();
                let name = entry.file_name();
                let name = name.to_string_lossy();

                if should_skip_download(&name) {
                    continue;
                }

                let metadata = match entry.metadata().await {
                    Ok(metadata) if metadata.is_file() => metadata,
                    Ok(_) => continue,
                    Err(e) => {
                        warn!(path = %path.display(), error = %e, "could not stat download file");
                        continue;
                    }
                };

                let modified = match metadata.modified() {
                    Ok(modified) => modified,
                    Err(e) => {
                        warn!(path = %path.display(), error = %e, "could not read download mtime");
                        continue;
                    }
                };

                if modified < session_started_at {
                    continue;
                }

                active_paths.push(path.clone());
                let signature = FileSignature {
                    size: metadata.len(),
                    modified_ms: system_time_ms(modified),
                };
                let tracker = trackers.entry(path.clone()).or_insert(DownloadTracker {
                    signature,
                    stable_polls: 0,
                    sent_signature: None,
                });

                if tracker.signature == signature {
                    tracker.stable_polls = tracker.stable_polls.saturating_add(1);
                } else {
                    tracker.signature = signature;
                    tracker.stable_polls = 0;
                }

                if tracker.stable_polls < DOWNLOAD_STABLE_POLLS_REQUIRED {
                    continue;
                }
                if tracker.sent_signature == Some(signature) {
                    continue;
                }

                match stream_download_file(&path, signature.size, next_download_id, &to_gateway)
                    .await
                {
                    Ok(()) => {
                        tracker.sent_signature = Some(signature);
                        next_download_id = next_download_id.wrapping_add(1).max(1);
                    }
                    Err(e) => {
                        warn!(path = %path.display(), error = %e, "download forwarding failed");
                    }
                }
            }

            trackers.retain(|path, _| active_paths.iter().any(|active| active == path));
        }
    })
}

fn should_skip_download(name: &str) -> bool {
    name.is_empty()
        || name.starts_with('.')
        || name.ends_with(".crdownload")
        || name.ends_with(".tmp")
        || name.ends_with(".part")
}

async fn stream_download_file(
    path: &Path,
    size: u64,
    id: u32,
    to_gateway: &mpsc::Sender<Frame>,
) -> anyhow::Result<()> {
    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| sanitize_filename(value, &format!("download-{id}")))
        .unwrap_or_else(|| format!("download-{id}"));
    let mime = default_download_mime(path);

    to_gateway
        .send(
            FileMessage::FileHeader {
                id,
                filename: encode_fixed_string::<256>(&filename),
                size,
                mime: encode_fixed_string::<64>(&mime),
            }
            .to_frame(ChannelId::FileDown),
        )
        .await
        .context("send download header")?;

    let mut file = fs::File::open(path)
        .await
        .with_context(|| format!("open download file {}", path.display()))?;
    let mut seq: u32 = 0;
    let mut buffer = vec![0u8; FILE_CHUNK_SIZE];

    loop {
        let read = file
            .read(&mut buffer)
            .await
            .with_context(|| format!("read download file {}", path.display()))?;
        if read == 0 {
            break;
        }

        to_gateway
            .send(
                FileMessage::FileChunk {
                    id,
                    seq,
                    data: buffer[..read].to_vec(),
                }
                .to_frame(ChannelId::FileDown),
            )
            .await
            .with_context(|| format!("send download chunk for {}", path.display()))?;
        seq = seq.wrapping_add(1);
    }

    to_gateway
        .send(FileMessage::FileComplete { id }.to_frame(ChannelId::FileDown))
        .await
        .context("send download completion")?;

    info!(
        id,
        filename = %filename,
        size,
        path = %path.display(),
        "download forwarded to client"
    );
    Ok(())
}

async fn discard_upload(state: UploadState) {
    let path = state.path.clone();
    drop(state);
    if let Err(e) = fs::remove_file(&path).await {
        warn!(path = %path.display(), error = %e, "could not remove partial upload");
    }
}

fn transfer_root_from_env(env_name: &str, default_leaf: &str) -> PathBuf {
    std::env::var_os(env_name)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/home/bpane"))
                .join(default_leaf)
        })
}

fn decode_fixed_string(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

fn sanitize_filename(input: &str, fallback: &str) -> String {
    let candidate = input.rsplit(['/', '\\']).next().unwrap_or(input).trim();
    let mut sanitized = String::new();

    for ch in candidate.chars() {
        if ch.is_control() || matches!(ch, '/' | '\\' | '\0') {
            sanitized.push('_');
        } else {
            sanitized.push(ch);
        }
    }

    if sanitized.is_empty() || sanitized.chars().all(|ch| ch == '.') {
        return fallback.to_string();
    }
    if sanitized.starts_with('.') {
        sanitized.insert(0, '_');
    }

    utf8_truncate(sanitized, 200)
}

fn unique_upload_path(upload_dir: &Path, filename: &str) -> PathBuf {
    let candidate = upload_dir.join(filename);
    if !candidate.exists() {
        return candidate;
    }

    let stem = Path::new(filename)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("upload");
    let ext = Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();

    for idx in 1..10_000u32 {
        let name = utf8_truncate(format!("{stem}-{idx}{ext}"), 200);
        let path = upload_dir.join(name);
        if !path.exists() {
            return path;
        }
    }

    upload_dir.join(format!("upload-{}", unix_time_ms_now()))
}

fn utf8_truncate(input: String, max_bytes: usize) -> String {
    if input.len() <= max_bytes {
        return input;
    }

    let mut out = String::new();
    for ch in input.chars() {
        if out.len() + ch.len_utf8() > max_bytes {
            break;
        }
        out.push(ch);
    }
    out
}

fn encode_fixed_string<const N: usize>(input: &str) -> [u8; N] {
    let mut out = [0u8; N];
    let mut offset = 0usize;

    for ch in input.chars() {
        let mut buf = [0u8; 4];
        let encoded = ch.encode_utf8(&mut buf).as_bytes();
        if offset + encoded.len() > N {
            break;
        }
        out[offset..offset + encoded.len()].copy_from_slice(encoded);
        offset += encoded.len();
    }

    out
}

fn default_download_mime(path: &Path) -> String {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("txt") => "text/plain",
        Some("csv") => "text/csv",
        Some("json") => "application/json",
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("zip") => "application/zip",
        Some("mp3") => "audio/mpeg",
        Some("mp4") => "video/mp4",
        _ => "application/octet-stream",
    }
    .to_string()
}

fn system_time_ms(value: SystemTime) -> u128 {
    value
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn unix_time_ms_now() -> u128 {
    system_time_ms(SystemTime::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn sanitize_filename_rejects_paths_and_hidden_names() {
        assert_eq!(sanitize_filename("../bad.txt", "fallback"), "bad.txt");
        assert_eq!(sanitize_filename("..", "fallback"), "fallback");
        assert_eq!(sanitize_filename(".secret", "fallback"), "_.secret");
        assert_eq!(
            sanitize_filename("line\nbreak.txt", "fallback"),
            "line_break.txt"
        );
    }

    #[tokio::test]
    async fn upload_messages_write_expected_file() {
        let temp = tempdir().unwrap();
        let upload_dir = temp.path().join("uploads");
        let download_dir = temp.path().join("downloads");
        fs::create_dir_all(&upload_dir).await.unwrap();
        fs::create_dir_all(&download_dir).await.unwrap();

        let mut state = FileTransferState {
            upload_dir: upload_dir.clone(),
            download_dir,
            active_uploads: HashMap::new(),
        };

        state
            .handle_upload_message(FileMessage::FileHeader {
                id: 7,
                filename: encode_fixed_string::<256>("report.txt"),
                size: 11,
                mime: encode_fixed_string::<64>("text/plain"),
            })
            .await
            .unwrap();
        state
            .handle_upload_message(FileMessage::FileChunk {
                id: 7,
                seq: 0,
                data: b"hello ".to_vec(),
            })
            .await
            .unwrap();
        state
            .handle_upload_message(FileMessage::FileChunk {
                id: 7,
                seq: 1,
                data: b"world".to_vec(),
            })
            .await
            .unwrap();
        state
            .handle_upload_message(FileMessage::FileComplete { id: 7 })
            .await
            .unwrap();

        let written = fs::read(upload_dir.join("report.txt")).await.unwrap();
        assert_eq!(written, b"hello world");
    }
}
