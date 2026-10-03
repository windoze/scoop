use scoop_identity::{
    ConeIdentity, DeclarationScope, DefinitionOwnerAtom, DefinitionOwnerChain, NonEmptyVec,
    PackagePath, PersistentTypeAliasId, SignatureTypeKey, SourceDeclarationKey,
    SourceDeclarationKind, SourceDeclarationSite,
};

use super::*;
use crate::{
    PublicNominalKindV1, PublicNominalShapeV1, SignatureBinderScopeError,
    SignatureTypeSemanticError, TypeAliasTargetV1,
};

mod support;

use support::*;

#[test]
fn validates_current_top_level_alias_and_empty_signature_scope() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();

    assert!(fixture.record().validate_semantics(&mut authority).is_ok());
    assert_eq!(authority.source.declaration(), &fixture.alias_key);
    assert_eq!(authority.source.access(), PublicLookupAccessV1::DirectOnly);
    assert_eq!(
        authority.source.definition_origin(),
        fixture.origin.origin()
    );
}

#[test]
fn rejects_wrong_declaration_kind_and_same_kind_identity() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();
    authority.source.declaration =
        SourceDeclarationKey::property(top_level_site(ConeIdentity::CORE), identifier("Alias"));
    assert_eq!(
        fixture.record().validate_semantics(&mut authority),
        Err(TypeAliasInterfaceSemanticValidationError::DeclarationKind {
            actual: SourceDeclarationKind::Property,
        })
    );

    let other_key = alias_key("Other", top_level_site(ConeIdentity::CORE));
    let other = PersistentTypeAliasId::from_source_declaration(&other_key).unwrap();
    let mut authority = fixture.authority();
    authority.source.declaration = other_key;
    assert_eq!(
        fixture.record().validate_semantics(&mut authority),
        Err(
            TypeAliasInterfaceSemanticValidationError::DeclarationIdentityMismatch {
                expected: fixture.alias,
                actual: other,
            }
        )
    );
}

#[test]
fn rejects_foreign_nested_and_non_cone_wide_declarations() {
    let fixture = Fixture::new();
    let foreign = foreign_cone();
    let foreign_key = alias_key("Alias", top_level_site(foreign));
    let foreign_alias = PersistentTypeAliasId::from_source_declaration(&foreign_key).unwrap();
    let foreign_record = record_for(&fixture, foreign_alias);
    let mut authority = fixture.authority();
    authority.alias = foreign_alias;
    authority.source.declaration = foreign_key;
    assert_eq!(
        foreign_record.validate_semantics(&mut authority),
        Err(TypeAliasInterfaceSemanticValidationError::DeclarationCone {
            expected: ConeIdentity::CORE,
            actual: foreign,
        })
    );

    let nested_key = alias_key(
        "Alias",
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::from_outer_to_inner(vec![DefinitionOwnerAtom::Type(
                fixture.target,
            )]),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
    );
    let nested_alias = PersistentTypeAliasId::from_source_declaration(&nested_key).unwrap();
    let nested_record = record_for(&fixture, nested_alias);
    let mut authority = fixture.authority();
    authority.alias = nested_alias;
    authority.source.declaration = nested_key;
    assert_eq!(
        nested_record.validate_semantics(&mut authority),
        Err(TypeAliasInterfaceSemanticValidationError::NestedDeclaration { owner_depth: 1 })
    );

    let scoped_source = source(ConeIdentity::CORE, "src/scoped.scoop");
    let scoped_key = alias_key(
        "Alias",
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::SourceScoped(scoped_source.clone()),
        )
        .unwrap(),
    );
    let scoped_alias = PersistentTypeAliasId::from_source_declaration(&scoped_key).unwrap();
    let scoped_record = record_for(&fixture, scoped_alias);
    let mut authority = fixture.authority();
    authority.alias = scoped_alias;
    authority.source.declaration = scoped_key;
    assert_eq!(
        scoped_record.validate_semantics(&mut authority),
        Err(
            TypeAliasInterfaceSemanticValidationError::DeclarationScope {
                actual: DeclarationScope::SourceScoped(scoped_source),
            }
        )
    );
}

#[test]
fn rejects_source_access_and_definition_origin_mismatches() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();
    authority.source.access = PublicLookupAccessV1::PublicSlot;
    assert_eq!(
        fixture.record().validate_semantics(&mut authority),
        Err(TypeAliasInterfaceSemanticValidationError::Access {
            expected: PublicLookupAccessV1::PublicSlot,
            actual: PublicLookupAccessV1::DirectOnly,
        })
    );

    let mut authority = fixture.authority();
    authority.source.definition_origin =
        definition_origin(source(ConeIdentity::CORE, "src/Alias.scoop"), 1, 10)
            .origin()
            .clone();
    assert!(matches!(
        fixture.record().validate_semantics(&mut authority),
        Err(TypeAliasInterfaceSemanticValidationError::DefinitionOriginMismatch { .. })
    ));
}

#[test]
fn rejects_foreign_foundation_definition_origin() {
    let fixture = Fixture::new();
    let foreign = foreign_cone();
    let foreign_origin = definition_origin(source(foreign, "src/Alias.scoop"), 0, 10);
    let record = fixture.record_with(
        TypeAliasTargetV1::Signature(SignatureTypeKey::Nominal(fixture.target)),
        foreign_origin.clone(),
    );
    let mut authority = fixture.authority();
    authority.source.definition_origin = foreign_origin.origin().clone();

    assert_eq!(
        record.validate_semantics(&mut authority),
        Err(
            TypeAliasInterfaceSemanticValidationError::DefinitionOriginCone {
                expected: ConeIdentity::CORE,
                actual: foreign,
            }
        )
    );
}

#[test]
fn validates_signature_target_shape_with_no_binder_frames() {
    let fixture = Fixture::new();
    let binder = fixture.record_with(
        TypeAliasTargetV1::Signature(SignatureTypeKey::Binder { depth: 0, index: 0 }),
        fixture.origin.clone(),
    );
    let mut authority = fixture.authority();
    assert_eq!(
        binder.validate_semantics(&mut authority),
        Err(TypeAliasInterfaceSemanticValidationError::Target(
            SignatureTypeSemanticError::BinderScope(SignatureBinderScopeError::DepthOutOfRange {
                depth: 0,
                available_depths: 0,
            })
        ))
    );

    let generic = generic_nominal("Generic", 1);
    let wrong_arity = fixture.record_with(
        TypeAliasTargetV1::Signature(SignatureTypeKey::NominalApplication {
            origin: generic,
            arguments: NonEmptyVec::new(vec![SignatureTypeKey::Nominal(fixture.target)]).unwrap(),
        }),
        fixture.origin.clone(),
    );
    let mut authority = fixture.authority();
    authority.generic_nominals.insert(
        generic,
        PublicNominalShapeV1::new(PublicNominalKindV1::Class, 2),
    );
    assert!(matches!(
        wrong_arity.validate_semantics(&mut authority),
        Err(TypeAliasInterfaceSemanticValidationError::Target(
            SignatureTypeSemanticError::GenericNominalArity {
                declaration,
                expected: 2,
                actual: 1,
            }
        )) if declaration == generic
    ));
}

#[test]
fn alias_edge_defers_target_authorization_to_closure_validation() {
    let fixture = Fixture::new();
    let record = fixture.record_with(
        TypeAliasTargetV1::Alias(fixture.other_alias),
        fixture.origin.clone(),
    );
    let mut authority = fixture.authority();
    authority.concrete_nominals.clear();

    assert!(record.validate_semantics(&mut authority).is_ok());
}

#[test]
fn reports_missing_source_authority() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();
    authority.alias = fixture.other_alias;

    assert_eq!(
        fixture.record().validate_semantics(&mut authority),
        Err(TypeAliasInterfaceSemanticValidationError::Declaration(
            TestAuthorityError::Alias(fixture.alias)
        ))
    );
}

fn record_for(fixture: &Fixture, alias: PersistentTypeAliasId) -> TypeAliasInterfaceRecordV1 {
    TypeAliasInterfaceRecordV1::try_new(
        alias,
        TypeAliasTargetV1::Signature(SignatureTypeKey::Nominal(fixture.target)),
        PublicLookupAccessV1::DirectOnly,
        fixture.origin.clone(),
    )
    .unwrap()
}
