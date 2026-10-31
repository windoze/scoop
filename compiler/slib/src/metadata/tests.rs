use scoop_wire::encode;

use super::*;
use crate::{
    hir_core_bootstrap_interface_capability, hir_identity_foundation_capability,
    lir_identity_foundation_capability, lir_strong_production_capability,
    manifest_single_cone_production_capability, mir_core_bootstrap_bridge_capability,
    mir_identity_foundation_capability,
};

fn capability(name: &str) -> CapabilityId {
    CapabilityId::new("org.scoop-lang.test", name, 1).unwrap()
}

#[test]
fn empty_outer_envelopes_have_distinct_fixed_magic() {
    for (location, expected) in [
        (MetadataLocation::Hir, "a4014853434f4f50484952020403800440"),
        (MetadataLocation::Mir, "a4014853434f4f504d4952020403800440"),
        (MetadataLocation::Lir, "a4014853434f4f504c4952020403800440"),
    ] {
        let envelope = MetadataEnvelope::new(location, Vec::new()).unwrap();
        assert_eq!(hex(&encode(&envelope).unwrap()), expected);
    }
}

#[test]
fn foundation_section_round_trips_without_copying_payload() {
    let payload = vec![0x18, 42];
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
    assert_eq!(
        MetadataReadError::UnsupportedSchema { actual: 1 }.to_string(),
        "metadata outer schema must be 4, found 1"
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

#[test]
fn storage_shares_ids_across_sections_and_preserves_opaque_extensions() {
    let digest = scoop_wire::sha256(b"one persistent identity");
    let payload = encode(&digest).unwrap();
    for (location, capabilities) in [
        (
            MetadataLocation::Hir,
            [
                hir_identity_foundation_capability(),
                hir_core_bootstrap_interface_capability(),
            ],
        ),
        (
            MetadataLocation::Mir,
            [
                mir_identity_foundation_capability(),
                mir_core_bootstrap_bridge_capability(),
            ],
        ),
        (
            MetadataLocation::Lir,
            [
                lir_identity_foundation_capability(),
                lir_strong_production_capability(),
            ],
        ),
    ] {
        let mut sections = capabilities
            .into_iter()
            .map(|capability| {
                let purpose = CapabilityContractRegistry::contract(&capability)
                    .unwrap()
                    .required_for();
                MetadataSection::new(location, capability, purpose, payload.clone()).unwrap()
            })
            .collect::<Vec<_>>();
        sections.push(
            MetadataSection::new(
                location,
                capability("opaque"),
                MemberPurposeSet::NONE,
                b"not CBOR".to_vec(),
            )
            .unwrap(),
        );
        let encoded = encode(&MetadataEnvelope::new(location, sections).unwrap()).unwrap();
        let raw = decode_canonical_borrowed::<UnvalidatedMetadataEnvelope<'_>>(&encoded).unwrap();
        assert_eq!(raw.id_table, digest.as_array());
        let decoded = raw.validate(location).unwrap();
        for section in decoded.sections() {
            if section.capability().namespace() == "org.scoop-lang.test" {
                assert_eq!(section.payload(), b"not CBOR");
            } else {
                assert_eq!(section.payload(), &[0]);
                assert_eq!(section.decode::<scoop_wire::Digest256>().unwrap(), digest);
            }
        }
    }
    // Storage interning must not enter semantic identity hashing.
    assert_eq!(encode(&digest).unwrap(), payload);
}

#[test]
fn storage_rejects_invalid_tables_and_out_of_range_references() {
    let digest = scoop_wire::sha256(b"identity");
    for (location, capability) in [
        (MetadataLocation::Hir, hir_identity_foundation_capability()),
        (MetadataLocation::Mir, mir_identity_foundation_capability()),
        (MetadataLocation::Lir, lir_identity_foundation_capability()),
    ] {
        let section = MetadataSection::new(
            location,
            capability,
            MemberPurposeSet::COMPILE_AND_LINK,
            encode(&digest).unwrap(),
        )
        .unwrap();
        let encoded = encode(&MetadataEnvelope::new(location, vec![section]).unwrap()).unwrap();
        for table in [&[0u8; 31][..], &[0u8; 64][..]] {
            let mut raw =
                decode_canonical_borrowed::<UnvalidatedMetadataEnvelope<'_>>(&encoded).unwrap();
            raw.id_table = table;
            assert_eq!(
                raw.validate(location),
                Err(MetadataReadError::InvalidIdTable)
            );
        }
        let mut raw =
            decode_canonical_borrowed::<UnvalidatedMetadataEnvelope<'_>>(&encoded).unwrap();
        raw.sections[0].payload = &[1];
        let decoded = raw.validate(location).unwrap();
        assert!(matches!(
            decoded.sections()[0]
                .decode::<scoop_wire::Digest256>()
                .unwrap_err()
                .kind(),
            scoop_wire::WireErrorKind::InvalidIndex {
                index: 1,
                length: 1
            }
        ));
    }
}
