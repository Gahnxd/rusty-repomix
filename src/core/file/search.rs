//! Git-aware, deterministic file discovery.
//!
//! The traversal settings port `repomix/src/core/file/fileSearch.ts`: dotfiles
//! are visible, symlinks are not followed, `.repomixignore` is always active,
//! `.ignore` is configurable, and Git ignore/global/exclude rules are enabled
//! only when requested. `WalkBuilder` implements hierarchical ignore-file
//! precedence while the Repomix built-ins and custom patterns are evaluated as
//! ordered gitignore-style rules.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};

use globset::{GlobBuilder, GlobMatcher};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::{Match, WalkBuilder};

use crate::config::RepomixConfig;
use crate::shared::error::{RepomixError, Result};

use super::search_constants::DEFAULT_IGNORE_PATTERNS;

/// A regular file located by the search pipeline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredFile {
    /// Absolute, non-symlink path suitable for reading.
    pub absolute_path: PathBuf,
    /// Path relative to the search root, using platform-native components.
    pub relative_path: PathBuf,
}

/// Files and empty directories discovered below a target root.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FileSearchResult {
    pub files: Vec<DiscoveredFile>,
    pub empty_dir_paths: Vec<PathBuf>,
}

enum SearchEntry {
    File(DiscoveredFile),
    Directory {
        absolute_path: PathBuf,
        relative_path: PathBuf,
    },
}

/// Finds files and, when requested, empty directories in deterministic Repomix order.
pub fn find_files(root_dir: &Path, config: &RepomixConfig) -> Result<FileSearchResult> {
    let metadata = fs::metadata(root_dir).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => {
            RepomixError::FileSearch(format!("Target path does not exist: {}", root_dir.display()))
        }
        std::io::ErrorKind::PermissionDenied => RepomixError::FileSearch(format!(
            "Permission denied while accessing path. Please check folder access permissions for your terminal app. path: {}",
            root_dir.display()
        )),
        _ => RepomixError::FileSearch(format!(
            "Failed to access path: {}. Reason: {error}",
            root_dir.display()
        )),
    })?;
    if !metadata.is_dir() {
        return Err(RepomixError::FileSearch(format!(
            "Target path is not a directory: {}. Please specify a directory path, not a file path.",
            root_dir.display()
        )));
    }

    let absolute_root = fs::canonicalize(root_dir).map_err(|error| {
        RepomixError::io(format!("canonicalizing {}", root_dir.display()), error)
    })?;
    let ignore_matcher = build_repomix_ignore_matcher(&absolute_root, config)?;
    let include_matchers = build_include_matchers(&config.include)?;

    let mut walker = WalkBuilder::new(&absolute_root);
    walker
        .hidden(false)
        .follow_links(false)
        .ignore(config.ignore.use_dot_ignore)
        .git_ignore(config.ignore.use_gitignore)
        .git_global(config.ignore.use_gitignore)
        .git_exclude(config.ignore.use_gitignore)
        .require_git(false)
        .add_custom_ignore_filename(".repomixignore");

    let (sender, receiver) = mpsc::channel();
    let ignore_matcher = Arc::new(ignore_matcher);
    let include_matchers = Arc::new(include_matchers);
    let absolute_root_arc = Arc::new(absolute_root.clone());
    let search_errors = Arc::new(Mutex::new(Vec::new()));
    let include_empty_directories = config.output.include_empty_directories.unwrap_or(false);

    walker.build_parallel().run(|| {
        let sender = sender.clone();
        let ignore_matcher = Arc::clone(&ignore_matcher);
        let include_matchers = Arc::clone(&include_matchers);
        let absolute_root = Arc::clone(&absolute_root_arc);
        let search_errors = Arc::clone(&search_errors);

        Box::new(move |entry: std::result::Result<ignore::DirEntry, ignore::Error>| -> ignore::WalkState {
            match entry {
                Ok(entry) => {
                    let file_type = entry.file_type();
                    let is_file = file_type.is_some_and(|kind| kind.is_file());
                    let is_directory = file_type.is_some_and(|kind| kind.is_dir());
                    if !is_file && (!include_empty_directories || !is_directory) {
                        return ignore::WalkState::Continue;
                    }

                    let absolute_path = entry.into_path();
                    let relative_path = match absolute_path.strip_prefix(absolute_root.as_path()) {
                        Ok(p) => p.to_path_buf(),
                        Err(error) => {
                            record_search_error(&search_errors, RepomixError::FileSearch(format!(
                                "Failed to make discovered path relative to {}: {error}",
                                absolute_root.display()
                            )));
                            return ignore::WalkState::Continue;
                        }
                    };

                    if is_ignored(&relative_path, &ignore_matcher)
                        || !is_included(&relative_path, &include_matchers)
                    {
                        return ignore::WalkState::Continue;
                    }

                    if is_file {
                        let _ = sender.send(SearchEntry::File(DiscoveredFile {
                            absolute_path,
                            relative_path,
                        }));
                    } else if !relative_path.as_os_str().is_empty() {
                        let _ = sender.send(SearchEntry::Directory {
                            absolute_path,
                            relative_path,
                        });
                    }
                    ignore::WalkState::Continue
                }
                Err(error) => {
                    record_search_error(&search_errors, RepomixError::FileSearch(format!(
                        "Failed to filter files in directory {}. Reason: {error}",
                        absolute_root.display()
                    )));
                    ignore::WalkState::Continue
                }
            }
        })
    });
    drop(sender);

    let mut files = Vec::new();
    let mut directories = Vec::new();
    for entry in receiver {
        match entry {
            SearchEntry::File(file) => files.push(file),
            SearchEntry::Directory {
                absolute_path,
                relative_path,
            } => directories.push((absolute_path, relative_path)),
        }
    }
    if let Some(error) = take_deterministic_search_error(&search_errors) {
        return Err(error);
    }

    files.sort_by(|left, right| compare_relative_paths(&left.relative_path, &right.relative_path));
    let mut empty_dir_paths = directories
        .into_iter()
        .filter_map(|(absolute_path, relative_path)| {
            is_empty_directory(&absolute_path).then_some(relative_path)
        })
        .collect::<Vec<_>>();
    empty_dir_paths.sort_by(|left, right| compare_relative_paths(left, right));

    Ok(FileSearchResult {
        files,
        empty_dir_paths,
    })
}

fn record_search_error(errors: &Mutex<Vec<RepomixError>>, error: RepomixError) {
    let mut guard = match errors.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.push(error);
}

fn take_deterministic_search_error(errors: &Mutex<Vec<RepomixError>>) -> Option<RepomixError> {
    let mut guard = match errors.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.sort_by_key(ToString::to_string);
    (!guard.is_empty()).then(|| guard.remove(0))
}

fn is_empty_directory(path: &Path) -> bool {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return false,
    };
    !entries
        .flatten()
        .any(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
}

fn build_repomix_ignore_matcher(root: &Path, config: &RepomixConfig) -> Result<Gitignore> {
    let mut builder = GitignoreBuilder::new(root);
    if config.ignore.use_default_patterns {
        for pattern in DEFAULT_IGNORE_PATTERNS {
            add_ignore_pattern(&mut builder, pattern, root)?;
        }
    }

    let output_path = resolve_output_path(root, config);
    if let Ok(relative) = output_path.strip_prefix(root) {
        add_ignore_pattern(&mut builder, &to_posix_path(relative), root)?;
    }
    for pattern in &config.ignore.custom_patterns {
        add_ignore_pattern(&mut builder, pattern, root)?;
    }

    builder.build().map_err(|error| {
        RepomixError::FileSearch(format!(
            "Invalid ignore pattern for {}: {error}",
            root.display()
        ))
    })
}

fn add_ignore_pattern(builder: &mut GitignoreBuilder, pattern: &str, root: &Path) -> Result<()> {
    builder
        .add_line(None, pattern)
        .map(|_| ())
        .map_err(|error| {
            RepomixError::FileSearch(format!(
                "Invalid ignore pattern '{pattern}' for {}: {error}",
                root.display()
            ))
        })
}

fn resolve_output_path(root: &Path, config: &RepomixConfig) -> PathBuf {
    let output = Path::new(&config.output.file_path);
    if output.is_absolute() {
        output.to_path_buf()
    } else if config.cwd.is_absolute() {
        config.cwd.join(output)
    } else {
        root.join(output)
    }
}

fn is_ignored(relative_path: &Path, matcher: &Gitignore) -> bool {
    matches!(
        matcher.matched_path_or_any_parents(relative_path, false),
        Match::Ignore(_)
    )
}

#[derive(Debug)]
struct IncludeMatcher {
    includes: bool,
    matcher: GlobMatcher,
}

fn build_include_matchers(patterns: &[String]) -> Result<Vec<IncludeMatcher>> {
    patterns
        .iter()
        .filter(|pattern| !pattern.trim().is_empty())
        .map(|raw_pattern| {
            let (includes, pattern) = match raw_pattern.strip_prefix('!') {
                Some(pattern) => (false, pattern),
                None => (true, raw_pattern.as_str()),
            };
            let normalized_pattern = pattern.replace('\\', "/");
            let mut builder = GlobBuilder::new(&normalized_pattern);
            builder.literal_separator(false);
            let glob = builder.build().map_err(|error| {
                RepomixError::FileSearch(format!(
                    "Invalid include pattern '{raw_pattern}': {error}"
                ))
            })?;
            Ok(IncludeMatcher {
                includes,
                matcher: glob.compile_matcher(),
            })
        })
        .collect()
}

fn is_included(relative_path: &Path, matchers: &[IncludeMatcher]) -> bool {
    if matchers.is_empty() {
        return true;
    }
    let path = to_posix_path(relative_path);
    let mut included = false;
    for rule in matchers {
        if rule.matcher.is_match(&path) {
            included = rule.includes;
        }
    }
    included
}

fn to_posix_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn compare_relative_paths(left: &Path, right: &Path) -> std::cmp::Ordering {
    let left_parts: Vec<_> = to_posix_path(left).split('/').map(str::to_owned).collect();
    let right_parts: Vec<_> = to_posix_path(right).split('/').map(str::to_owned).collect();
    for index in 0..left_parts.len().min(right_parts.len()) {
        if left_parts[index] != right_parts[index] {
            let left_is_directory = index + 1 < left_parts.len();
            let right_is_directory = index + 1 < right_parts.len();
            if left_is_directory != right_is_directory {
                return if left_is_directory {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                };
            }
            let insensitive = left_parts[index]
                .to_lowercase()
                .cmp(&right_parts[index].to_lowercase());
            return if insensitive.is_eq() {
                left_parts[index].cmp(&right_parts[index])
            } else {
                insensitive
            };
        }
    }
    left_parts.len().cmp(&right_parts.len())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use tempfile::tempdir;

    use crate::config::RepomixConfig;

    use super::find_files;

    fn write_fixture(root: &Path, path: &str, contents: &str) {
        let full_path = root.join(path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).expect("fixture parent");
        }
        fs::write(full_path, contents).expect("fixture file");
    }

    fn relative_paths(result: &super::FileSearchResult) -> Vec<String> {
        result
            .files
            .iter()
            .map(|file| file.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    fn config_for(root: &Path) -> RepomixConfig {
        RepomixConfig {
            cwd: root.to_path_buf(),
            ..RepomixConfig::default()
        }
    }

    #[test]
    fn honors_nested_gitignore_and_negation_rules() {
        let directory = tempdir().expect("temporary directory");
        write_fixture(directory.path(), ".gitignore", "*.draft\n!keep.draft\n");
        write_fixture(directory.path(), "noisy.draft", "ignored");
        write_fixture(directory.path(), "keep.draft", "included");
        write_fixture(directory.path(), "pkg/.gitignore", "generated/\n");
        write_fixture(directory.path(), "pkg/generated/build.data", "ignored");
        write_fixture(directory.path(), "pkg/source.rs", "included");

        let paths = relative_paths(
            &find_files(directory.path(), &config_for(directory.path())).expect("search files"),
        );
        assert!(paths.contains(&"keep.draft".into()));
        assert!(paths.contains(&"pkg/source.rs".into()));
        assert!(!paths.contains(&"noisy.draft".into()));
        assert!(!paths.contains(&"pkg/generated/build.data".into()));
    }

    #[test]
    fn applies_dot_and_repomix_ignore_files_with_the_correct_toggles() {
        let directory = tempdir().expect("temporary directory");
        write_fixture(directory.path(), ".ignore", "*.draft\n");
        write_fixture(directory.path(), ".repomixignore", "*.secret\n");
        write_fixture(directory.path(), "visible.rs", "visible");
        write_fixture(directory.path(), "ignored.draft", "draft");
        write_fixture(directory.path(), "ignored.secret", "secret");

        let mut config = config_for(directory.path());
        config.ignore.use_default_patterns = false;
        let paths = relative_paths(&find_files(directory.path(), &config).expect("search files"));
        assert!(!paths.contains(&"ignored.draft".into()));
        assert!(!paths.contains(&"ignored.secret".into()));

        config.ignore.use_dot_ignore = false;
        let paths = relative_paths(&find_files(directory.path(), &config).expect("search files"));
        assert!(paths.contains(&"ignored.draft".into()));
        assert!(!paths.contains(&"ignored.secret".into()));
    }

    #[test]
    fn custom_negative_patterns_can_reinclude_default_ignored_files() {
        let directory = tempdir().expect("temporary directory");
        write_fixture(directory.path(), "target/drop.rs", "ignored");
        write_fixture(directory.path(), "target/keep.rs", "included");
        let mut config = config_for(directory.path());
        config.ignore.custom_patterns = vec!["!target/keep.rs".into()];

        let paths = relative_paths(&find_files(directory.path(), &config).expect("search files"));
        assert!(paths.contains(&"target/keep.rs".into()));
        assert!(!paths.contains(&"target/drop.rs".into()));
    }

    #[test]
    fn honors_git_info_exclude_when_gitignore_support_is_enabled() {
        let directory = tempdir().expect("temporary directory");
        write_fixture(directory.path(), ".git/info/exclude", "private.data\n");
        write_fixture(directory.path(), "private.data", "ignored");
        write_fixture(directory.path(), "public.data", "included");
        let mut config = config_for(directory.path());
        config.ignore.use_default_patterns = false;

        let paths = relative_paths(&find_files(directory.path(), &config).expect("search files"));
        assert!(paths.contains(&"public.data".into()));
        assert!(!paths.contains(&"private.data".into()));
    }

    #[test]
    fn applies_include_ordering_and_returns_deterministic_relative_and_absolute_paths() {
        let directory = tempdir().expect("temporary directory");
        write_fixture(directory.path(), "src/z.rs", "z");
        write_fixture(directory.path(), "src/a.rs", "a");
        write_fixture(directory.path(), "tests/skip.rs", "skip");
        write_fixture(directory.path(), "README.md", "readme");
        let mut config = config_for(directory.path());
        config.ignore.use_default_patterns = false;
        config.include = vec!["**/*.rs".into(), "!tests/**".into()];

        let files = find_files(directory.path(), &config).expect("search files");
        assert_eq!(relative_paths(&files), vec!["src/a.rs", "src/z.rs"]);
        assert!(files
            .files
            .iter()
            .all(|file| file.absolute_path.is_absolute()));
        assert!(files
            .files
            .iter()
            .all(|file| file.absolute_path.ends_with(&file.relative_path)));
    }

    #[test]
    fn reports_missing_and_non_directory_targets_like_the_typescript_searcher() {
        let directory = tempdir().expect("temporary directory");
        let missing = directory.path().join("missing");
        let error =
            find_files(&missing, &config_for(directory.path())).expect_err("missing target");
        assert_eq!(
            error.to_string(),
            format!("Target path does not exist: {}", missing.display())
        );

        let file = directory.path().join("file.rs");
        fs::write(&file, "source").expect("fixture file");
        let error = find_files(&file, &config_for(directory.path())).expect_err("file target");
        assert_eq!(error.to_string(), format!("Target path is not a directory: {}. Please specify a directory path, not a file path.", file.display()));
    }

    #[cfg(unix)]
    #[test]
    fn does_not_follow_symbolic_links() {
        use std::os::unix::fs::symlink;

        let directory = tempdir().expect("temporary directory");
        write_fixture(directory.path(), "real/source.rs", "source");
        symlink(
            directory.path().join("real"),
            directory.path().join("linked"),
        )
        .expect("symlink");

        let paths = relative_paths(
            &find_files(directory.path(), &config_for(directory.path())).expect("search files"),
        );
        assert!(paths.contains(&"real/source.rs".into()));
        assert!(!paths.iter().any(|path| path.starts_with("linked/")));
    }

    #[test]
    fn exposes_workspace_relative_paths() {
        let relative = PathBuf::from("nested/file.rs");
        assert_eq!(relative.to_string_lossy(), "nested/file.rs");
    }

    #[test]
    fn returns_empty_directories_when_enabled_with_typescript_compatible_rules() {
        let directory = tempdir().expect("temporary directory");
        fs::create_dir(directory.path().join("empty")).expect("empty directory");
        write_fixture(directory.path(), ".gitignore", "ignored_content/\n");
        write_fixture(directory.path(), "dot_only/.keep", "hidden");
        write_fixture(directory.path(), "non_empty/source.rs", "source");
        write_fixture(directory.path(), "ignored_content/ignored.rs", "ignored");
        let mut config = config_for(directory.path());
        config.ignore.use_default_patterns = false;
        config.output.include_empty_directories = Some(true);

        let result = find_files(directory.path(), &config).expect("search files");
        let empty_directories = result
            .empty_dir_paths
            .iter()
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .collect::<Vec<_>>();
        assert!(empty_directories.contains(&"dot_only".into()));
        assert!(empty_directories.contains(&"empty".into()));
        assert!(!empty_directories.contains(&"non_empty".into()));
        assert!(!empty_directories.contains(&"ignored_content".into()));
    }
}
