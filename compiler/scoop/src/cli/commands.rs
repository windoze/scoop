use std::path::{Path, PathBuf};
use std::process::ExitStatus;

use scoop::{
    ArtifactCacheRoot, ArtifactSearchRoot, BuildDumpRequest, BuildFailure, BuildFailurePhase,
    BuildOutcome, BuildProfile, BuildRequest, BuildResult, BuildRootInput, DiagnosticsPolicy,
    PairedScoopcLocator, RuntimeInput, TrustedSysrootRoot,
};
use scoop_manifest::{CurrentConeInput, classify_current_cone_operand};

use super::args::*;
use super::presentation::Reporter;

pub(super) fn execute(command: Command, reporter: &Reporter) -> BuildResult<Option<ExitStatus>> {
    match command {
        Command::Build(args) => {
            let mut request = build_request(args.options, args.output, false)?;
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
            Ok(None)
        }
        Command::Run(args) => {
            let request = build_request(args.options, None, true)?;
            let cwd = request.cwd.clone();
            let result = scoop::build(request)?;
            reporter.built(&result).map_err(config)?;
            match result {
                BuildOutcome::Executable {
                    build,
                    execution_copy: Some(copy),
                    ..
                } => super::run::program(&build.output, copy, &args.program_args, &cwd).map(Some),
                _ => Err(config("run did not produce a private executable copy")),
            }
        }
        Command::Link(args) => {
            let cwd = std::env::current_dir().map_err(config)?;
            let request = scoop::LinkRequest {
                root_slib: absolute(&cwd, args.root_slib)?,
                dependency_slibs: paths(&cwd, args.dependency_slib)?,
                cone_paths: paths(&cwd, args.cone_path)?,
                sysroot: absolute(&cwd, scoop_toolchain::configured_sysroot_root(args.sysroot))?,
                target: target(args.target)?,
                runtime_index: absolute(&cwd, args.runtime_objects)?,
                library_paths: paths(&cwd, args.library_path)?,
                output: absolute(&cwd, args.output)?,
            };
            reporter
                .linked(&scoop::link_artifacts(request)?, args.dump_plan)
                .map_err(config)?;
            Ok(None)
        }
    }
}

fn build_request(
    args: BuildOptions,
    output: Option<PathBuf>,
    keep_for_run: bool,
) -> BuildResult<BuildRequest> {
    let cwd = std::env::current_dir().map_err(config)?;
    let input = absolute(
        &cwd,
        args.root_input
            .unwrap_or_else(|| PathBuf::from("Cone.toml")),
    )?;
    let root = match classify_current_cone_operand(input)
        .map_err(|error| config(&error).at_host(error.path()))?
    {
        CurrentConeInput::Manifest { root } => BuildRootInput::manifest(root).map_err(config)?,
        CurrentConeInput::SingleFile { source } => BuildRootInput::single_file(source),
    };
    if root.single_file_source().is_some() && !args.cone_path.is_empty() {
        return Err(config(
            "--cone-path is not accepted for a single-file input",
        ));
    }
    let compiler = match args.scoopc {
        Some(path) => path,
        None => std::env::current_exe()
            .map_err(config)?
            .parent()
            .ok_or_else(|| config("scoop executable has no parent"))?
            .join("scoopc"),
    };
    let cache = args
        .cache_dir
        .or_else(|| std::env::var_os("SCOOP_CACHE_DIR").map(PathBuf::from))
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|home| home.is_absolute())
                .map(|home| home.join("Library/Caches/Scoop"))
        })
        .ok_or_else(|| config("no usable HOME; provide --cache-dir or SCOOP_CACHE_DIR"))?;
    let graph = scoop::BuildGraphRequest::new(
        root,
        paths(&cwd, args.cone_path)?
            .into_iter()
            .map(ArtifactSearchRoot::new)
            .collect::<Result<_, _>>()
            .map_err(config)?,
        ArtifactCacheRoot::new(absolute(&cwd, cache)?).map_err(config)?,
        TrustedSysrootRoot::new(absolute(
            &cwd,
            scoop_toolchain::configured_sysroot_root(args.sysroot),
        )?)
        .map_err(config)?,
        scoop_protocol::TargetSelectionRequestV1::new(target(args.target)?).map_err(config)?,
        PairedScoopcLocator::new(absolute(&cwd, compiler)?).map_err(config)?,
        DiagnosticsPolicy::Structured,
    )
    .map_err(BuildFailure::classified)?;
    let runtime = match args.runtime_objects {
        Some(index) => RuntimeInput::ObjectIndex(absolute(&cwd, index)?),
        None => RuntimeInput::SourceRoot(absolute(
            &cwd,
            args.runtime_root
                .unwrap_or_else(scoop_toolchain::development_runtime_root),
        )?),
    };
    Ok(BuildRequest {
        graph,
        profile: args.profile.unwrap_or(if args.release {
            BuildProfile::Release
        } else {
            BuildProfile::Debug
        }),
        output: output.map(|path| absolute(&cwd, path)).transpose()?,
        target_dir: args
            .target_dir
            .map(|path| absolute(&cwd, path))
            .transpose()?,
        runtime,
        library_paths: paths(&cwd, args.library_path)?,
        cwd,
        dumps: None,
        keep_for_run,
    })
}

fn target(explicit: Option<String>) -> BuildResult<String> {
    match explicit {
        Some(target) => Ok(target),
        None => scoop_toolchain::host_target_triple()
            .map(str::to_owned)
            .map_err(config),
    }
}
fn absolute(cwd: &Path, path: PathBuf) -> BuildResult<PathBuf> {
    if path.as_os_str().is_empty() {
        return Err(config("path must not be empty"));
    }
    Ok(if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    })
}
fn paths(cwd: &Path, paths: Vec<PathBuf>) -> BuildResult<Vec<PathBuf>> {
    paths.into_iter().map(|path| absolute(cwd, path)).collect()
}
fn config(error: impl std::fmt::Display) -> Box<BuildFailure> {
    BuildFailure::tool("SCOOP_CLI_CONFIG", BuildFailurePhase::Request, error)
}
