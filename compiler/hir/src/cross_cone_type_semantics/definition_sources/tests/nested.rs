use super::*;

mod properties;

pub(super) fn fixture() -> Fixture {
    let mut fixture = Fixture::default();
    let cone = ConeIdentity::CORE;
    let outer = SourceNominalId::from_source_declaration(&nominal_key(
        cone,
        &[],
        "Outer",
        SourceNominalKind::Class,
        0,
    ))
    .unwrap();
    let root_key = nominal_key(cone, &[outer], "Nested", SourceNominalKind::Class, 1);
    let root = SourceNominalId::from_source_declaration(&root_key).unwrap();
    let root_origin = origin(cone, 20);
    let chain = vec![outer, root];
    let (property, mut support) = properties::runtime(&mut fixture, root, &chain);
    let child = child(&mut fixture, root, &chain);
    let child_id = child.declaration();
    let child_support = NestedSourceSupportV1::NestedNominal(Box::new(child));
    fixture.expect(
        UseKey::Nested(root, child_support.declaration()),
        child_support.declaration_access().definition_origin(),
    );
    support.push(child_support);
    let source = ProtectedNestedSourceInterfaceV1::try_new(
        PublicNominalKindV1::Class,
        NominalInheritanceModalityV1::Open,
        CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
            CanonicalIdentifier::new("T").unwrap(),
            TypeParameterBoundsV1::Unconstrained,
        )])
        .unwrap(),
        CanonicalSignatureTypesV1::try_new(vec![]).unwrap(),
        CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
        CanonicalNestedMemberRefsV1::try_new(vec![NestedSourceMemberRefV1::Property(
            property.declaration(),
        )])
        .unwrap(),
        CanonicalNestedNominalRefsV1::try_new(vec![child_id]).unwrap(),
        NominalSourceShapeV1::Class,
        CanonicalNestedSourceSupportV1::try_new(support).unwrap(),
    )
    .unwrap();
    let record = ProtectedDeclarationInterfaceV1::NestedNominal(Box::new(
        ProtectedNestedNominalInterfaceV1::try_new(
            root,
            access(vec![outer], DeclaredVisibilityV1::Protected, &root_origin),
            ProtectedNestedNominalPayloadV1::try_new(
                root,
                source,
                NestedNominalSupportV1::GenericTemplate,
            )
            .unwrap(),
        )
        .unwrap(),
    ));
    fixture.expect(UseKey::Protected(record.reference()), &root_origin);
    let property = ProtectedDeclarationInterfaceV1::Property(Box::new(property));
    fixture.expect(
        UseKey::Protected(property.reference()),
        property.declaration_access().definition_origin(),
    );
    fixture.protected =
        CanonicalProtectedDeclarationInterfacesV1::try_new(vec![record, property]).unwrap();
    fixture
}
fn child(
    fixture: &mut Fixture,
    parent: SourceNominalId,
    parents: &[SourceNominalId],
) -> NominalSupportNestedInterfaceV1 {
    let key = nominal_key(
        ConeIdentity::CORE,
        parents,
        "Constants",
        SourceNominalKind::Object,
        0,
    );
    let id = PersistentTypeId::from_source_declaration(&key).unwrap();
    let owner = SourceNominalId::Concrete(id);
    let child_origin = origin(ConeIdentity::CORE, 24);
    let mut chain = parents.to_vec();
    chain.push(owner);
    let property_key = SourceDeclarationKey::property(
        site(ConeIdentity::CORE, &chain),
        CanonicalIdentifier::new("enabled").unwrap(),
    );
    let property = PersistentPropertyId::from_source_declaration(&property_key).unwrap();
    let const_origin = origin(ConeIdentity::CORE, 25);
    let boolean = PersistentTypeId::from_source_declaration(&nominal_key(
        ConeIdentity::CORE,
        &[],
        "Boolean",
        SourceNominalKind::Struct,
        0,
    ))
    .unwrap();
    let record = NominalSupportPropertyInterfaceV1::try_new(
        property,
        access(chain, DeclaredVisibilityV1::Public, &const_origin),
        NominalSupportPropertyPayloadV1::Const {
            value: ExportConstValueV1::new(
                property,
                SignatureTypeKey::Nominal(boolean),
                CanonicalConstValueV1::Boolean(CanonicalBooleanV1::True),
                const_origin.clone(),
            ),
        },
    )
    .unwrap();
    let record = NestedSourceSupportV1::Property(Box::new(record));
    fixture.expect(UseKey::Nested(owner, record.declaration()), &const_origin);
    fixture.expect(UseKey::Const(property), &const_origin);
    let source = ProtectedNestedSourceInterfaceV1::try_new(
        PublicNominalKindV1::Object,
        NominalInheritanceModalityV1::Final,
        CanonicalBinderListV1::try_new(vec![]).unwrap(),
        CanonicalSignatureTypesV1::try_new(vec![]).unwrap(),
        CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
        CanonicalNestedMemberRefsV1::try_new(vec![NestedSourceMemberRefV1::Property(property)])
            .unwrap(),
        CanonicalNestedNominalRefsV1::try_new(vec![]).unwrap(),
        NominalSourceShapeV1::Object(ObjectSourceShapeV1::new(
            PersistentObjectValueId::from_source_object(&key).unwrap(),
        )),
        CanonicalNestedSourceSupportV1::try_new(vec![record]).unwrap(),
    )
    .unwrap();
    assert_eq!(parents.last(), Some(&parent));
    NominalSupportNestedInterfaceV1::try_new(
        owner,
        access(
            parents.to_vec(),
            DeclaredVisibilityV1::Private,
            &child_origin,
        ),
        ProtectedNestedNominalPayloadV1::try_new(
            owner,
            source,
            NestedNominalSupportV1::ParamFree {
                inheritance_exact: PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(id))
                    .unwrap(),
                representation_owner: id,
            },
        )
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn recursive_nested_runtime_setter_and_const_each_replay_all_typed_uses() {
    let fixture = fixture();
    assert_eq!(fixture.expected.len(), 10);
    let declared = fixture.declared();
    assert!(declared.sources().len() < fixture.expected.len());
    assert_eq!(
        fixture
            .validate(&declared, DecodeLimits::default())
            .unwrap()
            .0,
        10
    );
}
