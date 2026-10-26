use std::path::PathBuf;

use clap::Args;
use scoop::{
    ArtifactCacheRoot, ArtifactSearchRoot, BuildProfile, BuildRootInput, CheckRequest,
    PairedScoopcLocator, TrustedSysrootRoot,
};
use scoop_manifest::{CurrentConeInput, classify_current_cone_operand};

use super::args::MessageFormat;
use super::commands::{absolute, config, paths, target};
use super::presentation::Reporter;

#[derive(Args)]
pub(super) struct CheckArgs {
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
    native_sysroot: Option<PathBuf>,
    #[arg(long, default_value = "human")]
    pub message_format: MessageFormat,
}

pub(super) fn execute(
    args: CheckArgs,
    sysroot: Option<PathBuf>,
    reporter: &Reporter,
) -> scoop::BuildResult<()> {
    let cwd = std::env::current_dir().map_err(config)?;
    let input = absolute(&cwd, args.root_input.unwrap_or_else(|| "Cone.toml".into()))?;
    let root = match classify_current_cone_operand(input)
        .map_err(|cause| config(&cause).at_host(cause.path()))?
    {
        CurrentConeInput::Manifest { root } => BuildRootInput::manifest(root).map_err(config)?,
        CurrentConeInput::SingleFile { source } => BuildRootInput::single_file(source),
    };
    let selection = scoop_toolchain::select_target(&target(args.target)?).map_err(config)?;
    let target = scoop_toolchain::CToolchainOptions {
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
        None => scoop_toolchain::installed_executable()
            .map_err(config)?
            .parent()
            .ok_or_else(|| config("scoop executable has no parent"))?
            .join("scoopc"),
    };
    let outcome = scoop::check(CheckRequest {
        root,
        target,
        sysroot: TrustedSysrootRoot::new(absolute(
            &cwd,
            scoop_toolchain::configured_sysroot_root(sysroot).map_err(config)?,
        )?)
        .map_err(config)?,
        cache: ArtifactCacheRoot::new(absolute(
            &cwd,
            scoop_toolchain::configured_cache_root(args.cache_dir).map_err(config)?,
        )?)
        .map_err(config)?,
        compiler: PairedScoopcLocator::new(compiler).map_err(config)?,
        search: paths(&cwd, args.cone_path)?
            .into_iter()
            .map(ArtifactSearchRoot::new)
            .collect::<Result<_, _>>()
            .map_err(config)?,
        purpose: if args.tests {
            scoop_protocol::CompilationPurpose::Test
        } else {
            scoop_protocol::CompilationPurpose::Production
        },
        profile: args.profile.unwrap_or(if args.release {
            BuildProfile::Release
        } else {
            BuildProfile::Debug
        }),
        jobs: args.jobs.unwrap_or_else(|| {
            std::thread::available_parallelism().unwrap_or(std::num::NonZeroUsize::MIN)
        }),
        generators: scoop_toolchain::GeneratorPrograms::from_cli(&args.generators, &cwd)
            .map_err(config)?,
    })?;
    reporter.checked(&outcome).map_err(config)
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
