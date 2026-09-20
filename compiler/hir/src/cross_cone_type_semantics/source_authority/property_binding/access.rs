use super::super::foundation::binding::sources::AccessAuthority;
use super::*;

pub(super) fn validate<'a>(
    foundation: &BoundTypeFoundationSourcesV1<'_>,
    declaration: PersistentPropertyId,
    key: &'a SourceDeclarationKey,
    subject: DefinitionOriginSubject,
    access: &'a DeclarationAccessSourceV1,
    meter: &mut BudgetMeter,
) -> Result<CheckedDeclarationAccessSourceV1<'a>, InheritancePropertyBindingError> {
    use InheritancePropertyBindingError as Error;
    let path = WirePath::root();
    let entries = foundation.source().entries();
    NominalRepresentationSupportV1::charge_source_key_resources(key, meter, &path)?;
    if key.origin() != entries.provider {
        return Err(Error::Owner(declaration));
    }
    query(
        foundation
            .foundation
            .as_canonical()
            .counts()
            .definition_origins,
        meter,
    )?;
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
    if foundation
        .foundation
        .definition_origin(subject)
        .map(|record| record.origin())
        != Some(access.definition_origin().origin())
    {
        return Err(Error::DefinitionOrigin(subject));
    }
    access
        .validate_for_declaration(key, &mut AccessAuthority(foundation))
        .map_err(|error| Error::Access {
            declaration,
            reason: error.to_string(),
        })
}

pub(super) fn accessor(
    foundation: &BoundTypeFoundationSourcesV1<'_>,
    property: PersistentPropertyId,
    accessor: PersistentPropertyAccessorId,
    role: AccessorRole,
    meter: &mut BudgetMeter,
) -> Result<(), InheritancePropertyBindingError> {
    query(
        foundation.source().entries().accessor_keys.values().len(),
        meter,
    )?;
    let key = foundation.accessor_key(accessor)?;
    if key.owner() != PropertyOwner::Property(property) || key.role() != role {
        return Err(InheritancePropertyBindingError::Accessor(accessor));
    }
    Ok(())
}
