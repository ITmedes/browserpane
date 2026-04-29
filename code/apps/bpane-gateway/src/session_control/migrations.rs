use sqlx::Connection;

use super::*;

static CONTROL_PLANE_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

pub(super) async fn run_postgres_migrations(database_url: &str) -> Result<(), SessionStoreError> {
    let max_attempts = 30;
    let mut last_error = String::new();

    for attempt in 0..max_attempts {
        match sqlx::postgres::PgConnection::connect(database_url).await {
            Ok(mut connection) => {
                CONTROL_PLANE_MIGRATOR
                    .run(&mut connection)
                    .await
                    .map_err(|error| {
                        SessionStoreError::Backend(format!(
                            "failed to run postgres migrations: {error}"
                        ))
                    })?;
                return Ok(());
            }
            Err(error) => {
                last_error = error.to_string();
                if attempt + 1 < max_attempts {
                    sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }

    Err(SessionStoreError::Backend(format!(
        "failed to connect to postgres for migrations after retries: {last_error}"
    )))
}
