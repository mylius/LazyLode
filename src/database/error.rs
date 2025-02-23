use std::error::Error;
use std::fmt;

/// Custom error type for database operations
#[derive(Debug)]
pub enum DatabaseError {
    /// Connection-related errors
    ConnectionError(String),

    /// Query execution errors
    QueryError(String),

    /// Schema-related errors
    SchemaError(String),

    /// Authentication errors
    AuthError(String),

    /// Validation errors
    ValidationError(String),

    /// Generic database errors
    Other(String),
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DatabaseError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            DatabaseError::QueryError(msg) => write!(f, "Query error: {}", msg),
            DatabaseError::SchemaError(msg) => write!(f, "Schema error: {}", msg),
            DatabaseError::AuthError(msg) => write!(f, "Authentication error: {}", msg),
            DatabaseError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            DatabaseError::Other(msg) => write!(f, "Database error: {}", msg),
        }
    }
}

impl Error for DatabaseError {}

/// Type alias for database operation results
pub type DatabaseResult<T> = Result<T, DatabaseError>;

// Implement From traits for common error types
impl From<tokio_postgres::Error> for DatabaseError {
    fn from(err: tokio_postgres::Error) -> Self {
        DatabaseError::QueryError(err.to_string())
    }
}

impl From<mongodb::error::Error> for DatabaseError {
    fn from(err: mongodb::error::Error) -> Self {
        DatabaseError::QueryError(err.to_string())
    }
}

impl From<std::io::Error> for DatabaseError {
    fn from(err: std::io::Error) -> Self {
        DatabaseError::Other(err.to_string())
    }
}

// Convert from anyhow::Error to DatabaseError
impl From<anyhow::Error> for DatabaseError {
    fn from(err: anyhow::Error) -> Self {
        DatabaseError::Other(err.to_string())
    }
}
