use super::*;
use crate::{
    CanonicalPersistentIdsV1, CanonicalPublicMemberRefsV1, NominalSourceFieldV1,
    NominalSourceFieldsV1, PublicNominalShapeV1,
};
use scoop_identity::{
    CanonicalIdentifier, DeclarationScope, DefinitionOwnerAtom, DefinitionOwnerChain,
    FieldIdentityKey, NonEmptyVec, PersistentFieldId, SourceDeclarationSite, SourceNominalKind,
};

fn inner_record(
    authority: &mut TestAuthority,
    outer: SourceNominalId,
    ty: SignatureTypeKey,
) -> NominalInterfaceRecordV1 {
    let outer_key = &authority.nominals[&outer];
    let atom = match outer {
        SourceNominalId::Concrete(id) => DefinitionOwnerAtom::Type(id),
        SourceNominalId::GenericTemplate(id) => DefinitionOwnerAtom::GenericType(id),
    };
    let key = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            outer_key.origin(),
            outer_key.package().clone(),
            DefinitionOwnerChain::from_outer_to_inner(vec![atom]),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("Inner").unwrap(),
        SourceNominalKind::Class,
        1,
    );
    let owner = SourceNominalId::from_source_declaration(&key).unwrap();
    let field_key = FieldIdentityKey::source_enclosing_instance(&key).unwrap();
    let field = PersistentFieldId::from_key(&field_key).unwrap();
    authority.fields.insert(field, field_key);
    authority.nominals.insert(owner, key);
    crate::nominal_interface_fixture::public_record(
        owner,
        PublicNominalKindV1::Class,
        binders(),
        empty_supertypes(),
        CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
        CanonicalPublicMemberRefsV1::try_new(vec![]).unwrap(),
        CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
        NominalSourceShapeV1::Class(
            NominalSourceFieldsV1::try_new(vec![NominalSourceFieldV1::new(field, ty)]).unwrap(),
        ),
    )
    .unwrap()
}

#[test]
fn enclosing_field_requires_lexical_class_and_exact_binder_prefix() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();
    authority.shapes.insert(
        fixture.owner,
        PublicNominalShapeV1::new(PublicNominalKindV1::Class, 1),
    );
    let SourceNominalId::GenericTemplate(outer) = fixture.owner else {
        panic!("generic fixture")
    };
    let correct = SignatureTypeKey::NominalApplication {
        origin: outer,
        arguments: NonEmptyVec::from_first(binder(0), []),
    };
    let record = inner_record(&mut authority, fixture.owner, correct.clone());
    assert!(record.validate_semantics(&mut authority).is_ok());

    let different = inner_record(
        &mut authority,
        fixture.owner,
        SignatureTypeKey::Nominal(fixture.base_class),
    );
    assert!(matches!(
        different.validate_semantics(&mut authority),
        Err(NominalInterfaceSemanticValidationError::EnclosingInstance(
            _
        ))
    ));
    let concrete = SignatureTypeKey::NominalApplication {
        origin: outer,
        arguments: NonEmptyVec::from_first(SignatureTypeKey::Nominal(fixture.base_class), []),
    };
    let record = inner_record(&mut authority, fixture.owner, concrete);
    assert!(matches!(
        record.validate_semantics(&mut authority),
        Err(NominalInterfaceSemanticValidationError::EnclosingInstance(
            _
        ))
    ));
    authority.shapes.insert(
        fixture.owner,
        PublicNominalShapeV1::new(PublicNominalKindV1::Interface, 1),
    );
    let record = inner_record(&mut authority, fixture.owner, correct);
    assert!(matches!(
        record.validate_semantics(&mut authority),
        Err(NominalInterfaceSemanticValidationError::EnclosingInstance(
            _
        ))
    ));
}
