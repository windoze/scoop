mod error;
mod layout;
use layout::*;
mod references;
mod relations;

pub use error::CoreCompilerProtocolSurfaceValidationError;
use references::validate_entry;

use relations::validate_foundation_relations;

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
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
            fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
        self.program_arguments.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCoreCompilerProtocolSurfaceV1 {
    program_arguments: crate::DecodedCoreProtocolCallableV1,
    fundamental_types: DecodedCoreFundamentalTypeProtocolV1,
    option_protocol: DecodedCoreOptionProtocolV1,
    iteration_protocol: DecodedCoreIterationProtocolV1,
    exception_protocol: DecodedCoreExceptionProtocolV1,
    coroutine_protocol: DecodedCoreCoroutineProtocolV1,
    ffi_protocol: DecodedCoreFfiProtocolV1,
    foreign_callback_protocol: DecodedCoreForeignCallbackProtocolV1,
    source_location_protocol: DecodedCoreSourceLocationProtocolV1,
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
            program_arguments: self
                .program_arguments
                .validate_against(foundation)
                .map_err(CoreCompilerProtocolSurfaceValidationError::Callable)?,
        };
        surface
            .validate_internal_relations()
            .map_err(CoreCompilerProtocolSurfaceValidationError::Relation)?;
        validate_foundation_relations(&surface, foundation)?;
        validate_fixed_callable_signatures(&surface)
            .map_err(CoreCompilerProtocolSurfaceValidationError::Relation)?;
        surface
            .validate_fixed_intrinsic_signatures()
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
        self.program_arguments.encode(encoder)
    }
}

impl WireDecode for DecodedCoreCompilerProtocolSurfaceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
            program_arguments: decoder.field(9, crate::DecodedCoreProtocolCallableV1::decode)?,
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

fn require_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
