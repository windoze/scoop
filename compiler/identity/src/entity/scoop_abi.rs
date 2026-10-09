use std::fmt;
use std::num::NonZeroU64;

use scoop_wire::{Encoder, WireEncode};

use super::{ExactCallableSignature, GcEffect};
use crate::{AbiCoercion, PersistentExactTypeId};

mod decode;

pub use decode::{
    DecodedCanonicalScoopAbiFunctionSignature, DecodedCanonicalScoopStorage,
    DecodedScoopAbiArgument, DecodedScoopAbiReturn, ScoopAbiResolutionError,
};

mod storage;
pub use storage::{CanonicalScoopStorage, ScoopAbiValueShape};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ScoopAbiArgument {
    ElidedZst(CanonicalScoopStorage),
    Direct(CanonicalScoopStorage),
    Indirect(CanonicalScoopStorage),
    DirectParts(CanonicalScoopStorage, AbiCoercion),
}

impl ScoopAbiArgument {
    pub fn elided_zst(storage: CanonicalScoopStorage) -> Result<Self, ScoopAbiError> {
        require_zero_size(storage)?;
        Ok(Self::ElidedZst(storage))
    }

    pub fn direct(storage: CanonicalScoopStorage) -> Result<Self, ScoopAbiError> {
        require_nonzero_shape(storage, ScoopAbiValueShape::Scalar)?;
        Ok(Self::Direct(storage))
    }

    pub fn direct_parts(
        storage: CanonicalScoopStorage,
        coercion: AbiCoercion,
    ) -> Result<Self, ScoopAbiError> {
        require_direct_parts(storage, coercion)?;
        Ok(Self::DirectParts(storage, coercion))
    }

    pub fn indirect(storage: CanonicalScoopStorage) -> Result<Self, ScoopAbiError> {
        require_nonzero_shape(storage, ScoopAbiValueShape::Aggregate)?;
        Ok(Self::Indirect(storage))
    }

    pub const fn storage(self) -> CanonicalScoopStorage {
        match self {
            Self::ElidedZst(storage)
            | Self::Direct(storage)
            | Self::Indirect(storage)
            | Self::DirectParts(storage, _) => storage,
        }
    }
}

impl WireEncode for ScoopAbiArgument {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, storage) = match self {
            Self::ElidedZst(storage) => (1, storage),
            Self::Direct(storage) => (2, storage),
            Self::Indirect(storage) => (3, storage),
            Self::DirectParts(storage, coercion) => {
                return encode_parts(encoder, 4, storage, coercion);
            }
        };
        encode_value_sum(encoder, tag, storage)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ScoopAbiReturn {
    UnitVoid,
    ElidedZst(CanonicalScoopStorage),
    Direct(CanonicalScoopStorage),
    Indirect(CanonicalScoopStorage),
    DirectParts(CanonicalScoopStorage, AbiCoercion),
}

impl ScoopAbiReturn {
    pub const fn unit_void() -> Self {
        Self::UnitVoid
    }

    pub fn elided_zst(storage: CanonicalScoopStorage) -> Result<Self, ScoopAbiError> {
        require_zero_size(storage)?;
        Ok(Self::ElidedZst(storage))
    }

    pub fn direct(storage: CanonicalScoopStorage) -> Result<Self, ScoopAbiError> {
        require_nonzero_shape(storage, ScoopAbiValueShape::Scalar)?;
        Ok(Self::Direct(storage))
    }

    pub fn direct_parts(
        storage: CanonicalScoopStorage,
        coercion: AbiCoercion,
    ) -> Result<Self, ScoopAbiError> {
        require_direct_parts(storage, coercion)?;
        Ok(Self::DirectParts(storage, coercion))
    }

    pub fn indirect(storage: CanonicalScoopStorage) -> Result<Self, ScoopAbiError> {
        require_nonzero_shape(storage, ScoopAbiValueShape::Aggregate)?;
        Ok(Self::Indirect(storage))
    }

    pub fn storage(self) -> Option<CanonicalScoopStorage> {
        match self {
            Self::UnitVoid => None,
            Self::ElidedZst(storage)
            | Self::Direct(storage)
            | Self::Indirect(storage)
            | Self::DirectParts(storage, _) => Some(storage),
        }
    }
}

impl WireEncode for ScoopAbiReturn {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::UnitVoid => encode_empty_sum(encoder, 1),
            Self::ElidedZst(storage) => encode_value_sum(encoder, 2, storage),
            Self::Direct(storage) => encode_value_sum(encoder, 3, storage),
            Self::Indirect(storage) => encode_value_sum(encoder, 4, storage),
            Self::DirectParts(storage, coercion) => encode_parts(encoder, 5, storage, coercion),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalScoopAbiFunctionSignature {
    signature: ExactCallableSignature,
    arguments: Vec<ScoopAbiArgument>,
    result: ScoopAbiReturn,
    gc_effect: GcEffect,
}

impl CanonicalScoopAbiFunctionSignature {
    pub fn new(
        signature: ExactCallableSignature,
        arguments: Vec<ScoopAbiArgument>,
        result: ScoopAbiReturn,
        gc_effect: GcEffect,
    ) -> Result<Self, ScoopAbiError> {
        let receiver = signature.receiver().into_option();
        let logical_argument_count = signature.parameters().len() + usize::from(receiver.is_some());
        if logical_argument_count != arguments.len() {
            return Err(ScoopAbiError::ArgumentCountMismatch);
        }
        for (exact_type, argument) in receiver
            .into_iter()
            .chain(signature.parameters().iter().copied())
            .zip(&arguments)
        {
            if exact_type != argument.storage().exact_type() {
                return Err(ScoopAbiError::ArgumentExactTypeMismatch);
            }
        }
        if let Some(storage) = result.storage()
            && signature.result() != storage.exact_type()
        {
            return Err(ScoopAbiError::ResultExactTypeMismatch);
        }
        Ok(Self {
            signature,
            arguments,
            result,
            gc_effect,
        })
    }

    pub fn signature(&self) -> &ExactCallableSignature {
        &self.signature
    }

    pub fn arguments(&self) -> &[ScoopAbiArgument] {
        &self.arguments
    }

    pub const fn result(&self) -> ScoopAbiReturn {
        self.result
    }

    pub const fn gc_effect(&self) -> GcEffect {
        self.gc_effect
    }
}

impl WireEncode for CanonicalScoopAbiFunctionSignature {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.signature.encode(encoder)?;
        encoder.field(2)?;
        encoder.array(self.arguments.len() as u64)?;
        for argument in &self.arguments {
            argument.encode(encoder)?;
        }
        encoder.field(3)?;
        self.result.encode(encoder)?;
        encoder.field(4)?;
        self.gc_effect.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScoopAbiError {
    ExpectedZeroSize,
    ExpectedNonZeroSize,
    PassingShapeMismatch,
    ArgumentCountMismatch,
    ArgumentExactTypeMismatch,
    ResultExactTypeMismatch,
}

impl fmt::Display for ScoopAbiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ExpectedZeroSize => "elided Scoop ABI storage must have zero byte size",
            Self::ExpectedNonZeroSize => "direct or indirect Scoop ABI storage must be non-zero",
            Self::PassingShapeMismatch => {
                "Scoop ABI passing convention does not match the target value shape"
            }
            Self::ArgumentCountMismatch => {
                "Scoop ABI physical argument count does not match its exact signature"
            }
            Self::ArgumentExactTypeMismatch => {
                "Scoop ABI argument exact type does not match its signature parameter"
            }
            Self::ResultExactTypeMismatch => {
                "Scoop ABI result exact type does not match its signature result"
            }
        })
    }
}

impl std::error::Error for ScoopAbiError {}

fn require_zero_size(storage: CanonicalScoopStorage) -> Result<(), ScoopAbiError> {
    if storage.byte_size() == 0 {
        Ok(())
    } else {
        Err(ScoopAbiError::ExpectedZeroSize)
    }
}

fn require_nonzero_shape(
    storage: CanonicalScoopStorage,
    expected: ScoopAbiValueShape,
) -> Result<(), ScoopAbiError> {
    if storage.byte_size() == 0 {
        return Err(ScoopAbiError::ExpectedNonZeroSize);
    }
    if storage.shape() != expected {
        return Err(ScoopAbiError::PassingShapeMismatch);
    }
    Ok(())
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

#[cfg(test)]
mod tests;

fn require_direct_parts(
    storage: CanonicalScoopStorage,
    coercion: AbiCoercion,
) -> Result<(), ScoopAbiError> {
    coercion
        .validate_storage(storage.byte_size(), storage.alignment().get())
        .map_err(|_| ScoopAbiError::PassingShapeMismatch)?;
    let valid = match storage.shape() {
        ScoopAbiValueShape::Interface => {
            storage.byte_size() == 16
                && storage.alignment().get() == 8
                && coercion == AbiCoercion::interface()
        }
        ScoopAbiValueShape::Aggregate => {
            storage.byte_size() <= 16
                && storage.alignment().get() <= 8
                && !coercion.has_managed_pointer()
        }
        ScoopAbiValueShape::Scalar => false,
    };
    if valid {
        Ok(())
    } else {
        Err(ScoopAbiError::PassingShapeMismatch)
    }
}

fn encode_parts(
    e: &mut Encoder,
    tag: u64,
    storage: &CanonicalScoopStorage,
    coercion: &AbiCoercion,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    e.map(3)?;
    encode_tag(e, tag)?;
    e.field(1)?;
    storage.encode(e)?;
    e.field(2)?;
    coercion.encode(e)
}
