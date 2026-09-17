//! Foundation-backed authority for exported definition locations.

use std::fmt;

use scoop_hir::{ExportDefinitionSourceSemanticAuthority, ExportDefinitionSourceV1};
use scoop_identity::{ConeIdentity, PersistentSourceContextId};

use super::CanonicalCrossConeHirSurfaceAuthority;

impl ExportDefinitionSourceSemanticAuthority<CrossConeHirDefinitionSourceAuthorityError>
    for CanonicalCrossConeHirSurfaceAuthority<'_>
{
    fn current_cone(&self) -> ConeIdentity {
        self.current
    }

    fn validate_export_definition_source(
        &mut self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), CrossConeHirDefinitionSourceAuthorityError> {
        let origin = source.origin();
        let context_id = origin.context();
        let context = self
            .current_foundation
            .source_context_key(context_id)
            .ok_or(
                CrossConeHirDefinitionSourceAuthorityError::MissingSourceContext {
                    context: context_id,
                },
            )?;
        if context.source() != origin.source() {
            return Err(
                CrossConeHirDefinitionSourceAuthorityError::SourceContextMismatch {
                    context: context_id,
                },
            );
        }
        let record = self
            .current_foundation
            .source_record(origin.source())
            .ok_or(
                CrossConeHirDefinitionSourceAuthorityError::MissingSourceRecord {
                    context: context_id,
                },
            )?;
        let span = origin.span();
        record
            .require_points([span.start_byte(), span.end_byte()])
            .map_err(
                |error| CrossConeHirDefinitionSourceAuthorityError::MissingSourcePoint {
                    context: context_id,
                    byte_offset: error.byte_offset,
                },
            )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CrossConeHirDefinitionSourceAuthorityError {
    MissingSourceContext {
        context: PersistentSourceContextId,
    },
    SourceContextMismatch {
        context: PersistentSourceContextId,
    },
    MissingSourceRecord {
        context: PersistentSourceContextId,
    },
    MissingSourcePoint {
        context: PersistentSourceContextId,
        byte_offset: u64,
    },
}

impl fmt::Display for CrossConeHirDefinitionSourceAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSourceContext { context } => {
                write!(
                    formatter,
                    "source context {context} is absent from the HIR foundation"
                )
            }
            Self::SourceContextMismatch { context } => write!(
                formatter,
                "source context {context} belongs to a different source identity"
            ),
            Self::MissingSourceRecord { context } => write!(
                formatter,
                "source context {context} has no matching HIR source record"
            ),
            Self::MissingSourcePoint {
                context,
                byte_offset,
            } => write!(
                formatter,
                "source context {context} does not declare byte offset {byte_offset}"
            ),
        }
    }
}

impl std::error::Error for CrossConeHirDefinitionSourceAuthorityError {}
