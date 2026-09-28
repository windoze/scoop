//! Repack one changed production section with correct container fingerprints.

use super::*;
use scoop_wire::{Encoder, WireEncode};

pub(super) fn replace_production(
    target: &scoop_toolchain::ResolvedTargetProfile,
    bytes: &[u8],
    replacement: Vec<u8>,
) -> Vec<u8> {
    let archive = object::read::archive::ArchiveFile::parse(bytes).unwrap();
    let mut entries = archive.members();
    let first = entries.next().unwrap().unwrap();
    assert_eq!(first.name(), b"manifest.cbor");
    let manifest =
        scoop_wire::decode_canonical::<slib::DecodedBootstrapManifest>(first.data(bytes).unwrap())
            .unwrap()
            .validate(target.lir_target_selection())
            .unwrap();
    let mut members = Vec::new();
    let mut lir_fingerprint = None;
    let mut changed = 0;
    for record in manifest.members() {
        let original = entries.next().unwrap().unwrap().data(bytes).unwrap();
        let payload = if matches!(record.role(), slib::SlibMemberRole::LirMetadata) {
            let envelope =
                slib::DecodedMetadataEnvelope::decode(original, slib::MetadataLocation::Lir)
                    .unwrap();
            let sections = envelope
                .sections()
                .iter()
                .map(|section| {
                    let payload = if section.capability() == &slib::lir_cone_production_capability()
                    {
                        changed += 1;
                        replacement.clone()
                    } else {
                        section.payload().to_vec()
                    };
                    slib::MetadataSection::new(
                        slib::MetadataLocation::Lir,
                        section.capability().clone(),
                        section.required_for(),
                        payload,
                    )
                    .unwrap()
                })
                .collect();
            let envelope =
                slib::MetadataEnvelope::new(slib::MetadataLocation::Lir, sections).unwrap();
            lir_fingerprint = Some(
                slib::SemanticFingerprintRecord::from_metadata_sections(
                    manifest.compatibility(),
                    manifest.direct_dependencies(),
                    &[],
                    &[],
                    envelope.sections(),
                )
                .unwrap()
                .lir(),
            );
            encode(&envelope).unwrap()
        } else {
            original.to_vec()
        };
        members.push(
            slib::SlibMember::new(
                manifest.cone().identity(),
                record.stable_key().clone(),
                record.role().clone(),
                payload,
            )
            .unwrap(),
        );
    }
    assert!(entries.next().is_none());
    assert_eq!(changed, 1);
    let original = manifest.semantic_fingerprints();
    let fingerprints = slib::SemanticFingerprintRecord::from_digests(
        original.hir(),
        original.mir(),
        lir_fingerprint.unwrap(),
        original.code(),
        original.runtime_image(),
    );
    let manifest = slib::BootstrapManifest::new(
        slib::ProducerRecord::new("delegate-registration-negative").unwrap(),
        manifest.compatibility().clone(),
        manifest.cone().clone(),
        manifest.direct_dependencies().to_vec(),
        &members,
        fingerprints,
        manifest.sections().to_vec(),
    )
    .unwrap();
    slib::CanonicalSlibArchive::write_bootstrap(&manifest, members)
        .unwrap()
        .into_bytes()
}

pub(super) fn replace_once(bytes: &[u8], old: &[u8], new: &[u8]) -> Vec<u8> {
    let positions = bytes
        .windows(old.len())
        .enumerate()
        .filter_map(|(index, value)| (value == old).then_some(index))
        .collect::<Vec<_>>();
    let [position] = positions.as_slice() else {
        panic!(
            "one actual wire record must match, found {}",
            positions.len()
        );
    };
    [&bytes[..*position], new, &bytes[position + old.len()..]].concat()
}

pub(super) fn remove_plan<T: WireEncode>(bytes: &[u8], plans: &[T], index: usize) -> Vec<u8> {
    struct Plans<'a, T> {
        values: &'a [T],
        skip: Option<usize>,
    }
    impl<T: WireEncode> WireEncode for Plans<'_, T> {
        fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
            encoder.array((self.values.len() - usize::from(self.skip.is_some())) as u64)?;
            for (index, value) in self.values.iter().enumerate() {
                if self.skip != Some(index) {
                    value.encode(encoder)?;
                }
            }
            Ok(())
        }
    }
    assert!(index < plans.len());
    replace_once(
        bytes,
        &encode(&Plans {
            values: plans,
            skip: None,
        })
        .unwrap(),
        &encode(&Plans {
            values: plans,
            skip: Some(index),
        })
        .unwrap(),
    )
}

pub(super) struct CallableReference(pub(super) lir::StrongInitializationCallableRefPlanV1);

impl WireEncode for CallableReference {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let value = self.0;
        encoder.map(9)?;
        encoder.field(1)?;
        value.body().encode(encoder)?;
        encoder.field(2)?;
        value.entry_symbol().encode(encoder)?;
        encoder.field(3)?;
        value.registration_symbol().encode(encoder)?;
        encoder.field(4)?;
        value.body_definition_plan().encode(encoder)?;
        encoder.field(5)?;
        value.body_primary_atom().encode(encoder)?;
        encoder.field(6)?;
        value.body_definition_node().encode(encoder)?;
        encoder.field(7)?;
        value.registration_definition_plan().encode(encoder)?;
        encoder.field(8)?;
        value.registration_primary_atom().encode(encoder)?;
        encoder.field(9)?;
        value.registration_fingerprint_node().encode(encoder)
    }
}
