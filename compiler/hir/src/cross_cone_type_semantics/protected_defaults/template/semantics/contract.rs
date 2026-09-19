use super::*;
use crate::{CanonicalBooleanV1, compare_default_signature_reference_targets};
use scoop_identity::Effect;
use scoop_wire::WirePath;

pub(super) fn validate<A, E>(
    template: &ProtectedDefaultTemplateV1,
    owner: ProtectedDefaultOwnerSourceV1<'_>,
    protocol: CheckedProtectedSourceProtocolV1<'_>,
    authority: &mut A,
    meter: &mut BudgetMeter,
) -> Result<DefaultTemplateProviderShapeV1, ProtectedDefaultTemplateContractSemanticError<E>>
where
    A: NominalInterfaceShapeAuthority<E> + ProtectedDefaultRootSemanticAuthority<E>,
{
    use ProtectedDefaultTemplateContractSemanticError as Error;
    let key = template.key();
    let path = WirePath::root();
    meter.charge_work(1, &path).map_err(Error::Resource)?;
    if owner.declaration() != key.owner() {
        return Err(Error::SourceOwner);
    }
    if protocol.record().owner() != key.owner() {
        return Err(Error::ProtocolOwner);
    }
    let parameters = protocol.record().parameters().parameters();
    let position = key.parameter_position() as usize;
    let current = parameters.get(position).ok_or(Error::ParameterOutOfRange {
        position: key.parameter_position(),
        arity: parameters.len(),
    })?;
    if current.calling().template() != Some(key) {
        return Err(Error::ParameterTemplate);
    }
    let owner_shape = owner::shape(owner, authority, meter)?;
    meter
        .charge_work(template.definition_path().segments().len() as u64, &path)
        .map_err(Error::Resource)?;
    let provider = root::validate(
        key,
        template.definition_root(),
        template.definition_path(),
        authority,
        meter,
    )
    .map_err(Error::DefinitionRoot)?;
    if template.definition_root().declaration() == key.owner() && provider != owner_shape.binders {
        return Err(Error::ProviderOwnerShape);
    }
    types::validate(template, provider, &owner_shape.scope, authority, meter)?;
    template
        .receiver()
        .validate_expected_semantics_metered(
            owner_shape.receiver.as_ref(),
            template.locals(),
            provider,
            template.type_parameters(),
            meter,
            &path,
        )
        .map_err(Error::Receiver)?;
    template
        .value_parameters()
        .validate_prefix_types_metered(
            parameters[..position]
                .iter()
                .map(|parameter| parameter.value_type()),
            template.locals(),
            provider,
            template.type_parameters(),
            meter,
            &path,
        )
        .map_err(Error::ValueParameters)?;
    let mapped = template
        .type_parameters()
        .substitute_provider_type_metered(provider, template.result(), meter, &path)
        .map_err(Error::ResultSubstitution)?;
    if !compare_default_signature_reference_targets(&mapped, current.value_type(), meter, &path)
        .map_err(Error::Resource)?
        .is_eq()
    {
        return Err(Error::ResultMismatch);
    }
    let suspend =
        CanonicalBooleanV1::from(owner.payload().effects().execution() == Effect::Suspend);
    if template.allows_suspend() != suspend {
        return Err(Error::SuspendPermission);
    }
    Ok(provider)
}
