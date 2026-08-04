use anyhow::{anyhow, bail, Context};
use tokio::task::JoinHandle;
use tokio_postgres::{Client, NoTls};
use url::Url;

use super::*;

const SCHEMA_PREFIX: &str = "bpane_store_contract_";

pub(super) struct PostgresContractFixture {
    store: SessionStore,
    admin_client: Client,
    admin_connection: JoinHandle<Result<(), tokio_postgres::Error>>,
    schema: String,
}

impl PostgresContractFixture {
    pub(super) async fn create(database_url: &str) -> anyhow::Result<Self> {
        let schema = contract_schema_name();
        let scoped_url = database_url_with_schema(database_url, &schema)?;
        let (admin_client, admin_connection) =
            tokio_postgres::connect(database_url, NoTls)
                .await
                .map_err(|_| anyhow!("connect to Postgres contract database"))?;
        let admin_connection = tokio::spawn(admin_connection);

        if let Err(error) = admin_client
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .await
        {
            drop(admin_client);
            let _ = admin_connection.await;
            return Err(error).context("create isolated Postgres contract schema");
        }

        let store = match SessionStore::from_database_url_with_config(
            &scoped_url,
            contract_runtime_profile(),
        )
        .await
        {
            Ok(store) => store,
            Err(error) => {
                let _ = admin_client
                    .batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
                    .await;
                drop(admin_client);
                let _ = admin_connection.await;
                return Err(error).context("initialize Postgres contract store");
            }
        };

        Ok(Self {
            store,
            admin_client,
            admin_connection,
            schema,
        })
    }

    pub(super) fn store(&self) -> &SessionStore {
        &self.store
    }

    pub(super) async fn cleanup(self) -> anyhow::Result<()> {
        let Self {
            store,
            admin_client,
            admin_connection,
            schema,
        } = self;
        drop(store);
        let drop_result = admin_client
            .batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
            .await
            .context("drop isolated Postgres contract schema");
        drop(admin_client);
        let connection_result = admin_connection
            .await
            .context("join Postgres contract admin connection")?;
        if connection_result.is_err() {
            bail!("Postgres contract admin connection failed");
        }
        drop_result
    }
}

fn contract_schema_name() -> String {
    format!(
        "{SCHEMA_PREFIX}{}",
        Uuid::now_v7().simple().to_string().to_ascii_lowercase()
    )
}

fn contract_runtime_profile() -> SessionManagerProfile {
    SessionManagerProfile {
        runtime_binding: "legacy_single_session".to_string(),
        compatibility_mode: "legacy_single_runtime".to_string(),
        max_runtime_sessions: 1,
        supports_legacy_global_routes: true,
        supports_session_extensions: false,
    }
}

fn database_url_with_schema(database_url: &str, schema: &str) -> anyhow::Result<String> {
    if !schema.starts_with(SCHEMA_PREFIX)
        || !schema
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        bail!("invalid Postgres contract schema identifier");
    }

    let mut url = Url::parse(database_url).context("parse Postgres contract database URL")?;
    if !matches!(url.scheme(), "postgres" | "postgresql") {
        bail!("Postgres contract database URL must use postgres or postgresql");
    }
    let existing_pairs = url
        .query_pairs()
        .filter(|(key, _)| key != "options")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    let existing_options = url
        .query_pairs()
        .find(|(key, _)| key == "options")
        .map(|(_, value)| value.into_owned());
    url.set_query(None);
    {
        let mut query = url.query_pairs_mut();
        query.extend_pairs(existing_pairs);
        let search_path = format!("-csearch_path={schema}");
        let options = existing_options
            .map(|value| format!("{value} {search_path}"))
            .unwrap_or(search_path);
        query.append_pair("options", &options);
    }
    Ok(url.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_database_url_preserves_safe_parameters() {
        let scoped = database_url_with_schema(
            "postgresql://user:secret@localhost/browserpane?application_name=contract",
            "bpane_store_contract_01900000000070008000000000000000",
        )
        .unwrap();
        let parsed = Url::parse(&scoped).unwrap();
        let pairs = parsed.query_pairs().collect::<HashMap<_, _>>();

        assert_eq!(pairs.get("application_name").unwrap(), "contract");
        assert_eq!(
            pairs.get("options").unwrap(),
            "-csearch_path=bpane_store_contract_01900000000070008000000000000000"
        );
    }

    #[test]
    fn scoped_database_url_rejects_unsafe_inputs() {
        assert!(database_url_with_schema(
            "https://localhost/browserpane",
            "bpane_store_contract_01900000000070008000000000000000"
        )
        .is_err());
        assert!(database_url_with_schema(
            "postgresql://localhost/browserpane",
            "bpane_store_contract_bad;drop"
        )
        .is_err());
    }
}
