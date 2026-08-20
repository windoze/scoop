use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, CoreBuiltinNominal, DeclarationScope, DefinitionOwnerChain,
    NonEmptyVec, PackagePath, PersistentGenericTypeId, SourceDeclarationKey, SourceDeclarationSite,
    SourceNominalKind,
};

use super::*;

#[test]
fn nominal_origin_retains_the_definition_provider_and_rejects_structural_ownership() {
    let identity = SourceExactTypeIdentity::checked(
        Type::Unit,
        unit(),
        SourceExactTypeOrigin::Nominal(CoreBuiltinNominal::Unit.declaration_key().origin()),
    )
    .unwrap();
    assert_eq!(
        identity.owner(),
        SourceExactTypeOwner::Cone(ConeIdentity::CORE)
    );
    assert_eq!(
        SourceExactTypeIdentity::checked(Type::Unit, unit(), SourceExactTypeOrigin::Structural)
            .unwrap_err(),
        SourceExactTypeIdentityError::OriginKindMismatch,
    );
    let exact = CborIdentityRecord::from_key(ExactTypeKey::Tuple(NonEmptyVec::from_first(
        unit().id(),
        [],
    )))
    .unwrap();
    let ty = Type::Tuple(vec![Type::Unit]);
    let structural = SourceExactTypeIdentity::checked(
        ty.clone(),
        exact.clone(),
        SourceExactTypeOrigin::Structural,
    )
    .unwrap();
    assert_eq!(structural.owner(), SourceExactTypeOwner::Structural);
    assert_eq!(
        SourceExactTypeIdentity::checked(
            ty,
            exact,
            SourceExactTypeOrigin::Nominal(ConeIdentity::SINGLE_FILE)
        )
        .unwrap_err(),
        SourceExactTypeIdentityError::OriginKindMismatch,
    );
}

fn unit() -> SourceExactTypeRecord {
    CborIdentityRecord::from_key(ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap()
}

fn application() -> (SourceExactTypeRecord, SourceNominalSpecializationRecord) {
    let declaration = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("Box").unwrap(),
        SourceNominalKind::Class,
        1,
    );
    let origin = PersistentGenericTypeId::from_source_declaration(&declaration).unwrap();
    let arguments = NonEmptyVec::from_first(unit().id(), []);
    let exact = CborIdentityRecord::from_key(ExactTypeKey::NominalApplication {
        origin,
        arguments: arguments.clone(),
    })
    .unwrap();
    let specialization =
        CborIdentityRecord::from_key(SpecializationKey::Nominal { origin, arguments }).unwrap();
    (exact, specialization)
}

#[test]
fn nominal_application_keeps_its_exact_specialization_group() {
    let (exact, specialization) = application();
    let identity = SourceExactTypeIdentity::checked(
        Type::Class(crate::ClassId::from_raw(3_u32.into())),
        exact.clone(),
        SourceExactTypeOrigin::NominalApplication(specialization.clone()),
    )
    .unwrap();
    let relation = SourceExactTypeIdentities::checked(vec![identity]).unwrap();

    let found = relation.get_by_identity(exact.id()).unwrap();
    assert_eq!(found.identity_record().id(), exact.id());
    assert_eq!(
        found.nominal_specialization().unwrap().id(),
        specialization.id()
    );
}

#[test]
fn nominal_application_requires_its_specialization_group() {
    let (exact, _) = application();
    assert_eq!(
        SourceExactTypeIdentity::checked(
            Type::Unit,
            exact,
            SourceExactTypeOrigin::Nominal(ConeIdentity::SINGLE_FILE)
        )
        .unwrap_err(),
        SourceExactTypeIdentityError::OriginKindMismatch
    );
}

#[test]
fn relation_rejects_duplicate_mir_types_and_exact_identities() {
    let exact = unit();
    let first = SourceExactTypeIdentity::checked(
        Type::Unit,
        exact.clone(),
        SourceExactTypeOrigin::Nominal(ConeIdentity::CORE),
    )
    .unwrap();
    let same_type = SourceExactTypeIdentity::checked(
        Type::Unit,
        CborIdentityRecord::from_key(ExactTypeKey::Nominal(
            CoreBuiltinNominal::Any.identity_record().id(),
        ))
        .unwrap(),
        SourceExactTypeOrigin::Nominal(ConeIdentity::CORE),
    )
    .unwrap();
    assert_eq!(
        SourceExactTypeIdentities::checked(vec![first.clone(), same_type]).unwrap_err(),
        SourceExactTypeRelationError::DuplicateType { first: 0, index: 1 }
    );

    let same_identity = SourceExactTypeIdentity::checked(
        Type::Boolean,
        exact,
        SourceExactTypeOrigin::Nominal(ConeIdentity::CORE),
    )
    .unwrap();
    assert_eq!(
        SourceExactTypeIdentities::checked(vec![first, same_identity]).unwrap_err(),
        SourceExactTypeRelationError::DuplicateIdentity { first: 0, index: 1 }
    );
}
