use bb8::Pool;
use bb8_postgres::PostgresConnectionManager;

use super::*;

type PostgresPool = Pool<PostgresConnectionManager<NoTls>>;
type PostgresPooledClient<'a> = bb8::PooledConnection<'a, PostgresConnectionManager<NoTls>>;

pub(in crate::session_control) struct PostgresDb {
    pool: PostgresPool,
}

impl PostgresDb {
    pub(super) async fn connect(database_url: &str) -> Result<Self, SessionStoreError> {
        let max_attempts = 30;
        let manager = PostgresConnectionManager::new_from_stringlike(database_url, NoTls).map_err(
            |error| {
                SessionStoreError::Backend(format!(
                    "failed to parse postgres connection settings: {error}"
                ))
            },
        )?;

        let mut last_error = String::new();
        for attempt in 0..max_attempts {
            let pool = Pool::builder()
                .build(manager.clone())
                .await
                .map_err(|error| {
                    SessionStoreError::Backend(format!("failed to build postgres pool: {error}"))
                })?;
            let ready = {
                match pool.get().await {
                    Ok(connection) => {
                        drop(connection);
                        true
                    }
                    Err(error) => {
                        last_error = error.to_string();
                        false
                    }
                }
            };
            if ready {
                return Ok(Self { pool });
            }
            if attempt + 1 < max_attempts {
                sleep(Duration::from_secs(2)).await;
            }
        }

        Err(SessionStoreError::Backend(format!(
            "failed to connect to postgres after retries: {last_error}"
        )))
    }

    pub(in crate::session_control) async fn client(
        &self,
    ) -> Result<PostgresPooledClient<'_>, SessionStoreError> {
        self.pool.get().await.map_err(|error| {
            SessionStoreError::Backend(format!(
                "failed to acquire postgres connection from pool: {error}"
            ))
        })
    }
}
