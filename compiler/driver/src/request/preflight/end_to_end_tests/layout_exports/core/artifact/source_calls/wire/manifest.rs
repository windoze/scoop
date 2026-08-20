//! Keep all digests valid so a semantic corruption reaches the HIR reader.

use super::*;

pub(super) fn with_hir_fingerprint(
    manifest: &slib::BootstrapManifest,
    hir: slib::HirFingerprint,
) -> Vec<u8> {
    let mut bytes = encode(manifest).unwrap();
    let fingerprints = encode(&manifest.semantic_fingerprints()).unwrap();
    let positions = bytes
        .windows(fingerprints.len())
        .enumerate()
        .filter_map(|(index, value)| (value == fingerprints).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 1);
    // The canonical five-field fingerprint record starts with its HIR digest.
    assert_eq!(&fingerprints[..4], &[0xa5, 1, 0x58, 32]);
    let start = positions[0] + 4;
    bytes[start..start + 32].copy_from_slice(hir.as_array());

    // Artifact hashing excludes field 11, then includes every member digest.
    assert_eq!(bytes[0], 0xab);
    let payload_end = bytes.len() - 35;
    assert_eq!(&bytes[payload_end..payload_end + 3], &[11, 0x58, 32]);
    let mut payload = bytes[..payload_end].to_vec();
    payload[0] = 0xaa;
    let mut hash = scoop_wire::CanonicalHashStream::new();
    hash.update_byte_span(b"scoop-artifact-v1").unwrap();
    hash.update_byte_span(&payload).unwrap();
    for member in manifest.members() {
        hash.update_byte_span(member.fingerprint().unwrap().as_array())
            .unwrap();
    }
    bytes[payload_end + 3..].copy_from_slice(hash.finalize().as_array());
    bytes
}
