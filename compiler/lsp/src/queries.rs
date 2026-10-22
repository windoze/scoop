use lsp_server::{ErrorCode, Request, ResponseError};
use lsp_types::{
    DocumentSymbolParams, FoldingRange, FoldingRangeKind, FoldingRangeParams, GotoDefinitionParams,
    HoverParams, Position, SelectionRange, SelectionRangeParams, Uri,
};
use scoop_analysis::{Cancellation, FoldKind, SourceInput};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::{convert, initialize::Settings};

mod completion;
mod definition;
mod hover;
mod semantic;
mod symbols;
pub(crate) use completion::completion;
pub(crate) use definition::definition;
pub(crate) use hover::hover;

#[derive(Clone)]
pub(crate) enum QueryKind {
    Syntax(SyntaxQuery),
    Hover(Position),
    Definition(Position),
    Completion(Position),
}

#[derive(Clone)]
pub(crate) enum SyntaxQuery {
    Symbols,
    Folds,
    Selection(Vec<Position>),
}

#[derive(Clone)]
pub(crate) struct Query {
    pub uri: Uri,
    pub kind: QueryKind,
}

pub(crate) fn params<T: DeserializeOwned>(value: Value) -> Result<T, ResponseError> {
    serde_json::from_value(value).map_err(|err| convert::error(ErrorCode::InvalidParams, err))
}

pub(crate) fn json(value: impl Serialize) -> Result<Value, ResponseError> {
    serde_json::to_value(value).map_err(|err| convert::error(ErrorCode::InternalError, err))
}

impl Query {
    pub fn from_request(request: &Request) -> Result<Self, ResponseError> {
        match request.method.as_str() {
            "textDocument/completion" => {
                let params: lsp_types::CompletionParams = params(request.params.clone())?;
                Ok(Self {
                    uri: params.text_document_position.text_document.uri,
                    kind: QueryKind::Completion(params.text_document_position.position),
                })
            }
            "textDocument/documentSymbol" => {
                let params: DocumentSymbolParams = params(request.params.clone())?;
                Ok(Self {
                    uri: params.text_document.uri,
                    kind: QueryKind::Syntax(SyntaxQuery::Symbols),
                })
            }
            "textDocument/foldingRange" => {
                let params: FoldingRangeParams = params(request.params.clone())?;
                Ok(Self {
                    uri: params.text_document.uri,
                    kind: QueryKind::Syntax(SyntaxQuery::Folds),
                })
            }
            "textDocument/selectionRange" => {
                let params: SelectionRangeParams = params(request.params.clone())?;
                Ok(Self {
                    uri: params.text_document.uri,
                    kind: QueryKind::Syntax(SyntaxQuery::Selection(params.positions)),
                })
            }
            "textDocument/hover" => {
                let params: HoverParams = params(request.params.clone())?;
                Ok(Self {
                    uri: params.text_document_position_params.text_document.uri,
                    kind: QueryKind::Hover(params.text_document_position_params.position),
                })
            }
            "textDocument/definition" => {
                let params: GotoDefinitionParams = params(request.params.clone())?;
                Ok(Self {
                    uri: params.text_document_position_params.text_document.uri,
                    kind: QueryKind::Definition(params.text_document_position_params.position),
                })
            }
            _ => Err(convert::error(
                ErrorCode::MethodNotFound,
                format!("unknown request {}", request.method),
            )),
        }
    }
}

impl SyntaxQuery {
    pub fn run(
        &self,
        uri: &Uri,
        source: &SourceInput,
        settings: Settings,
        cancellation: &Cancellation,
    ) -> Result<Value, ResponseError> {
        let parsed = source
            .parse()
            .map_err(|err| convert::error(ErrorCode::RequestFailed, err))?;
        cancellation
            .check()
            .map_err(|_| convert::error(ErrorCode::RequestCanceled, "request cancelled"))?;
        match self {
            Self::Symbols => {
                symbols::document_symbols(parsed.declarations(), &source.text, uri, settings)
            }
            Self::Folds => {
                let mut ranges = Vec::new();
                for fold in scoop_analysis::folding_ranges(&parsed.syntax()) {
                    let range = convert::range(&source.text, fold.span, settings.encoding)?;
                    let end_line = if settings.line_folding_only && range.end.character == 0 {
                        range.end.line.saturating_sub(1)
                    } else {
                        range.end.line
                    };
                    if range.start.line >= end_line {
                        continue;
                    }
                    ranges.push(FoldingRange {
                        start_line: range.start.line,
                        end_line,
                        start_character: (!settings.line_folding_only)
                            .then_some(range.start.character),
                        end_character: (!settings.line_folding_only).then_some(range.end.character),
                        kind: Some(match fold.kind {
                            FoldKind::Comment => FoldingRangeKind::Comment,
                            FoldKind::Imports => FoldingRangeKind::Imports,
                            FoldKind::Region => FoldingRangeKind::Region,
                        }),
                        collapsed_text: None,
                    });
                }
                if let Some(limit) = settings.folding_limit {
                    ranges.truncate(limit as usize);
                }
                json(ranges)
            }
            Self::Selection(positions) => {
                let syntax = parsed.syntax();
                let mut results = Vec::with_capacity(positions.len());
                for position in positions {
                    cancellation.check().map_err(|_| {
                        convert::error(ErrorCode::RequestCanceled, "request cancelled")
                    })?;
                    let offset = convert::offset(&source.text, *position, settings.encoding)?;
                    let mut parent = None;
                    for span in scoop_analysis::selection_ranges(&syntax, offset)
                        .into_iter()
                        .rev()
                    {
                        parent = Some(Box::new(SelectionRange {
                            range: convert::range(&source.text, span, settings.encoding)?,
                            parent,
                        }));
                    }
                    results.push(parent.expect("a valid position includes the root range"));
                }
                json(results)
            }
        }
    }
}
