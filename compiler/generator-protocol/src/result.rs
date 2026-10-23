use crate::{MethodId, Reference};
use scoop_identity::{ConeIdentity, SourceIdentity};
use scoop_source::{Diagnostic, DiagnosticSeverity, Span};
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;
pub const QUERY_ERROR: i32 = -32000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Generated {
    pub bodies: Vec<GeneratedBody>,
    pub diagnostics: Vec<GeneratorDiagnostic>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneratedBody {
    pub method: MethodId,
    pub source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceReference {
    pub cone: Reference<ConeIdentity>,
    pub path: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ByteRange {
    pub start_byte: u32,
    pub end_byte: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceLocation {
    pub source: SourceReference,
    pub span: ByteRange,
}

impl SourceLocation {
    pub fn new(source: &SourceIdentity, span: Span) -> Self {
        Self {
            source: SourceReference {
                cone: source.cone().into(),
                path: source.logical_path().as_str().to_owned(),
            },
            span: ByteRange {
                start_byte: span.start,
                end_byte: span.end,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GeneratorSeverity {
    Error,
    Warning,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneratorDiagnostic {
    pub severity: GeneratorSeverity,
    pub message: String,
    pub location: Option<SourceLocation>,
    pub notes: Vec<GeneratorNote>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneratorNote {
    pub message: String,
    pub location: SourceLocation,
}

impl GeneratorDiagnostic {
    pub fn from_diagnostic(
        diagnostic: &Diagnostic,
        fallback: &scoop_hir::SemanticSourceLocation,
    ) -> Self {
        Self {
            severity: match diagnostic.severity {
                DiagnosticSeverity::Error => GeneratorSeverity::Error,
                DiagnosticSeverity::Warning => GeneratorSeverity::Warning,
            },
            message: diagnostic.message.clone(),
            location: diagnostic.span.map(|span| {
                SourceLocation::new(
                    diagnostic.source.as_deref().unwrap_or(&fallback.source),
                    span,
                )
            }),
            notes: diagnostic
                .notes
                .iter()
                .map(|note| GeneratorNote {
                    message: note.message.clone(),
                    location: SourceLocation::new(
                        note.source.as_deref().unwrap_or(&fallback.source),
                        note.span,
                    ),
                })
                .collect(),
        }
    }
}
