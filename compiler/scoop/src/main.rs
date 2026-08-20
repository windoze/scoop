//! Public Scoop build, run, and artifact-link commands.
mod cli;

fn main() -> std::process::ExitCode {
    cli::main()
}
