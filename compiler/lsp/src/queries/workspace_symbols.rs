use lsp_server::{ErrorCode, ResponseError};
use lsp_types::{Location, SymbolInformation};
use scoop_analysis::{AnalysisSnapshot, Cancellation, SymbolQueryError};
use serde_json::Value;

use crate::{convert, initialize::Settings};

#[allow(deprecated)]
pub(crate) fn workspace_symbols(
    snapshot: &AnalysisSnapshot,
    query: &str,
    settings: Settings,
    cancellation: &Cancellation,
) -> Result<Value, ResponseError> {
    let symbols = snapshot
        .workspace_symbols(query, || cancellation.check().is_err())
        .map_err(|error| match error {
            SymbolQueryError::Cancelled => super::semantic::cancelled(),
            SymbolQueryError::MissingSource(source) => convert::error(
                ErrorCode::RequestFailed,
                format!("source {} is unavailable", source.0),
            ),
            SymbolQueryError::SourceLength(error) => {
                convert::error(ErrorCode::RequestFailed, error)
            }
        })?;
    let mut results = Vec::with_capacity(symbols.len());
    for entry in symbols {
        let symbol = entry.symbol;
        let Some(kind) = super::symbols::symbol_kind(symbol.declaration.kind, symbol.parent_kind)
        else {
            continue;
        };
        let source = snapshot
            .source(entry.file.source)
            .map_err(|_| super::semantic::cancelled())?
            .ok_or_else(|| convert::error(ErrorCode::RequestFailed, "source is unavailable"))?;
        let uri = url::Url::from_file_path(&entry.file.path)
            .map_err(|_| convert::error(ErrorCode::RequestFailed, "source path has no file URI"))?;
        results.push(SymbolInformation {
            name: symbol.name.clone(),
            kind,
            tags: None,
            deprecated: None,
            location: Location {
                uri: uri
                    .as_str()
                    .parse()
                    .map_err(|error| convert::error(ErrorCode::RequestFailed, error))?,
                range: convert::range(&source.text, symbol.name_span, settings.encoding)?,
            },
            container_name: symbol.container.clone(),
        });
    }
    super::json(results)
}
