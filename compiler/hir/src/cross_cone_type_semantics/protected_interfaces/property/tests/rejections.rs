use super::*;
use crate::cross_cone_type_semantics::protected_interfaces::tests::support::nominal;

#[test]
fn property_source_rejects_wrong_logical_type_accessor_role_and_source_shape() {
    let (fixture, _, property, _, _) = setup(Some(DeclaredVisibilityV1::Private));
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        graph_source.records.values(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    let mut bad = property.clone();
    bad.payload.source.value_type = SignatureTypeKey::Nominal(nominal(fixture.unit));
    assert!(matches!(
        bad.validate_source(&graph, &mut fixture.clone(), &mut meter()),
        Err(ProtectedPropertySemanticError::ValueType)
    ));
    let mut bad = property.clone();
    let ProtectedPropertyMutabilityV1::ReadWrite { setter, .. } = bad.payload.source.mutability
    else {
        unreachable!()
    };
    bad.payload.source.getter = setter;
    let mut changed = fixture.clone();
    changed
        .property_shapes
        .get_mut(&bad.declaration())
        .unwrap()
        .getter = setter;
    assert!(matches!(
        bad.validate_source(&graph, &mut changed, &mut meter()),
        Err(ProtectedPropertySemanticError::Accessor)
    ));
    let mut changed = fixture.clone();
    changed
        .property_shapes
        .get_mut(&property.declaration())
        .unwrap()
        .setter = None;
    assert!(matches!(
        property.validate_source(&graph, &mut changed, &mut meter()),
        Err(ProtectedPropertySemanticError::SourceShape)
    ));
    let mut bad = property;
    bad.payload.source.value_type = SignatureTypeKey::Binder { depth: 0, index: 0 };
    assert!(matches!(
        bad.validate_source(&graph, &mut fixture.clone(), &mut meter()),
        Err(ProtectedPropertySemanticError::Signature(_))
    ));
}

#[test]
fn property_construction_rejects_public_setter_wrong_owner_and_unbacked_abstract_slot() {
    let (fixture, owner, property, _, _) = setup(Some(DeclaredVisibilityV1::Private));
    let mut payload = property.payload().clone();
    let ProtectedPropertyMutabilityV1::ReadWrite { setter, .. } = payload.source.mutability else {
        unreachable!()
    };
    payload.source.mutability = ProtectedPropertyMutabilityV1::ReadWrite {
        setter,
        setter_access: fixture.access(owner, DeclaredVisibilityV1::Public),
    };
    assert!(matches!(
        ProtectedPropertyPayloadV1::try_new(
            payload.source.owner,
            payload.source.value_type.clone(),
            payload.source.getter,
            payload.source.mutability,
            payload.source.representation,
            payload.source.slot_relations.clone()
        ),
        Err(ProtectedPropertyBuildError::SetterAccess)
    ));
    let mut payload = property.payload().clone();
    payload.source.owner = fixture.unit.source;
    assert!(matches!(
        ProtectedPropertyInterfaceV1::try_new(
            property.declaration(),
            property.declaration_access().clone(),
            payload
        ),
        Err(ProtectedPropertyBuildError::Owner)
    ));
    let payload = property.payload().clone();
    assert!(matches!(
        ProtectedPropertyPayloadV1::try_new(
            payload.source.owner,
            payload.source.value_type,
            payload.source.getter,
            payload.source.mutability,
            PropertyRepresentationV1::AbstractSlot,
            payload.source.slot_relations
        ),
        Err(ProtectedPropertyBuildError::MissingSlot)
    ));
}
