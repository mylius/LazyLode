use super::core::*;
use super::ssh_tunnel::SshTunnelProcess;
use crate::logging;
use anyhow::Result;
use async_trait::async_trait;
use tokio_postgres::{Client, NoTls};

pub struct PostgresConnection {
    config: super::ConnectionConfig,
    client: Option<Client>,
    ssh_tunnel: Option<SshTunnelProcess>,
}

impl PostgresConnection {
    pub fn new(config: super::ConnectionConfig) -> Self {
        Self {
            config,
            client: None,
            ssh_tunnel: None,
        }
    }

    async fn setup_connection(&mut self) -> Result<Client> {
        let mut config = tokio_postgres::Config::new();
        let (effective_host, effective_port) = if let Some(ref tunnel) = self.ssh_tunnel {
            ("127.0.0.1", tunnel.local_port)
        } else {
            (self.config.host.as_str(), self.config.port)
        };

        config
            .host(effective_host)
            .port(effective_port)
            .user(&self.config.username)
            .password(self.config.password.as_deref().unwrap_or(""))
            .dbname(
                self.config
                    .default_database
                    .as_deref()
                    .unwrap_or("postgres"),
            );

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

fn quote_identifier_exact(identifier: &str) -> String {
    let escaped = identifier.replace('"', "\"");
    format!("\"{}\"", escaped)
}

#[async_trait]
impl DatabaseConnection for PostgresConnection {
    async fn connect(&mut self) -> Result<()> {
        if let Some(ssh) = &self.config.ssh_tunnel {
            let tunnel = SshTunnelProcess::start(ssh, &self.config.host, self.config.port).await?;
            self.ssh_tunnel = Some(tunnel);
        }
        self.client = Some(self.setup_connection().await?);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.client = None;
        if let Some(tunnel) = &mut self.ssh_tunnel {
            let _ = tunnel.stop().await;
        }
        self.ssh_tunnel = None;
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
            logging::debug(&format!("Executing query: {}", query));
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
                                "varchar" | "text" | "name" | "char" => {
                                    if let Ok(Some(val)) = row.try_get::<_, Option<String>>(i) {
                                        val
                                    } else {
                                        "NULL".to_string()
                                    }
                                }
                                "json" | "jsonb" => {
                                    if let Ok(Some(val)) =
                                        row.try_get::<_, Option<serde_json::Value>>(i)
                                    {
                                        val.to_string()
                                    } else if let Ok(Some(val)) =
                                        row.try_get::<_, Option<String>>(i)
                                    {
                                        val
                                    } else {
                                        "NULL".to_string()
                                    }
                                }
                                "uuid" => {
                                    if let Ok(Some(val)) = row.try_get::<_, Option<uuid::Uuid>>(i) {
                                        val.to_string()
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
                                    } else if let Ok(Some(val)) = row.try_get::<_, Option<&str>>(i)
                                    {
                                        val.to_string()
                                    } else if let Ok(Some(val)) =
                                        row.try_get::<_, Option<uuid::Uuid>>(i)
                                    {
                                        val.to_string()
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
        ));

        // Prepare identifiers
        let schema_ident = sanitize_column_name(schema);
        let table_ident = sanitize_column_name(table);

        // Discover column names in ordinal order
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected to database"))?;

        let col_rows = client
            .query(
                "SELECT column_name
                 FROM information_schema.columns
                 WHERE table_schema = $1 AND table_name = $2
                 ORDER BY ordinal_position",
                &[&schema, &table],
            )
            .await?;

        let column_names: Vec<String> = col_rows.iter().map(|r| r.get::<_, String>(0)).collect();

        // Build select list casting each column to text to ensure enums/json/uuid display correctly
        let select_list = if column_names.is_empty() {
            "*".to_string()
        } else {
            column_names
                .iter()
                .map(|c| {
                    let ident = quote_identifier_exact(c);
                    format!("({})::text AS {}", ident, ident)
                })
                .collect::<Vec<_>>()
                .join(", ")
        };

        let mut query = format!(
            "SELECT {} FROM {}.{}",
            select_list, schema_ident, table_ident
        );

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

        logging::debug(&format!("Executing query: {}", query));
        self.execute_query(&query).await
    }

    async fn count_table_rows(
        &self,
        schema: &str,
        table: &str,
        where_clause: Option<&str>,
    ) -> Result<u64> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected to database"))?;

        let schema_ident = sanitize_column_name(schema);
        let table_ident = sanitize_column_name(table);

        let mut query = format!(
            "SELECT COUNT(*)::bigint FROM {}.{}",
            schema_ident, table_ident
        );
        if let Some(w) = where_clause {
            if !w.trim().is_empty() {
                query.push_str(&format!(" WHERE {}", w));
            }
        }

        let rows = client.query(&query, &[]).await?;
        let count: i64 = rows
            .get(0)
            .and_then(|r| r.try_get::<_, i64>(0).ok())
            .unwrap_or(0);
        Ok(u64::try_from(count).unwrap_or(0))
    }

    async fn lookup_foreign_key(
        &self,
        schema: &str,
        table: &str,
        column: &str,
    ) -> Result<Option<ForeignKeyTarget>> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected to database"))?;

        let rows = client
            .query(
                r#"
                SELECT
                    tc.table_schema AS src_schema,
                    tc.table_name AS src_table,
                    kcu.column_name AS src_column,
                    ccu.table_schema AS tgt_schema,
                    ccu.table_name AS tgt_table,
                    ccu.column_name AS tgt_column
                FROM information_schema.table_constraints AS tc
                JOIN information_schema.key_column_usage AS kcu
                  ON tc.constraint_name = kcu.constraint_name
                 AND tc.table_schema = kcu.table_schema
                JOIN information_schema.constraint_column_usage AS ccu
                  ON ccu.constraint_name = tc.constraint_name
                 AND ccu.table_schema = tc.table_schema
                WHERE tc.constraint_type = 'FOREIGN KEY'
                  AND tc.table_schema = $1
                  AND tc.table_name = $2
                  AND kcu.column_name = $3
                LIMIT 1
                "#,
                &[&schema, &table, &column],
            )
            .await?;

        if let Some(row) = rows.get(0) {
            let tgt_schema: String = row.get("tgt_schema");
            let tgt_table: String = row.get("tgt_table");
            let tgt_column: String = row.get("tgt_column");
            Ok(Some(ForeignKeyTarget {
                schema: tgt_schema,
                table: tgt_table,
                column: tgt_column,
            }))
        } else {
            Ok(None)
        }
    }
}
