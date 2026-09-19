use super::{ProtectedCallableInterfaceBuildError, wire};
use crate::CallableDeclarationIdResolver;
use scoop_identity::{
    CallableTemplateOrigin, DecodedPersistentId, PersistentConstructorId, PersistentEnumVariantId,
    PersistentFunctionId, PersistentGenericFunctionId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

/// Nested support uses an independent source protocol family. Accessors have no
/// source argument/default protocol, including setters with a value parameter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NominalSupportSourceInterfaceUseV1 {
    AccessorNoSourceInterface,
    Function(PersistentFunctionId),
    GenericFunction(PersistentGenericFunctionId),
    Constructor(PersistentConstructorId),
    VariantConstructor(PersistentEnumVariantId),
}
impl NominalSupportSourceInterfaceUseV1 {
    pub fn for_declaration(
        declaration: CallableTemplateOrigin,
    ) -> Result<Self, ProtectedCallableInterfaceBuildError> {
        match declaration {
            CallableTemplateOrigin::Function(id) => Ok(Self::Function(id)),
            CallableTemplateOrigin::GenericFunction(id) => Ok(Self::GenericFunction(id)),
            CallableTemplateOrigin::Constructor(id) => Ok(Self::Constructor(id)),
            CallableTemplateOrigin::Accessor(_) => Ok(Self::AccessorNoSourceInterface),
            CallableTemplateOrigin::VariantConstructor(id) => Ok(Self::VariantConstructor(id)),
        }
    }
}
impl WireEncode for NominalSupportSourceInterfaceUseV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, value): (u64, &dyn WireEncode) = match self {
            Self::AccessorNoSourceInterface => return wire::tag(encoder, 1, 1),
            Self::Function(id) => (2, id),
            Self::GenericFunction(id) => (3, id),
            Self::Constructor(id) => (4, id),
            Self::VariantConstructor(id) => (5, id),
        };
        wire::tag(encoder, 2, tag)?;
        encoder.field(1)?;
        value.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedNominalSupportSourceInterfaceUseV1 {
    AccessorNoSourceInterface,
    Function(DecodedPersistentId<PersistentFunctionId>),
    GenericFunction(DecodedPersistentId<PersistentGenericFunctionId>),
    Constructor(DecodedPersistentId<PersistentConstructorId>),
    VariantConstructor(DecodedPersistentId<PersistentEnumVariantId>),
}
impl DecodedNominalSupportSourceInterfaceUseV1 {
    pub fn resolve<R: CallableDeclarationIdResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<NominalSupportSourceInterfaceUseV1, E> {
        match self {
            Self::AccessorNoSourceInterface => {
                Ok(NominalSupportSourceInterfaceUseV1::AccessorNoSourceInterface)
            }
            Self::Function(id) => resolver
                .resolve(id)
                .map(NominalSupportSourceInterfaceUseV1::Function),
            Self::GenericFunction(id) => resolver
                .resolve(id)
                .map(NominalSupportSourceInterfaceUseV1::GenericFunction),
            Self::Constructor(id) => resolver
                .resolve(id)
                .map(NominalSupportSourceInterfaceUseV1::Constructor),
            Self::VariantConstructor(id) => resolver
                .resolve(id)
                .map(NominalSupportSourceInterfaceUseV1::VariantConstructor),
        }
    }
}
impl WireEncode for DecodedNominalSupportSourceInterfaceUseV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, value): (u64, &dyn WireEncode) = match self {
            Self::AccessorNoSourceInterface => return wire::tag(encoder, 1, 1),
            Self::Function(id) => (2, id),
            Self::GenericFunction(id) => (3, id),
            Self::Constructor(id) => (4, id),
            Self::VariantConstructor(id) => (5, id),
        };
        wire::tag(encoder, 2, tag)?;
        encoder.field(1)?;
        value.encode(encoder)
    }
}
impl WireDecode for DecodedNominalSupportSourceInterfaceUseV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                wire::expect_fields(decoder, fields, 1)?;
                Ok(Self::AccessorNoSourceInterface)
            }
            2 => {
                wire::expect_fields(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::Function)
            }
            3 => {
                wire::expect_fields(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::GenericFunction)
            }
            4 => {
                wire::expect_fields(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::Constructor)
            }
            5 => {
                wire::expect_fields(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::VariantConstructor)
            }
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
