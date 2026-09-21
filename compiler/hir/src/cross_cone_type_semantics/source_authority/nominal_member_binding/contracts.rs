use super::*;
use scoop_identity::DefinitionOriginSubject;

mod properties;
pub(super) use properties::validate as properties;

pub(super) fn prepare(
    bound: &mut BoundNominalMemberSourcesV1<'_, '_, '_>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let path = WirePath::root();
    binding_keys::charge_map(bound.properties.records().len(), meter, &path)?;
    for record in bound.properties.records() {
        let id = record.declaration();
        query(bound.property_keys.len(), meter)?;
        let key = bound.property_key(id)?;
        access::key(bound, key, meter)?;
        access::origin(
            bound,
            DefinitionOriginSubject::Property(id),
            record.declaration_access(),
            meter,
        )?;
        match record.payload() {
            NominalSupportPropertyPayloadV1::Runtime { interface } => {
                if let ProtectedPropertyMutabilityV1::ReadWrite {
                    setter,
                    setter_access,
                } = interface.mutability()
                {
                    access::origin(
                        bound,
                        DefinitionOriginSubject::PropertyAccessor(*setter),
                        setter_access,
                        meter,
                    )?;
                }
            }
            NominalSupportPropertyPayloadV1::Const { .. } => {
                let bytes =
                    scoop_wire::encoded_length(key).map_err(|e| Error::Identity(e.to_string()))?;
                meter.check_semantic_leaf(bytes, &path)?;
                meter.charge_owned_bytes(bytes, &path)?;
                meter.charge_collection_slots(key.owners().owners().len() as u64, &path)?;
                let origin = record.declaration_access().definition_origin().origin();
                meter.charge_owned_bytes(
                    origin.source().logical_path().as_str().len() as u64,
                    &path,
                )?;
                bound.constants.insert(
                    id,
                    ConstPropertyDeclarationSourceV1::new(key.clone(), origin.clone()),
                );
            }
        }
    }
    for record in bound.callables.records() {
        let subject = match record.declaration() {
            CallableTemplateOrigin::Function(id) => DefinitionOriginSubject::Function(id),
            CallableTemplateOrigin::GenericFunction(id) => {
                DefinitionOriginSubject::GenericFunction(id)
            }
            CallableTemplateOrigin::Accessor(id) => DefinitionOriginSubject::PropertyAccessor(id),
            CallableTemplateOrigin::VariantConstructor(id) => {
                DefinitionOriginSubject::EnumVariant(id)
            }
            other => return Err(Error::MissingCallableKey(other)),
        };
        access::origin(bound, subject, record.declaration_access(), meter)?;
        if !matches!(
            record.declaration(),
            CallableTemplateOrigin::VariantConstructor(_)
        ) {
            query(bound.callable_keys.len(), meter)?;
            access::key(bound, bound.callable_key(record.declaration())?, meter)?;
        }
    }
    Ok(())
}
