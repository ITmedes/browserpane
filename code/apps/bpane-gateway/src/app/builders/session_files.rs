use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use chrono::{Duration as ChronoDuration, Utc};

use crate::config::Config;
use crate::session_control::SessionStore;
use crate::session_files::SessionFileRetentionManager;
use crate::workspaces::WorkspaceFileStore;

pub(in crate::app) async fn start_session_file_retention(
    config: &Config,
    session_store: SessionStore,
    file_store: Arc<WorkspaceFileStore>,
) -> anyhow::Result<()> {
    let retention = session_file_retention_window(
        "session-file-retention-secs",
        config.storage.session_file_retention_secs,
    )?;
    let Some(retention) = retention else {
        return Ok(());
    };
    if config.storage.session_file_cleanup_interval_secs == 0 {
        return Ok(());
    }

    let manager = Arc::new(SessionFileRetentionManager::new(
        session_store,
        file_store,
        Duration::from_secs(config.storage.session_file_cleanup_interval_secs),
        retention,
    ));
    manager.run_cleanup_pass(Utc::now()).await?;
    manager.start();
    Ok(())
}

pub(in crate::app) fn session_file_retention_window(
    flag_name: &str,
    retention_secs: u64,
) -> anyhow::Result<Option<ChronoDuration>> {
    if retention_secs == 0 {
        return Ok(None);
    }

    Ok(Some(ChronoDuration::seconds(
        i64::try_from(retention_secs).map_err(|error| {
            anyhow!("--{flag_name} is out of range for chrono duration: {error}")
        })?,
    )))
}
