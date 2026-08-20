use scoop_wire::encode;

use super::*;
use crate::{
    hir_identity_foundation_capability, lir_identity_foundation_capability,
    lir_strong_production_capability, manifest_single_cone_production_capability,
};

fn capability(name: &str) -> CapabilityId {
    CapabilityId::new("org.scoop-lang.test", name, 1).unwrap()
}

#[test]
fn empty_outer_envelopes_have_distinct_fixed_magic() {
    for (location, expected) in [
        (MetadataLocation::Hir, "a3014853434f4f5048495202020380"),
        (MetadataLocation::Mir, "a3014853434f4f504d495202020380"),
        (MetadataLocation::Lir, "a3014853434f4f504c495202020380"),
    ] {
        let envelope = MetadataEnvelope::new(location, Vec::new()).unwrap();
        assert_eq!(hex(&encode(&envelope).unwrap()), expected);
    }
}

#[test]
fn foundation_section_round_trips_without_copying_payload() {
    let payload = vec![0x55; 65];
    let envelope = MetadataEnvelope::new(
        MetadataLocation::Hir,
        vec![
            MetadataSection::new(
                MetadataLocation::Hir,
                hir_identity_foundation_capability(),
                MemberPurposeSet::COMPILE_AND_LINK,
                payload.clone(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let encoded = encode(&envelope).unwrap();
    let decoded = DecodedMetadataEnvelope::decode(&encoded, MetadataLocation::Hir).unwrap();

    assert_eq!(decoded.sections()[0].payload(), payload);
    let payload_pointer = decoded.sections()[0].payload().as_ptr() as usize;
    let input_start = encoded.as_ptr() as usize;
    assert!(payload_pointer >= input_start && payload_pointer < input_start + encoded.len());
}

#[test]
fn decoder_rejects_wrong_magic_schema_and_wire_order() {
    let first = MetadataSection::new(
        MetadataLocation::Hir,
        capability("first"),
        MemberPurposeSet::NONE,
        Vec::new(),
    )
    .unwrap();
    let second = MetadataSection::new(
        MetadataLocation::Hir,
        capability("second"),
        MemberPurposeSet::NONE,
        Vec::new(),
    )
    .unwrap();
    let encoded =
        encode(&MetadataEnvelope::new(MetadataLocation::Hir, vec![first, second]).unwrap())
            .unwrap();

    assert_eq!(
        DecodedMetadataEnvelope::decode(&encoded, MetadataLocation::Mir,),
        Err(MetadataReadError::BadMagic {
            expected: MetadataLocation::Mir,
        })
    );

    let mut unvalidated =
        decode_canonical_borrowed::<UnvalidatedMetadataEnvelope<'_>>(&encoded).unwrap();
    unvalidated.outer_schema = 1;
    assert_eq!(
        unvalidated.validate(MetadataLocation::Hir),
        Err(MetadataReadError::UnsupportedSchema { actual: 1 })
    );

    let mut unvalidated =
        decode_canonical_borrowed::<UnvalidatedMetadataEnvelope<'_>>(&encoded).unwrap();
    unvalidated.sections.swap(0, 1);
    assert!(matches!(
        unvalidated.validate(MetadataLocation::Hir),
        Err(MetadataReadError::NonIncreasingSection { .. })
    ));
}

#[test]
fn purpose_and_known_location_contracts_are_closed() {
    assert!(
        MetadataSection::new(
            MetadataLocation::Lir,
            capability("link"),
            MemberPurposeSet::LINK,
            Vec::new(),
        )
        .is_ok()
    );
    assert!(
        MetadataSection::new(
            MetadataLocation::Lir,
            capability("compile-link"),
            MemberPurposeSet::COMPILE_AND_LINK,
            Vec::new(),
        )
        .is_ok()
    );
    assert!(matches!(
        MetadataSection::new(
            MetadataLocation::Hir,
            capability("link"),
            MemberPurposeSet::LINK,
            Vec::new(),
        ),
        Err(MetadataSectionError::InvalidPurpose { .. })
    ));
    assert!(matches!(
        MetadataSection::new(
            MetadataLocation::Mir,
            lir_identity_foundation_capability(),
            MemberPurposeSet::COMPILE_AND_LINK,
            Vec::new(),
        ),
        Err(MetadataSectionError::KnownCapabilityWrongLocation { .. })
    ));
    assert!(matches!(
        MetadataSection::new(
            MetadataLocation::Hir,
            hir_identity_foundation_capability(),
            MemberPurposeSet::NONE,
            Vec::new(),
        ),
        Err(MetadataSectionError::KnownCapabilityWrongPurpose { .. })
    ));
    assert!(matches!(
        MetadataSection::new(
            MetadataLocation::Lir,
            lir_strong_production_capability(),
            MemberPurposeSet::COMPILE,
            Vec::new(),
        ),
        Err(MetadataSectionError::KnownCapabilityWrongPurpose { .. })
    ));
    assert!(matches!(
        MetadataSection::new(
            MetadataLocation::Lir,
            manifest_single_cone_production_capability(),
            MemberPurposeSet::LINK,
            Vec::new(),
        ),
        Err(MetadataSectionError::KnownCapabilityWrongLocation { .. })
    ));
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
