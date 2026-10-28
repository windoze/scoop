//! Host input observations shared by all HIR probes in one compilation.

use std::collections::BTreeMap;
use std::sync::Mutex;

use scoop_hir::{CompileTimeEnvironmentName, CompileTimeInputHost};

mod directories;
mod files;
mod paths;
mod patterns;
type FileValues = BTreeMap<
    (scoop_identity::SourceIdentity, scoop_hir::CompileTimePath),
    Result<files::ReadFile, String>,
>;

type EnvironmentValues = BTreeMap<CompileTimeEnvironmentName, Result<Option<String>, String>>;
type DirectoryValues = BTreeMap<
    (
        scoop_identity::SourceIdentity,
        scoop_hir::CompileTimePath,
        scoop_hir::CompileTimeDirectoryFilter,
    ),
    Result<directories::ReadDirectory, scoop_hir::CompileTimeDirectoryError>,
>;

#[derive(Default)]
pub struct CompileTimeInputs {
    environment: Mutex<EnvironmentValues>,
    sources: Option<files::Sources>,
    files: Mutex<FileValues>,
    directories: Mutex<DirectoryValues>,
}

impl CompileTimeInputHost for CompileTimeInputs {
    fn directory(
        &self,
        source: &scoop_identity::SourceIdentity,
        path: &scoop_hir::CompileTimePath,
        filter: &scoop_hir::CompileTimeDirectoryFilter,
    ) -> Result<std::sync::Arc<[scoop_hir::CompileTimeFile]>, scoop_hir::CompileTimeDirectoryError>
    {
        let sources = self.sources.as_ref().ok_or_else(|| {
            scoop_hir::CompileTimeDirectoryError::Input(
                "include call has no physical definition source".into(),
            )
        })?;
        self.directories
            .lock()
            .expect("directory input cache is not poisoned")
            .entry((source.clone(), path.clone(), filter.clone()))
            .or_insert_with(|| directories::read(sources, source, path, filter))
            .as_ref()
            .map(|directory| directory.files.clone())
            .map_err(Clone::clone)
    }

    fn file(
        &self,
        source: &scoop_identity::SourceIdentity,
        path: &scoop_hir::CompileTimePath,
    ) -> Result<std::sync::Arc<[u8]>, String> {
        let sources = self
            .sources
            .as_ref()
            .ok_or("include call has no physical definition source")?;
        self.files
            .lock()
            .expect("file input cache is not poisoned")
            .entry((source.clone(), path.clone()))
            .or_insert_with(|| sources.read(source, path))
            .as_ref()
            .map(|file| file.bytes.clone())
            .map_err(Clone::clone)
    }

    fn environment(&self, name: &CompileTimeEnvironmentName) -> Result<Option<String>, String> {
        let mut values = self
            .environment
            .lock()
            .expect("environment input cache is not poisoned");
        values
            .entry(name.clone())
            .or_insert_with(|| match std::env::var(name.as_str()) {
                Ok(value) => Ok(Some(value)),
                Err(std::env::VarError::NotPresent) => Ok(None),
                Err(std::env::VarError::NotUnicode(_)) => Err(format!(
                    "compile-time environment variable `{}` is not valid Unicode",
                    name.as_str(),
                )),
            })
            .clone()
    }
}

impl CompileTimeInputs {
    /// Editor analyses retain this host while bodies are checked on demand.
    pub fn inputs_current(&self) -> bool {
        if self
            .environment
            .lock()
            .expect("environment input cache is not poisoned")
            .values()
            .any(Result::is_err)
            || self
                .files
                .lock()
                .expect("file input cache is not poisoned")
                .values()
                .any(Result::is_err)
            || self
                .directories
                .lock()
                .expect("directory input cache is not poisoned")
                .values()
                .any(Result::is_err)
        {
            return false;
        }
        compiler_inputs_current(&self.compiler_inputs())
    }

    pub fn with_sources(
        mut self,
        root: std::path::PathBuf,
        sources: impl IntoIterator<Item = (scoop_identity::SourceIdentity, std::path::PathBuf)>,
    ) -> Self {
        self.sources = Some(files::Sources {
            root,
            files: sources.into_iter().collect(),
        });
        self
    }

    pub fn compiler_inputs(&self) -> Vec<scoop_protocol::CompilerInputV1> {
        self.environment
            .lock()
            .expect("environment input cache is not poisoned")
            .iter()
            .filter_map(|(name, value)| {
                value
                    .as_ref()
                    .ok()
                    .map(|value| scoop_protocol::EnvironmentInputV1 {
                        name: name.as_str().to_owned(),
                        value: value.clone(),
                    })
            })
            .map(scoop_protocol::CompilerInputV1::Environment)
            .chain(
                self.files
                    .lock()
                    .expect("file input cache is not poisoned")
                    .values()
                    .filter_map(|file| {
                        file.as_ref()
                            .ok()
                            .map(|file| scoop_protocol::CompilerInputV1::File(file.input.clone()))
                    }),
            )
            .chain(
                self.directories
                    .lock()
                    .expect("directory input cache is not poisoned")
                    .values()
                    .filter_map(|directory| {
                        directory.as_ref().ok().map(|directory| {
                            scoop_protocol::CompilerInputV1::Directory(directory.input.clone())
                        })
                    }),
            )
            .collect()
    }
}

pub fn compiler_inputs_current(inputs: &[scoop_protocol::CompilerInputV1]) -> bool {
    inputs.iter().all(|input| match input {
        scoop_protocol::CompilerInputV1::File(input) => files::current(input),
        scoop_protocol::CompilerInputV1::Directory(input) => directories::current(input),
        scoop_protocol::CompilerInputV1::Environment(input) => match std::env::var(&input.name) {
            Ok(value) => input.value.as_ref() == Some(&value),
            Err(std::env::VarError::NotPresent) => input.value.is_none(),
            Err(std::env::VarError::NotUnicode(_)) => false,
        },
    })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod file_tests;

#[cfg(test)]
mod directory_tests;
