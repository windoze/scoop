use scoop_identity::SourceDeclarationKey;
use scoop_wire::{
    BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath, encode,
};

use super::*;
use crate::{
    CanonicalExactDescriptorExportsV1, CanonicalExactLayoutExportsV1,
    DecodedParamFreeShapeSupportRolesV1, OdrFreeLirFoundation,
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
        meter: &mut BudgetMeter,
    ) -> Result<ParamFreeShapeSupportExportV1, ParamFreeShapeSupportWireError> {
        meter.charge_work(1, &WirePath::root())?;
        let actual = encode(&self).map_err(ParamFreeShapeSupportWireError::Encode)?;
        let expected_bytes = encode(expected).map_err(ParamFreeShapeSupportWireError::Encode)?;
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
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
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
        foundation: &OdrFreeLirFoundation,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalParamFreeShapeSupportExportsV1, ParamFreeShapeSupportTableError> {
        let path = WirePath::root();
        meter.check_table_entries(self.records.len() as u64, &path)?;
        meter.charge_work(self.records.len() as u64, &path)?;
        let actual = encode(&self)?;
        let expected = CanonicalParamFreeShapeSupportExportsV1::from_sources(
            sources,
            layouts,
            descriptors,
            foundation,
            meter,
        )?;
        if actual != encode(&expected)? {
            return Err(ParamFreeShapeSupportTableError::Coverage);
        }
        Ok(expected)
    }
}

impl WireEncode for DecodedCanonicalParamFreeShapeSupportExportsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_records(encoder, &self.records)
    }
}

impl WireDecode for DecodedCanonicalParamFreeShapeSupportExportsV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
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
