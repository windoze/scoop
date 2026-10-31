use std::{
    collections::BTreeSet,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use scoop_identity::ConeCoordinate;
use scoop_process::CommandExt;
use scoop_protocol::{
    CompilationPurpose, DiagnosticOriginV1, HostPathCarrier,
    package::{
        ManifestOverlayV1, ManifestOverlaysV1, PACKAGE_PROTOCOL_VERSION, PackageDiagnosticV1,
        PackageMembersV1, PreparedWorkspaceV1,
    },
};
use serde::de::DeserializeOwned;

use super::{LoadError, ProjectConfiguration};
use crate::AnalysisHost;

pub(super) struct PackageInputs<'a> {
    configuration: &'a ProjectConfiguration,
    overlays: ManifestOverlaysV1,
}

impl<'a> PackageInputs<'a> {
    pub fn new(
        configuration: &'a ProjectConfiguration,
        host: &AnalysisHost,
        overlays: &BTreeSet<PathBuf>,
    ) -> Result<Self, LoadError> {
        let manifests = overlays
            .iter()
            .filter(|path| path.file_name().is_some_and(|name| name == "Cone.toml"))
            .filter_map(|path| {
                host.file_source(path)
                    .and_then(|id| host.source(id))
                    .map(|source| (path, source))
            })
            .map(|(path, source)| {
                Ok(ManifestOverlayV1 {
                    path: HostPathCarrier::from_path(path)
                        .map_err(|error| LoadError::new(path, error))?,
                    text: source.text.as_str().into(),
                })
            })
            .collect::<Result<_, LoadError>>()?;
        Ok(Self {
            configuration,
            overlays: ManifestOverlaysV1 {
                version: PACKAGE_PROTOCOL_VERSION,
                manifests,
            },
        })
    }

    pub fn members(&self, root: &Path) -> Result<PackageMembersV1, LoadError> {
        let result: PackageMembersV1 =
            self.invoke(self.command("members", root), Some(&self.overlays))?;
        result
            .validate()
            .map_err(|error| LoadError::new(root, error))?;
        Ok(result)
    }

    pub fn load_member(
        &self,
        root: &Path,
        member: &ConeCoordinate,
        initialize_lockfile: bool,
    ) -> Result<PreparedWorkspaceV1, LoadError> {
        if initialize_lockfile {
            self.invoke::<serde_json::Value>(
                self.dependency_command("resolve", root, member),
                None,
            )?;
        }
        let mut command = self.dependency_command("metadata", root, member);
        command.arg("--offline");
        let result: PreparedWorkspaceV1 = self.invoke(command, Some(&self.overlays))?;
        result
            .validate()
            .map_err(|error| LoadError::new(root, error))?;
        Ok(result)
    }

    fn dependency_command(&self, action: &str, root: &Path, member: &ConeCoordinate) -> Command {
        let mut command = self.command(action, root);
        command
            .arg("-p")
            .arg(format!("{}:{}", member.group(), member.name()))
            .arg("--purpose")
            .arg(match self.configuration.purpose {
                CompilationPurpose::Production => "production",
                CompilationPurpose::Test => "test",
            })
            .arg("--target")
            .arg(
                self.configuration
                    .target
                    .target()
                    .contract()
                    .canonical_triple(),
            )
            .arg("--sysroot")
            .arg(&self.configuration.sysroot);
        for path in &self.configuration.artifact_search_roots {
            command.arg("--cone-path").arg(path);
        }
        if let Some(path) = &self.configuration.package_cache {
            command.arg("--package-cache-dir").arg(path);
        }
        if let Some(path) = &self.configuration.lockfile {
            command.arg("--lockfile").arg(path);
        }
        command
    }

    fn command(&self, action: &str, root: &Path) -> Command {
        let mut command = Command::new(&self.configuration.package_manager);
        command.arg(action).arg(root).arg("--json");
        command
    }

    fn invoke<T: DeserializeOwned>(
        &self,
        mut command: Command,
        overlays: Option<&ManifestOverlaysV1>,
    ) -> Result<T, LoadError> {
        let path = &self.configuration.root;
        let bytes = if let Some(overlays) = overlays.filter(|value| !value.manifests.is_empty()) {
            command.arg("--overlays-stdin");
            Some(serde_json::to_vec(overlays).map_err(|error| LoadError::new(path, error))?)
        } else {
            None
        };
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .scoop_spawn()
            .map_err(|error| {
                LoadError::new(
                    path,
                    format!(
                        "cannot run package manager {}: {error}",
                        self.configuration.package_manager.display()
                    ),
                )
            })?;
        let written = if let Some(bytes) = bytes {
            child.stdin.take().expect("piped stdin").write_all(&bytes)
        } else {
            drop(child.stdin.take());
            Ok(())
        };
        let output = child
            .wait_with_output()
            .map_err(|error| LoadError::new(path, error))?;
        if !output.status.success() {
            if let Ok(record) = serde_json::from_slice::<PackageDiagnosticV1>(&output.stderr) {
                record
                    .validate()
                    .map_err(|error| LoadError::new(path, error))?;
                let diagnostic = record.diagnostic;
                let mut error = LoadError::new(path, diagnostic.message());
                if let DiagnosticOriginV1::HostPathSpan { path, span } = diagnostic.origin() {
                    error.path = path
                        .to_path_buf()
                        .map_err(|error| LoadError::new(&self.configuration.root, error))?;
                    error = error.with_span(span.start() as usize..span.end() as usize);
                }
                return Err(error);
            }
            return Err(LoadError::new(
                path,
                format!(
                    "package manager failed ({}): {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            ));
        }
        written.map_err(|error| LoadError::new(path, error))?;
        serde_json::from_slice(&output.stdout).map_err(|error| {
            LoadError::new(path, format!("invalid package manager response: {error}"))
        })
    }
}
