//! Uses the shared artifact query for every actual nested body occurrence.
use super::*;

pub(super) fn validate<'p, 's, 'a, 'f>(
    current: &BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f>,
    dependencies: &[&BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f>],
    nested: &DefaultSourceNestedCallablesV1<'_>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), Error> {
    for occurrence in nested.occurrences() {
        let origin = occurrence.definition_origin();
        let source = sources::provider(
            current,
            dependencies,
            origin.origin().source().cone(),
            meter,
            path,
        )?;
        let foundation = source.members().nominals.foundation;
        DefaultTargetIdentityQueriesV1::new(
            source.provider(),
            foundation.foundation,
            foundation.identities,
        )
        .validate_nested_callable_identity(occurrence.descriptor(), origin, meter, path)
        .map_err(|error| match error {
            DefaultNestedIdentityValidationError::Resource(error) => Error::Resource(error),
            DefaultNestedIdentityValidationError::Foundation(error) => {
                Error::Nominal(Box::new(NominalSourceBindingError::Foundation(error)))
            }
            DefaultNestedIdentityValidationError::Identity { identity, reason } => {
                Error::NestedIdentity { identity, reason }
            }
        })?;
    }
    Ok(())
}
