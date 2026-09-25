//! Rebuild the outer archive digest while keeping a deliberately false Code value.

use super::*;

pub(super) fn rewrite(bytes: &[u8], selection: lir::ValidatedLirTargetSelection) -> Vec<u8> {
    let archive = object::read::archive::ArchiveFile::parse(bytes).unwrap();
    let mut entries = archive.members();
    let original = entries.next().unwrap().unwrap().data(bytes).unwrap();
    let manifest = scoop_wire::decode_canonical::<slib::DecodedBootstrapManifest>(
        original,
        DecodeLimits::default(),
    )
    .unwrap()
    .validate(selection, &mut meter())
    .unwrap();
    let members = manifest
        .members()
        .iter()
        .map(|record| {
            slib::SlibMember::new(
                manifest.cone().identity(),
                record.stable_key().clone(),
                record.role().clone(),
                entries
                    .next()
                    .unwrap()
                    .unwrap()
                    .data(bytes)
                    .unwrap()
                    .to_vec(),
            )
            .unwrap()
        })
        .collect();
    assert!(entries.next().is_none());
    let mut changed = original.to_vec();
    let semantic = wire::field_range(original, 9);
    let code = wire::field_range(&original[semantic.clone()], 4);
    changed[semantic.start + code.end - 1] ^= 1;
    let decoded = scoop_wire::decode_canonical::<slib::DecodedBootstrapManifest>(
        &changed,
        DecodeLimits::default(),
    )
    .unwrap();
    let slib::BootstrapManifestValidationError::ArtifactFingerprintMismatch { expected, .. } =
        decoded.validate(selection, &mut meter()).unwrap_err()
    else {
        panic!("only the enclosing artifact digest is stale")
    };
    changed = wire::replace_field(&changed, 11, &encode(&expected).unwrap());
    slib::CanonicalSlibArchive::write(&changed, members)
        .unwrap()
        .into_bytes()
}
