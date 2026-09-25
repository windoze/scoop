use super::*;
use crate::cross_cone_type_semantics::inheritance::interface_test_support::{Bundle, fixture};
use crate::*;
use scoop_wire::{WireDecode, decode_canonical, encode};

mod rejection;

fn decoded<T: WireDecode>(value: &impl WireEncode) -> T {
    decode_canonical(&encode(value).unwrap()).unwrap()
}

fn source_inventory(bundle: &Bundle) -> CanonicalSourceInheritanceInventoriesV1 {
    let source = &bundle.fixture;
    let records = source
        .required_inheritance_owners()
        .unwrap()
        .values()
        .iter()
        .map(|owner| {
            SourceInheritanceInventoryV1::try_new(
                *owner,
                source
                    .required_inheritance_constructors(*owner)
                    .unwrap()
                    .clone(),
                source
                    .required_inheritance_protected_members(*owner)
                    .unwrap()
                    .clone(),
                source.schemas(*owner).unwrap().clone(),
            )
            .unwrap()
        })
        .collect();
    CanonicalSourceInheritanceInventoriesV1::try_new(records).unwrap()
}

#[test]
fn source_inventory_transports_constructors_protected_members_and_slot_roles() {
    let mut bundle = fixture();
    let source = source_inventory(&bundle);
    let bytes = encode(&source).unwrap();
    let decoded: DecodedCanonicalSourceInheritanceInventoriesV1 = decoded(&source);
    assert_eq!(encode(&decoded).unwrap(), bytes);
    let restored = decoded.resolve(&mut bundle.fixture).unwrap();
    assert_eq!(restored, source);
    assert_eq!(restored.owners().values().len(), 2);
    let base = restored.get(bundle.base.exact).unwrap();
    assert_eq!(base.constructors().values().len(), 1);
    assert!(!base.protected_members().values().is_empty());
    assert_eq!(base.slot_schemas().records()[0].slots(), &[bundle.slot]);
    let expected = [
        vec![0xa4, 1],
        encode(&base.owner()).unwrap(),
        vec![2],
        encode(base.constructors()).unwrap(),
        vec![3],
        encode(base.protected_members()).unwrap(),
        vec![4],
        encode(base.slot_schemas()).unwrap(),
    ]
    .concat();
    assert_eq!(encode(base).unwrap(), expected);
    let empty = CanonicalSourceInheritanceInventoriesV1::try_new(vec![]).unwrap();
    assert_eq!(encode(&empty).unwrap(), [0x80]);
    assert!(empty.owners().is_empty());
    assert!(empty.get(bundle.base.exact).is_none());
}

#[test]
fn byte_restored_source_inventory_drives_existing_missing_constructor_validation() {
    let mut bundle = fixture();
    let source = source_inventory(&bundle);
    let decoded: DecodedCanonicalSourceInheritanceInventoriesV1 = decoded(&source);
    let restored = decoded.resolve(&mut bundle.fixture).unwrap();
    let input = &mut bundle.fixture.inheritance_interfaces;
    input.owners = restored.owners().clone();
    input.constructors.clear();
    input.members.clear();
    input.schemas.clear();
    for record in restored.records() {
        input
            .constructors
            .insert(record.owner(), record.constructors().clone());
        input
            .members
            .insert(record.owner(), record.protected_members().clone());
        input
            .schemas
            .insert(record.owner(), record.slot_schemas().clone());
    }
    bundle.validate().unwrap();
    bundle.change(bundle.base, |record| {
        *record = NominalInheritanceInterfaceV1::try_new(
            record.edges().clone(),
            record.domains().clone(),
            CanonicalInheritanceConstructorsV1::default(),
            record.slots().clone(),
            record.protected_members().clone(),
            record.slot_schemas().clone(),
        )
        .unwrap();
    });
    assert!(matches!(
        bundle.validate(),
        Err(InheritanceInterfaceSemanticError::Inventory)
    ));
}

#[test]
fn producer_orders_only_owners_and_keeps_slot_declaration_order() {
    let mut bundle = fixture();
    let source = source_inventory(&bundle);
    let original = source.get(bundle.base.exact).unwrap();
    let mut slots = [bundle.slot, {
        let InheritanceCallableDeclarationV1::Function(helper) = bundle.target else {
            panic!("fixture target must be a function");
        };
        let slot = scoop_identity::PersistentDispatchSlotId::from_key(
            &scoop_identity::DispatchSlotKey::virtual_method(helper),
        )
        .unwrap();
        bundle.fixture.slots.push(slot);
        slot
    }];
    slots.sort_unstable_by(|left, right| right.cmp(left));
    let schemas = CanonicalInheritanceSlotSchemasV1::try_new(vec![
        InheritanceSlotSchemaV1::try_new(InheritanceSlotSchemaRoleV1::ClassVtable, slots.to_vec())
            .unwrap(),
    ])
    .unwrap();
    let record = SourceInheritanceInventoryV1::try_new(
        original.owner(),
        original.constructors().clone(),
        original.protected_members().clone(),
        schemas,
    )
    .unwrap();
    let decoded: DecodedSourceInheritanceInventoryV1 = decoded(&record);
    assert_eq!(
        decoded
            .resolve(&mut bundle.fixture)
            .unwrap()
            .slot_schemas()
            .records()[0]
            .slots(),
        slots
    );
    let reversed = CanonicalSourceInheritanceInventoriesV1::try_new(
        source.records().iter().rev().cloned().collect(),
    )
    .unwrap();
    assert_eq!(encode(&reversed).unwrap(), encode(&source).unwrap());
}
