use la_arena::Arena;
use scoop_ast::Span;
use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, PackagePath,
    PersistentObjectValueId, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};

use super::*;
use crate::{
    ClassApplicationId, ClassDecl, ClassId, EnumDecl, HirNominalIdentity, InitializationUnitId,
    InterfaceDecl, NominalAccess, NominalLinkStem, ObjectKind, ObjectTypeId,
    SingletonPublishedRootId, StructDecl, TypeId,
};

fn object_key() -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("Registry").unwrap(),
        SourceNominalKind::Object,
        0,
    )
}

fn object_declaration() -> ObjectDecl {
    ObjectDecl {
        link_stem: NominalLinkStem::from_session_local_encoding("Registry".to_string()),
        name: "Registry".to_string(),
        owner: None,
        access: NominalAccess::public(),
        object_type: ObjectTypeId::from_raw(0_u32.into()),
        singleton_value: SingletonValueId::from_raw(0_u32.into()),
        kind: ObjectKind::Standalone,
        backing_class: ClassId::from_raw(0_u32.into()),
        span: Span::new(0, 0),
    }
}

fn relation_inputs() -> (Arena<ObjectDecl>, Arena<ObjectType>, Arena<SingletonValue>) {
    let mut objects = Arena::new();
    let declaration = objects.alloc(object_declaration());
    let mut object_types = Arena::new();
    let object_type = object_types.alloc(ObjectType {
        declaration,
        representation: ClassApplicationId::from_raw(0_u32.into()),
        canonical_type: TypeId::from_raw(0_u32.into()),
    });
    let mut values = Arena::new();
    values.alloc(SingletonValue {
        declaration,
        object_type,
        published_root: SingletonPublishedRootId::from_raw(0_u32.into()),
        initialization: InitializationUnitId::from_raw(0_u32.into()),
    });
    (objects, object_types, values)
}

fn nominal_identities(objects: &Arena<ObjectDecl>) -> HirNominalIdentities {
    let structs = Arena::<StructDecl>::new();
    let enums = Arena::<EnumDecl>::new();
    let classes = Arena::<ClassDecl>::new();
    let interfaces = Arena::<InterfaceDecl>::new();
    HirNominalIdentities::checked(
        &structs,
        Vec::new(),
        &enums,
        Vec::new(),
        &classes,
        Vec::new(),
        &interfaces,
        Vec::new(),
        objects,
        vec![HirNominalIdentity::from_source_declaration(object_key()).unwrap()],
    )
    .unwrap()
}

#[test]
fn relation_is_total_and_indexed_by_singleton_value() {
    let (objects, object_types, values) = relation_inputs();
    let nominals = nominal_identities(&objects);
    let identities =
        HirObjectValueIdentities::from_declarations(&objects, &object_types, &values, &nominals)
            .unwrap();
    let value = SingletonValueId::from_raw(0_u32.into());

    assert_eq!(identities[value].declaration(), values[value].declaration);
    assert_eq!(
        identities[value].id(),
        PersistentObjectValueId::from_source_object(&object_key()).unwrap()
    );
}

#[test]
fn relation_rejects_a_value_with_the_wrong_object_type() {
    let (objects, object_types, mut values) = relation_inputs();
    let value = SingletonValueId::from_raw(0_u32.into());
    values[value].object_type = ObjectTypeId::from_raw(1_u32.into());
    let nominals = nominal_identities(&objects);

    assert!(matches!(
        HirObjectValueIdentities::from_declarations(&objects, &object_types, &values, &nominals),
        Err(HirObjectValueIdentityError::ValueObjectType {
            object: 0,
            value: 0,
            expected: 0,
            actual: 1,
        })
    ));
}

#[test]
fn relation_rejects_a_missing_singleton_value() {
    let (objects, object_types, _) = relation_inputs();
    let values = Arena::new();
    let nominals = nominal_identities(&objects);

    assert!(matches!(
        HirObjectValueIdentities::from_declarations(&objects, &object_types, &values, &nominals),
        Err(HirObjectValueIdentityError::Count {
            table: ObjectRelationTable::SingletonValue,
            expected: 1,
            actual: 0,
        })
    ));
}
