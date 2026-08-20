use scoop_identity::SourceDeclarationKey;
use scoop_wire::{
    Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath, encode_canonical_temporary,
};

use super::*;
use crate::{
    CanonicalExactDescriptorExportsV1, CanonicalExactLayoutExportsV1, ConeLirFoundation,
    DecodedParamFreeShapeSupportRolesV1,
};

impl WireEncode for ParamFreeShapeSupportExportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.roles.encode(encoder)
    }
}

#[derive(Debug)]
pub struct DecodedParamFreeShapeSupportExportV1(DecodedParamFreeShapeSupportRolesV1);

impl DecodedParamFreeShapeSupportExportV1 {
    pub fn validate_against(
        self,
        expected: &ParamFreeShapeSupportExportV1,
    ) -> Result<ParamFreeShapeSupportExportV1, ParamFreeShapeSupportWireError> {
        let path = WirePath::root();
        let actual = encode_canonical_temporary(&self, &path)?;
        let expected_bytes = encode_canonical_temporary(expected, &path)?;

        if actual != expected_bytes {
            return Err(ParamFreeShapeSupportWireError::RecordMismatch);
        }
        Ok(expected.clone())
    }
}

impl WireEncode for DecodedParamFreeShapeSupportExportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}

impl WireDecode for DecodedParamFreeShapeSupportExportV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        DecodedParamFreeShapeSupportRolesV1::decode(decoder).map(Self)
    }
}

impl WireEncode for CanonicalParamFreeShapeSupportExportsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_records(encoder, self.records())
    }
}

#[derive(Debug)]
pub struct DecodedCanonicalParamFreeShapeSupportExportsV1 {
    records: Vec<DecodedParamFreeShapeSupportExportV1>,
}

impl DecodedCanonicalParamFreeShapeSupportExportsV1 {
    pub fn validate(
        self,
        sources: &[SourceDeclarationKey],
        layouts: &CanonicalExactLayoutExportsV1,
        descriptors: &CanonicalExactDescriptorExportsV1,
        foundation: &ConeLirFoundation,
    ) -> Result<CanonicalParamFreeShapeSupportExportsV1, ParamFreeShapeSupportTableError> {
        let expected = CanonicalParamFreeShapeSupportExportsV1::from_sources(
            sources,
            layouts,
            descriptors,
            foundation,
        )?;
        self.validate_against(&expected)
    }

    pub fn validate_against(
        self,
        expected: &CanonicalParamFreeShapeSupportExportsV1,
    ) -> Result<CanonicalParamFreeShapeSupportExportsV1, ParamFreeShapeSupportTableError> {
        let path = WirePath::root();

        let actual = encode_canonical_temporary(&self, &path)?;
        let expected_bytes = encode_canonical_temporary(expected, &path)?;

        if actual != expected_bytes {
            return Err(ParamFreeShapeSupportTableError::Coverage);
        }
        Ok(expected.clone())
    }
}

impl WireEncode for DecodedCanonicalParamFreeShapeSupportExportsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_records(encoder, &self.records)
    }
}

impl WireDecode for DecodedCanonicalParamFreeShapeSupportExportsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedParamFreeShapeSupportExportV1::decode(decoder))
            .map(|records| Self { records })
    }
}

fn encode_records<T: WireEncode>(
    encoder: &mut Encoder,
    records: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(records.len() as u64)?;
    for record in records {
        record.encode(encoder)?;
    }
    Ok(())
}

#[derive(Debug)]
pub enum ParamFreeShapeSupportWireError {
    RecordMismatch,
    Encode(scoop_wire::cbor::EncodeError),
    Resource(WireError),
}

impl From<WireError> for ParamFreeShapeSupportWireError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for ParamFreeShapeSupportWireError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid ordinary shape-support wire: {self:?}")
    }
}

impl std::error::Error for ParamFreeShapeSupportWireError {}
