use super::*;
use crate::{
    CanonicalProtectedDeclarationRefsV1, DeclaredVisibilityV1, NestedSourceMemberRefV1,
    NominalInheritanceInterfaceV1, NominalInterfaceRecordV1, ProtectedCallableDeclarationRefV1,
    ProtectedDeclarationRefV1,
};
use scoop_identity::{CallableTemplateOrigin, DefinitionOriginSubject, PropertyOwner};

pub(super) fn validate(
    provider: CheckedSharedTypeFoundationV1<'_>,
    nominal: &NominalInterfaceRecordV1,
    record: &NominalInheritanceInterfaceV1,
    context: &Context<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let required = required(provider.metadata, nominal, context, meter)?;
    contracts::charge_compare(record.protected_members(), &required, meter)?;
    if record.protected_members() != &required {
        return Err(Error::ProtectedMemberInventory(record.owner()));
    }
    Ok(())
}

fn required(
    metadata: SharedTypeMetadataV1<'_>,
    nominal: &NominalInterfaceRecordV1,
    context: &Context<'_>,
    meter: &mut BudgetMeter,
) -> Result<CanonicalProtectedDeclarationRefsV1, Error> {
    let path = WirePath::root();
    let mut references = Vec::new();
    for member in nominal.declaration_details().members().values() {
        match *member {
            NestedSourceMemberRefV1::Function(id) => {
                push_callable(
                    metadata,
                    CallableTemplateOrigin::Function(id),
                    &mut references,
                    meter,
                )?;
            }
            NestedSourceMemberRefV1::GenericFunction(id) => {
                push_callable(
                    metadata,
                    CallableTemplateOrigin::GenericFunction(id),
                    &mut references,
                    meter,
                )?;
            }
            NestedSourceMemberRefV1::Property(id) => {
                let table = metadata.public.property_interfaces();
                contracts::lookup(table.declaration_count(), meter)?;
                let property = table.declaration(PropertyOwner::Property(id)).ok_or(
                    Error::DeclarationMetadata(DefinitionOriginSubject::Property(id)),
                )?;
                if property.declared_visibility() == DeclaredVisibilityV1::Protected {
                    meter.try_reserve_collection_slots(&mut references, 1, &path)?;
                    references.push(ProtectedDeclarationRefV1::Property(id));
                }
                if property.representation() == crate::PropertyRepresentationV1::Const {
                    continue;
                }
                let accessors = property.accessors();
                for id in std::iter::once(accessors.getter()).chain(accessors.setter()) {
                    push_callable(
                        metadata,
                        CallableTemplateOrigin::Accessor(id),
                        &mut references,
                        meter,
                    )?;
                }
            }
        }
    }
    for child in nominal.declaration_details().children().values() {
        contracts::lookup(context.sources.len(), meter)?;
        if context.source(*child)?.access.declared_visibility() == DeclaredVisibilityV1::Protected {
            meter.try_reserve_collection_slots(&mut references, 1, &path)?;
            references.push(ProtectedDeclarationRefV1::NestedNominal(*child));
        }
    }
    meter.charge_work(
        (references.len() as u64).saturating_mul(1 + u64::from(references.len().max(1).ilog2())),
        &path,
    )?;
    CanonicalProtectedDeclarationRefsV1::try_new(references)
        .map_err(|_| Error::InheritanceSource(nominal.declaration()))
}

fn push_callable(
    metadata: SharedTypeMetadataV1<'_>,
    id: CallableTemplateOrigin,
    references: &mut Vec<ProtectedDeclarationRefV1>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    if contracts::callable(metadata, id, meter)?.declared_visibility()
        == DeclaredVisibilityV1::Protected
    {
        let reference = ProtectedCallableDeclarationRefV1::try_new(id)
            .map_err(|_| Error::CallableContract(id))?;
        meter.try_reserve_collection_slots(references, 1, &WirePath::root())?;
        references.push(ProtectedDeclarationRefV1::Callable(reference));
    }
    Ok(())
}
