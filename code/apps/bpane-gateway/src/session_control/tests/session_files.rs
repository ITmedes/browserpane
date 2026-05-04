use super::support::principal;
use super::*;
use serde_json::json;

fn create_session_request() -> CreateSessionRequest {
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
    }
}

async fn create_workspace_file(
    store: &SessionStore,
    owner: &AuthenticatedPrincipal,
) -> StoredFileWorkspaceFile {
    let workspace = store
        .create_file_workspace(
            owner,
            PersistFileWorkspaceRequest {
                name: "inputs".to_string(),
                description: None,
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap();
    store
        .create_file_workspace_file_for_owner(
            owner,
            PersistFileWorkspaceFileRequest {
                id: Uuid::now_v7(),
                workspace_id: workspace.id,
                name: "input.csv".to_string(),
                media_type: Some("text/csv".to_string()),
                byte_count: 12,
                sha256_hex: "abc123".to_string(),
                provenance: Some(json!({ "source": "test" })),
                artifact_ref: "local_fs:workspace/input.csv".to_string(),
            },
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn in_memory_store_tracks_session_file_binding_lifecycle() {
    let store = SessionStore::in_memory();
    let owner = principal("owner");
    let session = store
        .create_session(
            &owner,
            create_session_request(),
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();
    let file = create_workspace_file(&store, &owner).await;

    let created = store
        .create_session_file_binding_for_owner(
            &owner,
            PersistSessionFileBindingRequest {
                id: Uuid::now_v7(),
                session_id: session.id,
                workspace_id: file.workspace_id,
                file_id: file.id,
                mount_path: "inputs/input.csv".to_string(),
                mode: SessionFileBindingMode::ReadOnly,
                labels: HashMap::from([("suite".to_string(), "unit".to_string())]),
            },
        )
        .await
        .unwrap();

    assert_eq!(created.session_id, session.id);
    assert_eq!(created.workspace_id, file.workspace_id);
    assert_eq!(created.file_id, file.id);
    assert_eq!(created.file_name, file.name);
    assert_eq!(created.state, SessionFileBindingState::Pending);
    assert_eq!(
        created.artifact_ref,
        "local_fs:workspace/input.csv".to_string()
    );

    let listed = store
        .list_session_file_bindings_for_session(session.id)
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);

    let fetched = store
        .get_session_file_binding_for_session(session.id, created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched.id, created.id);

    let removed = store
        .remove_session_file_binding_for_owner(&owner, session.id, created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(removed.state, SessionFileBindingState::Removed);

    let listed = store
        .list_session_file_bindings_for_session(session.id)
        .await
        .unwrap();
    assert!(listed.is_empty());
}

#[tokio::test]
async fn in_memory_store_scopes_session_file_bindings_to_owner() {
    let store = SessionStore::in_memory();
    let owner = principal("owner");
    let other = principal("other");
    let session = store
        .create_session(
            &owner,
            create_session_request(),
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();
    let file = create_workspace_file(&store, &owner).await;

    let error = store
        .create_session_file_binding_for_owner(
            &other,
            PersistSessionFileBindingRequest {
                id: Uuid::now_v7(),
                session_id: session.id,
                workspace_id: file.workspace_id,
                file_id: file.id,
                mount_path: "inputs/input.csv".to_string(),
                mode: SessionFileBindingMode::ReadOnly,
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap_err();

    assert!(matches!(error, SessionStoreError::NotFound(_)));
}

#[tokio::test]
async fn in_memory_store_rejects_unsafe_or_duplicate_session_file_mounts() {
    let store = SessionStore::in_memory();
    let owner = principal("owner");
    let session = store
        .create_session(
            &owner,
            create_session_request(),
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();
    let file = create_workspace_file(&store, &owner).await;

    let traversal_error = store
        .create_session_file_binding_for_owner(
            &owner,
            PersistSessionFileBindingRequest {
                id: Uuid::now_v7(),
                session_id: session.id,
                workspace_id: file.workspace_id,
                file_id: file.id,
                mount_path: "../input.csv".to_string(),
                mode: SessionFileBindingMode::ReadOnly,
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        traversal_error,
        SessionStoreError::InvalidRequest(_)
    ));

    store
        .create_session_file_binding_for_owner(
            &owner,
            PersistSessionFileBindingRequest {
                id: Uuid::now_v7(),
                session_id: session.id,
                workspace_id: file.workspace_id,
                file_id: file.id,
                mount_path: "inputs/input.csv".to_string(),
                mode: SessionFileBindingMode::ReadOnly,
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap();
    let duplicate_error = store
        .create_session_file_binding_for_owner(
            &owner,
            PersistSessionFileBindingRequest {
                id: Uuid::now_v7(),
                session_id: session.id,
                workspace_id: file.workspace_id,
                file_id: file.id,
                mount_path: "inputs/input.csv".to_string(),
                mode: SessionFileBindingMode::ReadOnly,
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(duplicate_error, SessionStoreError::Conflict(_)));
}
