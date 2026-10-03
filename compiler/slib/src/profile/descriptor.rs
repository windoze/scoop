use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactCapabilityProfileDescriptor {
    pub(super) id: ArtifactCapabilityProfileId,
    pub(super) required_manifest: Vec<CapabilityId>,
    pub(super) required_hir: Vec<CapabilityId>,
    pub(super) required_mir: Vec<CapabilityId>,
    pub(super) required_lir: Vec<CapabilityId>,
}

impl ArtifactCapabilityProfileDescriptor {
    pub const fn id(&self) -> &ArtifactCapabilityProfileId {
        &self.id
    }

    pub fn required_manifest(&self) -> &[CapabilityId] {
        &self.required_manifest
    }

    pub fn required_hir(&self) -> &[CapabilityId] {
        &self.required_hir
    }

    pub fn required_mir(&self) -> &[CapabilityId] {
        &self.required_mir
    }

    pub fn required_lir(&self) -> &[CapabilityId] {
        &self.required_lir
    }
}

impl WireEncode for ArtifactCapabilityProfileDescriptor {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.id.encode(encoder)?;
        encode_capabilities(encoder, 2, &self.required_manifest)?;
        encode_capabilities(encoder, 3, &self.required_hir)?;
        encode_capabilities(encoder, 4, &self.required_mir)?;
        encode_capabilities(encoder, 5, &self.required_lir)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactCapabilityProfileFingerprint([u8; 32]);

impl ArtifactCapabilityProfileFingerprint {
    pub fn from_descriptor(
        descriptor: &ArtifactCapabilityProfileDescriptor,
    ) -> Result<Self, HashError> {
        domain_separated_cbor_hash(ARTIFACT_PROFILE_DOMAIN, descriptor)
            .map(|digest| Self(*digest.as_array()))
    }

    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}

impl WireEncode for ArtifactCapabilityProfileFingerprint {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl fmt::Display for ArtifactCapabilityProfileFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

fn encode_capabilities(
    encoder: &mut Encoder,
    field: u32,
    capabilities: &[CapabilityId],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    encoder.array(capabilities.len() as u64)?;
    for capability in capabilities {
        capability.encode(encoder)?;
    }
    Ok(())
}
