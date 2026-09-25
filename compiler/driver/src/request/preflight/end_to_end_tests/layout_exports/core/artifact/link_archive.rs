//! Rehash one intentional Link mutation without changing shared semantics.

use super::*;
use object::read::macho::MachHeader as _;
use object::{Endianness, macho};
use scoop_slib as slib;

mod manifest;
pub(super) use manifest::rewrite_production;

pub(super) fn symbol_offset(bytes: &[u8], index: u32) -> usize {
    let header = macho::MachHeader64::<Endianness>::parse(bytes, 0).unwrap();
    let endian = header.endian().unwrap();
    let mut commands = header.load_commands(endian, bytes, 0).unwrap();
    while let Some(command) = commands.next().unwrap() {
        if let Some(table) = command.symtab().unwrap() {
            let symbols = table
                .symbols::<macho::MachHeader64<Endianness>, _>(endian, bytes)
                .unwrap();
            let symbol = symbols.iter().nth(index as usize).unwrap();
            return table.stroff.get(endian) as usize + symbol.n_strx.get(endian) as usize;
        }
    }
    panic!("verified object has a symbol table");
}

pub(super) enum Rewrite {
    Keep,
    Replace(Vec<u8>),
    Remove,
}

pub(super) fn rewrite(
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    mut change: impl FnMut(&slib::SlibMemberRecord, &[u8]) -> Rewrite,
) -> Vec<u8> {
    let bytes = artifact.as_bytes();
    let archive = object::read::archive::ArchiveFile::parse(bytes).unwrap();
    let mut entries = archive.members();
    let first = entries.next().unwrap().unwrap();
    assert_eq!(first.name(), b"manifest.cbor");
    let manifest = scoop_wire::decode_canonical::<slib::DecodedBootstrapManifest>(
        first.data(bytes).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap()
    .validate(artifact.target_selection(), &mut meter())
    .unwrap();
    let mut members = Vec::new();
    let mut changed = 0;
    for record in manifest.members() {
        let data = entries.next().unwrap().unwrap().data(bytes).unwrap();
        let payload = match change(record, data) {
            Rewrite::Keep => data.to_vec(),
            Rewrite::Replace(payload) => {
                changed += 1;
                payload
            }
            Rewrite::Remove => {
                changed += 1;
                continue;
            }
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
    let manifest = slib::BootstrapManifest::new(
        slib::ProducerRecord::new("layout-link-negative").unwrap(),
        manifest.compatibility().clone(),
        manifest.cone().clone(),
        manifest.direct_dependencies().to_vec(),
        &members,
        manifest.semantic_fingerprints(),
        manifest.sections().to_vec(),
    )
    .unwrap();
    slib::CanonicalSlibArchive::write_bootstrap(&manifest, members)
        .unwrap()
        .into_bytes()
}

pub(super) fn metadata(
    payload: &[u8],
    mut change: impl FnMut(&slib::DecodedMetadataSection<'_>) -> Vec<u8>,
) -> Vec<u8> {
    let decoded = slib::DecodedMetadataEnvelope::decode(
        payload,
        slib::MetadataLocation::Lir,
        DecodeLimits::default(),
    )
    .unwrap();
    let sections = decoded
        .sections()
        .iter()
        .map(|section| {
            slib::MetadataSection::new(
                slib::MetadataLocation::Lir,
                section.capability().clone(),
                section.required_for(),
                change(section),
            )
            .unwrap()
        })
        .collect();
    encode(&slib::MetadataEnvelope::new(slib::MetadataLocation::Lir, sections).unwrap()).unwrap()
}

pub(super) fn payloads(
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
) -> std::collections::BTreeMap<slib::SlibMemberId, &[u8]> {
    let bytes = artifact.as_bytes();
    let archive = object::read::archive::ArchiveFile::parse(bytes).unwrap();
    let mut entries = archive.members();
    let first = entries.next().unwrap().unwrap();
    let manifest = scoop_wire::decode_canonical::<slib::DecodedBootstrapManifest>(
        first.data(bytes).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap()
    .validate(artifact.target_selection(), &mut meter())
    .unwrap();
    manifest
        .members()
        .iter()
        .map(|record| {
            (
                record.id(),
                entries.next().unwrap().unwrap().data(bytes).unwrap(),
            )
        })
        .collect()
}
