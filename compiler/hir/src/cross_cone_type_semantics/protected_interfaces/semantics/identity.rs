use super::{
    NominalSourceCallablePayloadV1, ProtectedCallableSemanticAuthority,
    ProtectedCallableSemanticError as Error, signature::parameters_match,
};
use crate::SourceNominalId;
use scoop_identity::{
    AccessorRole, CallableTemplateOrigin, DuplicateSignatureKey, PersistentConstructorId,
    PersistentFunctionId, PersistentGenericFunctionId, PersistentPropertyAccessorId,
    PersistentPropertyId, PropertyOwner, SignatureTypeKey, SourceDeclarationKey,
};
use scoop_wire::{BudgetMeter, WirePath};

pub(super) fn validate<'a, A: ProtectedCallableSemanticAuthority<E>, E>(
    declaration: CallableTemplateOrigin,
    payload: &NominalSourceCallablePayloadV1,
    owner: &SourceDeclarationKey,
    authority: &'a A,
    meter: &mut BudgetMeter,
) -> Result<&'a SourceDeclarationKey, Error<E>> {
    let key = authority
        .callable_source_key(declaration)
        .map_err(Error::Foundation)?;
    meter
        .charge_sha256(
            scoop_wire::encoded_length(key).map_err(Error::Encoding)?,
            &WirePath::root(),
        )
        .map_err(Error::Resource)?;
    if key.origin() != owner.origin()
        || key.package() != owner.package()
        || key.owners().owners().split_last().map(|(_, outer)| outer)
            != Some(owner.owners().owners())
        || key.duplicate_signature().type_parameter_count() != payload.type_parameters().len_u32()
        || key.duplicate_signature().receiver_is_present()
    {
        return Err(Error::Identity);
    }
    match declaration {
        CallableTemplateOrigin::Function(id) => {
            if PersistentFunctionId::from_source_declaration(key).ok() != Some(id) {
                return Err(Error::Identity);
            }
            function_parameters(payload, key)?;
        }
        CallableTemplateOrigin::GenericFunction(id) => {
            if PersistentGenericFunctionId::from_source_declaration(key).ok() != Some(id) {
                return Err(Error::Identity);
            }
            function_parameters(payload, key)?;
        }
        CallableTemplateOrigin::Constructor(id) => {
            if PersistentConstructorId::from_source_declaration(key).ok() != Some(id) {
                return Err(Error::Identity);
            }
            let DuplicateSignatureKey::Constructor { parameters } = key.duplicate_signature()
            else {
                return Err(Error::Identity);
            };
            if !parameters_match(payload, parameters) {
                return Err(Error::ParameterShape);
            }
            let matches = match (payload.owner(), payload.result()) {
                (SourceNominalId::Concrete(owner), SignatureTypeKey::Nominal(result)) => owner == *result,
                (SourceNominalId::GenericTemplate(owner), SignatureTypeKey::NominalApplication { origin, arguments }) => owner == *origin && arguments.as_slice().iter().enumerate().all(|(index, argument)| matches!(argument, SignatureTypeKey::Binder { depth: 0, index: actual } if *actual as usize == index)),
                _ => false,
            };
            if !matches {
                return Err(Error::Result);
            }
        }
        CallableTemplateOrigin::Accessor(id) => {
            let accessor = authority
                .property_accessor_key(id)
                .map_err(Error::Foundation)?;
            if PersistentPropertyAccessorId::from_key(accessor).ok() != Some(id) {
                return Err(Error::Identity);
            }
            let PropertyOwner::Property(property) = accessor.owner() else {
                return Err(Error::Identity);
            };
            if PersistentPropertyId::from_source_declaration(key).ok() != Some(property) {
                return Err(Error::Identity);
            }
            let value = authority
                .property_value_type(property)
                .map_err(Error::Foundation)?;
            match accessor.role() {
                AccessorRole::Getter => {
                    if !payload.parameters().is_empty() {
                        return Err(Error::ParameterShape);
                    }
                    if payload.result() != value {
                        return Err(Error::Result);
                    }
                }
                AccessorRole::Setter => {
                    if !parameters_match(payload, std::slice::from_ref(value)) {
                        return Err(Error::ParameterShape);
                    }
                    if payload.result()
                        != &SignatureTypeKey::Nominal(
                            authority.unit_type().map_err(Error::Foundation)?,
                        )
                    {
                        return Err(Error::Result);
                    }
                }
            }
        }
        CallableTemplateOrigin::VariantConstructor(_) => return Err(Error::Identity),
    }
    Ok(key)
}

fn function_parameters<E>(
    payload: &NominalSourceCallablePayloadV1,
    key: &SourceDeclarationKey,
) -> Result<(), Error<E>> {
    let DuplicateSignatureKey::Function { parameters, .. } = key.duplicate_signature() else {
        return Err(Error::Identity);
    };
    if !parameters_match(payload, parameters) {
        return Err(Error::ParameterShape);
    }
    Ok(())
}
