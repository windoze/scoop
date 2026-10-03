//! Source locations checked against the actual provider's foundation.

use scoop_identity::{
    ConeIdentity, DefinitionOrigin, PersistentSourceContextId, SourceIdentity, SourceSpan,
};

use super::CanonicalHirFoundation;
use crate::ExportDefinitionSourceV1;

impl CanonicalHirFoundation {
    /// Checks source/context membership and declared byte positions. The caller
    /// selects this already validated foundation from the actual provider graph.
    pub fn validate_definition_source_location(
        &self,
        provider: ConeIdentity,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), DefinitionSourceLocationValidationError> {
        self.validate_definition_origin_location(provider, source.origin())
    }

    pub fn validate_definition_origin_location(
        &self,
        provider: ConeIdentity,
        origin: &DefinitionOrigin,
    ) -> Result<(), DefinitionSourceLocationValidationError> {
        self.validate_source_location(provider, origin.source(), origin.span(), origin.context())
    }
}

impl CanonicalHirFoundation {
    pub(super) fn validate_source_location(
        &self,
        provider: ConeIdentity,
        source: &SourceIdentity,
        span: SourceSpan,
        context_id: PersistentSourceContextId,
    ) -> Result<(), DefinitionSourceLocationValidationError> {
        use DefinitionSourceLocationValidationError as Error;

        if source.cone() != provider {
            return Err(Error::Provider {
                expected: provider,
                actual: source.cone(),
            });
        }
        let context = self
            .source_contexts
            .binary_search_by_key(&context_id, |record| record.id())
            .ok()
            .map(|index| self.source_contexts[index].key())
            .ok_or(Error::MissingSourceContext {
                context: context_id,
            })?;
        if context.source() != source {
            return Err(Error::SourceContextMismatch {
                context: context_id,
            });
        }
        let record = self
            .sources
            .binary_search_by(|record| record.identity().cmp(source))
            .ok()
            .map(|index| &self.sources[index])
            .ok_or(Error::MissingSourceRecord {
                context: context_id,
            })?;

        record
            .require_points([span.start_byte(), span.end_byte()])
            .map_err(|error| Error::MissingSourcePoint {
                context: context_id,
                byte_offset: error.byte_offset,
            })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DefinitionSourceLocationValidationError {
    Provider {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
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

impl std::fmt::Display for DefinitionSourceLocationValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Provider { expected, actual } => write!(
                f,
                "source origin belongs to provider {actual}, expected {expected}"
            ),
            Self::MissingSourceContext { context } => write!(
                f,
                "source context {context} is absent from the HIR foundation"
            ),
            Self::SourceContextMismatch { context } => write!(
                f,
                "source context {context} belongs to a different source identity"
            ),
            Self::MissingSourceRecord { context } => write!(
                f,
                "source context {context} has no matching HIR source record"
            ),
            Self::MissingSourcePoint {
                context,
                byte_offset,
            } => write!(
                f,
                "source context {context} does not declare byte offset {byte_offset}"
            ),
        }
    }
}
impl std::error::Error for DefinitionSourceLocationValidationError {}
