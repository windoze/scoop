use scoop_identity::{ConeIdentity, ValidatedIdentityGraph};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::*;
use crate::{
    DecodedParamFreeShapeSupportRolesV1, LinkDataError, LirTargetProfile, link_data::link_error,
};

impl WireEncode for ParamFreeShapeSupportExportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.roles.encode(encoder)
    }
}

#[derive(Debug)]
pub struct DecodedParamFreeShapeSupportExportV1(DecodedParamFreeShapeSupportRolesV1);

impl DecodedParamFreeShapeSupportExportV1 {
    pub fn read(
        self,
        target: LirTargetProfile,
        provider: ConeIdentity,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<ParamFreeShapeSupportExportV1, LinkDataError> {
        let roles = self.0.read(identities)?;
        let source = *roles
            .source_nominal()
            .available()
            .ok_or_else(|| link_error("shape support has no source nominal"))?;
        let exact = roles
            .type_descriptor()
            .available()
            .ok_or_else(|| link_error("shape support has no type descriptor"))?
            .semantic_id();
        Ok(ParamFreeShapeSupportExportV1 {
            source,
            exact,
            provider,
            target,
            roles,
        })
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
    pub fn read(
        self,
        target: LirTargetProfile,
        provider: ConeIdentity,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<CanonicalParamFreeShapeSupportExportsV1, LinkDataError> {
        let records = self
            .records
            .into_iter()
            .map(|record| record.read(target, provider, identities))
            .collect::<Result<Vec<_>, _>>()?;
        if records
            .windows(2)
            .any(|pair| pair[0].source_nominal() >= pair[1].source_nominal())
        {
            return Err(link_error("noncanonical shape-support source order"));
        }
        Ok(CanonicalParamFreeShapeSupportExportsV1::from_saved(
            target, provider, records,
        ))
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
