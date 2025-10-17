#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

// Re-export Pane from navigation module to avoid duplication
pub use crate::navigation::types::Pane;
