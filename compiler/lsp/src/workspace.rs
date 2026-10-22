use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use lsp_types::{InitializeParams, Uri, WorkspaceFolder};
use scoop_analysis::{ProjectConfiguration, ProjectDiagnostic};
use scoop_protocol::CompilationPurpose;
use serde::Deserialize;
use serde_json::Value;

use crate::documents::{Documents, file_path};

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Purpose {
    #[default]
    Production,
    Test,
}

impl From<Purpose> for CompilationPurpose {
    fn from(value: Purpose) -> Self {
        match value {
            Purpose::Production => Self::Production,
            Purpose::Test => Self::Test,
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct Options {
    target: Option<String>,
    sysroot: Option<PathBuf>,
    purpose: Purpose,
    artifact_search_roots: Vec<PathBuf>,
    configurations: Vec<Configuration>,
    active_configuration: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Configuration {
    name: String,
    root_uri: Uri,
    target: Option<String>,
    sysroot: Option<PathBuf>,
    purpose: Option<Purpose>,
    artifact_search_roots: Option<Vec<PathBuf>>,
}

#[derive(Default)]
pub(crate) struct WorkspaceSettings {
    folders: Vec<WorkspaceFolder>,
    options: Value,
    pub watch_registration: bool,
}

impl WorkspaceSettings {
    pub fn initialize(&mut self, params: &InitializeParams) {
        self.folders = params.workspace_folders.clone().unwrap_or_default();
        #[allow(deprecated)]
        if self.folders.is_empty()
            && let Some(uri) = &params.root_uri
        {
            self.folders.push(WorkspaceFolder {
                uri: uri.clone(),
                name: uri.as_str().to_owned(),
            });
        }
        self.options = params.initialization_options.clone().unwrap_or(Value::Null);
        self.watch_registration = params
            .capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.did_change_watched_files.as_ref())
            .and_then(|files| files.dynamic_registration)
            .unwrap_or(false);
    }

    pub fn change(&mut self, settings: Value) {
        self.options = settings.get("scoop").cloned().unwrap_or(settings);
    }

    pub fn change_folders(&mut self, change: lsp_types::WorkspaceFoldersChangeEvent) {
        self.folders.retain(|folder| {
            !change
                .removed
                .iter()
                .any(|removed| removed.uri == folder.uri)
        });
        for folder in change.added {
            if !self.folders.iter().any(|old| old.uri == folder.uri) {
                self.folders.push(folder);
            }
        }
    }

    pub fn configurations(
        &self,
        documents: &Documents,
    ) -> (
        Vec<ProjectConfiguration>,
        Vec<ProjectDiagnostic>,
        Option<String>,
    ) {
        let mut errors = Vec::new();
        let options = if self.options.is_null() {
            Ok(Options::default())
        } else {
            serde_json::from_value(self.options.clone())
        };
        let options = match options {
            Ok(options) => options,
            Err(error) => {
                errors.push(self.diagnostic("workspace", self.error_path(), error));
                return (Vec::new(), errors, None);
            }
        };
        let mut projects = Vec::new();
        let mut names = BTreeSet::new();
        if options.configurations.is_empty() {
            for folder in &self.folders {
                let root = match file_path(&folder.uri) {
                    Ok(Some(root)) => root,
                    Ok(None) => continue,
                    Err(error) => {
                        errors.push(self.diagnostic(
                            &folder.name,
                            self.error_path(),
                            error.message,
                        ));
                        continue;
                    }
                };
                let mut roots = BTreeSet::new();
                if root.join("Cone.toml").is_file() {
                    roots.insert(root.clone());
                }
                for path in documents.open_paths() {
                    if !path.starts_with(&root) {
                        continue;
                    }
                    for ancestor in path
                        .ancestors()
                        .skip(1)
                        .take_while(|path| path.starts_with(&root))
                    {
                        if ancestor.join("Cone.toml").is_file() {
                            roots.insert(ancestor.to_owned());
                            break;
                        }
                    }
                }
                for root in roots {
                    let name = root.to_string_lossy().into_owned();
                    if !names.insert(name.clone()) {
                        continue;
                    }
                    match project(&name, root.clone(), &options, None) {
                        Ok(project) => projects.push(project),
                        Err(error) => {
                            errors.push(self.diagnostic(&name, root.join("Cone.toml"), error))
                        }
                    }
                }
            }
        } else {
            for configuration in &options.configurations {
                let root = match file_path(&configuration.root_uri) {
                    Ok(Some(root)) => root,
                    _ => {
                        errors.push(self.diagnostic(
                            &configuration.name,
                            self.error_path(),
                            "configuration rootUri must be a local file URI",
                        ));
                        continue;
                    }
                };
                if configuration.name.trim().is_empty() || !names.insert(configuration.name.clone())
                {
                    errors.push(self.diagnostic(
                        &configuration.name,
                        root.join("Cone.toml"),
                        "configuration names must be nonempty and unique",
                    ));
                    continue;
                }
                match project(
                    &configuration.name,
                    root.clone(),
                    &options,
                    Some(configuration),
                ) {
                    Ok(project) => projects.push(project),
                    Err(error) => errors.push(self.diagnostic(
                        &configuration.name,
                        root.join("Cone.toml"),
                        error,
                    )),
                }
            }
        }
        if let Some(active) = &options.active_configuration
            && !names.contains(active)
        {
            errors.push(self.diagnostic(
                active,
                self.error_path(),
                "activeConfiguration does not name a loaded configuration",
            ));
        }
        (projects, errors, options.active_configuration)
    }

    fn error_path(&self) -> PathBuf {
        self.folders
            .first()
            .and_then(|folder| file_path(&folder.uri).ok().flatten())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default()
            .join("Cone.toml")
    }

    fn diagnostic(
        &self,
        name: &str,
        path: PathBuf,
        message: impl std::fmt::Display,
    ) -> ProjectDiagnostic {
        ProjectDiagnostic {
            configuration: name.to_owned(),
            path,
            span: None,
            message: message.to_string(),
        }
    }
}

fn project(
    name: &str,
    root: PathBuf,
    options: &Options,
    entry: Option<&Configuration>,
) -> Result<ProjectConfiguration, String> {
    let target = entry
        .and_then(|entry| entry.target.as_deref())
        .or(options.target.as_deref());
    let target = match target {
        Some(target) => target,
        None => scoop_toolchain::host_target_triple().map_err(|error| error.to_string())?,
    };
    let target = scoop_toolchain::select_target(target).map_err(|error| error.to_string())?;
    let sysroot = entry
        .and_then(|entry| entry.sysroot.clone())
        .or_else(|| options.sysroot.clone());
    let sysroot =
        scoop_toolchain::configured_sysroot_root(sysroot).map_err(|error| error.to_string())?;
    let search = entry
        .and_then(|entry| entry.artifact_search_roots.as_ref())
        .unwrap_or(&options.artifact_search_roots);
    Ok(ProjectConfiguration {
        name: name.to_owned(),
        root: root.clone(),
        target,
        purpose: entry
            .and_then(|entry| entry.purpose)
            .unwrap_or(options.purpose)
            .into(),
        sysroot: absolute(&root, &sysroot),
        artifact_search_roots: search.iter().map(|path| absolute(&root, path)).collect(),
    })
}

fn absolute(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        root.join(path)
    }
}
