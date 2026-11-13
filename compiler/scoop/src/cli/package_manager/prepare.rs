use std::path::{Path, PathBuf};
use std::process::Command;

use scoop::{BuildContext, BuildResult, BuildRootInput, ResolvedBuildGraph};
use scoop_process::CommandExt;
use scoop_protocol::package::{PACKAGE_PROTOCOL_VERSION, PrepareRequestV1, PreparedWorkspaceV1};
use scoop_protocol::{CompilationPurpose, HostPathCarrier};

use crate::cli::commands::{absolute, config};

pub(in crate::cli) use scoop::options::PackageOptions;

pub(in crate::cli) struct PrepareInput<'a> {
    pub cwd: &'a Path,
    pub input: Option<PathBuf>,
    pub options: PackageOptions,
    pub executable: Option<PathBuf>,
    pub purpose: CompilationPurpose,
    pub target: &'a str,
    pub sysroot: &'a Path,
    pub system_library_root: Option<&'a Path>,
    pub search: &'a [PathBuf],
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
    if let Some(source) = input.input.as_ref().filter(|path| {
        path.extension().is_some_and(|ext| ext == "scoop") && !input.cwd.join(path).is_dir()
    }) {
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
        system_library_root: input.system_library_root.map(path).transpose()?,
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
    let request = serde_json::to_vec(&request).map_err(config)?;
    let output = Command::new(&executable)
        .arg("prepare")
        .scoop_output_with_input(&request)
        .map_err(|error| {
            config(format!(
                "cannot run package manager {}: {error}",
                executable.display()
            ))
        })?;
    super::check_output(&output, "preparation")?;
    let response: PreparedWorkspaceV1 = serde_json::from_slice(&output.stdout).map_err(config)?;
    response.validate().map_err(config)?;
    Ok(Inputs::Workspace(Box::new(response)))
}
