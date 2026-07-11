//! Unified application errors and terminal error reporting.
//!
//! The presentation follows `repomix/src/shared/errorHandle.ts`: expected
//! Repomix errors are concise, unexpected errors remain inspectable through
//! tracing, and users receive a debug-log-level hint when appropriate.

use std::io;

use anyhow::Error as AnyhowError;
use thiserror::Error;
use tracing::error;

use crate::shared::logger::{global_logger, LogLevel};

/// Convenient result alias used by Rusty Repomix library modules.
pub type Result<T, E = RepomixError> = std::result::Result<T, E>;

/// Errors emitted by Rusty Repomix's user-facing operations.
#[derive(Debug, Error)]
pub enum RepomixError {
    #[error("I/O error while {context}: {source}")]
    Io {
        context: String,
        #[source]
        source: io::Error,
    },

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Configuration validation error: {0}")]
    ConfigValidation(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Security error: {0}")]
    Security(String),

    #[error("Tokenization error: {0}")]
    Tokenization(String),

    #[error("Operation cancelled")]
    OperationCancelled,

    #[error("Unexpected error: {0}")]
    Unexpected(#[source] AnyhowError),
}

impl RepomixError {
    /// Adds the failed operation to an I/O error without discarding its source.
    pub fn io(context: impl Into<String>, source: io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }
}

impl From<AnyhowError> for RepomixError {
    fn from(error: AnyhowError) -> Self {
        Self::Unexpected(error)
    }
}

/// Prints a concise, colorized error and emits rich diagnostic context via tracing.
///
/// This intentionally keeps expected failures readable, mirroring the TypeScript
/// `handleError` behavior. Enable `RUST_LOG=debug` for chained diagnostics.
pub fn handle_error(error: &RepomixError) {
    let logger = global_logger();
    logger.log("");
    logger.error(format!("✖ {error}"));

    if logger.level() < LogLevel::Debug {
        logger.log("");
        logger.note("For detailed debug information, set RUST_LOG=debug.");
    }

    error!(error = ?error, "Rusty Repomix operation failed");
}

#[cfg(test)]
mod tests {
    use std::io;

    use anyhow::anyhow;

    use super::RepomixError;

    #[test]
    fn io_error_preserves_context_and_source() {
        let error = RepomixError::io(
            "reading configuration",
            io::Error::new(io::ErrorKind::NotFound, "missing"),
        );

        assert_eq!(
            error.to_string(),
            "I/O error while reading configuration: missing"
        );
        assert!(std::error::Error::source(&error).is_some());
    }

    #[test]
    fn covers_the_domain_error_categories() {
        assert_eq!(
            RepomixError::Config("invalid format".into()).to_string(),
            "Configuration error: invalid format"
        );
        assert_eq!(
            RepomixError::ConfigValidation("output.style".into()).to_string(),
            "Configuration validation error: output.style"
        );
        assert_eq!(
            RepomixError::Parse("invalid XML".into()).to_string(),
            "Parse error: invalid XML"
        );
        assert_eq!(
            RepomixError::Security("secret found".into()).to_string(),
            "Security error: secret found"
        );
        assert_eq!(
            RepomixError::Tokenization("unsupported encoding".into()).to_string(),
            "Tokenization error: unsupported encoding"
        );
        assert_eq!(
            RepomixError::OperationCancelled.to_string(),
            "Operation cancelled"
        );
    }

    #[test]
    fn converts_anyhow_errors_without_losing_the_message() {
        let error: RepomixError = anyhow!("worker failed").into();

        assert_eq!(error.to_string(), "Unexpected error: worker failed");
    }
}
