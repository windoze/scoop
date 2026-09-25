use super::*;
use scoop_identity::{DispatchSlotKey, PersistentDispatchSlotId};

#[test]
fn abstract_property_joins_accessor_modality_and_preserves_slot_relations() {
    let (mut fixture, owner, mut property, getter, _) = setup(None);
    let slot = PersistentDispatchSlotId::from_key(&DispatchSlotKey::property_getter(
        property.payload().getter(),
    ))
    .unwrap();
    let mut abstract_payload = getter.payload().clone();
    abstract_payload.source_signature.modality = CallableModalityV1::Abstract;
    abstract_payload.source_signature.slot_relations =
        CanonicalProtectedSlotRefsV1::try_new(vec![slot]).unwrap();
    let abstract_getter = fixture.record(owner, getter.declaration(), abstract_payload);
    property.payload.source.representation = PropertyRepresentationV1::AbstractSlot;
    property.payload.source.slot_relations =
        CanonicalProtectedSlotRefsV1::try_new(vec![slot]).unwrap();
    fixture
        .property_shapes
        .get_mut(&property.declaration())
        .unwrap()
        .representation = PropertyRepresentationV1::AbstractSlot;
    let graph_source = fixture.graph.clone();
    let graph =
        CheckedNominalInheritanceGraphV1::validate(graph_source.records.values(), &graph_source)
            .unwrap();
    let mut getter_authority = fixture.clone();
    let mut concrete_authority = fixture.clone();
    let checked_abstract = abstract_getter
        .validate_source(&graph, &mut getter_authority)
        .unwrap();
    let checked_concrete = getter
        .validate_source(&graph, &mut concrete_authority)
        .unwrap();
    let checked_property = property.validate_source(&graph, &mut fixture).unwrap();
    checked_property
        .validate_accessor_contracts(checked_abstract, None)
        .unwrap();
    assert!(matches!(
        checked_property.validate_accessor_contracts(checked_concrete, None),
        Err(ProtectedPropertyAccessorClosureError::Representation)
    ));
}

#[test]
fn runtime_property_cannot_omit_its_open_getter_slot() {
    let (mut fixture, owner, property, getter, _) = setup(None);
    let slot = PersistentDispatchSlotId::from_key(&DispatchSlotKey::property_getter(
        property.payload().getter(),
    ))
    .unwrap();
    let mut payload = getter.payload().clone();
    payload.source_signature.modality = CallableModalityV1::Open;
    payload.source_signature.slot_relations =
        CanonicalProtectedSlotRefsV1::try_new(vec![slot]).unwrap();
    let getter = fixture.record(owner, getter.declaration(), payload);
    let graph_source = fixture.graph.clone();
    let graph =
        CheckedNominalInheritanceGraphV1::validate(graph_source.records.values(), &graph_source)
            .unwrap();
    let mut getter_authority = fixture.clone();
    let checked_property = property.validate_source(&graph, &mut fixture).unwrap();
    let checked_getter = getter
        .validate_source(&graph, &mut getter_authority)
        .unwrap();
    assert!(matches!(
        checked_property.validate_accessor_contracts(checked_getter, None),
        Err(ProtectedPropertyAccessorClosureError::Slot)
    ));
}
