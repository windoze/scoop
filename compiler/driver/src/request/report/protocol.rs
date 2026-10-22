use std::fmt;

use scoop_ast::{Diagnostic, DiagnosticSeverity, Span};
use scoop_identity::SourceIdentity;
use scoop_protocol::{
    DiagnosticNoteV1, DiagnosticOriginV1, DiagnosticSeverityV1, ProtocolByteSpan,
    ProtocolConeIdentity, StructuredDiagnosticV1,
};

use super::CurrentConeDiagnosticSet;
use crate::{CurrentConeHirStageError, CurrentConeProductionFailure, SingleConeProductionError};

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub struct DiagnosticMappingError(pub String);

impl fmt::Display for DiagnosticMappingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for DiagnosticMappingError {}

fn mapping(error: impl fmt::Display) -> DiagnosticMappingError {
    DiagnosticMappingError(error.to_string())
}

fn source_origin(
    source: Option<&SourceIdentity>,
    span: Span,
    source_text: impl FnOnce(&SourceIdentity) -> Option<String>,
) -> Result<DiagnosticOriginV1, DiagnosticMappingError> {
    let source =
        source.ok_or_else(|| mapping("internal error: diagnostic source index is unresolved"))?;
    let cone = ProtocolConeIdentity::from_array(*source.cone().as_array());
    let logical_path = source.logical_path().clone();
    let span =
        ProtocolByteSpan::new(u64::from(span.start), u64::from(span.end)).map_err(mapping)?;
    Ok(match source_text(source) {
        Some(text) => DiagnosticOriginV1::SourceTextSpan {
            cone,
            logical_path,
            span,
            text,
        },
        None => DiagnosticOriginV1::SemanticSourceSpan {
            cone,
            logical_path,
            span,
        },
    })
}

fn convert<'a>(
    diagnostic: &'a Diagnostic,
    code: &str,
    source_at: impl Fn(usize) -> Option<&'a SourceIdentity>,
    source_text: impl Fn(&SourceIdentity) -> Option<String>,
) -> Result<StructuredDiagnosticV1, DiagnosticMappingError> {
    let origin = match diagnostic.span {
        Some(span) => source_origin(
            diagnostic
                .source
                .as_deref()
                .or_else(|| source_at(diagnostic.file)),
            span,
            &source_text,
        )?,
        None => DiagnosticOriginV1::None,
    };
    let notes = diagnostic
        .notes
        .iter()
        .map(|note| {
            DiagnosticNoteV1::new(
                note.message.clone(),
                source_origin(
                    note.source.as_deref().or_else(|| source_at(note.file)),
                    note.span,
                    &source_text,
                )?,
            )
            .map_err(mapping)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let severity = match diagnostic.severity {
        DiagnosticSeverity::Error => DiagnosticSeverityV1::Error,
        DiagnosticSeverity::Warning => DiagnosticSeverityV1::Warning,
    };
    StructuredDiagnosticV1::new(
        severity,
        if severity.is_error() {
            code
        } else {
            "SCOOPC_COMPILER_WARNING"
        }
        .to_owned(),
        diagnostic.message.clone(),
        origin,
        notes,
    )
    .map_err(mapping)
}

impl CurrentConeDiagnosticSet {
    pub fn structured(&self) -> Result<Vec<StructuredDiagnosticV1>, DiagnosticMappingError> {
        self.structured_with_code("SCOOPC_HIR_ERROR")
    }

    fn structured_with_code(
        &self,
        code: &str,
    ) -> Result<Vec<StructuredDiagnosticV1>, DiagnosticMappingError> {
        self.diagnostics
            .iter()
            .map(|diagnostic| {
                convert(
                    diagnostic,
                    code,
                    |index| self.sources.get(index).map(|source| &source.identity),
                    |identity| {
                        self.sources
                            .iter()
                            .find(|source| source.in_memory && &source.identity == identity)
                            .and_then(|source| source.source_text.as_ref())
                            .map(|text| text.as_str().to_owned())
                    },
                )
            })
            .collect()
    }
}

impl SingleConeProductionError {
    pub fn structured_diagnostics(
        &self,
    ) -> Result<Vec<StructuredDiagnosticV1>, DiagnosticMappingError> {
        use crate::CurrentConeSourceStageError;
        let mut diagnostics = match self {
            Self::Sources(CurrentConeSourceStageError::Package(diagnostics)) => {
                diagnostics.structured_with_code("SCOOPC_PACKAGE_ERROR")?
            }
            Self::Sources(CurrentConeSourceStageError::Parser(error)) => match error.as_ref() {
                scoop_parser::ParseCurrentConeError::Diagnostics(diagnostics) => diagnostics
                    .iter()
                    .map(|parsed| {
                        convert(
                            parsed.diagnostic(),
                            "SCOOPC_PARSE_ERROR",
                            |_| Some(parsed.identity()),
                            |_| None,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                _ => vec![tool_error(self)?],
            },
            Self::Production(error) => match error.cause() {
                CurrentConeProductionFailure::Hir(CurrentConeHirStageError::Lowering(
                    diagnostics,
                )) => diagnostics.structured_with_code("SCOOPC_HIR_ERROR")?,
                _ => vec![tool_error(self)?],
            },
            _ => vec![tool_error(self)?],
        };
        if let Some(warnings) = self.warnings() {
            diagnostics.extend(warnings.structured()?);
        }
        Ok(diagnostics)
    }
}

fn tool_error(
    error: &SingleConeProductionError,
) -> Result<StructuredDiagnosticV1, DiagnosticMappingError> {
    let dependency = match error {
        SingleConeProductionError::Validation(
            crate::SingleConeDependencyValidationError::ExplicitDependencies(error),
        )
        | SingleConeProductionError::Preflight(crate::SingleConePreflightError::Dependencies(
            error,
        )) => Some(error),
        SingleConeProductionError::Production(error) => match error.cause() {
            CurrentConeProductionFailure::Dependencies(error) => Some(error),
            _ => None,
        },
        _ => None,
    };
    let origin = match dependency.and_then(|error| error.artifact_location()) {
        Some((path, member)) => DiagnosticOriginV1::artifact_path(
            scoop_protocol::HostPathCarrier::from_path(path).map_err(mapping)?,
            member,
        )
        .map_err(mapping)?,
        None => match error {
            SingleConeProductionError::Preflight(crate::SingleConePreflightError::SingleFile(
                error,
            )) => {
                let path =
                    scoop_protocol::HostPathCarrier::from_path(error.path()).map_err(mapping)?;
                if let Some(span) = error.span() {
                    DiagnosticOriginV1::HostPathSpan {
                        path,
                        span: ProtocolByteSpan::new(span.start as u64, span.end as u64)
                            .map_err(mapping)?,
                    }
                } else {
                    DiagnosticOriginV1::HostPath { path }
                }
            }
            SingleConeProductionError::Sources(crate::CurrentConeSourceStageError::Discovery(
                error,
            )) => DiagnosticOriginV1::HostPath {
                path: scoop_protocol::HostPathCarrier::from_path(error.path()).map_err(mapping)?,
            },
            SingleConeProductionError::Sources(crate::CurrentConeSourceStageError::SingleFile(
                error,
            )) => DiagnosticOriginV1::HostPath {
                path: scoop_protocol::HostPathCarrier::from_path(error.path()).map_err(mapping)?,
            },
            _ => DiagnosticOriginV1::None,
        },
    };
    StructuredDiagnosticV1::new(
        DiagnosticSeverityV1::Error,
        "SCOOPC_BUILD_FAILED".to_owned(),
        error.to_string(),
        origin,
        Vec::new(),
    )
    .map_err(mapping)
}
