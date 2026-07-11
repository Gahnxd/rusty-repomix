use std::process::ExitCode;

use rusty_repomix::{handle_error, init_tracing};

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
    Ok(())
}
