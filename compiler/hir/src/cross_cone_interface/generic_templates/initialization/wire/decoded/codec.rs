use super::*;

impl WireDecode for DecodedCanonicalExportGenericInitializationsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedNominalInitialization::decode(decoder))
            .map(|records| Self { records })
    }
}

impl WireEncode for DecodedCanonicalExportGenericInitializationsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_values(encoder, &self.records)
    }
}

impl WireDecode for DecodedNominalInitialization {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            owner: decoder.field(1, DecodedPersistentId::decode)?,
            common: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| DecodedCommonStep::decode(decoder))
            })?,
            constructors: decoder.field(3, |decoder| {
                decoder.decode_array(|decoder, _| DecodedConstructor::decode(decoder))
            })?,
        })
    }
}

impl WireEncode for DecodedNominalInitialization {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        encode_values(encoder, &self.common)?;
        encoder.field(3)?;
        encode_values(encoder, &self.constructors)
    }
}

impl WireDecode for DecodedCommonStep {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                require_length(decoder, fields, 3)?;
                Ok(Self::Field(
                    decoder.field(1, DecodedDefaultFieldRefV1::decode)?,
                    decoder.field(2, DecodedExportTemplateFragmentV1::decode)?,
                ))
            }
            2 => {
                require_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedExportTemplateFragmentV1::decode)
                    .map(Self::Body)
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireEncode for DecodedCommonStep {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Field(field, value) => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                field.encode(encoder)?;
                encoder.field(2)?;
                value.encode(encoder)
            }
            Self::Body(body) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                body.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedDelegation {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            target: decoder.field(1, DecodedDefaultConstructorRefV1::decode)?,
            arguments: decoder.field(2, DecodedExportTemplateFragmentV1::decode)?,
        })
    }
}

impl WireEncode for DecodedDelegation {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.target.encode(encoder)?;
        encoder.field(2)?;
        self.arguments.encode(encoder)
    }
}

impl WireDecode for DecodedPrimaryStore {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            field: decoder.field(1, DecodedDefaultFieldRefV1::decode)?,
            parameter: decoder.field(2, LocalValueSelector::decode)?,
        })
    }
}

impl WireEncode for DecodedPrimaryStore {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.field.encode(encoder)?;
        encoder.field(2)?;
        self.parameter.encode(encoder)
    }
}

impl WireDecode for DecodedConstructor {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            declaration: decoder.field(1, DecodedDefaultConstructorRefV1::decode)?,
            inputs: decoder.field(2, DecodedCanonicalTemplateLocalTableV1::decode)?,
            effects: decoder.field(3, DecodedCallableSourceEffectsV1::decode)?,
            predicates: decoder.field(4, DecodedGenericTemplatePredicatesV1::decode)?,
            definition_origin: decoder.field(5, DecodedExportDefinitionSourceV1::decode)?,
            kind: decoder.field(6, DecodedConstructorKind::decode)?,
        })
    }
}

impl WireEncode for DecodedConstructor {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.inputs.encode(encoder)?;
        encoder.field(3)?;
        self.effects.encode(encoder)?;
        encoder.field(4)?;
        self.predicates.encode(encoder)?;
        encoder.field(5)?;
        self.definition_origin.encode(encoder)?;
        encoder.field(6)?;
        self.kind.encode(encoder)
    }
}

impl WireDecode for DecodedConstructorKind {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                require_length(decoder, fields, 1)?;
                Ok(Self::StructPrimary)
            }
            2 | 4 => {
                require_length(decoder, fields, 3)?;
                let delegation = decoder.field(1, DecodedDelegation::decode)?;
                let body = decoder.field(2, DecodedExportTemplateFragmentV1::decode)?;
                Ok(if tag == 2 {
                    Self::StructSecondary(delegation, body)
                } else {
                    Self::ClassSecondaryThis(delegation, body)
                })
            }
            3 => {
                require_length(decoder, fields, 3)?;
                let base = decoder.field(1, decode_base)?;
                let stores = decoder.field(2, |decoder| {
                    decoder.decode_array(|decoder, _| DecodedPrimaryStore::decode(decoder))
                })?;
                Ok(Self::ClassPrimary(base, stores))
            }
            5 => {
                require_length(decoder, fields, 3)?;
                Ok(Self::ClassSecondaryTerminal(
                    decoder.field(1, decode_base)?,
                    decoder.field(2, DecodedExportTemplateFragmentV1::decode)?,
                ))
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

fn decode_base(decoder: &mut Decoder<'_>) -> Result<Option<DecodedDelegation>, WireError> {
    let length = decoder.array()?;
    match length {
        0 => Ok(None),
        1 => decoder.index(0, DecodedDelegation::decode).map(Some),
        _ => {
            require_length(decoder, length, 1)?;
            unreachable!("a non-unit array length was rejected")
        }
    }
}

impl WireEncode for DecodedConstructorKind {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(if matches!(self, Self::StructPrimary) {
            1
        } else {
            3
        })?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::StructPrimary => 1,
            Self::StructSecondary(..) => 2,
            Self::ClassPrimary(..) => 3,
            Self::ClassSecondaryThis(..) => 4,
            Self::ClassSecondaryTerminal(..) => 5,
        })?;
        match self {
            Self::StructPrimary => Ok(()),
            Self::StructSecondary(delegation, body)
            | Self::ClassSecondaryThis(delegation, body) => {
                encoder.field(1)?;
                delegation.encode(encoder)?;
                encoder.field(2)?;
                body.encode(encoder)
            }
            Self::ClassPrimary(base, stores) => {
                encoder.field(1)?;
                super::super::indexed::encode_base(encoder, base.as_ref())?;
                encoder.field(2)?;
                encode_values(encoder, stores)
            }
            Self::ClassSecondaryTerminal(base, body) => {
                encoder.field(1)?;
                super::super::indexed::encode_base(encoder, base.as_ref())?;
                encoder.field(2)?;
                body.encode(encoder)
            }
        }
    }
}
