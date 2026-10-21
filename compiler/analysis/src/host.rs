use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

use scoop_ast::SourceFile;
use scoop_source::{Revision, SourceId, SourceLengthError, SourceText, SourceVersion};
use scoop_syntax::Parse;

use crate::{Cancellation, Cancelled};

#[derive(Debug)]
pub struct SourceInput {
    pub version: SourceVersion,
    pub text: SourceText,
    parsed: OnceLock<Result<Parse<SourceFile>, SourceLengthError>>,
}

impl SourceInput {
    pub fn parse(&self) -> Result<&Parse<SourceFile>, &SourceLengthError> {
        self.parsed
            .get_or_init(|| scoop_parser::parse_recovering(self.text.as_str()))
            .as_ref()
    }
}

#[derive(Debug)]
pub struct AnalysisHost {
    sources: Arc<HashMap<SourceId, Arc<SourceInput>>>,
    revision: Revision,
    next_source: u64,
    cancellation: Cancellation,
}

impl Default for AnalysisHost {
    fn default() -> Self {
        Self {
            sources: Arc::default(),
            revision: Revision(0),
            next_source: 0,
            cancellation: Cancellation::default(),
        }
    }
}

impl AnalysisHost {
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
        }
    }

    pub fn source(&self, id: SourceId) -> Option<&Arc<SourceInput>> {
        self.sources.get(&id)
    }

    pub fn snapshot(&self) -> AnalysisSnapshot {
        AnalysisSnapshot {
            sources: self.sources.clone(),
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
    pub revision: Revision,
    cancellation: Cancellation,
}

impl AnalysisSnapshot {
    pub fn check_cancelled(&self) -> Result<(), Cancelled> {
        self.cancellation.check()
    }

    pub fn source(&self, id: SourceId) -> Result<Option<&Arc<SourceInput>>, Cancelled> {
        self.check_cancelled()?;
        Ok(self.sources.get(&id))
    }
}
