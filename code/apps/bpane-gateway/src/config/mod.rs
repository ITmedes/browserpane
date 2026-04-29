use clap::Parser;

mod auth;
mod gateway;
mod recording;
mod runtime;
mod storage;
mod workflow;

pub use self::auth::AuthConfig;
pub use self::gateway::GatewayConfig;
pub use self::recording::RecordingConfig;
pub use self::runtime::RuntimeConfig;
pub use self::storage::StorageConfig;
pub use self::workflow::WorkflowConfig;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "bpane-gateway",
    about = "BrowserPane WebTransport gateway server"
)]
pub struct Config {
    #[command(flatten)]
    pub gateway: GatewayConfig,

    #[command(flatten)]
    pub runtime: RuntimeConfig,

    #[command(flatten)]
    pub auth: AuthConfig,

    #[command(flatten)]
    pub storage: StorageConfig,

    #[command(flatten)]
    pub recording: RecordingConfig,

    #[command(flatten)]
    pub workflow: WorkflowConfig,
}
