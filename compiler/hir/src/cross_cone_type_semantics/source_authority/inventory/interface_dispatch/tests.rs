use super::*;
mod malformed;
use crate::cross_cone_type_semantics::slot_schemas::tests::support::Fixture;
use scoop_identity::{DecodedPersistentId, PersistentIdResolver, SourceNominalKind};
use scoop_wire::{DecodeLimits, WireDecode, decode_canonical, encode};

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
fn decoded<T: WireDecode>(value: &impl WireEncode) -> T {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}

#[test]
fn interface_source_wire_preserves_parent_member_order_and_canonical_overrides() {
    let mut fixture = Fixture::default();
    let a = fixture.add("A", SourceNominalKind::Interface);
    let b = fixture.add("B", SourceNominalKind::Interface);
    let c = fixture.add("C", SourceNominalKind::Interface);
    let inherited = fixture.function(a, "run");
    let own = fixture.function(c, "run");
    let second = fixture.function(c, "other");
    let members = vec![
        InterfaceSourceMemberV1::new(second, CanonicalPersistentIdsV1::empty()),
        InterfaceSourceMemberV1::new(
            own,
            CanonicalPersistentIdsV1::try_new(vec![inherited]).unwrap(),
        ),
    ];
    let source =
        InterfaceSourceDispatchV1::try_new(c.exact, vec![b.exact, a.exact], members, &mut meter())
            .unwrap();
    let expected = [
        vec![0xa3, 1],
        encode(&c.exact).unwrap(),
        vec![2, 0x82],
        encode(&b.exact).unwrap(),
        encode(&a.exact).unwrap(),
        vec![3, 0x82, 0xa2, 1],
        encode(&second).unwrap(),
        vec![2, 0x80, 0xa2, 1],
        encode(&own).unwrap(),
        vec![2, 0x81],
        encode(&inherited).unwrap(),
    ]
    .concat();
    assert_eq!(encode(&source).unwrap(), expected);
    let read: DecodedInterfaceSourceDispatchV1 = decoded(&source);
    assert_eq!(read.resolve(&mut fixture, &mut meter()).unwrap(), source);
    let table = CanonicalInterfaceSourceDispatchesV1::try_new(vec![source], &mut meter()).unwrap();
    let read: DecodedCanonicalInterfaceSourceDispatchesV1 = decoded(&table);
    assert_eq!(read.resolve(&mut fixture, &mut meter()).unwrap(), table);
}

#[test]
fn interface_source_rejects_duplicate_parents_members_self_overrides_and_wrong_maps() {
    let mut fixture = Fixture::default();
    let owner = fixture.add("Owner", SourceNominalKind::Interface).exact;
    let node = fixture.add("Parent", SourceNominalKind::Interface);
    let slot = fixture.function(node, "run");
    let member = InterfaceSourceMemberV1::new(slot, CanonicalPersistentIdsV1::empty());
    for (parents, members) in [
        (vec![node.exact, node.exact], vec![]),
        (vec![], vec![member.clone(), member]),
        (
            vec![],
            vec![InterfaceSourceMemberV1::new(
                slot,
                CanonicalPersistentIdsV1::try_new(vec![slot]).unwrap(),
            )],
        ),
    ] {
        assert!(matches!(
            InterfaceSourceDispatchV1::try_new(owner, parents, members, &mut meter()),
            Err(SourceInventoryError::InvalidInterfaceDispatch { .. })
        ));
    }
    assert!(
        decode_canonical::<DecodedInterfaceSourceDispatchV1>(&[0xa0], DecodeLimits::default())
            .is_err()
    );
}

#[test]
fn interface_source_reader_rejects_noncanonical_tables_unknown_refs_and_nested_budget_before_lookup()
 {
    let mut fixture = Fixture::default();
    let first = fixture.add("First", SourceNominalKind::Interface);
    let second = fixture.add("Second", SourceNominalKind::Interface);
    let record =
        InterfaceSourceDispatchV1::try_new(first.exact, vec![second.exact], vec![], &mut meter())
            .unwrap();
    let other =
        InterfaceSourceDispatchV1::try_new(second.exact, vec![], vec![], &mut meter()).unwrap();
    let table =
        CanonicalInterfaceSourceDispatchesV1::try_new(vec![record.clone(), other], &mut meter())
            .unwrap();
    for records in [
        vec![record.clone(), record.clone()],
        table.records().iter().rev().cloned().collect(),
    ] {
        let read: DecodedCanonicalInterfaceSourceDispatchesV1 = decoded(&RawTable(records));
        assert!(matches!(
            read.resolve(&mut fixture, &mut meter()),
            Err(SourceInventoryError::NonCanonicalOrder { .. })
        ));
    }
    let mut rejecting = Rejecting(0);
    let read: DecodedInterfaceSourceDispatchV1 = decoded(&record);
    assert!(matches!(
        read.resolve(
            &mut rejecting,
            &mut BudgetMeter::new(DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            })
        ),
        Err(SourceInventoryError::Resource(_))
    ));
    assert_eq!(rejecting.0, 0);
    let read: DecodedInterfaceSourceDispatchV1 = decoded(&record);
    assert!(matches!(
        read.resolve(&mut rejecting, &mut meter()),
        Err(SourceInventoryError::Reference(_))
    ));
}

#[test]
fn nested_override_budget_is_preflighted_before_the_owner_lookup() {
    let mut fixture = Fixture::default();
    let node = fixture.add("Owner", SourceNominalKind::Interface);
    let slot = fixture.function(node, "run");
    let first = fixture.function(node, "first");
    let second = fixture.function(node, "second");
    let record = InterfaceSourceDispatchV1::try_new(
        node.exact,
        vec![],
        vec![InterfaceSourceMemberV1::new(
            slot,
            CanonicalPersistentIdsV1::try_new(vec![first, second]).unwrap(),
        )],
        &mut meter(),
    )
    .unwrap();
    let mut resolver = Rejecting(0);
    for limits in [
        DecodeLimits {
            semantic_table_entries: 1,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
    ] {
        let read: DecodedInterfaceSourceDispatchV1 = decoded(&record);
        assert!(matches!(
            read.resolve(&mut resolver, &mut BudgetMeter::new(limits)),
            Err(SourceInventoryError::Resource(_))
        ));
        assert_eq!(resolver.0, 0);
    }
}

struct RawTable(Vec<InterfaceSourceDispatchV1>);
impl WireEncode for RawTable {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        super::super::wire::sequence(encoder, &self.0)
    }
}
struct Rejecting(usize);
impl PersistentIdResolver<PersistentExactTypeId> for Rejecting {
    type Error = &'static str;
    fn resolve(
        &mut self,
        _: DecodedPersistentId<PersistentExactTypeId>,
    ) -> Result<PersistentExactTypeId, Self::Error> {
        self.0 += 1;
        Err("unknown exact")
    }
}
impl PersistentIdResolver<PersistentDispatchSlotId> for Rejecting {
    type Error = &'static str;
    fn resolve(
        &mut self,
        _: DecodedPersistentId<PersistentDispatchSlotId>,
    ) -> Result<PersistentDispatchSlotId, Self::Error> {
        self.0 += 1;
        Err("unknown slot")
    }
}
