pub mod core;
pub use core::{DatabaseConnection, QueryParams, QueryResult};

// Database implementations
mod mongodb;
mod postgres;
pub use mongodb::MongoConnection;
pub use postgres::PostgresConnection;

// SSH tunneling support
pub mod ssh_tunnel;

// Connection management
pub mod factory;
pub use factory::ConnectionManager;

// Error handling
pub mod error;
pub use error::{DatabaseError, DatabaseResult};

// Common types and configurations
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum DatabaseType {
    Postgres,
    MongoDB,
}

impl Default for DatabaseType {
    fn default() -> Self {
        DatabaseType::Postgres
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionStatus {
    NotConnected,
    Connecting,
    Connected,
    Failed,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SSHConfig {
    #[serde(default)]
    pub host: String,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub private_key_path: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
}

fn default_ssh_port() -> u16 {
    22
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConnectionConfig {
    pub name: String,
    pub db_type: DatabaseType,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: Option<String>,
    pub database: String,
    #[serde(default)]
    pub ssh_tunnel: Option<SSHConfig>,
    #[serde(default)]
    pub ssh_tunnel_name: Option<String>,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            db_type: DatabaseType::default(),
            host: String::from("localhost"),
            port: 5432,
            username: String::new(),
            password: None,
            database: String::new(),
            ssh_tunnel: None,
            ssh_tunnel_name: None,
        }
    }
}
