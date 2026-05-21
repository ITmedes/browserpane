use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;

use crate::browser_contexts::BrowserContextRetentionManager;
use crate::config::Config;
use crate::session_control::SessionStore;
use crate::session_manager::SessionManager;

pub(in crate::app) async fn start_browser_context_retention(
    config: &Config,
    session_store: SessionStore,
    session_manager: Arc<SessionManager>,
) -> anyhow::Result<()> {
    if config
        .storage
        .browser_context_retention_cleanup_interval_secs
        == 0
    {
        return Ok(());
    }

    let manager = Arc::new(BrowserContextRetentionManager::new(
        session_store,
        session_manager,
        Duration::from_secs(
            config
                .storage
                .browser_context_retention_cleanup_interval_secs,
        ),
    ));
    manager.run_cleanup_pass(Utc::now()).await?;
    manager.start();
    Ok(())
}
