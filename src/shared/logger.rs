//! Terminal logging and tracing initialization.
//!
//! Log levels and colors correspond to `repomix/src/shared/logger.ts`:
//! errors use stderr, ordinary terminal messages use stdout, and debug/trace
//! output is only enabled at the debug level.

use std::fmt::Display;
use std::sync::{Once, OnceLock, RwLock};

use colored::Colorize;
use tracing_subscriber::EnvFilter;

/// Controls which terminal messages are emitted.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
#[repr(i8)]
pub enum LogLevel {
    Silent = -1,
    Error = 0,
    Warn = 1,
    #[default]
    Info = 2,
    Debug = 3,
}

/// Terminal logger with a level independent of the tracing subscriber.
#[derive(Debug)]
pub struct RepomixLogger {
    level: RwLock<LogLevel>,
}

impl Default for RepomixLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl RepomixLogger {
    pub fn new() -> Self {
        Self {
            level: RwLock::new(LogLevel::Info),
        }
    }

    pub fn set_level(&self, level: LogLevel) {
        if let Ok(mut current) = self.level.write() {
            *current = level;
        }
    }

    pub fn level(&self) -> LogLevel {
        self.level
            .read()
            .map(|level| *level)
            .unwrap_or(LogLevel::Info)
    }

    pub fn error(&self, message: impl Display) {
        if self.level() >= LogLevel::Error {
            eprintln!("{}", message.to_string().red());
        }
    }

    pub fn warn(&self, message: impl Display) {
        if self.level() >= LogLevel::Warn {
            println!("{}", message.to_string().yellow());
        }
    }

    pub fn success(&self, message: impl Display) {
        if self.level() >= LogLevel::Info {
            println!("{}", message.to_string().green());
        }
    }

    pub fn info(&self, message: impl Display) {
        if self.level() >= LogLevel::Info {
            println!("{}", message.to_string().cyan());
        }
    }

    pub fn log(&self, message: impl Display) {
        if self.level() >= LogLevel::Info {
            println!("{message}");
        }
    }

    pub fn note(&self, message: impl Display) {
        if self.level() >= LogLevel::Info {
            println!("{}", message.to_string().dimmed());
        }
    }

    pub fn debug(&self, message: impl Display) {
        if self.level() >= LogLevel::Debug {
            println!("{}", message.to_string().blue());
        }
    }

    pub fn trace(&self, message: impl Display) {
        if self.level() >= LogLevel::Debug {
            println!("{}", message.to_string().bright_black());
        }
    }
}

static LOGGER: OnceLock<RepomixLogger> = OnceLock::new();
static TRACING: Once = Once::new();

/// Returns the process-wide terminal logger.
pub fn global_logger() -> &'static RepomixLogger {
    LOGGER.get_or_init(RepomixLogger::new)
}

/// Sets the process-wide terminal log level.
pub fn set_log_level(level: LogLevel) {
    global_logger().set_level(level);
}

/// Initializes structured tracing exactly once.
///
/// `RUST_LOG` takes precedence; without it, errors are retained and terminal
/// diagnostics remain governed by [`LogLevel`].
pub fn init_tracing() {
    TRACING.call_once(|| {
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("error"));
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .init();
    });
}

#[cfg(test)]
mod tests {
    use super::{LogLevel, RepomixLogger};

    #[test]
    fn default_level_is_info() {
        assert_eq!(RepomixLogger::new().level(), LogLevel::Info);
    }

    #[test]
    fn levels_match_the_typescript_logger_ordering() {
        assert!(LogLevel::Silent < LogLevel::Error);
        assert!(LogLevel::Error < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Debug);
    }

    #[test]
    fn level_can_be_changed() {
        let logger = RepomixLogger::new();
        logger.set_level(LogLevel::Debug);

        assert_eq!(logger.level(), LogLevel::Debug);
    }
}
