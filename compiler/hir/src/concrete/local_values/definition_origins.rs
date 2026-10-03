//! Persistent definition origins for source-backed LocalConcrete values.

use scoop_identity::{
    DefinitionOrigin as PersistentDefinitionOrigin, DefinitionOriginRecord,
    DefinitionOriginSubject, PersistentLocalValueId, SourceSpan,
};

use super::{LocalValueIdentityError, LocalValueIdentityInputs, LocalValueLocation};

/// Canonical definition origins for the source-backed subset of HIR-first
/// local values. Synthetic locals remain present in `LocalValueIdentities`
/// but cannot acquire a fabricated source record.
#[derive(Clone, Debug, Default)]
pub struct LocalValueDefinitionOrigins {
    pub(super) records: Vec<DefinitionOriginRecord>,
}

impl LocalValueDefinitionOrigins {
    pub fn records(&self) -> &[DefinitionOriginRecord] {
        &self.records
    }

    pub fn get(&self, identity: PersistentLocalValueId) -> Option<&DefinitionOriginRecord> {
        let subject = DefinitionOriginSubject::LocalValue(identity);
        self.records
            .binary_search_by(|record| record.subject().compare_sort_key(subject))
            .ok()
            .map(|index| &self.records[index])
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

pub(super) fn record(
    inputs: &LocalValueIdentityInputs<'_>,
    identity: PersistentLocalValueId,
    origin: crate::DefinitionOrigin,
    location: LocalValueLocation,
) -> Result<DefinitionOriginRecord, LocalValueIdentityError> {
    let file = usize::try_from(origin.file).map_err(|_| {
        LocalValueIdentityError::UnknownDefinitionSource {
            location,
            file: origin.file,
        }
    })?;
    let source =
        inputs
            .source_files
            .get(file)
            .ok_or(LocalValueIdentityError::UnknownDefinitionSource {
                location,
                file: origin.file,
            })?;
    if source.provider != origin.provider {
        return Err(LocalValueIdentityError::DefinitionProviderMismatch { location });
    }
    let context = inputs.source_contexts.get(origin.context).ok_or(
        LocalValueIdentityError::UnknownDefinitionContext {
            location,
            context: origin.context.into_raw().into_u32(),
        },
    )?;
    if context.key().source() != &source.identity {
        return Err(LocalValueIdentityError::DefinitionContextSourceMismatch { location });
    }
    let span = SourceSpan::new(u64::from(origin.span.start), u64::from(origin.span.end))
        .map_err(|error| LocalValueIdentityError::InvalidDefinitionSpan { location, error })?;
    let origin = PersistentDefinitionOrigin::new(source.identity.clone(), span, context.key())
        .map_err(|error| LocalValueIdentityError::InvalidDefinitionOrigin { location, error })?;
    Ok(DefinitionOriginRecord::new(
        DefinitionOriginSubject::LocalValue(identity),
        origin,
    ))
}
