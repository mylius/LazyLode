use async_trait::async_trait;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QueryParams {
    pub where_clause: Option<String>,
    pub order_by: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
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

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub affected_rows: u64,
}

#[async_trait]
pub trait DatabaseConnection: Send + Sync {
    /// Connect to the database
    async fn connect(&mut self) -> Result<()>;
    
    /// Disconnect from the database
    async fn disconnect(&mut self) -> Result<()>;
    
    /// List all available databases
    async fn list_databases(&self) -> Result<Vec<String>>;
    
    /// List all schemas in a database
    async fn list_schemas(&self, database: &str) -> Result<Vec<String>>;
    
    /// List all tables in a schema
    async fn list_tables(&self, schema: &str) -> Result<Vec<String>>;
    
    /// Execute a query with parameters
    async fn execute_query(&self, query: &str) -> Result<QueryResult>;
    
    /// Fetch table data with optional filtering and sorting
    async fn fetch_table_data(
        &self,
        schema: &str,
        table: &str,
        params: &QueryParams,
    ) -> Result<QueryResult>;

    /// Clone the connection
    fn clone_box(&self) -> Box<dyn DatabaseConnection>;
}
