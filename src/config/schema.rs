//! Serializable configuration types and TypeScript-compatible defaults.
//!
//! Defaults and field names are ported from
//! `repomix/src/config/configSchema.ts`. `FileConfig` deliberately retains
//! optional fields so that loading a config file cannot accidentally override
//! a value supplied by a later CLI flag.

use std::path::PathBuf;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// Output formats supported by Repomix.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum OutputStyle {
    #[default]
    Xml,
    Markdown,
    Json,
    Plain,
}

impl OutputStyle {
    pub const fn default_file_path(self) -> &'static str {
        match self {
            Self::Xml => "repomix-output.xml",
            Self::Markdown => "repomix-output.md",
            Self::Json => "repomix-output.json",
            Self::Plain => "repomix-output.txt",
        }
    }
}

/// Controls whether output file paths are relative to the target or CWD.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
pub enum OutputFilePathStyle {
    #[serde(rename = "target-relative")]
    #[default]
    TargetRelative,
    #[serde(rename = "cwd-relative")]
    CwdRelative,
}

/// OpenAI-compatible token encodings accepted by the TypeScript schema.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
pub enum TokenEncoding {
    #[serde(rename = "o200k_base")]
    #[value(name = "o200k_base")]
    #[default]
    O200kBase,
    #[serde(rename = "cl100k_base")]
    #[value(name = "cl100k_base")]
    Cl100kBase,
    #[serde(rename = "p50k_base")]
    #[value(name = "p50k_base")]
    P50kBase,
    #[serde(rename = "p50k_edit")]
    #[value(name = "p50k_edit")]
    P50kEdit,
    #[serde(rename = "r50k_base")]
    #[value(name = "r50k_base")]
    R50kBase,
}

/// The flexible token-count-tree value accepted by Repomix configuration.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged)]
pub enum TokenCountTree {
    Enabled(bool),
    Threshold(u64),
    ThresholdText(String),
}

/// A file-pattern-specific output override.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputPattern {
    pub pattern: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compress: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory_structure_only: Option<bool>,
}

/// Resolved git-output settings.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitConfig {
    pub sort_by_changes: bool,
    pub sort_by_changes_max_commits: u64,
    pub include_diffs: bool,
    pub include_logs: bool,
    pub include_logs_count: u64,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            sort_by_changes: true,
            sort_by_changes_max_commits: 100,
            include_diffs: false,
            include_logs: false,
            include_logs_count: 50,
        }
    }
}

/// Resolved output settings.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputConfig {
    pub file_path: String,
    pub style: OutputStyle,
    pub file_path_style: OutputFilePathStyle,
    pub parsable_style: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instruction_file_path: Option<String>,
    pub file_summary: bool,
    pub directory_structure: bool,
    pub files: bool,
    pub remove_comments: bool,
    pub remove_empty_lines: bool,
    pub compress: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patterns: Option<Vec<OutputPattern>>,
    pub top_files_length: u64,
    pub show_line_numbers: bool,
    pub truncate_base64: bool,
    pub copy_to_clipboard: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_empty_directories: Option<bool>,
    pub include_full_directory_structure: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split_output: Option<u64>,
    pub token_count_tree: TokenCountTree,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_budget: Option<u64>,
    #[serde(skip_serializing)]
    pub stdout: bool,
    pub git: GitConfig,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            file_path: OutputStyle::Xml.default_file_path().into(),
            style: OutputStyle::Xml,
            file_path_style: OutputFilePathStyle::TargetRelative,
            parsable_style: false,
            header_text: None,
            instruction_file_path: None,
            file_summary: true,
            directory_structure: true,
            files: true,
            remove_comments: false,
            remove_empty_lines: false,
            compress: false,
            patterns: None,
            top_files_length: 5,
            show_line_numbers: false,
            truncate_base64: false,
            copy_to_clipboard: false,
            include_empty_directories: None,
            include_full_directory_structure: false,
            split_output: None,
            token_count_tree: TokenCountTree::Enabled(false),
            token_budget: None,
            stdout: false,
            git: GitConfig::default(),
        }
    }
}

/// Resolved ignore settings.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IgnoreConfig {
    pub use_gitignore: bool,
    pub use_dot_ignore: bool,
    pub use_default_patterns: bool,
    pub custom_patterns: Vec<String>,
}

impl Default for IgnoreConfig {
    fn default() -> Self {
        Self {
            use_gitignore: true,
            use_dot_ignore: true,
            use_default_patterns: true,
            custom_patterns: Vec::new(),
        }
    }
}

/// Resolved security settings.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityConfig {
    pub enable_security_check: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_security_check: true,
        }
    }
}

/// Resolved input settings.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputConfig {
    pub max_file_size: u64,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            max_file_size: 50 * 1024 * 1024,
        }
    }
}

/// Resolved token-count settings.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenCountConfig {
    pub encoding: TokenEncoding,
}

impl Default for TokenCountConfig {
    fn default() -> Self {
        Self {
            encoding: TokenEncoding::O200kBase,
        }
    }
}

/// Complete configuration used by the packer after defaults and overrides merge.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepomixConfig {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema_url: Option<String>,
    #[serde(skip_serializing)]
    pub cwd: PathBuf,
    pub input: InputConfig,
    pub output: OutputConfig,
    pub include: Vec<String>,
    pub ignore: IgnoreConfig,
    pub security: SecurityConfig,
    pub token_count: TokenCountConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_generate: Option<SkillGenerate>,
}

impl Default for RepomixConfig {
    fn default() -> Self {
        Self {
            schema_url: None,
            cwd: PathBuf::from("."),
            input: InputConfig::default(),
            output: OutputConfig::default(),
            include: Vec::new(),
            ignore: IgnoreConfig::default(),
            security: SecurityConfig::default(),
            token_count: TokenCountConfig::default(),
            skill_generate: None,
        }
    }
}

/// A CLI-only skill-generation option.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged)]
pub enum SkillGenerate {
    Name(String),
    Enabled(bool),
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialInputConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_file_size: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialGitConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by_changes: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by_changes_max_commits: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_diffs: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_logs: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_logs_count: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialOutputConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<OutputStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path_style: Option<OutputFilePathStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parsable_style: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instruction_file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_summary: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory_structure: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remove_comments: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remove_empty_lines: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compress: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patterns: Option<Vec<OutputPattern>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_files_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_line_numbers: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncate_base64: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copy_to_clipboard: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_empty_directories: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_full_directory_structure: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split_output: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count_tree: Option<TokenCountTree>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_budget: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<PartialGitConfig>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialIgnoreConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_gitignore: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_dot_ignore: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_default_patterns: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_patterns: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialSecurityConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_security_check: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialTokenCountConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<TokenEncoding>,
}

/// Configuration exactly as supplied by a file or CLI arguments.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileConfig {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<PartialInputConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<PartialOutputConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore: Option<PartialIgnoreConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<PartialSecurityConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<PartialTokenCountConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_generate: Option<SkillGenerate>,
}
