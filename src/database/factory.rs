use super::{
    core::DatabaseConnection, mongodb::MongoConnection, postgres::PostgresConnection,
    ConnectionConfig, DatabaseType,
};
use anyhow::Result;
use std::collections::HashMap;

pub fn create_database_connection(config: ConnectionConfig) -> Box<dyn DatabaseConnection> {
    match config.db_type {
        DatabaseType::Postgres => Box::new(PostgresConnection::new(config)),
        DatabaseType::MongoDB => Box::new(MongoConnection::new(config)),
    }
}

// Connection manager to handle database connections
pub struct ConnectionManager {
    connections: HashMap<String, Box<dyn DatabaseConnection>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    pub async fn connect(&mut self, config: ConnectionConfig) -> Result<()> {
        let mut connection = create_database_connection(config.clone());
        connection.connect().await?;
        self.connections.insert(config.name.clone(), connection);
        Ok(())
    }

    pub async fn disconnect(&mut self, name: &str) -> Result<()> {
        if let Some(connection) = self.connections.get_mut(name) {
            connection.disconnect().await?;
            self.connections.remove(name);
        }
        Ok(())
    }

    pub fn get_connection(&self, name: &str) -> Option<&Box<dyn DatabaseConnection>> {
        self.connections.get(name)
    }
}
