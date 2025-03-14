use super::core::*;
use crate::logging;
use anyhow::Result;
use async_trait::async_trait;
use tokio_postgres::{Client, NoTls};

pub struct PostgresConnection {
    config: super::ConnectionConfig,
    client: Option<Client>,
}

impl PostgresConnection {
    pub fn new(config: super::ConnectionConfig) -> Self {
        Self {
            config,
            client: None,
        }
    }

    async fn setup_connection(&mut self) -> Result<Client> {
        let mut config = tokio_postgres::Config::new();
        config
            .host(&self.config.host)
            .port(self.config.port)
            .user(&self.config.username)
            .password(self.config.password.as_deref().unwrap_or(""))
            .dbname(&self.config.database);

        let (client, connection) = config.connect(NoTls).await?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                let _ = logging::error(&format!("Connection error: {}", e));
            }
        });
        Ok(client)
    }
}

fn sanitize_column_name(column: &str) -> String {
    // Remove any dangerous characters, only allow alphanumeric and underscore
    let sanitized: String = column
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    format!("\"{}\"", sanitized)
}

#[async_trait]
impl DatabaseConnection for PostgresConnection {
    async fn connect(&mut self) -> Result<()> {
        self.client = Some(self.setup_connection().await?);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.client = None;
        Ok(())
    }

    async fn list_databases(&self) -> Result<Vec<String>> {
        if let Some(client) = &self.client {
            let rows = client
                .query(
                    "SELECT datname FROM pg_database 
                 WHERE datistemplate = false 
                 ORDER BY datname",
                    &[],
                )
                .await?;

            Ok(rows.iter().map(|row| row.get::<_, String>(0)).collect())
        } else {
            Err(anyhow::anyhow!("Not connected to database"))
        }
    }

    async fn list_schemas(&self, _database: &str) -> Result<Vec<String>> {
        if let Some(client) = &self.client {
            let rows = client
                .query(
                    "SELECT schema_name 
                 FROM information_schema.schemata 
                 WHERE schema_name NOT IN ('information_schema', 'pg_catalog')
                 ORDER BY schema_name",
                    &[],
                )
                .await?;

            Ok(rows.iter().map(|row| row.get::<_, String>(0)).collect())
        } else {
            Err(anyhow::anyhow!("Not connected to database"))
        }
    }

    async fn list_tables(&self, schema: &str) -> Result<Vec<String>> {
        if let Some(client) = &self.client {
            let rows = client
                .query(
                    "SELECT table_name 
                 FROM information_schema.tables 
                 WHERE table_schema = $1 
                 AND table_type = 'BASE TABLE'
                 ORDER BY table_name",
                    &[&schema],
                )
                .await?;

            Ok(rows.iter().map(|row| row.get::<_, String>(0)).collect())
        } else {
            Err(anyhow::anyhow!("Not connected to database"))
        }
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
                                }
                                "int4" => {
                                    if let Ok(Some(val)) = row.try_get::<_, Option<i32>>(i) {
                                        val.to_string()
                                    } else {
                                        "NULL".to_string()
                                    }
                                }
                                "int8" => {
                                    if let Ok(Some(val)) = row.try_get::<_, Option<i64>>(i) {
                                        val.to_string()
                                    } else {
                                        "NULL".to_string()
                                    }
                                }
                                "float4" => {
                                    if let Ok(Some(val)) = row.try_get::<_, Option<f32>>(i) {
                                        val.to_string()
                                    } else {
                                        "NULL".to_string()
                                    }
                                }
                                "float8" => {
                                    if let Ok(Some(val)) = row.try_get::<_, Option<f64>>(i) {
                                        val.to_string()
                                    } else {
                                        "NULL".to_string()
                                    }
                                }
                                "bool" => {
                                    if let Ok(Some(val)) = row.try_get::<_, Option<bool>>(i) {
                                        val.to_string()
                                    } else {
                                        "NULL".to_string()
                                    }
                                }
                                "varchar" | "text" | "name" | "char" | "json" | "jsonb" => {
                                    if let Ok(Some(val)) = row.try_get::<_, Option<String>>(i) {
                                        val
                                    } else {
                                        "NULL".to_string()
                                    }
                                }
                                "timestamptz" => {
                                    if let Ok(Some(val)) =
                                        row.try_get::<_, Option<chrono::DateTime<chrono::Utc>>>(i)
                                    {
                                        val.to_string()
                                    } else {
                                        "NULL".to_string()
                                    }
                                }
                                "timestamp" => {
                                    if let Ok(Some(val)) =
                                        row.try_get::<_, Option<chrono::NaiveDateTime>>(i)
                                    {
                                        val.to_string()
                                    } else {
                                        "NULL".to_string()
                                    }
                                }
                                "date" => {
                                    if let Ok(Some(val)) =
                                        row.try_get::<_, Option<chrono::NaiveDate>>(i)
                                    {
                                        val.to_string()
                                    } else {
                                        "NULL".to_string()
                                    }
                                }
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

    async fn fetch_table_data(
        &self,
        schema: &str,
        table: &str,
        params: &QueryParams,
    ) -> Result<QueryResult> {
        logging::debug(&format!(
            "Fetching table data for schema {}, table {}",
            schema, table
        ))?;

        // Sanitize schema and table names
        let schema = sanitize_column_name(schema);
        let table = sanitize_column_name(table);

        let mut query = format!("SELECT * FROM {}.{}", schema, table);

        if let Some(where_clause) = &params.where_clause {
            if !where_clause.trim().is_empty() {
                query.push_str(&format!(" WHERE {}", where_clause));
            }
        }

        if let Some(order_by) = &params.order_by {
            if !order_by.trim().is_empty() {
                // Split by comma to handle multiple columns
                let orders: Vec<&str> = order_by.split(',').collect();
                let mut sanitized_orders = Vec::new();

                for order in orders {
                    let parts: Vec<&str> = order.trim().split_whitespace().collect();
                    if !parts.is_empty() {
                        let column = sanitize_column_name(parts[0]);
                        let direction = parts
                            .get(1)
                            .map(|d| d.to_uppercase())
                            .filter(|d| d == "ASC" || d == "DESC")
                            .unwrap_or_else(|| "ASC".to_string());
                        sanitized_orders.push(format!("{} {}", column, direction));
                    }
                }

                if !sanitized_orders.is_empty() {
                    query.push_str(" ORDER BY ");
                    query.push_str(&sanitized_orders.join(", "));
                }
            }
        }

        if let Some(limit) = params.limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = params.offset {
            query.push_str(&format!(" OFFSET {}", offset));
        }

        logging::debug(&format!("Executing query: {}", query))?;
        self.execute_query(&query).await
    }
}
