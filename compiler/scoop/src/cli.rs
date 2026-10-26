use std::process::ExitCode;

use clap::Parser;
use scoop::{BuildFailure, BuildFailurePhase};

mod args;
mod check;
mod clean;
mod commands;
mod doc;
mod external;
mod format;
mod native;
mod new;
mod paths;
mod presentation;
mod run;
mod test_run;
mod test_selection;

use args::{Cli, MessageFormat};
use presentation::Reporter;

pub(super) fn main() -> ExitCode {
    let arguments = args::with_single_file_arguments(std::env::args_os().collect());
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
    match commands::execute(
        cli.command,
        cli.sysroot,
        cli.enable_gc_stress,
        cli.enable_startup_stats,
        &reporter,
    ) {
        Ok(commands::CommandOutcome::Complete(success)) => {
            scoop_process::finish_interruption();
            if success {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Ok(commands::CommandOutcome::Child(status)) => scoop_process::exit_with_status(status),
        Err(error) => {
            scoop_process::finish_interruption();
            let _ = reporter.failure(&error);
            ExitCode::FAILURE
        }
    }
}
