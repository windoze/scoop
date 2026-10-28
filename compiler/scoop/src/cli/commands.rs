use std::path::{Path, PathBuf};
use std::process::ExitStatus;

use scoop::{BuildDumpRequest, BuildFailure, BuildFailurePhase, BuildOutcome, BuildResult};

use super::args::*;
use super::presentation::Reporter;

pub(super) enum CommandOutcome {
    Complete(bool),
    Child(ExitStatus),
}

pub(super) fn execute(
    command: Command,
    sysroot: Option<PathBuf>,
    package_manager: Option<PathBuf>,
    enable_gc_stress: bool,
    enable_startup_stats: bool,
    reporter: &Reporter,
) -> BuildResult<CommandOutcome> {
    match command {
        Command::Pm(args) => {
            super::package_manager::execute(args, package_manager).map(CommandOutcome::Child)
        }
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
            super::doc::execute(args, sysroot, package_manager, reporter)?;
            Ok(CommandOutcome::Complete(true))
        }
        Command::Check(args) => {
            super::check::execute(args, sysroot, package_manager, reporter)?;
            Ok(CommandOutcome::Complete(true))
        }
        Command::Build(args) => {
            let mut requests = super::build::requests(
                args.options,
                sysroot,
                package_manager,
                args.output,
                scoop_protocol::CompilationPurpose::Production,
                false,
                enable_gc_stress,
                enable_startup_stats,
            )?;
            for request in &mut requests {
                if let (Some(emit), Some(directory)) = (args.emit, args.dump_dir.as_ref()) {
                    request.dumps = Some(BuildDumpRequest {
                        stages: emit.stages(),
                        directory: absolute(&request.cwd, directory.clone())?,
                        scope: match args.dump_scope.unwrap_or_default() {
                            DumpScope::Root => scoop::DumpScope::Root,
                            DumpScope::Sources => scoop::DumpScope::Sources,
                        },
                    });
                }
            }
            for result in scoop::build_all(requests)? {
                reporter.built(&result).map_err(config)?;
            }
            Ok(CommandOutcome::Complete(true))
        }
        Command::Run(args) => {
            let mut requests = super::build::requests(
                args.options,
                sysroot,
                package_manager,
                None,
                scoop_protocol::CompilationPurpose::Production,
                true,
                enable_gc_stress,
                enable_startup_stats,
            )?;
            if requests.len() > 1 {
                requests.retain(|request| {
                    request.graph.root_kind() == scoop_identity::RequestedConeKind::Executable
                });
            }
            if requests.len() != 1 {
                return Err(config(
                    "run requires exactly one selected executable; use -p group:name",
                ));
            }
            let request = requests.pop().expect("one selected root");
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
            let requests = super::build::requests(
                args.options,
                sysroot,
                package_manager,
                None,
                scoop_protocol::CompilationPurpose::Test,
                !args.no_run && !args.list,
                enable_gc_stress,
                enable_startup_stats,
            )?;
            let cwd = std::env::current_dir().map_err(config)?;
            for result in scoop::build_all(requests)? {
                reporter.built(&result).map_err(config)?;
                if args.no_run {
                    continue;
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
                    continue;
                }
                let target = summary.cone().coordinate().name().to_owned();
                let outcome = match result {
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
                    .map(|status| {
                        status.map_or(CommandOutcome::Complete(true), CommandOutcome::Child)
                    }),
                    _ => Err(config("test did not produce an executable")),
                }?;
                if let CommandOutcome::Child(status) = outcome {
                    if !status.success() {
                        return Ok(CommandOutcome::Child(status));
                    }
                }
            }
            Ok(CommandOutcome::Complete(true))
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
