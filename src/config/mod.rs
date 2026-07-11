//! Configuration schema, loading, and CLI override merging.

pub mod loader;
pub mod schema;

pub use loader::{load_file_config, merge_configs, write_default_config};
pub use schema::{FileConfig, RepomixConfig};
