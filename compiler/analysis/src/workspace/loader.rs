use std::{collections::BTreeSet, path::PathBuf};

use super::{
    ProjectConfiguration, ProjectDiagnostic, WorkspaceInputs, artifacts::ArtifactInputs,
    files::FileInputs, graph::ProjectBuilder,
};
use crate::AnalysisHost;

/// Keeps unchanged disk inputs; open documents are provided by the caller.
#[derive(Default)]
pub struct WorkspaceLoader {
    pub(super) files: FileInputs,
    pub(super) artifacts: ArtifactInputs,
}

impl WorkspaceLoader {
    pub fn load(
        &mut self,
        host: &mut AnalysisHost,
        configurations: &[ProjectConfiguration],
        overlays: &BTreeSet<PathBuf>,
        active_configuration: Option<String>,
    ) -> WorkspaceInputs {
        let mut inputs = WorkspaceInputs {
            active_configuration,
            ..WorkspaceInputs::default()
        };
        for configuration in configurations {
            match ProjectBuilder::load(self, host, overlays, configuration.clone()) {
                Ok((project, errors)) => {
                    inputs.projects.push(project);
                    inputs.diagnostics.extend(
                        errors
                            .into_iter()
                            .map(|error| diagnostic(configuration, error)),
                    );
                }
                Err(error) => inputs.diagnostics.push(diagnostic(configuration, error)),
            }
        }
        inputs
    }
}

fn diagnostic(
    configuration: &ProjectConfiguration,
    mut error: super::LoadError,
) -> ProjectDiagnostic {
    if error
        .path
        .file_name()
        .is_none_or(|name| name != "Cone.toml")
    {
        error.message = format!("{}: {}", error.path.display(), error.message);
        error.path = if configuration
            .root
            .file_name()
            .is_some_and(|name| name == "Cone.toml")
        {
            configuration.root.clone()
        } else {
            configuration.root.join("Cone.toml")
        };
        error.span = None;
    }
    ProjectDiagnostic {
        configuration: configuration.name.clone(),
        path: error.path,
        span: error.span,
        message: error.message,
    }
}
