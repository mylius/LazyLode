use crate::navigation::types::NavigationAction;

/// Generic field navigator for moving between input fields
#[derive(Debug)]
pub struct FieldNavigator {
    current_field: usize,
    field_count: usize,
}

impl FieldNavigator {
    pub fn new(field_count: usize) -> Self {
        Self {
            current_field: 0,
            field_count,
        }
    }

    pub fn current_field(&self) -> usize {
        self.current_field
    }

    pub fn next_field(&mut self) {
        if self.field_count > 0 {
            self.current_field = (self.current_field + 1) % self.field_count;
        }
    }

    pub fn previous_field(&mut self) {
        if self.field_count > 0 {
            self.current_field = if self.current_field == 0 {
                self.field_count - 1
            } else {
                self.current_field - 1
            };
        }
    }

    pub fn set_field(&mut self, field: usize) {
        if field < self.field_count {
            self.current_field = field;
        }
    }

    pub fn handle_action(&mut self, action: NavigationAction) -> bool {
        match action {
            NavigationAction::MoveDown => {
                self.next_field();
                true
            }
            NavigationAction::MoveUp => {
                self.previous_field();
                true
            }
            _ => false,
        }
    }
}
