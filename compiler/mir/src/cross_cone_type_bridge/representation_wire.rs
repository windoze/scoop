use super::*;
use scoop_identity::PersistentIdResolver;

mod fields;
mod policy;
pub use fields::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedMirTypeRepresentationV1 {
    Intrinsic(MirParamFreeIntrinsicV1),
    Struct {
        fields: Vec<DecodedMirRepresentationFieldV1>,
        c_layout: MirTypeCLayoutPolicyV1,
        interior_mutable: bool,
    },
    Enum {
        variants: Vec<DecodedMirRepresentationVariantV1>,
    },
    Class {
        kind: MirClassKindV1,
        declared_fields: Vec<DecodedMirRepresentationFieldV1>,
    },
    Interface,
    InlineArray {
        element: DecodedPersistentId<PersistentExactTypeId>,
    },
    ObjectBacking {
        declared_fields: Vec<DecodedMirRepresentationFieldV1>,
    },
    BoxedValue {
        payload: DecodedMirRepresentationFieldV1,
    },
    CoroutineStep {
        variants: Vec<DecodedMirRepresentationVariantV1>,
    },
    CoroutineSlot {
        variants: Vec<DecodedMirRepresentationVariantV1>,
    },
    Object {
        backing: DecodedPersistentId<PersistentExactTypeId>,
    },
}
impl DecodedMirTypeRepresentationV1 {
    pub(super) fn resolve(
        self,
        graph: &mut ValidatedIdentityGraph,
    ) -> Result<MirTypeRepresentationV1, MirTypeBridgeError> {
        Ok(match self {
            Self::Intrinsic(value) => MirTypeRepresentationV1::Intrinsic(value),
            Self::Struct {
                fields,
                c_layout,
                interior_mutable,
            } => MirTypeRepresentationV1::Struct {
                fields: resolve_fields(fields, graph)?,
                c_layout,
                interior_mutable,
            },
            Self::Enum { variants } => MirTypeRepresentationV1::Enum {
                variants: resolve_variants(variants, graph)?,
            },
            Self::Class {
                kind,
                declared_fields,
            } => MirTypeRepresentationV1::Class {
                kind,
                declared_fields: resolve_fields(declared_fields, graph)?,
            },
            Self::Interface => MirTypeRepresentationV1::Interface,
            Self::InlineArray { element } => MirTypeRepresentationV1::InlineArray {
                element: graph.resolve(element)?,
            },
            Self::ObjectBacking { declared_fields } => MirTypeRepresentationV1::ObjectBacking {
                declared_fields: resolve_fields(declared_fields, graph)?,
            },
            Self::BoxedValue { payload } => MirTypeRepresentationV1::BoxedValue {
                payload: payload.resolve(graph)?,
            },
            Self::CoroutineStep { variants } => MirTypeRepresentationV1::CoroutineStep {
                variants: resolve_variants(variants, graph)?,
            },
            Self::CoroutineSlot { variants } => MirTypeRepresentationV1::CoroutineSlot {
                variants: resolve_variants(variants, graph)?,
            },
            Self::Object { backing } => MirTypeRepresentationV1::Object {
                backing: graph.resolve(backing)?,
            },
        })
    }
}
fn resolve_fields(
    fields: Vec<DecodedMirRepresentationFieldV1>,
    graph: &mut ValidatedIdentityGraph,
) -> Result<Vec<MirRepresentationFieldV1>, MirTypeBridgeError> {
    wire::resolve_sequence(fields, graph, |field, graph| field.resolve(graph))
}
fn resolve_variants(
    variants: Vec<DecodedMirRepresentationVariantV1>,
    graph: &mut ValidatedIdentityGraph,
) -> Result<Vec<MirRepresentationVariantV1>, MirTypeBridgeError> {
    wire::resolve_sequence(variants, graph, |variant, graph| variant.resolve(graph))
}

// Both trust levels serialize the same representation sum. Sharing this
// implementation prevents a wire drift between producer and decoded input.
macro_rules! encode_representation {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                match self {
                    Self::Intrinsic(value) => {
                        tag(encoder, 2, 1)?;
                        encoder.field(1)?;
                        value.encode(encoder)
                    }
                    Self::Struct {
                        fields,
                        c_layout,
                        interior_mutable,
                    } => {
                        tag(encoder, 4, 2)?;
                        encoder.field(1)?;
                        sequence(encoder, fields)?;
                        encoder.field(2)?;
                        c_layout.encode(encoder)?;
                        encoder.field(3)?;
                        encoder.unsigned(u64::from(*interior_mutable))
                    }
                    Self::Enum { variants } => {
                        tag(encoder, 2, 3)?;
                        encoder.field(1)?;
                        sequence(encoder, variants)
                    }
                    Self::Class {
                        kind,
                        declared_fields,
                    } => {
                        tag(encoder, 3, 4)?;
                        encoder.field(1)?;
                        kind.encode(encoder)?;
                        encoder.field(2)?;
                        sequence(encoder, declared_fields)
                    }
                    Self::Interface => tag(encoder, 1, 5),
                    Self::InlineArray { element } => {
                        tag(encoder, 2, 11)?;
                        encoder.field(1)?;
                        element.encode(encoder)
                    }
                    Self::ObjectBacking { declared_fields } => {
                        tag(encoder, 2, 6)?;
                        encoder.field(1)?;
                        sequence(encoder, declared_fields)
                    }
                    Self::BoxedValue { payload } => {
                        tag(encoder, 2, 7)?;
                        encoder.field(1)?;
                        payload.encode(encoder)
                    }
                    Self::CoroutineStep { variants } => {
                        tag(encoder, 2, 8)?;
                        encoder.field(1)?;
                        sequence(encoder, variants)
                    }
                    Self::CoroutineSlot { variants } => {
                        tag(encoder, 2, 9)?;
                        encoder.field(1)?;
                        sequence(encoder, variants)
                    }
                    Self::Object { backing } => {
                        tag(encoder, 2, 10)?;
                        encoder.field(1)?;
                        backing.encode(encoder)
                    }
                }
            }
        }
    };
}
encode_representation!(MirTypeRepresentationV1);
encode_representation!(DecodedMirTypeRepresentationV1);

impl WireDecode for DecodedMirTypeRepresentationV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let count = decoder.map()?;
        let kind = decoder.field(0, Decoder::unsigned)?;
        fields(
            decoder,
            count,
            match kind {
                2 => 4,
                4 => 3,
                5 => 1,
                _ => 2,
            },
        )?;
        match kind {
            1 => decoder
                .field(1, MirParamFreeIntrinsicV1::decode)
                .map(Self::Intrinsic),
            2 => Ok(Self::Struct {
                fields: decoder.field(1, decode_fields)?,
                c_layout: decoder.field(2, MirTypeCLayoutPolicyV1::decode)?,
                interior_mutable: decoder.field(3, decode_boolean)?,
            }),
            3 => Ok(Self::Enum {
                variants: decoder.field(1, decode_variants)?,
            }),
            4 => Ok(Self::Class {
                kind: decoder.field(1, MirClassKindV1::decode)?,
                declared_fields: decoder.field(2, decode_fields)?,
            }),
            5 => Ok(Self::Interface),
            6 => Ok(Self::ObjectBacking {
                declared_fields: decoder.field(1, decode_fields)?,
            }),
            7 => Ok(Self::BoxedValue {
                payload: decoder.field(1, DecodedMirRepresentationFieldV1::decode)?,
            }),
            8 => Ok(Self::CoroutineStep {
                variants: decoder.field(1, decode_variants)?,
            }),
            9 => Ok(Self::CoroutineSlot {
                variants: decoder.field(1, decode_variants)?,
            }),
            10 => Ok(Self::Object {
                backing: decoder.field(1, DecodedPersistentId::decode)?,
            }),
            11 => Ok(Self::InlineArray {
                element: decoder.field(1, DecodedPersistentId::decode)?,
            }),
            tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
fn decode_fields(
    decoder: &mut Decoder<'_>,
) -> Result<Vec<DecodedMirRepresentationFieldV1>, WireError> {
    decoder.decode_array(|decoder, _| DecodedMirRepresentationFieldV1::decode(decoder))
}
fn decode_variants(
    decoder: &mut Decoder<'_>,
) -> Result<Vec<DecodedMirRepresentationVariantV1>, WireError> {
    decoder.decode_array(|decoder, _| DecodedMirRepresentationVariantV1::decode(decoder))
}

fn decode_boolean(decoder: &mut Decoder<'_>) -> Result<bool, WireError> {
    match decoder.unsigned()? {
        0 => Ok(false),
        1 => Ok(true),
        tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
    }
}
