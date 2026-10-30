//! Small build-result records, without archive bytes or production replay.
use super::*;
use scoop_identity::DecodedCapabilityId;
use scoop_wire::{Decoder, Digest256, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

impl WireEncode for ArtifactManifestSummaryV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(7)?;
        e.field(1)?;
        self.cone.encode(e)?;
        e.field(2)?;
        e.array(self.direct_dependencies.len() as u64)?;
        for dependency in &self.direct_dependencies {
            dependency.encode(e)?;
        }
        e.field(3)?;
        e.text(self.target_selection.target().id().canonical_name())?;
        e.field(4)?;
        self.profile.encode(e)?;
        e.field(5)?;
        self.semantic_fingerprints.encode(e)?;
        e.field(6)?;
        self.artifact_fingerprint.encode(e)?;
        e.field(7)?;
        e.array(u64::from(self.test_directory.is_some()))?;
        if let Some(directory) = &self.test_directory {
            directory.encode(e)?;
        }
        Ok(())
    }
}

impl WireDecode for ArtifactManifestSummaryV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(7)?;
        let cone = d.field(1, |d| {
            crate::DecodedConeRecord::decode(d)?
                .validate()
                .map_err(|_| invalid(d))
        })?;
        let direct_dependencies = d.field(2, |d| {
            d.decode_array(|d, _| {
                crate::DecodedDependencyRecord::decode(d)?
                    .validate()
                    .map_err(|_| invalid(d))
            })
        })?;
        let target_selection = d.field(3, |d| {
            let name = d.text()?;
            scoop_target_data::TARGETS
                .iter()
                .map(|target| target.selection)
                .find(|target| target.target().id().canonical_name() == name)
                .ok_or_else(|| invalid(d))
        })?;
        let profile = d.field(4, |d| {
            let capability = DecodedCapabilityId::decode(d)?
                .validate()
                .map_err(|_| invalid(d))?;
            ArtifactCapabilityProfileId::refine(capability).map_err(|_| invalid(d))
        })?;
        let semantic_fingerprints = d.field(5, semantic)?;
        let artifact_fingerprint = d.field(6, |d| {
            Ok(ArtifactFingerprint::from_array(
                *Digest256::decode(d)?.as_array(),
            ))
        })?;
        let test_directory = d.field(7, |d| match d.array()? {
            0 => Ok(None),
            1 => crate::TestDirectory::decode(d).map(Some),
            _ => Err(invalid(d)),
        })?;
        Ok(Self::new(
            cone,
            direct_dependencies,
            target_selection,
            profile,
            semantic_fingerprints,
            artifact_fingerprint,
            test_directory,
        ))
    }
}

fn semantic(d: &mut Decoder<'_>) -> Result<SemanticFingerprintRecord, WireError> {
    d.expect_map(5)?;
    Ok(SemanticFingerprintRecord::from_digests(
        crate::HirFingerprint::from_array(*d.field(1, Digest256::decode)?.as_array()),
        crate::MirFingerprint::from_array(*d.field(2, Digest256::decode)?.as_array()),
        crate::LirFingerprint::from_array(*d.field(3, Digest256::decode)?.as_array()),
        d.field(4, |d| availability(d, crate::CodeFingerprint::from_array))?,
        d.field(5, |d| availability(d, RuntimeImageFingerprint::from_array))?,
    ))
}

fn availability<T>(
    d: &mut Decoder<'_>,
    from: impl FnOnce([u8; 32]) -> T,
) -> Result<FingerprintAvailability<T>, WireError> {
    let count = d.map()?;
    match (d.field(0, Decoder::unsigned)?, count) {
        (1, 1) => Ok(FingerprintAvailability::Unavailable),
        (2, 2) => Ok(FingerprintAvailability::Available(from(
            *d.field(1, Digest256::decode)?.as_array(),
        ))),
        _ => Err(invalid(d)),
    }
}

fn invalid(d: &Decoder<'_>) -> WireError {
    WireError::new(
        WireErrorKind::NonCanonicalCbor,
        d.path().clone(),
        Some(d.position()),
    )
}
