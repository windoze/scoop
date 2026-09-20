use super::*;
use scoop_identity::{AccessorRole, PropertyOwner};

pub(super) fn validate(
    bound: &BoundInheritanceProtectedCallableSourcesV1<'_, '_>,
    record: &ProtectedCallableInterfaceV1,
    meter: &mut BudgetMeter,
) -> Result<(), InheritanceProtectedCallableBindingError> {
    use InheritanceProtectedCallableBindingError as Error;
    let path = WirePath::root();
    query(bound.keys.len(), meter)?;
    let key = bound.callable_key(record.declaration())?;
    NominalRepresentationSupportV1::charge_source_key_resources(key, meter, &path)?;
    let entries = bound.foundation.source().entries();
    if key.origin() != entries.provider {
        return Err(Error::Owner(record.declaration()));
    }
    let access = record.declaration_access();
    let owners = access.lexical_owners().len() as u64;
    meter.check_semantic_depth(owners + 1, &path)?;
    meter.charge_work((owners + 1).saturating_pow(2).saturating_mul(64), &path)?;
    meter.charge_work(
        (owners + 1).saturating_mul(u64::from(entries.sources.records().len().max(1).ilog2()) + 1),
        &path,
    )?;
    meter.charge_work(
        (access
            .definition_origin()
            .origin()
            .source()
            .logical_path()
            .as_str()
            .len() as u64)
            .saturating_mul(
                u64::from(entries.definition_sources.sources().len().max(1).ilog2()) + owners + 1,
            ),
        &path,
    )?;
    query(
        bound
            .foundation
            .foundation
            .as_canonical()
            .counts()
            .definition_origins,
        meter,
    )?;
    let subject = subject(record.declaration())?;
    if bound
        .foundation
        .foundation
        .definition_origin(subject)
        .map(|r| r.origin())
        != Some(access.definition_origin().origin())
    {
        return Err(Error::DefinitionOrigin(subject));
    }
    if let CallableTemplateOrigin::Accessor(id) = record.declaration() {
        query(entries.accessor_keys.values().len(), meter)?;
        let accessor = bound.foundation.accessor_key(id)?;
        let PropertyOwner::Property(property) = accessor.owner() else {
            return Err(Error::Declaration(record.declaration()));
        };
        query(bound.properties.records().len(), meter)?;
        let property = bound
            .properties
            .get(property)
            .ok_or(Error::MissingProperty(property))?;
        let hir_access = match accessor.role() {
            AccessorRole::Getter => property.declaration_access(),
            AccessorRole::Setter => {
                let NominalSupportPropertyPayloadV1::Runtime { interface } = property.payload()
                else {
                    return Err(Error::MissingProperty(property.declaration()));
                };
                let ProtectedPropertyMutabilityV1::ReadWrite {
                    setter,
                    setter_access,
                } = interface.mutability()
                else {
                    return Err(Error::AccessorAccess(id));
                };
                if *setter != id {
                    return Err(Error::AccessorAccess(id));
                }
                setter_access
            }
        };
        if hir_access.declared_visibility() != access.declared_visibility()
            || hir_access.lexical_owners() != access.lexical_owners()
        {
            return Err(Error::AccessorAccess(id));
        }
    }
    Ok(())
}
