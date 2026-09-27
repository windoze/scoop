//! Exact definition-origin coverage and source-point validation.

use std::collections::HashMap;

use scoop_identity::{ConeIdentity, DefinitionOriginRecord};
use scoop_wire::WirePath;

use crate::CanonicalHirFoundation;

use super::{
    DefinitionOriginValidationError, HirFoundationValidationError, OriginExpectation,
    OriginRequirements, SourceRecord,
};

#[cfg(test)]
mod tests;

pub(super) fn validate_records(
    artifact: ConeIdentity,
    sources: &[SourceRecord],
    requirements: OriginRequirements<'_>,
    origins: &[DefinitionOriginRecord],
    dependencies: &[&CanonicalHirFoundation],
) -> Result<(), HirFoundationValidationError> {
    let source_path = WirePath::root().field(1);
    let mut source_records = HashMap::new();
    scoop_wire::allocation::try_reserve_map(&mut source_records, sources.len(), &source_path)
        .map_err(HirFoundationValidationError::Resource)?;
    source_records.extend(sources.iter().map(|source| (source.identity(), source)));
    let origin_path = WirePath::root().field(29);
    let mut actual = HashMap::new();
    scoop_wire::allocation::try_reserve_map(&mut actual, origins.len(), &origin_path)
        .map_err(HirFoundationValidationError::Resource)?;
    for record in origins {
        if actual.insert(record.subject(), record).is_some() {
            return Err(DefinitionOriginValidationError::DuplicateSubject {
                subject: record.subject(),
            }
            .into());
        }
        if !requirements.required.contains_key(&record.subject())
            && !requirements.optional.contains_key(&record.subject())
        {
            return Err(DefinitionOriginValidationError::UnexpectedSubject {
                subject: record.subject(),
            }
            .into());
        }
    }
    if let Some(subject) = requirements
        .required
        .keys()
        .filter(|subject| !actual.contains_key(subject))
        .min()
    {
        return Err(DefinitionOriginValidationError::MissingSubject { subject: *subject }.into());
    }

    for record in origins {
        let subject = record.subject();
        let Some(expectation) = requirements
            .required
            .get(&subject)
            .or_else(|| requirements.optional.get(&subject))
        else {
            return Err(DefinitionOriginValidationError::UnexpectedSubject { subject }.into());
        };
        let source = record.origin().source();
        match expectation {
            OriginExpectation::Direct {
                cone,
                source: exact,
            } => {
                if source.cone() != *cone || source.cone() != artifact {
                    return Err(DefinitionOriginValidationError::SourceConeMismatch {
                        subject,
                        expected: *cone,
                        actual: source.cone(),
                    }
                    .into());
                }
                if let Some(expected) = exact
                    && source != *expected
                {
                    let expected = Box::new((*expected).clone());
                    let actual = Box::new((source).clone());
                    return Err(DefinitionOriginValidationError::SourceMismatch {
                        subject,
                        expected,
                        actual,
                    }
                    .into());
                }
            }
            OriginExpectation::SameSource(anchor) => {
                let anchor_record = actual.get(anchor).copied().or_else(|| {
                    dependencies
                        .iter()
                        .find_map(|foundation| foundation.definition_origin(*anchor))
                });
                let Some(anchor_record) = anchor_record else {
                    return Err(
                        DefinitionOriginValidationError::MissingSourceAnchor { subject }.into(),
                    );
                };
                let expected = anchor_record.origin().source();
                if source != expected {
                    let expected = Box::new((expected).clone());
                    let actual = Box::new((source).clone());
                    return Err(DefinitionOriginValidationError::SourceMismatch {
                        subject,
                        expected,
                        actual,
                    }
                    .into());
                }
            }
        }
        let Some(source_record) = source_records.get(source) else {
            return Err(DefinitionOriginValidationError::UnknownSource {
                subject,
                source: Box::new((source).clone()),
            }
            .into());
        };
        let span = record.origin().span();
        if let Err(error) = source_record.require_points([span.start_byte(), span.end_byte()]) {
            return Err(DefinitionOriginValidationError::MissingPoint {
                subject,
                source: Box::new((source).clone()),
                byte_offset: error.byte_offset,
            }
            .into());
        }
    }
    Ok(())
}
