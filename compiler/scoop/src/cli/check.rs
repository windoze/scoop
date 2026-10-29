use std::path::PathBuf;

use clap::Args;
use scoop::{
    ArtifactSearchRoot, BuildContext, BuildProfile, CheckRequest, DiagnosticsPolicy, GraphTarget,
    PairedScoopcLocator, TrustedSysrootRoot,
};

use super::args::MessageFormat;
use super::commands::{absolute, config, paths, target};
use super::presentation::Reporter;

#[derive(Args)]
pub(super) struct CheckArgs {
    #[command(flatten)]
    packages: super::package_manager::PackageOptions,
    /// Cone directory, Cone.toml, or one .scoop file; defaults to ./Cone.toml.
    root_input: Option<PathBuf>,
    /// Include test sources and dependencies and check the generated test entry.
    #[arg(long)]
    tests: bool,
    #[arg(long)]
    target: Option<String>,
    #[arg(long)]
    scoopc: Option<PathBuf>,
    #[arg(long)]
    cache_dir: Option<PathBuf>,
    #[arg(long)]
    cone_path: Vec<PathBuf>,
    #[arg(long = "generator", value_name = "NAME=PATH")]
    generators: Vec<std::ffi::OsString>,
    /// Maximum concurrent dependency compiler processes.
    #[arg(short = 'j', long)]
    jobs: Option<std::num::NonZeroUsize>,
    /// Optimization profile for source dependencies.
    #[arg(long, conflicts_with = "release")]
    profile: Option<BuildProfile>,
    #[arg(long, conflicts_with = "profile")]
    release: bool,
    /// C compiler for source dependencies whose artifacts need building.
    #[arg(long)]
    cc: Option<PathBuf>,
    #[arg(long)]
    cxx: Option<PathBuf>,
    #[arg(long)]
    native_sysroot: Option<PathBuf>,
    #[arg(long, default_value = "human")]
    pub message_format: MessageFormat,
}

pub(super) fn execute(
    args: CheckArgs,
    sysroot: Option<PathBuf>,
    package_manager: Option<PathBuf>,
    reporter: &Reporter,
) -> scoop::BuildResult<()> {
    let cwd = std::env::current_dir().map_err(config)?;
    let selection = scoop_toolchain::select_target(&target(args.target)?).map_err(config)?;
    let profile = args.profile.unwrap_or(if args.release {
        BuildProfile::Release
    } else {
        BuildProfile::Debug
    });
    let purpose = if args.tests {
        scoop_protocol::CompilationPurpose::Test
    } else {
        scoop_protocol::CompilationPurpose::Production
    };
    let sysroot = absolute(
        &cwd,
        scoop_toolchain::configured_sysroot_root(sysroot).map_err(config)?,
    )?;
    let search = paths(&cwd, args.cone_path)?;
    let inputs = super::package_manager::prepare(super::package_manager::PrepareInput {
        cwd: &cwd,
        input: args.root_input,
        options: args.packages,
        executable: package_manager,
        purpose,
        target: selection.target().contract().canonical_triple(),
        sysroot: &sysroot,
        search: &search,
        cache: args.cache_dir.clone(),
    })?;
    let cache = super::paths::build_cache(
        &cwd,
        &inputs.target_root(&cwd, None)?,
        selection.target().contract().canonical_triple(),
        profile,
        args.cache_dir,
    )?;
    let target = scoop_toolchain::CToolchainOptions {
        cxx_compiler: args.cxx,
        compiler: args
            .cc
            .map(|path| {
                if path.components().count() > 1 {
                    absolute(&cwd, path)
                } else {
                    Ok(path)
                }
            })
            .transpose()?,
        native_sysroot: args
            .native_sysroot
            .map(|path| absolute(&cwd, path))
            .transpose()?,
    }
    .request(selection.target().contract().canonical_triple().into())
    .map_err(config)?;
    let compiler = match args.scoopc {
        Some(path) => absolute(&cwd, path)?,
        None => scoop_toolchain::installed_host_tool("scoopc").map_err(config)?,
    };
    let compiler = PairedScoopcLocator::new(compiler).map_err(config)?;
    let generators =
        scoop_toolchain::GeneratorPrograms::from_cli(&args.generators, &cwd).map_err(config)?;
    let graphs = inputs.graphs(BuildContext {
        artifact_search_roots: search
            .into_iter()
            .map(ArtifactSearchRoot::new)
            .collect::<Result<_, _>>()
            .map_err(config)?,
        cache_root: cache.clone(),
        sysroot: TrustedSysrootRoot::new(sysroot).map_err(config)?,
        target: GraphTarget::Frontend {
            selection,
            request: target.clone(),
        },
        compiler: compiler.clone(),
        diagnostics: DiagnosticsPolicy::Structured,
        optimization: profile.optimization(),
        debug_info: scoop_lir::DebugInfoLevel::None,
        purpose,
        jobs: args.jobs.unwrap_or_else(|| {
            std::thread::available_parallelism().unwrap_or(std::num::NonZeroUsize::MIN)
        }),
        generators: generators.clone(),
    })?;
    for graph in graphs {
        let outcome = scoop::check(
            graph,
            CheckRequest {
                target: target.clone(),
                cache: cache.clone(),
                compiler: compiler.clone(),
                purpose,
                profile,
                generators: generators.clone(),
            },
        )?;
        reporter.checked(&outcome).map_err(config)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::args::{Cli, Command};
    use clap::Parser;

    #[test]
    fn check_is_builtin_and_rejects_output_and_execution_options() {
        assert!(matches!(
            Cli::try_parse_from(["scoop", "check", "--tests", "--release", "-j", "2"])
                .unwrap()
                .command,
            Command::Check(_)
        ));
        for flag in [
            "-o",
            "--target-dir",
            "--emit",
            "--runtime-root",
            "--link-mode",
            "--filter",
        ] {
            assert!(Cli::try_parse_from(["scoop", "check", flag, "value"]).is_err());
        }
    }
}
