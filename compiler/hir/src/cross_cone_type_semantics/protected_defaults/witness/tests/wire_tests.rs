use super::*;
use scoop_wire::{Encoder, WireEncode, WireErrorKind};

#[test]
fn witness_branches_have_independent_exact_wire_shapes() {
    let mut fixture = Fixture::default();
    let owner = fixture.class("Owner");
    let function = fixture.function(owner, "method", false, vec![]);
    for (record, prefix) in [
        (param_free(function), [0xa5, 0, 1]),
        (
            ProtectedDefaultAccessWitnessV1::generic_source_metadata(function).unwrap(),
            [0xa2, 0, 2],
        ),
    ] {
        let bytes = encode(&record).unwrap();
        assert_eq!(&bytes[..3], &prefix);
        let decoded: DecodedProtectedDefaultAccessWitnessV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert_eq!(decoded.resolve(&mut fixture).unwrap(), record);
        let mut wide = bytes.clone();
        wide[0] += 1;
        assert!(matches!(
            decode_canonical::<DecodedProtectedDefaultAccessWitnessV1>(&wide)
                .unwrap_err()
                .kind(),
            WireErrorKind::InvalidLength { .. }
        ));
    }
    assert!(decode_canonical::<DecodedProtectedDefaultAccessWitnessV1>(&[0xa1, 0, 3]).is_err());
}

#[test]
fn constructor_and_generic_callable_defaults_cannot_claim_dispatch_roots() {
    let mut fixture = Fixture::default();
    let owner = fixture.class("Owner");
    let method = fixture.function(owner, "method", false, vec![]);
    let slot = fixture.slot(method);
    let generic = fixture.function(owner, "generic", true, vec![]);
    let constructor = CallableTemplateOrigin::Constructor(fixture.constructor(owner));
    let slots = CanonicalProtectedDefaultSlotCallDomainsV1::try_new(vec![
        ProtectedDefaultSlotCallDomainV1::new(
            slot,
            PersistentSlotContractDomainV1::new(PersistentAccessDomainV1::universal()),
        ),
    ])
    .unwrap();
    for declaration in [generic, constructor] {
        assert_eq!(
            ProtectedDefaultAccessWitnessV1::param_free(
                declaration,
                lookup(PersistentAccessDomainV1::universal()),
                slots.clone(),
                lookup(PersistentAccessDomainV1::universal())
            ),
            Err(ProtectedDefaultAccessWitnessBuildError::SlotsForNonDispatchOwner)
        );
        let raw = Raw {
            owner: declaration,
            slots: &slots,
        };
        let decoded: DecodedProtectedDefaultAccessWitnessV1 =
            decode_canonical(&encode(&raw).unwrap()).unwrap();
        assert!(matches!(
            decoded.resolve(&mut fixture),
            Err(ProtectedDefaultAccessWitnessResolutionError::Build(
                ProtectedDefaultAccessWitnessBuildError::SlotsForNonDispatchOwner
            ))
        ));
    }
}
struct Raw<'a> {
    owner: CallableTemplateOrigin,
    slots: &'a CanonicalProtectedDefaultSlotCallDomainsV1,
}
impl WireEncode for Raw<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(0)?;
        encoder.unsigned(1)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        PersistentAccessDomainV1::universal().encode(encoder)?;
        encoder.field(3)?;
        self.slots.encode(encoder)?;
        encoder.field(4)?;
        PersistentAccessDomainV1::universal().encode(encoder)
    }
}
