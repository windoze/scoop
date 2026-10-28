use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::Args;
use scoop::{BuildContext, BuildResult, BuildRootInput, ResolvedBuildGraph};
use scoop_process::CommandExt;
use scoop_protocol::package::{PACKAGE_PROTOCOL_VERSION, PrepareRequestV1, PreparedWorkspaceV1};
use scoop_protocol::{CompilationPurpose, HostPathCarrier};

use crate::cli::commands::{absolute, config};

#[derive(Args, Clone, Default)]
pub(in crate::cli) struct PackageOptions {
    #[command(flatten)]
    members: super::MemberOptions,
    #[arg(long)]
    lockfile: Option<PathBuf>,
    /// Require an existing dependency selection without changing Cone.lock.
    #[arg(long)]
    locked: bool,
    /// Do not access the network.
    #[arg(long)]
    offline: bool,
    /// Require both --locked and --offline.
    #[arg(long)]
    frozen: bool,
    /// Shared Git and registry source cache; defaults to ~/.scoop/packages.
    #[arg(long)]
    package_cache: Option<PathBuf>,
}

impl PackageOptions {
    pub fn selects_members(&self) -> bool {
        self.members.explicit()
    }
}

pub(in crate::cli) struct PrepareInput<'a> {
    pub cwd: &'a Path,
    pub input: Option<PathBuf>,
    pub options: PackageOptions,
    pub executable: Option<PathBuf>,
    pub purpose: CompilationPurpose,
    pub target: &'a str,
    pub sysroot: &'a Path,
    pub search: &'a [PathBuf],
    pub cache: Option<PathBuf>,
}

pub(in crate::cli) enum Inputs {
    SingleFile(BuildRootInput),
    Workspace(Box<PreparedWorkspaceV1>),
}

impl Inputs {
    pub fn target_root(&self, cwd: &Path, explicit: Option<PathBuf>) -> BuildResult<PathBuf> {
        match explicit {
            Some(path) => absolute(cwd, path),
            None => Ok(match self {
                Self::SingleFile(_) => cwd.join("target"),
                Self::Workspace(prepared) => prepared
                    .workspace
                    .to_path_buf()
                    .map_err(config)?
                    .join("target"),
            }),
        }
    }

    pub fn graphs(self, context: BuildContext) -> BuildResult<Vec<ResolvedBuildGraph>> {
        match self {
            Self::SingleFile(root) => {
                ResolvedBuildGraph::load(root, context).map(|graph| vec![graph])
            }
            Self::Workspace(prepared) => prepared
                .resolutions
                .into_iter()
                .map(|resolution| {
                    let root = BuildRootInput::prepared(resolution).map_err(config)?;
                    ResolvedBuildGraph::load(root, context.clone())
                })
                .collect(),
        }
    }
}

pub(in crate::cli) fn prepare(input: PrepareInput<'_>) -> BuildResult<Inputs> {
    if let Some(source) = input
        .input
        .as_ref()
        .filter(|path| path.extension().is_some_and(|ext| ext == "scoop"))
    {
        if input.options.selects_members() {
            return Err(config(
                "workspace member selection requires a manifest input",
            ));
        }
        return crate::cli::paths::single_file_root(input.cwd, source.clone())
            .map(Inputs::SingleFile);
    }
    let path = |path: &Path| HostPathCarrier::from_path(path).map_err(config);
    let request = PrepareRequestV1 {
        version: PACKAGE_PROTOCOL_VERSION,
        cwd: path(input.cwd)?,
        input: input
            .input
            .map(|value| absolute(input.cwd, value).and_then(|value| path(&value)))
            .transpose()?,
        members: input.options.members.selection(),
        purpose: input.purpose,
        target: input.target.into(),
        sysroot: path(input.sysroot)?,
        artifact_roots: input
            .search
            .iter()
            .map(|value| path(value))
            .collect::<BuildResult<_>>()?,
        package_cache: input
            .options
            .package_cache
            .map(|value| absolute(input.cwd, value).and_then(|value| path(&value)))
            .transpose()?,
        lockfile: input
            .options
            .lockfile
            .map(|value| absolute(input.cwd, value).and_then(|value| path(&value)))
            .transpose()?,
        locked: input.options.locked || input.options.frozen,
        offline: input.options.offline || input.options.frozen,
    };
    let executable = super::executable(input.cwd, input.executable)?;
    let staging = absolute(
        input.cwd,
        scoop_toolchain::configured_cache_root(input.cache).map_err(config)?,
    )?
    .join("pm-requests");
    std::fs::create_dir_all(&staging).map_err(config)?;
    let directory = tempfile::Builder::new()
        .prefix("prepare-")
        .tempdir_in(staging)
        .map_err(config)?;
    let request_path = directory.path().join("request.json");
    let response_path = directory.path().join("response.json");
    std::fs::write(&request_path, serde_json::to_vec(&request).map_err(config)?).map_err(config)?;
    let output = Command::new(&executable)
        .args(["prepare", "--request"])
        .arg(&request_path)
        .arg("--response")
        .arg(&response_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .scoop_spawn()
        .and_then(|child| child.wait_with_output())
        .map_err(|error| {
            config(format!(
                "cannot run package manager {}: {error}",
                executable.display()
            ))
        })?;
    super::check_output(&output, "preparation")?;
    let response: PreparedWorkspaceV1 =
        serde_json::from_slice(&std::fs::read(response_path).map_err(config)?).map_err(config)?;
    response.validate().map_err(config)?;
    Ok(Inputs::Workspace(Box::new(response)))
}
