use scoop_wire::{DecodeLimits, WireEncode, decode_canonical, encode};

use super::*;

#[test]
fn empty_section_has_a_fixed_closed_wire() {
    let producer = ConeCoordinate::new("test", "empty-lir-bridge", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let foundation = empty_foundation(producer);
    let section =
        CrossConeLirBridgeSectionV1::try_new(&foundation, Vec::new(), Vec::new()).unwrap();
    let bytes = encode(&section).unwrap();

    assert_eq!(bytes, vec![0xa2, 0x01, 0x80, 0x02, 0x80]);
    let decoded =
        decode_canonical::<DecodedCrossConeLirBridgeSectionV1>(&bytes, DecodeLimits::default())
            .unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(producer).unwrap();
    let mut identities = pending.finish().unwrap();
    assert_eq!(
        decoded.validate(&mut identities, &foundation).unwrap(),
        section
    );
}

#[test]
fn decoded_section_rejects_a_forged_required_definition() {
    let fixture = Fixture::new("forgedDefinition");
    let export = fixture.export();
    let section =
        CrossConeLirBridgeSectionV1::try_new(&fixture.foundation, vec![export.clone()], Vec::new())
            .unwrap();
    let mut bytes = encode(&section).unwrap();
    let required_definition = export.required_definition();
    let needle = required_definition.as_array();
    let positions = bytes
        .windows(needle.len())
        .enumerate()
        .filter_map(|(index, window)| (window == needle).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 1);
    bytes[positions[0] + needle.len() - 1] ^= 1;

    let decoded =
        decode_canonical::<DecodedCrossConeLirBridgeSectionV1>(&bytes, DecodeLimits::default())
            .unwrap();
    assert!(matches!(
        decoded.validate(&mut fixture.identities(&[]), &fixture.foundation),
        Err(CrossConeLirBridgeValidationError::SectionMismatch)
    ));
}

#[test]
fn reader_rejects_noncanonical_export_order() {
    let first = Fixture::new("first");
    let second = Fixture::new("second");
    let mut exports = vec![first.export(), second.export()];
    exports.sort_unstable_by_key(ParamFreeLirCallableExportV1::declaration);
    exports.reverse();
    let bytes = encode(&RawSection(&exports)).unwrap();
    let decoded =
        decode_canonical::<DecodedCrossConeLirBridgeSectionV1>(&bytes, DecodeLimits::default())
            .unwrap();
    let mut identities = first.identities(&[second.declaration]);

    assert!(matches!(
        decoded.validate(&mut identities, &first.foundation),
        Err(CrossConeLirBridgeValidationError::NonCanonicalExportOrder { index: 1 })
    ));
}

#[test]
fn reader_rejects_unknown_root_plan_and_open_section_shapes() {
    for bytes in [
        vec![0xa1, 0x01, 0x80],
        vec![0xa3, 0x01, 0x80, 0x02, 0x80, 0x03, 0x80],
    ] {
        assert!(
            decode_canonical::<DecodedCrossConeLirBridgeSectionV1>(
                &bytes,
                DecodeLimits::default(),
            )
            .is_err()
        );
    }
    assert!(
        decode_canonical::<DependencyExternalCallableRootPlanV1>(&[0x03], DecodeLimits::default(),)
            .is_err()
    );
}

struct RawSection<'a>(&'a [ParamFreeLirCallableExportV1]);

impl WireEncode for RawSection<'_> {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.array(self.0.len() as u64)?;
        for export in self.0 {
            export.encode(encoder)?;
        }
        encoder.field(2)?;
        encoder.array(0)
    }
}
