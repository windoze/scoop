use std::fmt;

use scoop_wire::{
    BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, WirePath,
};

use super::{
    CanonicalProtectedDefaultExpressionUsesV1, ProtectedDefaultExpressionUseV1,
    ProtectedDefaultExpressionUsesBuildError, ProtectedDefaultReceiverUseV1, wire,
};

impl WireDecode for ProtectedDefaultReceiverUseV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        if tag == 3 {
            wire::expect_fields(decoder, fields, 2)?;
            return decoder
                .field(1, Decoder::u32)
                .map(|receiver_expression_index| Self::Explicit {
                    receiver_expression_index,
                });
        }
        let receiver = match tag {
            1 => Self::None,
            2 => Self::ImplicitThis,
            4 => Self::ConstructorDelegation,
            tag => return Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        };
        wire::expect_fields(decoder, fields, 1)?;
        Ok(receiver)
    }
}

impl WireDecode for ProtectedDefaultExpressionUseV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self::new(
            decoder.field(1, Decoder::u32)?,
            decoder.field(2, ProtectedDefaultReceiverUseV1::decode)?,
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalProtectedDefaultExpressionUsesV1 {
    values: Vec<ProtectedDefaultExpressionUseV1>,
}

impl DecodedCanonicalProtectedDefaultExpressionUsesV1 {
    pub fn resolve(
        self,
        meter: &mut BudgetMeter,
    ) -> Result<
        CanonicalProtectedDefaultExpressionUsesV1,
        ProtectedDefaultExpressionUsesResolutionError,
    > {
        let path = WirePath::root();
        meter
            .charge_nodes(1, &path)
            .map_err(ProtectedDefaultExpressionUsesResolutionError::Resource)?;
        meter
            .charge_work(self.values.len() as u64, &path)
            .map_err(ProtectedDefaultExpressionUsesResolutionError::Resource)?;
        CanonicalProtectedDefaultExpressionUsesV1::from_ordered(self.values)
            .map_err(ProtectedDefaultExpressionUsesResolutionError::Build)
    }
}

impl WireDecode for DecodedCanonicalProtectedDefaultExpressionUsesV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| ProtectedDefaultExpressionUseV1::decode(decoder))
            .map(|values| Self { values })
    }
}

impl WireEncode for DecodedCanonicalProtectedDefaultExpressionUsesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.values)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ProtectedDefaultExpressionUsesResolutionError {
    Resource(WireError),
    Build(ProtectedDefaultExpressionUsesBuildError),
}

impl fmt::Display for ProtectedDefaultExpressionUsesResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Build(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ProtectedDefaultExpressionUsesResolutionError {}
