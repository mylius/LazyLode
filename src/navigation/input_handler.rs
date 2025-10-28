use crate::app::App;
use crate::input::{Action, NavigationAction as OldNavigationAction, TreeAction};
use crate::navigation::types::{NavigationAction, Pane as OldPane};
use crate::ui::types::Direction as OldDirection;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyModifiers};
use futures::executor;

/// Unified input handler that uses the new navigation system
pub struct NavigationInputHandler;

impl NavigationInputHandler {
    /// Handle a key event using the new navigation system
    pub async fn handle_key(key: KeyCode, modifiers: KeyModifiers, app: &mut App) -> Result<()> {
        // Handle modal input using the modal manager
        if app.modal_manager.has_modals() {
            // Allow command mode to be opened even when modal is active
            if let Some(action) = app
                .navigation_manager
                .config()
                .key_mapping
                .get_action(key, modifiers)
            {
                match action {
                    NavigationAction::EnterCommandMode => {
                        app.input_mode = crate::app::InputMode::Command;
                        app.command_input.clear();
                        app.command_buffer.clear();
                        app.update_command_suggestions();
                        app.modal_manager.push(Box::new(crate::ui::modals::CommandModal::new()));
                        return Ok(());
                    }
                    NavigationAction::Cancel | NavigationAction::Quit => {
                        app.modal_manager.close_active();
                        return Ok(());
                    }
                    _ => {}
                }
            }

            // Delegate all other input to the modal
            if app.input_mode != crate::app::InputMode::Command {
                // Check common modal keys first
                let common_result =
                    crate::ui::modal_manager::utils::handle_common_keys(key, modifiers, app);
                if let Some(result) = common_result {
                    if matches!(result, crate::ui::modal_manager::ModalResult::Closed) {
                        app.modal_manager.close_active();
                    }
                    return Ok(());
                }

                // For all modals, use their own handle_input
                // Clone key mapping first to avoid borrow conflicts
                let key_mapping = app.navigation_manager.config().key_mapping.clone();
                let nav_action = key_mapping.get_action(key, modifiers);

                let result = if let Some(modal) = app.modal_manager.stack.last_mut() {
                    // Pass the navigation action we already resolved
                    modal.handle_input(key, modifiers, nav_action)
                } else {
                    crate::ui::modal_manager::ModalResult::Continue
                };

                // Process result with mutable access to app
                match result {
                    crate::ui::modal_manager::ModalResult::Closed => {
                        app.modal_manager.close_active();
                    }
                    crate::ui::modal_manager::ModalResult::Action(action) => {
                        // Handle modal actions
                        if action.starts_with("apply_theme:") {
                            let theme_name = action.strip_prefix("apply_theme:").unwrap_or("");
                            let _ = app.switch_theme(theme_name);
                            app.modal_manager.close_active();
                        } else if action.starts_with("create_connection:") {
                            // TODO: Parse and create connection
                            let parts: Vec<&str> = action.split(':').collect();
                            if parts.len() >= 6 {
                                let name = parts[1];
                                let host = parts[2];
                                let port = parts[3];
                                let _username = parts[4];
                                let _password = parts[5];
                                let _database = if parts.len() > 6 { parts[6] } else { "" };
                                // TODO: Actually create the connection in app
                                println!("Create connection: {name}@{host}:{port}/{_database}");
                                app.modal_manager.close_active();
                            }
                        }
                    }
                    _ => {}
                }
                return Ok(());
            }
        }

        // Handle command mode input
        if app.input_mode == crate::app::InputMode::Command {
            Self::handle_command_mode_input(key, modifiers, app).await?;
            return Ok(());
        }

        // Handle pane-specific input based on input mode
        match app.active_pane {
            OldPane::Connections => {
                Self::handle_connections_input(key, modifiers, app).await?;
            }
            OldPane::QueryInput => {
                Self::handle_query_input(key, modifiers, app).await?;
            }
            OldPane::Results => {
                Self::handle_results_input(key, modifiers, app).await?;
            }
            OldPane::SchemaExplorer => {}
            OldPane::CommandLine => {}
        }

        Ok(())
    }

    /// Handle command mode input
    async fn handle_command_mode_input(
        key: KeyCode,
        _modifiers: KeyModifiers,
        app: &mut App,
    ) -> Result<()> {
        match key {
            KeyCode::Esc => {
                // Exit command mode
                app.input_mode = crate::app::InputMode::Normal;
                app.command_input.clear();
                app.command_buffer.clear();
                app.command_suggestions.clear();
                app.selected_suggestion = None;
                app.modal_manager.close_active();
                // Sync navigation manager's vim mode
                app.navigation_manager
                    .box_manager_mut()
                    .vim_editor_mut()
                    .mode = crate::navigation::types::VimMode::Normal;
            }
            KeyCode::Enter => {
                // Build command string first
                let command = if let Some(suggestion) = app.get_selected_suggestion() {
                    suggestion.clone()
                } else {
                    app.command_input.clone()
                };

                // Exit command mode and close the command modal BEFORE executing
                app.input_mode = crate::app::InputMode::Normal;
                app.command_input.clear();
                app.command_suggestions.clear();
                app.selected_suggestion = None;
                app.modal_manager.close_active();
                // Sync navigation manager's vim mode
                app.navigation_manager
                    .box_manager_mut()
                    .vim_editor_mut()
                    .mode = crate::navigation::types::VimMode::Normal;

                if !command.is_empty() {
                    // Sync command to command_buffer for processing
                    app.command_buffer.clear();
                    for c in command.chars() {
                        app.command_buffer.push(c);
                    }

                    // Process the command
                    match crate::command::CommandProcessor::process_command(app) {
                        Ok(true) => {}
                        Ok(false) => {
                            app.status_message = Some(format!("Unknown command: {}", command));
                        }
                        Err(e) => {
                            let _ = crate::logging::error(&format!(
                                "Error processing command: {}",
                                e
                            ));
                            app.status_message = Some(format!("Error: {}", e));
                        }
                    }
                }
            }
            KeyCode::Backspace => {
                // Delete last character
                app.command_input.pop();
                app.update_command_suggestions();
            }
            KeyCode::Up => {
                // Navigate suggestions
                app.select_previous_suggestion();
            }
            KeyCode::Down => {
                // Navigate suggestions
                app.select_next_suggestion();
            }
            KeyCode::Tab => {
                // Auto-complete with selected suggestion
                if let Some(suggestion) = app.get_selected_suggestion() {
                    app.command_input = suggestion.clone();
                    app.update_command_suggestions();
                }
            }
            KeyCode::Char(c) => {
                // Add character to command input
                app.command_input.push(c);
                app.update_command_suggestions();

                // Reset selection to first suggestion when typing
                if !app.command_suggestions.is_empty() {
                    app.selected_suggestion = Some(0);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle navigation keys through the new system
    fn handle_navigation_key(key: KeyCode, modifiers: KeyModifiers, app: &mut App) -> bool {
        // Don't process navigation shortcuts when in insert mode, except for cursor movement
        if app.input_mode == crate::app::InputMode::Insert {
            // Allow cursor movement keys in insert mode
            match key {
                KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down => {
                    // Let the vim editor handle cursor movement
                    return app
                        .navigation_manager
                        .box_manager_mut()
                        .vim_editor_mut()
                        .handle_key(key, modifiers);
                }
                _ => return false,
            }
        }

        // Check if this key combination maps to a navigation action
        if let Some(action) = app
            .navigation_manager
            .config()
            .key_mapping
            .get_action(key, modifiers)
        {
            return Self::handle_navigation_action(action, app);
        }

        // Delegate to box manager for box-specific handling
        app.navigation_manager
            .box_manager_mut()
            .handle_key(key, modifiers)
    }

    /// Handle a navigation action
    fn handle_navigation_action(
        action: crate::navigation::types::NavigationAction,
        app: &mut App,
    ) -> bool {
        match action {
            
            // Open New Connection modal when pressing 'a' in Connections pane
            crate::navigation::types::NavigationAction::Append => {
                if app.active_pane == OldPane::Connections {
                    app.show_connection_modal();
                    return true;
                }
                app.navigation_manager.handle_action(action)
            }
            // Mode switching actions - sync with app input mode
            crate::navigation::types::NavigationAction::EnterInsertMode => {
                app.input_mode = crate::app::InputMode::Insert;
                app.navigation_manager.handle_action(action);
                // Sync app cursor position with vim editor cursor position
                app.cursor_position = app
                    .navigation_manager
                    .box_manager_mut()
                    .vim_editor_mut()
                    .cursor_position();
                true
            }
            crate::navigation::types::NavigationAction::EnterNormalMode => {
                app.input_mode = crate::app::InputMode::Normal;
                app.navigation_manager.handle_action(action)
            }
            crate::navigation::types::NavigationAction::EnterCommandMode => {
                app.input_mode = crate::app::InputMode::Command;
                app.command_input.clear();
                app.command_buffer.clear();
                app.update_command_suggestions();
                app
                    .modal_manager
                    .push(Box::new(crate::ui::modals::CommandModal::new()));
                app.navigation_manager.handle_action(action)
            }
            // Movement actions - delegate to app functions
            crate::navigation::types::NavigationAction::MoveLeft => {
                match app.active_pane {
                    OldPane::QueryInput => {
                        app.handle_navigation(OldNavigationAction::Direction(OldDirection::Left));
                        // Sync vim editor cursor position with app cursor position
                        app.navigation_manager
                            .box_manager_mut()
                            .vim_editor_mut()
                            .set_cursor_position(app.cursor_position);
                    }
                    OldPane::Results => app.move_cursor_in_results(OldDirection::Left),
                    OldPane::Connections => {
                        // In connections pane, left should collapse tree items
                        if let Err(e) = executor::block_on(
                            app.handle_tree_action(crate::input::TreeAction::Collapse),
                        ) {
                            let _ = crate::logging::error(&format!(
                                "Error collapsing tree item: {}",
                                e
                            ));
                        }
                    }
                    _ => {}
                }
                true
            }
            crate::navigation::types::NavigationAction::MoveRight => {
                match app.active_pane {
                    OldPane::QueryInput => {
                        app.handle_navigation(OldNavigationAction::Direction(OldDirection::Right));
                        // Sync vim editor cursor position with app cursor position
                        app.navigation_manager
                            .box_manager_mut()
                            .vim_editor_mut()
                            .set_cursor_position(app.cursor_position);
                    }
                    OldPane::Results => app.move_cursor_in_results(OldDirection::Right),
                    OldPane::Connections => {
                        // In connections pane, right should expand tree items
                        if let Err(e) = executor::block_on(
                            app.handle_tree_action(crate::input::TreeAction::Expand),
                        ) {
                            let _ =
                                crate::logging::error(&format!("Error expanding tree item: {}", e));
                        }
                    }
                    _ => {}
                }
                true
            }
            crate::navigation::types::NavigationAction::MoveUp => {
                match app.active_pane {
                    OldPane::Results => app.move_cursor_in_results(OldDirection::Up),
                    OldPane::Connections => app.move_selection_up(),
                    OldPane::QueryInput => {
                        app.handle_navigation(OldNavigationAction::Direction(OldDirection::Up));
                        // Sync vim editor cursor position with app cursor position
                        app.navigation_manager
                            .box_manager_mut()
                            .vim_editor_mut()
                            .set_cursor_position(app.cursor_position);
                    }
                    _ => {}
                }
                true
            }
            crate::navigation::types::NavigationAction::MoveDown => {
                match app.active_pane {
                    OldPane::Results => app.move_cursor_in_results(OldDirection::Down),
                    OldPane::Connections => app.move_selection_down(),
                    OldPane::QueryInput => {
                        app.handle_navigation(OldNavigationAction::Direction(OldDirection::Down));
                        // Sync vim editor cursor position with app cursor position
                        app.navigation_manager
                            .box_manager_mut()
                            .vim_editor_mut()
                            .set_cursor_position(app.cursor_position);
                    }
                    _ => {}
                }
                true
            }
            // Pane navigation actions - delegate to navigation manager
            crate::navigation::types::NavigationAction::FocusConnections
            | crate::navigation::types::NavigationAction::FocusQueryInput
            | crate::navigation::types::NavigationAction::FocusResults
            // | crate::navigation::types::NavigationAction::FocusSchemaExplorer // Removed - not implemented in UI
            // | crate::navigation::types::NavigationAction::FocusCommandLine // Removed - command mode is handled via InputMode::Command
            | crate::navigation::types::NavigationAction::FocusPaneLeft
            | crate::navigation::types::NavigationAction::FocusPaneRight
            | crate::navigation::types::NavigationAction::FocusPaneUp
            | crate::navigation::types::NavigationAction::FocusPaneDown
            | crate::navigation::types::NavigationAction::NextPane
            | crate::navigation::types::NavigationAction::PreviousPane => {
                let handled = app.navigation_manager.handle_action(action);
                if handled {
                    // Sync app's active_pane with navigation manager's state
                    app.active_pane = app.navigation_manager.get_active_pane();
                }
                handled
            }
            // Special actions
            crate::navigation::types::NavigationAction::FocusCommandLine => {
                // Enter command mode instead of navigating to a pane
                app.input_mode = crate::app::InputMode::Command;
                app.command_input.clear();
                app.command_buffer.clear();
                app.update_command_suggestions();
                app.navigation_manager.handle_action(crate::navigation::types::NavigationAction::EnterCommandMode);
                true
            }
            crate::navigation::types::NavigationAction::Quit => {
                app.quit();
                true
            }
            crate::navigation::types::NavigationAction::Search => {
                if !app.modal_manager.has_modals() {
                    app.focus_where_input();
                }
                true
            }
            // Other actions - delegate to navigation manager
            _ => {
                let handled = app.navigation_manager.handle_action(action);
                if handled {
                    // Sync app's active_pane with navigation manager's state
                    app.active_pane = app.navigation_manager.get_active_pane();
                }
                handled
            }
        }
    }

    /// Handle legacy key events for backward compatibility
    async fn handle_legacy_key(key: KeyCode, modifiers: KeyModifiers, app: &mut App) -> Result<()> {
        // Quit is now handled by the navigation system mappings

        // Search key is now handled by the navigation system mappings

        // Handle modal input
        if app.modal_manager.has_modals() {
            if let crate::app::ActiveBlock::ConnectionModal = app.active_block {
                // Handle connection modal input - this will be handled by the modal manager
                return Ok(());
            }
            return Ok(());
        }

        // Handle pane-specific input
        match app.active_pane {
            OldPane::Connections => {
                Self::handle_connections_input(key, modifiers, app).await?;
            }
            OldPane::QueryInput => {
                Self::handle_query_input(key, modifiers, app).await?;
            }
            OldPane::Results => {
                Self::handle_results_input(key, modifiers, app).await?;
            }
            OldPane::SchemaExplorer => {
                Self::handle_connections_input(key, modifiers, app).await?;
            }
            OldPane::CommandLine => {
                Self::handle_query_input(key, modifiers, app).await?;
            }
        }

        Ok(())
    }

    async fn handle_connections_input(
        key: KeyCode,
        modifiers: KeyModifiers,
        app: &mut App,
    ) -> Result<()> {
        match app.input_mode {
            crate::app::InputMode::Normal => {
                // In normal mode, try the new navigation system first
                if Self::handle_navigation_key(key, modifiers, app) {
                    return Ok(());
                }
                // Fall back to legacy connections handling
                Self::handle_connections_input_normal_mode(key, modifiers, app).await
            }
            _ => Ok(()),
        }
    }

    async fn handle_connections_input_normal_mode(
        key: KeyCode,
        modifiers: KeyModifiers,
        app: &mut App,
    ) -> Result<()> {
        if let Some(action) = app.config.keymap.get_action(key, KeyModifiers::empty()) {
            match action {
                Action::Navigation(nav_action) => match nav_action {
                    OldNavigationAction::Direction(direction) => match direction {
                        OldDirection::Up => app.move_selection_up(),
                        OldDirection::Down => app.move_selection_down(),
                        OldDirection::Right => {
                            if let Err(e) = app.handle_tree_action(TreeAction::Expand).await {
                                let _ = crate::logging::error(&format!(
                                    "Error expanding connection: {}",
                                    e
                                ));
                            }
                        }
                        _ => {}
                    },
                    OldNavigationAction::FocusPane(pane) => {
                        app.active_pane = pane;
                    }
                    _ => {
                        app.handle_navigation(nav_action);
                    }
                },
                Action::FirstPage => {
                    if let Err(e) = app.first_page().await {
                        let _ = crate::logging::error(&format!("Error going to first page: {}", e));
                    }
                }
                Action::PreviousPage => {
                    if let Err(e) = app.previous_page().await {
                        let _ =
                            crate::logging::error(&format!("Error going to previous page: {}", e));
                    }
                }
                Action::NextPage => {
                    if let Err(e) = app.next_page().await {
                        let _ = crate::logging::error(&format!("Error going to next page: {}", e));
                    }
                }
                Action::LastPage => {
                    if let Err(e) = app.last_page().await {
                        let _ = crate::logging::error(&format!("Error going to last page: {}", e));
                    }
                }
                Action::TreeAction(tree_action) => {
                    if let Err(e) = app.handle_tree_action(tree_action).await {
                        let _ = crate::logging::error(&format!("Error in tree action: {}", e));
                    }
                }
                Action::Edit => {
                    if let Some(index) = app.selected_connection_idx {
                        let connection = &app.saved_connections[index];
                        app.connection_form = crate::app::ConnectionForm {
                            name: connection.name.clone(),
                            db_type: connection.db_type.clone(),
                            host: connection.host.clone(),
                            port: connection.port.to_string(),
                            username: connection.username.clone(),
                            password: connection.password.clone().unwrap_or_default(),
                            database: connection.database.clone().unwrap_or_default(),
                            editing_index: Some(index),
                            current_field: 0,
                            ssh_enabled: connection.ssh_tunnel.is_some(),
                            ssh_host: connection.ssh_tunnel.clone().unwrap_or_default().host,
                            ssh_username: connection
                                .ssh_tunnel
                                .clone()
                                .unwrap_or_default()
                                .username,
                            ssh_port: connection
                                .ssh_tunnel
                                .clone()
                                .unwrap_or_default()
                                .port
                                .to_string(),
                            ssh_password: connection
                                .ssh_tunnel
                                .clone()
                                .unwrap_or_default()
                                .password
                                .unwrap_or_default(),
                            ssh_key_path: connection
                                .ssh_tunnel
                                .clone()
                                .unwrap_or_default()
                                .private_key_path
                                .unwrap_or_default(),
                            ssh_tunnel_name: connection.ssh_tunnel_name.clone(),
                        };
                        app.show_connection_modal();
                        app.active_block = crate::app::ActiveBlock::ConnectionModal;
                        app.input_mode = crate::app::InputMode::Normal;
                        // Sync navigation manager's vim mode
                        app.navigation_manager
                            .box_manager_mut()
                            .vim_editor_mut()
                            .mode = crate::navigation::types::VimMode::Normal;
                    }
                }
                Action::Delete => {
                    app.delete_connection();
                }
                _ => {}
            }
        } else {
            match key {
                // 'q' is now handled by the navigation system mappings
                KeyCode::Char('a') if modifiers.is_empty() => {
                    app.show_connection_modal();
                    app.active_block = crate::app::ActiveBlock::ConnectionModal;
                    app.input_mode = crate::app::InputMode::Normal;
                    // Sync navigation manager's vim mode
                    app.navigation_manager
                        .box_manager_mut()
                        .vim_editor_mut()
                        .mode = crate::navigation::types::VimMode::Normal;
                }
                _ => {}
            }
        }
        Ok(())
    }

    async fn handle_query_input(
        key: KeyCode,
        modifiers: KeyModifiers,
        app: &mut App,
    ) -> Result<()> {
        // Use the new QueryInputPane for input handling
        let nav_action = app
            .navigation_manager
            .config()
            .key_mapping
            .get_action(key, modifiers);

        if app
            .query_input_pane
            .handle_input(key, modifiers, nav_action)
        {
            return Ok(());
        }

        // Check if we need to handle Enter key for query execution
        if key == KeyCode::Enter {
            let current_mode = app.query_input_pane.current_vim_mode();
            if current_mode == crate::navigation::types::VimMode::Insert {
                // Sync content from QueryInputPane to query state before executing
                let where_content = app.query_input_pane.get_where_content();
                let order_by_content = app.query_input_pane.get_order_by_content();
                if let Some(state) = app.current_query_state_mut() {
                    state.where_clause = where_content;
                    state.order_by_clause = order_by_content;
                }
                if let Err(e) = app.refresh_results().await {
                    let _ = crate::logging::error(&format!("Error refreshing results: {}", e));
                }
                app.input_mode = crate::app::InputMode::Normal;
                app.query_input_pane.exit_insert_mode();
                return Ok(());
            }
        }

        // Fallback to old system if pane didn't handle it
        match app.input_mode {
            crate::app::InputMode::Normal => {
                // In normal mode, try the new navigation system first
                if Self::handle_navigation_key(key, modifiers, app) {
                    return Ok(());
                }
                // Fall back to legacy normal mode handling
                Self::handle_query_input_normal_mode(key, modifiers, app).await
            }
            crate::app::InputMode::Insert => {
                // In insert mode, only allow navigation keys and text editing
                if matches!(key, KeyCode::Up | KeyCode::Down) && modifiers.is_empty() {
                    Self::handle_navigation_key(key, modifiers, app);
                    return Ok(());
                }
                // Handle text editing directly - don't process shortcuts
                Self::handle_query_input_insert_mode(key, modifiers, app).await
            }
            _ => Ok(()),
        }
    }

    async fn handle_query_input_normal_mode(
        key: KeyCode,
        modifiers: KeyModifiers,
        app: &mut App,
    ) -> Result<()> {
        // First try the navigation system for mapped keys
        if Self::handle_navigation_key(key, modifiers, app) {
            return Ok(());
        }

        // All keys should now be handled by the navigation system mappings

        // Fallback to keymap (pane switching etc.)
        if let Some(action) = app.config.keymap.get_action(key, modifiers) {
            match action {
                Action::Navigation(OldNavigationAction::FocusPane(pane)) => {
                    app.active_pane = pane;
                    if pane == OldPane::QueryInput {
                        let len = app.get_current_field_length();
                        app.cursor_position.1 = app.cursor_position.1.min(len);
                    }
                }
                Action::Navigation(nav_action) => {
                    app.handle_navigation(nav_action);
                }
                _ => {}
            }
            app.last_key_was_d = false;
            app.awaiting_replace = false;
        }
        Ok(())
    }

    async fn handle_query_input_insert_mode(
        key: KeyCode,
        modifiers: KeyModifiers,
        app: &mut App,
    ) -> Result<()> {
        match key {
            KeyCode::Esc => {
                app.input_mode = crate::app::InputMode::Normal;
                app.navigation_manager
                    .box_manager_mut()
                    .vim_editor_mut()
                    .mode = crate::navigation::types::VimMode::Normal;
                // Sync content back to query state
                app.sync_vim_editor_to_query_state();
            }
            KeyCode::Enter => {
                // Sync content from QueryInputPane to query state before executing
                let where_content = app.query_input_pane.get_where_content();
                let order_by_content = app.query_input_pane.get_order_by_content();
                if let Some(state) = app.current_query_state_mut() {
                    state.where_clause = where_content;
                    state.order_by_clause = order_by_content;
                }
                if let Err(e) = app.refresh_results().await {
                    let _ = crate::logging::error(&format!("Error refreshing results: {}", e));
                }
                app.input_mode = crate::app::InputMode::Normal;
                // Exit insert mode for all fields in the pane
                app.query_input_pane.exit_insert_mode();
            }
            KeyCode::Char('y') => {
                // Handle yank word in insert mode
                let vim_editor = app.navigation_manager.box_manager_mut().vim_editor_mut();
                if let Some(status) = vim_editor.yank_word() {
                    app.status_message = Some(format!("DEBUG: {}", status));
                }
                // Sync cursor position back to app
                app.cursor_position = vim_editor.cursor_position();
            }
            KeyCode::Char('Y') => {
                // Handle yank line in insert mode
                let vim_editor = app.navigation_manager.box_manager_mut().vim_editor_mut();
                if let Some(status) = vim_editor.yank_line() {
                    app.status_message = Some(format!("DEBUG: {}", status));
                }
                // Sync cursor position back to app
                app.cursor_position = vim_editor.cursor_position();
            }
            _ => {
                // Let VimEditor handle all other keys
                let vim_editor = app.navigation_manager.box_manager_mut().vim_editor_mut();
                vim_editor.handle_key(key, modifiers);
                // Sync cursor position back to app
                app.cursor_position = vim_editor.cursor_position();
            }
        }
        Ok(())
    }

    async fn handle_results_input(
        key: KeyCode,
        modifiers: KeyModifiers,
        app: &mut App,
    ) -> Result<()> {
        match app.input_mode {
            crate::app::InputMode::Normal => {
                // In normal mode, try the new navigation system first
                if Self::handle_navigation_key(key, modifiers, app) {
                    return Ok(());
                }
                // Fall back to legacy results handling
                Self::handle_results_input_normal_mode(key, modifiers, app).await
            }
            _ => Ok(()),
        }
    }

    async fn handle_results_input_normal_mode(
        _key: KeyCode,
        _modifiers: KeyModifiers,
        _app: &mut App,
    ) -> Result<()> {
        // TODO: Handle deletion modal when implemented
        return Ok(());
    }
}
