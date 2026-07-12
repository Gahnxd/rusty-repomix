//! Parallel, memory-mapped collection of discovered repository files.
//!
//! The stable output order deliberately matches the deterministic order from
//! `fileSearch.ts`/`fileCollect.ts`, while Rayon replaces its bounded Node.js
//! promise pool for native parallel I/O.

use std::fs::File;
use std::path::{Path, PathBuf};

use memmap2::MmapOptions;
use rayon::prelude::*;

use crate::config::RepomixConfig;

use super::process::{is_binary_path, process_file_bytes, FileReadResult, FileSkipReason};
use super::search::DiscoveredFile;

/// Text input admitted to later package-processing stages.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawFile {
    pub path: PathBuf,
    pub content: String,
    pub size_bytes: usize,
    pub line_count: usize,
}

/// A skipped file and the reason it was excluded from text output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkippedFileInfo {
    pub path: PathBuf,
    pub reason: FileSkipReason,
}

/// Deterministically ordered results from the parallel file-reading stage.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FileCollection {
    pub raw_files: Vec<RawFile>,
    pub skipped_files: Vec<SkippedFileInfo>,
}

/// Reads one file using a memory map and converts accepted text to UTF-8.
///
/// This mirrors `readRawFile`: individual I/O and decoding failures do not
/// abort collection; they become `encoding-error` skips instead.
pub fn read_raw_file(file_path: &Path, max_file_size: u64) -> FileReadResult {
    if is_binary_path(file_path) {
        return FileReadResult::skipped(FileSkipReason::BinaryExtension, 0);
    }

    let file = match File::open(file_path) {
        Ok(file) => file,
        Err(_) => return FileReadResult::skipped(FileSkipReason::EncodingError, 0),
    };
    let size_bytes = match file.metadata() {
        Ok(metadata) => metadata.len(),
        Err(_) => return FileReadResult::skipped(FileSkipReason::EncodingError, 0),
    };
    if size_bytes > max_file_size {
        return FileReadResult::skipped(
            FileSkipReason::SizeLimit,
            usize::try_from(size_bytes).unwrap_or(usize::MAX),
        );
    }
    if size_bytes == 0 {
        return process_file_bytes(&[]);
    }

    // SAFETY: the file stays open and is not mutated through this read-only
    // mapping; its borrowed bytes are consumed before both map and file drop.
    let mapped = match unsafe { MmapOptions::new().map(&file) } {
        Ok(mapped) => mapped,
        Err(_) => return FileReadResult::skipped(FileSkipReason::EncodingError, 0),
    };
    process_file_bytes(&mapped)
}

/// Reads every discovered file in parallel and keeps discovery order in both outputs.
pub fn collect_files(files: &[DiscoveredFile], config: &RepomixConfig) -> FileCollection {
    let max_file_size = config.input.max_file_size;
    let results: Vec<_> = files
        .par_iter()
        .map(|file| (file, read_raw_file(&file.absolute_path, max_file_size)))
        .collect();

    let mut collection = FileCollection::default();
    for (file, result) in results {
        match (result.content, result.skipped_reason) {
            (Some(content), None) => collection.raw_files.push(RawFile {
                path: file.relative_path.clone(),
                line_count: line_count(&content),
                content,
                size_bytes: result.size_bytes,
            }),
            (None, Some(reason)) => collection.skipped_files.push(SkippedFileInfo {
                path: file.relative_path.clone(),
                reason,
            }),
            _ => {}
        }
    }
    collection
}

fn line_count(content: &str) -> usize {
    if content.is_empty() {
        0
    } else if content.ends_with('\n') {
        content.bytes().filter(|byte| *byte == b'\n').count()
    } else {
        content.bytes().filter(|byte| *byte == b'\n').count() + 1
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use tempfile::tempdir;

    use crate::config::RepomixConfig;

    use super::{collect_files, read_raw_file, FileSkipReason};
    use crate::core::file::DiscoveredFile;

    fn discovered(root: &Path, relative_path: &str) -> DiscoveredFile {
        DiscoveredFile {
            absolute_path: root.join(relative_path),
            relative_path: PathBuf::from(relative_path),
        }
    }

    #[test]
    fn skips_an_oversized_file_before_mapping_it() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("large.txt");
        fs::write(&path, "too large").expect("fixture file");

        let result = read_raw_file(&path, 3);

        assert_eq!(result.skipped_reason, Some(FileSkipReason::SizeLimit));
    }

    #[test]
    fn collects_in_discovery_order_with_metadata_and_skip_reasons() {
        let directory = tempdir().expect("temporary directory");
        fs::write(directory.path().join("second.txt"), "two\r\nlines\r\n").expect("fixture file");
        fs::write(directory.path().join("first.txt"), "one\n").expect("fixture file");
        fs::write(directory.path().join("logo.png"), b"not read").expect("fixture file");
        let files = vec![
            discovered(directory.path(), "second.txt"),
            discovered(directory.path(), "logo.png"),
            discovered(directory.path(), "first.txt"),
        ];

        let collection = collect_files(&files, &RepomixConfig::default());

        assert_eq!(
            collection
                .raw_files
                .iter()
                .map(|file| file.path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            ["second.txt", "first.txt"]
        );
        assert_eq!(collection.raw_files[0].content, "two\r\nlines\r\n");
        assert_eq!(collection.raw_files[0].line_count, 2);
        assert_eq!(collection.raw_files[0].size_bytes, 12);
        assert_eq!(
            collection.skipped_files[0].reason,
            FileSkipReason::BinaryExtension
        );
    }

    #[test]
    fn processes_more_than_a_thousand_files_without_changing_order() {
        let directory = tempdir().expect("temporary directory");
        let files = (0..1_001)
            .map(|index| {
                let relative_path = format!("file-{index:04}.txt");
                fs::write(directory.path().join(&relative_path), format!("{index}\n"))
                    .expect("fixture file");
                discovered(directory.path(), &relative_path)
            })
            .collect::<Vec<_>>();

        let collection = collect_files(&files, &RepomixConfig::default());

        assert_eq!(collection.raw_files.len(), 1_001);
        assert_eq!(collection.raw_files[0].path, PathBuf::from("file-0000.txt"));
        assert_eq!(
            collection.raw_files[1_000].path,
            PathBuf::from("file-1000.txt")
        );
        assert!(collection.skipped_files.is_empty());
    }

    #[test]
    fn matches_typescript_line_count_rules_for_empty_and_unterminated_content() {
        assert_eq!(super::line_count(""), 0);
        assert_eq!(super::line_count("one"), 1);
        assert_eq!(super::line_count("one\ntwo"), 2);
        assert_eq!(super::line_count("one\ntwo\n"), 2);
    }
}
