//! Closure-wide provenance checks for source metadata copied by consumers.

use std::fmt;

use scoop_hir::OdrFreeHirFoundation;
use scoop_identity::{ConeIdentity, PersistentSourceContextId, SourceIdentity};

pub(super) fn validate_imported_source_metadata<'provider>(
    artifact: ConeIdentity,
    foundation: &OdrFreeHirFoundation,
    mut provider: impl FnMut(ConeIdentity) -> Option<&'provider OdrFreeHirFoundation>,
) -> Result<(), CrossConeSourceProvenanceError> {
    for record in foundation.source_records() {
        let provider_identity = record.identity().cone();
        if provider_identity == artifact {
            continue;
        }
        let provider_foundation = provider(provider_identity).ok_or_else(|| {
            CrossConeSourceProvenanceError::MissingProvider {
                source: record.identity().clone(),
            }
        })?;
        let expected = provider_foundation
            .source_record(record.identity())
            .ok_or_else(|| CrossConeSourceProvenanceError::MissingSourceRecord {
                source: record.identity().clone(),
            })?;
        if record != expected {
            return Err(CrossConeSourceProvenanceError::SourceRecordMismatch {
                source: record.identity().clone(),
            });
        }
    }

    for (context, key) in foundation.source_context_records() {
        let provider_identity = key.source().cone();
        if provider_identity == artifact {
            continue;
        }
        let provider_foundation = provider(provider_identity).ok_or(
            CrossConeSourceProvenanceError::MissingContextProvider {
                context,
                provider: provider_identity,
            },
        )?;
        let expected = provider_foundation.source_context_key(context).ok_or(
            CrossConeSourceProvenanceError::MissingSourceContext {
                context,
                provider: provider_identity,
            },
        )?;
        if key != expected {
            return Err(CrossConeSourceProvenanceError::SourceContextMismatch {
                context,
                provider: provider_identity,
            });
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrossConeSourceProvenanceError {
    MissingProvider {
        source: SourceIdentity,
    },
    MissingSourceRecord {
        source: SourceIdentity,
    },
    SourceRecordMismatch {
        source: SourceIdentity,
    },
    MissingContextProvider {
        context: PersistentSourceContextId,
        provider: ConeIdentity,
    },
    MissingSourceContext {
        context: PersistentSourceContextId,
        provider: ConeIdentity,
    },
    SourceContextMismatch {
        context: PersistentSourceContextId,
        provider: ConeIdentity,
    },
}

impl fmt::Display for CrossConeSourceProvenanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingProvider { source } => write!(
                formatter,
                "source {source:?} does not belong to the artifact or its dependency closure"
            ),
            Self::MissingSourceRecord { source } => write!(
                formatter,
                "source {source:?} is absent from its provider's HIR foundation"
            ),
            Self::SourceRecordMismatch { source } => write!(
                formatter,
                "source {source:?} does not match its provider's canonical HIR source record"
            ),
            Self::MissingContextProvider { context, provider } => write!(
                formatter,
                "source context {context} names unavailable provider {provider}"
            ),
            Self::MissingSourceContext { context, provider } => write!(
                formatter,
                "source context {context} is absent from provider {provider}"
            ),
            Self::SourceContextMismatch { context, provider } => write!(
                formatter,
                "source context {context} does not match provider {provider}"
            ),
        }
    }
}

impl std::error::Error for CrossConeSourceProvenanceError {}

#[cfg(test)]
mod tests {
    use scoop_hir::{CanonicalHirFoundation, OdrFreeHirFoundation, SourceRecord};
    use scoop_identity::{
        CborIdentityRecord, ConeCoordinate, NormalizedSourcePath, SourceContextKey, SourceIdentity,
    };

    use super::*;

    #[test]
    fn accepts_exact_dependency_source_metadata() {
        let provider = ConeCoordinate::new("dev.example", "provider", "1.0.0")
            .unwrap()
            .identity()
            .unwrap();
        let source = SourceIdentity::new(
            provider,
            NormalizedSourcePath::new("src/api.scoop").unwrap(),
        )
        .unwrap();
        let record = SourceRecord::from_utf8(source.clone(), "abc", [0, 3]).unwrap();
        let context = CborIdentityRecord::from_key(SourceContextKey::File {
            source: source.clone(),
        })
        .unwrap();
        let provider_foundation = foundation(vec![record.clone()], vec![context.clone()]);
        let consumer_foundation = foundation(vec![record], vec![context]);

        validate_imported_source_metadata(
            ConeCoordinate::new("dev.example", "consumer", "1.0.0")
                .unwrap()
                .identity()
                .unwrap(),
            &consumer_foundation,
            |identity| (identity == provider).then_some(&provider_foundation),
        )
        .unwrap();
    }

    #[test]
    fn rejects_dependency_source_metadata_that_differs_from_its_provider() {
        let provider = ConeCoordinate::new("dev.example", "provider", "1.0.0")
            .unwrap()
            .identity()
            .unwrap();
        let source = SourceIdentity::new(
            provider,
            NormalizedSourcePath::new("src/api.scoop").unwrap(),
        )
        .unwrap();
        let provider_record = SourceRecord::from_utf8(source.clone(), "abc", [0, 3]).unwrap();
        let forged_record = SourceRecord::from_utf8(source.clone(), "abd", [0, 3]).unwrap();
        let provider_foundation = foundation(vec![provider_record], Vec::new());
        let consumer_foundation = foundation(vec![forged_record], Vec::new());

        assert_eq!(
            validate_imported_source_metadata(
                ConeCoordinate::new("dev.example", "consumer", "1.0.0")
                    .unwrap()
                    .identity()
                    .unwrap(),
                &consumer_foundation,
                |identity| (identity == provider).then_some(&provider_foundation),
            ),
            Err(CrossConeSourceProvenanceError::SourceRecordMismatch { source })
        );
    }

    fn foundation(
        sources: Vec<SourceRecord>,
        contexts: Vec<
            CborIdentityRecord<
                scoop_identity::PersistentSourceContextId,
                scoop_identity::SourceContextKey,
            >,
        >,
    ) -> OdrFreeHirFoundation {
        let mut foundation = CanonicalHirFoundation::empty();
        foundation.set_sources(sources).unwrap();
        foundation.set_source_contexts(contexts).unwrap();
        OdrFreeHirFoundation::try_new(foundation).unwrap()
    }
}
