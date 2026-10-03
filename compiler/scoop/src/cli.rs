use std::process::ExitCode;

use clap::Parser;
use scoop::{BuildFailure, BuildFailurePhase};

mod args;
mod commands;
mod presentation;
mod run;

use args::{Cli, MessageFormat};
use presentation::Reporter;

pub(super) fn main() -> ExitCode {
    let arguments = std::env::args_os().collect::<Vec<_>>();
    let cli = match Cli::try_parse_from(&arguments) {
        Ok(cli) => cli,
        Err(error) => {
            if !error.use_stderr() {
                let _ = error.print();
                return ExitCode::SUCCESS;
            }
            let json = arguments
                .iter()
                .take_while(|arg| *arg != "--")
                .collect::<Vec<_>>();
            if json.iter().any(|arg| *arg == "--message-format=json")
                || json
                    .windows(2)
                    .any(|pair| pair[0] == "--message-format" && pair[1] == "json")
            {
                let _ = Reporter(MessageFormat::Json).failure(&BuildFailure::tool(
                    "SCOOP_CLI_ARGS",
                    BuildFailurePhase::Request,
                    error,
                ));
            } else {
                let _ = error.print();
            }
            return ExitCode::from(2);
        }
    };
    let reporter = Reporter(cli.command.message_format());
    if let Err(error) = scoop_process::initialize() {
        let _ = reporter.failure(&BuildFailure::tool(
            "SCOOP_PROCESS_INIT",
            BuildFailurePhase::Request,
            error,
        ));
        return ExitCode::FAILURE;
    }
    match commands::execute(cli.command, &reporter) {
        Ok(None) => {
            scoop_process::finish_interruption();
            ExitCode::SUCCESS
        }
        Ok(Some(status)) => scoop_process::exit_with_status(status),
        Err(error) => {
            scoop_process::finish_interruption();
            let _ = reporter.failure(&error);
            ExitCode::FAILURE
        }
    }
}
