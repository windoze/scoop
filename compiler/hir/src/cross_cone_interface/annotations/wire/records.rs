use super::*;
use crate::DecodedExportDefinitionSourceV1;
use scoop_identity::{DecodedCanonicalIdentifier, DecodedPersistentId};
use scoop_wire::WireErrorKind;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DecodedAnnotationParameterV1 {
    pub name: DecodedCanonicalIdentifier,
    pub value_type: DecodedPersistentId<PersistentTypeId>,
    pub default: Option<CanonicalConstValueV1>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DecodedAnnotationDeclarationV1 {
    pub annotation: DecodedPersistentId<PersistentAnnotationId>,
    pub parameters: Vec<DecodedAnnotationParameterV1>,
    pub visibility: DeclaredVisibilityV1,
    pub definition_origin: DecodedExportDefinitionSourceV1,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DecodedAnnotationApplicationV1 {
    pub annotation: DecodedPersistentId<PersistentAnnotationId>,
    pub arguments: Vec<CanonicalConstValueV1>,
    pub definition_origin: DecodedExportDefinitionSourceV1,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DecodedAnnotatedTargetV1 {
    pub target: DecodedAnnotationTargetV1,
    pub annotations: Vec<DecodedAnnotationApplicationV1>,
}

macro_rules! encode_parameter {
    ($type:ty) => {
        impl WireEncode for $type {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(3)?;
                encoder.field(1)?;
                self.name.encode(encoder)?;
                encoder.field(2)?;
                self.value_type.encode(encoder)?;
                encoder.field(3)?;
                encoder.array(u64::from(self.default.is_some()))?;
                if let Some(value) = &self.default {
                    value.encode(encoder)?;
                }
                Ok(())
            }
        }
    };
}
encode_parameter!(AnnotationParameterV1);
encode_parameter!(DecodedAnnotationParameterV1);

macro_rules! encode_declaration {
    ($type:ty) => {
        impl WireEncode for $type {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(4)?;
                encoder.field(1)?;
                self.annotation.encode(encoder)?;
                encoder.field(2)?;
                sequence(encoder, &self.parameters)?;
                encoder.field(3)?;
                self.visibility.encode(encoder)?;
                encoder.field(4)?;
                self.definition_origin.encode(encoder)
            }
        }
    };
}
encode_declaration!(AnnotationDeclarationV1);
encode_declaration!(DecodedAnnotationDeclarationV1);

macro_rules! encode_application {
    ($type:ty) => {
        impl WireEncode for $type {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(3)?;
                encoder.field(1)?;
                self.annotation.encode(encoder)?;
                encoder.field(2)?;
                sequence(encoder, &self.arguments)?;
                encoder.field(3)?;
                self.definition_origin.encode(encoder)
            }
        }
    };
}
encode_application!(AnnotationApplicationV1);
encode_application!(DecodedAnnotationApplicationV1);

macro_rules! encode_target {
    ($type:ty) => {
        impl WireEncode for $type {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(2)?;
                encoder.field(1)?;
                self.target.encode(encoder)?;
                encoder.field(2)?;
                sequence(encoder, &self.annotations)
            }
        }
    };
}
encode_target!(AnnotatedTargetV1);
encode_target!(DecodedAnnotatedTargetV1);

impl WireDecode for DecodedAnnotationParameterV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            name: decoder.field(1, DecodedCanonicalIdentifier::decode)?,
            value_type: decoder.field(2, DecodedPersistentId::decode)?,
            default: decoder.field(3, |d| match d.array()? {
                0 => Ok(None),
                1 => d.index(0, CanonicalConstValueV1::decode).map(Some),
                actual => Err(WireError::new(
                    WireErrorKind::InvalidLength {
                        expected: 1,
                        actual,
                    },
                    d.path().clone(),
                    Some(d.position()),
                )),
            })?,
        })
    }
}
impl WireDecode for DecodedAnnotationDeclarationV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            annotation: decoder.field(1, DecodedPersistentId::decode)?,
            parameters: decoder.field(2, |d| {
                d.decode_array(|d, _| DecodedAnnotationParameterV1::decode(d))
            })?,
            visibility: decoder.field(3, DeclaredVisibilityV1::decode)?,
            definition_origin: decoder.field(4, DecodedExportDefinitionSourceV1::decode)?,
        })
    }
}
impl WireDecode for DecodedAnnotationApplicationV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            annotation: decoder.field(1, DecodedPersistentId::decode)?,
            arguments: decoder.field(2, |d| {
                d.decode_array(|d, _| CanonicalConstValueV1::decode(d))
            })?,
            definition_origin: decoder.field(3, DecodedExportDefinitionSourceV1::decode)?,
        })
    }
}
impl WireDecode for DecodedAnnotatedTargetV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            target: decoder.field(1, DecodedAnnotationTargetV1::decode)?,
            annotations: decoder.field(2, |d| {
                d.decode_array(|d, _| DecodedAnnotationApplicationV1::decode(d))
            })?,
        })
    }
}
