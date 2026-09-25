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
) -> Result<(), InheritanceConstructorBindingError> {
    use InheritanceConstructorBindingError as Error;
    let declaration = record.declaration();
    let entries = foundation.source().entries();
    let path = WirePath::root();

    if key.origin() != entries.provider {
        return Err(Error::ForeignDeclaration(declaration));
    }

    let ExactTypeKey::Nominal(nominal) = foundation.exact_type_key(owner)? else {
        return Err(Error::Owner(declaration));
    };
    if record.payload().owner() != SourceNominalId::Concrete(*nominal) {
        return Err(Error::Owner(declaration));
    }

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

    if foundation
        .foundation
        .definition_origin(DefinitionOriginSubject::Constructor(declaration))
        .map(|record| record.origin())
        != Some(origin.origin())
    {
        return Err(Error::DefinitionOrigin(declaration));
    }

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

    if parameters.len() != actual.len() {
        return Err(Error::Signature(declaration));
    }
    for (expected, actual) in parameters.iter().zip(actual) {
        if !crate::compare_default_signature_reference_targets(expected, actual.value_type(), &path)
            .map(|ordering| ordering.is_eq())?
        {
            return Err(Error::Signature(declaration));
        }
    }
    if !crate::compare_default_signature_reference_targets(
        &SignatureTypeKey::Nominal(*nominal),
        record.payload().result(),
        &path,
    )
    .map(|ordering| ordering.is_eq())?
    {
        return Err(Error::Signature(declaration));
    }
    Ok(())
}
