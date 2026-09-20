use super::super::foundation::binding::sources::AccessAuthority;
use super::*;
use scoop_identity::DefinitionOriginSubject;

pub(super) fn validate(
    foundation: &BoundTypeFoundationSourcesV1<'_>,
    key: &SourceDeclarationKey,
    record: &NominalSupportConstructorInterfaceV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let declaration = record.declaration();
    let access = record.declaration_access();
    let origin = access.definition_origin();
    let entries = foundation.source().entries();
    let path = WirePath::root();
    NominalRepresentationSupportV1::charge_source_key_resources(key, meter, &path)?;
    if key.origin() != entries.provider {
        return Err(invalid(
            declaration,
            "constructor belongs to another provider",
        ));
    }
    query(
        foundation
            .foundation
            .as_canonical()
            .counts()
            .definition_origins,
        meter,
    )?;
    meter.charge_work(
        origin.origin().source().logical_path().as_str().len() as u64 + 1,
        &path,
    )?;
    if foundation
        .foundation
        .definition_origin(DefinitionOriginSubject::Constructor(declaration))
        .map(|record| record.origin())
        != Some(origin.origin())
    {
        return Err(Error::Origin(declaration));
    }
    let owners = access.lexical_owners();
    meter.check_semantic_depth(owners.len() as u64 + 1, &path)?;
    meter.charge_work(
        (owners.len() as u64 + 1)
            .saturating_pow(2)
            .saturating_mul(64),
        &path,
    )?;
    for owner in owners {
        query(entries.sources.records().len(), meter)?;
        query(entries.sources.records().len(), meter)?;
        NominalRepresentationSupportV1::charge_source_key_resources(
            foundation.nominal_key(*owner)?,
            meter,
            &path,
        )?;
    }
    meter.charge_work(
        (origin.origin().source().logical_path().as_str().len() as u64).saturating_mul(
            u64::from(entries.definition_sources.sources().len().max(1).ilog2())
                + owners.len() as u64
                + 1,
        ),
        &path,
    )?;
    access
        .validate_for_declaration(key, &mut AccessAuthority(foundation))
        .map_err(|error| invalid(declaration, error))?;
    Ok(())
}
