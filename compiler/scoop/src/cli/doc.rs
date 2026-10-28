use std::path::PathBuf;

use clap::Args;
use scoop::{
    ArtifactSearchRoot, BuildContext, BuildProfile, DiagnosticsPolicy, DocumentRequest,
    GraphTarget, PairedScoopcLocator, TrustedSysrootRoot,
};

use super::args::MessageFormat;
use super::commands::{absolute, config, paths, target};
use super::presentation::Reporter;

#[derive(Args)]
pub(super) struct DocArgs {
    #[command(flatten)]
    pub packages: super::package_manager::PackageOptions,
    #[arg(short = 'j', long)]
    pub jobs: Option<std::num::NonZeroUsize>,
    /// Cone directories or Cone.toml files; defaults to ./Cone.toml.
    pub roots: Vec<PathBuf>,
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long)]
    pub scoopc: Option<PathBuf>,
    #[arg(long)]
    pub cache_dir: Option<PathBuf>,
    #[arg(long)]
    pub cone_path: Vec<PathBuf>,
    #[arg(long)]
    pub target_dir: Option<PathBuf>,
    #[arg(long)]
    pub output_dir: Option<PathBuf>,
    /// Include non-public, non-local declarations.
    #[arg(long)]
    pub document_private_items: bool,
    /// Document only explicitly selected roots.
    #[arg(long)]
    pub no_deps: bool,
    /// Keep the previous site if any documentation warning is reported.
    #[arg(long)]
    pub deny_doc_warnings: bool,
    /// Open the generated entry in the default browser after publication.
    #[arg(long)]
    pub open: bool,
    /// C compiler for source dependencies whose artifacts need building.
    #[arg(long)]
    pub cc: Option<PathBuf>,
    #[arg(long)]
    pub cxx: Option<PathBuf>,
    #[arg(long)]
    pub native_sysroot: Option<PathBuf>,
    #[arg(long, default_value = "human")]
    pub message_format: MessageFormat,
}

pub(super) fn execute(
    args: DocArgs,
    sysroot: Option<PathBuf>,
    package_manager: Option<PathBuf>,
    reporter: &Reporter,
) -> scoop::BuildResult<()> {
    let cwd = std::env::current_dir().map_err(config)?;
    if args.roots.len() > 1 && args.packages.selects_members() {
        return Err(config(
            "workspace member selection accepts at most one explicit workspace input",
        ));
    }
    if args.roots.iter().any(|path| {
        path.extension()
            .is_some_and(|extension| extension == "scoop")
    }) {
        return Err(config(
            "doc requires manifest Cones; single-file input is not accepted",
        ));
    }
    let triple = target(args.target)?;
    let selection = scoop_toolchain::select_target(&triple).map_err(config)?;
    let target = scoop_toolchain::CToolchainOptions {
        compiler: args.cc,
        cxx_compiler: args.cxx,
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
    let sysroot = absolute(
        &cwd,
        scoop_toolchain::configured_sysroot_root(sysroot).map_err(config)?,
    )?;
    let search = paths(&cwd, args.cone_path)?;
    let roots = if args.roots.is_empty() {
        vec![None]
    } else {
        args.roots.into_iter().map(Some).collect()
    };
    let mut prepared = Vec::new();
    for root in roots {
        prepared.push(super::package_manager::prepare(
            super::package_manager::PrepareInput {
                cwd: &cwd,
                input: root,
                options: args.packages.clone(),
                executable: package_manager.clone(),
                purpose: scoop_protocol::CompilationPurpose::Production,
                target: selection.target().contract().canonical_triple(),
                sysroot: &sysroot,
                search: &search,
                cache: args.cache_dir.clone(),
            },
        )?);
    }
    let target_dir = prepared[0].target_root(&cwd, args.target_dir)?;
    let cache = super::paths::build_cache(
        &cwd,
        &target_dir,
        selection.target().contract().canonical_triple(),
        BuildProfile::Debug,
        args.cache_dir,
    )?;
    let compiler = PairedScoopcLocator::new(compiler).map_err(config)?;
    let context = BuildContext {
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
        optimization: Default::default(),
        purpose: scoop_protocol::CompilationPurpose::Production,
        jobs: args.jobs.unwrap_or_else(|| {
            std::thread::available_parallelism().unwrap_or(std::num::NonZeroUsize::MIN)
        }),
        generators: Default::default(),
    };
    let mut graphs = Vec::new();
    for input in prepared {
        graphs.extend(input.graphs(context.clone())?);
    }
    let output = args
        .output_dir
        .map(|path| absolute(&cwd, path))
        .transpose()?
        .unwrap_or_else(|| {
            target_dir
                .join("doc")
                .join(selection.target().contract().canonical_triple())
        });
    let request = DocumentRequest {
        target,
        cache,
        compiler,
        output,
        no_deps: args.no_deps,
        private: args.document_private_items,
        deny_warnings: args.deny_doc_warnings,
    };
    let outcome = scoop::document(graphs, request)?;
    reporter.documented(&outcome).map_err(config)?;
    if args.open {
        use scoop_process::CommandExt;
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        let status = std::process::Command::new(opener)
            .arg(&outcome.output)
            .scoop_spawn()
            .and_then(|mut child| child.wait())
            .map_err(|e| {
                config(format!(
                    "generated {}, but could not open it: {e}",
                    outcome.output.display()
                ))
            })?;
        if !status.success() {
            return Err(config(format!(
                "generated {}, but {opener} exited with {status}",
                outcome.output.display()
            )));
        }
    }
    Ok(())
}
