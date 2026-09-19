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
    abstract_payload.modality = CallableModalityV1::Abstract;
    abstract_payload.slot_relations = CanonicalProtectedSlotRefsV1::try_new(vec![slot]).unwrap();
    let abstract_getter = fixture.record(owner, getter.declaration(), abstract_payload);
    property.payload.representation = PropertyRepresentationV1::AbstractSlot;
    property.payload.slot_relations = CanonicalProtectedSlotRefsV1::try_new(vec![slot]).unwrap();
    fixture
        .property_shapes
        .get_mut(&property.declaration())
        .unwrap()
        .representation = PropertyRepresentationV1::AbstractSlot;
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        graph_source.records.values(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    let mut getter_authority = fixture.clone();
    let mut concrete_authority = fixture.clone();
    let checked_abstract = abstract_getter
        .validate_source(&graph, &mut getter_authority, &mut meter())
        .unwrap();
    let checked_concrete = getter
        .validate_source(&graph, &mut concrete_authority, &mut meter())
        .unwrap();
    let checked_property = property
        .validate_source(&graph, &mut fixture, &mut meter())
        .unwrap();
    checked_property
        .validate_accessor_contracts(checked_abstract, None, &mut meter())
        .unwrap();
    assert!(matches!(
        checked_property.validate_accessor_contracts(checked_concrete, None, &mut meter()),
        Err(ProtectedPropertyAccessorClosureError::Representation)
    ));
    let mut budget = BudgetMeter::new(DecodeLimits {
        decoded_nodes: 0,
        ..DecodeLimits::default()
    });
    assert!(matches!(
        checked_property.validate_accessor_contracts(checked_abstract, None, &mut budget),
        Err(ProtectedPropertyAccessorClosureError::Resource(_))
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
    payload.modality = CallableModalityV1::Open;
    payload.slot_relations = CanonicalProtectedSlotRefsV1::try_new(vec![slot]).unwrap();
    let getter = fixture.record(owner, getter.declaration(), payload);
    let graph_source = fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        graph_source.records.values(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    let mut getter_authority = fixture.clone();
    let checked_property = property
        .validate_source(&graph, &mut fixture, &mut meter())
        .unwrap();
    let checked_getter = getter
        .validate_source(&graph, &mut getter_authority, &mut meter())
        .unwrap();
    assert!(matches!(
        checked_property.validate_accessor_contracts(checked_getter, None, &mut meter()),
        Err(ProtectedPropertyAccessorClosureError::Slot)
    ));
}
