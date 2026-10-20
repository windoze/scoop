//! The production data needed by downstream compilation and linking.

use crate::link_object::DecodedCanonicalOdrMemberDirectoryV1;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

#[derive(Debug)]
pub struct DecodedSingleConeProductionManifestV1 {
    odr_members: DecodedCanonicalOdrMemberDirectoryV1,
}

impl DecodedSingleConeProductionManifestV1 {
    pub(crate) fn read_odr_members(
        &self,
        identities: &mut scoop_identity::ValidatedIdentityGraph,
    ) -> Result<crate::CanonicalOdrMemberDirectoryV1, scoop_identity::IdentityReferenceError> {
        self.odr_members.read(identities)
    }
}

impl WireEncode for DecodedSingleConeProductionManifestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(11)?;
        self.odr_members.encode(encoder)
    }
}

impl WireDecode for DecodedSingleConeProductionManifestV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(1)?;
        Ok(Self {
            odr_members: decoder.field(11, DecodedCanonicalOdrMemberDirectoryV1::decode)?,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests;
