use super::super::foundation::binding::sources::AccessAuthority;
use super::*;
use scoop_identity::{
    DefinitionOriginSubject, DuplicateSignatureKey, ExactTypeKey, SignatureTypeKey,
    SourceDeclarationKind,
};

pub(super) fn validate(
    foundation: &BoundTypeFoundationSourcesV1<'_>,
    owner: PersistentExactTypeId,
    key: &SourceDeclarationKey,
    record: &NominalSupportConstructorInterfaceV1,
    meter: &mut BudgetMeter,
) -> Result<(), InheritanceConstructorBindingError> {
    use InheritanceConstructorBindingError as Error;
    let declaration = record.declaration();
    let entries = foundation.source().entries();
    let path = WirePath::root();
    NominalRepresentationSupportV1::charge_source_key_resources(key, meter, &path)?;
    if key.origin() != entries.provider {
        return Err(Error::ForeignDeclaration(declaration));
    }
    charge_queries(1, entries.exact_keys.values().len(), meter)?;
    let ExactTypeKey::Nominal(nominal) = foundation.exact_type_key(owner)? else {
        return Err(Error::Owner(declaration));
    };
    if record.payload().owner() != SourceNominalId::Concrete(*nominal) {
        return Err(Error::Owner(declaration));
    }
    charge_queries(1, entries.sources.records().len(), meter)?;
    if !matches!(
        foundation
            .nominal_key(record.payload().owner())?
            .declaration_kind(),
        SourceDeclarationKind::Class | SourceDeclarationKind::Struct
    ) {
        return Err(Error::Owner(declaration));
    }
    let access = record.declaration_access();
    if !matches!(
        access.declared_visibility(),
        DeclaredVisibilityV1::Public | DeclaredVisibilityV1::Protected
    ) {
        return Err(Error::Visibility(declaration));
    }
    let origin = access.definition_origin();
    charge_queries(
        1,
        foundation
            .foundation
            .as_canonical()
            .counts()
            .definition_origins,
        meter,
    )?;
    if foundation
        .foundation
        .definition_origin(DefinitionOriginSubject::Constructor(declaration))
        .map(|record| record.origin())
        != Some(origin.origin())
    {
        return Err(Error::DefinitionOrigin(declaration));
    }
    let owners = access.lexical_owners().len();
    meter.check_semantic_depth(owners as u64 + 1, &path)?;
    meter.charge_work(
        (owners as u64 + 1).saturating_pow(2).saturating_mul(64),
        &path,
    )?;
    charge_queries(
        owners.saturating_mul(2),
        entries.sources.records().len(),
        meter,
    )?;
    meter.charge_work(
        (origin.origin().source().logical_path().as_str().len() as u64).saturating_mul(
            u64::from(entries.definition_sources.sources().len().max(1).ilog2())
                + owners as u64
                + 1,
        ),
        &path,
    )?;
    access
        .validate_for_declaration(key, &mut AccessAuthority(foundation))
        .map_err(|error| Error::Access {
            declaration,
            reason: error.to_string(),
        })?;
    let DuplicateSignatureKey::Constructor { parameters } = key.duplicate_signature() else {
        return Err(Error::Signature(declaration));
    };
    let actual = record.payload().parameters().parameters();
    meter.check_table_entries(actual.len() as u64, &path)?;
    meter.charge_work(actual.len() as u64, &path)?;
    if parameters.len() != actual.len() {
        return Err(Error::Signature(declaration));
    }
    for (expected, actual) in parameters.iter().zip(actual) {
        if !NominalRepresentationSupportV1::signature_types_match_metered(
            expected,
            actual.value_type(),
            3,
            meter,
            &path,
        )? {
            return Err(Error::Signature(declaration));
        }
    }
    if !NominalRepresentationSupportV1::signature_types_match_metered(
        &SignatureTypeKey::Nominal(*nominal),
        record.payload().result(),
        3,
        meter,
        &path,
    )? {
        return Err(Error::Signature(declaration));
    }
    Ok(())
}
