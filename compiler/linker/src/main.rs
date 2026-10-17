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
    cache_dir: Option<PathBuf>,
    #[arg(long)]
    root_slib: PathBuf,
    #[arg(long = "dep-slib")]
    dependencies: Vec<PathBuf>,
    #[arg(long = "runtime-objects")]
    runtime_index: Option<PathBuf>,
    #[arg(long)]
    target: String,
    /// Linux C compiler driver.
    #[arg(long)]
    cc: Option<PathBuf>,
    #[arg(long)]
    native_sysroot: Option<PathBuf>,
    /// Scoop sysroot containing the target runtime and native libraries.
    #[arg(long)]
    sysroot: Option<PathBuf>,
    #[arg(long)]
    unwind_prefix: Option<PathBuf>,
    #[arg(long, value_enum)]
    link_mode: Option<LinkMode>,
    /// Search these explicit roots for libraries required by the artifacts.
    #[arg(long = "library-path")]
    library_paths: Vec<PathBuf>,
    #[arg(short = 'o', long = "output")]
    output: PathBuf,
    /// Print the canonical input order and generated startup source.
    #[arg(long)]
    dump_plan: bool,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum LinkMode {
    Static,
    Dynamic,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Err(error) = scoop_process::initialize() {
        eprintln!("cannot initialize process signal handling: {error}");
        return ExitCode::FAILURE;
    }
    let cache_dir = match scoop_toolchain::configured_cache_root(cli.cache_dir) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("scoop-link: {error}");
            return ExitCode::FAILURE;
        }
    };
    let request = ArtifactLinkRequest {
        cache_dir,
        root_slib: cli.root_slib,
        dependency_slibs: cli.dependencies,
        runtime_index: cli.runtime_index,
        target: cli.target,
        c_toolchain: scoop_toolchain::CToolchainOptions {
            compiler: cli.cc,
            native_sysroot: cli.native_sysroot,
        },
        final_link: scoop_toolchain::FinalLinkOptions {
            sysroot: cli.sysroot,
            unwind_prefix: cli.unwind_prefix,
            mode: cli.link_mode.map(|mode| match mode {
                LinkMode::Static => scoop_toolchain::LinkMode::Static,
                LinkMode::Dynamic => scoop_toolchain::LinkMode::Dynamic,
            }),
        },
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
