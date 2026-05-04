use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use bpane_protocol::channel::ChannelId;
use bpane_protocol::frame::Frame;
use bpane_protocol::FileMessage;
use sha2::{Digest, Sha256};
use tracing::warn;
use uuid::Uuid;

use crate::session_control::{PersistSessionFileRequest, SessionStore};
use crate::workspaces::{StoreWorkspaceFileRequest, WorkspaceFileStore};

use super::SessionFileSource;

#[derive(Debug)]
pub(crate) struct ActiveTransfer {
    name: String,
    media_type: Option<String>,
    expected_size: u64,
    received_size: u64,
    next_seq: u32,
    chunks: Vec<Vec<u8>>,
}

#[derive(Clone)]
pub(crate) struct SessionFileRecorder {
    session_id: Uuid,
    source: SessionFileSource,
    session_store: SessionStore,
    workspace_file_store: Arc<WorkspaceFileStore>,
}

impl SessionFileRecorder {
    pub(crate) fn new(
        session_id: Uuid,
        source: SessionFileSource,
        session_store: SessionStore,
        workspace_file_store: Arc<WorkspaceFileStore>,
    ) -> Self {
        Self {
            session_id,
            source,
            session_store,
            workspace_file_store,
        }
    }

    pub(crate) async fn observe_frame(
        &self,
        active: &mut HashMap<u32, ActiveTransfer>,
        frame: &Frame,
    ) -> anyhow::Result<()> {
        if frame.channel != self.channel() {
            return Ok(());
        }
        let message = FileMessage::decode_on_channel(&frame.payload, frame.channel)?;
        match message {
            FileMessage::FileHeader {
                id,
                filename,
                size,
                mime,
            } => {
                active.insert(
                    id,
                    ActiveTransfer {
                        name: sanitize_transfer_name(
                            &decode_fixed_string(filename.as_ref()),
                            &format!("session-file-{id}"),
                        ),
                        media_type: optional_fixed_string(mime.as_ref()),
                        expected_size: size,
                        received_size: 0,
                        next_seq: 0,
                        chunks: Vec::new(),
                    },
                );
            }
            FileMessage::FileChunk { id, seq, data } => {
                let Some(transfer) = active.get_mut(&id) else {
                    warn!(%self.session_id, id, seq, "received file chunk without header");
                    return Ok(());
                };
                if seq != transfer.next_seq {
                    warn!(
                        %self.session_id,
                        id,
                        expected_seq = transfer.next_seq,
                        received_seq = seq,
                        "dropping file transfer metadata after chunk sequence mismatch"
                    );
                    active.remove(&id);
                    return Ok(());
                }
                transfer.received_size = transfer.received_size.saturating_add(data.len() as u64);
                if transfer.received_size > transfer.expected_size {
                    warn!(
                        %self.session_id,
                        id,
                        expected_size = transfer.expected_size,
                        received_size = transfer.received_size,
                        "dropping file transfer metadata after size overflow"
                    );
                    active.remove(&id);
                    return Ok(());
                }
                transfer.next_seq = transfer.next_seq.wrapping_add(1);
                transfer.chunks.push(data);
            }
            FileMessage::FileComplete { id } => {
                let Some(transfer) = active.remove(&id) else {
                    warn!(%self.session_id, id, "received file completion without header");
                    return Ok(());
                };
                self.persist_completed_transfer(id, transfer).await?;
            }
        }
        Ok(())
    }

    fn channel(&self) -> ChannelId {
        match self.source {
            SessionFileSource::BrowserUpload => ChannelId::FileUp,
            SessionFileSource::BrowserDownload => ChannelId::FileDown,
        }
    }

    async fn persist_completed_transfer(
        &self,
        transfer_id: u32,
        transfer: ActiveTransfer,
    ) -> anyhow::Result<()> {
        if transfer.received_size != transfer.expected_size {
            warn!(
                %self.session_id,
                transfer_id,
                expected_size = transfer.expected_size,
                received_size = transfer.received_size,
                "dropping incomplete session file metadata"
            );
            return Ok(());
        }

        let bytes = transfer.chunks.concat();
        let file_id = Uuid::now_v7();
        let stored_artifact = self
            .workspace_file_store
            .write(StoreWorkspaceFileRequest {
                workspace_id: self.session_id,
                file_id,
                file_name: transfer.name.clone(),
                bytes: bytes.clone(),
            })
            .await?;
        let persisted = self
            .session_store
            .record_session_file(PersistSessionFileRequest {
                id: file_id,
                session_id: self.session_id,
                name: transfer.name,
                media_type: transfer.media_type,
                byte_count: bytes.len() as u64,
                sha256_hex: hex::encode(Sha256::digest(&bytes)),
                artifact_ref: stored_artifact.artifact_ref.clone(),
                source: self.source,
                labels: HashMap::new(),
            })
            .await;

        if let Err(error) = persisted {
            let _ = self
                .workspace_file_store
                .delete(&stored_artifact.artifact_ref)
                .await;
            return Err(error.into());
        }
        Ok(())
    }
}

pub(crate) fn new_active_transfer_map() -> HashMap<u32, ActiveTransfer> {
    HashMap::new()
}

fn decode_fixed_string(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

fn optional_fixed_string(bytes: &[u8]) -> Option<String> {
    let value = decode_fixed_string(bytes);
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn sanitize_transfer_name(input: &str, fallback: &str) -> String {
    let candidate = input.rsplit(['/', '\\']).next().unwrap_or(input).trim();
    let path = Path::new(candidate);
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(candidate)
        .trim();
    if name.is_empty() || name == "." || name == ".." {
        return fallback.to_string();
    }
    if name.starts_with('.') {
        return format!("_{name}");
    }
    name.chars()
        .map(|ch| if ch.is_control() { '_' } else { ch })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use tempfile::tempdir;

    use crate::auth::AuthenticatedPrincipal;
    use crate::session_control::{CreateSessionRequest, SessionOwnerMode, SessionRecordingPolicy};

    use super::*;

    #[test]
    fn sanitize_transfer_name_uses_basename_and_rejects_hidden_names() {
        assert_eq!(
            sanitize_transfer_name("../report.txt", "fallback"),
            "report.txt"
        );
        assert_eq!(sanitize_transfer_name("..", "fallback"), "fallback");
        assert_eq!(sanitize_transfer_name(".env", "fallback"), "_.env");
        assert_eq!(
            sanitize_transfer_name("line\nbreak.txt", "fallback"),
            "line_break.txt"
        );
    }

    #[tokio::test]
    async fn recorder_persists_completed_upload_metadata_and_content() {
        let store = SessionStore::in_memory();
        let owner = AuthenticatedPrincipal {
            subject: "owner".to_string(),
            issuer: "issuer".to_string(),
            display_name: None,
            client_id: None,
        };
        let session = store
            .create_session(
                &owner,
                CreateSessionRequest {
                    recording: SessionRecordingPolicy::default(),
                    ..CreateSessionRequest::default()
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();
        let temp = tempdir().unwrap();
        let file_store = Arc::new(WorkspaceFileStore::local_fs(temp.path().join("files")));
        let recorder = SessionFileRecorder::new(
            session.id,
            SessionFileSource::BrowserUpload,
            store.clone(),
            file_store.clone(),
        );
        let mut active = HashMap::new();

        recorder
            .observe_frame(
                &mut active,
                &FileMessage::header(
                    7,
                    fixed_string::<256>("upload.txt"),
                    11,
                    fixed_string::<64>("text/plain"),
                )
                .to_frame(ChannelId::FileUp),
            )
            .await
            .unwrap();
        recorder
            .observe_frame(
                &mut active,
                &FileMessage::chunk(7, 0, b"hello ".to_vec()).to_frame(ChannelId::FileUp),
            )
            .await
            .unwrap();
        recorder
            .observe_frame(
                &mut active,
                &FileMessage::chunk(7, 1, b"world".to_vec()).to_frame(ChannelId::FileUp),
            )
            .await
            .unwrap();
        recorder
            .observe_frame(
                &mut active,
                &FileMessage::complete(7).to_frame(ChannelId::FileUp),
            )
            .await
            .unwrap();

        let files = store
            .list_session_files_for_session(session.id)
            .await
            .unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "upload.txt");
        assert_eq!(files[0].source, SessionFileSource::BrowserUpload);
        assert_eq!(files[0].byte_count, 11);
        assert_eq!(
            file_store.read(&files[0].artifact_ref).await.unwrap(),
            b"hello world"
        );
    }

    fn fixed_string<const N: usize>(input: &str) -> [u8; N] {
        let mut out = [0u8; N];
        out[..input.len()].copy_from_slice(input.as_bytes());
        out
    }
}
