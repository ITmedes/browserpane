mod model;
mod player;
mod prepare;

pub use model::{
    PreparedSessionRecordingPlayback, RecordingPlaybackError, SessionRecordingPlaybackManifest,
    SessionRecordingPlaybackResource,
};
pub use prepare::prepare_session_recording_playback;

#[cfg(test)]
pub(crate) use model::SessionRecordingPlaybackState;

#[cfg(test)]
mod tests;
