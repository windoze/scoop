use scoop_identity::{LocalValueSelector, SignatureTypeKey};
use scoop_wire::{Encoder, WireEncode};

mod decoded;
mod errors;

pub use decoded::{DecodedDefaultPatternFieldV1, DecodedDefaultPatternV1};
pub use errors::{
    DefaultPatternBuildError, DefaultPatternIndexError, DefaultPatternResolutionError,
};

use super::DefaultLiteralEqualityV1;
use crate::{
    DefaultEnumVariantRefV1, DefaultExpressionKindV1, DefaultExpressionReferenceResolver,
    DefaultExpressionV1, IndexedDefaultExpressionV1, TemplateLocalIndexResolver,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultPatternV1(DefaultPatternKindV1);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum DefaultPatternKindV1 {
    Binding {
        local: LocalValueSelector,
    },
    Wildcard,
    Literal {
        value: Box<DefaultExpressionV1>,
        equality: DefaultLiteralEqualityV1,
        subject_type: SignatureTypeKey,
    },
    Variant {
        variant: DefaultEnumVariantRefV1,
        fields: Vec<DefaultPatternFieldV1>,
    },
    Tuple {
        elements: Vec<DefaultPatternV1>,
    },
    Struct {
        owner_type: SignatureTypeKey,
        fields: Vec<DefaultPatternFieldV1>,
    },
}

#[derive(Clone, Copy, Debug)]
pub enum DefaultPatternViewV1<'a> {
    Binding {
        local: &'a LocalValueSelector,
    },
    Wildcard,
    Literal {
        value: &'a DefaultExpressionV1,
        equality: &'a DefaultLiteralEqualityV1,
        subject_type: &'a SignatureTypeKey,
    },
    Variant {
        variant: &'a DefaultEnumVariantRefV1,
        fields: &'a [DefaultPatternFieldV1],
    },
    Tuple {
        elements: &'a [DefaultPatternV1],
    },
    Struct {
        owner_type: &'a SignatureTypeKey,
        fields: &'a [DefaultPatternFieldV1],
    },
}

impl DefaultPatternV1 {
    pub const fn binding(local: LocalValueSelector) -> Self {
        Self(DefaultPatternKindV1::Binding { local })
    }

    pub const fn wildcard() -> Self {
        Self(DefaultPatternKindV1::Wildcard)
    }

    pub fn try_literal(
        value: DefaultExpressionV1,
        equality: DefaultLiteralEqualityV1,
        subject_type: SignatureTypeKey,
    ) -> Result<Self, DefaultPatternBuildError> {
        if !matches!(
            value.kind(),
            DefaultExpressionKindV1::IntegerLiteral(_)
                | DefaultExpressionKindV1::CharLiteral(_)
                | DefaultExpressionKindV1::FloatLiteral(_)
                | DefaultExpressionKindV1::BooleanLiteral(_)
                | DefaultExpressionKindV1::StringLiteral { .. }
        ) {
            return Err(DefaultPatternBuildError::InvalidLiteralExpression);
        }
        Ok(Self(DefaultPatternKindV1::Literal {
            value: Box::new(value),
            equality,
            subject_type,
        }))
    }

    pub fn try_variant(
        variant: DefaultEnumVariantRefV1,
        mut fields: Vec<DefaultPatternFieldV1>,
    ) -> Result<Self, DefaultPatternBuildError> {
        canonicalize_fields(&mut fields)?;
        Ok(Self(DefaultPatternKindV1::Variant { variant, fields }))
    }

    pub fn try_tuple(elements: Vec<Self>) -> Result<Self, DefaultPatternBuildError> {
        require_u32_len(elements.len(), DefaultPatternBuildError::TooManyElements)?;
        Ok(Self(DefaultPatternKindV1::Tuple { elements }))
    }

    pub fn try_struct(
        owner_type: SignatureTypeKey,
        mut fields: Vec<DefaultPatternFieldV1>,
    ) -> Result<Self, DefaultPatternBuildError> {
        canonicalize_fields(&mut fields)?;
        Ok(Self(DefaultPatternKindV1::Struct { owner_type, fields }))
    }

    pub fn view(&self) -> DefaultPatternViewV1<'_> {
        match &self.0 {
            DefaultPatternKindV1::Binding { local } => DefaultPatternViewV1::Binding { local },
            DefaultPatternKindV1::Wildcard => DefaultPatternViewV1::Wildcard,
            DefaultPatternKindV1::Literal {
                value,
                equality,
                subject_type,
            } => DefaultPatternViewV1::Literal {
                value,
                equality,
                subject_type,
            },
            DefaultPatternKindV1::Variant { variant, fields } => {
                DefaultPatternViewV1::Variant { variant, fields }
            }
            DefaultPatternKindV1::Tuple { elements } => DefaultPatternViewV1::Tuple { elements },
            DefaultPatternKindV1::Struct { owner_type, fields } => {
                DefaultPatternViewV1::Struct { owner_type, fields }
            }
        }
    }

    pub fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultPatternV1<'_>, DefaultPatternIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        let kind =
            match &self.0 {
                DefaultPatternKindV1::Binding { local } => {
                    let local_index = resolver
                        .resolve_template_local_index(local)
                        .map_err(DefaultPatternIndexError::Local)?;
                    IndexedDefaultPatternKindV1::Binding { local_index }
                }
                DefaultPatternKindV1::Wildcard => IndexedDefaultPatternKindV1::Wildcard,
                DefaultPatternKindV1::Literal {
                    value,
                    equality,
                    subject_type,
                } => IndexedDefaultPatternKindV1::Literal {
                    value: Box::new(value.index_locals(resolver).map_err(|error| {
                        DefaultPatternIndexError::LiteralValue(Box::new(error))
                    })?),
                    equality,
                    subject_type,
                },
                DefaultPatternKindV1::Variant { variant, fields } => {
                    IndexedDefaultPatternKindV1::Variant {
                        variant,
                        fields: index_fields(fields, resolver)?,
                    }
                }
                DefaultPatternKindV1::Tuple { elements } => {
                    let mut indexed = Vec::with_capacity(elements.len());
                    for (index, element) in elements.iter().enumerate() {
                        indexed.push(element.index_locals(resolver).map_err(|error| {
                            DefaultPatternIndexError::Element {
                                index,
                                error: Box::new(error),
                            }
                        })?);
                    }
                    IndexedDefaultPatternKindV1::Tuple { elements: indexed }
                }
                DefaultPatternKindV1::Struct { owner_type, fields } => {
                    IndexedDefaultPatternKindV1::Struct {
                        owner_type,
                        fields: index_fields(fields, resolver)?,
                    }
                }
            };
        Ok(IndexedDefaultPatternV1 { kind })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultPatternFieldV1 {
    declaration_index: u32,
    pattern: DefaultPatternV1,
}

impl DefaultPatternFieldV1 {
    pub const fn new(declaration_index: u32, pattern: DefaultPatternV1) -> Self {
        Self {
            declaration_index,
            pattern,
        }
    }

    pub const fn declaration_index(&self) -> u32 {
        self.declaration_index
    }

    pub const fn pattern(&self) -> &DefaultPatternV1 {
        &self.pattern
    }
}

#[derive(Debug)]
pub struct IndexedDefaultPatternV1<'a> {
    kind: IndexedDefaultPatternKindV1<'a>,
}

#[derive(Debug)]
enum IndexedDefaultPatternKindV1<'a> {
    Binding {
        local_index: u32,
    },
    Wildcard,
    Literal {
        value: Box<IndexedDefaultExpressionV1<'a>>,
        equality: &'a DefaultLiteralEqualityV1,
        subject_type: &'a SignatureTypeKey,
    },
    Variant {
        variant: &'a DefaultEnumVariantRefV1,
        fields: Vec<IndexedDefaultPatternFieldV1<'a>>,
    },
    Tuple {
        elements: Vec<IndexedDefaultPatternV1<'a>>,
    },
    Struct {
        owner_type: &'a SignatureTypeKey,
        fields: Vec<IndexedDefaultPatternFieldV1<'a>>,
    },
}

#[derive(Debug)]
struct IndexedDefaultPatternFieldV1<'a> {
    declaration_index: u32,
    pattern: IndexedDefaultPatternV1<'a>,
}

impl WireEncode for IndexedDefaultPatternV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match &self.kind {
            IndexedDefaultPatternKindV1::Binding { local_index } => {
                encode_single_payload(encoder, 1, &u32_wire(*local_index))
            }
            IndexedDefaultPatternKindV1::Wildcard => encode_empty_sum(encoder, 2),
            IndexedDefaultPatternKindV1::Literal {
                value,
                equality,
                subject_type,
            } => encode_literal(encoder, value.as_ref(), *equality, *subject_type),
            IndexedDefaultPatternKindV1::Variant { variant, fields } => {
                encode_composite(encoder, 4, *variant, fields)
            }
            IndexedDefaultPatternKindV1::Tuple { elements } => {
                encoder.map(2)?;
                encode_tag(encoder, 5)?;
                encoder.field(1)?;
                encode_sequence(encoder, elements)
            }
            IndexedDefaultPatternKindV1::Struct { owner_type, fields } => {
                encode_composite(encoder, 6, *owner_type, fields)
            }
        }
    }
}

impl WireEncode for IndexedDefaultPatternFieldV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.unsigned(u64::from(self.declaration_index))?;
        encoder.field(2)?;
        self.pattern.encode(encoder)
    }
}

pub trait DefaultPatternReferenceResolver<E>: DefaultExpressionReferenceResolver<E> {}

impl<R, E> DefaultPatternReferenceResolver<E> for R where R: DefaultExpressionReferenceResolver<E> {}

fn canonicalize_fields(
    fields: &mut [DefaultPatternFieldV1],
) -> Result<(), DefaultPatternBuildError> {
    require_u32_len(fields.len(), DefaultPatternBuildError::TooManyFields)?;
    fields.sort_unstable_by_key(DefaultPatternFieldV1::declaration_index);
    for pair in fields.windows(2) {
        if pair[0].declaration_index == pair[1].declaration_index {
            return Err(DefaultPatternBuildError::DuplicateField {
                declaration_index: pair[0].declaration_index,
            });
        }
    }
    Ok(())
}

fn index_fields<'a, I>(
    fields: &'a [DefaultPatternFieldV1],
    resolver: &mut I,
) -> Result<Vec<IndexedDefaultPatternFieldV1<'a>>, DefaultPatternIndexError<I::Error>>
where
    I: TemplateLocalIndexResolver,
{
    let mut indexed = Vec::with_capacity(fields.len());
    for (index, field) in fields.iter().enumerate() {
        indexed.push(IndexedDefaultPatternFieldV1 {
            declaration_index: field.declaration_index,
            pattern: field.pattern.index_locals(resolver).map_err(|error| {
                DefaultPatternIndexError::Field {
                    index,
                    error: Box::new(error),
                }
            })?,
        });
    }
    Ok(indexed)
}

fn require_u32_len<E>(length: usize, error: E) -> Result<u32, E> {
    u32::try_from(length).map_err(|_| error)
}

fn encode_literal(
    encoder: &mut Encoder,
    value: &impl WireEncode,
    equality: &impl WireEncode,
    subject_type: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(4)?;
    encode_tag(encoder, 3)?;
    encoder.field(1)?;
    value.encode(encoder)?;
    encoder.field(2)?;
    equality.encode(encoder)?;
    encoder.field(3)?;
    subject_type.encode(encoder)
}

fn encode_composite(
    encoder: &mut Encoder,
    tag: u64,
    owner: &impl WireEncode,
    fields: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    owner.encode(encoder)?;
    encoder.field(2)?;
    encode_sequence(encoder, fields)
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

struct U32Wire(u32);

impl WireEncode for U32Wire {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(u64::from(self.0))
    }
}

const fn u32_wire(value: u32) -> U32Wire {
    U32Wire(value)
}

fn encode_single_payload(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

#[cfg(test)]
mod tests;
