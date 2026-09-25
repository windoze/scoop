use super::*;
use scoop_wire::{Encoder, WireEncode};

#[test]
fn protected_callable_wire_preserves_source_semantics_and_uses_an_independent_ref_family() {
    let mut fixture = Fixture::default();
    let owner = fixture.class("Owner");
    let declaration = fixture.function(owner, "method", false, vec![]);
    let payload = fixture.payload(
        owner,
        declaration,
        vec![],
        SignatureTypeKey::Nominal(nominal(fixture.unit)),
    );
    let record = fixture.record(owner, declaration, payload.clone());
    let bytes = encode(&record).unwrap();
    let decoded: DecodedProtectedCallableInterfaceV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(decoded.clone().resolve(&mut fixture).unwrap(), record);

    assert_eq!(
        encode(&ProtectedSourceInterfaceUseV1::AccessorNoSourceInterface).unwrap(),
        [0xa1, 0, 1]
    );
    assert!(decode_canonical::<DecodedProtectedSourceInterfaceUseV1>(&[0xa1, 0, 5]).is_err());
    assert!(matches!(
        ProtectedCallableInterfaceV1::try_new(
            declaration,
            fixture.access(owner, DeclaredVisibilityV1::Public),
            payload
        ),
        Err(ProtectedCallableInterfaceBuildError::Access)
    ));
}

struct RawRefs(Vec<scoop_identity::PersistentDispatchSlotId>);
impl WireEncode for RawRefs {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for id in &self.0 {
            id.encode(encoder)?;
        }
        Ok(())
    }
}
#[test]
fn protected_slot_refs_reject_reordered_or_duplicate_reader_input() {
    let mut fixture = Fixture::default();
    let owner = fixture.class("Owner");
    let a = fixture.function(owner, "a", false, vec![]);
    let b = fixture.function(owner, "b", false, vec![]);
    let mut slots = vec![fixture.slot(a), fixture.slot(b)];
    slots.sort_unstable();
    assert_eq!(
        CanonicalProtectedSlotRefsV1::try_new(slots.iter().rev().copied().collect())
            .unwrap()
            .slots(),
        slots
    );
    assert!(CanonicalProtectedSlotRefsV1::try_new(vec![slots[0], slots[0]]).is_err());
    for input in [
        slots.iter().rev().copied().collect(),
        vec![slots[0], slots[0]],
    ] {
        let decoded: DecodedCanonicalProtectedSlotRefsV1 =
            decode_canonical(&encode(&RawRefs(input)).unwrap()).unwrap();
        assert!(matches!(
            decoded.resolve(&mut fixture),
            Err(ProtectedCallableInterfaceResolutionError::Interface(
                ProtectedCallableInterfaceBuildError::SlotOrder { .. }
            ))
        ));
    }
}
