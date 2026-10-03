use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use scoop_linker::ArtifactLinkRequest;

#[derive(Parser)]
#[command(
    name = "scoop-link",
    about = "Link Scoop artifacts into a native executable"
)]
struct Cli {
    #[arg(long)]
    root_slib: PathBuf,
    #[arg(long = "dep-slib")]
    dependencies: Vec<PathBuf>,
    #[arg(long = "runtime-objects")]
    runtime_index: PathBuf,
    #[arg(long)]
    target: String,
    /// Search these explicit roots for libraries required by the artifacts.
    #[arg(long = "library-path")]
    library_paths: Vec<PathBuf>,
    #[arg(short = 'o', long = "output")]
    output: PathBuf,
    /// Print the canonical input order and generated startup source.
    #[arg(long)]
    dump_plan: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Err(error) = scoop_process::initialize() {
        eprintln!("cannot initialize process signal handling: {error}");
        return ExitCode::FAILURE;
    }
    let request = ArtifactLinkRequest {
        root_slib: cli.root_slib,
        dependency_slibs: cli.dependencies,
        runtime_index: cli.runtime_index,
        target: cli.target,
        library_paths: cli.library_paths,
        output: cli.output,
    };
    let result = match request.link() {
        Ok(output) => {
            if cli.dump_plan {
                print!("{}", output.plan_dump);
            }
            println!("link-plan {}", output.fingerprint);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("scoop-link: {error}");
            ExitCode::FAILURE
        }
    };
    scoop_process::finish_interruption();
    result
}
