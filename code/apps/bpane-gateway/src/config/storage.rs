use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct StorageConfig {
    /// Optional Postgres connection string for the versioned session control plane.
    #[arg(long)]
    pub database_url: Option<String>,

    /// Vault base URL used for credential binding storage and resolution.
    #[arg(long)]
    pub credential_vault_addr: Option<String>,

    /// Vault token used for credential binding storage and resolution.
    #[arg(long)]
    pub credential_vault_token: Option<String>,

    /// Vault KV v2 mount path used for credential binding storage and resolution.
    #[arg(long, default_value = "secret")]
    pub credential_vault_mount_path: String,

    /// Vault key prefix used for managed credential binding secrets.
    #[arg(long, default_value = "browserpane/credential-bindings")]
    pub credential_vault_prefix: String,

    /// Managed local root for finalized recording artifacts served by the gateway's local_fs artifact store.
    #[arg(long, default_value = "/tmp/bpane-recording-artifacts")]
    pub recording_artifact_local_root: std::path::PathBuf,

    /// Managed local root for persisted file workspace content served by the gateway's local_fs workspace file store.
    #[arg(long, default_value = "/tmp/bpane-file-workspaces")]
    pub file_workspace_local_root: std::path::PathBuf,
}
