use super::core::*;
use super::ssh_tunnel::SshTunnelProcess;
use crate::logging;
use anyhow::Result;
use async_trait::async_trait;
use futures::TryStreamExt;
use mongodb::{
    bson::{doc, Bson, Document},
    options::{ClientOptions, FindOptions},
    Client, Database,
};
use std::collections::HashSet;

pub struct MongoConnection {
    config: super::ConnectionConfig,
    client: Option<Client>,
    current_db: Option<Database>,
    ssh_tunnel: Option<SshTunnelProcess>,
}

impl MongoConnection {
    pub fn new(config: super::ConnectionConfig) -> Self {
        Self {
            config,
            client: None,
            current_db: None,
            ssh_tunnel: None,
        }
    }

    async fn setup_connection(&mut self) -> Result<Client> {
        let (effective_host, effective_port) = if let Some(ref tunnel) = self.ssh_tunnel {
            ("127.0.0.1".to_string(), tunnel.local_port)
        } else {
            (self.config.host.clone(), self.config.port)
        };

        let connection_string = if !self.config.username.is_empty() {
            // Connection with authentication
            format!(
                "mongodb://{}:{}@{}:{}",
                self.config.username,
                self.config.password.as_deref().unwrap_or(""),
                effective_host,
                effective_port
            )
        } else {
            // Connection without authentication
            format!("mongodb://{}:{}", effective_host, effective_port)
        };

        let mut client_options = ClientOptions::parse(connection_string).await?;
        client_options.app_name = Some("LazyLode".to_string());
        Ok(Client::with_options(client_options)?)
    }

    async fn parse_sort_expression(&self, order_by: &str) -> Option<Document> {
        if order_by.trim().is_empty() {
            return None;
        }

        let mut sort_doc = Document::new();

        // Split by comma to handle multiple fields
        for order in order_by.split(',') {
            let parts: Vec<&str> = order.trim().split_whitespace().collect();
            if !parts.is_empty() {
                let field = parts[0].trim();
                if !field.is_empty() {
                    // Default to ascending (1) if no direction specified or invalid
                    let value =
                        if parts.get(1).map(|s| s.to_uppercase()) == Some("DESC".to_string()) {
                            -1
                        } else {
                            1
                        };
                    sort_doc.insert(field, value);
                }
            }
        }

        if sort_doc.is_empty() {
            None
        } else {
            Some(sort_doc)
        }
    }

    fn collect_field_names(&self, prefix: &str, doc: &Document, fields: &mut HashSet<String>) {
        for (key, value) in doc {
            let field_name = if prefix.is_empty() {
                key.to_string()
            } else {
                format!("{}.{}", prefix, key)
            };

            fields.insert(field_name.clone());

            // Recursively process nested documents
            if let Bson::Document(nested_doc) = value {
                self.collect_field_names(&field_name, nested_doc, fields);
            }
        }
    }

    // Helper function to get nested field values
    fn get_nested_field(&self, doc: &Document, field_path: &str) -> String {
        let parts: Vec<&str> = field_path.split('.').collect();
        let mut current = doc;

        for (i, &part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                // Last part - get the value
                return match current.get(part) {
                    Some(value) => MongoConnection::bson_to_string(value),
                    None => "null".to_string(),
                };
            } else {
                // Navigate to nested document
                match current.get(part) {
                    Some(&Bson::Document(ref nested)) => current = nested,
                    _ => return "null".to_string(),
                }
            }
        }

        "null".to_string()
    }

    fn bson_to_string(bson: &Bson) -> String {
        match bson {
            Bson::Int32(v) => v.to_string(),
            Bson::Int64(v) => v.to_string(),
            Bson::Double(v) => v.to_string(),
            Bson::String(v) => v.clone(),
            Bson::Boolean(v) => v.to_string(),
            Bson::ObjectId(v) => v.to_string(),
            Bson::DateTime(v) => v.to_string(),
            Bson::Null => "NULL".to_string(),
            _ => bson.to_string(),
        }
    }
}

#[async_trait]
impl DatabaseConnection for MongoConnection {
    async fn connect(&mut self) -> Result<()> {
        if let Some(ssh) = &self.config.ssh_tunnel {
            let tunnel = SshTunnelProcess::start(ssh, &self.config.host, self.config.port).await?;
            self.ssh_tunnel = Some(tunnel);
        }
        let client = self.setup_connection().await?;
        self.current_db = Some(client.database(&self.config.database));
        self.client = Some(client);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.client = None;
        self.current_db = None;
        if let Some(tunnel) = &mut self.ssh_tunnel {
            let _ = tunnel.stop().await;
        }
        self.ssh_tunnel = None;
        Ok(())
    }

    async fn list_databases(&self) -> Result<Vec<String>> {
        if let Some(client) = &self.client {
            let mut names = Vec::new();
            let dbs = client.list_database_names(None, None).await?;

            for name in dbs {
                if !name.starts_with("admin") && !name.starts_with("local") {
                    names.push(name);
                }
            }

            Ok(names)
        } else {
            Err(anyhow::anyhow!("Not connected to database"))
        }
    }

    /// List all schemas in a database
    /// For MongoDB, we only show the database name as the schema
    async fn list_schemas(&self, database: &str) -> Result<Vec<String>> {
        Ok(vec![database.to_string()])
    }

    async fn list_tables(&self, _schema: &str) -> Result<Vec<String>> {
        // For MongoDB, list collections under the default schema
        if let Some(client) = &self.client {
            let db = client.database(&self.config.database);
            Ok(db.list_collection_names(None).await?)
        } else {
            Err(anyhow::anyhow!("Not connected to database"))
        }
    }

    async fn execute_query(&self, query: &str) -> Result<QueryResult> {
        if let Some(db) = &self.current_db {
            let filter: Document = serde_json::from_str(query)?;
            let collection = db.collection::<Document>("default_collection");
            let mut cursor = collection.find(filter, None).await?;

            let mut columns = Vec::new();
            let mut rows = Vec::new();

            // First document determines the columns
            if let Some(doc) = cursor.try_next().await? {
                columns = doc.keys().map(|k| k.to_string()).collect();
                let row = columns
                    .iter()
                    .map(|k| Self::bson_to_string(doc.get(k).unwrap_or(&Bson::Null)))
                    .collect();
                rows.push(row);
            }

            // Process remaining documents
            while let Some(doc) = cursor.try_next().await? {
                let row = columns
                    .iter()
                    .map(|k| Self::bson_to_string(doc.get(k).unwrap_or(&Bson::Null)))
                    .collect();
                rows.push(row);
            }

            // Get the length before moving rows
            let affected_rows = rows.len() as u64;

            Ok(QueryResult {
                columns,
                rows,
                affected_rows,
            })
        } else {
            Err(anyhow::anyhow!("Not connected to database"))
        }
    }

    async fn fetch_table_data(
        &self,
        _collection: &str,
        table: &str,
        params: &QueryParams,
    ) -> Result<QueryResult> {
        if let Some(db) = &self.current_db {
            logging::debug(&format!("Fetching data from table: {}", table))?;

            let collection = db.collection::<Document>(table);
            let mut options = FindOptions::default();

            // Build filter from where clause
            let filter = match &params.where_clause {
                Some(where_clause) if !where_clause.trim().is_empty() => {
                    match serde_json::from_str(where_clause) {
                        Ok(filter) => filter,
                        Err(e) => {
                            logging::error(&format!("Invalid MongoDB filter: {}", e))?;
                            doc! {}
                        }
                    }
                }
                _ => doc! {},
            };

            // Handle sorting
            if let Some(order_by) = &params.order_by {
                if let Some(sort_doc) = self.parse_sort_expression(order_by).await {
                    logging::debug(&format!("Applying sort: {:?}", sort_doc))?;
                    options.sort = Some(sort_doc);
                }
            }

            // Handle pagination
            let limit = params.limit.unwrap_or(50).max(1) as i64;
            options.limit = Some(limit);

            if let Some(offset) = params.offset {
                options.skip = Some(offset as u64);
            }

            // First, get a sample document to determine the schema
            let mut columns = HashSet::new();

            // Sample a few documents to get a comprehensive schema
            let mut sample_options = FindOptions::default();
            sample_options.limit = Some(10);
            let mut sample_cursor = collection.find(doc! {}, sample_options).await?;

            while let Some(doc) = sample_cursor.try_next().await? {
                logging::debug(&format!("Sample doc: {:?}", doc))?;
                self.collect_field_names("", &doc, &mut columns);
            }

            // Convert HashSet to Vec and sort for consistent column order
            let mut columns: Vec<String> = columns.into_iter().collect();
            columns.sort();

            // Now fetch the actual data
            let mut cursor = collection.find(filter, options).await?;
            let mut rows = Vec::new();
            let mut affected_rows = 0;

            while let Some(doc) = cursor.try_next().await? {
                affected_rows += 1;
                let mut row = Vec::new();

                for column in &columns {
                    let value = if column.contains('.') {
                        // Handle nested fields
                        self.get_nested_field(&doc, column)
                    } else {
                        // Handle top-level fields
                        doc.get(column)
                            .map(Self::bson_to_string)
                            .unwrap_or_else(|| "null".to_string())
                    };
                    row.push(value);
                }

                rows.push(row);
            }

            Ok(QueryResult {
                columns,
                rows,
                affected_rows: affected_rows as u64,
            })
        } else {
            Err(anyhow::anyhow!("Not connected to database"))
        }
    }
}
