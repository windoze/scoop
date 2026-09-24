use super::dependency::DecodedMirTypeBridgeDependencyV1;
use super::*;

mod types;

pub use types::TypeResolvedCrossConeMirTypeBridgeSectionV1;

/// Untrusted seven-field transport. Resolution alone does not create selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCrossConeMirTypeBridgeSectionV1 {
    types: DecodedCanonicalParamFreeMirTypeExportsV1,
    callables: DecodedCanonicalMirCallableBindingsV1,
    dispatch: DecodedCanonicalMirDispatchSchemasV1,
    object_values: DecodedCanonicalMirObjectValuesV1,
    shape_support: DecodedCanonicalMirShapeSupportsV1,
    initialization_uses: DecodedCanonicalMirExternalInitializationUsesV1,
    selected: Vec<DecodedMirTypeBridgeDependencyV1>,
}
impl WireDecode for DecodedCrossConeMirTypeBridgeSectionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(7)?;
        Ok(Self {
            types: decoder.field(1, DecodedCanonicalParamFreeMirTypeExportsV1::decode)?,
            callables: decoder.field(2, DecodedCanonicalMirCallableBindingsV1::decode)?,
            dispatch: decoder.field(3, DecodedCanonicalMirDispatchSchemasV1::decode)?,
            object_values: decoder.field(4, DecodedCanonicalMirObjectValuesV1::decode)?,
            shape_support: decoder.field(5, DecodedCanonicalMirShapeSupportsV1::decode)?,
            initialization_uses: decoder
                .field(6, DecodedCanonicalMirExternalInitializationUsesV1::decode)?,
            selected: decoder.field(7, |decoder| {
                decoder.decode_array(|decoder, _| DecodedMirTypeBridgeDependencyV1::decode(decoder))
            })?,
        })
    }
}
impl WireEncode for DecodedCrossConeMirTypeBridgeSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(7)?;
        encoder.field(1)?;
        self.types.encode(encoder)?;
        encoder.field(2)?;
        self.callables.encode(encoder)?;
        encoder.field(3)?;
        self.dispatch.encode(encoder)?;
        encoder.field(4)?;
        self.object_values.encode(encoder)?;
        encoder.field(5)?;
        self.shape_support.encode(encoder)?;
        encoder.field(6)?;
        self.initialization_uses.encode(encoder)?;
        encoder.field(7)?;
        sequence(encoder, &self.selected)
    }
}
impl WireEncode for CrossConeMirTypeBridgeSectionV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(7)?;
        encoder.field(1)?;
        self.types().encode(encoder)?;
        encoder.field(2)?;
        self.callables().encode(encoder)?;
        encoder.field(3)?;
        self.dispatch().encode(encoder)?;
        encoder.field(4)?;
        self.object_values().encode(encoder)?;
        encoder.field(5)?;
        self.shape_support().encode(encoder)?;
        encoder.field(6)?;
        self.initialization_uses().encode(encoder)?;
        encoder.field(7)?;
        self.selected().encode(encoder)
    }
}
