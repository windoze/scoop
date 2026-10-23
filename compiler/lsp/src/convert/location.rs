use lsp_server::{ErrorCode, ResponseError};
use lsp_types::Location;
use scoop_analysis::{AnalysisSnapshot, ProjectInput};
use scoop_hir::SemanticSourceLocation;

use crate::{convert, initialize::Settings};

pub(crate) fn location(
    snapshot: &AnalysisSnapshot,
    project: &ProjectInput,
    origin: &SemanticSourceLocation,
    settings: Settings,
) -> Result<Option<Location>, ResponseError> {
    source_location(snapshot, project, origin, settings, false)
}

pub(crate) fn definition_location(
    snapshot: &AnalysisSnapshot,
    project: &ProjectInput,
    origin: &SemanticSourceLocation,
    settings: Settings,
) -> Result<Option<Location>, ResponseError> {
    source_location(snapshot, project, origin, settings, true)
}

fn source_location(
    snapshot: &AnalysisSnapshot,
    project: &ProjectInput,
    origin: &SemanticSourceLocation,
    settings: Settings,
    select_declaration_name: bool,
) -> Result<Option<Location>, ResponseError> {
    let Some(file) = project
        .cones
        .values()
        .filter_map(|cone| cone.source())
        .flat_map(|cone| &cone.sources)
        .find(|file| file.identity == origin.source)
    else {
        return Ok(None);
    };
    let Some(source) = snapshot
        .source(file.source)
        .map_err(|_| convert::error(ErrorCode::RequestCanceled, "analysis revision changed"))?
    else {
        return Ok(None);
    };
    let url = url::Url::from_file_path(&file.path).map_err(|_| {
        convert::error(
            ErrorCode::RequestFailed,
            "source path cannot be represented as a URI",
        )
    })?;
    let uri = url
        .as_str()
        .parse()
        .map_err(|error| convert::error(ErrorCode::RequestFailed, error))?;
    let span = if select_declaration_name {
        source
            .declaration_name_span(origin.span)
            .map_err(|error| convert::error(ErrorCode::RequestFailed, error))?
            .unwrap_or(origin.span)
    } else {
        origin.span
    };
    Ok(Some(Location {
        uri,
        range: convert::range(&source.text, span, settings.encoding)?,
    }))
}
