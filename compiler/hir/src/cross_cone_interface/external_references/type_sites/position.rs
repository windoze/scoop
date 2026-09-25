use super::*;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum HirCallableTypePositionV1 {
    Receiver,
    Parameter(u32),
    Result,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum HirDependencyTypePositionV1 {
    Expression(ExecutableExpressionPosition, HirExpressionTypeRoleV1),
    CallableSignature(CallableMaterialization, HirCallableTypePositionV1),
    LocalValue(PersistentLocalValueId),
    BackingStorage(PropertyOwner),
    DelegateStorage(PropertyOwner),
    FieldStorage(PersistentFieldId),
    EnumVariantFieldStorage(PersistentEnumVariantFieldId),
    ConstructorInitializerResult(CallableMaterialization),
    InitializationCycleMessage(PersistentInitializationUnitId),
}

impl WireEncode for HirCallableTypePositionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(if matches!(self, Self::Parameter(_)) {
            2
        } else {
            1
        })?;
        encoder.field(0)?;
        match self {
            Self::Receiver => encoder.unsigned(1),
            Self::Parameter(index) => {
                encoder.unsigned(2)?;
                encoder.field(1)?;
                encoder.unsigned(u64::from(*index))
            }
            Self::Result => encoder.unsigned(3),
        }
    }
}

impl WireDecode for HirCallableTypePositionV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                super::wire::require_fields(decoder, fields, 1)?;
                Ok(Self::Receiver)
            }
            2 => {
                super::wire::require_fields(decoder, fields, 2)?;
                decoder.field(1, Decoder::u32).map(Self::Parameter)
            }
            3 => {
                super::wire::require_fields(decoder, fields, 1)?;
                Ok(Self::Result)
            }
            tag => Err(super::wire::unknown_tag(decoder, tag)),
        }
    }
}
