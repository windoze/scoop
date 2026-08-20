use crate::{
    CanonicalConstValueKindV1, CanonicalConstValueV1, CanonicalIntegerConstantV1,
    DefaultIntegerKindV1, FloatKind,
};
use scoop_identity::{
    PersistentAnnotationId, PersistentEnumVariantId, SignatureTypeKey, SourceIdentity, SourceSpan,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

mod decoded;
mod element_type;
mod scalar_kind;
mod wire;
pub use decoded::DecodedAnnotationValueV1;
pub use element_type::AnnotationElementTypeV1;
#[cfg(test)]
mod tests;

/// Complete static data; neither nested annotations nor arrays allocate runtime objects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalAnnotationValueV1 {
    Scalar(CanonicalConstValueV1),
    IntegralEnum {
        value_type: SignatureTypeKey,
        variant: PersistentEnumVariantId,
        value: CanonicalIntegerConstantV1,
    },
    Annotation {
        annotation: PersistentAnnotationId,
        arguments: Vec<Self>,
    },
    Array {
        element_type: AnnotationElementTypeV1,
        elements: Vec<AnnotationElementV1>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnnotationElementV1 {
    pub value: CanonicalAnnotationValueV1,
    pub source: SourceIdentity,
    pub span: SourceSpan,
}

pub(super) fn expect_fields(
    decoder: &Decoder<'_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}
pub(super) fn error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
