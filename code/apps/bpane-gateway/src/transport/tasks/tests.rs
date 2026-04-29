use super::recorder_role_suppresses_bitrate_feedback;
use crate::session_hub::BrowserClientRole;

#[test]
fn recorder_role_disables_bitrate_feedback() {
    assert!(!recorder_role_suppresses_bitrate_feedback(
        BrowserClientRole::Interactive
    ));
    assert!(recorder_role_suppresses_bitrate_feedback(
        BrowserClientRole::Recorder
    ));
}
