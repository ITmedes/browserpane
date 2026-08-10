use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::session_access::token_codec::{
    PurposeBoundTokenCodec, PurposeBoundTokenError, TokenPurpose,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordingWorkerAccessTokenClaims {
    pub session_id: Uuid,
    pub recording_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedRecordingWorkerAccessToken {
    pub token: String,
    pub claims: RecordingWorkerAccessTokenClaims,
}

#[derive(Debug, Clone)]
pub struct RecordingWorkerAccessTokenManager {
    codec: PurposeBoundTokenCodec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordingWorkerAccessTokenError {
    Malformed,
    WrongPurpose,
    InvalidSignature,
    InvalidPayload,
}

impl std::fmt::Display for RecordingWorkerAccessTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed => write!(f, "malformed recording worker access token"),
            Self::WrongPurpose => write!(f, "wrong recording worker access token purpose"),
            Self::InvalidSignature => {
                write!(f, "invalid recording worker access token signature")
            }
            Self::InvalidPayload => write!(f, "invalid recording worker access token payload"),
        }
    }
}

impl std::error::Error for RecordingWorkerAccessTokenError {}

impl RecordingWorkerAccessTokenManager {
    pub fn new(secret: impl AsRef<[u8]>) -> Self {
        Self {
            codec: PurposeBoundTokenCodec::new(secret.as_ref(), TokenPurpose::RecordingWorker),
        }
    }

    pub fn issue_token(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
    ) -> Result<IssuedRecordingWorkerAccessToken, RecordingWorkerAccessTokenError> {
        let claims = RecordingWorkerAccessTokenClaims {
            session_id,
            recording_id,
        };
        Ok(IssuedRecordingWorkerAccessToken {
            token: self.codec.encode(&claims).map_err(map_codec_error)?,
            claims,
        })
    }

    pub fn validate_token(
        &self,
        token: &str,
    ) -> Result<RecordingWorkerAccessTokenClaims, RecordingWorkerAccessTokenError> {
        self.codec.decode(token).map_err(map_codec_error)
    }
}

fn map_codec_error(error: PurposeBoundTokenError) -> RecordingWorkerAccessTokenError {
    match error {
        PurposeBoundTokenError::Malformed => RecordingWorkerAccessTokenError::Malformed,
        PurposeBoundTokenError::WrongPurpose => RecordingWorkerAccessTokenError::WrongPurpose,
        PurposeBoundTokenError::InvalidSignature => {
            RecordingWorkerAccessTokenError::InvalidSignature
        }
        PurposeBoundTokenError::InvalidPayload => RecordingWorkerAccessTokenError::InvalidPayload,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issues_and_validates_session_and_recording_bound_token() {
        let manager = RecordingWorkerAccessTokenManager::new([9; 32]);
        let session_id = Uuid::now_v7();
        let recording_id = Uuid::now_v7();

        let issued = manager.issue_token(session_id, recording_id).unwrap();

        assert!(issued.token.starts_with("v2.recording-worker."));
        assert_eq!(
            manager.validate_token(&issued.token).unwrap(),
            issued.claims
        );
    }

    #[test]
    fn rejects_tampered_token() {
        let manager = RecordingWorkerAccessTokenManager::new([9; 32]);
        let mut issued = manager
            .issue_token(Uuid::now_v7(), Uuid::now_v7())
            .unwrap()
            .token;
        issued.push('x');

        assert_eq!(
            manager.validate_token(&issued),
            Err(RecordingWorkerAccessTokenError::InvalidSignature)
        );
    }
}
