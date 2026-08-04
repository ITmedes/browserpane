use anyhow::{ensure, Context};

use super::*;

pub(super) async fn run_resource_contracts(store: &SessionStore) -> anyhow::Result<()> {
    let suffix = Uuid::now_v7().simple().to_string();
    let owner = principal(&format!("resource-owner-{suffix}"));
    let other_owner = principal(&format!("resource-other-{suffix}"));
    let project = project_contract(store, &owner, &other_owner, &suffix).await?;
    session_template_contract(store, &owner, &other_owner, project.id, &suffix).await?;
    browser_context_contract(store, &owner, &other_owner, project.id, &suffix).await?;
    egress_profile_contract(store, &owner, &other_owner, project.id, &suffix).await?;
    identity_contract(store, &owner, &other_owner, project.id, &suffix).await
}

async fn project_contract(
    store: &SessionStore,
    owner: &AuthenticatedPrincipal,
    other_owner: &AuthenticatedPrincipal,
    suffix: &str,
) -> anyhow::Result<StoredProject> {
    let created = store
        .create_project(
            owner,
            PersistProjectRequest {
                name: format!("contract-project-{suffix}"),
                description: Some("contract project".to_string()),
                labels: HashMap::from([("contract".to_string(), "project".to_string())]),
                quotas: ProjectQuotas {
                    max_active_sessions: Some(2),
                    ..ProjectQuotas::default()
                },
                policy: ProjectPolicy::default(),
                state: ProjectState::Active,
            },
        )
        .await
        .context("create contract project")?;
    ensure!(
        created.owner_subject == owner.subject,
        "project owner changed"
    );
    ensure!(
        store
            .get_project_for_owner(other_owner, created.id)
            .await
            .context("read project as another owner")?
            .is_none(),
        "project was visible to another owner"
    );
    ensure!(
        store
            .list_projects_for_owner(other_owner)
            .await
            .context("list projects as another owner")?
            .iter()
            .all(|project| project.id != created.id),
        "project appeared in another owner's catalog"
    );

    let updated = store
        .update_project_for_owner(
            owner,
            created.id,
            PersistProjectRequest {
                name: created.name.clone(),
                description: Some("updated contract project".to_string()),
                labels: HashMap::from([("contract".to_string(), "updated".to_string())]),
                quotas: ProjectQuotas {
                    max_active_sessions: Some(3),
                    ..ProjectQuotas::default()
                },
                policy: ProjectPolicy::default(),
                state: ProjectState::Active,
            },
        )
        .await
        .context("update contract project")?
        .context("updated contract project disappeared")?;
    ensure!(
        updated.description.as_deref() == Some("updated contract project")
            && updated.quotas.max_active_sessions == Some(3),
        "project update was not persisted"
    );
    ensure!(
        store
            .update_project_for_owner(
                other_owner,
                created.id,
                PersistProjectRequest {
                    name: "forbidden".to_string(),
                    description: None,
                    labels: HashMap::new(),
                    quotas: ProjectQuotas::default(),
                    policy: ProjectPolicy::default(),
                    state: ProjectState::Active,
                },
            )
            .await
            .context("update project as another owner")?
            .is_none(),
        "another owner updated the project"
    );
    Ok(updated)
}

async fn session_template_contract(
    store: &SessionStore,
    owner: &AuthenticatedPrincipal,
    other_owner: &AuthenticatedPrincipal,
    project_id: Uuid,
    suffix: &str,
) -> anyhow::Result<()> {
    let created = store
        .create_session_template(
            owner,
            PersistSessionTemplateRequest {
                name: format!("contract-template-{suffix}"),
                description: Some("contract session template".to_string()),
                labels: HashMap::from([("contract".to_string(), "template".to_string())]),
                defaults: SessionTemplateDefaults {
                    project_id: Some(project_id),
                    owner_mode: Some(SessionOwnerMode::Collaborative),
                    viewport: Some(SessionViewport {
                        width: 1440,
                        height: 900,
                    }),
                    idle_timeout_sec: Some(300),
                    ..SessionTemplateDefaults::default()
                },
            },
        )
        .await
        .context("create contract session template")?;
    ensure!(created.version == 1, "new template version was not one");
    ensure!(
        store
            .list_session_templates_for_owner(owner)
            .await
            .context("list owner session templates")?
            .iter()
            .any(|template| template.id == created.id),
        "session template was missing from the owner catalog"
    );
    ensure!(
        store
            .get_session_template_for_owner(other_owner, created.id)
            .await
            .context("read template as another owner")?
            .is_none(),
        "session template was visible to another owner"
    );

    let updated = store
        .update_session_template_for_owner(
            owner,
            created.id,
            PersistSessionTemplateRequest {
                name: created.name.clone(),
                description: Some("updated contract template".to_string()),
                labels: created.labels.clone(),
                defaults: SessionTemplateDefaults {
                    project_id: Some(project_id),
                    idle_timeout_sec: Some(600),
                    ..created.defaults.clone()
                },
            },
        )
        .await
        .context("update contract session template")?
        .context("updated session template disappeared")?;
    ensure!(
        updated.version == 2 && updated.defaults.idle_timeout_sec == Some(600),
        "session template update/version contract diverged"
    );
    Ok(())
}

async fn browser_context_contract(
    store: &SessionStore,
    owner: &AuthenticatedPrincipal,
    other_owner: &AuthenticatedPrincipal,
    project_id: Uuid,
    suffix: &str,
) -> anyhow::Result<()> {
    let created = store
        .create_browser_context(
            owner,
            PersistBrowserContextRequest {
                id: None,
                project_id: Some(project_id),
                name: format!("contract-context-{suffix}"),
                description: Some("contract browser context".to_string()),
                labels: HashMap::from([("contract".to_string(), "context".to_string())]),
                persistence_mode: BrowserContextPersistenceMode::Reusable,
                retention_sec: Some(3_600),
                max_profile_storage_bytes: Some(1_048_576),
            },
        )
        .await
        .context("create contract browser context")?;
    ensure!(
        created.state == BrowserContextState::Ready,
        "new context was not ready"
    );
    ensure!(
        store
            .get_browser_context_for_owner(other_owner, created.id)
            .await
            .context("read context as another owner")?
            .is_none(),
        "browser context was visible to another owner"
    );
    ensure!(
        store
            .list_browser_contexts_for_owner(owner)
            .await
            .context("list owner browser contexts")?
            .iter()
            .any(|context| context.id == created.id),
        "browser context was missing from the owner catalog"
    );
    ensure!(
        store
            .mark_browser_context_used_for_owner(other_owner, created.id)
            .await
            .context("mark context used as another owner")?
            .is_none(),
        "another owner changed browser context usage"
    );
    let used = store
        .mark_browser_context_used_for_owner(owner, created.id)
        .await
        .context("mark context used")?
        .context("browser context disappeared before use")?;
    ensure!(
        used.last_used_at.is_some(),
        "browser context usage was not persisted"
    );
    let deleted = store
        .delete_browser_context_for_owner(owner, created.id)
        .await
        .context("delete contract browser context")?
        .context("browser context disappeared before delete")?;
    ensure!(
        deleted.state == BrowserContextState::Deleted && deleted.deleted_at.is_some(),
        "browser context delete was not persisted"
    );

    let foreign_project_error = store
        .create_browser_context(
            other_owner,
            PersistBrowserContextRequest {
                id: None,
                project_id: Some(project_id),
                name: format!("foreign-context-{suffix}"),
                description: None,
                labels: HashMap::new(),
                persistence_mode: BrowserContextPersistenceMode::Reusable,
                retention_sec: None,
                max_profile_storage_bytes: None,
            },
        )
        .await
        .expect_err("foreign project context should be rejected");
    ensure!(
        matches!(foreign_project_error, SessionStoreError::NotFound(_)),
        "foreign project context returned the wrong error class"
    );
    Ok(())
}

async fn egress_profile_contract(
    store: &SessionStore,
    owner: &AuthenticatedPrincipal,
    other_owner: &AuthenticatedPrincipal,
    project_id: Uuid,
    suffix: &str,
) -> anyhow::Result<()> {
    let created = store
        .create_egress_profile(
            owner,
            PersistEgressProfileRequest {
                project_id: Some(project_id),
                name: format!("contract-egress-{suffix}"),
                description: Some("contract egress profile".to_string()),
                labels: HashMap::from([("contract".to_string(), "egress".to_string())]),
                proxy: None,
                bypass_rules: vec!["localhost".to_string()],
                custom_ca: None,
                traffic_observation: EgressTrafficObservationConfig::default(),
                state: EgressProfileState::Ready,
            },
        )
        .await
        .context("create contract egress profile")?;
    ensure!(
        store
            .get_egress_profile_for_owner(other_owner, created.id)
            .await
            .context("read egress profile as another owner")?
            .is_none(),
        "egress profile was visible to another owner"
    );
    ensure!(
        store
            .list_egress_profiles_for_owner(owner)
            .await
            .context("list owner egress profiles")?
            .iter()
            .any(|profile| profile.id == created.id),
        "egress profile was missing from the owner catalog"
    );
    let updated = store
        .update_egress_profile_for_owner(
            owner,
            created.id,
            PersistEgressProfileRequest {
                project_id: Some(project_id),
                name: created.name.clone(),
                description: created.description.clone(),
                labels: created.labels.clone(),
                proxy: None,
                bypass_rules: created.bypass_rules.clone(),
                custom_ca: None,
                traffic_observation: EgressTrafficObservationConfig::default(),
                state: EgressProfileState::Disabled,
            },
        )
        .await
        .context("update contract egress profile")?
        .context("updated egress profile disappeared")?;
    ensure!(
        updated.state == EgressProfileState::Disabled,
        "egress state was not updated"
    );

    let foreign_project_error = store
        .create_egress_profile(
            other_owner,
            PersistEgressProfileRequest {
                project_id: Some(project_id),
                name: format!("foreign-egress-{suffix}"),
                description: None,
                labels: HashMap::new(),
                proxy: None,
                bypass_rules: Vec::new(),
                custom_ca: None,
                traffic_observation: EgressTrafficObservationConfig::default(),
                state: EgressProfileState::Ready,
            },
        )
        .await
        .expect_err("foreign project egress profile should be rejected");
    ensure!(
        matches!(foreign_project_error, SessionStoreError::NotFound(_)),
        "foreign project egress profile returned the wrong error class"
    );
    Ok(())
}

async fn identity_contract(
    store: &SessionStore,
    owner: &AuthenticatedPrincipal,
    other_owner: &AuthenticatedPrincipal,
    project_id: Uuid,
    suffix: &str,
) -> anyhow::Result<()> {
    let issuer = format!("https://issuer-{suffix}.example");
    let client_id = format!("contract-client-{suffix}");
    let service_principal = store
        .create_service_principal(
            owner,
            PersistServicePrincipalRequest {
                name: "Contract service principal".to_string(),
                description: Some("contract identity".to_string()),
                client_id: client_id.clone(),
                issuer: issuer.clone(),
                labels: HashMap::from([("contract".to_string(), "identity".to_string())]),
                scopes: vec!["session:create".to_string()],
                allowed_project_ids: vec![project_id],
                state: ServicePrincipalState::Active,
            },
        )
        .await
        .context("create contract service principal")?;
    ensure!(
        store
            .get_service_principal_for_owner(other_owner, service_principal.id)
            .await
            .context("read service principal as another owner")?
            .is_none(),
        "service principal was visible to another owner"
    );
    ensure!(
        store
            .list_service_principals_for_owner(owner)
            .await
            .context("list owner service principals")?
            .iter()
            .any(|item| item.id == service_principal.id),
        "service principal was missing from the owner catalog"
    );
    let duplicate_error = store
        .create_service_principal(
            owner,
            PersistServicePrincipalRequest {
                name: "Duplicate contract service principal".to_string(),
                description: None,
                client_id: client_id.clone(),
                issuer: issuer.clone(),
                labels: HashMap::new(),
                scopes: Vec::new(),
                allowed_project_ids: Vec::new(),
                state: ServicePrincipalState::Active,
            },
        )
        .await
        .expect_err("duplicate service principal should be rejected");
    ensure!(
        matches!(duplicate_error, SessionStoreError::Conflict(_)),
        "duplicate service principal returned the wrong error class"
    );
    let service_principal = store
        .update_service_principal_for_owner(
            owner,
            service_principal.id,
            PersistServicePrincipalRequest {
                name: "Disabled contract service principal".to_string(),
                description: None,
                client_id: client_id.clone(),
                issuer: issuer.clone(),
                labels: HashMap::new(),
                scopes: vec!["session:create".to_string()],
                allowed_project_ids: vec![project_id],
                state: ServicePrincipalState::Disabled,
            },
        )
        .await
        .context("update contract service principal")?
        .context("updated service principal disappeared")?;
    ensure!(
        service_principal.state == ServicePrincipalState::Disabled,
        "service principal state was not updated"
    );

    let mapping_request = || PersistIdentityMappingRequest {
        name: "Contract identity mapping".to_string(),
        description: Some("contract project grant".to_string()),
        kind: IdentityMappingKind::ServicePrincipal,
        issuer: issuer.clone(),
        external_id: client_id.clone(),
        claim_name: None,
        service_principal_id: Some(service_principal.id),
        project_id,
        labels: HashMap::from([("contract".to_string(), "mapping".to_string())]),
        scopes: vec!["session:create".to_string()],
        state: IdentityMappingState::Active,
    };
    let mapping = store
        .create_identity_mapping(owner, mapping_request())
        .await
        .context("create contract identity mapping")?;
    ensure!(
        store
            .get_identity_mapping_for_owner(other_owner, mapping.id)
            .await
            .context("read identity mapping as another owner")?
            .is_none(),
        "identity mapping was visible to another owner"
    );
    ensure!(
        store
            .list_identity_mappings_for_owner(owner)
            .await
            .context("list owner identity mappings")?
            .iter()
            .any(|item| item.id == mapping.id),
        "identity mapping was missing from the owner catalog"
    );
    let duplicate_mapping_error = store
        .create_identity_mapping(owner, mapping_request())
        .await
        .expect_err("duplicate identity mapping should be rejected");
    ensure!(
        matches!(duplicate_mapping_error, SessionStoreError::Conflict(_)),
        "duplicate identity mapping returned the wrong error class"
    );
    let updated_mapping = store
        .update_identity_mapping_for_owner(
            owner,
            mapping.id,
            PersistIdentityMappingRequest {
                name: "Disabled contract identity mapping".to_string(),
                state: IdentityMappingState::Disabled,
                ..mapping_request()
            },
        )
        .await
        .context("update contract identity mapping")?
        .context("updated identity mapping disappeared")?;
    ensure!(
        updated_mapping.state == IdentityMappingState::Disabled,
        "identity mapping state was not updated"
    );
    let foreign_project_error = store
        .create_identity_mapping(
            other_owner,
            PersistIdentityMappingRequest {
                name: "Foreign project mapping".to_string(),
                description: None,
                kind: IdentityMappingKind::User,
                issuer: other_owner.issuer.clone(),
                external_id: other_owner.subject.clone(),
                claim_name: None,
                service_principal_id: None,
                project_id,
                labels: HashMap::new(),
                scopes: Vec::new(),
                state: IdentityMappingState::Active,
            },
        )
        .await
        .expect_err("foreign project identity mapping should be rejected");
    ensure!(
        matches!(foreign_project_error, SessionStoreError::NotFound(_)),
        "foreign project identity mapping returned the wrong error class"
    );
    Ok(())
}
