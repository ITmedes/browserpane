use super::bitrate::{compute_adapted_bitrate, DatagramStats};
use super::policy::{
    adapt_control_message_for_client, adapt_frame_for_client, adapt_frame_for_client_with_policy,
    client_can_forward_frame, viewer_can_forward_frame, viewer_can_receive_frame,
    SessionTransportPolicy,
};
use super::request::{
    extract_token, sanitized_request_path_for_log, validate_request_path, RequestValidationError,
    ValidatedConnectRequest,
};
use crate::auth::{AuthError, AuthValidator};
use crate::session_control::SessionCapabilities;
use bpane_protocol::channel::ChannelId;
use bpane_protocol::frame::Frame;
use bpane_protocol::{ClientAccessFlags, ControlMessage, SessionFlags};

mod bitrate;
mod policy;
mod request;
