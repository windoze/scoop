use super::*;
use scoop_wire::{Encoder, WireEncode};

struct Records<'a>(&'a [&'a NominalInheritanceInterfaceV1]);
impl WireEncode for Records<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for record in self.0 {
            record.encode(encoder)?;
        }
        Ok(())
    }
}
#[test]
fn inheritance_wire_is_a_strict_flat_nine_field_product() {
    let mut bundle = fixture();
    let record = bundle.table.get(bundle.base.exact).unwrap();
    let bytes = encode(record).unwrap();
    assert_eq!(bytes[0], 0xa9);
    let decoded: DecodedNominalInheritanceInterfaceV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(
        decoded.resolve(&mut bundle.fixture, &mut meter()).unwrap(),
        *record
    );
    let mut short = bytes;
    short[0] = 0xa8;
    assert!(
        decode_canonical::<DecodedNominalInheritanceInterfaceV1>(&short, DecodeLimits::default())
            .is_err()
    );
}
#[test]
fn inheritance_reader_rejects_owner_order_duplicates_and_resource_exhaustion() {
    let mut bundle = fixture();
    let records = bundle.table.records();
    for order in [[&records[1], &records[0]], [&records[0], &records[0]]] {
        let decoded: DecodedCanonicalNominalInheritanceInterfacesV1 =
            decode_canonical(&encode(&Records(&order)).unwrap(), DecodeLimits::default()).unwrap();
        assert!(matches!(
            decoded.resolve(&mut bundle.fixture, &mut meter()),
            Err(InheritanceInterfaceResolutionError::Build(
                InheritanceInterfaceBuildError::OwnerOrder
            ))
        ));
    }
    let decoded: DecodedNominalInheritanceInterfaceV1 =
        decode_canonical(&encode(&records[0]).unwrap(), DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.resolve(
            &mut bundle.fixture,
            &mut BudgetMeter::new(DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            })
        ),
        Err(InheritanceInterfaceResolutionError::Resource(_))
    ));
}
#[test]
fn slot_schema_union_and_nominal_empty_slot_domain_are_required_on_read() {
    for mode in 0..3 {
        let mut bundle = fixture();
        bundle.change(bundle.base, |record| match mode {
            0 => record.slots = CanonicalInheritanceSlotContractsV1::try_new(vec![]).unwrap(),
            1 => record.slot_schemas = CanonicalInheritanceSlotSchemasV1::default(),
            2 => {
                record.domains = NominalAccessDomainsV1::new(
                    record.domains.lookup().clone(),
                    record.domains.inheritance().clone(),
                    PersistentSlotContractDomainV1::new(PersistentAccessDomainV1::universal()),
                )
            }
            _ => unreachable!(),
        });
        let decoded: DecodedCanonicalNominalInheritanceInterfacesV1 =
            decode_canonical(&encode(&bundle.table).unwrap(), DecodeLimits::default()).unwrap();
        let expected = if mode == 2 {
            InheritanceInterfaceBuildError::NominalSlotDomain
        } else {
            InheritanceInterfaceBuildError::SlotClosure
        };
        assert!(
            matches!(decoded.resolve(&mut bundle.fixture, &mut meter()), Err(InheritanceInterfaceResolutionError::Build(actual)) if actual == expected)
        );
    }
}
