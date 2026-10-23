use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::PathBuf,
};

use lsp_server::{ErrorCode, ResponseError};
use lsp_types::{DidChangeTextDocumentParams, TextDocumentItem, Uri};
use scoop_analysis::{AnalysisHost, SourceId};
use scoop_source::{PositionEncoding, SourceText};

use crate::convert;

#[derive(Clone)]
pub(crate) struct Document {
    pub source: SourceId,
    pub version: Option<i32>,
    pub path: Option<PathBuf>,
}

#[derive(Default)]
pub(crate) struct Documents {
    pub host: AnalysisHost,
    files: HashMap<Uri, Document>,
}

impl Documents {
    pub fn get(&self, uri: &Uri) -> Option<Document> {
        self.files.get(uri).cloned()
    }

    pub fn open_paths(&self) -> BTreeSet<PathBuf> {
        self.files
            .values()
            .filter(|document| document.version.is_some())
            .filter_map(|document| document.path.clone())
            .collect()
    }

    pub fn open_uris(&self) -> Vec<Uri> {
        self.files
            .iter()
            .filter(|(_, document)| document.version.is_some())
            .map(|(uri, _)| uri.clone())
            .collect()
    }

    pub fn uri_for_path(&self, path: &std::path::Path) -> Option<&Uri> {
        self.files
            .iter()
            .find(|(_, document)| {
                document.version.is_some() && document.path.as_deref() == Some(path)
            })
            .or_else(|| {
                self.files
                    .iter()
                    .find(|(_, document)| document.path.as_deref() == Some(path))
            })
            .map(|(uri, _)| uri)
    }

    pub fn open(&mut self, document: TextDocumentItem) -> Result<(), ResponseError> {
        let text = SourceText::from(document.text);
        let path = file_path(&document.uri)?;
        if let Some(previous) = self.files.get(&document.uri)
            && previous.version.is_some()
        {
            return Err(convert::error(
                ErrorCode::InvalidRequest,
                "document is already open",
            ));
        }
        let source = if let Some(path) = &path {
            self.host.set_file_text(path.clone(), text)
        } else if let Some(previous) = self.files.get(&document.uri) {
            self.host.set_text(previous.source, text);
            previous.source
        } else {
            self.host.add_source(text)
        };
        self.files.insert(
            document.uri,
            Document {
                source,
                version: Some(document.version),
                path,
            },
        );
        Ok(())
    }

    pub fn change(
        &mut self,
        change: DidChangeTextDocumentParams,
        encoding: PositionEncoding,
    ) -> Result<(), ResponseError> {
        let uri = &change.text_document.uri;
        let document = self
            .files
            .get(uri)
            .cloned()
            .ok_or_else(|| convert::error(ErrorCode::InvalidRequest, "document is not open"))?;
        let version = document
            .version
            .ok_or_else(|| convert::error(ErrorCode::InvalidRequest, "document is not open"))?;
        if change.text_document.version <= version {
            return Err(convert::error(
                ErrorCode::InvalidParams,
                "document version must increase",
            ));
        }
        let mut text = self
            .host
            .source(document.source)
            .expect("document owns its source")
            .text
            .clone();
        for edit in change.content_changes {
            if let Some(range) = edit.range {
                let start = convert::offset(&text, range.start, encoding)? as usize;
                let end = convert::offset(&text, range.end, encoding)? as usize;
                if start > end {
                    return Err(convert::error(
                        ErrorCode::InvalidParams,
                        "edit range is reversed",
                    ));
                }
                let mut updated = text.as_str().to_owned();
                updated.replace_range(start..end, &edit.text);
                text = SourceText::from(updated);
            } else {
                text = SourceText::from(edit.text);
            }
        }
        self.host.set_text(document.source, text);
        self.files.insert(
            uri.clone(),
            Document {
                source: document.source,
                version: Some(change.text_document.version),
                path: document.path,
            },
        );
        Ok(())
    }

    pub fn close(&mut self, uri: &Uri) -> Result<(), ResponseError> {
        if let Some(document) = self.files.get_mut(uri) {
            document.version = None;
        }
        self.reload(uri)
    }

    pub fn save(&mut self, uri: &Uri, text: Option<String>) {
        if let Some(document) = self.get(uri)
            && document.version.is_some()
            && let Some(text) = text
        {
            self.host.set_text(document.source, SourceText::from(text));
        }
    }

    pub fn load(&mut self, uri: &Uri) -> Result<Document, ResponseError> {
        if let Some(document) = self.get(uri) {
            return Ok(document);
        }
        if let Some(path) = file_path(uri)?
            && let Some(source) = self.host.file_source(&path)
        {
            let document = Document {
                source,
                version: None,
                path: Some(path),
            };
            self.files.insert(uri.clone(), document.clone());
            return Ok(document);
        }
        self.reload(uri)?;
        self.get(uri)
            .ok_or_else(|| convert::error(ErrorCode::RequestFailed, "source file is unavailable"))
    }

    pub fn reload(&mut self, uri: &Uri) -> Result<(), ResponseError> {
        if self
            .get(uri)
            .is_some_and(|document| document.version.is_some())
        {
            return Ok(());
        }
        let path = file_path(uri)?;
        let loaded = path.as_ref().map(fs::read_to_string);
        match loaded {
            Some(Ok(text)) => {
                let source = self.host.set_file_text(
                    path.clone().expect("loaded a file path"),
                    SourceText::from(text),
                );
                self.files.insert(
                    uri.clone(),
                    Document {
                        source,
                        version: None,
                        path,
                    },
                );
                Ok(())
            }
            other => {
                if let Some(document) = self.files.remove(uri) {
                    self.host.remove_source(document.source);
                }
                match other {
                    Some(Err(error)) if error.kind() != std::io::ErrorKind::NotFound => {
                        Err(convert::error(ErrorCode::RequestFailed, error))
                    }
                    _ => Ok(()),
                }
            }
        }
    }
}

pub(crate) fn file_path(uri: &Uri) -> Result<Option<PathBuf>, ResponseError> {
    let parsed = url::Url::parse(uri.as_str())
        .map_err(|error| convert::error(ErrorCode::InvalidParams, error))?;
    if parsed.scheme() != "file" {
        return Ok(None);
    }
    let path = parsed
        .to_file_path()
        .map_err(|()| convert::error(ErrorCode::InvalidParams, "invalid file URI"))?;
    scoop_analysis::normalize_path(&path)
        .map(Some)
        .map_err(|error| convert::error(ErrorCode::RequestFailed, error))
}
