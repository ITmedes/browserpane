use std::sync::Arc;

use bpane_protocol::{ControlMessage, SessionFlags};
use tokio::io::AsyncWriteExt;
use tokio::net::UnixListener;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::super::negotiation::ConnectionProtocol;
use super::super::policy::SessionTransportPolicy;
use super::{send_initial_frames_or_leave, InitialFramesContext};
use crate::session_hub::BrowserClientRole;
use crate::session_registry::SessionRegistry;

#[tokio::test]
async fn bootstrap_write_failure_removes_admitted_registry_client() {
    let directory = tempfile::tempdir().unwrap();
    let socket_path = directory.path().join("agent.sock");
    let listener = UnixListener::bind(&socket_path).unwrap();
    let agent = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let ready = ControlMessage::SessionReady {
            version: 1,
            flags: SessionFlags::empty(),
        }
        .to_frame()
        .encode();
        stream.write_all(&ready).await.unwrap();
        std::future::pending::<()>().await;
    });

    let registry = Arc::new(SessionRegistry::new(10, false));
    let routed_session_id = Uuid::now_v7();
    let (client, hub) = registry
        .join_with_role(
            routed_session_id,
            socket_path.to_str().unwrap(),
            BrowserClientRole::Interactive,
            true,
        )
        .await
        .unwrap();
    assert_eq!(hub.client_count(), 1);

    let (writer, reader) = tokio::io::duplex(64);
    drop(reader);
    let send_stream = Arc::new(Mutex::new(writer));
    let initial_frames = vec![Arc::new(
        ControlMessage::SessionReady {
            version: 1,
            flags: SessionFlags::empty(),
        }
        .to_frame(),
    )];

    let result = send_initial_frames_or_leave(
        &send_stream,
        &initial_frames,
        InitialFramesContext {
            joined_as_owner: client.is_owner,
            initial_access_state: client.initial_access_state,
            policy: SessionTransportPolicy::default(),
            protocol: ConnectionProtocol::Legacy,
            session_id: 1,
            client_id: client.client_id,
        },
        &registry,
        routed_session_id,
    )
    .await;

    assert!(result.is_err());
    assert_eq!(hub.client_count(), 0);
    agent.abort();
}
