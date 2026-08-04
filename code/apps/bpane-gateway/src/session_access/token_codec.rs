use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::Sha256;

type TokenHmacSha256 = Hmac<Sha256>;

const TOKEN_VERSION: &str = "v2";
const KEY_DERIVATION_CONTEXT: &str = "browserpane/token-domain/v2/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TokenPurpose {
    SessionConnect,
    SessionAutomation,
}

impl TokenPurpose {
    fn as_str(self) -> &'static str {
        match self {
            Self::SessionConnect => "session-connect",
            Self::SessionAutomation => "session-automation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PurposeBoundTokenError {
    Malformed,
    WrongPurpose,
    InvalidSignature,
    InvalidPayload,
}

#[derive(Debug, Clone)]
pub(super) struct PurposeBoundTokenCodec {
    purpose: TokenPurpose,
    signing_key: Vec<u8>,
}

impl PurposeBoundTokenCodec {
    pub(super) fn new(root_secret: &[u8], purpose: TokenPurpose) -> Self {
        let mut derivation = TokenHmacSha256::new_from_slice(root_secret)
            .expect("HMAC-SHA-256 accepts root secrets of any size");
        derivation.update(KEY_DERIVATION_CONTEXT.as_bytes());
        derivation.update(purpose.as_str().as_bytes());
        Self {
            purpose,
            signing_key: derivation.finalize().into_bytes().to_vec(),
        }
    }

    pub(super) fn encode<T: Serialize>(
        &self,
        claims: &T,
    ) -> Result<String, PurposeBoundTokenError> {
        let payload =
            serde_json::to_vec(claims).map_err(|_| PurposeBoundTokenError::InvalidPayload)?;
        let payload_encoded = URL_SAFE_NO_PAD.encode(payload);
        let signing_input = self.signing_input(&payload_encoded);
        let signature = self.sign(signing_input.as_bytes());
        Ok(format!(
            "{signing_input}.{}",
            URL_SAFE_NO_PAD.encode(signature)
        ))
    }

    pub(super) fn decode<T: DeserializeOwned>(
        &self,
        token: &str,
    ) -> Result<T, PurposeBoundTokenError> {
        let mut parts = token.split('.');
        let version = parts.next().ok_or(PurposeBoundTokenError::Malformed)?;
        let purpose = parts.next().ok_or(PurposeBoundTokenError::Malformed)?;
        let payload_encoded = parts.next().ok_or(PurposeBoundTokenError::Malformed)?;
        let signature_encoded = parts.next().ok_or(PurposeBoundTokenError::Malformed)?;
        if parts.next().is_some() || version != TOKEN_VERSION {
            return Err(PurposeBoundTokenError::Malformed);
        }
        if purpose != self.purpose.as_str() {
            return Err(PurposeBoundTokenError::WrongPurpose);
        }

        let signature = URL_SAFE_NO_PAD
            .decode(signature_encoded)
            .map_err(|_| PurposeBoundTokenError::Malformed)?;
        let signing_input = format!("{version}.{purpose}.{payload_encoded}");
        let mut verifier = TokenHmacSha256::new_from_slice(&self.signing_key)
            .expect("the derived HMAC-SHA-256 key has a valid size");
        verifier.update(signing_input.as_bytes());
        verifier
            .verify_slice(&signature)
            .map_err(|_| PurposeBoundTokenError::InvalidSignature)?;

        let payload = URL_SAFE_NO_PAD
            .decode(payload_encoded)
            .map_err(|_| PurposeBoundTokenError::Malformed)?;
        serde_json::from_slice(&payload).map_err(|_| PurposeBoundTokenError::InvalidPayload)
    }

    fn signing_input(&self, payload_encoded: &str) -> String {
        format!(
            "{TOKEN_VERSION}.{}.{payload_encoded}",
            self.purpose.as_str()
        )
    }

    fn sign(&self, input: &[u8]) -> Vec<u8> {
        let mut mac = TokenHmacSha256::new_from_slice(&self.signing_key)
            .expect("the derived HMAC-SHA-256 key has a valid size");
        mac.update(input);
        mac.finalize().into_bytes().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct Claims {
        subject: String,
    }

    #[test]
    fn encodes_and_decodes_a_purpose_bound_v2_token() {
        let codec = PurposeBoundTokenCodec::new(b"root-secret", TokenPurpose::SessionConnect);
        let token = codec
            .encode(&Claims {
                subject: "operator".to_string(),
            })
            .unwrap();

        assert!(token.starts_with("v2.session-connect."));
        assert_eq!(
            codec.decode::<Claims>(&token).unwrap(),
            Claims {
                subject: "operator".to_string()
            }
        );
    }

    #[test]
    fn rejects_wrong_purpose_and_purpose_substitution() {
        let connect = PurposeBoundTokenCodec::new(b"root-secret", TokenPurpose::SessionConnect);
        let automation =
            PurposeBoundTokenCodec::new(b"root-secret", TokenPurpose::SessionAutomation);
        let token = connect
            .encode(&Claims {
                subject: "operator".to_string(),
            })
            .unwrap();

        assert_eq!(
            automation.decode::<Claims>(&token),
            Err(PurposeBoundTokenError::WrongPurpose)
        );
        let substituted = token.replacen("session-connect", "session-automation", 1);
        assert_eq!(
            automation.decode::<Claims>(&substituted),
            Err(PurposeBoundTokenError::InvalidSignature)
        );
    }

    #[test]
    fn rejects_tampered_and_legacy_tokens() {
        let codec = PurposeBoundTokenCodec::new(b"root-secret", TokenPurpose::SessionConnect);
        let mut token = codec
            .encode(&Claims {
                subject: "operator".to_string(),
            })
            .unwrap();
        token.push('x');

        assert_eq!(
            codec.decode::<Claims>(&token),
            Err(PurposeBoundTokenError::InvalidSignature)
        );
        assert_eq!(
            codec.decode::<Claims>("v1.payload.signature"),
            Err(PurposeBoundTokenError::Malformed)
        );
    }
}
