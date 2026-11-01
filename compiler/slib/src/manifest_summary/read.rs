use std::io::{self, Read};

use scoop_lir::ValidatedLirTargetSelection;

use super::{ArtifactManifestSummaryError as Error, ArtifactManifestSummaryV1};
use crate::archive::{HEADER_LENGTH, decompress_member, read_manifest_header};
use crate::{ArchiveMemberOrdinal, ArchiveReadError, DecodedBootstrapManifest, SlibReadError};

/// Reads only the archive header and manifest, leaving all member payloads unread.
pub fn read_artifact_manifest_summary_from_reader(
    reader: &mut impl Read,
    selection: ValidatedLirTargetSelection,
) -> Result<ArtifactManifestSummaryV1, Error> {
    let (manifest, _) = read_manifest(reader, selection).map_err(|error| match error {
        SlibReadError::Io(source) => Error::Io(source),
        source => Error::Envelope(source),
    })?;
    ArtifactManifestSummaryV1::from_manifest(&manifest, selection).map_err(Error::TestDirectory)
}

pub(crate) fn read_manifest(
    reader: &mut impl Read,
    selection: ValidatedLirTargetSelection,
) -> Result<(crate::BootstrapManifest, u64), SlibReadError> {
    let mut header = [0; HEADER_LENGTH as usize];
    reader.read_exact(&mut header).map_err(SlibReadError::Io)?;
    let length = read_manifest_header(&header).map_err(SlibReadError::Container)?;
    let next = HEADER_LENGTH
        .checked_add(length as u64)
        .ok_or(SlibReadError::Container(ArchiveReadError::LengthOverflow))?;
    let mut bytes = Vec::new();
    reader
        .take(length as u64)
        .read_to_end(&mut bytes)
        .map_err(SlibReadError::Io)?;
    if bytes.len() != length {
        return Err(SlibReadError::Io(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "incomplete slib manifest",
        )));
    }
    let bytes = decompress_member(&bytes, ArchiveMemberOrdinal::Manifest)
        .map_err(SlibReadError::Container)?;
    let decoded: DecodedBootstrapManifest =
        scoop_wire::decode_canonical(&bytes).map_err(SlibReadError::CanonicalWire)?;
    let manifest = decoded
        .validate(selection)
        .map_err(|error| SlibReadError::Manifest(Box::new(error)))?;
    Ok((manifest, next))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_rejects_overflow_before_reading_a_manifest_payload() {
        let mut bytes = crate::archive::GLOBAL_MAGIC.to_vec();
        bytes.extend_from_slice(&u64::MAX.to_le_bytes());
        let mut reader = io::Cursor::new(bytes);
        assert!(matches!(
            read_manifest(&mut reader, scoop_target_data::DARWIN_AARCH64_LLVM_22_1),
            Err(SlibReadError::Container(ArchiveReadError::LengthOverflow))
        ));
        assert_eq!(reader.position(), HEADER_LENGTH);
    }

    #[test]
    fn summary_reads_only_the_manifest_of_a_complete_artifact() {
        let bytes = crate::link_decode::complete_strong_artifact_for_test();
        let header_end = HEADER_LENGTH as usize;
        let length = read_manifest_header(&bytes[..header_end]).unwrap();
        let end = header_end + length;
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
