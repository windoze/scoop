use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::*;

impl WireEncode for DecodedDefaultBindingActionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Project {
                source,
                result,
                projection,
                definition_origin,
            } => encode_project(encoder, source, result, projection, definition_origin),
            Self::Component {
                source,
                index,
                result,
                setup,
                call,
                definition_origin,
            } => encode_component(
                encoder,
                source,
                *index,
                result,
                setup,
                call.as_ref(),
                definition_origin,
            ),
            Self::Bind {
                source,
                target,
                definition_origin,
            } => encode_bind(encoder, source, target, definition_origin),
        }
    }
}

impl WireDecode for DecodedDefaultBindingActionV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 5)?;
                Ok(Self::Project {
                    source: decoder.field(1, DecodedDefaultBindingTemporaryV1::decode)?,
                    result: decoder.field(2, DecodedDefaultBindingTemporaryV1::decode)?,
                    projection: decoder.field(3, DecodedDefaultBindingProjectionV1::decode)?,
                    definition_origin: decoder.field(4, DecodedExportDefinitionSourceV1::decode)?,
                })
            }
            2 => {
                expect_sum_length(decoder, fields, 7)?;
                let source = decoder.field(1, DecodedDefaultBindingTemporaryV1::decode)?;
                let index = decoder.field(2, Decoder::u32)?;
                Ok(Self::Component {
                    source,
                    index: NonZeroU32::new(index).ok_or_else(|| integer_out_of_range(decoder))?,
                    result: decoder.field(3, DecodedDefaultBindingTemporaryV1::decode)?,
                    setup: decoder.field(4, decode_statements)?,
                    call: decoder
                        .field(5, DecodedDefaultExpressionV1::decode)
                        .map(Box::new)?,
                    definition_origin: decoder.field(6, DecodedExportDefinitionSourceV1::decode)?,
                })
            }
            3 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::Bind {
                    source: decoder.field(1, DecodedDefaultBindingTemporaryV1::decode)?,
                    target: decoder.field(2, DecodedDefaultBindingLeafV1::decode)?,
                    definition_origin: decoder.field(3, DecodedExportDefinitionSourceV1::decode)?,
                })
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

impl WireEncode for DecodedDefaultBindingPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.subject.encode(encoder)?;
        encoder.field(2)?;
        self.shape.encode(encoder)?;
        encoder.field(3)?;
        encode_sequence(encoder, &self.actions)
    }
}

impl WireDecode for DecodedDefaultBindingPlanV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            subject: decoder.field(1, DecodedDefaultBindingTemporaryV1::decode)?,
            shape: decoder.field(2, DecodedDefaultBindingShapeV1::decode)?,
            actions: decoder.field(3, |decoder| {
                decoder.decode_array(|decoder, _| DecodedDefaultBindingActionV1::decode(decoder))
            })?,
        })
    }
}

impl WireEncode for DecodedDefaultIteratorConformanceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.source.encode(encoder)?;
        encoder.field(2)?;
        self.iterator.encode(encoder)?;
        encoder.field(3)?;
        self.interface_type.encode(encoder)?;
        encoder.field(4)?;
        self.definition_origin.encode(encoder)
    }
}

impl WireDecode for DecodedDefaultIteratorConformanceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            source: decoder.field(1, DecodedDefaultBindingTemporaryV1::decode)?,
            iterator: decoder.field(2, DecodedDefaultBindingTemporaryV1::decode)?,
            interface_type: decoder.field(3, DecodedSignatureTypeKey::decode)?,
            definition_origin: decoder.field(4, DecodedExportDefinitionSourceV1::decode)?,
        })
    }
}

impl WireEncode for DecodedDefaultAppliedOptionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.some_payload.encode(encoder)?;
        encoder.field(2)?;
        self.none.encode(encoder)
    }
}

impl WireDecode for DecodedDefaultAppliedOptionV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            some_payload: decoder.field(1, DecodedDefaultEnumVariantFieldRefV1::decode)?,
            none: decoder.field(2, DecodedDefaultEnumVariantRefV1::decode)?,
        })
    }
}

impl WireEncode for DecodedDefaultIteratorNextV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.callable.encode(encoder)?;
        encoder.field(2)?;
        self.result.encode(encoder)?;
        encoder.field(3)?;
        self.option.encode(encoder)?;
        encoder.field(4)?;
        self.element.encode(encoder)?;
        encoder.field(5)?;
        self.definition_origin.encode(encoder)
    }
}

impl WireDecode for DecodedDefaultIteratorNextV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        Ok(Self {
            callable: decoder.field(1, DecodedDefaultCallableRefV1::decode)?,
            result: decoder.field(2, DecodedDefaultBindingTemporaryV1::decode)?,
            option: decoder.field(3, DecodedDefaultAppliedOptionV1::decode)?,
            element: decoder.field(4, DecodedDefaultBindingTemporaryV1::decode)?,
            definition_origin: decoder.field(5, DecodedExportDefinitionSourceV1::decode)?,
        })
    }
}

impl WireEncode for DecodedDefaultForIterationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(9)?;
        encoder.field(1)?;
        encode_sequence(encoder, &self.source_setup)?;
        encoder.field(2)?;
        self.source.encode(encoder)?;
        encoder.field(3)?;
        self.source_init.encode(encoder)?;
        encoder.field(4)?;
        encode_sequence(encoder, &self.iterator_setup)?;
        encoder.field(5)?;
        self.iterator_call.encode(encoder)?;
        encoder.field(6)?;
        self.conformance.encode(encoder)?;
        encoder.field(7)?;
        self.next.encode(encoder)?;
        encoder.field(8)?;
        self.binding.encode(encoder)?;
        encoder.field(9)?;
        encode_sequence(encoder, &self.body)
    }
}

impl WireDecode for DecodedDefaultForIterationPlanV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(9)?;
        Ok(Self {
            source_setup: decoder.field(1, decode_statements)?,
            source: decoder.field(2, DecodedDefaultBindingTemporaryV1::decode)?,
            source_init: decoder
                .field(3, DecodedDefaultExpressionV1::decode)
                .map(Box::new)?,
            iterator_setup: decoder.field(4, decode_statements)?,
            iterator_call: decoder
                .field(5, DecodedDefaultExpressionV1::decode)
                .map(Box::new)?,
            conformance: decoder.field(6, DecodedDefaultIteratorConformanceV1::decode)?,
            next: decoder.field(7, DecodedDefaultIteratorNextV1::decode)?,
            binding: decoder.field(8, DecodedDefaultBindingPlanV1::decode)?,
            body: decoder.field(9, decode_statements)?,
        })
    }
}

fn decode_statements(
    decoder: &mut Decoder<'_>,
) -> Result<Vec<DecodedDefaultStatementV1>, WireError> {
    decoder.decode_array(|decoder, _| DecodedDefaultStatementV1::decode(decoder))
}

fn encode_project(
    encoder: &mut Encoder,
    source: &impl WireEncode,
    result: &impl WireEncode,
    projection: &impl WireEncode,
    definition_origin: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(5)?;
    encode_tag(encoder, 1)?;
    encoder.field(1)?;
    source.encode(encoder)?;
    encoder.field(2)?;
    result.encode(encoder)?;
    encoder.field(3)?;
    projection.encode(encoder)?;
    encoder.field(4)?;
    definition_origin.encode(encoder)
}

fn encode_component(
    encoder: &mut Encoder,
    source: &impl WireEncode,
    index: NonZeroU32,
    result: &impl WireEncode,
    setup: &[impl WireEncode],
    call: &impl WireEncode,
    definition_origin: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(7)?;
    encode_tag(encoder, 2)?;
    encoder.field(1)?;
    source.encode(encoder)?;
    encoder.field(2)?;
    encoder.unsigned(u64::from(index.get()))?;
    encoder.field(3)?;
    result.encode(encoder)?;
    encoder.field(4)?;
    encode_sequence(encoder, setup)?;
    encoder.field(5)?;
    call.encode(encoder)?;
    encoder.field(6)?;
    definition_origin.encode(encoder)
}

fn encode_bind(
    encoder: &mut Encoder,
    source: &impl WireEncode,
    target: &impl WireEncode,
    definition_origin: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(4)?;
    encode_tag(encoder, 3)?;
    encoder.field(1)?;
    source.encode(encoder)?;
    encoder.field(2)?;
    target.encode(encoder)?;
    encoder.field(3)?;
    definition_origin.encode(encoder)
}

fn encode_sequence(
    encoder: &mut Encoder,
    values: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn expect_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn integer_out_of_range(decoder: &Decoder<'_>) -> WireError {
    wire_error(decoder, WireErrorKind::IntegerOutOfRange)
}

fn wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
