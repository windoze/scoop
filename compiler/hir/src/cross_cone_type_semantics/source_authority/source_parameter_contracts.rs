//! Shared leaf checks for complete and inheritance-only source protocols.
use crate::*;
use scoop_identity::{CallableTemplateOrigin, PersistentGenericTypeId, SignatureTypeKey};
use scoop_wire::{WireError, WirePath};

mod errors;
pub use errors::SourceParameterContractError;
type Error = SourceParameterContractError;

pub(super) fn validate(
    foundation: &BoundTypeFoundationSourcesV1<'_>,
    owner: CallableTemplateOrigin,
    parameters: &[InheritanceSourceParameterV1],
    expected: &CanonicalSourceParameterShapesV1,
    access: &DeclarationAccessSourceV1,
    array: PersistentGenericTypeId,
) -> Result<(), Error> {
    let expected = expected.parameters();
    if parameters.len() != expected.len() {
        return Err(Error::Arity(owner));
    }

    for (index, (parameter, expected)) in parameters.iter().zip(expected).enumerate() {
        let position = u32::try_from(index).map_err(|_| Error::Arity(owner))?;
        let path = WirePath::root().field(2).index(index as u64);

        let shape = parameter.shape();

        if shape.name() != expected.name()
            || !crate::compare_default_signature_reference_targets(
                shape.value_type(),
                expected.value_type(),
                &path,
            )
            .map(|ordering| ordering.is_eq())?
        {
            return Err(Error::Shape { owner, position });
        }
        if matches!(
            parameter.calling_kind(),
            ProtectedParameterCallingKindV1::VarargEmpty
                | ProtectedParameterCallingKindV1::VarargDefault
        ) && !matches!(shape.value_type(),
                SignatureTypeKey::NominalApplication { origin, arguments }
                    if *origin == array && arguments.as_slice().len() == 1)
        {
            return Err(Error::Vararg { owner, position });
        }
        let origin = parameter.definition_origin();
        foundation.validate_origin(origin)?;
        let source = origin.origin().source();

        if source != access.definition_origin().origin().source() {
            return Err(Error::Origin { owner, position });
        }
    }
    Ok(())
}
