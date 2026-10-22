use lsp_server::{ErrorCode, ResponseError};
use lsp_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location, Position, Range,
};
use scoop_source::{PositionEncoding, SourceText, Span, TextPosition};

mod location;
pub(crate) use location::{definition_location, location};

pub(crate) fn error(code: ErrorCode, message: impl ToString) -> ResponseError {
    ResponseError {
        code: code as i32,
        message: message.to_string(),
        data: None,
    }
}

pub(crate) fn offset(
    text: &SourceText,
    position: Position,
    encoding: PositionEncoding,
) -> Result<u32, ResponseError> {
    let offset = text
        .line_index()
        .offset(
            TextPosition::new(position.line as usize, position.character as usize),
            encoding,
        )
        .map_err(|err| error(ErrorCode::InvalidParams, err))?;
    u32::try_from(offset).map_err(|err| error(ErrorCode::InvalidParams, err))
}

pub(crate) fn range(
    text: &SourceText,
    span: Span,
    encoding: PositionEncoding,
) -> Result<Range, ResponseError> {
    let index = text.line_index();
    let position = |offset| {
        let point = index
            .position(offset as usize, encoding)
            .map_err(|err| error(ErrorCode::InternalError, err))?;
        Ok(Position::new(point.line as u32, point.column as u32))
    };
    Ok(Range {
        start: position(span.start)?,
        end: position(span.end)?,
    })
}

pub(crate) fn diagnostic(
    text: &SourceText,
    diagnostic: &scoop_source::Diagnostic,
    encoding: PositionEncoding,
    mut note_location: impl FnMut(
        &scoop_source::DiagnosticNote,
    ) -> Result<Option<Location>, ResponseError>,
) -> Result<Option<Diagnostic>, ResponseError> {
    let Some(span) = diagnostic.span else {
        return Ok(None);
    };
    let mut notes = Vec::new();
    for note in &diagnostic.notes {
        if let Some(location) = note_location(note)? {
            notes.push(DiagnosticRelatedInformation {
                location,
                message: note.message.clone(),
            });
        }
    }
    Ok(Some(Diagnostic {
        range: range(text, span, encoding)?,
        severity: Some(match diagnostic.severity {
            scoop_source::DiagnosticSeverity::Error => DiagnosticSeverity::ERROR,
            scoop_source::DiagnosticSeverity::Warning => DiagnosticSeverity::WARNING,
        }),
        source: Some("scoop".to_owned()),
        message: diagnostic.message.clone(),
        related_information: (!notes.is_empty()).then_some(notes),
        ..Diagnostic::default()
    }))
}
