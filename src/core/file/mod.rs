//! Git-aware file discovery and file-content processing.

pub mod process;
mod process_constants;
pub mod read;
pub mod search;
pub mod search_constants;

pub use process::{FileReadResult, FileSkipReason};
pub use read::{collect_files, read_raw_file, FileCollection, RawFile, SkippedFileInfo};
pub use search::{find_files, DiscoveredFile, FileSearchResult};
pub use search_constants::DEFAULT_IGNORE_PATTERNS;
