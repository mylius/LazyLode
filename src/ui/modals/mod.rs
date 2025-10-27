//! Modal implementations for the UI system
//!
//! This module contains concrete implementations of the Modal trait
//! for different types of modals in the application.

pub mod command;
pub mod connection;
pub mod deletion;
pub mod themes;

// Re-export modal types for convenience
pub use command::CommandModal;
pub use connection::ConnectionModal;
pub use deletion::DeletionModal;
pub use themes::ThemesModal;
