use super::*;

#[tokio::test]
async fn in_memory_store_allows_delegated_client_to_load_session() {
    let store = SessionStore::in_memory();
    let owner = principal("owner");
    let delegate = service_principal("service-account-id", "bpane-mcp-bridge");

    let created = store
        .create_session(
            &owner,
            CreateSessionRequest {
                template_id: None,
                owner_mode: None,
                viewport: None,
                idle_timeout_sec: None,
                labels: HashMap::new(),
                integration_context: None,
                extension_ids: Vec::new(),
                extensions: Vec::new(),
                recording: SessionRecordingPolicy::default(),
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();

    let updated = store
        .set_automation_delegate_for_owner(
            &owner,
            created.id,
            SetAutomationDelegateRequest {
                client_id: "bpane-mcp-bridge".to_string(),
                issuer: None,
                display_name: Some("BrowserPane MCP bridge".to_string()),
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        updated.automation_delegate.as_ref().unwrap().client_id,
        "bpane-mcp-bridge"
    );

    let visible = store
        .get_session_for_principal(&delegate, created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(visible.id, created.id);
}
