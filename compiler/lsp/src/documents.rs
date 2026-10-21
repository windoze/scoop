use std::{collections::HashMap, fs};

use lsp_server::{ErrorCode, ResponseError};
use lsp_types::{DidChangeTextDocumentParams, TextDocumentItem, Uri};
use scoop_analysis::{AnalysisHost, SourceId};
use scoop_source::{PositionEncoding, SourceText};

use crate::convert;

#[derive(Clone, Copy)]
pub(crate) struct Document {
    pub source: SourceId,
    pub version: Option<i32>,
}

#[derive(Default)]
pub(crate) struct Documents {
    pub host: AnalysisHost,
    files: HashMap<Uri, Document>,
}

impl Documents {
    pub fn get(&self, uri: &Uri) -> Option<Document> {
        self.files.get(uri).copied()
    }

    pub fn open(&mut self, document: TextDocumentItem) -> Result<(), ResponseError> {
        let text = SourceText::from(document.text);
        let source = if let Some(previous) = self.files.get(&document.uri) {
            if previous.version.is_some() {
                return Err(convert::error(
                    ErrorCode::InvalidRequest,
                    "document is already open",
                ));
            }
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
            .copied()
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
        let loaded = url::Url::parse(uri.as_str())
            .ok()
            .and_then(|url| url.to_file_path().ok())
            .map(fs::read_to_string);
        match loaded {
            Some(Ok(text)) => {
                let source = if let Some(document) = self.get(uri) {
                    self.host.set_text(document.source, SourceText::from(text));
                    document.source
                } else {
                    self.host.add_source(SourceText::from(text))
                };
                self.files.insert(
                    uri.clone(),
                    Document {
                        source,
                        version: None,
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
