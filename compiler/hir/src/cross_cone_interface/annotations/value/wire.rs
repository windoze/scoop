use super::super::wire::sequence;
use super::decoded::DecodedAnnotationElementV1;
use super::*;

macro_rules! encode_value {
    ($type:ty) => {
        impl WireEncode for $type {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(match self {
                    Self::Scalar(_) => 2,
                    Self::IntegralEnum { .. } => 4,
                    _ => 3,
                })?;
                encoder.field(0)?;
                encoder.unsigned(match self {
                    Self::Scalar(_) => 1,
                    Self::Array { .. } => 2,
                    Self::IntegralEnum { .. } => 3,
                    Self::Annotation { .. } => 4,
                })?;
                encoder.field(1)?;
                match self {
                    Self::Scalar(value) => value.encode(encoder),
                    Self::IntegralEnum {
                        value_type,
                        variant,
                        value,
                    } => {
                        value_type.encode(encoder)?;
                        encoder.field(2)?;
                        variant.encode(encoder)?;
                        encoder.field(3)?;
                        value.encode(encoder)
                    }
                    Self::Annotation {
                        annotation,
                        arguments,
                    } => {
                        annotation.encode(encoder)?;
                        encoder.field(2)?;
                        sequence(encoder, arguments)
                    }
                    Self::Array {
                        element_type,
                        elements,
                    } => {
                        element_type.encode(encoder)?;
                        encoder.field(2)?;
                        sequence(encoder, elements)
                    }
                }
            }
        }
    };
}
encode_value!(CanonicalAnnotationValueV1);
encode_value!(DecodedAnnotationValueV1);

macro_rules! encode_element {
    ($type:ty) => {
        impl WireEncode for $type {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(3)?;
                encoder.field(1)?;
                self.value.encode(encoder)?;
                encoder.field(2)?;
                self.source.encode(encoder)?;
                encoder.field(3)?;
                self.span.encode(encoder)
            }
        }
    };
}
encode_element!(AnnotationElementV1);
encode_element!(DecodedAnnotationElementV1);
