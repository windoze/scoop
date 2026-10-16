use scoop_identity::{PersistentDispatchSlotId, SourceNominalKind};
use scoop_wire::{Encoder, WireEncode, decode_canonical, encode};

use super::*;
use crate::cross_cone_type_semantics::inheritance::tests::support::Node;

pub(in crate::cross_cone_type_semantics) mod support;
use support::Fixture;

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
    assert!(decode_canonical::<DecodedInheritanceSlotSchemaRoleV1>(&[0xa1, 0, 3]).is_err());
    let mut fixture = Fixture::default();
    let class = fixture.add("Owner", SourceNominalKind::Class);
    let a = fixture.function(class, "a");
    let b = fixture.function(class, "b");
    let mut slots = vec![a, b];
    slots.sort_unstable_by(|left, right| right.cmp(left));
    let schema = vtable(&slots);
    let bytes = encode(&schema).unwrap();
    let decoded: DecodedInheritanceSlotSchemaV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);

    assert_eq!(decoded.resolve(&mut fixture).unwrap(), schema);
    assert_eq!(schema.slots(), slots);
    assert!(
        InheritanceSlotSchemaV1::try_new(InheritanceSlotSchemaRoleV1::ClassVtable, vec![a, a])
            .is_err()
    );
    let bad = RawSchema(vec![a, a]);
    let decoded: DecodedInheritanceSlotSchemaV1 = decode_canonical(&encode(&bad).unwrap()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture),
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
        decode_canonical(&encode(&table).unwrap()).unwrap();
    assert_eq!(decoded.resolve(&mut fixture).unwrap(), table);
    let reversed = RawTable(vec![interface(owner, &[]), vtable(&[])]);
    let decoded: DecodedCanonicalInheritanceSlotSchemasV1 =
        decode_canonical(&encode(&reversed).unwrap()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture),
        Err(InheritanceSlotSchemaResolutionError::Schema(
            InheritanceSlotSchemaBuildError::RoleOrder { .. }
        ))
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
