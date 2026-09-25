use super::*;
use scoop_wire::{Encoder, WireEncode, decode_canonical, encode};

#[test]
fn complete_slot_wire_round_trips_abstract_concrete_and_default_targets() {
    let mut fixture = Fixture::default();
    let class = fixture.add("Class", SourceNominalKind::Class);
    let slot = fixture.function(class, "method");
    let interface = fixture.add("Interface", SourceNominalKind::Interface);
    let default_slot = fixture.function(interface, "method");
    let mut target = fixture.concrete(interface, default_slot);
    target.modality = CallableModalityV1::InterfaceDefault;
    let records = [
        fixture.contract(class, slot, InheritanceSlotImplementationV1::Abstract),
        fixture.contract(
            class,
            slot,
            InheritanceSlotImplementationV1::Concrete(fixture.concrete(class, slot)),
        ),
        fixture.contract(
            interface,
            default_slot,
            InheritanceSlotImplementationV1::InterfaceDefault(target),
        ),
    ];
    for record in records {
        let bytes = encode(&record).unwrap();
        assert_eq!(bytes[0], 0xa7);
        let decoded: DecodedInheritanceSlotContractV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert_eq!(decoded.clone().resolve(&mut fixture).unwrap(), record);
    }
    assert_eq!(
        encode(&InheritanceSlotImplementationV1::Abstract).unwrap(),
        [0xa1, 0, 1]
    );
    assert!(decode_canonical::<DecodedInheritanceSlotImplementationV1>(&[0xa1, 0, 4]).is_err());
    assert!(decode_canonical::<DecodedInheritanceCallableDeclarationV1>(&[0xa1, 0, 1]).is_err());
}

struct RawTable(Vec<InheritanceSlotContractV1>);
impl WireEncode for RawTable {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for record in &self.0 {
            record.encode(encoder)?;
        }
        Ok(())
    }
}
#[test]
fn contract_tables_sort_only_at_production_and_reject_wire_reordering_or_duplicates() {
    let mut fixture = Fixture::default();
    let owner = fixture.add("Owner", SourceNominalKind::Class);
    let a = fixture.function(owner, "a");
    let b = fixture.function(owner, "b");
    let records =
        [a, b].map(|slot| fixture.contract(owner, slot, InheritanceSlotImplementationV1::Abstract));
    let table = CanonicalInheritanceSlotContractsV1::try_new(records.to_vec()).unwrap();
    let decoded: DecodedCanonicalInheritanceSlotContractsV1 =
        decode_canonical(&encode(&table).unwrap()).unwrap();
    assert_eq!(decoded.resolve(&mut fixture).unwrap(), table);
    assert!(
        CanonicalInheritanceSlotContractsV1::try_new(vec![records[0].clone(), records[0].clone()])
            .is_err()
    );
    for records in [
        table.records().iter().rev().cloned().collect(),
        vec![records[0].clone(), records[0].clone()],
    ] {
        let decoded: DecodedCanonicalInheritanceSlotContractsV1 =
            decode_canonical(&encode(&RawTable(records)).unwrap()).unwrap();
        assert!(matches!(
            decoded.resolve(&mut fixture),
            Err(InheritanceSlotResolutionError::Contract(
                InheritanceSlotContractBuildError::SlotOrder { .. }
            ))
        ));
    }
}
