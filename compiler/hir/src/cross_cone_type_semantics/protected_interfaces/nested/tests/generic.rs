use super::*;
use scoop_identity::{CanonicalIdentifier, SourceDeclarationKey};

#[test]
fn generic_source_root_preserves_binders_without_an_exact_inheritance_node() {
    let mut fixture = Fixture::default();
    let outer = fixture.class("Outer");
    let key = SourceDeclarationKey::nominal(
        site(&[outer.source]),
        CanonicalIdentifier::new("NestedGeneric").unwrap(),
        SourceNominalKind::Class,
        1,
    );
    let owner = SourceNominalId::from_source_declaration(&key).unwrap();
    let access = DeclarationAccessSourceV1::try_new(
        DeclaredVisibilityV1::Protected,
        vec![outer.source],
        fixture.graph.origins[&outer.source].clone(),
    )
    .unwrap();
    fixture.graph.keys.insert(owner, key);
    fixture.graph.access.insert(owner, access.clone());
    fixture
        .graph
        .origins
        .insert(owner, access.definition_origin().clone());
    let mut interface = source(
        NominalInheritanceModalityV1::Abstract,
        vec![],
        vec![],
        vec![],
        vec![],
    );
    interface.type_parameters = CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
        CanonicalIdentifier::new("T").unwrap(),
        TypeParameterBoundsV1::Unconstrained,
    )])
    .unwrap();
    fixture.nominal_sources.insert(owner, interface.clone());
    assert!(matches!(
        ProtectedNestedNominalPayloadV1::try_new(
            owner,
            interface.clone(),
            NestedNominalSupportV1::ParamFree {
                inheritance_exact: outer.exact,
                representation_owner: nominal(outer)
            }
        ),
        Err(NestedSourceBuildError::SupportKind)
    ));
    let payload = ProtectedNestedNominalPayloadV1::try_new(
        owner,
        interface,
        NestedNominalSupportV1::GenericTemplate,
    )
    .unwrap();
    let record = ProtectedNestedNominalInterfaceV1::try_new(owner, access, payload).unwrap();
    let bytes = encode(&record).unwrap();
    let decoded: DecodedProtectedNestedNominalInterfaceV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(decoded.resolve(&mut fixture).unwrap(), record);
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate_with_source_roots(
        graph_source.records.values(),
        graph_source.keys.keys().copied(),
        &graph_source,
    )
    .unwrap();
    assert!(graph.source_exact(owner).is_err());
    record
        .validate_source(
            &graph,
            &CanonicalNominalRepresentationSupportV1::default(),
            &mut fixture,
        )
        .unwrap();
    fixture.nominal_sources.get_mut(&owner).unwrap().modality = NominalInheritanceModalityV1::Open;
    assert!(matches!(
        record.validate_source(
            &graph,
            &CanonicalNominalRepresentationSupportV1::default(),
            &mut fixture
        ),
        Err(NestedSourceSemanticError::Modality)
    ));
}
