//! Shared leaf checks for complete and inheritance-only source protocols.
use crate::*;
use scoop_identity::{CallableTemplateOrigin, PersistentGenericTypeId, SignatureTypeKey};
use scoop_wire::{BudgetMeter, WireError, WirePath};

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
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let expected = expected.parameters();
    if parameters.len() != expected.len() {
        return Err(Error::Arity(owner));
    }
    meter.check_table_entries(parameters.len() as u64, &WirePath::root())?;
    for (index, (parameter, expected)) in parameters.iter().zip(expected).enumerate() {
        let position = u32::try_from(index).map_err(|_| Error::Arity(owner))?;
        let path = WirePath::root().field(2).index(index as u64);
        meter.check_semantic_depth(5, &path)?;
        meter.charge_nodes(1, &path)?;
        let shape = parameter.shape();
        let name = shape.name().as_str();
        meter.check_semantic_leaf(name.len() as u64, &path)?;
        meter.charge_work(
            name.len() as u64 + expected.name().as_str().len() as u64,
            &path,
        )?;
        if shape.name() != expected.name()
            || !NominalRepresentationSupportV1::signature_types_match_metered(
                shape.value_type(),
                expected.value_type(),
                3,
                meter,
                &path,
            )?
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
        foundation.validate_origin(origin, meter, &path.clone().field(3))?;
        let source = origin.origin().source();
        meter.charge_work(source.logical_path().as_str().len() as u64, &path)?;
        if source != access.definition_origin().origin().source() {
            return Err(Error::Origin { owner, position });
        }
    }
    Ok(())
}
