use scoop_identity::{AccessorRole, DispatchSlotKey, PersistentDispatchSlotId, SourceNominalKind};
use scoop_wire::{BudgetMeter, DecodeLimits, Encoder, WireEncode, decode_canonical, encode};

use super::*;
use crate::CheckedNominalInheritanceGraphV1;
use crate::cross_cone_type_semantics::inheritance::tests::support::Node;

mod support;
use support::Fixture;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
fn vtable(slots: &[PersistentDispatchSlotId]) -> InheritanceSlotSchemaV1 {
    InheritanceSlotSchemaV1::try_new(InheritanceSlotSchemaRoleV1::ClassVtable, slots.to_vec())
        .unwrap()
}
fn interface(owner: Node, slots: &[PersistentDispatchSlotId]) -> InheritanceSlotSchemaV1 {
    InheritanceSlotSchemaV1::try_new(
        InheritanceSlotSchemaRoleV1::Interface {
            interface_exact: owner.exact,
        },
        slots.to_vec(),
    )
    .unwrap()
}

#[test]
fn schema_wire_keeps_semantic_slot_order_and_rejects_duplicate_or_unknown_forms() {
    assert_eq!(
        encode(&InheritanceSlotSchemaRoleV1::ClassVtable).unwrap(),
        [0xa1, 0, 1]
    );
    assert!(
        decode_canonical::<DecodedInheritanceSlotSchemaRoleV1>(
            &[0xa1, 0, 3],
            DecodeLimits::default()
        )
        .is_err()
    );
    let mut fixture = Fixture::default();
    let class = fixture.add("Owner", SourceNominalKind::Class);
    let a = fixture.function(class, "a");
    let b = fixture.function(class, "b");
    let mut slots = vec![a, b];
    slots.sort_unstable_by(|left, right| right.cmp(left));
    let schema = vtable(&slots);
    let bytes = encode(&schema).unwrap();
    let decoded: DecodedInheritanceSlotSchemaV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert!(matches!(
        decoded.clone().resolve(
            &mut fixture,
            &mut BudgetMeter::new(DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            })
        ),
        Err(InheritanceSlotSchemaResolutionError::Resource(_))
    ));
    assert_eq!(decoded.resolve(&mut fixture, &mut meter()).unwrap(), schema);
    assert_eq!(schema.slots(), slots);
    assert!(
        InheritanceSlotSchemaV1::try_new(InheritanceSlotSchemaRoleV1::ClassVtable, vec![a, a])
            .is_err()
    );
    let bad = RawSchema(vec![a, a]);
    let decoded: DecodedInheritanceSlotSchemaV1 =
        decode_canonical(&encode(&bad).unwrap(), DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture, &mut meter()),
        Err(InheritanceSlotSchemaResolutionError::Schema(
            InheritanceSlotSchemaBuildError::DuplicateSlot { .. }
        ))
    ));
}

#[test]
fn schema_tables_sort_roles_only_and_reader_rejects_reversed_roles() {
    let mut fixture = Fixture::default();
    let owner = fixture.add("Contract", SourceNominalKind::Interface);
    let table =
        CanonicalInheritanceSlotSchemasV1::try_new(vec![interface(owner, &[]), vtable(&[])])
            .unwrap();
    assert_eq!(
        table.records()[0].role(),
        InheritanceSlotSchemaRoleV1::ClassVtable
    );
    let decoded: DecodedCanonicalInheritanceSlotSchemasV1 =
        decode_canonical(&encode(&table).unwrap(), DecodeLimits::default()).unwrap();
    assert_eq!(decoded.resolve(&mut fixture, &mut meter()).unwrap(), table);
    let reversed = RawTable(vec![interface(owner, &[]), vtable(&[])]);
    let decoded: DecodedCanonicalInheritanceSlotSchemasV1 =
        decode_canonical(&encode(&reversed).unwrap(), DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture, &mut meter()),
        Err(InheritanceSlotSchemaResolutionError::Schema(
            InheritanceSlotSchemaBuildError::RoleOrder { .. }
        ))
    ));
}

#[test]
fn class_schema_preserves_base_prefix_and_joins_interface_provider_order() {
    let mut fixture = Fixture::default();
    let base = fixture.add("Base", SourceNominalKind::Class);
    let derived = fixture.add("Derived", SourceNominalKind::Class);
    let contract = fixture.add("Contract", SourceNominalKind::Interface);
    fixture.inheritance.edges(derived, Some(base), &[contract]);
    let inherited = fixture.function(base, "base");
    let added = fixture.function(derived, "added");
    let getter = fixture.accessor(contract, "value", AccessorRole::Getter);
    let setter = fixture.accessor(contract, "value", AccessorRole::Setter);
    fixture.set(base, vec![vtable(&[inherited])]);
    fixture.set(contract, vec![interface(contract, &[getter, setter])]);
    fixture.set(
        derived,
        vec![
            vtable(&[inherited, added]),
            interface(contract, &[getter, setter]),
        ],
    );
    let graph = CheckedNominalInheritanceGraphV1::validate(
        fixture.inheritance.records.values(),
        &fixture.inheritance,
        &mut meter(),
    )
    .unwrap();
    assert!(
        graph
            .validate_slot_schemas(derived.exact, &fixture, &mut meter())
            .is_ok()
    );
    fixture.schemas.insert(
        derived.exact,
        CanonicalInheritanceSlotSchemasV1::try_new(vec![
            vtable(&[added, inherited]),
            interface(contract, &[getter, setter]),
        ])
        .unwrap(),
    );
    assert!(matches!(
        graph.validate_slot_schemas(derived.exact, &fixture, &mut meter()),
        Err(InheritanceSlotSchemaSemanticError::BasePrefix(_))
    ));
    fixture.schemas.insert(
        derived.exact,
        CanonicalInheritanceSlotSchemasV1::try_new(vec![
            vtable(&[inherited, added]),
            interface(contract, &[setter, getter]),
        ])
        .unwrap(),
    );
    assert!(matches!(
        graph.validate_slot_schemas(derived.exact, &fixture, &mut meter()),
        Err(InheritanceSlotSchemaSemanticError::InterfaceOrder { .. })
    ));
    fixture.schemas.insert(
        derived.exact,
        CanonicalInheritanceSlotSchemasV1::try_new(vec![vtable(&[inherited, added])]).unwrap(),
    );
    assert!(matches!(
        graph.validate_slot_schemas(derived.exact, &fixture, &mut meter()),
        Err(InheritanceSlotSchemaSemanticError::RoleCoverage(_))
    ));
}

#[test]
fn diamond_interface_schema_deduplicates_inherited_roots_before_new_slots() {
    let mut fixture = Fixture::default();
    let root = fixture.add("Root", SourceNominalKind::Interface);
    let left = fixture.add("Left", SourceNominalKind::Interface);
    let right = fixture.add("Right", SourceNominalKind::Interface);
    let joined = fixture.add("Joined", SourceNominalKind::Interface);
    fixture.inheritance.edges(left, None, &[root]);
    fixture.inheritance.edges(right, None, &[root]);
    fixture.inheritance.edges(joined, None, &[left, right]);
    let root_slot = fixture.function(root, "root");
    let left_slot = fixture.function(left, "left");
    let right_slot = fixture.function(right, "right");
    let new_slot = fixture.function(joined, "new");
    fixture.set(root, vec![interface(root, &[root_slot])]);
    fixture.set(left, vec![interface(left, &[root_slot, left_slot])]);
    fixture.set(right, vec![interface(right, &[root_slot, right_slot])]);
    fixture.set(
        joined,
        vec![interface(
            joined,
            &[root_slot, right_slot, left_slot, new_slot],
        )],
    );
    let graph = CheckedNominalInheritanceGraphV1::validate(
        fixture.inheritance.records.values(),
        &fixture.inheritance,
        &mut meter(),
    )
    .unwrap();
    assert!(
        graph
            .validate_slot_schemas(joined.exact, &fixture, &mut meter())
            .is_ok()
    );
    fixture.schemas.insert(
        joined.exact,
        CanonicalInheritanceSlotSchemasV1::try_new(vec![interface(
            joined,
            &[root_slot, new_slot, right_slot, left_slot],
        )])
        .unwrap(),
    );
    assert!(matches!(
        graph.validate_slot_schemas(joined.exact, &fixture, &mut meter()),
        Err(InheritanceSlotSchemaSemanticError::InheritedSlots(_))
    ));
}

#[test]
fn slot_role_and_source_owner_are_replayed_from_foundation_keys() {
    let mut fixture = Fixture::default();
    let base = fixture.add("Base", SourceNominalKind::Class);
    let other = fixture.add("Other", SourceNominalKind::Class);
    let foreign = fixture.function(other, "foreign");
    fixture.set(base, vec![vtable(&[foreign])]);
    let graph = CheckedNominalInheritanceGraphV1::validate(
        fixture.inheritance.records.values(),
        &fixture.inheritance,
        &mut meter(),
    )
    .unwrap();
    assert!(matches!(
        graph.validate_slot_schemas(base.exact, &fixture, &mut meter()),
        Err(InheritanceSlotSchemaSemanticError::NewSlotOwner { .. })
    ));
    let function = fixture.functions.keys().next().copied().unwrap();
    let wrong = DispatchSlotKey::interface_method(function);
    fixture.slots.insert(foreign, wrong);
    assert!(matches!(
        graph.validate_slot_schemas(base.exact, &fixture, &mut meter()),
        Err(InheritanceSlotSchemaSemanticError::SlotIdentity(_))
    ));
    assert!(matches!(
        graph.validate_slot_schemas(
            base.exact,
            &fixture,
            &mut BudgetMeter::new(DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            })
        ),
        Err(InheritanceSlotSchemaSemanticError::Resource(_))
    ));
}

#[test]
fn canonical_getter_identity_cannot_be_registered_as_a_setter_slot() {
    let mut fixture = Fixture::default();
    let owner = fixture.add("Contract", SourceNominalKind::Interface);
    let getter = fixture.accessor(owner, "value", AccessorRole::Getter);
    let scoop_identity::DispatchDeclarationOwner::Accessor(accessor) =
        fixture.slots[&getter].owner()
    else {
        unreachable!()
    };
    let wrong = fixture.slot(DispatchSlotKey::property_setter(accessor));
    fixture.set(owner, vec![interface(owner, &[wrong])]);
    let graph = CheckedNominalInheritanceGraphV1::validate(
        fixture.inheritance.records.values(),
        &fixture.inheritance,
        &mut meter(),
    )
    .unwrap();
    assert!(matches!(
        graph.validate_slot_schemas(owner.exact, &fixture, &mut meter()),
        Err(InheritanceSlotSchemaSemanticError::SlotIdentity(_))
    ));
}

struct RawSchema(Vec<PersistentDispatchSlotId>);
impl WireEncode for RawSchema {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        InheritanceSlotSchemaRoleV1::ClassVtable.encode(encoder)?;
        encoder.field(2)?;
        wire::sequence(encoder, &self.0)
    }
}
struct RawTable(Vec<InheritanceSlotSchemaV1>);
impl WireEncode for RawTable {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.0)
    }
}
