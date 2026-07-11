//! Native Rust implementation of Repomix.
//!
//! The public modules mirror the major areas of the TypeScript reference
//! implementation: CLI, configuration, core packing, MCP, and shared helpers.

pub mod cli;
pub mod config;
pub mod core;
pub mod mcp;
pub mod shared;

pub use shared::error::{handle_error, RepomixError, Result};
pub use shared::logger::{init_tracing, set_log_level, LogLevel};
