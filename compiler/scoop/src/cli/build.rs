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
) -> BuildResult<Vec<BuildRequest>> {
    let cwd = std::env::current_dir().map_err(config)?;
    let generators =
        scoop_toolchain::GeneratorPrograms::from_cli(&args.generators, &cwd).map_err(config)?;
    let triple = target(args.target)?;
    let selection = scoop_toolchain::select_target(&triple).map_err(config)?;
    let profile = args.profile.unwrap_or(if args.release {
        BuildProfile::Release
    } else {
        BuildProfile::Debug
    });
    let sysroot = absolute(
        &cwd,
        scoop_toolchain::configured_sysroot_root(sysroot).map_err(config)?,
    )?;
    let search = paths(&cwd, args.cone_path)?;
    let inputs = prepare(PrepareInput {
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
        None => scoop_toolchain::installed_executable()
            .map_err(config)?
            .with_file_name(format!("scoopc{}", std::env::consts::EXE_SUFFIX)),
    };
    let (c_toolchain, final_link) = args.native.resolve(&cwd, &sysroot)?;
    let target = scoop_toolchain::ResolvedTargetProfile::resolve_request_cached(
        &super::native::target_request(triple, &c_toolchain)?,
        cache.as_path(),
    )
    .map_err(config)?;
    let runtime = match (args.runtime_objects, args.runtime_root) {
        (Some(index), _) => RuntimeInput::ObjectIndex(absolute(&cwd, index)?),
        (_, Some(root)) => RuntimeInput::SourceRoot(absolute(&cwd, root)?),
        (None, None) => RuntimeInput::ObjectIndex(scoop_toolchain::default_runtime_index(
            &sysroot,
            target.lir_target(),
            enable_gc_stress,
            enable_startup_stats,
        )),
    };
    let context = BuildContext {
        artifact_search_roots: search
            .into_iter()
            .map(ArtifactSearchRoot::new)
            .collect::<Result<_, _>>()
            .map_err(config)?,
        cache_root: cache,
        sysroot: TrustedSysrootRoot::new(sysroot).map_err(config)?,
        target: GraphTarget::Artifact(Box::new(target)),
        compiler: PairedScoopcLocator::new(compiler).map_err(config)?,
        diagnostics: DiagnosticsPolicy::Structured,
        optimization: profile.optimization(),
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
    Ok(graphs
        .into_iter()
        .map(|graph| BuildRequest {
            graph,
            profile,
            cwd: cwd.clone(),
            output: output.clone(),
            target_dir: Some(target_dir.clone()),
            runtime: runtime.clone(),
            enable_gc_stress,
            enable_startup_stats,
            final_link: final_link.clone(),
            library_paths: libraries.clone(),
            dumps: None,
            require_executable,
        })
        .collect())
}
