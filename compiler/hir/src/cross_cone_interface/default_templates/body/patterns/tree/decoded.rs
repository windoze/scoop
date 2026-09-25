use scoop_identity::DecodedSignatureTypeKey;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::super::DecodedDefaultLiteralEqualityV1;
use super::{
    DefaultPatternBuildError, DefaultPatternFieldV1, DefaultPatternKindV1,
    DefaultPatternReferenceResolver, DefaultPatternResolutionError, DefaultPatternV1,
    encode_composite, encode_empty_sum, encode_literal, encode_sequence, encode_single_payload,
    require_u32_len, u32_wire,
};
use crate::{CanonicalConstValueV1, DecodedDefaultEnumVariantRefV1, TemplateLocalSelectorResolver};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultPatternV1(DecodedDefaultPatternKindV1);

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodedDefaultPatternKindV1 {
    Binding {
        local_index: u32,
    },
    Wildcard,
    Literal {
        value: CanonicalConstValueV1,
        equality: DecodedDefaultLiteralEqualityV1,
        subject_type: DecodedSignatureTypeKey,
    },
    Variant {
        variant: DecodedDefaultEnumVariantRefV1,
        fields: Vec<DecodedDefaultPatternFieldV1>,
    },
    Tuple {
        elements: Vec<DecodedDefaultPatternV1>,
    },
    Struct {
        owner_type: DecodedSignatureTypeKey,
        fields: Vec<DecodedDefaultPatternFieldV1>,
    },
}

impl DecodedDefaultPatternV1 {
    pub fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultPatternV1, DefaultPatternResolutionError<E, L::Error>>
    where
        R: DefaultPatternReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        match self.0 {
            DecodedDefaultPatternKindV1::Binding { local_index } => locals
                .resolve_template_local_selector(local_index)
                .map(DefaultPatternV1::binding)
                .map_err(DefaultPatternResolutionError::Local),
            DecodedDefaultPatternKindV1::Wildcard => Ok(DefaultPatternV1::wildcard()),
            DecodedDefaultPatternKindV1::Literal {
                value,
                equality,
                subject_type,
            } => Ok(DefaultPatternV1::literal(
                value,
                equality
                    .resolve(resolver)
                    .map_err(DefaultPatternResolutionError::LiteralEquality)?,
                subject_type
                    .resolve(resolver)
                    .map_err(DefaultPatternResolutionError::SubjectType)?,
            )),
            DecodedDefaultPatternKindV1::Variant { variant, fields } => {
                let variant = variant
                    .resolve(resolver)
                    .map_err(DefaultPatternResolutionError::Variant)?;
                let fields = resolve_fields(fields, resolver, locals)?;
                Ok(DefaultPatternV1(DefaultPatternKindV1::Variant {
                    variant,
                    fields,
                }))
            }
            DecodedDefaultPatternKindV1::Tuple { elements } => {
                require_u32_len(
                    elements.len(),
                    DefaultPatternResolutionError::Shape(DefaultPatternBuildError::TooManyElements),
                )?;
                let mut resolved = Vec::with_capacity(elements.len());
                for (index, element) in elements.into_iter().enumerate() {
                    resolved.push(element.resolve(resolver, locals).map_err(|error| {
                        DefaultPatternResolutionError::Element {
                            index,
                            error: Box::new(error),
                        }
                    })?);
                }
                Ok(DefaultPatternV1(DefaultPatternKindV1::Tuple {
                    elements: resolved,
                }))
            }
            DecodedDefaultPatternKindV1::Struct { owner_type, fields } => {
                let owner_type = owner_type
                    .resolve(resolver)
                    .map_err(DefaultPatternResolutionError::StructOwnerType)?;
                let fields = resolve_fields(fields, resolver, locals)?;
                Ok(DefaultPatternV1(DefaultPatternKindV1::Struct {
                    owner_type,
                    fields,
                }))
            }
        }
    }
}

impl WireEncode for DecodedDefaultPatternV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match &self.0 {
            DecodedDefaultPatternKindV1::Binding { local_index } => {
                encode_single_payload(encoder, 1, &u32_wire(*local_index))
            }
            DecodedDefaultPatternKindV1::Wildcard => encode_empty_sum(encoder, 2),
            DecodedDefaultPatternKindV1::Literal {
                value,
                equality,
                subject_type,
            } => encode_literal(encoder, value, equality, subject_type),
            DecodedDefaultPatternKindV1::Variant { variant, fields } => {
                encode_composite(encoder, 4, variant, fields)
            }
            DecodedDefaultPatternKindV1::Tuple { elements } => {
                encoder.map(2)?;
                encode_tag(encoder, 5)?;
                encoder.field(1)?;
                encode_sequence(encoder, elements)
            }
            DecodedDefaultPatternKindV1::Struct { owner_type, fields } => {
                encode_composite(encoder, 6, owner_type, fields)
            }
        }
    }
}

impl WireDecode for DecodedDefaultPatternV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, Decoder::u32)
                    .map(|local_index| Self(DecodedDefaultPatternKindV1::Binding { local_index }))
            }
            2 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self(DecodedDefaultPatternKindV1::Wildcard))
            }
            3 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self(DecodedDefaultPatternKindV1::Literal {
                    value: decoder.field(1, CanonicalConstValueV1::decode)?,
                    equality: decoder.field(2, DecodedDefaultLiteralEqualityV1::decode)?,
                    subject_type: decoder.field(3, DecodedSignatureTypeKey::decode)?,
                }))
            }
            4 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self(DecodedDefaultPatternKindV1::Variant {
                    variant: decoder.field(1, DecodedDefaultEnumVariantRefV1::decode)?,
                    fields: decoder.field(2, decode_fields)?,
                }))
            }
            5 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, |decoder| {
                        decoder.decode_array(|decoder, _| Self::decode(decoder))
                    })
                    .map(|elements| Self(DecodedDefaultPatternKindV1::Tuple { elements }))
            }
            6 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self(DecodedDefaultPatternKindV1::Struct {
                    owner_type: decoder.field(1, DecodedSignatureTypeKey::decode)?,
                    fields: decoder.field(2, decode_fields)?,
                }))
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultPatternFieldV1 {
    declaration_index: u32,
    pattern: DecodedDefaultPatternV1,
}

impl WireEncode for DecodedDefaultPatternFieldV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.unsigned(u64::from(self.declaration_index))?;
        encoder.field(2)?;
        self.pattern.encode(encoder)
    }
}

impl WireDecode for DecodedDefaultPatternFieldV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            declaration_index: decoder.field(1, Decoder::u32)?,
            pattern: decoder.field(2, DecodedDefaultPatternV1::decode)?,
        })
    }
}

fn validate_fields(
    fields: &[DecodedDefaultPatternFieldV1],
) -> Result<(), DefaultPatternBuildError> {
    require_u32_len(fields.len(), DefaultPatternBuildError::TooManyFields)?;
    for (offset, pair) in fields.windows(2).enumerate() {
        if pair[0].declaration_index == pair[1].declaration_index {
            return Err(DefaultPatternBuildError::DuplicateField {
                declaration_index: pair[0].declaration_index,
            });
        }
        if pair[0].declaration_index > pair[1].declaration_index {
            return Err(DefaultPatternBuildError::NonCanonicalFieldOrder {
                index: offset + 1,
                previous: pair[0].declaration_index,
                actual: pair[1].declaration_index,
            });
        }
    }
    Ok(())
}

fn resolve_fields<R, L, E>(
    fields: Vec<DecodedDefaultPatternFieldV1>,
    resolver: &mut R,
    locals: &mut L,
) -> Result<Vec<DefaultPatternFieldV1>, DefaultPatternResolutionError<E, L::Error>>
where
    R: DefaultPatternReferenceResolver<E>,
    L: TemplateLocalSelectorResolver,
{
    validate_fields(&fields).map_err(DefaultPatternResolutionError::Shape)?;
    let mut resolved = Vec::with_capacity(fields.len());
    for (index, field) in fields.into_iter().enumerate() {
        resolved.push(DefaultPatternFieldV1 {
            declaration_index: field.declaration_index,
            pattern: field.pattern.resolve(resolver, locals).map_err(|error| {
                DefaultPatternResolutionError::Field {
                    index,
                    error: Box::new(error),
                }
            })?,
        });
    }
    Ok(resolved)
}

fn decode_fields(
    decoder: &mut Decoder<'_>,
) -> Result<Vec<DecodedDefaultPatternFieldV1>, WireError> {
    decoder.decode_array(|decoder, _| DecodedDefaultPatternFieldV1::decode(decoder))
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

fn wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
