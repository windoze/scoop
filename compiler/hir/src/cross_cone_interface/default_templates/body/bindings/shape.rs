use std::{collections::BTreeSet, fmt, num::NonZeroU32};

use scoop_identity::SignatureTypeKey;
use scoop_wire::{Encoder, WireEncode};

use super::{
    DefaultBindingLeafIndexError, DefaultBindingLeafResolutionError, DefaultBindingLeafV1,
    IndexedDefaultBindingLeafV1,
};
use crate::TemplateLocalIndexResolver;

mod decoded;

pub use decoded::{
    DecodedDefaultBindingClassComponentV1, DecodedDefaultBindingShapeV1,
    DecodedDefaultBindingStructFieldV1,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultBindingShapeV1(DefaultBindingShapeKindV1);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum DefaultBindingShapeKindV1 {
    Binding(DefaultBindingLeafV1),
    Wildcard,
    Tuple(Vec<DefaultBindingShapeV1>),
    Struct {
        owner_type: SignatureTypeKey,
        fields: Vec<DefaultBindingStructFieldV1>,
    },
    Class {
        owner_type: SignatureTypeKey,
        components: Vec<DefaultBindingClassComponentV1>,
    },
}

#[derive(Clone, Copy, Debug)]
pub enum DefaultBindingShapeViewV1<'a> {
    Binding(&'a DefaultBindingLeafV1),
    Wildcard,
    Tuple(&'a [DefaultBindingShapeV1]),
    Struct {
        owner_type: &'a SignatureTypeKey,
        fields: &'a [DefaultBindingStructFieldV1],
    },
    Class {
        owner_type: &'a SignatureTypeKey,
        components: &'a [DefaultBindingClassComponentV1],
    },
}

impl DefaultBindingShapeV1 {
    pub const fn binding(leaf: DefaultBindingLeafV1) -> Self {
        Self(DefaultBindingShapeKindV1::Binding(leaf))
    }

    pub const fn wildcard() -> Self {
        Self(DefaultBindingShapeKindV1::Wildcard)
    }

    pub fn try_tuple(elements: Vec<Self>) -> Result<Self, DefaultBindingShapeBuildError> {
        require_u32_len(
            elements.len(),
            DefaultBindingShapeBuildError::TooManyElements,
        )?;
        Ok(Self(DefaultBindingShapeKindV1::Tuple(elements)))
    }

    pub fn try_struct(
        owner_type: SignatureTypeKey,
        mut fields: Vec<DefaultBindingStructFieldV1>,
    ) -> Result<Self, DefaultBindingShapeBuildError> {
        canonicalize_struct_fields(&mut fields)?;
        Ok(Self(DefaultBindingShapeKindV1::Struct {
            owner_type,
            fields,
        }))
    }

    pub fn try_class(
        owner_type: SignatureTypeKey,
        components: Vec<DefaultBindingClassComponentV1>,
    ) -> Result<Self, DefaultBindingShapeBuildError> {
        validate_class_components(&components)?;
        Ok(Self(DefaultBindingShapeKindV1::Class {
            owner_type,
            components,
        }))
    }

    pub fn view(&self) -> DefaultBindingShapeViewV1<'_> {
        match &self.0 {
            DefaultBindingShapeKindV1::Binding(leaf) => DefaultBindingShapeViewV1::Binding(leaf),
            DefaultBindingShapeKindV1::Wildcard => DefaultBindingShapeViewV1::Wildcard,
            DefaultBindingShapeKindV1::Tuple(elements) => {
                DefaultBindingShapeViewV1::Tuple(elements)
            }
            DefaultBindingShapeKindV1::Struct { owner_type, fields } => {
                DefaultBindingShapeViewV1::Struct { owner_type, fields }
            }
            DefaultBindingShapeKindV1::Class {
                owner_type,
                components,
            } => DefaultBindingShapeViewV1::Class {
                owner_type,
                components,
            },
        }
    }

    pub fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultBindingShapeV1<'_>, DefaultBindingShapeIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        let kind = match &self.0 {
            DefaultBindingShapeKindV1::Binding(leaf) => IndexedDefaultBindingShapeKindV1::Binding(
                leaf.index_local(resolver)
                    .map_err(DefaultBindingShapeIndexError::Binding)?,
            ),
            DefaultBindingShapeKindV1::Wildcard => IndexedDefaultBindingShapeKindV1::Wildcard,
            DefaultBindingShapeKindV1::Tuple(elements) => {
                IndexedDefaultBindingShapeKindV1::Tuple(index_shapes(elements, resolver)?)
            }
            DefaultBindingShapeKindV1::Struct { owner_type, fields } => {
                let mut indexed = Vec::with_capacity(fields.len());
                for (index, field) in fields.iter().enumerate() {
                    indexed.push(IndexedDefaultBindingStructFieldV1 {
                        declaration_index: field.declaration_index,
                        shape: field.shape.index_locals(resolver).map_err(|error| {
                            DefaultBindingShapeIndexError::StructField {
                                index,
                                error: Box::new(error),
                            }
                        })?,
                    });
                }
                IndexedDefaultBindingShapeKindV1::Struct {
                    owner_type,
                    fields: indexed,
                }
            }
            DefaultBindingShapeKindV1::Class {
                owner_type,
                components,
            } => {
                let mut indexed = Vec::with_capacity(components.len());
                for (index, component) in components.iter().enumerate() {
                    indexed.push(IndexedDefaultBindingClassComponentV1 {
                        index: component.index,
                        shape: component.shape.index_locals(resolver).map_err(|error| {
                            DefaultBindingShapeIndexError::ClassComponent {
                                index,
                                error: Box::new(error),
                            }
                        })?,
                    });
                }
                IndexedDefaultBindingShapeKindV1::Class {
                    owner_type,
                    components: indexed,
                }
            }
        };
        Ok(IndexedDefaultBindingShapeV1 { kind })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultBindingStructFieldV1 {
    declaration_index: u32,
    shape: DefaultBindingShapeV1,
}

impl DefaultBindingStructFieldV1 {
    pub const fn new(declaration_index: u32, shape: DefaultBindingShapeV1) -> Self {
        Self {
            declaration_index,
            shape,
        }
    }

    pub const fn declaration_index(&self) -> u32 {
        self.declaration_index
    }

    pub const fn shape(&self) -> &DefaultBindingShapeV1 {
        &self.shape
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultBindingClassComponentV1 {
    index: NonZeroU32,
    shape: DefaultBindingShapeV1,
}

impl DefaultBindingClassComponentV1 {
    pub const fn new(index: NonZeroU32, shape: DefaultBindingShapeV1) -> Self {
        Self { index, shape }
    }

    pub const fn index(&self) -> NonZeroU32 {
        self.index
    }

    pub const fn shape(&self) -> &DefaultBindingShapeV1 {
        &self.shape
    }
}

#[derive(Debug)]
pub struct IndexedDefaultBindingShapeV1<'a> {
    kind: IndexedDefaultBindingShapeKindV1<'a>,
}

#[derive(Debug)]
enum IndexedDefaultBindingShapeKindV1<'a> {
    Binding(IndexedDefaultBindingLeafV1<'a>),
    Wildcard,
    Tuple(Vec<IndexedDefaultBindingShapeV1<'a>>),
    Struct {
        owner_type: &'a SignatureTypeKey,
        fields: Vec<IndexedDefaultBindingStructFieldV1<'a>>,
    },
    Class {
        owner_type: &'a SignatureTypeKey,
        components: Vec<IndexedDefaultBindingClassComponentV1<'a>>,
    },
}

#[derive(Debug)]
struct IndexedDefaultBindingStructFieldV1<'a> {
    declaration_index: u32,
    shape: IndexedDefaultBindingShapeV1<'a>,
}

#[derive(Debug)]
struct IndexedDefaultBindingClassComponentV1<'a> {
    index: NonZeroU32,
    shape: IndexedDefaultBindingShapeV1<'a>,
}

impl WireEncode for IndexedDefaultBindingShapeV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match &self.kind {
            IndexedDefaultBindingShapeKindV1::Binding(leaf) => {
                encode_single_payload(encoder, 1, leaf)
            }
            IndexedDefaultBindingShapeKindV1::Wildcard => encode_empty_sum(encoder, 2),
            IndexedDefaultBindingShapeKindV1::Tuple(elements) => {
                encode_single_payload(encoder, 3, &WireSequence(elements))
            }
            IndexedDefaultBindingShapeKindV1::Struct { owner_type, fields } => {
                encode_composite(encoder, 4, *owner_type, fields)
            }
            IndexedDefaultBindingShapeKindV1::Class {
                owner_type,
                components,
            } => encode_composite(encoder, 5, *owner_type, components),
        }
    }
}

impl WireEncode for IndexedDefaultBindingStructFieldV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_indexed_shape(encoder, self.declaration_index, &self.shape)
    }
}

impl WireEncode for IndexedDefaultBindingClassComponentV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_indexed_shape(encoder, self.index.get(), &self.shape)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultBindingShapeBuildError {
    TooManyElements,
    TooManyStructFields,
    DuplicateStructField {
        declaration_index: u32,
    },
    NonCanonicalStructFieldOrder {
        index: usize,
        previous: u32,
        actual: u32,
    },
    TooManyClassComponents,
    DuplicateClassComponent {
        index: NonZeroU32,
    },
}

impl fmt::Display for DefaultBindingShapeBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyElements => formatter.write_str("default binding tuple exceeds u32"),
            Self::TooManyStructFields => {
                formatter.write_str("default binding struct field count exceeds u32")
            }
            Self::DuplicateStructField { declaration_index } => write!(
                formatter,
                "duplicate default binding struct field {declaration_index}"
            ),
            Self::NonCanonicalStructFieldOrder {
                index,
                previous,
                actual,
            } => write!(
                formatter,
                "default binding struct field {index} is out of order: {actual} follows {previous}"
            ),
            Self::TooManyClassComponents => {
                formatter.write_str("default binding class component count exceeds u32")
            }
            Self::DuplicateClassComponent { index } => {
                write!(
                    formatter,
                    "duplicate default binding class component {index}"
                )
            }
        }
    }
}

impl std::error::Error for DefaultBindingShapeBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultBindingShapeResolutionError<E, L> {
    Binding(DefaultBindingLeafResolutionError<E, L>),
    Element { index: usize, error: Box<Self> },
    StructOwnerType(E),
    StructField { index: usize, error: Box<Self> },
    ClassOwnerType(E),
    ClassComponent { index: usize, error: Box<Self> },
    Shape(DefaultBindingShapeBuildError),
}

impl<E: fmt::Display, L: fmt::Display> fmt::Display for DefaultBindingShapeResolutionError<E, L> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Binding(error) => write!(formatter, "invalid default binding leaf: {error}"),
            Self::Element { index, error } => {
                write!(
                    formatter,
                    "invalid default binding tuple element {index}: {error}"
                )
            }
            Self::StructOwnerType(error) => {
                write!(formatter, "invalid default binding struct owner: {error}")
            }
            Self::StructField { index, error } => {
                write!(
                    formatter,
                    "invalid default binding struct field {index}: {error}"
                )
            }
            Self::ClassOwnerType(error) => {
                write!(formatter, "invalid default binding class owner: {error}")
            }
            Self::ClassComponent { index, error } => write!(
                formatter,
                "invalid default binding class component {index}: {error}"
            ),
            Self::Shape(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static, L: std::error::Error + 'static> std::error::Error
    for DefaultBindingShapeResolutionError<E, L>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultBindingShapeIndexError<E> {
    Binding(DefaultBindingLeafIndexError<E>),
    Element { index: usize, error: Box<Self> },
    StructField { index: usize, error: Box<Self> },
    ClassComponent { index: usize, error: Box<Self> },
}

impl<E: fmt::Display> fmt::Display for DefaultBindingShapeIndexError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Binding(error) => write!(formatter, "cannot index default binding leaf: {error}"),
            Self::Element { index, error } => {
                write!(
                    formatter,
                    "cannot index default binding tuple element {index}: {error}"
                )
            }
            Self::StructField { index, error } => {
                write!(
                    formatter,
                    "cannot index default binding struct field {index}: {error}"
                )
            }
            Self::ClassComponent { index, error } => write!(
                formatter,
                "cannot index default binding class component {index}: {error}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefaultBindingShapeIndexError<E> {}

fn canonicalize_struct_fields(
    fields: &mut [DefaultBindingStructFieldV1],
) -> Result<(), DefaultBindingShapeBuildError> {
    require_u32_len(
        fields.len(),
        DefaultBindingShapeBuildError::TooManyStructFields,
    )?;
    fields.sort_unstable_by_key(DefaultBindingStructFieldV1::declaration_index);
    for pair in fields.windows(2) {
        if pair[0].declaration_index == pair[1].declaration_index {
            return Err(DefaultBindingShapeBuildError::DuplicateStructField {
                declaration_index: pair[0].declaration_index,
            });
        }
    }
    Ok(())
}

fn validate_class_components(
    components: &[DefaultBindingClassComponentV1],
) -> Result<(), DefaultBindingShapeBuildError> {
    require_u32_len(
        components.len(),
        DefaultBindingShapeBuildError::TooManyClassComponents,
    )?;
    let mut indices = BTreeSet::new();
    for component in components {
        if !indices.insert(component.index) {
            return Err(DefaultBindingShapeBuildError::DuplicateClassComponent {
                index: component.index,
            });
        }
    }
    Ok(())
}

fn index_shapes<'a, I>(
    shapes: &'a [DefaultBindingShapeV1],
    resolver: &mut I,
) -> Result<Vec<IndexedDefaultBindingShapeV1<'a>>, DefaultBindingShapeIndexError<I::Error>>
where
    I: TemplateLocalIndexResolver,
{
    let mut indexed = Vec::with_capacity(shapes.len());
    for (index, shape) in shapes.iter().enumerate() {
        indexed.push(shape.index_locals(resolver).map_err(|error| {
            DefaultBindingShapeIndexError::Element {
                index,
                error: Box::new(error),
            }
        })?);
    }
    Ok(indexed)
}

fn require_u32_len<E>(length: usize, error: E) -> Result<u32, E> {
    u32::try_from(length).map_err(|_| error)
}

fn encode_indexed_shape(
    encoder: &mut Encoder,
    index: u32,
    shape: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(1)?;
    encoder.unsigned(u64::from(index))?;
    encoder.field(2)?;
    shape.encode(encoder)
}

fn encode_composite(
    encoder: &mut Encoder,
    tag: u64,
    owner_type: &impl WireEncode,
    children: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    owner_type.encode(encoder)?;
    encoder.field(2)?;
    encode_sequence(encoder, children)
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

struct WireSequence<'a, T>(&'a [T]);

impl<T: WireEncode> WireEncode for WireSequence<'_, T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_sequence(encoder, self.0)
    }
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
