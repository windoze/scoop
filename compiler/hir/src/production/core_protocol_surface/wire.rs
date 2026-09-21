mod relations;

use relations::validate_foundation_relations;

use std::fmt;

use scoop_identity::{
    CoreBuiltinNominal, DecodedPersistentId, DefinitionOriginSubject, DefinitionOwnerAtom,
    DispatchDeclarationOwner, DispatchRole, EnumVariantFieldSelector, NominalDeclarationOwner,
    PersistentDispatchSlotId, PersistentEnumVariantFieldId, PersistentEnumVariantId,
    PersistentExactTypeId, PersistentGenericTypeId, PersistentTypeId, SourceDeclarationKind,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::*;
use crate::{
    CanonicalHirFoundation, CoreProtocolCallableDefinitionV1, CoreProtocolCallableValidationError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProtocolEntryKind {
    Type,
    GenericType,
    Callable,
    EnumVariant,
    EnumVariantField,
    DispatchSlot,
    ExactType,
}

const FUNDAMENTAL_LAYOUT: [ProtocolEntryKind; FUNDAMENTAL_TYPE_COUNT] = [
    ProtocolEntryKind::Type,
    ProtocolEntryKind::Type,
    ProtocolEntryKind::Type,
    ProtocolEntryKind::Type,
    ProtocolEntryKind::Type,
    ProtocolEntryKind::Type,
    ProtocolEntryKind::Type,
    ProtocolEntryKind::Type,
    ProtocolEntryKind::Type,
    ProtocolEntryKind::Type,
    ProtocolEntryKind::Type,
    ProtocolEntryKind::GenericType,
    ProtocolEntryKind::GenericType,
    ProtocolEntryKind::GenericType,
    ProtocolEntryKind::GenericType,
];
const OPTION_LAYOUT: [ProtocolEntryKind; OPTION_PROTOCOL_COUNT] = [
    ProtocolEntryKind::GenericType,
    ProtocolEntryKind::EnumVariant,
    ProtocolEntryKind::EnumVariantField,
    ProtocolEntryKind::EnumVariant,
];
const ITERATION_LAYOUT: [ProtocolEntryKind; ITERATION_PROTOCOL_COUNT] = [
    ProtocolEntryKind::GenericType,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::DispatchSlot,
];
const EXCEPTION_LAYOUT: [ProtocolEntryKind; EXCEPTION_PROTOCOL_COUNT] = [
    ProtocolEntryKind::Type,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Type,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Type,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Type,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Type,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Type,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Callable,
];
const COROUTINE_LAYOUT: [ProtocolEntryKind; COROUTINE_PROTOCOL_COUNT] = [
    ProtocolEntryKind::GenericType,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::DispatchSlot,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::DispatchSlot,
    ProtocolEntryKind::GenericType,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::DispatchSlot,
    ProtocolEntryKind::GenericType,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::DispatchSlot,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Callable,
];
const FFI_LAYOUT: [ProtocolEntryKind; FFI_PROTOCOL_COUNT] = [
    ProtocolEntryKind::GenericType,
    ProtocolEntryKind::GenericType,
    ProtocolEntryKind::GenericType,
    ProtocolEntryKind::GenericType,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Callable,
];
const FOREIGN_CALLBACK_LAYOUT: [ProtocolEntryKind; FOREIGN_CALLBACK_PROTOCOL_COUNT] = [
    ProtocolEntryKind::GenericType,
    ProtocolEntryKind::Type,
    ProtocolEntryKind::EnumVariant,
    ProtocolEntryKind::EnumVariant,
    ProtocolEntryKind::Type,
    ProtocolEntryKind::EnumVariant,
    ProtocolEntryKind::EnumVariant,
    ProtocolEntryKind::EnumVariant,
    ProtocolEntryKind::EnumVariant,
    ProtocolEntryKind::ExactType,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Callable,
    ProtocolEntryKind::Callable,
];
const SOURCE_LOCATION_LAYOUT: [ProtocolEntryKind; SOURCE_LOCATION_PROTOCOL_COUNT] =
    [ProtocolEntryKind::Type, ProtocolEntryKind::Callable];

impl WireEncode for CoreProtocolNominalV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Type(id) => encode_value_sum(encoder, 1, id),
            Self::GenericType(id) => encode_value_sum(encoder, 2, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DecodedCoreProtocolNominalV1 {
    Type(DecodedPersistentId<PersistentTypeId>),
    GenericType(DecodedPersistentId<PersistentGenericTypeId>),
}

impl WireEncode for DecodedCoreProtocolNominalV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Type(id) => encode_value_sum(encoder, 1, id),
            Self::GenericType(id) => encode_value_sum(encoder, 2, id),
        }
    }
}

impl WireDecode for DecodedCoreProtocolNominalV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        if fields == 0 {
            return Err(wire_error(
                decoder,
                WireErrorKind::MissingField { field: 0 },
            ));
        }
        let tag = decoder.field(0, Decoder::unsigned)?;
        require_sum_length(decoder, fields, 2)?;
        match tag {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Type),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::GenericType),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

impl WireEncode for CoreProtocolEntryV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Nominal(nominal) => encode_value_sum(encoder, 1, nominal),
            Self::Callable(callable) => encode_value_sum(encoder, 2, callable),
            Self::EnumVariant(id) => encode_value_sum(encoder, 3, id),
            Self::EnumVariantField(id) => encode_value_sum(encoder, 4, id),
            Self::DispatchSlot(id) => encode_value_sum(encoder, 5, id),
            Self::ExactType(id) => encode_value_sum(encoder, 6, id),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodedCoreProtocolEntryV1 {
    Nominal(DecodedCoreProtocolNominalV1),
    Callable(crate::DecodedCoreProtocolCallableV1),
    EnumVariant(DecodedPersistentId<PersistentEnumVariantId>),
    EnumVariantField(DecodedPersistentId<PersistentEnumVariantFieldId>),
    DispatchSlot(DecodedPersistentId<PersistentDispatchSlotId>),
    ExactType(DecodedPersistentId<PersistentExactTypeId>),
}

impl WireEncode for DecodedCoreProtocolEntryV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Nominal(nominal) => encode_value_sum(encoder, 1, nominal),
            Self::Callable(callable) => encode_value_sum(encoder, 2, callable),
            Self::EnumVariant(id) => encode_value_sum(encoder, 3, id),
            Self::EnumVariantField(id) => encode_value_sum(encoder, 4, id),
            Self::DispatchSlot(id) => encode_value_sum(encoder, 5, id),
            Self::ExactType(id) => encode_value_sum(encoder, 6, id),
        }
    }
}

impl WireDecode for DecodedCoreProtocolEntryV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        if fields == 0 {
            return Err(wire_error(
                decoder,
                WireErrorKind::MissingField { field: 0 },
            ));
        }
        let tag = decoder.field(0, Decoder::unsigned)?;
        require_sum_length(decoder, fields, 2)?;
        match tag {
            1 => decoder
                .field(1, DecodedCoreProtocolNominalV1::decode)
                .map(Self::Nominal),
            2 => decoder
                .field(1, crate::DecodedCoreProtocolCallableV1::decode)
                .map(Self::Callable),
            3 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::EnumVariant),
            4 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::EnumVariantField),
            5 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::DispatchSlot),
            6 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::ExactType),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

impl<const N: usize> WireEncode for CoreProtocolProductV1<N> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(N as u64)?;
        for (index, entry) in self.entries.iter().enumerate() {
            encoder.field((index + 1) as u32)?;
            entry.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedCoreProtocolProductV1<const N: usize> {
    entries: [DecodedCoreProtocolEntryV1; N],
}

impl<const N: usize> WireEncode for DecodedCoreProtocolProductV1<N> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(N as u64)?;
        for (index, entry) in self.entries.iter().enumerate() {
            encoder.field((index + 1) as u32)?;
            entry.encode(encoder)?;
        }
        Ok(())
    }
}

impl<const N: usize> WireDecode for DecodedCoreProtocolProductV1<N> {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(N as u64)?;
        let mut entries = Vec::with_capacity(N);
        for index in 0..N {
            entries.push(decoder.field((index + 1) as u32, DecodedCoreProtocolEntryV1::decode)?);
        }
        Ok(Self {
            entries: entries
                .try_into()
                .unwrap_or_else(|_| unreachable!("decoder read the fixed protocol product length")),
        })
    }
}

macro_rules! wire_protocol_product {
    ($name:ident, $decoded:ident, $count:ident) => {
        impl WireEncode for $name {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                self.0.encode(encoder)
            }
        }

        #[derive(Clone, Debug, Eq, PartialEq)]
        struct $decoded(DecodedCoreProtocolProductV1<$count>);

        impl WireEncode for $decoded {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                self.0.encode(encoder)
            }
        }

        impl WireDecode for $decoded {
            fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
                DecodedCoreProtocolProductV1::decode(decoder).map(Self)
            }
        }
    };
}

wire_protocol_product!(
    CoreFundamentalTypeProtocolV1,
    DecodedCoreFundamentalTypeProtocolV1,
    FUNDAMENTAL_TYPE_COUNT
);
wire_protocol_product!(
    CoreOptionProtocolV1,
    DecodedCoreOptionProtocolV1,
    OPTION_PROTOCOL_COUNT
);
wire_protocol_product!(
    CoreIterationProtocolV1,
    DecodedCoreIterationProtocolV1,
    ITERATION_PROTOCOL_COUNT
);
wire_protocol_product!(
    CoreExceptionProtocolV1,
    DecodedCoreExceptionProtocolV1,
    EXCEPTION_PROTOCOL_COUNT
);
wire_protocol_product!(
    CoreCoroutineProtocolV1,
    DecodedCoreCoroutineProtocolV1,
    COROUTINE_PROTOCOL_COUNT
);
wire_protocol_product!(
    CoreFfiProtocolV1,
    DecodedCoreFfiProtocolV1,
    FFI_PROTOCOL_COUNT
);
wire_protocol_product!(
    CoreForeignCallbackProtocolV1,
    DecodedCoreForeignCallbackProtocolV1,
    FOREIGN_CALLBACK_PROTOCOL_COUNT
);
wire_protocol_product!(
    CoreSourceLocationProtocolV1,
    DecodedCoreSourceLocationProtocolV1,
    SOURCE_LOCATION_PROTOCOL_COUNT
);

impl WireEncode for CoreCompilerOperationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.kind.encode(encoder)?;
        encoder.field(2)?;
        self.callable.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedCoreCompilerOperationV1 {
    kind: IntrinsicFunctionKind,
    callable: crate::DecodedCoreProtocolCallableV1,
}

impl WireEncode for DecodedCoreCompilerOperationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.kind.encode(encoder)?;
        encoder.field(2)?;
        self.callable.encode(encoder)
    }
}

impl WireDecode for DecodedCoreCompilerOperationV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            kind: decoder.field(1, IntrinsicFunctionKind::decode)?,
            callable: decoder.field(2, crate::DecodedCoreProtocolCallableV1::decode)?,
        })
    }
}

impl WireEncode for CoreCompilerOperationProtocolV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.operations.len() as u64)?;
        for operation in &self.operations {
            operation.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedCoreCompilerOperationProtocolV1 {
    operations: Vec<DecodedCoreCompilerOperationV1>,
}

impl WireEncode for DecodedCoreCompilerOperationProtocolV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.operations.len() as u64)?;
        for operation in &self.operations {
            operation.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCoreCompilerOperationProtocolV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedCoreCompilerOperationV1::decode(decoder))
            .map(|operations| Self { operations })
    }
}

impl WireEncode for CoreCompilerProtocolSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(9)?;
        encoder.field(1)?;
        self.fundamental_types.encode(encoder)?;
        encoder.field(2)?;
        self.option_protocol.encode(encoder)?;
        encoder.field(3)?;
        self.iteration_protocol.encode(encoder)?;
        encoder.field(4)?;
        self.exception_protocol.encode(encoder)?;
        encoder.field(5)?;
        self.coroutine_protocol.encode(encoder)?;
        encoder.field(6)?;
        self.ffi_protocol.encode(encoder)?;
        encoder.field(7)?;
        self.foreign_callback_protocol.encode(encoder)?;
        encoder.field(8)?;
        self.source_location_protocol.encode(encoder)?;
        encoder.field(9)?;
        self.compiler_operation_protocol.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCoreCompilerProtocolSurfaceV1 {
    fundamental_types: DecodedCoreFundamentalTypeProtocolV1,
    option_protocol: DecodedCoreOptionProtocolV1,
    iteration_protocol: DecodedCoreIterationProtocolV1,
    exception_protocol: DecodedCoreExceptionProtocolV1,
    coroutine_protocol: DecodedCoreCoroutineProtocolV1,
    ffi_protocol: DecodedCoreFfiProtocolV1,
    foreign_callback_protocol: DecodedCoreForeignCallbackProtocolV1,
    source_location_protocol: DecodedCoreSourceLocationProtocolV1,
    compiler_operation_protocol: DecodedCoreCompilerOperationProtocolV1,
}

impl DecodedCoreCompilerProtocolSurfaceV1 {
    pub(in crate::production) fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
    ) -> Result<CoreCompilerProtocolSurfaceV1, CoreCompilerProtocolSurfaceValidationError> {
        let surface = CoreCompilerProtocolSurfaceV1 {
            fundamental_types: CoreFundamentalTypeProtocolV1(validate_product(
                self.fundamental_types.0,
                &FUNDAMENTAL_LAYOUT,
                foundation,
            )?),
            option_protocol: CoreOptionProtocolV1(validate_product(
                self.option_protocol.0,
                &OPTION_LAYOUT,
                foundation,
            )?),
            iteration_protocol: CoreIterationProtocolV1(validate_product(
                self.iteration_protocol.0,
                &ITERATION_LAYOUT,
                foundation,
            )?),
            exception_protocol: CoreExceptionProtocolV1(validate_product(
                self.exception_protocol.0,
                &EXCEPTION_LAYOUT,
                foundation,
            )?),
            coroutine_protocol: CoreCoroutineProtocolV1(validate_product(
                self.coroutine_protocol.0,
                &COROUTINE_LAYOUT,
                foundation,
            )?),
            ffi_protocol: CoreFfiProtocolV1(validate_product(
                self.ffi_protocol.0,
                &FFI_LAYOUT,
                foundation,
            )?),
            foreign_callback_protocol: CoreForeignCallbackProtocolV1(validate_product(
                self.foreign_callback_protocol.0,
                &FOREIGN_CALLBACK_LAYOUT,
                foundation,
            )?),
            source_location_protocol: CoreSourceLocationProtocolV1(validate_product(
                self.source_location_protocol.0,
                &SOURCE_LOCATION_LAYOUT,
                foundation,
            )?),
            compiler_operation_protocol: validate_operations(
                self.compiler_operation_protocol,
                foundation,
            )?,
        };
        surface
            .validate_internal_relations()
            .map_err(CoreCompilerProtocolSurfaceValidationError::Relation)?;
        validate_foundation_relations(&surface, foundation)?;
        validate_fixed_callable_signatures(&surface)
            .map_err(CoreCompilerProtocolSurfaceValidationError::Relation)?;
        surface
            .validate_operation_signatures()
            .map_err(CoreCompilerProtocolSurfaceValidationError::Relation)?;
        Ok(surface)
    }
}

impl WireEncode for DecodedCoreCompilerProtocolSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(9)?;
        encoder.field(1)?;
        self.fundamental_types.encode(encoder)?;
        encoder.field(2)?;
        self.option_protocol.encode(encoder)?;
        encoder.field(3)?;
        self.iteration_protocol.encode(encoder)?;
        encoder.field(4)?;
        self.exception_protocol.encode(encoder)?;
        encoder.field(5)?;
        self.coroutine_protocol.encode(encoder)?;
        encoder.field(6)?;
        self.ffi_protocol.encode(encoder)?;
        encoder.field(7)?;
        self.foreign_callback_protocol.encode(encoder)?;
        encoder.field(8)?;
        self.source_location_protocol.encode(encoder)?;
        encoder.field(9)?;
        self.compiler_operation_protocol.encode(encoder)
    }
}

impl WireDecode for DecodedCoreCompilerProtocolSurfaceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(9)?;
        Ok(Self {
            fundamental_types: decoder.field(1, DecodedCoreFundamentalTypeProtocolV1::decode)?,
            option_protocol: decoder.field(2, DecodedCoreOptionProtocolV1::decode)?,
            iteration_protocol: decoder.field(3, DecodedCoreIterationProtocolV1::decode)?,
            exception_protocol: decoder.field(4, DecodedCoreExceptionProtocolV1::decode)?,
            coroutine_protocol: decoder.field(5, DecodedCoreCoroutineProtocolV1::decode)?,
            ffi_protocol: decoder.field(6, DecodedCoreFfiProtocolV1::decode)?,
            foreign_callback_protocol: decoder
                .field(7, DecodedCoreForeignCallbackProtocolV1::decode)?,
            source_location_protocol: decoder
                .field(8, DecodedCoreSourceLocationProtocolV1::decode)?,
            compiler_operation_protocol: decoder
                .field(9, DecodedCoreCompilerOperationProtocolV1::decode)?,
        })
    }
}

fn validate_product<const N: usize>(
    decoded: DecodedCoreProtocolProductV1<N>,
    layout: &[ProtocolEntryKind; N],
    foundation: &CanonicalHirFoundation,
) -> Result<CoreProtocolProductV1<N>, CoreCompilerProtocolSurfaceValidationError> {
    let entries = decoded
        .entries
        .into_iter()
        .zip(layout)
        .enumerate()
        .map(|(index, (entry, expected))| {
            let actual = entry_kind(&entry);
            if actual != *expected {
                return Err(CoreCompilerProtocolSurfaceValidationError::RoleKindMismatch { index });
            }
            validate_entry(entry, foundation)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CoreProtocolProductV1 {
        entries: entries
            .try_into()
            .unwrap_or_else(|_| unreachable!("validated the fixed protocol product length")),
    })
}

fn validate_entry(
    decoded: DecodedCoreProtocolEntryV1,
    foundation: &CanonicalHirFoundation,
) -> Result<CoreProtocolEntryV1, CoreCompilerProtocolSurfaceValidationError> {
    match decoded {
        DecodedCoreProtocolEntryV1::Nominal(DecodedCoreProtocolNominalV1::Type(id)) => {
            let (id, source) = foundation.source_type_by_bytes(id.as_array()).ok_or(
                CoreCompilerProtocolSurfaceValidationError::UnknownType(*id.as_array()),
            )?;
            require_core_source(source)?;
            if id != CoreBuiltinNominal::Unit.identity_record().id() {
                require_origin(foundation, DefinitionOriginSubject::Type(id))?;
            }
            Ok(CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(
                id,
            )))
        }
        DecodedCoreProtocolEntryV1::Nominal(DecodedCoreProtocolNominalV1::GenericType(id)) => {
            let (id, source) = foundation.generic_type_by_bytes(id.as_array()).ok_or(
                CoreCompilerProtocolSurfaceValidationError::UnknownGenericType(*id.as_array()),
            )?;
            require_core_source(source)?;
            require_origin(foundation, DefinitionOriginSubject::GenericType(id))?;
            Ok(CoreProtocolEntryV1::Nominal(
                CoreProtocolNominalV1::GenericType(id),
            ))
        }
        DecodedCoreProtocolEntryV1::Callable(callable) => callable
            .validate_against(foundation)
            .map(CoreProtocolEntryV1::Callable)
            .map_err(CoreCompilerProtocolSurfaceValidationError::Callable),
        DecodedCoreProtocolEntryV1::EnumVariant(id) => {
            let (id, key) = foundation.enum_variant_by_bytes(id.as_array()).ok_or(
                CoreCompilerProtocolSurfaceValidationError::UnknownEnumVariant(*id.as_array()),
            )?;
            require_core_nominal_owner(foundation, key.source_owner())?;
            require_origin(foundation, DefinitionOriginSubject::EnumVariant(id))?;
            Ok(CoreProtocolEntryV1::EnumVariant(id))
        }
        DecodedCoreProtocolEntryV1::EnumVariantField(id) => {
            let (id, key) = foundation
                .enum_variant_field_by_bytes(id.as_array())
                .ok_or(
                    CoreCompilerProtocolSurfaceValidationError::UnknownEnumVariantField(
                        *id.as_array(),
                    ),
                )?;
            let (_, variant) = foundation
                .enum_variant_by_bytes(key.variant().as_array())
                .ok_or(
                    CoreCompilerProtocolSurfaceValidationError::UnknownEnumVariant(
                        *key.variant().as_array(),
                    ),
                )?;
            require_core_nominal_owner(foundation, variant.source_owner())?;
            require_origin(foundation, DefinitionOriginSubject::EnumVariantField(id))?;
            Ok(CoreProtocolEntryV1::EnumVariantField(id))
        }
        DecodedCoreProtocolEntryV1::DispatchSlot(id) => {
            let (id, _) = foundation.dispatch_slot_by_bytes(id.as_array()).ok_or(
                CoreCompilerProtocolSurfaceValidationError::UnknownDispatchSlot(*id.as_array()),
            )?;
            Ok(CoreProtocolEntryV1::DispatchSlot(id))
        }
        DecodedCoreProtocolEntryV1::ExactType(id) => {
            let (id, _) = foundation.exact_type_by_bytes(id.as_array()).ok_or(
                CoreCompilerProtocolSurfaceValidationError::UnknownExactType(*id.as_array()),
            )?;
            Ok(CoreProtocolEntryV1::ExactType(id))
        }
    }
}

fn validate_operations(
    decoded: DecodedCoreCompilerOperationProtocolV1,
    foundation: &CanonicalHirFoundation,
) -> Result<CoreCompilerOperationProtocolV1, CoreCompilerProtocolSurfaceValidationError> {
    let expected = intrinsic_function_kinds();
    if decoded.operations.len() != expected.len() {
        return Err(CoreCompilerProtocolSurfaceValidationError::Relation(
            CoreCompilerProtocolSurfaceRelationError::OperationCoverage {
                expected: expected.len(),
                actual: decoded.operations.len(),
            },
        ));
    }
    let operations = decoded
        .operations
        .into_iter()
        .zip(expected)
        .enumerate()
        .map(|(index, (operation, expected))| {
            if operation.kind != expected {
                return Err(CoreCompilerProtocolSurfaceValidationError::Relation(
                    CoreCompilerProtocolSurfaceRelationError::OperationRoleMismatch {
                        index,
                        expected,
                        actual: operation.kind,
                    },
                ));
            }
            operation
                .callable
                .validate_against(foundation)
                .map(|callable| CoreCompilerOperationV1 {
                    kind: operation.kind,
                    callable,
                })
                .map_err(CoreCompilerProtocolSurfaceValidationError::Callable)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CoreCompilerOperationProtocolV1 { operations })
}

fn entry_kind(entry: &DecodedCoreProtocolEntryV1) -> ProtocolEntryKind {
    match entry {
        DecodedCoreProtocolEntryV1::Nominal(DecodedCoreProtocolNominalV1::Type(_)) => {
            ProtocolEntryKind::Type
        }
        DecodedCoreProtocolEntryV1::Nominal(DecodedCoreProtocolNominalV1::GenericType(_)) => {
            ProtocolEntryKind::GenericType
        }
        DecodedCoreProtocolEntryV1::Callable(_) => ProtocolEntryKind::Callable,
        DecodedCoreProtocolEntryV1::EnumVariant(_) => ProtocolEntryKind::EnumVariant,
        DecodedCoreProtocolEntryV1::EnumVariantField(_) => ProtocolEntryKind::EnumVariantField,
        DecodedCoreProtocolEntryV1::DispatchSlot(_) => ProtocolEntryKind::DispatchSlot,
        DecodedCoreProtocolEntryV1::ExactType(_) => ProtocolEntryKind::ExactType,
    }
}

fn require_core_source(
    source: &scoop_identity::SourceDeclarationKey,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    if source.origin() == scoop_identity::ConeIdentity::CORE {
        Ok(())
    } else {
        Err(CoreCompilerProtocolSurfaceValidationError::NonCoreSource)
    }
}

fn require_core_nominal_owner(
    foundation: &CanonicalHirFoundation,
    owner: Option<NominalDeclarationOwner>,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    match owner.ok_or(CoreCompilerProtocolSurfaceValidationError::GeneratedEnumMember)? {
        NominalDeclarationOwner::Concrete(id) => {
            let (_, source) = foundation.source_type_by_bytes(id.as_array()).ok_or(
                CoreCompilerProtocolSurfaceValidationError::UnknownType(*id.as_array()),
            )?;
            require_core_source(source)
        }
        NominalDeclarationOwner::GenericTemplate(id) => {
            let (_, source) = foundation.generic_type_by_bytes(id.as_array()).ok_or(
                CoreCompilerProtocolSurfaceValidationError::UnknownGenericType(*id.as_array()),
            )?;
            require_core_source(source)
        }
    }
}

fn require_origin(
    foundation: &CanonicalHirFoundation,
    subject: DefinitionOriginSubject,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    if foundation.definition_origin(subject).is_some() {
        Ok(())
    } else {
        Err(CoreCompilerProtocolSurfaceValidationError::MissingDefinitionOrigin(subject))
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum CoreCompilerProtocolSurfaceValidationError {
    RoleKindMismatch {
        index: usize,
    },
    UnknownType([u8; 32]),
    UnknownGenericType([u8; 32]),
    UnknownEnumVariant([u8; 32]),
    UnknownEnumVariantField([u8; 32]),
    UnknownDispatchSlot([u8; 32]),
    UnknownExactType([u8; 32]),
    NonCoreSource,
    GeneratedEnumMember,
    MissingDefinitionOrigin(DefinitionOriginSubject),
    NominalRoleMismatch {
        product: CoreProtocolProductKindV1,
        index: usize,
    },
    OptionOwnerMismatch,
    OptionPayloadMismatch,
    OptionVariantFieldCount {
        variant: PersistentEnumVariantId,
        expected: usize,
        actual: usize,
    },
    CallbackVariantOwnerMismatch {
        index: usize,
    },
    CallbackFailureTypeMismatch,
    InterfaceDispatchMismatch {
        product: CoreProtocolProductKindV1,
        callable_index: usize,
        slot_index: usize,
    },
    RoleCallableOwnerMismatch {
        product: CoreProtocolProductKindV1,
        index: usize,
    },
    OperationOwnerMismatch(IntrinsicFunctionKind),
    Callable(CoreProtocolCallableValidationError),
    Relation(CoreCompilerProtocolSurfaceRelationError),
}

impl fmt::Display for CoreCompilerProtocolSurfaceValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid core compiler protocol surface: {self:?}"
        )
    }
}

impl std::error::Error for CoreCompilerProtocolSurfaceValidationError {}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn require_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
