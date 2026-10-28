use std::path::PathBuf;
use std::process::ExitCode;

#[derive(clap::Args)]
pub(super) struct CheckArgs {
    /// One `.scoop` file; use `scoop check` for Cone projects.
    input: PathBuf,
    #[arg(long = "direct-slib")]
    direct_slibs: Vec<PathBuf>,
    #[arg(long = "support-slib")]
    support_slibs: Vec<PathBuf>,
    #[arg(long)]
    target: Option<String>,
    #[arg(long)]
    sysroot: Option<PathBuf>,
    #[arg(long = "generator", value_name = "NAME=PATH")]
    generators: Vec<std::ffi::OsString>,
}

pub(super) fn run(args: CheckArgs) -> ExitCode {
    let request = scoopc::normalize_direct_check_request(
        args.input,
        args.direct_slibs,
        args.support_slibs,
        scoopc::DiagnosticOutputPolicy::Human,
        scoopc::DirectCheckOptions {
            target: args.target,
            sysroot: args.sysroot,
            generators: args.generators,
        },
    );
    let request = match request {
        Ok(request) => request,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    match request.compile(false) {
        Ok(result) => {
            if !result.warnings().is_empty() {
                eprintln!("{}", result.warnings().render_human());
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            if let Some(warnings) = error.warnings().filter(|warnings| !warnings.is_empty()) {
                eprintln!("{}", warnings.render_human());
            }
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    #[test]
    fn check_arguments_need_no_output_and_reject_machine_options() {
        assert!(super::super::Cli::try_parse_from(["scoopc", "check", "main.scoop"]).is_ok());
        for option in ["--out-slib", "--emit", "--cc", "--test"] {
            assert!(
                super::super::Cli::try_parse_from([
                    "scoopc",
                    "check",
                    "main.scoop",
                    option,
                    "value"
                ])
                .is_err()
            );
        }
    }
}
