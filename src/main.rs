use std::process::ExitCode;

use clap::Parser;
use rusty_repomix::{
    cli::build_cli_config, config::write_default_config, handle_error, init_tracing,
    load_file_config, merge_configs, CliArgs, RepomixError,
};

fn main() -> ExitCode {
    init_tracing();

    // CLI argument parsing and command dispatch are introduced in US-002.
    // Keeping the entrypoint intentionally small makes its error boundary
    // available to every future CLI action.
    if let Err(error) = run() {
        handle_error(&error);
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn run() -> rusty_repomix::Result<()> {
    let args = CliArgs::parse();
    let cwd = std::env::current_dir()
        .map_err(|error| RepomixError::io("determining the current directory", error))?;

    if args.init {
        write_default_config(&cwd)?;
        return Ok(());
    }

    let file_config = load_file_config(&cwd, args.config.as_deref())?;
    let config = merge_configs(cwd, file_config, build_cli_config(&args));
    tracing::debug!(?config, "Resolved Repomix configuration");

    // Packing actions are added by subsequent user stories.
    Ok(())
}
