#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)] // Remove Default derive
pub enum Pane {
    Connections,
    QueryInput,
    Results,
    ConnectionModal,
}

impl Default for Pane {
    fn default() -> Self {
        Pane::Connections
    }
}
