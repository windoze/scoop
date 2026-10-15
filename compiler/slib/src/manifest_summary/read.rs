use std::io::{self, Read};

use scoop_lir::ValidatedLirTargetSelection;

use super::{ArtifactManifestSummaryError as Error, ArtifactManifestSummaryV1};
use crate::archive::{GLOBAL_MAGIC, HEADER_LENGTH, read_member_header};
use crate::{ArchiveMemberOrdinal, ArchiveReadError, DecodedBootstrapManifest, SlibReadError};

/// Reads only the archive header and manifest, leaving all member payloads unread.
pub fn read_artifact_manifest_summary_from_reader(
    reader: &mut impl Read,
    selection: ValidatedLirTargetSelection,
) -> Result<ArtifactManifestSummaryV1, Error> {
    let container = |error| Error::Envelope(SlibReadError::Container(error));
    let mut magic = [0; GLOBAL_MAGIC.len()];
    reader.read_exact(&mut magic).map_err(Error::Io)?;
    if &magic != GLOBAL_MAGIC {
        return Err(container(ArchiveReadError::BadMagic));
    }
    let mut header = [0; HEADER_LENGTH as usize];
    reader.read_exact(&mut header).map_err(Error::Io)?;
    let length = read_member_header(&header, b"manifest.cbor", ArchiveMemberOrdinal::Manifest)
        .map_err(container)?;
    let mut bytes = Vec::new();
    reader
        .take(length as u64)
        .read_to_end(&mut bytes)
        .map_err(Error::Io)?;
    if bytes.len() != length {
        return Err(Error::Io(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "incomplete slib manifest",
        )));
    }
    let decoded: DecodedBootstrapManifest = scoop_wire::decode_canonical(&bytes)
        .map_err(|error| Error::Envelope(SlibReadError::CanonicalWire(error)))?;
    let manifest = decoded
        .validate(selection)
        .map_err(|error| Error::Envelope(SlibReadError::Manifest(Box::new(error))))?;
    ArtifactManifestSummaryV1::from_manifest(&manifest, selection).map_err(Error::TestDirectory)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_reads_only_the_manifest_of_a_complete_artifact() {
        let bytes = crate::link_decode::complete_strong_artifact_for_test(true);
        let manifest = crate::ManifestArchive::open(&bytes).unwrap();
        let end = GLOBAL_MAGIC.len() + HEADER_LENGTH as usize + manifest.manifest().len();
        let mut reader = io::Cursor::new(bytes.as_slice());
        let summary = read_artifact_manifest_summary_from_reader(
            &mut reader,
            scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
        )
        .unwrap();
        assert_eq!(reader.position(), end as u64);
        assert!(end < bytes.len());
        assert_eq!(
            summary.cone().identity(),
            crate::link_decode::complete_strong_artifact_identity_for_test()
        );
    }
}
