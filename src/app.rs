//! `app.rs` - Defines the main application logic and data structures.
use crate::command::CommandBuffer;
use crate::input::{NavigationAction, TreeAction};
use crate::logging;
use crate::ui::types::Direction;
use crate::navigation::types::Pane;
use crate::navigation::{NavigationManager, NavigationConfig, NavigationState};
use clipboard::{ClipboardContext, ClipboardProvider};

use crate::config::Config;
use crate::database::core::ForeignKeyTarget;
use crate::database::{
    ConnectionConfig, ConnectionManager, ConnectionStatus, DatabaseConnection, DatabaseType,
    QueryParams, QueryResult,
};
use anyhow::Result;
use std::collections::{HashMap, HashSet};

/// Represents the form data for creating or editing a database connection.
#[derive(Default, Clone)]
pub struct ConnectionForm {
    pub name: String,
    pub db_type: DatabaseType,
    pub host: String,
    pub port: String,
    pub username: String,
    pub password: String,
    pub database: String,
    pub ssh_enabled: bool,
    pub ssh_host: String,
    pub ssh_port: String,
    pub ssh_username: String,
    pub ssh_password: String,
    pub ssh_key_path: String,
    pub ssh_tunnel_name: Option<String>,
    pub current_field: usize,
    pub editing_index: Option<usize>,
}

/// Represents the query state for a single tab/table
#[derive(Clone, Default)]
pub struct QueryState {
    pub where_clause: String,
    pub order_by_clause: String,
    pub page_size: u32,
    pub current_page: u32,
    pub total_pages: Option<u32>,
    pub total_records: Option<u64>,
    pub sort_column: Option<String>,
    pub sort_order: Option<bool>,
    pub rows_marked_for_deletion: HashSet<usize>,
}

/// Represents the input mode of the application.
#[derive(PartialEq, Clone)]
pub enum InputMode {
    /// Normal mode for navigation and command execution.
    Normal,
    /// Insert mode for text input in forms and queries.
    Insert,
    /// Command mode for entering commands.
    Command,
}

/// Represents the currently active block or UI element.
#[derive(PartialEq, Clone)]
pub enum ActiveBlock {
    /// The connections tree block.
    Connections,
    /// The query input block.
    Query,
    /// The query form block (for structured query building).
    QueryForm,
    /// The results display block.
    Results,
    /// The connection modal for adding/editing connections.
    ConnectionModal,
    /// The schema explorer block.
    SchemaExplorer,
    /// The command input block.
    CommandInput,
    DeletionConfirmModal,
}

/// Represents the query mode (free-form SQL or structured form).
#[derive(PartialEq, Clone)]
pub enum QueryMode {
    /// Free-form SQL query mode.
    FreeForm,
    /// Structured query form mode.
    StructuredForm,
}

/// Represents the structured query form data.
#[derive(Clone)]
pub struct QueryForm {
    /// The table name for the query.
    pub table: String,
    /// Conditions for the WHERE clause (column, operator, value).
    pub conditions: Vec<(String, String, String)>,
    /// Order by clauses (column, is_ascending).
    pub order_by: Vec<(String, bool)>,
    /// Limit for the query results.
    pub limit: Option<u32>,
}

/// Represents an item in the connection tree.
#[derive(PartialEq, Debug, Clone, Copy)] // Add PartialEq here
pub enum TreeItem {
    /// Represents a connection at a given index.
    Connection(usize),
    /// Represents a database within a connection at given indices (connection, database).
    Database(usize, usize),
    /// Represents a schema within a database and connection (connection, database, schema).
    Schema(usize, usize, usize),
    /// Represents a table within a schema, database, and connection (connection, database, schema, table).
    Table(usize, usize, usize, usize),
}

/// Represents a connection item in the connection tree.
#[derive(Clone)]
pub struct ConnectionTreeItem {
    /// The configuration for the database connection.
    pub connection_config: ConnectionConfig,
    /// The current status of the connection.
    pub status: ConnectionStatus,
    /// List of databases under this connection.
    pub databases: Vec<DatabaseTreeItem>,
    /// Whether the connection is expanded in the tree.
    pub is_expanded: bool,
}

/// Represents a database item in the connection tree.
#[derive(Clone)]
pub struct DatabaseTreeItem {
    /// The name of the database.
    pub name: String,
    /// List of schemas within this database.
    pub schemas: Vec<SchemaTreeItem>,
    /// Whether the database is expanded in the tree.
    pub is_expanded: bool,
}

/// Represents a schema item in the connection tree.
#[derive(Clone)]
pub struct SchemaTreeItem {
    /// The name of the schema.
    pub name: String,
    /// List of tables within this schema.
    pub tables: Vec<String>,
    /// Whether the schema is expanded in the tree.
    pub is_expanded: bool,
}

/// The main application struct.
pub struct App {
    pub should_quit: bool,
    pub active_block: ActiveBlock,
    pub show_connection_modal: bool,
    pub saved_connections: Vec<ConnectionConfig>,
    pub selected_connection_idx: Option<usize>,
    pub connection_statuses: HashMap<String, ConnectionStatus>,
    pub connection_form: ConnectionForm,
    pub input_mode: InputMode,
    pub free_query: String,
    pub result_tabs: Vec<(String, QueryResult, QueryState)>,
    pub config: Config,
    pub status_message: Option<String>,
    pub command_input: String,
    pub cursor_position: (usize, usize),
    pub active_pane: Pane,
    pub connection_tree: Vec<ConnectionTreeItem>,
    pub last_table_info: Option<(String, String, String)>,
    pub selected_result_tab_index: Option<usize>,
    pub show_deletion_modal: bool,
    pub connection_manager: ConnectionManager,
    pub connections: HashMap<String, Box<dyn DatabaseConnection>>,
    pub command_buffer: CommandBuffer,
    pub clipboard: String,
    pub last_key_was_d: bool,
    pub awaiting_replace: bool,
    /// New navigation system
    pub navigation_manager: NavigationManager,
}

impl App {
    /// Constructs a new `App` instance with default settings and loads configurations.
    pub fn new() -> Self {
        let config = Config::new();
        let navigation_config = config.navigation.clone();
        let mut app = Self {
            should_quit: false,
            active_block: ActiveBlock::Connections,
            show_connection_modal: false,
            saved_connections: config.connections.clone(),
            selected_connection_idx: None,
            connection_statuses: config
                .connections
                .iter()
                .map(|c| (c.name.clone(), ConnectionStatus::NotConnected))
                .collect(),
            connection_form: ConnectionForm::default(),
            input_mode: InputMode::Normal,
            free_query: String::new(),
            result_tabs: Vec::new(),
            config,
            status_message: None,
            command_input: String::new(),
            connections: HashMap::new(),
            cursor_position: (0, 0),
            active_pane: Pane::default(),
            connection_tree: Vec::new(),
            last_table_info: None,
            selected_result_tab_index: None,
            show_deletion_modal: false,
            connection_manager: ConnectionManager::new(),
            command_buffer: CommandBuffer::new(),
            clipboard: String::new(),
            last_key_was_d: false,
            awaiting_replace: false,
            navigation_manager: NavigationManager::new(navigation_config),
        };

        app.load_connections();

        app.connection_tree = app
            .config
            .connections
            .iter()
            .map(|conn| ConnectionTreeItem {
                connection_config: conn.clone(),
                status: ConnectionStatus::NotConnected,
                databases: Vec::new(),
                is_expanded: false,
            })
            .collect();

        app
    }

    pub async fn follow_foreign_key(&mut self) -> Result<()> {
        let (conn_name, current_schema, current_table) = match &self.last_table_info {
            Some(info) => info.clone(),
            None => return Ok(()),
        };

        let (current_col_name, current_cell_value) = {
            let (col, val) = if let Some(idx) = self.selected_result_tab_index {
                if let Some((_, result, _)) = self.result_tabs.get(idx) {
                    let col_idx = self.cursor_position.0;
                    let row_idx = self.cursor_position.1;
                    let col_name = result.columns.get(col_idx).cloned();
                    let cell_val = result
                        .rows
                        .get(row_idx)
                        .and_then(|r| r.get(col_idx))
                        .cloned();
                    (col_name, cell_val)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };
            match (col, val) {
                (Some(c), Some(v)) if v.to_ascii_uppercase() != "NULL" => (c, v),
                _ => return Ok(()),
            }
        };

        let db = match self.connection_manager.get_connection(&conn_name) {
            Some(db) => db,
            None => return Ok(()),
        };

        if let Some(ForeignKeyTarget {
            schema,
            table,
            column,
        }) = db
            .lookup_foreign_key(&current_schema, &current_table, &current_col_name)
            .await?
        {
            let where_clause = format!(
                "{} = '{}'",
                column.replace('"', "\""),
                current_cell_value.replace('\'', "''")
            );
            let params = QueryParams {
                where_clause: Some(where_clause),
                order_by: None,
                limit: Some(50),
                offset: None,
            };

            let result = db.fetch_table_data(&schema, &table, &params).await?;

            let tab_name = format!("{}.{}", schema, table);
            let tab_index = self
                .result_tabs
                .iter()
                .position(|(name, _, _)| name == &tab_name);

            let mut query_state = QueryState {
                page_size: 50,
                current_page: 1,
                total_pages: Some(1),
                total_records: Some(0),
                sort_column: None,
                sort_order: None,
                rows_marked_for_deletion: HashSet::new(),
                where_clause: params.where_clause.clone().unwrap_or_default(),
                order_by_clause: String::new(),
            };

            let total_records = db
                .count_table_rows(&schema, &table, params.where_clause.as_deref())
                .await
                .unwrap_or(result.rows.len() as u64);
            let page_size = query_state.page_size.max(1);
            let total_pages =
                ((total_records + page_size as u64 - 1) / page_size as u64).max(1) as u32;

            if let Some(index) = tab_index {
                self.selected_result_tab_index = Some(index);
                if let Some((_, ref mut result_slot, ref mut state)) =
                    self.result_tabs.get_mut(index)
                {
                    *result_slot = result;
                    state.total_records = Some(total_records);
                    state.total_pages = Some(total_pages);
                    state.current_page = 1;
                    state.where_clause = query_state.where_clause.clone();
                }
            } else {
                query_state.total_records = Some(total_records);
                query_state.total_pages = Some(total_pages);
                self.result_tabs.push((tab_name, result, query_state));
                self.selected_result_tab_index = Some(self.result_tabs.len() - 1);
                self.cursor_position = (0, 0);
            }

            self.last_table_info = Some((conn_name, schema, table));
            self.active_pane = Pane::Results;
        }

        Ok(())
    }

    /// Sets the `should_quit` flag to true, signaling the application to terminate.
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Toggles the visibility of the connection modal.
    pub fn toggle_connection_modal(&mut self) {
        self.show_connection_modal = !self.show_connection_modal;
        if self.show_connection_modal {
            self.active_block = ActiveBlock::ConnectionModal;
        } else {
            self.input_mode = InputMode::Normal;
        }
    }

    /// Saves a new connection based on the data in `connection_form`.
    pub fn save_connection(&mut self) {
        let new_connection = ConnectionConfig {
            name: self.connection_form.name.clone(),
            db_type: self.connection_form.db_type.clone(),
            host: self.connection_form.host.clone(),
            port: self.connection_form.port.parse().unwrap_or(5432),
            username: self.connection_form.username.clone(),
            password: Some(self.connection_form.password.clone()),
            database: self.connection_form.database.clone(),
            ssh_tunnel: None,
            ssh_tunnel_name: self.connection_form.ssh_tunnel_name.clone(),
        };

        self.saved_connections.push(new_connection.clone());
        self.config
            .save_connections(&self.saved_connections)
            .expect("Failed to save connections");

        // Add to connection tree
        self.connection_tree.push(ConnectionTreeItem {
            connection_config: new_connection,
            status: ConnectionStatus::NotConnected,
            databases: Vec::new(),
            is_expanded: false,
        });

        self.connection_form = ConnectionForm::default();
    }

    /// Loads connections from the configuration file.
    pub fn load_connections(&mut self) {
        self.saved_connections = self.config.load_connections().unwrap_or_else(|err| {
            eprintln!("Error loading connections: {}", err);
            Vec::new()
        });

        self.connection_statuses = self
            .saved_connections
            .iter()
            .map(|c| (c.name.clone(), ConnectionStatus::NotConnected))
            .collect();
    }

    /// Moves the cursor within the results table based on the given direction.
    pub fn move_cursor_in_results(&mut self, direction: Direction) {
        if let Some(selected_tab_index) = self.selected_result_tab_index {
            if let Some((_, result, _)) = self.result_tabs.get(selected_tab_index) {
                match direction {
                    Direction::Left => {
                        if self.cursor_position.0 > 0 {
                            self.cursor_position.0 -= 1;
                        }
                    }
                    Direction::Right => {
                        if self.cursor_position.0 < result.columns.len().saturating_sub(1) {
                            self.cursor_position.0 += 1;
                        }
                    }
                    Direction::Up => {
                        if self.cursor_position.1 > 0 {
                            self.cursor_position.1 -= 1;
                        }
                    }
                    Direction::Down => {
                        if self.cursor_position.1 < result.rows.len().saturating_sub(1) {
                            self.cursor_position.1 += 1;
                        }
                    }
                }
            }
        }
    }

    pub fn get_current_field_length(&self) -> usize {
        if let Some(state) = self.current_query_state() {
            match self.cursor_position.0 {
                0 => state.where_clause.len(),
                1 => state.order_by_clause.len(),
                _ => 0,
            }
        } else {
            0
        }
    }

    /// Handles navigation actions based on the current active pane.
    pub fn handle_navigation(&mut self, action: NavigationAction) {
        match action {
            NavigationAction::Direction(direction) => {
                match self.active_pane {
                    Pane::QueryInput => {
                        match direction {
                            Direction::Up => {
                                self.cursor_position.0 = 0; // WHERE clause
                                let len = self.get_current_field_length();
                                self.cursor_position.1 = self.cursor_position.1.min(len);
                            }
                            Direction::Down => {
                                self.cursor_position.0 = 1; // ORDER BY clause
                                let len = self.get_current_field_length();
                                self.cursor_position.1 = self.cursor_position.1.min(len);
                            }
                            Direction::Left => {
                                if self.cursor_position.1 > 0 {
                                    self.cursor_position.1 -= 1;
                                }
                            }
                            Direction::Right => {
                                let max_pos = self.get_current_field_length();
                                if self.cursor_position.1 < max_pos {
                                    self.cursor_position.1 += 1;
                                }
                            }
                        }
                    }
                    _ => {} // Handle other panes as before
                }
            }
            NavigationAction::NextTab => self.select_next_tab(),
            NavigationAction::PreviousTab => self.select_previous_tab(),
            _ => {} // Handle other actions as before
        }
        self.command_buffer.clear();
    }

    pub fn focus_where_input(&mut self) {
        self.active_pane = Pane::QueryInput;
        self.input_mode = InputMode::Insert;
        self.cursor_position.0 = 0;
        let len = self
            .current_query_state()
            .map(|state| state.where_clause.len())
            .unwrap_or(0);
        self.cursor_position.1 = len;
        self.last_key_was_d = false;
        self.awaiting_replace = false;
    }

    /// Handles tree-related actions such as expand and collapse.
    pub async fn handle_tree_action(&mut self, action: TreeAction) -> Result<()> {
        if self.active_pane == Pane::Connections {
            if let Some(idx) = self.selected_connection_idx {
                match action {
                    TreeAction::Expand => {
                        logging::debug(&format!("Expanding connection at visual index {}", idx))
                            .unwrap_or_else(|e| eprintln!("Logging error: {}", e));
                        self.toggle_tree_item(idx).await?;
                    }
                    TreeAction::Collapse => {
                        // Just collapse without making any async calls
                        if let Some(tree_item) = self.get_tree_item_at_visual_index(idx) {
                            match tree_item {
                                TreeItem::Connection(conn_idx) => {
                                    if let Some(connection) = self.connection_tree.get_mut(conn_idx)
                                    {
                                        connection.is_expanded = false;
                                    }
                                }
                                TreeItem::Database(conn_idx, db_idx) => {
                                    if let Some(connection) = self.connection_tree.get_mut(conn_idx)
                                    {
                                        if let Some(database) = connection.databases.get_mut(db_idx)
                                        {
                                            database.is_expanded = false;
                                        }
                                    }
                                }
                                TreeItem::Schema(conn_idx, db_idx, schema_idx) => {
                                    if let Some(connection) = self.connection_tree.get_mut(conn_idx)
                                    {
                                        if let Some(database) = connection.databases.get_mut(db_idx)
                                        {
                                            if let Some(schema) =
                                                database.schemas.get_mut(schema_idx)
                                            {
                                                schema.is_expanded = false;
                                            }
                                        }
                                    }
                                }
                                TreeItem::Table(_, _, _, _) => {} // Tables don't expand/collapse
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Expands a connection in the tree to show databases.
    pub async fn expand_connection(&mut self, index: usize) -> Result<()> {
        logging::debug(&format!(
            "Attempting to expand connection at index {}",
            index
        ))?;

        if let Some(connection) = self.connection_tree.get_mut(index) {
            if !connection.is_expanded {
                connection.status = ConnectionStatus::Connecting;
                logging::info(&format!(
                    "Connecting to database: {}",
                    connection.connection_config.name
                ))?;

                // Try to connect
                let mut cfg = connection.connection_config.clone();
                if cfg.ssh_tunnel.is_none() {
                    if let Some(name) = &cfg.ssh_tunnel_name {
                        if let Some(tunnel) = self
                            .config
                            .ssh_tunnels
                            .iter()
                            .find(|t| &t.name == name)
                            .cloned()
                        {
                            cfg.ssh_tunnel = Some(tunnel.config);
                        }
                    }
                }

                match self.connection_manager.connect(cfg).await {
                    Ok(_) => {
                        if let Some(db_conn) = self
                            .connection_manager
                            .get_connection(&connection.connection_config.name)
                        {
                            match db_conn.list_databases().await {
                                Ok(databases) => {
                                    logging::debug(&format!(
                                        "Found {} databases",
                                        databases.len()
                                    ))?;

                                    connection.databases = databases
                                        .into_iter()
                                        .map(|db_name| DatabaseTreeItem {
                                            name: db_name,
                                            schemas: Vec::new(),
                                            is_expanded: false,
                                        })
                                        .collect();

                                    connection.status = ConnectionStatus::Connected;
                                    connection.is_expanded = true;

                                    logging::info(&format!(
                                        "Successfully expanded connection {}",
                                        connection.connection_config.name
                                    ))?;
                                }
                                Err(e) => {
                                    connection.status = ConnectionStatus::Failed;
                                    let error_msg = format!("Failed to list databases: {}", e);
                                    logging::error(&error_msg)?;
                                    return Err(anyhow::anyhow!(error_msg));
                                }
                            }
                        } else {
                            connection.status = ConnectionStatus::Failed;
                            let error_msg =
                                "Connection not found after successful connection".to_string();
                            logging::error(&error_msg)?;
                            return Err(anyhow::anyhow!(error_msg));
                        }
                    }
                    Err(e) => {
                        connection.status = ConnectionStatus::Failed;
                        let error_msg = format!("Failed to connect: {}", e);
                        logging::error(&error_msg)?;
                        return Err(anyhow::anyhow!(error_msg));
                    }
                }
            } else {
                // If already expanded, just collapse
                connection.is_expanded = false;
                logging::debug(&format!(
                    "Collapsed connection {}",
                    connection.connection_config.name
                ))?;
            }
        } else {
            let error_msg = format!("Connection at index {} not found", index);
            logging::error(&error_msg)?;
            return Err(anyhow::anyhow!(error_msg));
        }

        Ok(())
    }

    /// Expands a database in the tree to show schemas.
    pub async fn expand_database(&mut self, conn_idx: usize, db_idx: usize) -> Result<()> {
        if let Some(connection) = self.connection_tree.get_mut(conn_idx) {
            if let Some(database) = connection.databases.get_mut(db_idx) {
                if !database.is_expanded {
                    if let Some(db_conn) = self
                        .connection_manager
                        .get_connection(&connection.connection_config.name)
                    {
                        match db_conn.list_schemas(&database.name).await {
                            Ok(schemas) => {
                                database.schemas = schemas
                                    .into_iter()
                                    .map(|schema_name| SchemaTreeItem {
                                        name: schema_name,
                                        tables: Vec::new(),
                                        is_expanded: false,
                                    })
                                    .collect();
                                database.is_expanded = true;
                            }
                            Err(e) => {
                                logging::error(&format!("Failed to list schemas: {}", e))?;
                                return Err(e);
                            }
                        }
                    }
                } else {
                    database.is_expanded = false;
                }
            }
        }
        Ok(())
    }

    /// Expands a schema in the tree to show tables.
    pub async fn expand_schema(
        &mut self,
        conn_idx: usize,
        db_idx: usize,
        schema_idx: usize,
    ) -> Result<()> {
        logging::debug(&format!(
            "Attempting to expand schema at connection {}, database {}, schema {}",
            conn_idx, db_idx, schema_idx
        ))?;

        if let Some(connection) = self.connection_tree.get_mut(conn_idx) {
            if let Some(database) = connection.databases.get_mut(db_idx) {
                if let Some(schema) = database.schemas.get_mut(schema_idx) {
                    if !schema.is_expanded {
                        if let Some(db_connection) = self
                            .connection_manager
                            .get_connection(&connection.connection_config.name)
                        {
                            match db_connection.list_tables(&schema.name).await {
                                Ok(tables) => {
                                    logging::debug(&format!(
                                        "Found {} tables in schema {}",
                                        tables.len(),
                                        schema.name
                                    ))?;
                                    schema.tables = tables;
                                    schema.is_expanded = true;
                                    logging::info(&format!(
                                        "Successfully expanded schema {}",
                                        schema.name
                                    ))?;
                                }
                                Err(e) => {
                                    logging::error(&format!("Failed to list tables: {}", e))?;
                                    return Err(e.into());
                                }
                            }
                        } else {
                            let err = anyhow::anyhow!("Connection not found in connection manager");
                            logging::error(&format!(
                                "Connection {} not found in connection manager",
                                connection.connection_config.name
                            ))?;
                            return Err(err);
                        }
                    } else {
                        schema.is_expanded = false;
                        logging::debug(&format!("Collapsed schema {}", schema.name))?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Edits an existing connection based on the data in `connection_form`.
    pub fn edit_connection(&mut self) {
        if let Some(index) = self.connection_form.editing_index {
            let updated_connection = ConnectionConfig {
                name: self.connection_form.name.clone(),
                db_type: self.connection_form.db_type.clone(),
                host: self.connection_form.host.clone(),
                port: self.connection_form.port.parse().unwrap_or(5432),
                username: self.connection_form.username.clone(),
                password: Some(self.connection_form.password.clone()),
                database: self.connection_form.database.clone(),
                ssh_tunnel: None,
                ssh_tunnel_name: self.connection_form.ssh_tunnel_name.clone(),
            };

            self.saved_connections[index] = updated_connection.clone();
            self.config
                .save_connections(&self.saved_connections)
                .expect("Failed to save connections");

            // Update the connection tree
            if let Some(tree_item) = self.connection_tree.get_mut(index) {
                tree_item.connection_config = updated_connection;
            }

            self.connection_form = ConnectionForm::default();
        }
    }

    pub fn delete_connection(&mut self) {
        if let Some(index) = self.selected_connection_idx {
            // Remove the connection from saved_connections
            self.saved_connections.remove(index);
            self.config
                .save_connections(&self.saved_connections)
                .expect("Failed to save connections");

            // Remove from connection tree
            self.connection_tree.remove(index);

            // Update the selected index
            if self.connection_tree.is_empty() {
                self.selected_connection_idx = None; // No connections left
            } else if index >= self.connection_tree.len() {
                self.selected_connection_idx = Some(self.connection_tree.len() - 1);
                // Select the last connection
            }
        }
    }

    /// Calculates the total number of visible items in the connection tree.
    pub fn get_total_visible_items(&self) -> usize {
        let mut total = 0;
        for connection in &self.connection_tree {
            total += 1; // Count the connection itself
            if connection.is_expanded {
                for database in &connection.databases {
                    total += 1; // Count the database
                    if database.is_expanded {
                        for schema in &database.schemas {
                            total += 1; // Count the schema
                            if schema.is_expanded {
                                total += schema.tables.len(); // Count all tables
                            }
                        }
                    }
                }
            }
        }
        total
    }

    /// Gets the tree item at a specific visual index (considering expanded items).
    pub fn get_tree_item_at_visual_index(&self, visual_index: usize) -> Option<TreeItem> {
        let mut current_visual_index = 0;

        for (conn_idx, connection) in self.connection_tree.iter().enumerate() {
            if current_visual_index == visual_index {
                return Some(TreeItem::Connection(conn_idx));
            }
            current_visual_index += 1;

            if connection.is_expanded {
                for (db_idx, database) in connection.databases.iter().enumerate() {
                    if current_visual_index == visual_index {
                        return Some(TreeItem::Database(conn_idx, db_idx));
                    }
                    current_visual_index += 1;

                    if database.is_expanded {
                        for (schema_idx, schema) in database.schemas.iter().enumerate() {
                            if current_visual_index == visual_index {
                                return Some(TreeItem::Schema(conn_idx, db_idx, schema_idx));
                            }
                            current_visual_index += 1;

                            if schema.is_expanded {
                                for (table_idx, _) in schema.tables.iter().enumerate() {
                                    if current_visual_index == visual_index {
                                        return Some(TreeItem::Table(
                                            conn_idx, db_idx, schema_idx, table_idx,
                                        ));
                                    }
                                    current_visual_index += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Gets the visual index for a given tree item.
    pub fn get_visual_index_for_tree_item(&self, tree_item: &TreeItem) -> Option<usize> {
        let mut current_visual_index = 0;

        for (conn_idx, connection) in self.connection_tree.iter().enumerate() {
            if *tree_item == TreeItem::Connection(conn_idx) {
                return Some(current_visual_index);
            }
            current_visual_index += 1;

            if connection.is_expanded {
                for (db_idx, database) in connection.databases.iter().enumerate() {
                    if *tree_item == TreeItem::Database(conn_idx, db_idx) {
                        return Some(current_visual_index);
                    }
                    current_visual_index += 1;

                    if database.is_expanded {
                        for (schema_idx, schema) in database.schemas.iter().enumerate() {
                            if *tree_item == TreeItem::Schema(conn_idx, db_idx, schema_idx) {
                                return Some(current_visual_index);
                            }
                            current_visual_index += 1;

                            if schema.is_expanded {
                                for (table_idx, _) in schema.tables.iter().enumerate() {
                                    if *tree_item
                                        == TreeItem::Table(conn_idx, db_idx, schema_idx, table_idx)
                                    {
                                        return Some(current_visual_index);
                                    }
                                    current_visual_index += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Get current query state
    pub fn current_query_state(&self) -> Option<&QueryState> {
        self.selected_result_tab_index
            .and_then(|idx| self.result_tabs.get(idx))
            .map(|(_, _, state)| state)
    }

    /// Get mutable current query state
    pub fn current_query_state_mut(&mut self) -> Option<&mut QueryState> {
        self.selected_result_tab_index
            .and_then(|idx| self.result_tabs.get_mut(idx))
            .map(|(_, _, state)| state)
    }

    /// Insert character at cursor position in current query field
    pub fn insert_char(&mut self, c: char) {
        // Get cursor positions before mutable borrow
        let field_idx = self.cursor_position.0;
        let cursor_pos = self.cursor_position.1;

        if let Some(state) = self.current_query_state_mut() {
            match field_idx {
                0 => {
                    state.where_clause.insert(cursor_pos, c);
                    self.cursor_position.1 += 1;
                }
                1 => {
                    state.order_by_clause.insert(cursor_pos, c);
                    self.cursor_position.1 += 1;
                }
                _ => {}
            }
        }
    }

    /// Delete character before cursor in current query field
    pub fn delete_char(&mut self) {
        // Get cursor positions before mutable borrow
        let field_idx = self.cursor_position.0;
        let cursor_pos = self.cursor_position.1;

        if cursor_pos > 0 {
            if let Some(state) = self.current_query_state_mut() {
                match field_idx {
                    0 => {
                        state.where_clause.remove(cursor_pos - 1);
                        self.cursor_position.1 -= 1;
                    }
                    1 => {
                        state.order_by_clause.remove(cursor_pos - 1);
                        self.cursor_position.1 -= 1;
                    }
                    _ => {}
                }
            }
        }
    }

    /// Delete character at cursor position in current query field
    pub fn delete_char_at_cursor(&mut self) {
        let field_idx = self.cursor_position.0;
        let cursor_pos = self.cursor_position.1;

        if let Some(state) = self.current_query_state_mut() {
            match field_idx {
                0 => {
                    if cursor_pos < state.where_clause.len() {
                        state.where_clause.remove(cursor_pos);
                    }
                }
                1 => {
                    if cursor_pos < state.order_by_clause.len() {
                        state.order_by_clause.remove(cursor_pos);
                    }
                }
                _ => {}
            }
        }
    }

    /// Replace character at cursor position in current query field
    pub fn replace_char_at_cursor(&mut self, c: char) {
        let field_idx = self.cursor_position.0;
        let cursor_pos = self.cursor_position.1;

        if let Some(state) = self.current_query_state_mut() {
            match field_idx {
                0 => {
                    if cursor_pos < state.where_clause.len() {
                        state.where_clause.remove(cursor_pos);
                        state.where_clause.insert(cursor_pos, c);
                    }
                }
                1 => {
                    if cursor_pos < state.order_by_clause.len() {
                        state.order_by_clause.remove(cursor_pos);
                        state.order_by_clause.insert(cursor_pos, c);
                    }
                }
                _ => {}
            }
        }
    }

    /// Clear the current query field (acts like deleting the entire line)
    pub fn clear_current_field(&mut self) {
        let field_idx = self.cursor_position.0;
        if let Some(state) = self.current_query_state_mut() {
            match field_idx {
                0 => state.where_clause.clear(),
                1 => state.order_by_clause.clear(),
                _ => {}
            }
            self.cursor_position.1 = 0;
        }
    }

    pub async fn sort_results(&mut self) -> Result<()> {
        // Get current result and state
        let (current_result, query_state) = match self
            .selected_result_tab_index
            .and_then(|idx| self.result_tabs.get_mut(idx))
            .map(|(_, result, state)| (result, state))
        {
            Some((result, state)) if !result.columns.is_empty() => (result, state),
            _ => return Ok(()),
        };

        // Use cursor position to determine which column to sort
        let col_idx = self.cursor_position.0;
        let current_col = current_result.columns.get(col_idx).cloned();

        if let Some(current_col) = current_col {
            // Update sort state
            if query_state.sort_column.as_ref() != Some(&current_col) {
                query_state.sort_column = Some(current_col.clone());
                query_state.sort_order = Some(false); // default: descending
                query_state.order_by_clause = format!("{} DESC", current_col);
            } else {
                if let Some(order) = query_state.sort_order {
                    if !order {
                        query_state.sort_order = Some(true);
                        query_state.order_by_clause = format!("{} ASC", current_col);
                    } else {
                        query_state.sort_column = None;
                        query_state.sort_order = None;
                        query_state.order_by_clause.clear();
                    }
                }
            }

            // Refresh the results with new sort
            self.refresh_results().await?;
        }

        Ok(())
    }

    /// Refreshes the results for the current tab
    pub async fn refresh_results(&mut self) -> Result<()> {
        if let Some((name, schema, table)) = &self.last_table_info {
            if let Some(connection) = self.connection_manager.get_connection(name) {
                let query_state = self
                    .current_query_state()
                    .ok_or_else(|| anyhow::anyhow!("No active query state"))?;

                let params = QueryParams {
                    where_clause: Some(query_state.where_clause.clone()),
                    order_by: Some(query_state.order_by_clause.clone()),
                    limit: Some(query_state.page_size),
                    offset: Some((query_state.current_page - 1) * query_state.page_size),
                };

                let result = connection.fetch_table_data(schema, table, &params).await?;

                // Update totals
                let total_records = match connection
                    .count_table_rows(schema, table, params.where_clause.as_deref())
                    .await
                {
                    Ok(count) => count,
                    Err(_) => {
                        // Fallback: infer at least the number of currently visible rows
                        let offset = params.offset.unwrap_or(0) as u64;
                        let visible = result.rows.len() as u64;
                        (offset + visible).max(visible)
                    }
                };
                if let Some(state) = self.current_query_state_mut() {
                    state.total_records = Some(total_records);
                    let page_size = state.page_size.max(1);
                    let pages = ((total_records + page_size as u64 - 1) / page_size as u64) as u32;
                    state.total_pages = Some(pages.max(1));
                }

                if let Some(idx) = self.selected_result_tab_index {
                    if idx < self.result_tabs.len() {
                        self.result_tabs[idx].1 = result;
                    }
                }
            }
        }
        Ok(())
    }

    /// Toggles (expand/collapse) a tree item based on its visual index.
    pub async fn toggle_tree_item(&mut self, visual_index: usize) -> Result<()> {
        if let Some(tree_item) = self.get_tree_item_at_visual_index(visual_index) {
            logging::debug(&format!(
                "Toggling tree item at visual index {}",
                visual_index
            ))
            .unwrap_or_else(|e| eprintln!("Logging error: {}", e));
            match tree_item {
                TreeItem::Connection(conn_idx) => {
                    self.expand_connection(conn_idx).await?;
                }
                TreeItem::Database(conn_idx, db_idx) => {
                    self.expand_database(conn_idx, db_idx).await?;
                }
                TreeItem::Schema(conn_idx, db_idx, schema_idx) => {
                    self.expand_schema(conn_idx, db_idx, schema_idx).await?;
                }
                TreeItem::Table(conn_idx, db_idx, schema_idx, table_idx) => {
                    if let Some(connection) = self.connection_tree.get(conn_idx) {
                        if let Some(database) = connection.databases.get(db_idx) {
                            if let Some(schema) = database.schemas.get(schema_idx) {
                                if let Some(table) = schema.tables.get(table_idx) {
                                    if let Some(db_connection) = self
                                        .connection_manager
                                        .get_connection(&connection.connection_config.name)
                                    {
                                        let params = QueryParams {
                                            where_clause: None,
                                            order_by: None,
                                            limit: Some(50), // Default page size
                                            offset: None,
                                        };
                                        logging::debug(&format!(
                                            "Fetching table data for schema {}, table {}",
                                            schema.name, table
                                        ))
                                        .unwrap_or_else(|e| eprintln!("Logging error: {}", e));

                                        match db_connection
                                            .fetch_table_data(&schema.name, table, &params)
                                            .await
                                        {
                                            Ok(result) => {
                                                let tab_name = format!("{}.{}", schema.name, table);
                                                let tab_index = self
                                                    .result_tabs
                                                    .iter()
                                                    .position(|(name, _, _)| name == &tab_name);

                                                // Initialize new query state
                                                let query_state = QueryState {
                                                    page_size: 50, // Default page size
                                                    current_page: 1,
                                                    total_pages: Some(1),
                                                    total_records: Some(0),
                                                    sort_column: None,
                                                    sort_order: None,
                                                    rows_marked_for_deletion: HashSet::new(),
                                                    where_clause: String::new(),
                                                    order_by_clause: String::new(),
                                                };

                                                // Compute totals immediately
                                                let total_records = match db_connection
                                                    .count_table_rows(&schema.name, table, None)
                                                    .await
                                                {
                                                    Ok(count) => count,
                                                    Err(_) => result.rows.len() as u64,
                                                };
                                                let page_size = query_state.page_size.max(1);
                                                let total_pages =
                                                    ((total_records + page_size as u64 - 1)
                                                        / page_size as u64)
                                                        .max(1)
                                                        as u32;

                                                if let Some(index) = tab_index {
                                                    self.selected_result_tab_index = Some(index);
                                                    if let Some((
                                                        _,
                                                        ref mut result_slot,
                                                        ref mut state,
                                                    )) = self.result_tabs.get_mut(index)
                                                    {
                                                        *result_slot = result;
                                                        state.total_records = Some(total_records);
                                                        state.total_pages = Some(total_pages);
                                                        state.current_page = 1;
                                                    }
                                                } else {
                                                    // Create new tab with new query state
                                                    let mut new_state = query_state;
                                                    new_state.total_records = Some(total_records);
                                                    new_state.total_pages = Some(total_pages);
                                                    self.result_tabs
                                                        .push((tab_name, result, new_state));
                                                    self.selected_result_tab_index =
                                                        Some(self.result_tabs.len() - 1);
                                                    // Reset results cursor to top-left on newly opened table
                                                    self.cursor_position = (0, 0);
                                                    self.active_pane = Pane::Results;
                                                }

                                                self.last_table_info = Some((
                                                    connection.connection_config.name.clone(),
                                                    schema.name.clone(),
                                                    table.clone(),
                                                ));

                                                logging::info(&format!(
                                                    "Successfully fetched data from table {}",
                                                    table
                                                ))?;
                                            }
                                            Err(e) => {
                                                let error_msg =
                                                    format!("Failed to fetch table data: {}", e);
                                                logging::error(&error_msg)?;
                                                return Err(anyhow::anyhow!(error_msg));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Moves the selection up in the connections tree.
    pub fn move_selection_up(&mut self) {
        if let Some(current_idx) = self.selected_connection_idx {
            if current_idx > 0 {
                self.selected_connection_idx = Some(current_idx - 1);
            }
        } else {
            // If nothing is selected, select the last item
            let total_items = self.get_total_visible_items();
            if total_items > 0 {
                self.selected_connection_idx = Some(total_items - 1);
            }
        }
    }

    /// Moves the selection down in the connections tree.
    pub fn move_selection_down(&mut self) {
        let total_items = self.get_total_visible_items();

        if let Some(current_idx) = self.selected_connection_idx {
            logging::debug(&format!(
                "move_selection_down: current_idx = {}",
                current_idx
            ))
            .unwrap(); // ADD THIS LINE
            if current_idx + 1 < total_items {
                self.selected_connection_idx = Some(current_idx + 1);
            }
        } else if total_items > 0 {
            // If nothing is selected, select the first item
            self.selected_connection_idx = Some(0);
        }
        logging::debug(&format!(
            "move_selection_down: selected_connection_idx = {:?}",
            self.selected_connection_idx
        ))
        .unwrap();
    }

    pub fn copy_cell(&mut self) -> anyhow::Result<()> {
        if let Some(selected_tab_index) = self.selected_result_tab_index {
            if let Some((_, result, _)) = self.result_tabs.get(selected_tab_index) {
                if let Some(row) = result.rows.get(self.cursor_position.1) {
                    if let Some(cell) = row.get(self.cursor_position.0) {
                        // Store in internal clipboard
                        self.clipboard = cell.clone();

                        // Also copy to system clipboard
                        let mut ctx: ClipboardContext = match ClipboardProvider::new() {
                            Ok(ctx) => ctx,
                            Err(e) => {
                                let error_msg = format!("Failed to access clipboard: {}", e);
                                logging::error(&error_msg)?;
                                self.status_message = Some(error_msg);
                                return Ok(());
                            }
                        };

                        if let Err(e) = ctx.set_contents(cell.clone()) {
                            let error_msg = format!("Failed to copy to clipboard: {}", e);
                            logging::error(&error_msg)?;
                            self.status_message = Some(error_msg);
                            return Ok(());
                        }

                        self.status_message = Some("Cell copied to clipboard".to_string());
                        logging::info(&format!("Copied cell content to clipboard: {}", cell))?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn copy_row(&mut self) -> anyhow::Result<()> {
        if let Some(selected_tab_index) = self.selected_result_tab_index {
            if let Some((_, result, _)) = self.result_tabs.get(selected_tab_index) {
                if let Some(row) = result.rows.get(self.cursor_position.1) {
                    // Join row cells with tabs for easy pasting into spreadsheets
                    let row_content = row.join("\t");

                    // Store in internal clipboard
                    self.clipboard = row_content.clone();

                    // Also copy to system clipboard
                    let mut ctx: ClipboardContext = match ClipboardProvider::new() {
                        Ok(ctx) => ctx,
                        Err(e) => {
                            let error_msg = format!("Failed to access clipboard: {}", e);
                            logging::error(&error_msg)?;
                            self.status_message = Some(error_msg);
                            return Ok(());
                        }
                    };

                    if let Err(e) = ctx.set_contents(row_content.clone()) {
                        let error_msg = format!("Failed to copy to clipboard: {}", e);
                        logging::error(&error_msg)?;
                        self.status_message = Some(error_msg);
                        return Ok(());
                    }

                    self.status_message = Some("Row copied to clipboard".to_string());
                    logging::info(&format!("Copied row to clipboard: {} cells", row.len()))?;
                }
            }
        }
        Ok(())
    }

    /// Selects the next result tab.
    pub fn select_next_tab(&mut self) {
        if self.result_tabs.is_empty() {
            return;
        }
        self.selected_result_tab_index = Some(match self.selected_result_tab_index {
            Some(idx) => (idx + 1) % self.result_tabs.len(),
            None => 0, // Select first tab if none selected
        });
    }

    /// Selects the previous result tab.
    pub fn select_previous_tab(&mut self) {
        if self.result_tabs.is_empty() {
            return;
        }
        self.selected_result_tab_index = Some(match self.selected_result_tab_index {
            Some(idx) => {
                if idx > 0 {
                    idx - 1
                } else {
                    self.result_tabs.len() - 1
                }
            } // Wrap around
            None => 0, // Select first tab if none selected
        });
    }

    /// Checks if a given visual index is the currently selected item in the connections tree.
    pub fn highlight_selected_item(&self, visual_index: usize) -> bool {
        self.selected_connection_idx == Some(visual_index)
    }

    /// Move to the next page of results
    pub async fn next_page(&mut self) -> Result<()> {
        if let Some(state) = self.current_query_state_mut() {
            if let Some(total_pages) = state.total_pages {
                if state.current_page < total_pages {
                    state.current_page += 1;
                    self.refresh_results().await?;
                }
            }
        }
        Ok(())
    }

    /// Move to the previous page of results
    pub async fn previous_page(&mut self) -> Result<()> {
        if let Some(state) = self.current_query_state_mut() {
            if state.current_page > 1 {
                state.current_page -= 1;
                self.refresh_results().await?;
            }
        }
        Ok(())
    }

    /// Move to the first page of results
    pub async fn first_page(&mut self) -> Result<()> {
        if let Some(state) = self.current_query_state_mut() {
            if state.current_page != 1 {
                state.current_page = 1;
                self.refresh_results().await?;
            }
        }
        Ok(())
    }

    /// Move to the last page of results
    pub async fn last_page(&mut self) -> Result<()> {
        if let Some(state) = self.current_query_state_mut() {
            if let Some(total_pages) = state.total_pages {
                if state.current_page != total_pages {
                    state.current_page = total_pages;
                    self.refresh_results().await?;
                }
            }
        }
        Ok(())
    }

    /// Update the page size
    pub async fn set_page_size(&mut self, size: u32) -> Result<()> {
        if let Some(state) = self.current_query_state_mut() {
            if state.page_size != size {
                state.page_size = size;
                state.current_page = 1; // Reset to first page when changing page size
                self.refresh_results().await?;
            }
        }
        Ok(())
    }

    pub fn toggle_row_deletion_mark(&mut self) {
        if let Some(result_tab_index) = self.selected_result_tab_index {
            if let Some((_, _, state)) = self.result_tabs.get_mut(result_tab_index) {
                let row_index = self.cursor_position.1;
                if state.rows_marked_for_deletion.contains(&row_index) {
                    state.rows_marked_for_deletion.remove(&row_index);
                } else {
                    state.rows_marked_for_deletion.insert(row_index);
                }
            }
        }
    }

    pub fn get_deletion_preview(&self) -> Option<Vec<Vec<String>>> {
        self.selected_result_tab_index.and_then(|idx| {
            self.result_tabs.get(idx).map(|(_, result, state)| {
                state
                    .rows_marked_for_deletion
                    .iter()
                    .filter_map(|&row_idx| result.rows.get(row_idx))
                    .cloned()
                    .collect()
            })
        })
    }

    pub async fn confirm_deletions(&mut self) -> Result<()> {
        // First gather all the data we need
        let delete_info = {
            if let Some((conn_name, schema, table)) = &self.last_table_info {
                if let Some((_, result, state)) = self
                    .selected_result_tab_index
                    .and_then(|idx| self.result_tabs.get(idx))
                {
                    let pk_column = &result.columns[0];
                    let pk_values: Vec<String> = state
                        .rows_marked_for_deletion
                        .iter()
                        .filter_map(|&row_idx| result.rows.get(row_idx))
                        .filter_map(|row| row.first())
                        .cloned()
                        .collect();

                    // Return the info we need for deletion
                    Some((
                        conn_name.clone(),
                        schema.clone(),
                        table.clone(),
                        pk_column.clone(),
                        pk_values,
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        };

        // Now perform the deletion if we have the info
        if let Some((conn_name, schema, table, pk_column, pk_values)) = delete_info {
            if !pk_values.is_empty() {
                if let Some(db_connection) = self.connections.get(&conn_name) {
                    let query = format!(
                        "DELETE FROM {}.{} WHERE {} IN ({})",
                        schema,
                        table,
                        pk_column,
                        pk_values.join(", ")
                    );

                    match db_connection.execute_query(&query).await {
                        Ok(_) => {
                            // Clear deletion marks
                            if let Some((_, _, state)) = self
                                .selected_result_tab_index
                                .and_then(|idx| self.result_tabs.get_mut(idx))
                            {
                                state.rows_marked_for_deletion.clear();
                            }

                            // Refresh results
                            self.refresh_results().await?;

                            self.status_message =
                                Some(format!("Successfully deleted {} rows", pk_values.len()));
                            logging::info(&format!(
                                "Successfully deleted {} rows from {}.{}",
                                pk_values.len(),
                                schema,
                                table
                            ))?;

                            Ok(())
                        }
                        Err(e) => {
                            let error_msg = format!("Failed to delete rows: {}", e);
                            self.status_message = Some(error_msg.clone());
                            logging::error(&error_msg)?;
                            Err(anyhow::anyhow!(error_msg))
                        }
                    }
                } else {
                    let error_msg = "Database connection not found".to_string();
                    self.status_message = Some(error_msg.clone());
                    Err(anyhow::anyhow!(error_msg))
                }
            } else {
                Ok(())
            }
        } else {
            Ok(())
        }
    }

    pub fn clear_deletion_marks(&mut self) {
        if let Some((_, _, state)) = self
            .selected_result_tab_index
            .and_then(|idx| self.result_tabs.get_mut(idx))
        {
            state.rows_marked_for_deletion.clear();
        }
    }
}
