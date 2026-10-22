use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

use scoop_parser::ParsedFile;
use scoop_source::{Revision, SourceId, SourceLengthError, SourceText, SourceVersion};

use crate::{Cancellation, Cancelled};

#[derive(Debug)]
pub struct SourceInput {
    pub version: SourceVersion,
    pub text: SourceText,
    parsed: OnceLock<Result<ParsedFile, SourceLengthError>>,
    pub(crate) symbols: OnceLock<Vec<crate::SourceSymbol>>,
    pub(crate) documentation: OnceLock<scoop_ast::SourceDocumentation>,
}

impl SourceInput {
    pub fn parse(&self) -> Result<&ParsedFile, &SourceLengthError> {
        self.parsed
            .get_or_init(|| scoop_parser::parse_recovering(self.text.as_str()))
            .as_ref()
    }
}

#[derive(Debug)]
pub struct AnalysisHost {
    sources: Arc<HashMap<SourceId, Arc<SourceInput>>>,
    files: Arc<HashMap<PathBuf, SourceId>>,
    workspace: Arc<crate::WorkspaceInputs>,
    revision: Revision,
    next_source: u64,
    cancellation: Cancellation,
}

impl Default for AnalysisHost {
    fn default() -> Self {
        Self {
            sources: Arc::default(),
            files: Arc::default(),
            workspace: Arc::default(),
            revision: Revision(0),
            next_source: 0,
            cancellation: Cancellation::default(),
        }
    }
}

impl AnalysisHost {
    pub fn file_source(&self, path: &Path) -> Option<SourceId> {
        self.files.get(path).copied()
    }

    pub fn set_file_text(&mut self, path: PathBuf, text: SourceText) -> SourceId {
        if let Some(id) = self.file_source(&path) {
            self.set_text(id, text);
            return id;
        }
        let id = self.add_source(text);
        Arc::make_mut(&mut self.files).insert(path, id);
        id
    }

    pub fn set_workspace(&mut self, workspace: crate::WorkspaceInputs) {
        if *self.workspace != workspace {
            self.advance();
            self.workspace = Arc::new(workspace);
        }
    }

    pub fn workspace(&self) -> &crate::WorkspaceInputs {
        &self.workspace
    }

    pub fn revision(&self) -> Revision {
        self.revision
    }

    pub fn add_source(&mut self, text: SourceText) -> SourceId {
        let id = SourceId(self.next_source);
        self.next_source = self
            .next_source
            .checked_add(1)
            .expect("source handles exhausted");
        self.replace_source(id, text);
        id
    }

    /// Returns false for a released source. Identical text reuses its current parse.
    pub fn set_text(&mut self, id: SourceId, text: SourceText) -> bool {
        let Some(previous) = self.sources.get(&id) else {
            return false;
        };
        if previous.text != text {
            self.replace_source(id, text);
        }
        true
    }

    pub fn remove_source(&mut self, id: SourceId) {
        if self.sources.contains_key(&id) {
            self.advance();
            Arc::make_mut(&mut self.sources).remove(&id);
            Arc::make_mut(&mut self.files).retain(|_, source| *source != id);
        }
    }

    pub fn source(&self, id: SourceId) -> Option<&Arc<SourceInput>> {
        self.sources.get(&id)
    }

    pub fn snapshot(&self) -> AnalysisSnapshot {
        AnalysisSnapshot {
            sources: self.sources.clone(),
            files: self.files.clone(),
            workspace: self.workspace.clone(),
            revision: self.revision,
            cancellation: self.cancellation.clone(),
        }
    }

    fn replace_source(&mut self, id: SourceId, text: SourceText) {
        self.advance();
        let version = SourceVersion {
            source: id,
            revision: self.revision,
        };
        Arc::make_mut(&mut self.sources).insert(
            id,
            Arc::new(SourceInput {
                version,
                text,
                parsed: OnceLock::new(),
                symbols: OnceLock::new(),
                documentation: OnceLock::new(),
            }),
        );
    }

    fn advance(&mut self) {
        self.cancellation.cancel();
        self.cancellation = Cancellation::default();
        self.revision.0 = self
            .revision
            .0
            .checked_add(1)
            .expect("analysis revisions exhausted");
    }
}

#[derive(Debug, Clone)]
pub struct AnalysisSnapshot {
    sources: Arc<HashMap<SourceId, Arc<SourceInput>>>,
    files: Arc<HashMap<PathBuf, SourceId>>,
    pub workspace: Arc<crate::WorkspaceInputs>,
    pub revision: Revision,
    cancellation: Cancellation,
}

impl AnalysisSnapshot {
    pub fn file_source(&self, path: &Path) -> Result<Option<SourceId>, Cancelled> {
        self.check_cancelled()?;
        Ok(self.files.get(path).copied())
    }

    pub fn check_cancelled(&self) -> Result<(), Cancelled> {
        self.cancellation.check()
    }

    pub fn source(&self, id: SourceId) -> Result<Option<&Arc<SourceInput>>, Cancelled> {
        self.check_cancelled()?;
        Ok(self.sources.get(&id))
    }
}
