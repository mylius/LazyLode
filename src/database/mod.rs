pub mod core;
pub use core::{DatabaseConnection, QueryParams, QueryResult};

// Database implementations
mod mongodb;
mod postgres;

// SSH tunneling support
pub mod ssh_tunnel;

// Connection management
pub mod factory;
pub use factory::{ConnectionManager, PrefetchedDatabase, PrefetchedSchema, PrefetchedStructure};

// Error handling
pub mod error;

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
pub struct DatabaseConfig {
    /// List of specific schemas to show for this database
    /// If empty, all schemas will be discovered and shown
    #[serde(default)]
    pub schemas: Vec<String>,
    /// Whether to auto-expand this database on connection
    #[serde(default)]
    pub auto_expand: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            schemas: Vec::new(),
            auto_expand: false,
        }
    }
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
    /// Default database for connection context
    #[serde(default)]
    pub default_database: Option<String>,
    /// Specific databases to show with their configurations
    /// If empty, all available databases will be discovered and shown
    #[serde(default)]
    pub databases: std::collections::HashMap<String, DatabaseConfig>,
    #[serde(default)]
    pub ssh_tunnel: Option<SSHConfig>,
    #[serde(default)]
    pub ssh_tunnel_name: Option<String>,
    /// Legacy field for backward compatibility
    /// If set, it will be used as default_database and added to databases
    #[serde(default, skip_serializing)]
    pub database: Option<String>,
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
            default_database: None,
            databases: std::collections::HashMap::new(),
            ssh_tunnel: None,
            ssh_tunnel_name: None,
            database: None,
        }
    }
}

impl ConnectionConfig {
    /// Migrate from old format to new format
    /// This handles backward compatibility with the old `database` field
    pub fn migrate_from_legacy(&mut self) {
        // If we have a legacy database field but no default_database, use it
        if let Some(legacy_db) = &self.database {
            if self.default_database.is_none() {
                self.default_database = Some(legacy_db.clone());
            }

            // If we don't have any databases configured, add the legacy one
            if self.databases.is_empty() {
                self.databases
                    .insert(legacy_db.clone(), DatabaseConfig::default());
            }
        }
    }

    /// Get the effective default database name
    pub fn get_default_database(&self) -> Option<&String> {
        self.default_database.as_ref()
    }

    /// Check if a specific database should be shown
    pub fn should_show_database(&self, db_name: &str) -> bool {
        // If no databases are configured, show all
        if self.databases.is_empty() {
            return true;
        }

        // Otherwise, only show configured databases
        self.databases.contains_key(db_name)
    }

    /// Get configuration for a specific database
    pub fn get_database_config(&self, db_name: &str) -> Option<&DatabaseConfig> {
        self.databases.get(db_name)
    }
}
