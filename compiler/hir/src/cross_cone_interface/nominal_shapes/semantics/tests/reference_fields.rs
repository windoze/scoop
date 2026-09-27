use super::*;
use crate::NominalSourceFieldsV1;
use scoop_identity::{GeneratedNominalKey, PersistentPropertyId};

#[test]
fn class_source_fields_validate_roles_owners_and_own_generic_binders() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();
    let class = nominal("GenericClass", SourceNominalKind::Class, 1);
    let owner = SourceNominalId::from_source_declaration(&class).unwrap();
    let property = property();
    let key = FieldIdentityKey::source_property_backing(&class, property).unwrap();
    let field = PersistentFieldId::from_key(&key).unwrap();
    authority.fields.insert(field, key);
    let shape = NominalSourceShapeV1::Class(
        NominalSourceFieldsV1::try_new(vec![NominalSourceFieldV1::new(field, binder(0))]).unwrap(),
    );
    shape
        .validate_semantics(
            owner,
            PublicNominalKindV1::Class,
            &binders(),
            &mut authority,
        )
        .unwrap();
    assert!(matches!(
        shape.validate_semantics(
            fixture.struct_owner,
            PublicNominalKindV1::Class,
            &binders(),
            &mut authority
        ),
        Err(NominalSourceShapeSemanticError::NominalField {
            error: NominalSourceFieldSemanticError::Owner { .. },
            ..
        })
    ));
    let bad = NominalSourceShapeV1::Class(
        NominalSourceFieldsV1::try_new(vec![NominalSourceFieldV1::new(
            fixture.struct_field,
            binder(0),
        )])
        .unwrap(),
    );
    assert!(matches!(
        bad.validate_semantics(
            owner,
            PublicNominalKindV1::Class,
            &binders(),
            &mut authority
        ),
        Err(NominalSourceShapeSemanticError::NominalField {
            error: NominalSourceFieldSemanticError::FieldRole,
            ..
        })
    ));
    let bad_binder = NominalSourceShapeV1::Class(
        NominalSourceFieldsV1::try_new(vec![NominalSourceFieldV1::new(field, binder(1))]).unwrap(),
    );
    assert!(matches!(
        bad_binder.validate_semantics(
            owner,
            PublicNominalKindV1::Class,
            &binders(),
            &mut authority
        ),
        Err(NominalSourceShapeSemanticError::NominalField {
            error: NominalSourceFieldSemanticError::ValueType(_),
            ..
        })
    ));
}

#[test]
fn object_source_fields_require_the_actual_backing_class_and_property_role() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();
    let SourceNominalId::Concrete(object) = fixture.object_owner else {
        panic!("source object is concrete");
    };
    let property = property();
    let key = FieldIdentityKey::object_backing_property(
        &GeneratedNominalKey::ObjectBackingClass { object },
        property,
    )
    .unwrap();
    let field = PersistentFieldId::from_key(&key).unwrap();
    authority.fields.insert(field, key);
    authority.concrete.insert(
        object,
        crate::PublicNominalShapeV1::new(PublicNominalKindV1::Object, 0),
    );
    let build = |value| {
        NominalSourceShapeV1::Object(ObjectSourceShapeV1::new(
            crate::ObjectSourceKindV1::Standalone,
            value,
            NominalSourceFieldsV1::try_new(vec![NominalSourceFieldV1::new(
                field,
                SignatureTypeKey::Nominal(object),
            )])
            .unwrap(),
        ))
    };
    build(fixture.object_value)
        .validate_semantics(
            fixture.object_owner,
            PublicNominalKindV1::Object,
            &CanonicalBinderListV1::try_new(vec![]).unwrap(),
            &mut authority,
        )
        .unwrap();
    assert!(matches!(
        build(fixture.foreign_object_value).validate_semantics(
            fixture.foreign_object_owner,
            PublicNominalKindV1::Object,
            &CanonicalBinderListV1::try_new(vec![]).unwrap(),
            &mut authority
        ),
        Err(NominalSourceShapeSemanticError::NominalField {
            error: NominalSourceFieldSemanticError::FieldRole,
            ..
        })
    ));
}

fn property() -> PersistentPropertyId {
    use scoop_identity::{
        CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, PackagePath,
        SourceDeclarationSite,
    };
    PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("stored").unwrap(),
    ))
    .unwrap()
}
