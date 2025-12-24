use super::{
    core::DatabaseConnection, mongodb::MongoConnection, postgres::PostgresConnection,
    sqlite::SqliteConnection, ConnectionConfig, ConnectionStatus, DatabaseType,
};
use anyhow::Result;
use std::collections::HashMap;
use tokio::task::JoinSet;

pub fn create_database_connection(config: ConnectionConfig) -> Box<dyn DatabaseConnection> {
    match config.db_type {
        DatabaseType::Postgres => Box::new(PostgresConnection::new(config)),
        DatabaseType::MongoDB => Box::new(MongoConnection::new(config)),
        DatabaseType::SQLite => Box::new(SqliteConnection::new(config)),
    }
}

// Connection manager to handle database connections
pub struct ConnectionManager {
    pub connections: HashMap<String, Box<dyn DatabaseConnection>>,
    pub connection_statuses: HashMap<String, ConnectionStatus>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
            connection_statuses: HashMap::new(),
        }
    }

    pub async fn connect(&mut self, config: ConnectionConfig) -> Result<()> {
        self.connection_statuses
            .insert(config.name.clone(), ConnectionStatus::Connecting);
        let mut connection = create_database_connection(config.clone());
        match connection.connect().await {
            Ok(_) => {
                self.connections.insert(config.name.clone(), connection);
                self.connection_statuses
                    .insert(config.name.clone(), ConnectionStatus::Connected);
                Ok(())
            }
            Err(e) => {
                self.connection_statuses
                    .insert(config.name.clone(), ConnectionStatus::Failed);
                Err(e)
            }
        }
    }

    pub async fn disconnect(&mut self, name: &str) -> Result<()> {
        if let Some(connection) = self.connections.get_mut(name) {
            connection.disconnect().await?;
            self.connections.remove(name);
        }
        self.connection_statuses
            .insert(name.to_string(), ConnectionStatus::NotConnected);
        Ok(())
    }

    pub fn get_connection(&self, name: &str) -> Option<&Box<dyn DatabaseConnection>> {
        self.connections.get(name)
    }

    pub fn get_connection_status(&self, name: &str) -> ConnectionStatus {
        self.connection_statuses
            .get(name)
            .copied()
            .unwrap_or(ConnectionStatus::NotConnected)
    }

    pub async fn connect_all_async(
        &mut self,
        configs: Vec<ConnectionConfig>,
    ) -> HashMap<String, Result<()>> {
        let mut results = HashMap::new();
        let mut join_set = JoinSet::new();

        for config in configs {
            let config_clone = config.clone();
            join_set.spawn(async move {
                let mut connection = create_database_connection(config_clone.clone());
                let result = connection.connect().await;
                (config_clone.name, result, connection)
            });
        }

        while let Some(result) = join_set.join_next().await {
            match result {
                Ok((name, connection_result, connection)) => match connection_result {
                    Ok(_) => {
                        self.connections.insert(name.clone(), connection);
                        self.connection_statuses
                            .insert(name.clone(), ConnectionStatus::Connected);
                        results.insert(name, Ok(()));
                    }
                    Err(e) => {
                        self.connection_statuses
                            .insert(name.clone(), ConnectionStatus::Failed);
                        results.insert(name, Err(e));
                    }
                },
                Err(e) => {
                    let error = anyhow::anyhow!("Task join error: {}", e);
                    results.insert("unknown".to_string(), Err(error));
                }
            }
        }

        results
    }

    pub async fn prefetch_databases_only(
        &mut self,
        config: ConnectionConfig,
    ) -> Result<PrefetchedStructure> {
        // Reduced timeout for faster failure detection
        let timeout_duration = std::time::Duration::from_secs(5);

        let prefetch_result = tokio::time::timeout(timeout_duration, async {
            let mut connection = create_database_connection(config.clone());
            connection.connect().await?;

            let databases = connection.list_databases().await?;
            let mut prefetched_databases = Vec::new();

            // Only load database names, not schemas or tables
            for db_name in databases {
                prefetched_databases.push(PrefetchedDatabase {
                    name: db_name,
                    schemas: Vec::new(), // Empty - will be loaded on-demand
                });
            }

            // Store the connection for later use
            self.connections.insert(config.name.clone(), connection);
            self.connection_statuses
                .insert(config.name.clone(), ConnectionStatus::Connected);

            Ok(PrefetchedStructure {
                connection_name: config.name,
                databases: prefetched_databases,
            })
        })
        .await;

        match prefetch_result {
            Ok(result) => result,
            Err(_) => Err(anyhow::anyhow!(
                "Database prefetching timed out after 5 seconds"
            )),
        }
    }

    pub async fn prefetch_database_structure(
        &mut self,
        config: ConnectionConfig,
    ) -> Result<PrefetchedStructure> {
        // Add timeout to prevent hanging
        let timeout_duration = std::time::Duration::from_secs(30);

        let prefetch_result = tokio::time::timeout(timeout_duration, async {
            let mut connection = create_database_connection(config.clone());
            connection.connect().await?;

            let databases = connection.list_databases().await?;
            let mut prefetched_databases = Vec::new();

            for db_name in databases {
                let schemas = connection.list_schemas(&db_name).await?;
                let mut prefetched_schemas = Vec::new();

                for schema_name in schemas {
                    let tables = connection.list_tables(&schema_name).await?;
                    prefetched_schemas.push(PrefetchedSchema {
                        name: schema_name,
                        tables,
                    });
                }

                prefetched_databases.push(PrefetchedDatabase {
                    name: db_name,
                    schemas: prefetched_schemas,
                });
            }

            // Store the connection for later use
            self.connections.insert(config.name.clone(), connection);
            self.connection_statuses
                .insert(config.name.clone(), ConnectionStatus::Connected);

            Ok(PrefetchedStructure {
                connection_name: config.name,
                databases: prefetched_databases,
            })
        })
        .await;

        match prefetch_result {
            Ok(result) => result,
            Err(_) => Err(anyhow::anyhow!("Prefetching timed out after 30 seconds")),
        }
    }

    pub async fn prefetch_schemas_for_database(
        &mut self,
        connection_name: &str,
        database_name: &str,
    ) -> Result<Vec<PrefetchedSchema>> {
        if let Some(connection) = self.connections.get_mut(connection_name) {
            let schemas = connection.list_schemas(database_name).await?;
            let mut prefetched_schemas = Vec::new();

            // Only load schema names, not tables
            for schema_name in schemas {
                prefetched_schemas.push(PrefetchedSchema {
                    name: schema_name,
                    tables: Vec::new(), // Empty - will be loaded on-demand
                });
            }

            Ok(prefetched_schemas)
        } else {
            Err(anyhow::anyhow!("Connection not found: {}", connection_name)
                .context(format!("Failed to prefetch schemas for connection: {}", connection_name)))
        }
    }

    pub async fn prefetch_tables_for_schema(
        &mut self,
        connection_name: &str,
        schema_name: &str,
    ) -> Result<Vec<String>> {
        if let Some(connection) = self.connections.get_mut(connection_name) {
            connection.list_tables(schema_name).await
        } else {
            Err(anyhow::anyhow!("Connection not found: {}", connection_name)
                .context(format!("Failed to prefetch schemas for connection: {}", connection_name)))
        }
    }

    /// Fast prefetch that only gets database names without storing connections
    pub async fn fast_prefetch_databases_only(
        config: ConnectionConfig,
    ) -> Result<PrefetchedStructure> {
        // Very short timeout for fast failure
        let timeout_duration = std::time::Duration::from_secs(3);

        let prefetch_result = tokio::time::timeout(timeout_duration, async {
            let mut connection = create_database_connection(config.clone());
            connection.connect().await?;

            let databases = connection.list_databases().await?;
            let mut prefetched_databases = Vec::new();

            // Only load database names, not schemas or tables
            for db_name in databases {
                prefetched_databases.push(PrefetchedDatabase {
                    name: db_name,
                    schemas: Vec::new(), // Empty - will be loaded on-demand
                });
            }

            // Don't store the connection - it will be created when needed
            Ok(PrefetchedStructure {
                connection_name: config.name,
                databases: prefetched_databases,
            })
        })
        .await;

        match prefetch_result {
            Ok(result) => result,
            Err(_) => Err(anyhow::anyhow!(
                "Database prefetching timed out after 3 seconds"
            )),
        }
    }

    /// Validate that a connection can be established without storing it
    pub async fn validate_connection(config: ConnectionConfig) -> Result<()> {
        // Short timeout for validation
        let timeout_duration = std::time::Duration::from_secs(5);

        let validation_result = tokio::time::timeout(timeout_duration, async {
            let mut connection = create_database_connection(config.clone());
            connection.connect().await?;

            // Try to list databases to ensure the connection is working
            let _databases = connection.list_databases().await?;

            // Connection is valid, disconnect
            connection.disconnect().await?;

            Ok(())
        })
        .await;

        match validation_result {
            Ok(result) => result,
            Err(_) => Err(anyhow::anyhow!(
                "Connection validation timed out after 5 seconds"
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PrefetchedStructure {
    pub connection_name: String,
    pub databases: Vec<PrefetchedDatabase>,
}

#[derive(Debug, Clone)]
pub struct PrefetchedDatabase {
    pub name: String,
    pub schemas: Vec<PrefetchedSchema>,
}

#[derive(Debug, Clone)]
pub struct PrefetchedSchema {
    pub name: String,
    pub tables: Vec<String>,
}
