use super::*;

#[tokio::test]
async fn in_memory_store_scopes_sessions_to_owner() {
    let store = SessionStore::in_memory();
    let alpha = principal("alpha");
    let bravo = principal("bravo");

    let created = store
        .create_session(
            &alpha,
            CreateSessionRequest {
                template_id: Some("default".to_string()),
                owner_mode: None,
                viewport: Some(SessionViewport {
                    width: 1920,
                    height: 1080,
                }),
                idle_timeout_sec: Some(600),
                labels: HashMap::from([("suite".to_string(), "smoke".to_string())]),
                integration_context: Some(serde_json::json!({ "ticket": "BPANE-6" })),
                extension_ids: Vec::new(),
                extensions: Vec::new(),
                recording: SessionRecordingPolicy {
                    mode: SessionRecordingMode::Manual,
                    format: SessionRecordingFormat::Webm,
                    retention_sec: Some(86_400),
                },
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();

    let alpha_sessions = store.list_sessions_for_owner(&alpha).await.unwrap();
    assert_eq!(alpha_sessions.len(), 1);
    assert_eq!(alpha_sessions[0].id, created.id);
    assert_eq!(alpha_sessions[0].recording, created.recording);
    assert_eq!(created.recording.mode, SessionRecordingMode::Manual);
    assert_eq!(created.recording.format, SessionRecordingFormat::Webm);
    assert_eq!(created.recording.retention_sec, Some(86_400));

    let bravo_sessions = store.list_sessions_for_owner(&bravo).await.unwrap();
    assert!(bravo_sessions.is_empty());
    assert!(store
        .get_session_for_owner(&bravo, created.id)
        .await
        .unwrap()
        .is_none());
}
