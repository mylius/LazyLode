use anyhow::{Result, Context};
use async_trait::async_trait;
use tokio::net::TcpStream;
use tokio_postgres::Client as PgClient;
use mongodb::Client as MongoClient;
use ssh2::Session;
use serde::{Deserialize, Serialize};
use crate::logging;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum DatabaseType {
    Postgres,
    MongoDB,
}

impl Default for DatabaseType {
    fn default() -> Self {
        DatabaseType::Postgres
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SSHConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub private_key_path: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConnectionConfig {
    pub name: String,
    pub db_type: DatabaseType,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub database: String,
    pub ssh_tunnel: Option<SSHConfig>,
}



#[async_trait]
pub trait DatabaseConnection: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn execute_query(&self, query: &str) -> Result<QueryResult>;
    async fn get_schema(&self) -> Result<DatabaseSchema>;
    async fn list_databases(&self) -> Result<Vec<String>>;
    async fn list_schemas(&self, database: &str) -> Result<Vec<String>>;
    async fn list_tables(&self, schema: &str) -> Result<Vec<String>>;
    async fn fetch_table_data(&self, schema: &str, table: &str, order_by: Option<(String, bool)>) -> Result<QueryResult>;
    fn clone_box(&self) -> Box<dyn DatabaseConnection>;
}


impl Clone for Box<dyn DatabaseConnection> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

pub struct PostgresConnection {
    config: ConnectionConfig,
    client: Option<PgClient>,
    ssh_session: Option<Session>,
}

pub struct MongoConnection {
    config: ConnectionConfig,
    client: Option<MongoClient>,
    ssh_session: Option<Session>,
}

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub affected_rows: u64,
}

#[derive(Debug, Clone)]
pub struct DatabaseSchema {
    pub tables: Vec<TableInfo>,
    pub views: Vec<ViewInfo>,
}

#[derive(Debug, Clone)]
pub struct TableInfo {
    pub name: String,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Clone)]
pub struct ViewInfo {
    pub name: String,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub is_primary_key: bool,
}

impl PostgresConnection {
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            config,
            client: None,
            ssh_session: None,
        }
    }

   
    async fn setup_ssh_tunnel(&mut self) -> Result<TcpStream> {
        if let Some(ssh_config) = &self.config.ssh_tunnel {
            // SSH tunnel implementation
            let tcp = TcpStream::connect(&format!("{}:{}", ssh_config.host, ssh_config.port)).await?;
            let mut sess = Session::new()?;
            sess.set_tcp_stream(tcp);
            sess.handshake()?;
            
            if let Some(password) = &ssh_config.password {
                sess.userauth_password(&ssh_config.username, password)?;
            } else if let Some(key_path) = &ssh_config.private_key_path {
                sess.userauth_pubkey_file(
                    &ssh_config.username,
                    None,
                    std::path::Path::new(key_path),
                    None,
                )?;
            }

            // Create local TCP listener
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            let local_addr = listener.local_addr()?;

            // Set up the SSH tunnel
            sess.channel_direct_tcpip(
                &self.config.host,
                self.config.port.try_into().unwrap(),
                Some(("127.0.0.1", local_addr.port().try_into().unwrap())),
            )?;

            self.ssh_session = Some(sess);
            
            // Accept one connection
            let (stream, _) = listener.accept().await?;
            Ok(stream)
        } else {
            TcpStream::connect(&format!("{}:{}", self.config.host, self.config.port)).await
                .context("Failed to connect to database")
        }
    }

    async fn list_databases_query(&self) -> Result<Vec<String>> {
        if let Some(client) = &self.client {
            logging::debug("Executing list_databases query")?;
            
            let rows = client.query(
                "SELECT datname FROM pg_database 
                 WHERE datistemplate = false 
                 AND datname != 'postgres'  -- Exclude postgres system database
                 ORDER BY datname",
                &[],
            ).await?;
            
            let databases: Vec<String> = rows.iter()
                .map(|row| row.get::<_, String>(0))
                .collect();
            
            logging::debug(&format!("Found databases: {:?}", databases))?;
            Ok(databases)
        } else {
            let err = anyhow::anyhow!("Not connected to database");
            logging::error("Attempted to list databases without connection")?;
            Err(err)
        }
    }

    async fn list_schemas_query(&self, database: &str) -> Result<Vec<String>> {
        if let Some(client) = &self.client {
            logging::debug(&format!("Executing list_schemas query for database {}", database))?;
            
            let rows = client.query(
                "SELECT schema_name 
                 FROM information_schema.schemata 
                 WHERE schema_name NOT IN ('information_schema', 'pg_catalog')
                 ORDER BY schema_name",
                &[],
            ).await?;
            
            let schemas: Vec<String> = rows.iter()
                .map(|row| row.get::<_, String>(0))
                .collect();
            
            logging::debug(&format!("Found schemas: {:?}", schemas))?;
            Ok(schemas)
        } else {
            let err = anyhow::anyhow!("Not connected to database");
            logging::error("Attempted to list schemas without connection")?;
            Err(err)
        }
    }

    async fn list_tables_query(&self, schema: &str) -> Result<Vec<String>> {
        if let Some(client) = &self.client {
            logging::debug(&format!("Executing list_tables query for schema {}", schema))?;
            
            let rows = client.query(
                "SELECT table_name 
                 FROM information_schema.tables 
                 WHERE table_schema = $1 
                 AND table_type = 'BASE TABLE'
                 ORDER BY table_name",
                &[&schema],
            ).await?;
            
            let tables: Vec<String> = rows.iter()
                .map(|row| row.get::<_, String>(0))
                .collect();
            
            logging::debug(&format!("Found tables: {:?}", tables))?;
            Ok(tables)
        } else {
            let err = anyhow::anyhow!("Not connected to database");
            logging::error("Attempted to list tables without connection")?;
            Err(err)
        }
    }}

#[async_trait]
impl DatabaseConnection for PostgresConnection {
    async fn connect(&mut self) -> Result<()> {

        let stream = self.setup_ssh_tunnel().await?;
        
        let mut pg_config = tokio_postgres::Config::new();
        pg_config
            .user(&self.config.username)
            .password(self.config.password.as_deref().unwrap_or(""))
            .dbname(&self.config.database)
            .host(&self.config.host)
            .port(self.config.port);

        let (client, connection) = pg_config.connect_raw(stream, tokio_postgres::NoTls).await?;
        
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
                logging::error(&format!("Connection error: {}", e));
            }
        });

        self.client = Some(client);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.client = None;
        self.ssh_session = None;
        Ok(())
    }

    async fn execute_query(&self, query: &str) -> Result<QueryResult> {
    if let Some(client) = &self.client {
        logging::debug(&format!("Executing query: {}", query))?;
        let rows = client.query(query, &[]).await?;
        
        let columns = if !rows.is_empty() {
            rows[0]
                .columns()
                .iter()
                .map(|col| col.name().to_string())
                .collect()
        } else {
            vec![]
        };

        let result_rows: Vec<Vec<String>> = rows
            .iter()
            .map(|row| {
                (0..row.len())
                    .map(|i| {
                        let col = &row.columns()[i];
                        match col.type_().name() {
                            "int2" => {
                                if let Ok(Some(val)) = row.try_get::<_, Option<i16>>(i) {
                                    val.to_string()
                                } else {
                                    "NULL".to_string()
                                }
                            },
                            "int4" => {
                                if let Ok(Some(val)) = row.try_get::<_, Option<i32>>(i) {
                                    val.to_string()
                                } else {
                                    "NULL".to_string()
                                }
                            },
                            "int8" => {
                                if let Ok(Some(val)) = row.try_get::<_, Option<i64>>(i) {
                                    val.to_string()
                                } else {
                                    "NULL".to_string()
                                }
                            },
                            "float4" => {
                                if let Ok(Some(val)) = row.try_get::<_, Option<f32>>(i) {
                                    val.to_string()
                                } else {
                                    "NULL".to_string()
                                }
                            },
                            "float8" => {
                                if let Ok(Some(val)) = row.try_get::<_, Option<f64>>(i) {
                                    val.to_string()
                                } else {
                                    "NULL".to_string()
                                }
                            },
                            "bool" => {
                                if let Ok(Some(val)) = row.try_get::<_, Option<bool>>(i) {
                                    val.to_string()
                                } else {
                                    "NULL".to_string()
                                }
                            },
                            "varchar" | "text" | "name" | "char" | "json" | "jsonb" => {
                                if let Ok(Some(val)) = row.try_get::<_, Option<String>>(i) {
                                    val
                                } else {
                                    "NULL".to_string()
                                }
                            },
                            "timestamptz" => {
                                if let Ok(Some(val)) = row.try_get::<_, Option<chrono::DateTime<chrono::Utc>>>(i) {
                                    val.to_string()
                                } else {
                                    "NULL".to_string()
                                }
                            },
                            "timestamp" => {
                                if let Ok(Some(val)) = row.try_get::<_, Option<chrono::NaiveDateTime>>(i) {
                                    val.to_string()
                                } else {
                                    "NULL".to_string()
                                }
                            },
                            "date" => {
                                if let Ok(Some(val)) = row.try_get::<_, Option<chrono::NaiveDate>>(i) {
                                    val.to_string()
                                } else {
                                    "NULL".to_string()
                                }
                            },
                            _ => {
                                if let Ok(Some(val)) = row.try_get::<_, Option<String>>(i) {
                                    val
                                } else {
                                    "NULL".to_string()
                                }
                            }
                        }
                    })
                    .collect()
            })
            .collect();

        Ok(QueryResult {
            columns,
            rows: result_rows,
            affected_rows: rows.len() as u64,
        })
    } else {
        Err(anyhow::anyhow!("Not connected to database"))
    }
}

    async fn get_schema(&self) -> Result<DatabaseSchema> {
        if let Some(client) = &self.client {
            let tables_query = r#"
                SELECT 
                    t.table_name,
                    c.column_name,
                    c.data_type,
                    c.is_nullable,
                    tc.constraint_type = 'PRIMARY KEY' as is_primary_key
                FROM 
                    information_schema.tables t
                    JOIN information_schema.columns c ON t.table_name = c.column_name
                    LEFT JOIN information_schema.key_column_usage kcu ON 
                        c.table_name = kcu.column_name
                    LEFT JOIN information_schema.table_constraints tc ON 
                        kcu.constraint_name = tc.constraint_name
                WHERE 
                    t.table_schema = 'public'
                ORDER BY 
                    t.table_name, c.ordinal_position;
            "#;

            let rows = client.query(tables_query, &[]).await?;
            
            let mut schema = DatabaseSchema {
                tables: vec![],
                views: vec![],
            };

            let mut current_table = String::new();
            let mut current_columns = vec![];

            for row in rows {
                let table_name: String = row.get("table_name");
                
                if table_name != current_table && !current_table.is_empty() {
                    schema.tables.push(TableInfo {
                        name: current_table.clone(),
                        columns: current_columns.clone(),
                    });
                    current_columns.clear();
                }

                current_table = table_name;
                current_columns.push(ColumnInfo {
                    name: row.get("column_name"),
                    data_type: row.get("data_type"),
                    is_nullable: row.get("is_nullable"),
                    is_primary_key: row.get("is_primary_key"),
                });
            }

            if !current_table.is_empty() {
                schema.tables.push(TableInfo {
                    name: current_table,
                    columns: current_columns,
                });
            }

            Ok(schema)
        } else {
            Err(anyhow::anyhow!("Not connected to database"))
        }
    }

    fn clone_box(&self) -> Box<dyn DatabaseConnection> {
        Box::new(PostgresConnection::new(self.config.clone()))
    }

    async fn list_databases(&self) -> Result<Vec<String>> {
        self.list_databases_query().await
    }

    async fn list_schemas(&self, database: &str) -> Result<Vec<String>> {
        self.list_schemas_query(database).await
    }

    async fn list_tables(&self, schema: &str) -> Result<Vec<String>> {
        self.list_tables_query(schema).await
    }

    async fn fetch_table_data(
        &self,
        schema: &str,
        table: &str,
        order_by: Option<(String, bool)>
    ) -> Result<QueryResult> {
        let mut query = format!("SELECT * FROM {}.{}", schema, table);
        if let Some((col, asc)) = order_by {
            query.push_str(&format!(" ORDER BY {} {}", col, if asc { "ASC" } else { "DESC" }));
        }
        query.push_str(" LIMIT 1000");
        self.execute_query(&query).await
    }
}

impl MongoConnection {
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            config,
            client: None,
            ssh_session: None,
        }
    }

}

#[async_trait]
impl DatabaseConnection for MongoConnection {
    async fn connect(&mut self) -> Result<()> {
        // TODO: Implement MongoDB connection
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.client = None;
        self.ssh_session = None;
        Ok(())
    }

    async fn get_schema(&self) -> Result<DatabaseSchema> {
        // TODO: Implement MongoDB schema retrieval
        Ok(DatabaseSchema {
            tables: vec![],
            views: vec![],
        })
    }

    fn clone_box(&self) -> Box<dyn DatabaseConnection> {
        Box::new(MongoConnection::new(self.config.clone()))
    }

    async fn list_databases(&self) -> Result<Vec<String>> {
        if let Some(client) = &self.client {
            let db_names = client.list_database_names(None, None).await?;
            Ok(db_names)
        } else {
            Err(anyhow::anyhow!("Not connected to database"))
        }
    }

    async fn list_schemas(&self, _database: &str) -> Result<Vec<String>> {
        // MongoDB doesn't have schemas in the same way as PostgreSQL
        // Instead, we'll return a single "default" schema
        Ok(vec!["default".to_string()])
    }

    async fn list_tables(&self, _schema: &str) -> Result<Vec<String>> {
        if let Some(client) = &self.client {
            let db = client.database("admin"); // Use the specified database
            let collection_names = db.list_collection_names(None).await?;
            Ok(collection_names)
        } else {
            Err(anyhow::anyhow!("Not connected to database"))
        }
    }

    async fn execute_query(&self, _query: &str) -> Result<QueryResult> {
        Ok(QueryResult {
            columns: vec![],
            rows: vec![],
            affected_rows: 0,
        })
    }

    async fn fetch_table_data(&self, _schema: &str, _table: &str, order_by: Option<(String, bool)>) -> Result<QueryResult> {
        Ok(QueryResult {
            columns: vec![],
            rows: vec![],
            affected_rows: 0,
        })
    }
}
