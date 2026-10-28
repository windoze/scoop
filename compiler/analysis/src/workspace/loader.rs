use std::{collections::BTreeSet, path::PathBuf};

use super::{
    LoadError, LocalMemberInput, ProjectConfiguration, ProjectDiagnostic, WorkspaceInputs,
    artifacts::ArtifactInputs, files::FileInputs, graph::ProjectBuilder, normalize_path,
    package::PackageInputs,
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
            if let Err(error) = self.configuration(host, overlays, configuration, &mut inputs) {
                self.report(host, overlays, configuration, error, &mut inputs);
            }
        }
        inputs
    }

    fn configuration(
        &mut self,
        host: &mut AnalysisHost,
        overlays: &BTreeSet<PathBuf>,
        configuration: &ProjectConfiguration,
        inputs: &mut WorkspaceInputs,
    ) -> Result<(), LoadError> {
        let package = PackageInputs::new(configuration, host, overlays)?;
        let members = package.members(&configuration.root)?;
        let root = if configuration
            .root
            .file_name()
            .is_some_and(|name| name == "Cone.toml")
        {
            configuration.root.parent().expect("manifest has a parent")
        } else {
            &configuration.root
        };
        let all = normalize_path(root).map_err(|error| LoadError::new(root, error))?
            == members
                .root
                .to_path_buf()
                .map_err(|error| LoadError::new(root, error))?;
        for member in members
            .members
            .iter()
            .filter(|member| all || member.selected)
        {
            let directory = member
                .directory
                .to_path_buf()
                .map_err(|error| LoadError::new(root, error))?;
            let mut member_configuration = configuration.clone();
            member_configuration.root = directory.clone();
            let result =
                package
                    .metadata(&directory, &member.coordinate)
                    .and_then(|mut prepared| {
                        if prepared.resolutions.len() != 1
                            || prepared.resolutions[0].coordinate != member.coordinate
                        {
                            return Err(LoadError::new(
                                &directory,
                                "package manager returned a different member selection",
                            ));
                        }
                        ProjectBuilder::load(
                            self,
                            host,
                            overlays,
                            member_configuration.clone(),
                            prepared.resolutions.remove(0),
                        )
                    });
            match result {
                Ok((project, errors)) => {
                    inputs.projects.push(project);
                    for error in errors {
                        self.report(host, overlays, &member_configuration, error, inputs);
                    }
                }
                Err(error) => {
                    self.report(host, overlays, &member_configuration, error, inputs);
                    match self.files.member_sources(
                        host,
                        overlays,
                        member,
                        &member_configuration,
                        configuration.purpose == scoop_protocol::CompilationPurpose::Test,
                    ) {
                        Ok(sources) => inputs.unresolved_members.push(LocalMemberInput {
                            configuration: configuration.name.clone(),
                            sources,
                        }),
                        Err(error) => {
                            self.report(host, overlays, &member_configuration, error, inputs)
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn report(
        &mut self,
        host: &mut AnalysisHost,
        overlays: &BTreeSet<PathBuf>,
        configuration: &ProjectConfiguration,
        error: LoadError,
        inputs: &mut WorkspaceInputs,
    ) {
        let error = diagnostic(configuration, error);
        // The text is needed for diagnostic positions, never for a second parse.
        let _ = self.files.read(host, overlays, &error.path);
        inputs.diagnostics.push(error);
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
