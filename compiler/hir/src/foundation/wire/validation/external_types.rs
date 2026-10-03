//! Shared validation of external nominal references in every HIR foundation.

use super::*;

pub(super) fn validate_external_types(
    types: &[TypeRecord],
    generic_types: &[GenericTypeRecord],
    generated_types: &[GeneratedTypeRecord],
    exact_types: &[ExactTypeRecord],
    external_source_types: &[PersistentTypeId],
    external_generic_types: &[PersistentGenericTypeId],
) -> Result<(), HirFoundationValidationError> {
    let declared_source_types = types
        .iter()
        .map(CborIdentityRecord::id)
        .chain(generated_types.iter().map(CborIdentityRecord::id))
        .collect::<HashSet<_>>();
    let declared_generic_types = generic_types
        .iter()
        .map(CborIdentityRecord::id)
        .collect::<HashSet<_>>();

    for &identity in external_source_types {
        if declared_source_types.contains(&identity)
            || !exact_types.iter().any(|record| {
                matches!(record.key(), ExactTypeKey::Nominal(source) if *source == identity)
            })
        {
            return Err(HirFoundationValidationError::InvalidExternalSourceType(
                identity,
            ));
        }
    }
    for &identity in external_generic_types {
        if declared_generic_types.contains(&identity)
            || !exact_types.iter().any(|record| {
                matches!(
                    record.key(),
                    ExactTypeKey::NominalApplication { origin, .. } if *origin == identity
                )
            })
        {
            return Err(HirFoundationValidationError::InvalidExternalGenericType(
                identity,
            ));
        }
    }
    Ok(())
}
