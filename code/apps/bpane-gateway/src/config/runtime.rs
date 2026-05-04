use std::path::PathBuf;

use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct RuntimeConfig {
    /// Path to the host agent Unix socket.
    #[arg(long, default_value = "/tmp/bpane.sock")]
    pub agent_socket: PathBuf,

    /// Runtime backend: "static_single", "docker_single", or "docker_pool".
    #[arg(long = "runtime-backend", default_value = "static_single")]
    pub backend: String,

    /// Idle timeout before an unattached runtime assignment is released or shut down.
    #[arg(long = "runtime-idle-timeout-secs", default_value_t = 300)]
    pub idle_timeout_secs: u64,

    /// Optional CDP endpoint exposed for the static_single runtime backend.
    /// This should be reachable from internal automation services such as mcp-bridge.
    #[arg(long = "runtime-cdp-endpoint")]
    pub cdp_endpoint: Option<String>,

    /// Docker CLI binary used by the optional docker_single runtime backend.
    #[arg(long = "docker-runtime-bin", default_value = "docker")]
    pub docker_bin: String,

    /// Host worker image used by the optional docker_single runtime backend.
    #[arg(long = "docker-runtime-image")]
    pub docker_image: Option<String>,

    /// Docker network used by the optional docker_single runtime backend.
    #[arg(long = "docker-runtime-network")]
    pub docker_network: Option<String>,

    /// Docker named volume mounted at the session socket root for docker-backed runtimes.
    #[arg(long = "docker-runtime-socket-volume", alias = "docker-runtime-volume")]
    pub docker_socket_volume: Option<String>,

    /// Prefix for per-session Docker named volumes that hold browser profile/uploads/downloads.
    #[arg(
        long = "docker-runtime-session-data-volume-prefix",
        default_value = "bpane-session-data"
    )]
    pub docker_session_data_volume_prefix: String,

    /// Container name prefix used by docker-backed runtime workers.
    #[arg(
        long = "docker-runtime-container-name-prefix",
        default_value = "bpane-runtime"
    )]
    pub docker_container_name_prefix: String,

    /// Session-scoped socket root inside the shared run volume for docker_single.
    #[arg(
        long = "docker-runtime-socket-root",
        default_value = "/run/bpane/sessions"
    )]
    pub docker_socket_root: String,

    /// Session data root inside docker-backed runtime workers.
    #[arg(
        long = "docker-runtime-session-data-root",
        default_value = "/run/bpane/session"
    )]
    pub docker_session_data_root: String,

    /// CDP proxy port exposed by docker-backed runtime workers.
    #[arg(long = "docker-runtime-cdp-proxy-port", default_value_t = 9223)]
    pub docker_cdp_proxy_port: u16,

    /// shm-size passed to docker run for the optional docker_single runtime backend.
    #[arg(long = "docker-runtime-shm-size", default_value = "128m")]
    pub docker_shm_size: String,

    /// Startup timeout for the optional docker_single runtime backend.
    #[arg(long = "docker-runtime-start-timeout-secs", default_value_t = 60)]
    pub docker_start_timeout_secs: u64,

    /// Maximum number of runtime-backed sessions that can exist in parallel in docker_pool mode.
    #[arg(long, default_value_t = 1)]
    pub max_active_runtimes: usize,

    /// Maximum number of runtime workers that may be starting concurrently in docker_pool mode.
    #[arg(long, default_value_t = 1)]
    pub max_starting_runtimes: usize,

    /// Optional env-file forwarded to docker run for the optional docker_single runtime backend.
    #[arg(long = "docker-runtime-env-file")]
    pub docker_env_file: Option<PathBuf>,

    /// Apply --security-opt seccomp=unconfined when launching docker runtime workers.
    #[arg(long = "docker-runtime-seccomp-unconfined", default_value_t = false)]
    pub docker_seccomp_unconfined: bool,
}
