//! `app.rs` - Defines the main application logic and data structures.

use crate::logging;
use crate::input::{NavigationAction,TreeAction};
use crate::ui::types::{Direction, Pane};

use std::collections::HashMap;
use anyhow::Result;
use crate::config::Config;
use crate::database::{
    DatabaseConnection, DatabaseType, ConnectionConfig,
    QueryResult, PostgresConnection, MongoConnection
};

/// Represents the form data for creating or editing a database connection.
#[derive(Default, Clone)]
pub struct ConnectionForm {
    /// The name of the connection.
    pub name: String,
    /// The type of database (Postgres, MongoDB, etc.).
    pub db_type: DatabaseType,
    /// The host address of the database server.
    pub host: String,
    /// The port number of the database server.
    pub port: String,
    /// The username for database authentication.
    pub username: String,
    /// The password for database authentication.
    pub password: String,
    /// The name of the database to connect to.
    pub database: String,
    /// Whether SSH tunneling is enabled for this connection.
    pub ssh_enabled: bool,
    /// The host address of the SSH server.
    pub ssh_host: String,
    /// The port number of the SSH server.
    pub ssh_port: String,
    /// The username for SSH authentication.
    pub ssh_username: String,
    /// The password for SSH authentication.
    pub ssh_password: String,
    /// The path to the SSH private key file.
    pub ssh_key_path: String,
    /// The index of the currently focused field in the form.
    pub current_field: usize,
}

/// Represents the query state for a single tab/table
#[derive(Clone, Default)]
pub struct QueryState {
    /// The WHERE clause of the query
    pub where_clause: String,
    /// The ORDER BY clause of the query
    pub order_by_clause: String,
    /// Number of items per page
    pub page_size: u32,
    /// Current page number
    pub current_page: u32,
    /// Total number of pages
    pub total_pages: Option<u32>,
    /// Total number of records
    pub total_records: Option<u64>,
    /// Current sort column
    pub sort_column: Option<String>,
    /// Current sort order (true for ascending)
    pub sort_order: Option<bool>,
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

/// Represents the connection status of a database connection.
#[derive(PartialEq, Debug, Clone, Copy)]
pub enum ConnectionStatus {
    /// Not connected.
    NotConnected,
    /// Currently connecting.
    Connecting,
    /// Successfully connected.
    Connected,
    /// Connection failed.
    Failed,
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
#[derive(Clone)]
pub struct App {
    /// Indicates if the application should quit.
    pub should_quit: bool,
    /// The currently active block in the UI.
    pub active_block: ActiveBlock,
    /// Indicates if the connection modal is shown.
    pub show_connection_modal: bool,
    /// List of saved database connections.
    pub saved_connections: Vec<ConnectionConfig>,
    /// Index of the currently selected connection in the connections list.
    pub selected_connection_idx: Option<usize>,
    /// Status of each saved connection, mapped by connection name.
    pub connection_statuses: HashMap<String, ConnectionStatus>,
    /// Form data for creating new connections.
    pub connection_form: ConnectionForm,
    /// Current input mode of the application.
    pub input_mode: InputMode,
    /// Text content of the free-form query input.
    pub free_query: String,
    /// List of result tabs, each containing a tab name and query result.
    pub result_tabs: Vec<(String, QueryResult, QueryState)>,
    /// Application configuration.
    pub config: Config,
    /// Status message to display in the status bar.
    pub status_message: Option<String>,
    /// Text in the command input bar.
    pub command_input: String,
    /// Map of established database connections, keyed by connection name.
    pub connections: HashMap<String, Box<dyn DatabaseConnection>>,
    /// Cursor position in the current input area (row, column).
    pub cursor_position: (usize, usize),
    /// Currently focused pane in the UI.
    pub active_pane: Pane,
    /// Tree structure representing connections, databases, schemas, and tables.
    pub connection_tree: Vec<ConnectionTreeItem>,
    /// Column to sort results by.
    pub sort_column: Option<String>,
    /// Sort order (ascending/descending).
    pub sort_order: Option<bool>,
    /// Information about the last fetched table (connection name, schema, table).
    pub last_table_info: Option<(String, String, String)>,
    /// Index of the currently selected result tab.
    pub selected_result_tab_index: Option<usize>,
}

impl App {
    /// Constructs a new `App` instance with default settings and loads configurations.
    pub fn new() -> Self {
        let config = Config::new();
        let mut app = Self {
            should_quit: false,
            active_block: ActiveBlock::Connections,
            show_connection_modal: false,
            saved_connections: config.connections.clone(),
            selected_connection_idx: None,
            connection_statuses: config.connections.iter().map(|c| (c.name.clone(), ConnectionStatus::NotConnected)).collect(),
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
            sort_column: None,
            sort_order: None,
            last_table_info: None,
            selected_result_tab_index: None,
        };

        app.load_connections();

        app.connection_tree = app.config.connections.iter().map(|conn| {
            ConnectionTreeItem {
                connection_config: conn.clone(),
                status: ConnectionStatus::NotConnected,
                databases: Vec::new(),
                is_expanded: false,
            }
        }).collect();

        app
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
        };

        self.saved_connections.push(new_connection.clone());
        self.config.save_connections(&self.saved_connections).expect("Failed to save connections");

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

        self.connection_statuses = self.saved_connections.iter()
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
                    },
                    Direction::Right => {
                        if self.cursor_position.0 < result.columns.len().saturating_sub(1) {
                            self.cursor_position.0 += 1;
                        }
                    },
                    Direction::Up => {
                        if self.cursor_position.1 > 0 {
                            self.cursor_position.1 -= 1;
                        }
                    },
                    Direction::Down => {
                        if self.cursor_position.1 < result.rows.len().saturating_sub(1) {
                            self.cursor_position.1 += 1;
                        }
                    }
                }
            }
        }
    }

    pub fn get_current_cell_content(&self) -> Option<&str> {
        self.selected_result_tab_index.and_then(|tab_index| {
            self.result_tabs.get(tab_index).and_then(|(_, result, _)| {
                result.rows.get(self.cursor_position.1)
                    .and_then(|row| row.get(self.cursor_position.0))
                    .map(String::as_str)
            })
        })
    }
    
    /// Moves the cursor one position to the left in the query input.
    pub fn move_cursor_left(&mut self) {
        if self.active_pane == Pane::QueryInput {
            self.cursor_position.1 = self.cursor_position.1.saturating_sub(1);
        }
    }

    /// Moves the cursor one position to the right in the query input.
    pub fn move_cursor_right(&mut self) {
        if self.active_pane == Pane::QueryInput {
            self.cursor_position.1 += 1;
        }
    }

    /// Moves the cursor one position up in the query input.
    pub fn move_cursor_up(&mut self) {
        if self.active_pane == Pane::QueryInput {
            self.cursor_position.0 = self.cursor_position.0.saturating_sub(1);
        }
    }

    /// Moves the cursor one position down in the query input.
    pub fn move_cursor_down(&mut self) {
        if self.active_pane == Pane::QueryInput {
            self.cursor_position.0 += 1;
        }
    }

    /// Selects a connection at the given index in the connections tree.
    pub fn select_connection(&mut self, index: usize) {
        if index < self.connection_tree.len() {
            self.selected_connection_idx = Some(index);
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
                            },
                            Direction::Down => {
                                self.cursor_position.0 = 1; // ORDER BY clause
                                let len = self.get_current_field_length();
                                self.cursor_position.1 = self.cursor_position.1.min(len);
                            },
                            Direction::Left => {
                                if self.cursor_position.1 > 0 {
                                    self.cursor_position.1 -= 1;
                                }
                            },
                            Direction::Right => {
                                let max_pos = self.get_current_field_length();
                                if self.cursor_position.1 < max_pos {
                                    self.cursor_position.1 += 1;
                                }
                            },
                        }
                    },
                    _ => {} // Handle other panes as before
                }
            },
            _ => {} // Handle other actions as before
        }
    }


    /// Handles tree-related actions such as expand and collapse.
    pub async fn handle_tree_action(&mut self, action: TreeAction) -> Result<()> {
        if self.active_pane == Pane::Connections {
            if let Some(idx) = self.selected_connection_idx {
                match action {
                    TreeAction::Expand => {
                        self.toggle_tree_item(idx).await?;
                    },
                    TreeAction::Collapse => {
                        // Just collapse without making any async calls
                        if let Some(tree_item) = self.get_tree_item_at_visual_index(idx) {
                            match tree_item {
                                TreeItem::Connection(conn_idx) => {
                                    if let Some(connection) = self.connection_tree.get_mut(conn_idx) {
                                        connection.is_expanded = false;
                                    }
                                },
                                TreeItem::Database(conn_idx, db_idx) => {
                                    if let Some(connection) = self.connection_tree.get_mut(conn_idx) {
                                        if let Some(database) = connection.databases.get_mut(db_idx) {
                                            database.is_expanded = false;
                                        }
                                    }
                                },
                                TreeItem::Schema(conn_idx, db_idx, schema_idx) => {
                                    if let Some(connection) = self.connection_tree.get_mut(conn_idx) {
                                        if let Some(database) = connection.databases.get_mut(db_idx) {
                                            if let Some(schema) = database.schemas.get_mut(schema_idx) {
                                                schema.is_expanded = false;
                                            }
                                        }
                                    }
                                },
                                TreeItem::Table(_, _, _, _) => {}, // Tables don't expand/collapse
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
    logging::debug(&format!("Attempting to expand connection at index {}", index))?;

    if let Some(connection) = self.connection_tree.get_mut(index) {
        if !connection.is_expanded {
            // Create the appropriate database connection
            let mut db_connection: Box<dyn DatabaseConnection> = match connection.connection_config.db_type {
                DatabaseType::Postgres => Box::new(PostgresConnection::new(connection.connection_config.clone())),
                DatabaseType::MongoDB => Box::new(MongoConnection::new(connection.connection_config.clone())),
            };

            connection.status = ConnectionStatus::Connecting;
            logging::info(&format!("Connecting to database: {}", connection.connection_config.name))?;

            match db_connection.connect().await {
                Ok(_) => {
                    logging::info(&format!("Successfully connected to {}", connection.connection_config.name))?;

                    match db_connection.list_databases().await {
                        Ok(databases) => {
                            logging::debug(&format!("Found {} databases: {:?}", databases.len(), databases))?;

                            connection.databases = databases.into_iter()
                                .map(|db_name| DatabaseTreeItem {
                                    name: db_name,
                                    schemas: Vec::new(),
                                    is_expanded: false,
                                })
                                .collect();

                            // Store the connection for future use
                            self.connections.insert(connection.connection_config.name.clone(), db_connection);
                            connection.status = ConnectionStatus::Connected;
                            connection.is_expanded = true;

                            logging::info(&format!("Successfully expanded connection {}", connection.connection_config.name))?;
                        }
                        Err(e) => {
                            connection.status = ConnectionStatus::Failed;
                            logging::error(&format!("Failed to list databases: {}", e))?;
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    connection.status = ConnectionStatus::Failed;
                    logging::error(&format!("Failed to connect: {}", e))?;
                    return Err(e);
                }
            }
        } else {
            // If already expanded, just toggle the visibility
            connection.is_expanded = false;
            logging::debug(&format!("Collapsed connection {}", connection.connection_config.name))?;
        }
    }
    Ok(())
}

    /// Expands a database in the tree to show schemas.
    pub async fn expand_database(&mut self, conn_idx: usize, db_idx: usize) -> Result<()> {
        logging::debug(&format!("Attempting to expand database at connection {}, database {}", conn_idx, db_idx))?;

        if let Some(connection) = self.connection_tree.get_mut(conn_idx) {
            if let Some(database) = connection.databases.get_mut(db_idx) {
                if !database.is_expanded {
                    // Get the connection from the stored connections
                    if let Some(db_connection) = self.connections.get(&connection.connection_config.name) {
                        match db_connection.list_schemas(&database.name).await {
                            Ok(schemas) => {
                                logging::debug(&format!("Found {} schemas in database {}", schemas.len(), database.name))?;

                                database.schemas = schemas.into_iter()
                                    .map(|schema_name| SchemaTreeItem {
                                        name: schema_name,
                                        tables: Vec::new(),
                                        is_expanded: false,
                                    })
                                    .collect();
                                database.is_expanded = true;

                                logging::info(&format!("Successfully expanded database {}", database.name))?;
                            }
                            Err(e) => {
                                logging::error(&format!("Failed to list schemas: {}", e))?;
                                return Err(e);
                            }
                        }
                    } else {
                        let err = anyhow::anyhow!("Connection not found");
                        logging::error("Connection not found in stored connections")?;
                        return Err(err);
                    }
                } else {
                    database.is_expanded = false;
                    logging::debug(&format!("Collapsed database {}", database.name))?;
                }
            }
        }
        Ok(())
    }

    /// Expands a schema in the tree to show tables.
    pub async fn expand_schema(&mut self, conn_idx: usize, db_idx: usize, schema_idx: usize) -> Result<()> {
        logging::debug(&format!("Attempting to expand schema at connection {}, database {}, schema {}",
            conn_idx, db_idx, schema_idx))?;

        if let Some(connection) = self.connection_tree.get_mut(conn_idx) {
            if let Some(database) = connection.databases.get_mut(db_idx) {
                if let Some(schema) = database.schemas.get_mut(schema_idx) {
                    if !schema.is_expanded {
                        if let Some(db_connection) = self.connections.get(&connection.connection_config.name) {
                            match db_connection.list_tables(&schema.name).await {
                                Ok(tables) => {
                                    logging::debug(&format!("Found {} tables in schema {}", tables.len(), schema.name))?;

                                    schema.tables = tables;
                                    schema.is_expanded = true;

                                    logging::info(&format!("Successfully expanded schema {}", schema.name))?;
                                }
                                Err(e) => {
                                    logging::error(&format!("Failed to list tables: {}", e))?;
                                    return Err(e);
                                }
                            }
                        } else {
                            let err = anyhow::anyhow!("Connection not found");
                            logging::error("Connection not found in stored connections")?;
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
                                        return Some(TreeItem::Table(conn_idx, db_idx, schema_idx, table_idx));
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
                                    if *tree_item == TreeItem::Table(conn_idx, db_idx, schema_idx, table_idx) {
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
                },
                1 => {
                    state.order_by_clause.insert(cursor_pos, c);
                    self.cursor_position.1 += 1;
                },
                _ => {},
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
                    },
                    1 => {
                        state.order_by_clause.remove(cursor_pos - 1);
                        self.cursor_position.1 -= 1;
                    },
                    _ => {},
                }
            }
        }
    }

    pub async fn sort_results(&mut self) -> Result<()> {
        // Get current result and state
        let (current_result, query_state) = match self.selected_result_tab_index
            .and_then(|idx| self.result_tabs.get_mut(idx))
            .map(|(_, result, state)| (result, state)) {
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
        // Clone everything we need upfront
        let table_info = self.last_table_info.clone();
        let conn_name = match &table_info {
            Some((name, _, _)) => name.clone(),
            None => return Ok(()),
        };

        // Get query parameters before any database operations
        let (where_clause, order_by_clause, page_size, current_page) = {
            if let Some(state) = self.current_query_state() {
                (
                    state.where_clause.clone(),
                    state.order_by_clause.clone(),
                    state.page_size,
                    state.current_page,
                )
            } else {
                return Ok(());
            }
        };

        let db_connection = match self.connections.get(&conn_name) {
            Some(conn) => conn,
            None => return Ok(()),
        };

        if let Some((_, schema, table)) = table_info {
            let offset = (current_page - 1) * page_size;
            
            // Build queries
            let mut query = format!("SELECT * FROM {}.{}", schema, table);
            if !where_clause.is_empty() {
                query.push_str(&format!(" WHERE {}", where_clause));
            }
            if !order_by_clause.is_empty() {
                query.push_str(&format!(" ORDER BY {}", order_by_clause));
            }
            query.push_str(&format!(" LIMIT {} OFFSET {}", page_size, offset));
            
            let count_query = format!(
                "SELECT COUNT(*) FROM {}.{} {}",
                schema,
                table,
                if !where_clause.is_empty() {
                    format!("WHERE {}", where_clause)
                } else {
                    String::new()
                }
            );
            
            // Execute queries
            let count_result = db_connection.execute_query(&count_query).await?;
            let main_result = db_connection.execute_query(&query).await?;
            
            // Extract count
            let total_count = count_result.rows.first()
                .and_then(|row| row.first())
                .and_then(|count_str| count_str.parse::<u64>().ok())
                .unwrap_or(0);
            
            // Now update all the state at once
            if let Some(state) = self.current_query_state_mut() {
                state.total_records = Some(total_count);
                state.total_pages = Some(
                    ((total_count as f64 / page_size as f64).ceil() as u32).max(1)
                );
            }
            
            if let Some(idx) = self.selected_result_tab_index {
                if idx < self.result_tabs.len() {
                    self.result_tabs[idx].1 = main_result;
                }
            }
            
            logging::info(&format!("Successfully refreshed data from table {}", table))?;
        }
        
        Ok(())
    }

   /// Toggles (expand/collapse) a tree item based on its visual index.
   pub async fn toggle_tree_item(&mut self, visual_index: usize) -> Result<()> {
        if let Some(tree_item) = self.get_tree_item_at_visual_index(visual_index) {
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
                                    if let Some(db_connection) = self.connections.get(&connection.connection_config.name) {
                                        match db_connection.fetch_table_data(&schema.name, table, None).await {
                                            Ok(result) => {
                                                let tab_name = format!("{}.{}", schema.name, table);
                                                let tab_index = self.result_tabs.iter().position(|(name, _, _)| name == &tab_name);
                                                
                                                // Initialize new query state
                                                let query_state = QueryState {
                                                    page_size: 50, // Default page size
                                                    current_page: 1,
                                                    ..Default::default()
                                                };

                                                if let Some(index) = tab_index {
                                                    self.selected_result_tab_index = Some(index);
                                                    // Update the result but preserve existing query state
                                                    let existing_state = self.result_tabs[index].2.clone();
                                                    self.result_tabs[index] = (tab_name, result, existing_state);
                                                } else {
                                                    // Create new tab with new query state
                                                    self.result_tabs.push((tab_name, result, query_state));
                                                    self.selected_result_tab_index = Some(self.result_tabs.len() - 1);
                                                }

                                                self.last_table_info = Some((
                                                    connection.connection_config.name.clone(),
                                                    schema.name.clone(),
                                                    table.clone(),
                                                ));

                                                // Reset sort state for new tables
                                                if tab_index.is_none() {
                                                    if let Some(idx) = self.selected_result_tab_index {
                                                        self.result_tabs[idx].2.sort_column = None;
                                                        self.result_tabs[idx].2.sort_order = None;
                                                    }
                                                }

                                                logging::info(&format!("Successfully fetched data from table {}", table))?;
                                            }
                                            Err(e) => {
                                                logging::error(&format!("Failed to fetch table data: {}", e))?;
                                                return Err(e);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Ensure the selected index is still within bounds after the change
            let total_visible_items = self.get_total_visible_items();
            if let Some(idx) = &mut self.selected_connection_idx {
                *idx = (*idx).min(total_visible_items.saturating_sub(1));
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
            logging::debug(&format!("move_selection_down: current_idx = {}", current_idx)).unwrap(); // ADD THIS LINE
            if current_idx + 1 < total_items {
                self.selected_connection_idx = Some(current_idx + 1);
            }
        } else if total_items > 0 {
            // If nothing is selected, select the first item
            self.selected_connection_idx = Some(0);
        }
        logging::debug(&format!("move_selection_down: selected_connection_idx = {:?}", self.selected_connection_idx)).unwrap();
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
            Some(idx) => if idx > 0 { idx - 1 } else { self.result_tabs.len() - 1 }, // Wrap around
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

    /// Update page size from command input
    pub async fn handle_page_size_command(&mut self, command: &str) -> Result<()> {
        if let Some(size_str) = command.strip_prefix("page-size ") {
            if let Ok(size) = size_str.trim().parse::<u32>() {
                if size > 0 {
                    self.set_page_size(size).await?;
                }
            }
        }
        Ok(())
    }
}
