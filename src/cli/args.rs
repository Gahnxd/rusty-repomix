//! `clap` argument definitions and their conversion into configuration overrides.
//!
//! Names and false-only override semantics are ported from
//! `repomix/src/cli/cliRun.ts` and `cli/actions/defaultAction.ts`.

use std::path::PathBuf;

use clap::{ArgAction, Parser};

use crate::config::schema::{
    FileConfig, OutputFilePathStyle, OutputStyle, PartialIgnoreConfig, PartialOutputConfig,
    PartialSecurityConfig, PartialTokenCountConfig, TokenEncoding,
};

/// CLI arguments accepted by the current Rust implementation.
#[derive(Debug, Parser)]
#[command(
    name = "rusty_repomix",
    about = "Pack a repository into an AI-friendly file"
)]
pub struct CliArgs {
    /// Directories to process. Defaults to the current directory.
    #[arg(value_name = "DIRECTORY", default_value = ".")]
    pub directories: Vec<PathBuf>,

    #[arg(short, long, value_name = "FILE")]
    pub output: Option<String>,

    /// Output format. `--style` is accepted for TypeScript Repomix compatibility.
    #[arg(long = "format", visible_alias = "style", value_enum)]
    pub format: Option<OutputStyle>,

    #[arg(long, value_name = "PATTERNS")]
    pub include: Option<String>,

    #[arg(short, long, value_name = "PATTERNS")]
    pub ignore: Option<String>,

    #[arg(long, action = ArgAction::SetTrue)]
    pub compress: bool,

    /// Explicitly enables security scanning; useful when overriding a config file.
    #[arg(long = "security-check", action = ArgAction::SetTrue, conflicts_with = "no_security_check")]
    pub security_check: bool,

    /// TypeScript Repomix compatibility flag that disables security scanning.
    #[arg(long = "no-security-check", action = ArgAction::SetTrue, conflicts_with = "security_check")]
    pub no_security_check: bool,

    #[arg(long, value_enum)]
    pub token_count_encoding: Option<TokenEncoding>,

    #[arg(long, value_name = "URL")]
    pub remote: Option<String>,

    #[arg(long, action = ArgAction::SetTrue)]
    pub init: bool,

    #[arg(long, action = ArgAction::SetTrue)]
    pub mcp: bool,

    #[arg(short = 'c', long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    #[arg(long, action = ArgAction::SetTrue)]
    pub stdout: bool,

    #[arg(long, action = ArgAction::SetTrue)]
    pub no_file_summary: bool,

    #[arg(long, action = ArgAction::SetTrue)]
    pub no_directory_structure: bool,

    #[arg(long, action = ArgAction::SetTrue)]
    pub no_files: bool,

    #[arg(long, action = ArgAction::SetTrue)]
    pub no_gitignore: bool,

    #[arg(long, action = ArgAction::SetTrue)]
    pub no_dot_ignore: bool,

    #[arg(long, action = ArgAction::SetTrue)]
    pub no_default_patterns: bool,

    #[arg(long, value_enum)]
    pub output_file_path_style: Option<OutputFilePathStyle>,

    #[arg(long, action = ArgAction::SetTrue)]
    pub output_show_line_numbers: bool,

    #[arg(long, action = ArgAction::SetTrue)]
    pub remove_comments: bool,

    #[arg(long, action = ArgAction::SetTrue)]
    pub remove_empty_lines: bool,

    #[arg(long, action = ArgAction::SetTrue)]
    pub truncate_base64: bool,

    #[arg(long, action = ArgAction::SetTrue)]
    pub copy: bool,

    #[arg(long, value_name = "NUMBER")]
    pub token_budget: Option<u64>,
}

/// Converts only explicitly supplied CLI values into mergeable config overrides.
pub fn build_cli_config(args: &CliArgs) -> FileConfig {
    let mut config = FileConfig::default();
    let mut output = PartialOutputConfig::default();
    let mut has_output_override = false;

    if let Some(file_path) = &args.output {
        output.file_path = Some(file_path.clone());
        has_output_override = true;
    }
    if let Some(style) = args.format {
        output.style = Some(style);
        has_output_override = true;
    }
    if let Some(style) = args.output_file_path_style {
        output.file_path_style = Some(style);
        has_output_override = true;
    }
    if args.compress {
        output.compress = Some(true);
        has_output_override = true;
    }
    if args.stdout {
        output.stdout = Some(true);
        has_output_override = true;
    }
    if args.no_file_summary {
        output.file_summary = Some(false);
        has_output_override = true;
    }
    if args.no_directory_structure {
        output.directory_structure = Some(false);
        has_output_override = true;
    }
    if args.no_files {
        output.files = Some(false);
        has_output_override = true;
    }
    if args.output_show_line_numbers {
        output.show_line_numbers = Some(true);
        has_output_override = true;
    }
    if args.remove_comments {
        output.remove_comments = Some(true);
        has_output_override = true;
    }
    if args.remove_empty_lines {
        output.remove_empty_lines = Some(true);
        has_output_override = true;
    }
    if args.truncate_base64 {
        output.truncate_base64 = Some(true);
        has_output_override = true;
    }
    if args.copy {
        output.copy_to_clipboard = Some(true);
        has_output_override = true;
    }
    if let Some(token_budget) = args.token_budget {
        output.token_budget = Some(token_budget);
        has_output_override = true;
    }
    if has_output_override {
        config.output = Some(output);
    }

    if let Some(patterns) = &args.include {
        config.include = Some(split_patterns(patterns));
    }
    if let Some(patterns) = &args.ignore {
        config.ignore = Some(PartialIgnoreConfig {
            custom_patterns: Some(split_patterns(patterns)),
            ..Default::default()
        });
    }
    if args.no_gitignore || args.no_dot_ignore || args.no_default_patterns {
        let ignore = config
            .ignore
            .get_or_insert_with(PartialIgnoreConfig::default);
        if args.no_gitignore {
            ignore.use_gitignore = Some(false);
        }
        if args.no_dot_ignore {
            ignore.use_dot_ignore = Some(false);
        }
        if args.no_default_patterns {
            ignore.use_default_patterns = Some(false);
        }
    }
    if args.security_check || args.no_security_check {
        config.security = Some(PartialSecurityConfig {
            enable_security_check: Some(!args.no_security_check),
        });
    }
    if let Some(encoding) = args.token_count_encoding {
        config.token_count = Some(PartialTokenCountConfig {
            encoding: Some(encoding),
        });
    }

    config
}

fn split_patterns(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|pattern| !pattern.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::config::schema::{OutputStyle, TokenEncoding};

    use super::{build_cli_config, CliArgs};

    #[test]
    fn parses_required_us_002_flags() {
        let args = CliArgs::try_parse_from([
            "rusty_repomix",
            "--output",
            "packed.xml",
            "--format",
            "markdown",
            "--ignore",
            "target, *.log",
            "--compress",
            "--security-check",
            "--token-count-encoding",
            "cl100k_base",
            "--remote",
            "https://example.test/repo",
            "--init",
            "--mcp",
        ])
        .expect("arguments should parse");

        assert_eq!(args.output.as_deref(), Some("packed.xml"));
        assert_eq!(args.format, Some(OutputStyle::Markdown));
        assert_eq!(args.token_count_encoding, Some(TokenEncoding::Cl100kBase));
        assert!(args.compress && args.security_check && args.init && args.mcp);
    }

    #[test]
    fn cli_config_keeps_unspecified_values_out_of_the_override() {
        let args = CliArgs::try_parse_from(["rusty_repomix"]).expect("arguments should parse");
        let config = build_cli_config(&args);

        assert!(config.output.is_none());
        assert!(config.security.is_none());
    }

    #[test]
    fn cli_config_trims_comma_separated_patterns_and_honors_false_flags() {
        let args = CliArgs::try_parse_from([
            "rusty_repomix",
            "--include",
            "src/**, tests/**",
            "--ignore",
            "target/**, *.log",
            "--no-gitignore",
            "--no-security-check",
        ])
        .expect("arguments should parse");
        let config = build_cli_config(&args);

        assert_eq!(
            config.include,
            Some(vec!["src/**".into(), "tests/**".into()])
        );
        assert_eq!(
            config.ignore.expect("ignore").custom_patterns,
            Some(vec!["target/**".into(), "*.log".into()])
        );
        assert_eq!(
            config.security.expect("security").enable_security_check,
            Some(false)
        );
    }
}
