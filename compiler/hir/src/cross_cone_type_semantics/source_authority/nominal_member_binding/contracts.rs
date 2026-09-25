use super::*;
use scoop_identity::DefinitionOriginSubject;

mod properties;
pub(super) use properties::validate as properties;

pub(super) fn prepare(bound: &mut BoundNominalMemberSourcesV1<'_, '_, '_>) -> Result<(), Error> {
    for record in bound.properties.records() {
        let id = record.declaration();

        let key = bound.property_key(id)?;
        access::key(bound, key)?;
        access::origin(
            bound,
            DefinitionOriginSubject::Property(id),
            record.declaration_access(),
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
                    )?;
                }
            }
            NominalSupportPropertyPayloadV1::Const { .. } => {
                let origin = record.declaration_access().definition_origin().origin();

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
        access::origin(bound, subject, record.declaration_access())?;
        if !matches!(
            record.declaration(),
            CallableTemplateOrigin::VariantConstructor(_)
        ) {
            access::key(bound, bound.callable_key(record.declaration())?)?;
        }
    }
    Ok(())
}
