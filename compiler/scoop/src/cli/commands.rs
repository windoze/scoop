use std::path::{Path, PathBuf};
use std::process::ExitStatus;

use scoop::{
    ArtifactSearchRoot, BuildDumpRequest, BuildFailure, BuildFailurePhase, BuildOutcome,
    BuildProfile, BuildRequest, BuildResult, DiagnosticsPolicy, PairedScoopcLocator, RuntimeInput,
    TrustedSysrootRoot,
};

use super::args::*;
use super::presentation::Reporter;

pub(super) enum CommandOutcome {
    Complete(bool),
    Child(ExitStatus),
}

pub(super) fn execute(
    command: Command,
    sysroot: Option<PathBuf>,
    enable_gc_stress: bool,
    enable_startup_stats: bool,
    reporter: &Reporter,
) -> BuildResult<CommandOutcome> {
    match command {
        Command::External(args) => {
            super::external::execute(&args, sysroot).map(CommandOutcome::Child)
        }
        Command::Fmt(args) => super::format::execute(args, reporter).map(CommandOutcome::Complete),
        Command::Clean(args) => {
            super::clean::execute(args, reporter)?;
            Ok(CommandOutcome::Complete(true))
        }
        Command::New(args) => {
            super::new::execute(args)?;
            Ok(CommandOutcome::Complete(true))
        }
        Command::Doc(args) => {
            super::doc::execute(args, sysroot, reporter)?;
            Ok(CommandOutcome::Complete(true))
        }
        Command::Check(args) => {
            super::check::execute(args, sysroot, reporter)?;
            Ok(CommandOutcome::Complete(true))
        }
        Command::Build(args) => {
            let mut request = build_request(
                args.options,
                sysroot,
                args.output,
                false,
                enable_gc_stress,
                enable_startup_stats,
            )?;
            if let (Some(emit), Some(directory)) = (args.emit, args.dump_dir) {
                request.dumps = Some(BuildDumpRequest {
                    stages: emit.stages(),
                    directory: absolute(&request.cwd, directory)?,
                    scope: match args.dump_scope.unwrap_or_default() {
                        DumpScope::Root => scoop::DumpScope::Root,
                        DumpScope::Sources => scoop::DumpScope::Sources,
                    },
                });
            }
            reporter.built(&scoop::build(request)?).map_err(config)?;
            Ok(CommandOutcome::Complete(true))
        }
        Command::Run(args) => {
            let request = build_request(
                args.options,
                sysroot,
                None,
                true,
                enable_gc_stress,
                enable_startup_stats,
            )?;
            let cwd = request.cwd.clone();
            let result = scoop::build(request)?;
            reporter.built(&result).map_err(config)?;
            match result {
                BuildOutcome::Executable {
                    build, executable, ..
                } => super::run::program(&executable, &build.output, &args.program_args, &cwd)
                    .map(CommandOutcome::Child),
                _ => Err(config("run did not produce an executable")),
            }
        }
        Command::Test(args) => {
            let mut request = build_request(
                args.options,
                sysroot,
                None,
                !args.no_run && !args.list,
                enable_gc_stress,
                enable_startup_stats,
            )?;
            request.graph = request
                .graph
                .with_purpose(scoop_protocol::CompilationPurpose::Test)
                .map_err(config)?;
            let cwd = request.cwd.clone();
            let result = scoop::build(request)?;
            reporter.built(&result).map_err(config)?;
            if args.no_run {
                return Ok(CommandOutcome::Complete(true));
            }
            let summary = result.artifacts().graph.root().artifact().summary();
            let directory = summary
                .test_directory()
                .ok_or_else(|| config("test artifact has no test directory"))?;
            let selected =
                super::test_selection::select(directory, args.filter.as_deref(), args.ignored);
            if args.list {
                reporter
                    .test_list(
                        summary.cone().coordinate().name(),
                        &selected.directory,
                        selected.empty_reason,
                    )
                    .map_err(config)?;
                return Ok(CommandOutcome::Complete(true));
            }
            let target = summary.cone().coordinate().name().to_owned();
            match result {
                BuildOutcome::Executable {
                    build, executable, ..
                } => super::test_run::program(
                    &executable,
                    &build.output,
                    &selected,
                    args.nocapture,
                    &cwd,
                    reporter,
                    &target,
                )
                .map(|status| status.map_or(CommandOutcome::Complete(true), CommandOutcome::Child)),
                _ => Err(config("test did not produce an executable")),
            }
        }
        Command::Link(args) => {
            let cwd = std::env::current_dir().map_err(config)?;
            let sysroot = absolute(
                &cwd,
                scoop_toolchain::configured_sysroot_root(sysroot).map_err(config)?,
            )?;
            let (c_toolchain, final_link) = args.native.resolve(&cwd, &sysroot)?;
            let request = scoop::LinkRequest {
                cache_dir: absolute(
                    &cwd,
                    scoop_toolchain::configured_cache_root(args.cache_dir).map_err(config)?,
                )?,
                root_slib: absolute(&cwd, args.root_slib)?,
                dependency_slibs: paths(&cwd, args.dependency_slib)?,
                cone_paths: paths(&cwd, args.cone_path)?,
                sysroot,
                target: target(args.target)?,
                c_toolchain,
                final_link,
                runtime_index: args
                    .runtime_objects
                    .map(|path| absolute(&cwd, path))
                    .transpose()?,
                enable_gc_stress,
                enable_startup_stats,
                library_paths: paths(&cwd, args.library_path)?,
                output: absolute(&cwd, args.output)?,
            };
            reporter
                .linked(&scoop::link_artifacts(request)?, args.dump_plan)
                .map_err(config)?;
            Ok(CommandOutcome::Complete(true))
        }
    }
}

fn build_request(
    args: BuildOptions,
    sysroot: Option<PathBuf>,
    output: Option<PathBuf>,
    require_executable: bool,
    enable_gc_stress: bool,
    enable_startup_stats: bool,
) -> BuildResult<BuildRequest> {
    let cwd = std::env::current_dir().map_err(config)?;
    let generators =
        scoop_toolchain::GeneratorPrograms::from_cli(&args.generators, &cwd).map_err(config)?;
    let root = super::paths::build_root(&cwd, args.root_input)?;
    let target_dir = super::paths::target_root(&cwd, &root, args.target_dir)?;
    let triple = target(args.target)?;
    let selection = scoop_toolchain::select_target(&triple).map_err(config)?;
    let profile = args.profile.unwrap_or(if args.release {
        BuildProfile::Release
    } else {
        BuildProfile::Debug
    });
    let cache = super::paths::build_cache(
        &cwd,
        &target_dir,
        selection.target().contract().canonical_triple(),
        profile,
        args.cache_dir,
    )?;
    let compiler = match args.scoopc {
        Some(path) => path,
        None => scoop_toolchain::installed_executable()
            .map_err(config)?
            .parent()
            .ok_or_else(|| config("scoop executable has no parent"))?
            .join("scoopc"),
    };
    let sysroot = absolute(
        &cwd,
        scoop_toolchain::configured_sysroot_root(sysroot).map_err(config)?,
    )?;
    let (c_toolchain, final_link) = args.native.resolve(&cwd, &sysroot)?;
    let mut graph = scoop::BuildGraphRequest::new(
        root,
        paths(&cwd, args.cone_path)?
            .into_iter()
            .map(ArtifactSearchRoot::new)
            .collect::<Result<_, _>>()
            .map_err(config)?,
        cache,
        TrustedSysrootRoot::new(sysroot).map_err(config)?,
        super::native::target_request(triple, &c_toolchain)?,
        PairedScoopcLocator::new(absolute(&cwd, compiler)?).map_err(config)?,
        DiagnosticsPolicy::Structured,
    )
    .map_err(BuildFailure::classified)?
    .with_generators(generators);
    if let Some(jobs) = args.jobs {
        graph = graph.with_jobs(jobs);
    }
    let runtime = match (args.runtime_objects, args.runtime_root) {
        (Some(index), _) => RuntimeInput::ObjectIndex(absolute(&cwd, index)?),
        (_, Some(root)) => RuntimeInput::SourceRoot(absolute(&cwd, root)?),
        (None, None) => RuntimeInput::ObjectIndex(scoop_toolchain::default_runtime_index(
            graph.sysroot().as_path(),
            graph.target().lir_target(),
            enable_gc_stress,
            enable_startup_stats,
        )),
    };
    Ok(BuildRequest {
        graph,
        profile,
        output: output.map(|path| absolute(&cwd, path)).transpose()?,
        target_dir: Some(target_dir),
        runtime,
        enable_gc_stress,
        enable_startup_stats,
        final_link,
        library_paths: paths(&cwd, args.library_path)?,
        cwd,
        dumps: None,
        require_executable,
    })
}

pub(super) fn target(explicit: Option<String>) -> BuildResult<String> {
    match explicit {
        Some(target) => Ok(target),
        None => scoop_toolchain::host_target_triple()
            .map(str::to_owned)
            .map_err(config),
    }
}
pub(super) fn absolute(cwd: &Path, path: PathBuf) -> BuildResult<PathBuf> {
    if path.as_os_str().is_empty() {
        return Err(config("path must not be empty"));
    }
    Ok(if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    })
}
pub(super) fn paths(cwd: &Path, paths: Vec<PathBuf>) -> BuildResult<Vec<PathBuf>> {
    paths.into_iter().map(|path| absolute(cwd, path)).collect()
}
pub(super) fn config(error: impl std::fmt::Display) -> Box<BuildFailure> {
    BuildFailure::tool("SCOOP_CLI_CONFIG", BuildFailurePhase::Request, error)
}
