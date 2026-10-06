use super::*;
use scoop_wire::WireErrorKind;

impl WireEncode for NominalElementEncodingV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_fields(encoder, &self.element, &self.interface, &self.selections)
    }
}

impl WireEncode for DecodedNominalElementEncodingV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_fields(encoder, &self.element, &self.interface, &self.selections)
    }
}

fn encode_fields(
    encoder: &mut Encoder,
    element: &impl WireEncode,
    interface: &impl WireEncode,
    selections: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encoder.field(1)?;
    element.encode(encoder)?;
    encoder.field(2)?;
    interface.encode(encoder)?;
    encoder.field(3)?;
    selections.encode(encoder)
}

impl WireDecode for DecodedNominalElementEncodingV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            element: decoder.field(1, DecodedSignatureTypeKey::decode)?,
            interface: decoder.field(2, DecodedSignatureTypeKey::decode)?,
            selections: decoder.field(3, DecodedCanonicalNominalDispatchSelectionsV1::decode)?,
        })
    }
}

pub(in super::super) fn optional(
    decoder: &mut Decoder<'_>,
) -> Result<Option<DecodedNominalElementEncodingV1>, WireError> {
    match decoder.array()? {
        0 => Ok(None),
        1 => DecodedNominalElementEncodingV1::decode(decoder).map(Some),
        actual => Err(WireError::new(
            WireErrorKind::InvalidLength {
                expected: 1,
                actual,
            },
            decoder.path().clone(),
            Some(decoder.position()),
        )),
    }
}
