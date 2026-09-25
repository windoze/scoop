use super::*;

fn table(owner: PersistentTypeId, shape: NominalSourceShapeV1) -> CanonicalNominalInterfacesV1 {
    CanonicalNominalInterfacesV1::try_new(vec![
        crate::nominal_interface_fixture::public_record(
            SourceNominalId::Concrete(owner),
            shape.kind(),
            CanonicalBinderListV1::try_new(vec![]).unwrap(),
            CanonicalSignatureTypesV1::try_new(vec![]).unwrap(),
            CanonicalPersistentIdsV1::empty(),
            CanonicalPublicMemberRefsV1::try_new(vec![]).unwrap(),
            CanonicalPersistentIdsV1::empty(),
            shape,
        )
        .unwrap(),
    ])
    .unwrap()
}

#[test]
fn public_join_requires_source_root_and_equal_value_shape() {
    let root = Artifact::new("value", SourceNominalKind::Struct, None).load(None);
    let bound = root.bind();
    let shape = NominalSourceShapeV1::Struct(
        StructSourceShapeV1::try_new(vec![], crate::NominalCLayoutPolicyV1::Ordinary, false)
            .unwrap(),
    );
    let public = table(root.owner, shape.clone());
    super::super::provider::validate_public(&bound, &public).unwrap();
    let foreign = Artifact::new("foreign", SourceNominalKind::Struct, None).load(None);
    assert!(
        matches!(super::super::provider::validate_public(&bound, &table(foreign.owner, shape)),
        Err(TypeFoundationReplayError::PublicNominal(owner)) if owner == SourceNominalId::Concrete(foreign.owner))
    );
    let key = FieldIdentityKey::source_declared(
        bound
            .nominal_key(SourceNominalId::Concrete(root.owner))
            .unwrap(),
        CanonicalIdentifier::new("extra").unwrap(),
    )
    .unwrap();
    let field = PersistentFieldId::from_key(&key).unwrap();
    let changed = NominalSourceShapeV1::Struct(
        StructSourceShapeV1::try_new(
            vec![NominalSourceFieldV1::new(
                field,
                SignatureTypeKey::Nominal(root.owner),
            )],
            crate::NominalCLayoutPolicyV1::Ordinary,
            false,
        )
        .unwrap(),
    );
    assert!(
        matches!(super::super::provider::validate_public(&bound, &table(root.owner, changed)),
        Err(TypeFoundationReplayError::PublicSourceShape(owner)) if owner == root.owner)
    );
}
