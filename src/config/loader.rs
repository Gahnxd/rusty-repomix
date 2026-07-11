//! Multi-format configuration discovery, loading, and precedence-aware merging.
//!
//! Discovery and merge behavior follow `repomix/src/config/configLoad.ts`.
//! TOML and YAML extend the reference loader because they are explicit US-002
//! requirements.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::config::schema::{
    FileConfig, GitConfig, IgnoreConfig, OutputConfig, PartialGitConfig, PartialIgnoreConfig,
    PartialOutputConfig, RepomixConfig,
};
use crate::shared::error::{RepomixError, Result};

const CONFIG_FILE_NAMES: &[&str] = &[
    "repomix.config.json5",
    "repomix.config.jsonc",
    "repomix.config.json",
    "repomix.config.toml",
    "repomix.config.yaml",
    "repomix.config.yml",
];

/// Loads an explicit config file, or finds the first config file in priority order.
pub fn load_file_config(root_dir: &Path, explicit_path: Option<&Path>) -> Result<FileConfig> {
    if let Some(config_path) = explicit_path {
        let full_path = if config_path.is_absolute() {
            config_path.to_path_buf()
        } else {
            root_dir.join(config_path)
        };
        if !is_file(&full_path) {
            return Err(RepomixError::Config(format!(
                "Config file not found at {}",
                config_path.display()
            )));
        }
        return load_and_validate_config(&full_path);
    }

    if let Some(path) = find_config_file(root_dir) {
        return load_and_validate_config(&path);
    }

    if let Some(path) = find_config_file(&global_config_directory()) {
        return load_and_validate_config(&path);
    }

    Ok(FileConfig::default())
}

/// Creates the non-interactive default `repomix.config.json` file.
///
/// The TypeScript `--init` flow asks interactive overwrite questions. The Rust
/// CLI is deliberately safe in non-interactive contexts: an existing file is
/// never overwritten and returns a clear configuration error instead.
pub fn write_default_config(root_dir: &Path) -> Result<PathBuf> {
    let config_path = root_dir.join("repomix.config.json");
    let config = RepomixConfig {
        schema_url: Some("https://repomix.com/schemas/latest/schema.json".into()),
        ..RepomixConfig::default()
    };

    let contents = serde_json::to_string_pretty(&config).map_err(|error| {
        RepomixError::Config(format!("Unable to serialize default config: {error}"))
    })?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&config_path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                RepomixError::Config(format!(
                    "Config file already exists at {}",
                    config_path.display()
                ))
            } else {
                RepomixError::io(format!("writing {}", config_path.display()), error)
            }
        })?;
    file.write_all(format!("{contents}\n").as_bytes())
        .map_err(|error| RepomixError::io(format!("writing {}", config_path.display()), error))?;

    Ok(config_path)
}

/// Merges defaults, file settings, and CLI settings in that precedence order.
pub fn merge_configs(
    cwd: PathBuf,
    file_config: FileConfig,
    cli_config: FileConfig,
) -> RepomixConfig {
    let file_path_explicit = file_config
        .output
        .as_ref()
        .and_then(|output| output.file_path.as_ref())
        .is_some()
        || cli_config
            .output
            .as_ref()
            .and_then(|output| output.file_path.as_ref())
            .is_some();

    let mut merged = RepomixConfig {
        cwd,
        schema_url: file_config.schema_url.clone(),
        ..RepomixConfig::default()
    };
    apply_config(&mut merged, &file_config, false);
    apply_config(&mut merged, &cli_config, true);

    // The reference loader changes only implicit output filenames when style
    // changes, preserving any explicit file path from file config or the CLI.
    if !file_path_explicit {
        merged.output.file_path = merged.output.style.default_file_path().into();
    }

    merged
}

fn find_config_file(root_dir: &Path) -> Option<PathBuf> {
    CONFIG_FILE_NAMES
        .iter()
        .map(|name| root_dir.join(name))
        .find(|candidate| is_file(candidate))
}

fn is_file(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

fn global_config_directory() -> PathBuf {
    if cfg!(windows) {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("USERPROFILE")
                    .map(|home| PathBuf::from(home).join("AppData/Local"))
            })
            .unwrap_or_else(|| PathBuf::from("."));
        return base.join("Repomix");
    }

    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join(".config"))
                .unwrap_or_else(|| PathBuf::from(".config"))
        })
        .join("repomix")
}

fn load_and_validate_config(path: &Path) -> Result<FileConfig> {
    let contents = fs::read_to_string(path).map_err(|error| {
        RepomixError::io(format!("reading config file {}", path.display()), error)
    })?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    let result = match extension {
        "json" | "jsonc" | "json5" => json5::from_str(&contents).map_err(|error| {
            RepomixError::Config(format!(
                "Invalid syntax in config file {}: {error}",
                path.display()
            ))
        }),
        "toml" => toml::from_str(&contents).map_err(|error| {
            RepomixError::Config(format!(
                "Invalid syntax in config file {}: {error}",
                path.display()
            ))
        }),
        "yaml" | "yml" => serde_yaml::from_str(&contents).map_err(|error| {
            RepomixError::Config(format!(
                "Invalid syntax in config file {}: {error}",
                path.display()
            ))
        }),
        _ => {
            return Err(RepomixError::Config(format!(
                "Unsupported config file format: {}",
                path.display()
            )))
        }
    };

    let config = result.map_err(|error| {
        RepomixError::Config(format!(
            "Invalid config schema in {}: {error}",
            path.display()
        ))
    })?;
    validate_config(&config, path)?;
    Ok(config)
}

fn validate_config(config: &FileConfig, path: &Path) -> Result<()> {
    let invalid = |field: &str| {
        RepomixError::Config(format!(
            "Invalid config schema in {}: {field} must be a positive integer",
            path.display()
        ))
    };

    if config.input.as_ref().and_then(|input| input.max_file_size) == Some(0) {
        return Err(invalid("input.maxFileSize"));
    }
    if let Some(output) = &config.output {
        if output.split_output == Some(0) {
            return Err(invalid("output.splitOutput"));
        }
        if output.token_budget == Some(0) {
            return Err(invalid("output.tokenBudget"));
        }
        if let Some(git) = &output.git {
            if git.sort_by_changes_max_commits == Some(0) {
                return Err(invalid("output.git.sortByChangesMaxCommits"));
            }
            if git.include_logs_count == Some(0) {
                return Err(invalid("output.git.includeLogsCount"));
            }
        }
    }
    Ok(())
}

fn apply_config(target: &mut RepomixConfig, source: &FileConfig, is_cli: bool) {
    if let Some(input) = &source.input {
        if let Some(max_file_size) = input.max_file_size {
            target.input.max_file_size = max_file_size;
        }
    }
    if let Some(output) = &source.output {
        apply_output(&mut target.output, output);
    }
    if let Some(include) = &source.include {
        target.include.extend(include.iter().cloned());
    }
    if let Some(ignore) = &source.ignore {
        apply_ignore(&mut target.ignore, ignore);
    }
    if let Some(security) = &source.security {
        if let Some(enable_security_check) = security.enable_security_check {
            target.security.enable_security_check = enable_security_check;
        }
    }
    if let Some(token_count) = &source.token_count {
        if let Some(encoding) = token_count.encoding {
            target.token_count.encoding = encoding;
        }
    }
    if is_cli {
        target.skill_generate = source.skill_generate.clone();
    }
}

fn apply_output(target: &mut OutputConfig, source: &PartialOutputConfig) {
    if let Some(file_path) = &source.file_path {
        target.file_path.clone_from(file_path);
    }
    if let Some(style) = source.style {
        target.style = style;
    }
    if let Some(file_path_style) = source.file_path_style {
        target.file_path_style = file_path_style;
    }
    if let Some(parsable_style) = source.parsable_style {
        target.parsable_style = parsable_style;
    }
    if source.header_text.is_some() {
        target.header_text.clone_from(&source.header_text);
    }
    if source.instruction_file_path.is_some() {
        target
            .instruction_file_path
            .clone_from(&source.instruction_file_path);
    }
    if let Some(file_summary) = source.file_summary {
        target.file_summary = file_summary;
    }
    if let Some(directory_structure) = source.directory_structure {
        target.directory_structure = directory_structure;
    }
    if let Some(files) = source.files {
        target.files = files;
    }
    if let Some(remove_comments) = source.remove_comments {
        target.remove_comments = remove_comments;
    }
    if let Some(remove_empty_lines) = source.remove_empty_lines {
        target.remove_empty_lines = remove_empty_lines;
    }
    if let Some(compress) = source.compress {
        target.compress = compress;
    }
    if source.patterns.is_some() {
        target.patterns.clone_from(&source.patterns);
    }
    if let Some(top_files_length) = source.top_files_length {
        target.top_files_length = top_files_length;
    }
    if let Some(show_line_numbers) = source.show_line_numbers {
        target.show_line_numbers = show_line_numbers;
    }
    if let Some(truncate_base64) = source.truncate_base64 {
        target.truncate_base64 = truncate_base64;
    }
    if let Some(copy_to_clipboard) = source.copy_to_clipboard {
        target.copy_to_clipboard = copy_to_clipboard;
    }
    if source.include_empty_directories.is_some() {
        target.include_empty_directories = source.include_empty_directories;
    }
    if let Some(include_full_directory_structure) = source.include_full_directory_structure {
        target.include_full_directory_structure = include_full_directory_structure;
    }
    if source.split_output.is_some() {
        target.split_output = source.split_output;
    }
    if let Some(token_count_tree) = &source.token_count_tree {
        target.token_count_tree.clone_from(token_count_tree);
    }
    if source.token_budget.is_some() {
        target.token_budget = source.token_budget;
    }
    if let Some(stdout) = source.stdout {
        target.stdout = stdout;
    }
    if let Some(git) = &source.git {
        apply_git(&mut target.git, git);
    }
}

fn apply_git(target: &mut GitConfig, source: &PartialGitConfig) {
    if let Some(sort_by_changes) = source.sort_by_changes {
        target.sort_by_changes = sort_by_changes;
    }
    if let Some(sort_by_changes_max_commits) = source.sort_by_changes_max_commits {
        target.sort_by_changes_max_commits = sort_by_changes_max_commits;
    }
    if let Some(include_diffs) = source.include_diffs {
        target.include_diffs = include_diffs;
    }
    if let Some(include_logs) = source.include_logs {
        target.include_logs = include_logs;
    }
    if let Some(include_logs_count) = source.include_logs_count {
        target.include_logs_count = include_logs_count;
    }
}

fn apply_ignore(target: &mut IgnoreConfig, source: &PartialIgnoreConfig) {
    if let Some(use_gitignore) = source.use_gitignore {
        target.use_gitignore = use_gitignore;
    }
    if let Some(use_dot_ignore) = source.use_dot_ignore {
        target.use_dot_ignore = use_dot_ignore;
    }
    if let Some(use_default_patterns) = source.use_default_patterns {
        target.use_default_patterns = use_default_patterns;
    }
    if let Some(custom_patterns) = &source.custom_patterns {
        target
            .custom_patterns
            .extend(custom_patterns.iter().cloned());
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::config::schema::{FileConfig, OutputStyle, PartialOutputConfig};

    use super::{load_file_config, merge_configs, write_default_config};

    #[test]
    fn loads_json5_toml_and_yaml_config_files() {
        let directory = tempdir().expect("temporary directory");
        let root = directory.path();
        fs::write(
            root.join("repomix.config.json5"),
            "{ output: { style: 'markdown' } }",
        )
        .expect("json5 fixture");
        assert_eq!(
            load_file_config(root, None)
                .expect("load json5")
                .output
                .expect("output")
                .style,
            Some(OutputStyle::Markdown)
        );

        fs::remove_file(root.join("repomix.config.json5")).expect("remove json5 fixture");
        fs::write(
            root.join("repomix.config.toml"),
            "[output]\nstyle = 'plain'\n",
        )
        .expect("toml fixture");
        assert_eq!(
            load_file_config(root, None)
                .expect("load toml")
                .output
                .expect("output")
                .style,
            Some(OutputStyle::Plain)
        );

        fs::remove_file(root.join("repomix.config.toml")).expect("remove toml fixture");
        fs::write(root.join("repomix.config.yaml"), "output:\n  style: json\n")
            .expect("yaml fixture");
        assert_eq!(
            load_file_config(root, None)
                .expect("load yaml")
                .output
                .expect("output")
                .style,
            Some(OutputStyle::Json)
        );
    }

    #[test]
    fn json5_has_priority_over_json() {
        let directory = tempdir().expect("temporary directory");
        fs::write(
            directory.path().join("repomix.config.json"),
            r#"{ "output": { "style": "plain" } }"#,
        )
        .expect("json fixture");
        fs::write(
            directory.path().join("repomix.config.json5"),
            "{ output: { style: 'markdown' } }",
        )
        .expect("json5 fixture");

        let config = load_file_config(directory.path(), None).expect("load config");
        assert_eq!(
            config.output.expect("output").style,
            Some(OutputStyle::Markdown)
        );
    }

    #[test]
    fn cli_values_override_file_values_and_patterns_append() {
        let file = FileConfig {
            output: Some(PartialOutputConfig {
                style: Some(OutputStyle::Plain),
                ..Default::default()
            }),
            include: Some(vec!["src/**".into()]),
            ..Default::default()
        };
        let cli = FileConfig {
            output: Some(PartialOutputConfig {
                style: Some(OutputStyle::Markdown),
                ..Default::default()
            }),
            include: Some(vec!["tests/**".into()]),
            ..Default::default()
        };

        let merged = merge_configs(std::path::PathBuf::from("/project"), file, cli);
        assert_eq!(merged.output.style, OutputStyle::Markdown);
        assert_eq!(merged.output.file_path, "repomix-output.md");
        assert_eq!(merged.include, vec!["src/**", "tests/**"]);
    }

    #[test]
    fn explicit_output_path_is_not_rewritten_when_style_changes() {
        let file = FileConfig {
            output: Some(PartialOutputConfig {
                file_path: Some("chosen.any".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let cli = FileConfig {
            output: Some(PartialOutputConfig {
                style: Some(OutputStyle::Markdown),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_eq!(
            merge_configs(std::path::PathBuf::from("/project"), file, cli)
                .output
                .file_path,
            "chosen.any"
        );
    }

    #[test]
    fn init_writes_a_clean_default_json_config() {
        let directory = tempdir().expect("temporary directory");
        let path = write_default_config(directory.path()).expect("create default config");
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(path).expect("read config"))
                .expect("valid json");

        assert_eq!(
            value["$schema"],
            "https://repomix.com/schemas/latest/schema.json"
        );
        assert_eq!(value["output"]["style"], "xml");
        assert_eq!(value["output"]["filePath"], "repomix-output.xml");
        assert!(value.get("cwd").is_none());
    }

    #[test]
    fn validates_positive_integer_constraints_from_the_typescript_schema() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("invalid.toml");
        fs::write(&path, "[output]\nsplitOutput = 0\n").expect("invalid fixture");

        let error = load_file_config(directory.path(), Some(path.as_path()))
            .expect_err("zero must be rejected");
        assert!(error
            .to_string()
            .contains("output.splitOutput must be a positive integer"));
    }
}
