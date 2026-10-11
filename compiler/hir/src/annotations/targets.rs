use crate::{
    ClassConstructorId, EnumVariantRef, FunctionId, PropertyGetterId, PropertyId, PropertySetterId,
    StructConstructorId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

/// Source parameters remain separate from hidden ABI and capture inputs.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SourceAnnotationCallable {
    Function(FunctionId),
    StructConstructor(StructConstructorId),
    ClassConstructor(ClassConstructorId),
    VariantConstructor(EnumVariantRef),
    Getter(PropertyGetterId),
    Setter(PropertySetterId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SourceAnnotationReceiver {
    Callable(SourceAnnotationCallable),
    Property(PropertyId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AnnotationStorageRole {
    Backing,
    Delegate,
}

impl WireEncode for AnnotationStorageRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Backing => 1,
            Self::Delegate => 2,
        })
    }
}

impl WireDecode for AnnotationStorageRole {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Backing),
            2 => Ok(Self::Delegate),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}
