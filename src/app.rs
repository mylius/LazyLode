//! `app.rs` - Defines the main application logic and data structures.
use crate::command::CommandBuffer;
use crate::input::{NavigationAction, TreeAction};
use crate::logging;
use crate::navigation::types::Pane;
use crate::navigation::NavigationManager;
use crate::ui::layout::QueryField;
use crate::ui::modal_manager::ModalManager;
use crate::ui::panes::query_input::QueryInputPane;
use crate::ui::types::Direction;
use clipboard::{ClipboardContext, ClipboardProvider};

use crate::config::Config;
use crate::database::core::ForeignKeyTarget;
use crate::database::{
    ConnectionConfig, ConnectionManager, ConnectionStatus, DatabaseType, PrefetchedStructure,
    QueryParams, QueryResult,
};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum PrefetchResult {
    Success(String, PrefetchedStructure),
    Failed(String, String), // connection_name, error_message
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Insert,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveBlock {
    Main,
    Connections,
    ConnectionModal,
}

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
    pub modal_manager: ModalManager,
    pub saved_connections: Vec<ConnectionConfig>,
    pub selected_connection_idx: Option<usize>,
    pub connection_statuses: HashMap<String, ConnectionStatus>,
    pub connection_form: ConnectionForm,
    pub input_mode: InputMode,
    pub result_tabs: Vec<(String, QueryResult, QueryState)>,
    pub config: Config,
    pub status_message: Option<String>,
    pub status_message_timestamp: Option<std::time::Instant>,
    pub command_input: String,
    pub cursor_position: (usize, usize),
    pub active_pane: Pane,
    pub connection_tree: Vec<ConnectionTreeItem>,
    pub last_table_info: Option<(String, String, String)>,
    pub selected_result_tab_index: Option<usize>,
    pub connection_manager: ConnectionManager,
    pub prefetched_structures: HashMap<String, PrefetchedStructure>,
    pub prefetch_receiver: Option<mpsc::UnboundedReceiver<PrefetchResult>>,
    pub command_buffer: CommandBuffer,
    pub clipboard: String,
    pub last_key_was_d: bool,
    pub awaiting_replace: bool,
    /// New navigation system
    pub navigation_manager: NavigationManager,
    pub command_suggestions: Vec<String>,
    pub selected_suggestion: Option<usize>,
    pub suggestions_scroll_offset: usize,
    pub query: String,
    pub query_input_pane: QueryInputPane,
}

impl App {
    /// Constructs a new `App` instance with default settings and loads configurations.
    pub fn new() -> Self {
        let config = Config::new();
        let navigation_config = config.navigation.clone();
        let mut app = Self {
            should_quit: false,
            active_block: ActiveBlock::Connections,
            modal_manager: ModalManager::new(),
            saved_connections: config.connections.clone(),
            selected_connection_idx: None,
            connection_statuses: config
                .connections
                .iter()
                .map(|c| (c.name.clone(), ConnectionStatus::NotConnected))
                .collect(),
            connection_form: ConnectionForm::default(),
            input_mode: InputMode::Normal,
            result_tabs: Vec::new(),
            config,
            status_message: None,
            status_message_timestamp: None,
            command_input: String::new(),
            cursor_position: (0, 0),
            active_pane: Pane::default(),
            connection_tree: Vec::new(),
            last_table_info: None,
            selected_result_tab_index: None,
            connection_manager: ConnectionManager::new(),
            prefetched_structures: HashMap::new(),
            prefetch_receiver: None,
            command_buffer: CommandBuffer::new(),
            clipboard: String::new(),
            last_key_was_d: false,
            awaiting_replace: false,
            navigation_manager: NavigationManager::new(navigation_config),
            command_suggestions: Vec::new(),
            selected_suggestion: None,
            suggestions_scroll_offset: 0,
            query: String::new(),
            query_input_pane: QueryInputPane::new(),
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

    /// Constructs a new `App` instance with async database connection initialization.
    pub async fn new_with_async_connections() -> Result<Self> {
        let config = Config::new();
        let navigation_config = config.navigation.clone();
        let mut app = Self {
            should_quit: false,
            active_block: ActiveBlock::Connections,
            modal_manager: ModalManager::new(),
            saved_connections: config.connections.clone(),
            selected_connection_idx: None,
            connection_statuses: config
                .connections
                .iter()
                .map(|c| (c.name.clone(), ConnectionStatus::NotConnected))
                .collect(),
            connection_form: ConnectionForm::default(),
            input_mode: InputMode::Normal,
            query: String::new(),
            result_tabs: Vec::new(),
            config,
            status_message: None,
            status_message_timestamp: None,
            command_input: String::new(),
            cursor_position: (0, 0),
            active_pane: Pane::default(),
            connection_tree: Vec::new(),
            last_table_info: None,
            selected_result_tab_index: None,
            connection_manager: ConnectionManager::new(),
            prefetched_structures: HashMap::new(),
            prefetch_receiver: None,
            command_buffer: CommandBuffer::new(),
            clipboard: String::new(),
            last_key_was_d: false,
            awaiting_replace: false,
            command_suggestions: Vec::new(),
            selected_suggestion: None,
            suggestions_scroll_offset: 0,
            navigation_manager: NavigationManager::new(navigation_config),
            query_input_pane: QueryInputPane::new(),
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

        // Initialize connections and start background prefetching
        if !app.saved_connections.is_empty() {
            logging::info("Starting background database prefetching...")?;

            // Set all connections to "Connecting" status initially
            for connection in &app.saved_connections {
                app.connection_statuses
                    .insert(connection.name.clone(), ConnectionStatus::Connecting);
                if let Some(tree_item) = app
                    .connection_tree
                    .iter_mut()
                    .find(|item| item.connection_config.name == connection.name)
                {
                    tree_item.status = ConnectionStatus::Connecting;
                }
            }

            // Start background prefetching tasks
            app.start_background_prefetching();
        }

        Ok(app)
    }

    /// Start background connection validation for all connections
    pub fn start_background_prefetching(&mut self) {
        let (tx, rx) = mpsc::unbounded_channel();
        self.prefetch_receiver = Some(rx);

        // Initialize connections as not connected - databases will be loaded after validation
        for connection in &mut self.connection_tree {
            connection.status = ConnectionStatus::NotConnected; // Start as not connected
            connection.is_expanded = false; // Don't expand by default
            connection.databases = Vec::new(); // Will be populated after validation
        }

        // Start background connection validation
        for mut config in self.saved_connections.clone() {
            // Migrate from legacy format
            config.migrate_from_legacy();

            let config_clone = config.clone();
            let connection_name = config.name.clone();
            let tx_clone = tx.clone();

            // Spawn a background task to validate connection and fetch all databases
            tokio::spawn(async move {
                let result =
                    ConnectionManager::fast_prefetch_databases_only(config_clone.clone()).await;

                // Send the result back to the main app
                match result {
                    Ok(mut prefetched_structure) => {
                        // Filter databases based on configuration
                        if !config_clone.databases.is_empty() {
                            prefetched_structure
                                .databases
                                .retain(|db| config_clone.should_show_database(&db.name));
                        }

                        let _ = tx_clone.send(PrefetchResult::Success(
                            connection_name.clone(),
                            prefetched_structure,
                        ));
                        logging::info(&format!(
                            "Successfully loaded databases for: {}",
                            connection_name
                        ))
                        .unwrap_or_else(|e| eprintln!("Logging error: {}", e));
                    }
                    Err(e) => {
                        let _ = tx_clone.send(PrefetchResult::Failed(
                            connection_name.clone(),
                            e.to_string(),
                        ));
                        logging::error(&format!(
                            "Failed to load databases for {}: {}",
                            connection_name, e
                        ))
                        .unwrap_or_else(|e| eprintln!("Logging error: {}", e));
                    }
                }
            });
        }
    }

    /// Check for completed background prefetching results and update the UI
    pub fn check_background_prefetching(&mut self) -> Result<()> {
        if let Some(ref mut receiver) = self.prefetch_receiver {
            while let Ok(result) = receiver.try_recv() {
                match result {
                    PrefetchResult::Success(connection_name, prefetched_structure) => {
                        // Store the prefetched structure
                        self.prefetched_structures
                            .insert(connection_name.clone(), prefetched_structure);

                        // Update connection status
                        self.connection_statuses
                            .insert(connection_name.clone(), ConnectionStatus::Connected);

                        // Update connection tree with all available databases
                        if let Some(tree_item) = self
                            .connection_tree
                            .iter_mut()
                            .find(|item| item.connection_config.name == connection_name)
                        {
                            tree_item.status = ConnectionStatus::Connected;
                            // Don't expand by default - user needs to click to expand
                            tree_item.is_expanded = false;

                            // Populate databases from prefetched data
                            if let Some(prefetched) =
                                self.prefetched_structures.get(&connection_name)
                            {
                                tree_item.databases = prefetched
                                    .databases
                                    .iter()
                                    .map(|db| DatabaseTreeItem {
                                        name: db.name.clone(),
                                        schemas: Vec::new(), // Will be loaded on-demand
                                        is_expanded: false,
                                    })
                                    .collect();
                            }
                        }

                        logging::info(&format!(
                            "Successfully loaded {} databases for: {}",
                            self.prefetched_structures
                                .get(&connection_name)
                                .map(|p| p.databases.len())
                                .unwrap_or(0),
                            connection_name
                        ))?;
                    }
                    PrefetchResult::Failed(connection_name, error_message) => {
                        // Update connection status to failed
                        self.connection_statuses
                            .insert(connection_name.clone(), ConnectionStatus::Failed);

                        // Update connection tree status
                        if let Some(tree_item) = self
                            .connection_tree
                            .iter_mut()
                            .find(|item| item.connection_config.name == connection_name)
                        {
                            tree_item.status = ConnectionStatus::Failed;
                        }

                        logging::error(&format!(
                            "Background prefetching failed for {}: {}",
                            connection_name, error_message
                        ))?;
                    }
                }
            }
        }
        Ok(())
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

            let tab_name = format!("{}:{}.{}", conn_name, schema, table);
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

    /// Toggles the visibility of the themes modal.
    pub fn toggle_themes_modal(&mut self) {
        use crate::ui::modals::ThemesModal;

        // If themes modal is already open, bring it to the front
        if self.modal_manager.focus_modal_with_title("Themes") {
            return;
        }

        // Otherwise, open it (allowing modal stacking)
        let current_theme = self.config.theme_name.clone();
        let themes_modal = Box::new(ThemesModal::new(current_theme));
        self.modal_manager.push(themes_modal);
    }

    /// Toggles the visibility of the connection modal.
    pub fn toggle_connection_modal(&mut self) {
        use crate::ui::modals::ConnectionModal;

        // If connection modal is already open, bring it to the front
        if self.modal_manager.focus_modal_with_title("New Connection") {
            return;
        }

        // Otherwise, open it (allowing modal stacking)
        let connection_modal = Box::new(ConnectionModal::new());
        self.modal_manager.push(connection_modal);
        self.active_block = ActiveBlock::ConnectionModal;
    }

    /// Close the active modal
    pub fn close_active_modal(&mut self) {
        self.modal_manager.close_active();
        self.input_mode = InputMode::Normal;
    }

    /// Show themes modal
    pub fn show_themes_modal(&mut self) {
        use crate::ui::modals::ThemesModal;
        let current_theme = self.config.theme_name.clone();
        let themes_modal = Box::new(ThemesModal::new(current_theme));
        self.modal_manager.push(themes_modal);
    }

    /// Show connection modal
    pub fn show_connection_modal(&mut self) {
        use crate::ui::modals::ConnectionModal;
        let connection_modal = Box::new(ConnectionModal::new());
        self.modal_manager.push(connection_modal);
        self.active_block = ActiveBlock::ConnectionModal;
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
            default_database: Some(self.connection_form.database.clone()),
            databases: std::collections::HashMap::new(),
            ssh_tunnel: None,
            ssh_tunnel_name: self.connection_form.ssh_tunnel_name.clone(),
            database: Some(self.connection_form.database.clone()),
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
                0 => state.where_clause.chars().count(),
                1 => state.order_by_clause.chars().count(),
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
        // Get content before mutable borrow
        let where_content = self
            .current_query_state()
            .map(|state| state.where_clause.clone())
            .unwrap_or_default();

        // Sync navigation manager's vim mode and cursor position
        let vim_editor = self.navigation_manager.box_manager_mut().vim_editor_mut();
        vim_editor.mode = crate::navigation::types::VimMode::Insert;
        vim_editor.set_cursor_position(self.cursor_position);
        vim_editor.set_content(where_content);
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
                                if let Some(connection) = self.connection_tree.get_mut(conn_idx) {
                                    connection.is_expanded = false;
                                }
                            }
                            TreeItem::Database(conn_idx, db_idx) => {
                                if let Some(connection) = self.connection_tree.get_mut(conn_idx) {
                                    if let Some(database) = connection.databases.get_mut(db_idx) {
                                        database.is_expanded = false;
                                    }
                                }
                            }
                            TreeItem::Schema(conn_idx, db_idx, schema_idx) => {
                                if let Some(connection) = self.connection_tree.get_mut(conn_idx) {
                                    if let Some(database) = connection.databases.get_mut(db_idx) {
                                        if let Some(schema) = database.schemas.get_mut(schema_idx) {
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
                // Check if we already have prefetched data
                if let Some(prefetched) = self
                    .prefetched_structures
                    .get(&connection.connection_config.name)
                {
                    // Use existing prefetched data - no need to refetch
                    connection.status = ConnectionStatus::Connected;
                    connection.is_expanded = true;

                    connection.databases = prefetched
                        .databases
                        .iter()
                        .map(|db| DatabaseTreeItem {
                            name: db.name.clone(),
                            schemas: db
                                .schemas
                                .iter()
                                .map(|schema| SchemaTreeItem {
                                    name: schema.name.clone(),
                                    tables: schema.tables.clone(),
                                    is_expanded: false,
                                })
                                .collect(),
                            is_expanded: false,
                        })
                        .collect();

                    logging::info(&format!(
                        "Expanded connection {} using existing prefetched data",
                        connection.connection_config.name
                    ))?;
                    return Ok(());
                }

                // No prefetched data available, need to connect and fetch
                connection.status = ConnectionStatus::Connecting;
                logging::info(&format!(
                    "Connecting to database: {}",
                    connection.connection_config.name
                ))?;

                // Try to connect with prefetching
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

                // Try to prefetch the entire database structure
                match self
                    .connection_manager
                    .prefetch_database_structure(cfg.clone())
                    .await
                {
                    Ok(prefetched_structure) => {
                        // Store the prefetched structure
                        self.prefetched_structures.insert(
                            connection.connection_config.name.clone(),
                            prefetched_structure,
                        );

                        connection.status = ConnectionStatus::Connected;
                        connection.is_expanded = true;

                        // Populate databases with prefetched data
                        if let Some(prefetched) = self
                            .prefetched_structures
                            .get(&connection.connection_config.name)
                        {
                            connection.databases = prefetched
                                .databases
                                .iter()
                                .map(|db| DatabaseTreeItem {
                                    name: db.name.clone(),
                                    schemas: db
                                        .schemas
                                        .iter()
                                        .map(|schema| SchemaTreeItem {
                                            name: schema.name.clone(),
                                            tables: schema.tables.clone(),
                                            is_expanded: false,
                                        })
                                        .collect(),
                                    is_expanded: false,
                                })
                                .collect();
                        }

                        logging::info(&format!(
                            "Successfully connected and prefetched: {}",
                            connection.connection_config.name
                        ))?;
                    }
                    Err(e) => {
                        // Fallback to simple connection without prefetching
                        logging::warn(&format!(
                            "Prefetching failed for {}, falling back to simple connection: {}",
                            connection.connection_config.name, e
                        ))?;

                        match self.connection_manager.connect(cfg.clone()).await {
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
                                            let error_msg =
                                                format!("Failed to list databases: {}", e);
                                            logging::error(&error_msg)?;
                                            return Err(anyhow::anyhow!(error_msg));
                                        }
                                    }
                                } else {
                                    connection.status = ConnectionStatus::Failed;
                                    let error_msg =
                                        "Connection not found after successful connection"
                                            .to_string();
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
                    // Check if we have prefetched data
                    if let Some(prefetched) = self
                        .prefetched_structures
                        .get(&connection.connection_config.name)
                    {
                        if let Some(prefetched_db) = prefetched
                            .databases
                            .iter()
                            .find(|db| db.name == database.name)
                        {
                            // Check if schemas are already loaded (not empty)
                            if !prefetched_db.schemas.is_empty() {
                                // Use existing prefetched schemas
                                database.schemas = prefetched_db
                                    .schemas
                                    .iter()
                                    .map(|schema| SchemaTreeItem {
                                        name: schema.name.clone(),
                                        tables: schema.tables.clone(),
                                        is_expanded: false,
                                    })
                                    .collect();
                                database.is_expanded = true;
                                return Ok(());
                            }
                        }
                    }

                    // Need to prefetch schemas for this database
                    logging::info(&format!(
                        "Prefetching schemas for database: {}",
                        database.name
                    ))?;

                    // Ensure we have a connection first
                    if !self
                        .connection_manager
                        .connections
                        .contains_key(&connection.connection_config.name)
                    {
                        logging::info(&format!(
                            "Creating connection for: {}",
                            connection.connection_config.name
                        ))?;

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

                        self.connection_manager.connect(cfg).await?;
                    }

                    match self
                        .connection_manager
                        .prefetch_schemas_for_database(
                            &connection.connection_config.name,
                            &database.name,
                        )
                        .await
                    {
                        Ok(schemas) => {
                            // Filter schemas based on configuration
                            let filtered_schemas: Vec<_> = if let Some(db_config) = connection
                                .connection_config
                                .get_database_config(&database.name)
                            {
                                if !db_config.schemas.is_empty() {
                                    // Only show configured schemas
                                    schemas
                                        .into_iter()
                                        .filter(|schema| db_config.schemas.contains(&schema.name))
                                        .collect()
                                } else {
                                    // Show all schemas if none configured
                                    schemas
                                }
                            } else {
                                // Show all schemas if no database config
                                schemas
                            };

                            // Update the prefetched structure
                            if let Some(prefetched) = self
                                .prefetched_structures
                                .get_mut(&connection.connection_config.name)
                            {
                                if let Some(prefetched_db) = prefetched
                                    .databases
                                    .iter_mut()
                                    .find(|db| db.name == database.name)
                                {
                                    prefetched_db.schemas = filtered_schemas.clone();
                                }
                            }

                            // Update the UI
                            database.schemas = filtered_schemas
                                .iter()
                                .map(|schema| SchemaTreeItem {
                                    name: schema.name.clone(),
                                    tables: schema.tables.clone(),
                                    is_expanded: false,
                                })
                                .collect();

                            database.is_expanded = true;
                            logging::info(&format!(
                                "Successfully prefetched schemas for database: {}",
                                database.name
                            ))?;
                        }
                        Err(e) => {
                            logging::error(&format!("Failed to prefetch schemas: {}", e))?;
                            return Err(e);
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
                        // Check if we have prefetched data
                        if let Some(prefetched) = self
                            .prefetched_structures
                            .get(&connection.connection_config.name)
                        {
                            if let Some(prefetched_db) = prefetched
                                .databases
                                .iter()
                                .find(|db| db.name == database.name)
                            {
                                if let Some(prefetched_schema) =
                                    prefetched_db.schemas.iter().find(|s| s.name == schema.name)
                                {
                                    // Check if tables are already loaded (not empty)
                                    if !prefetched_schema.tables.is_empty() {
                                        // Use existing prefetched tables
                                        schema.tables = prefetched_schema.tables.clone();
                                        schema.is_expanded = true;
                                        logging::info(&format!(
                                            "Successfully expanded schema {} using prefetched data",
                                            schema.name
                                        ))?;
                                        return Ok(());
                                    }
                                }
                            }
                        }

                        // Need to prefetch tables for this schema
                        logging::info(&format!("Prefetching tables for schema: {}", schema.name))?;

                        match self
                            .connection_manager
                            .prefetch_tables_for_schema(
                                &connection.connection_config.name,
                                &schema.name,
                            )
                            .await
                        {
                            Ok(tables) => {
                                // Update the prefetched structure
                                if let Some(prefetched) = self
                                    .prefetched_structures
                                    .get_mut(&connection.connection_config.name)
                                {
                                    if let Some(prefetched_db) = prefetched
                                        .databases
                                        .iter_mut()
                                        .find(|db| db.name == database.name)
                                    {
                                        if let Some(prefetched_schema) = prefetched_db
                                            .schemas
                                            .iter_mut()
                                            .find(|s| s.name == schema.name)
                                        {
                                            prefetched_schema.tables = tables.clone();
                                        }
                                    }
                                }

                                // Update the UI
                                schema.tables = tables;
                                schema.is_expanded = true;
                                logging::info(&format!(
                                    "Successfully prefetched tables for schema: {}",
                                    schema.name
                                ))?;
                            }
                            Err(e) => {
                                logging::error(&format!("Failed to prefetch tables: {}", e))?;
                                return Err(e);
                            }
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
                default_database: Some(self.connection_form.database.clone()),
                databases: std::collections::HashMap::new(),
                ssh_tunnel: None,
                ssh_tunnel_name: self.connection_form.ssh_tunnel_name.clone(),
                database: Some(self.connection_form.database.clone()),
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

    pub fn get_visual_index_for_connection(&self, connection_index: usize) -> Option<usize> {
        let mut visual_index = 0;
        for (idx, connection) in self.connection_tree.iter().enumerate() {
            if idx == connection_index {
                return Some(visual_index);
            }
            visual_index += 1;
            if connection.is_expanded {
                for database in &connection.databases {
                    visual_index += 1;
                    if database.is_expanded {
                        for schema in &database.schemas {
                            visual_index += 1;
                            if schema.is_expanded {
                                visual_index += schema.tables.len();
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Gets the visual index for a given tree item.

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
                    // Convert character position to byte position safely
                    let byte_pos = state
                        .where_clause
                        .char_indices()
                        .nth(cursor_pos)
                        .map(|(pos, _)| pos)
                        .unwrap_or(state.where_clause.len());
                    state.where_clause.insert(byte_pos, c);
                    self.cursor_position.1 += 1;
                }
                1 => {
                    // Convert character position to byte position safely
                    let byte_pos = state
                        .order_by_clause
                        .char_indices()
                        .nth(cursor_pos)
                        .map(|(pos, _)| pos)
                        .unwrap_or(state.order_by_clause.len());
                    state.order_by_clause.insert(byte_pos, c);
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
                        // Convert character position to byte position safely
                        if let Some((byte_pos, _)) =
                            state.where_clause.char_indices().nth(cursor_pos - 1)
                        {
                            state.where_clause.remove(byte_pos);
                            self.cursor_position.1 -= 1;
                        }
                    }
                    1 => {
                        // Convert character position to byte position safely
                        if let Some((byte_pos, _)) =
                            state.order_by_clause.char_indices().nth(cursor_pos - 1)
                        {
                            state.order_by_clause.remove(byte_pos);
                            self.cursor_position.1 -= 1;
                        }
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
                    // Convert character position to byte position safely
                    if let Some((byte_pos, _)) = state.where_clause.char_indices().nth(cursor_pos) {
                        state.where_clause.remove(byte_pos);
                    }
                }
                1 => {
                    // Convert character position to byte position safely
                    if let Some((byte_pos, _)) =
                        state.order_by_clause.char_indices().nth(cursor_pos)
                    {
                        state.order_by_clause.remove(byte_pos);
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
                    // Convert character position to byte position safely
                    if let Some((byte_pos, _)) = state.where_clause.char_indices().nth(cursor_pos) {
                        state.where_clause.remove(byte_pos);
                        state.where_clause.insert(byte_pos, c);
                    }
                }
                1 => {
                    // Convert character position to byte position safely
                    if let Some((byte_pos, _)) =
                        state.order_by_clause.char_indices().nth(cursor_pos)
                    {
                        state.order_by_clause.remove(byte_pos);
                        state.order_by_clause.insert(byte_pos, c);
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

    /// Sync VimEditor content back to query state
    pub fn sync_vim_editor_to_query_state(&mut self) {
        let content = self
            .navigation_manager
            .box_manager()
            .vim_editor()
            .content()
            .to_string();
        let field_idx = self.cursor_position.0;

        if let Some(state) = self.current_query_state_mut() {
            match field_idx {
                0 => state.where_clause = content,
                1 => state.order_by_clause = content,
                _ => {}
            }
        }
    }

    /// Get word at position in text
    fn get_word_at_position(&self, text: &str, pos: usize) -> String {
        if pos >= text.len() {
            return String::new();
        }

        let chars: Vec<char> = text.chars().collect();
        let mut start = pos;
        let mut end = pos;

        // Find word boundaries
        while start > 0 && chars[start - 1].is_alphanumeric() {
            start -= 1;
        }

        while end < chars.len() && chars[end].is_alphanumeric() {
            end += 1;
        }

        if start < end {
            chars[start..end].iter().collect()
        } else {
            String::new()
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
                                                let tab_name = format!(
                                                    "{}:{}:{}.{}",
                                                    connection.connection_config.name,
                                                    database.name,
                                                    schema.name,
                                                    table
                                                );
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

    pub fn select_connection(&mut self, index: usize) {
        self.selected_connection_idx = Some(index);
    }

    pub fn focus_connections(&mut self) {
        self.active_pane = Pane::Connections;
        self.input_mode = InputMode::Normal;
        // Sync navigation manager's vim mode
        self.navigation_manager
            .box_manager_mut()
            .vim_editor_mut()
            .mode = crate::navigation::types::VimMode::Normal;
        self.last_key_was_d = false;
        self.awaiting_replace = false;
        self.navigation_manager
            .handle_action(crate::navigation::types::NavigationAction::FocusConnections);
    }

    pub fn focus_query_input(&mut self, field: QueryField, position: usize) {
        self.active_pane = Pane::QueryInput;
        self.input_mode = InputMode::Insert;
        self.last_key_was_d = false;
        self.awaiting_replace = false;
        // Get content before mutable borrow
        let content = self
            .current_query_state()
            .map(|state| match field {
                QueryField::Where => state.where_clause.clone(),
                QueryField::OrderBy => state.order_by_clause.clone(),
            })
            .unwrap_or_default();

        // Sync navigation manager's vim mode and cursor position
        let vim_editor = self.navigation_manager.box_manager_mut().vim_editor_mut();
        vim_editor.mode = crate::navigation::types::VimMode::Insert;
        vim_editor.set_cursor_position(self.cursor_position);
        vim_editor.set_content(content);
        self.cursor_position = match field {
            QueryField::Where => (0, position),
            QueryField::OrderBy => (1, position),
        };
        self.navigation_manager
            .handle_action(crate::navigation::types::NavigationAction::FocusQueryInput);
    }

    pub fn focus_results(&mut self, column: usize, row: usize) {
        self.active_pane = Pane::Results;
        self.input_mode = InputMode::Normal;
        // Sync navigation manager's vim mode
        self.navigation_manager
            .box_manager_mut()
            .vim_editor_mut()
            .mode = crate::navigation::types::VimMode::Normal;
        self.last_key_was_d = false;
        self.awaiting_replace = false;
        self.cursor_position = (column, row);
        self.navigation_manager
            .handle_action(crate::navigation::types::NavigationAction::FocusResults);
    }

    pub fn select_tab(&mut self, index: usize) {
        if index < self.result_tabs.len() {
            self.selected_result_tab_index = Some(index);
            self.cursor_position = (0, 0);
        }
    }

    pub fn select_previous_connection(&mut self) {
        if let Some(TreeItem::Connection(conn_idx)) = self
            .selected_connection_idx
            .and_then(|idx| self.get_tree_item_at_visual_index(idx))
        {
            if conn_idx > 0 {
                self.selected_connection_idx = self
                    .get_visual_index_for_connection(conn_idx - 1)
                    .or(Some(conn_idx - 1));
            }
        }
    }

    pub fn select_next_connection(&mut self) {
        if let Some(TreeItem::Connection(conn_idx)) = self
            .selected_connection_idx
            .and_then(|idx| self.get_tree_item_at_visual_index(idx))
        {
            let total_connections = self.connection_tree.len();
            if conn_idx + 1 < total_connections {
                self.selected_connection_idx = self
                    .get_visual_index_for_connection(conn_idx + 1)
                    .or(Some(conn_idx + 1));
            }
        }
    }

    /// Moves the selection down in the connections tree.
    pub fn move_selection_down(&mut self) {
        let total_items = self.get_total_visible_items();

        if let Some(current_idx) = self.selected_connection_idx {
            if current_idx + 1 < total_items {
                self.selected_connection_idx = Some(current_idx + 1);
            }
        } else if total_items > 0 {
            // If nothing is selected, select the first item
            self.selected_connection_idx = Some(0);
        }
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

    /// Lists available themes
    pub fn list_themes(&mut self) -> anyhow::Result<()> {
        self.toggle_themes_modal();
        Ok(())
    }

    /// Switches to a different theme
    pub fn switch_theme(&mut self, theme_name: &str) -> anyhow::Result<()> {
        match self.config.switch_theme(theme_name) {
            Ok(()) => {
                self.status_message = Some(format!("Switched to theme: {}", theme_name));
                logging::info(&format!("Theme switched to: {}", theme_name))?;
            }
            Err(e) => {
                let error_msg = format!("Failed to switch theme: {}", e);
                logging::error(&error_msg)?;
                self.status_message = Some(error_msg);
            }
        }
        Ok(())
    }

    /// Updates command suggestions based on current input
    pub fn update_command_suggestions(&mut self) {
        // Only show suggestions if there's meaningful input (more than just the command prompt)
        if !self.command_input.is_empty() {
            self.command_suggestions =
                crate::command::CommandProcessor::get_suggestions(&self.command_input);
        } else {
            self.command_suggestions.clear();
        }

        self.selected_suggestion = if self.command_suggestions.is_empty() {
            None
        } else {
            Some(0)
        };
        self.suggestions_scroll_offset = 0; // Reset scroll when suggestions change
    }

    /// Moves suggestion selection up
    pub fn select_previous_suggestion(&mut self) {
        if let Some(selected) = self.selected_suggestion {
            let new_selection = if selected > 0 {
                selected - 1
            } else {
                self.command_suggestions.len() - 1
            };
            self.selected_suggestion = Some(new_selection);

            // Update scroll offset to keep selected item visible
            self.update_scroll_offset();

            // Update command input with preview
            if let Some(suggestion) = self.command_suggestions.get(new_selection) {
                self.command_input = suggestion.clone();

                // Preview theme changes for theme suggestions
                if suggestion.starts_with("theme ") || suggestion.starts_with("switchTheme ") {
                    let theme_name = if suggestion.starts_with("theme ") {
                        suggestion.strip_prefix("theme ").unwrap_or("")
                    } else {
                        suggestion.strip_prefix("switchTheme ").unwrap_or("")
                    };
                    if !theme_name.is_empty() {
                        let theme_name = theme_name.to_string(); // Clone to avoid borrow issues
                        let _ = self.switch_theme(&theme_name);
                    }
                }
            }
        }
    }

    /// Moves suggestion selection down
    pub fn select_next_suggestion(&mut self) {
        if let Some(selected) = self.selected_suggestion {
            let new_selection = if selected + 1 < self.command_suggestions.len() {
                selected + 1
            } else {
                0
            };
            self.selected_suggestion = Some(new_selection);

            // Update scroll offset to keep selected item visible
            self.update_scroll_offset();

            // Update command input with preview
            if let Some(suggestion) = self.command_suggestions.get(new_selection) {
                self.command_input = suggestion.clone();

                // Preview theme changes for theme suggestions
                if suggestion.starts_with("theme ") || suggestion.starts_with("switchTheme ") {
                    let theme_name = if suggestion.starts_with("theme ") {
                        suggestion.strip_prefix("theme ").unwrap_or("")
                    } else {
                        suggestion.strip_prefix("switchTheme ").unwrap_or("")
                    };
                    if !theme_name.is_empty() {
                        let theme_name = theme_name.to_string(); // Clone to avoid borrow issues
                        let _ = self.switch_theme(&theme_name);
                    }
                }
            }
        }
    }

    /// Gets the currently selected suggestion
    pub fn get_selected_suggestion(&self) -> Option<&String> {
        self.selected_suggestion
            .and_then(|idx| self.command_suggestions.get(idx))
    }

    /// Updates scroll offset to keep selected item visible
    fn update_scroll_offset(&mut self) {
        const VISIBLE_ITEMS: usize = 6; // Max visible items in dropdown

        if let Some(selected) = self.selected_suggestion {
            let total_items = self.command_suggestions.len();

            if total_items <= VISIBLE_ITEMS {
                // All items fit, no scrolling needed
                self.suggestions_scroll_offset = 0;
            } else {
                // Calculate scroll offset to keep selected item visible
                if selected < self.suggestions_scroll_offset {
                    // Selected item is above visible area, scroll up
                    self.suggestions_scroll_offset = selected;
                } else if selected >= self.suggestions_scroll_offset + VISIBLE_ITEMS {
                    // Selected item is below visible area, scroll down
                    self.suggestions_scroll_offset = selected - VISIBLE_ITEMS + 1;
                }
            }
        }
    }

    pub fn get_current_theme_name(&self) -> &str {
        "default" // Placeholder - implement based on your theme system
    }

    pub async fn first_page(&mut self) -> Result<()> {
        // Implementation for first page
        Ok(())
    }

    pub async fn previous_page(&mut self) -> Result<()> {
        // Implementation for previous page
        Ok(())
    }

    pub async fn next_page(&mut self) -> Result<()> {
        // Implementation for next page
        Ok(())
    }

    pub async fn last_page(&mut self) -> Result<()> {
        // Implementation for last page
        Ok(())
    }

    pub async fn confirm_deletions(&mut self) -> Result<()> {
        // Implementation for confirming deletions
        Ok(())
    }

    pub async fn connect_to_database(&mut self, _index: usize) -> Result<()> {
        // Implementation for connecting to database
        Ok(())
    }

    pub async fn run_query(&mut self) -> Result<()> {
        // Implementation for running query
        Ok(())
    }

    pub fn clear_query(&mut self) {
        self.query.clear();
    }

    pub async fn save_query(&mut self) -> Result<()> {
        // Implementation for saving query
        Ok(())
    }

    pub async fn load_query(&mut self) -> Result<()> {
        // Implementation for loading query
        Ok(())
    }

    pub fn show_help(&mut self) {
        // Implementation for showing help
    }

    pub fn toggle_row_deletion_mark(&mut self) {
        // Implementation for toggling row deletion mark
    }

    pub fn clear_deletion_marks(&mut self) {
        // Implementation for clearing deletion marks
    }

    pub fn execute_command(&mut self) -> Result<()> {
        // Implementation for executing command
        Ok(())
    }

    pub fn command_history_up(&mut self) {
        // Implementation for command history up
    }

    pub fn command_history_down(&mut self) {
        // Implementation for command history down
    }

    pub fn cycle_suggestions(&mut self) {
        // Implementation for cycling suggestions
    }

    pub fn delete_selected_rows(&mut self) {
        // Implementation for deleting selected rows
    }

    pub fn undo_deletion(&mut self) {
        // Implementation for undoing deletion
    }

    pub fn move_cursor_down(&mut self) {
        // Implementation for moving cursor down
    }

    pub fn move_cursor_up(&mut self) {
        // Implementation for moving cursor up
    }

    pub fn move_cursor_left(&mut self) {
        // Implementation for moving cursor left
    }

    pub fn move_cursor_right(&mut self) {
        // Implementation for moving cursor right
    }

    pub fn page_down(&mut self) {
        // Implementation for page down
    }

    pub fn page_up(&mut self) {
        // Implementation for page up
    }

    pub fn move_cursor_to_start(&mut self) {
        // Implementation for moving cursor to start
    }

    pub fn move_cursor_to_end(&mut self) {
        // Implementation for moving cursor to end
    }

    pub fn select_next_tab(&mut self) {
        // Implementation for selecting next tab
    }

    pub fn select_previous_tab(&mut self) {
        // Implementation for selecting previous tab
    }

    pub fn get_deletion_preview(&self) -> Option<Vec<Vec<String>>> {
        // Implementation for getting deletion preview
        Some(vec![vec!["No items selected for deletion".to_string()]])
    }

    pub fn highlight_selected_item(&self, visible_index: usize) -> bool {
        if let Some(selected_idx) = self.selected_connection_idx {
            visible_index == selected_idx
        } else {
            false
        }
    }

    pub fn tree_item_at(&self, visual_index: usize) -> Option<TreeItem> {
        self.get_tree_item_at_visual_index(visual_index)
    }

    /// Sets a status message with timestamp
    pub fn set_status_message(&mut self, message: String) {
        self.status_message = Some(message);
        self.status_message_timestamp = Some(std::time::Instant::now());
    }

    /// Clears expired status messages (older than 3 seconds)
    pub fn clear_expired_status_message(&mut self) {
        if let Some(timestamp) = self.status_message_timestamp {
            if timestamp.elapsed().as_secs() >= 3 {
                self.status_message = None;
                self.status_message_timestamp = None;
            }
        }
    }
}
