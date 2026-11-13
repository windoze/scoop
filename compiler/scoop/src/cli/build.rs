use std::path::PathBuf;

use scoop::{
    ArtifactSearchRoot, BuildContext, BuildProfile, BuildRequest, BuildResult, DiagnosticsPolicy,
    GraphTarget, PairedScoopcLocator, RuntimeInput, TrustedSysrootRoot,
};
use scoop_protocol::CompilationPurpose;

use super::args::BuildOptions;
use super::commands::{absolute, config, paths, target};
use super::package_manager::{PrepareInput, prepare};

#[allow(clippy::too_many_arguments)]
pub(super) fn requests(
    args: BuildOptions,
    sysroot: Option<PathBuf>,
    package_manager: Option<PathBuf>,
    output: Option<PathBuf>,
    purpose: CompilationPurpose,
    require_executable: bool,
    enable_gc_stress: bool,
    enable_startup_stats: bool,
    diagnostics: DiagnosticsPolicy,
) -> BuildResult<Vec<BuildRequest>> {
    let cwd = std::env::current_dir().map_err(config)?;
    let args = args.normalized(&cwd)?;
    let mut saved_options = args.clone();
    let saved_package_manager = super::package_manager::executable(&cwd, package_manager.clone())?;
    let generators =
        scoop_toolchain::GeneratorPrograms::from_cli(&args.generators, &cwd).map_err(config)?;
    let triple = target(args.target)?;
    let selection = scoop_toolchain::select_target(&triple).map_err(config)?;
    let profile = args.profile.unwrap_or(if args.release {
        BuildProfile::Release
    } else {
        BuildProfile::Debug
    });
    if args.profile_use.is_some() && profile != BuildProfile::Release {
        return Err(config(
            "--profile-use requires --profile release (or --release)",
        ));
    }
    let profile_use = args
        .profile_use
        .map(|path| absolute(&cwd, path))
        .transpose()?;
    let debug_info = args.debug_info.unwrap_or(match profile {
        BuildProfile::Debug => scoop_lir::DebugInfoLevel::Full,
        BuildProfile::Release => scoop_lir::DebugInfoLevel::None,
        BuildProfile::Profiling => scoop_lir::DebugInfoLevel::LineTables,
    });
    let sysroot = absolute(
        &cwd,
        scoop_toolchain::configured_sysroot_root(sysroot).map_err(config)?,
    )?;
    let system_library_root = args
        .system_library_root
        .map(|root| absolute(&cwd, root))
        .transpose()?
        .unwrap_or_else(|| {
            scoop_toolchain::system_library_root(
                &sysroot,
                selection.target(),
                profile.instrumentation(),
                profile_use.is_some(),
            )
        });
    let search = paths(&cwd, args.cone_path)?;
    let inputs = prepare(PrepareInput {
        cwd: &cwd,
        input: args.root_input,
        options: args.packages,
        executable: package_manager,
        purpose,
        target: selection.target().contract().canonical_triple(),
        sysroot: &sysroot,
        system_library_root: Some(&system_library_root),
        search: &search,
    })?;
    let target_dir = inputs.target_root(&cwd, args.target_dir)?;
    let cache = super::paths::build_cache(
        &cwd,
        &target_dir,
        selection.target().contract().canonical_triple(),
        profile,
        args.cache_dir,
    )?;
    let compiler = match args.scoopc {
        Some(path) => absolute(&cwd, path)?,
        None => scoop_toolchain::installed_host_tool("scoopc").map_err(config)?,
    };
    let (c_toolchain, mut final_link) = args.native.resolve(&cwd, &sysroot)?;
    final_link.emit_debug_file |= debug_info != scoop_lir::DebugInfoLevel::None;
    let target = GraphTarget::requested(super::native::target_request(triple, &c_toolchain)?)
        .map_err(config)?;
    let runtime = match (args.runtime_objects, args.runtime_root) {
        (Some(index), _) => RuntimeInput::ObjectIndex(absolute(&cwd, index)?),
        (_, Some(root)) => RuntimeInput::SourceRoot(absolute(&cwd, root)?),
        (None, None) => RuntimeInput::ObjectIndex(if profile == BuildProfile::Profiling {
            scoop_toolchain::default_profiling_runtime_index(
                &sysroot,
                target.lir_target(),
                enable_gc_stress,
                enable_startup_stats,
            )
        } else {
            scoop_toolchain::default_runtime_index(
                &sysroot,
                target.lir_target(),
                enable_gc_stress,
                enable_startup_stats,
            )
        }),
    };
    let context = BuildContext {
        artifact_search_roots: search
            .into_iter()
            .map(ArtifactSearchRoot::new)
            .collect::<Result<_, _>>()
            .map_err(config)?,
        cache_root: cache,
        sysroot: TrustedSysrootRoot::new(sysroot).map_err(config)?,
        system_library_root,
        target,
        compiler: PairedScoopcLocator::new(compiler).map_err(config)?,
        diagnostics,
        optimization: profile.optimization(),
        debug_info,
        export_ir: args.export_ir,
        instrumentation: profile.instrumentation(),
        profile_use,
        purpose,
        jobs: args.jobs.unwrap_or_else(|| {
            std::thread::available_parallelism().unwrap_or(std::num::NonZeroUsize::MIN)
        }),
        generators,
    };
    let graphs = inputs.graphs(context)?;
    if output.is_some() && graphs.len() != 1 {
        return Err(config("-o requires exactly one selected root"));
    }
    if purpose == CompilationPurpose::Test
        && graphs
            .iter()
            .any(|graph| graph.root_identity() == scoop_identity::ConeIdentity::SINGLE_FILE)
    {
        return Err(config("test requires a manifest Cone"));
    }
    let output = output.map(|path| absolute(&cwd, path)).transpose()?;
    let libraries = paths(&cwd, args.library_path)?;
    saved_options.target = Some(selection.target().contract().canonical_triple().to_owned());
    saved_options.profile = Some(profile);
    saved_options.release = false;
    graphs
        .into_iter()
        .map(|graph| {
            let mut options = saved_options.clone();
            options.root_input = Some(
                graph
                    .root_source_path()
                    .ok_or_else(|| config("build root must have a source input"))?
                    .to_owned(),
            );
            options.packages.members = Default::default();
            options.scoopc = Some(graph.compiler().as_path().to_owned());
            let path = |value: &std::path::Path| {
                scoop_protocol::HostPathCarrier::from_path(value).map_err(config)
            };
            let invocation = scoop::options::BuildInvocation {
                cwd: path(&cwd)?,
                sysroot: path(graph.sysroot().as_path())?,
                package_manager: path(&saved_package_manager)?,
                enable_gc_stress,
                enable_startup_stats,
                effective_debug_info: debug_info.as_str().to_owned(),
                options,
            };
            Ok(BuildRequest {
                invocation: Some(invocation),
                graph,
                profile,
                cwd: cwd.clone(),
                output: output.clone(),
                emit_slib: false,
                target_dir: Some(target_dir.clone()),
                runtime: runtime.clone(),
                enable_gc_stress,
                enable_startup_stats,
                final_link: final_link.clone(),
                library_paths: libraries.clone(),
                dumps: None,
                require_executable,
                profile_use_required: args.profile_use_required,
            })
        })
        .collect::<BuildResult<_>>()
}
