//! Source locations checked against the actual provider's foundation.

use scoop_identity::{
    ConeIdentity, DefinitionOrigin, PersistentSourceContextId, SourceIdentity, SourceSpan,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::OdrFreeHirFoundation;
use crate::ExportDefinitionSourceV1;

impl OdrFreeHirFoundation {
    /// Checks source/context membership and declared byte positions. The caller
    /// selects this already validated foundation from the actual provider graph.
    pub fn validate_definition_source_location(
        &self,
        provider: ConeIdentity,
        source: &ExportDefinitionSourceV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), DefinitionSourceLocationValidationError> {
        self.validate_definition_origin_location(provider, source.origin(), meter, path)
    }

    pub fn validate_definition_origin_location(
        &self,
        provider: ConeIdentity,
        origin: &DefinitionOrigin,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), DefinitionSourceLocationValidationError> {
        self.validate_source_location(
            provider,
            origin.source(),
            origin.span(),
            origin.context(),
            meter,
            path,
        )
    }

    pub(super) fn validate_source_location(
        &self,
        provider: ConeIdentity,
        source: &SourceIdentity,
        span: SourceSpan,
        context_id: PersistentSourceContextId,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), DefinitionSourceLocationValidationError> {
        use DefinitionSourceLocationValidationError as Error;
        let bytes = source.logical_path().as_str().len() as u64;
        let counts = self.as_canonical().counts();
        meter.check_semantic_leaf(bytes, path)?;
        meter.charge_work(
            bytes.saturating_mul(u64::from(counts.sources.max(1).ilog2()) + 2),
            path,
        )?;
        meter.charge_work(u64::from(counts.source_contexts.max(1).ilog2()) + 1, path)?;
        if source.cone() != provider {
            return Err(Error::Provider {
                expected: provider,
                actual: source.cone(),
            });
        }
        let context = self
            .source_context_key(context_id)
            .ok_or(Error::MissingSourceContext {
                context: context_id,
            })?;
        if context.source() != source {
            return Err(Error::SourceContextMismatch {
                context: context_id,
            });
        }
        let record = self
            .source_record(source)
            .ok_or(Error::MissingSourceRecord {
                context: context_id,
            })?;
        meter.charge_work(
            2 * (u64::from(record.points().len().max(1).ilog2()) + 1),
            path,
        )?;
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
    Resource(WireError),
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

impl From<WireError> for DefinitionSourceLocationValidationError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for DefinitionSourceLocationValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
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
impl std::error::Error for DefinitionSourceLocationValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            _ => None,
        }
    }
}
