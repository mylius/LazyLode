use super::core::*;
use anyhow::Result;
use async_trait::async_trait;
use rusqlite::{types::ValueRef, Row as SyncRow};
use tokio_rusqlite::Connection;

pub struct SqliteConnection {
    config: super::ConnectionConfig,
    conn: Option<Connection>,
}

impl SqliteConnection {
    pub fn new(config: super::ConnectionConfig) -> Self {
        Self { config, conn: None }
    }

    fn resolve_path(&self) -> String {
        if let Some(db) = &self.config.default_database {
            db.clone()
        } else {
            self.config.host.clone()
        }
    }

    fn value_ref_to_string(value: ValueRef<'_>) -> String {
        match value {
            ValueRef::Null => "NULL".to_string(),
            ValueRef::Integer(v) => v.to_string(),
            ValueRef::Real(v) => v.to_string(),
            ValueRef::Text(v) => String::from_utf8_lossy(v).to_string(),
            ValueRef::Blob(v) => format!("0x{}", hex::encode(v)),
        }
    }

    fn sanitize_identifier(name: &str) -> String {
        let sanitized: String = name
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        format!("\"{}\"", sanitized)
    }

    fn map_row_to_strings(row: &SyncRow<'_>, col_count: usize) -> rusqlite::Result<Vec<String>> {
        (0..col_count)
            .map(|i| {
                row.get_ref(i)
                    .map(Self::value_ref_to_string)
                    .map_err(|_| rusqlite::Error::InvalidColumnIndex(i))
            })
            .collect()
    }
}

#[async_trait]
impl DatabaseConnection for SqliteConnection {
    async fn connect(&mut self) -> Result<()> {
        let path = self.resolve_path();
        let conn = Connection::open(path).await?;
        self.conn = Some(conn);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.conn = None;
        Ok(())
    }

    async fn list_databases(&self) -> Result<Vec<String>> {
        Ok(vec!["main".to_string()])
    }

    async fn list_schemas(&self, _database: &str) -> Result<Vec<String>> {
        Ok(vec!["main".to_string()])
    }

    async fn list_tables(&self, _schema: &str) -> Result<Vec<String>> {
        if let Some(conn) = &self.conn {
            let tables = conn
                .call(|c: &mut rusqlite::Connection| -> tokio_rusqlite::Result<Vec<String>> {
                    let mut stmt = c.prepare(
                        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
                    )?;
                    let iter = stmt.query_map([], |row| row.get::<_, String>(0))?;
                    let mut out = Vec::new();
                    for t in iter {
                        out.push(t?);
                    }
                    Ok(out)
                })
                .await?;
            Ok(tables)
        } else {
            Err(anyhow::anyhow!("Not connected to database"))
        }
    }

    async fn execute_query(&self, query: &str) -> Result<QueryResult> {
        if let Some(conn) = &self.conn {
            let q = query.to_string();
            let upper = q.trim_start().to_ascii_uppercase();
            let is_select = upper.starts_with("SELECT");

            let result = conn.call(move |c: &mut rusqlite::Connection| -> tokio_rusqlite::Result<QueryResult> {
                if is_select {
                    let mut stmt = c.prepare(&q)?;
                    let col_count = stmt.column_count();
                    let columns: Vec<String> = stmt
                        .column_names()
                        .iter()
                        .map(|s| s.to_string())
                        .collect();
                    let mut rows_vec = Vec::new();
                    let mut rows = stmt.query([])?;
                    while let Some(row) = rows.next()? {
                        rows_vec.push(SqliteConnection::map_row_to_strings(row, col_count)?);
                    }
                    let affected_rows = rows_vec.len() as u64;
                    Ok(QueryResult { columns, rows: rows_vec, affected_rows })
                } else {
                    let affected = c.execute(&q, [])? as u64;
                    Ok(QueryResult { columns: Vec::new(), rows: Vec::new(), affected_rows: affected })
                }
            })
            .await?;
            Ok(result)
        } else {
            Err(anyhow::anyhow!("Not connected to database"))
        }
    }

    async fn fetch_table_data(
        &self,
        _schema: &str,
        table: &str,
        params: &QueryParams,
    ) -> Result<QueryResult> {
        let table_ident = Self::sanitize_identifier(table);
        let mut query = format!("SELECT * FROM {}", table_ident);
        if let Some(where_clause) = &params.where_clause {
            if !where_clause.trim().is_empty() {
                query.push_str(&format!(" WHERE {}", where_clause));
            }
        }
        if let Some(order_by) = &params.order_by {
            if !order_by.trim().is_empty() {
                let orders: Vec<&str> = order_by.split(',').collect();
                let mut sanitized_orders = Vec::new();
                for order in orders {
                    let parts: Vec<&str> = order.trim().split_whitespace().collect();
                    if !parts.is_empty() {
                        let col = Self::sanitize_identifier(parts[0]);
                        let dir = parts
                            .get(1)
                            .map(|d| d.to_ascii_uppercase())
                            .filter(|d| d == "ASC" || d == "DESC")
                            .unwrap_or_else(|| "ASC".to_string());
                        sanitized_orders.push(format!("{} {}", col, dir));
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
        self.execute_query(&query).await
    }

    async fn count_table_rows(
        &self,
        _schema: &str,
        table: &str,
        where_clause: Option<&str>,
    ) -> Result<u64> {
        if let Some(conn) = &self.conn {
            let tbl = Self::sanitize_identifier(table);
            let mut query = format!("SELECT COUNT(*) FROM {}", tbl);
            if let Some(w) = where_clause {
                if !w.trim().is_empty() {
                    query.push_str(&format!(" WHERE {}", w));
                }
            }
            let count = conn
                .call(move |c: &mut rusqlite::Connection| -> tokio_rusqlite::Result<u64> {
                    let mut stmt = c.prepare(&query)?;
                    let mut rows = stmt.query([])?;
                    if let Some(row) = rows.next()? {
                        let v: i64 = row.get(0)?;
                        Ok(u64::try_from(v).unwrap_or(0))
                    } else {
                        Ok(0)
                    }
                })
                .await?;
            Ok(count)
        } else {
            Err(anyhow::anyhow!("Not connected to database"))
        }
    }

    async fn lookup_foreign_key(
        &self,
        _schema: &str,
        table: &str,
        column: &str,
    ) -> Result<Option<ForeignKeyTarget>> {
        if let Some(conn) = &self.conn {
            let t = table.to_string();
            let c = column.to_string();
            let res = conn
                .call(move |conn: &mut rusqlite::Connection| -> tokio_rusqlite::Result<Option<ForeignKeyTarget>> {
                    let mut stmt = conn.prepare(&format!(
                        "PRAGMA foreign_key_list({})",
                        SqliteConnection::sanitize_identifier(&t)
                    ))?;
                    let mut rows = stmt.query([])?;
                    while let Some(row) = rows.next()? {
                        let from_col: String = row.get(3)?;
                        if from_col == c {
                            let target_table: String = row.get(2)?;
                            let target_column: String = row.get(4)?;
                            return Ok(Some(ForeignKeyTarget {
                                schema: "main".to_string(),
                                table: target_table,
                                column: target_column,
                            }));
                        }
                    }
                    Ok(None)
                })
                .await?;
            Ok(res)
        } else {
            Err(anyhow::anyhow!("Not connected to database"))
        }
    }

    async fn get_columns(&self, _schema: &str, table: &str) -> Result<Vec<ColumnInfo>> {
        if let Some(conn) = &self.conn {
            let table_name = table.to_string();
            let columns = conn
                .call(move |c: &mut rusqlite::Connection| -> tokio_rusqlite::Result<Vec<ColumnInfo>> {
                    let query = format!("PRAGMA table_info({})", Self::sanitize_identifier(&table_name));
                    let mut stmt = c.prepare(&query)?;
                    let mut rows = stmt.query([])?;

                    let mut columns = Vec::new();
                    while let Some(row) = rows.next()? {
                        let column_name: String = row.get(1)?;
                        let data_type: String = row.get(2)?;
                        let not_null: i32 = row.get(3)?;
                        let pk_position: i32 = row.get(5)?;

                        columns.push(ColumnInfo {
                            name: column_name,
                            data_type,
                            is_nullable: not_null == 0,
                            is_primary_key: pk_position > 0,
                        });
                    }
                    Ok(columns)
                })
                .await?;

            Ok(columns)
        } else {
            Err(anyhow::anyhow!("Not connected to database"))
        }
    }
}


