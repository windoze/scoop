use std::path::PathBuf;

use clap::Args;
use scoop::{
    ArtifactSearchRoot, BuildProfile, BuildRootInput, DocumentRequest, PairedScoopcLocator,
    TrustedSysrootRoot,
};
use scoop_manifest::{CurrentConeInput, classify_current_cone_operand};

use super::args::MessageFormat;
use super::commands::{absolute, config, paths, target};
use super::presentation::Reporter;

#[derive(Args)]
pub(super) struct DocArgs {
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
    pub native_sysroot: Option<PathBuf>,
    #[arg(long, default_value = "human")]
    pub message_format: MessageFormat,
}

pub(super) fn execute(
    args: DocArgs,
    sysroot: Option<PathBuf>,
    reporter: &Reporter,
) -> scoop::BuildResult<()> {
    let cwd = std::env::current_dir().map_err(config)?;
    let roots = if args.roots.is_empty() {
        vec![PathBuf::from("Cone.toml")]
    } else {
        args.roots
    };
    let roots = paths(&cwd, roots)?
        .into_iter()
        .map(|path| {
            match classify_current_cone_operand(path).map_err(|e| config(&e).at_host(e.path()))? {
                CurrentConeInput::Manifest { root } => {
                    BuildRootInput::manifest(root).map_err(config)
                }
                CurrentConeInput::SingleFile { .. } => Err(config(
                    "doc requires manifest Cones; single-file input is not accepted",
                )),
            }
        })
        .collect::<scoop::BuildResult<Vec<_>>>()?;
    let triple = target(args.target)?;
    let selection = scoop_toolchain::select_target(&triple).map_err(config)?;
    let target = scoop_toolchain::CToolchainOptions {
        compiler: args.cc,
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
    let cache = super::paths::build_cache(
        &cwd,
        &super::paths::target_root(&cwd, &roots[0], args.target_dir.clone())?,
        selection.target().contract().canonical_triple(),
        BuildProfile::Debug,
        args.cache_dir,
    )?;
    let target_dir = absolute(
        &cwd,
        args.target_dir.unwrap_or_else(|| PathBuf::from("target")),
    )?;
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
        roots,
        target,
        sysroot: TrustedSysrootRoot::new(absolute(
            &cwd,
            scoop_toolchain::configured_sysroot_root(sysroot).map_err(config)?,
        )?)
        .map_err(config)?,
        cache,
        compiler: PairedScoopcLocator::new(compiler).map_err(config)?,
        search: paths(&cwd, args.cone_path)?
            .into_iter()
            .map(ArtifactSearchRoot::new)
            .collect::<Result<_, _>>()
            .map_err(config)?,
        output,
        no_deps: args.no_deps,
        private: args.document_private_items,
        deny_warnings: args.deny_doc_warnings,
    };
    let outcome = scoop::document(request)?;
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
