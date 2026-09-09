use std::fmt;
use std::num::NonZeroU64;

use scoop_wire::{Encoder, WireEncode};

use super::{ExactCallableSignature, GcEffect};
use crate::PersistentExactTypeId;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ScoopAbiValueShape {
    Scalar,
    Aggregate,
}

impl WireEncode for ScoopAbiValueShape {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Scalar => 1,
            Self::Aggregate => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalScoopStorage {
    exact_type: PersistentExactTypeId,
    byte_size: u64,
    alignment: NonZeroU64,
    shape: ScoopAbiValueShape,
}

impl CanonicalScoopStorage {
    pub const fn new(
        exact_type: PersistentExactTypeId,
        byte_size: u64,
        alignment: NonZeroU64,
        shape: ScoopAbiValueShape,
    ) -> Self {
        Self {
            exact_type,
            byte_size,
            alignment,
            shape,
        }
    }

    pub const fn exact_type(self) -> PersistentExactTypeId {
        self.exact_type
    }

    pub const fn byte_size(self) -> u64 {
        self.byte_size
    }

    pub const fn alignment(self) -> NonZeroU64 {
        self.alignment
    }

    pub const fn shape(self) -> ScoopAbiValueShape {
        self.shape
    }
}

impl WireEncode for CanonicalScoopStorage {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.exact_type.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(self.byte_size)?;
        encoder.field(3)?;
        encoder.unsigned(self.alignment.get())?;
        encoder.field(4)?;
        self.shape.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum ScoopAbiArgumentKind {
    ElidedZst,
    Direct,
    Indirect,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScoopAbiArgument {
    kind: ScoopAbiArgumentKind,
    storage: CanonicalScoopStorage,
}

impl ScoopAbiArgument {
    pub fn elided_zst(storage: CanonicalScoopStorage) -> Result<Self, ScoopAbiError> {
        require_zero_size(storage)?;
        Ok(Self {
            kind: ScoopAbiArgumentKind::ElidedZst,
            storage,
        })
    }

    pub fn direct(storage: CanonicalScoopStorage) -> Result<Self, ScoopAbiError> {
        require_nonzero_shape(storage, ScoopAbiValueShape::Scalar)?;
        Ok(Self {
            kind: ScoopAbiArgumentKind::Direct,
            storage,
        })
    }

    pub fn indirect(storage: CanonicalScoopStorage) -> Result<Self, ScoopAbiError> {
        require_nonzero_shape(storage, ScoopAbiValueShape::Aggregate)?;
        Ok(Self {
            kind: ScoopAbiArgumentKind::Indirect,
            storage,
        })
    }

    pub const fn storage(self) -> CanonicalScoopStorage {
        self.storage
    }
}

impl WireEncode for ScoopAbiArgument {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let tag = match self.kind {
            ScoopAbiArgumentKind::ElidedZst => 1,
            ScoopAbiArgumentKind::Direct => 2,
            ScoopAbiArgumentKind::Indirect => 3,
        };
        encode_value_sum(encoder, tag, &self.storage)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum ScoopAbiReturnKind {
    UnitVoid,
    ElidedZst(CanonicalScoopStorage),
    Direct(CanonicalScoopStorage),
    Indirect(CanonicalScoopStorage),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScoopAbiReturn(ScoopAbiReturnKind);

impl ScoopAbiReturn {
    pub const fn unit_void() -> Self {
        Self(ScoopAbiReturnKind::UnitVoid)
    }

    pub fn elided_zst(storage: CanonicalScoopStorage) -> Result<Self, ScoopAbiError> {
        require_zero_size(storage)?;
        Ok(Self(ScoopAbiReturnKind::ElidedZst(storage)))
    }

    pub fn direct(storage: CanonicalScoopStorage) -> Result<Self, ScoopAbiError> {
        require_nonzero_shape(storage, ScoopAbiValueShape::Scalar)?;
        Ok(Self(ScoopAbiReturnKind::Direct(storage)))
    }

    pub fn indirect(storage: CanonicalScoopStorage) -> Result<Self, ScoopAbiError> {
        require_nonzero_shape(storage, ScoopAbiValueShape::Aggregate)?;
        Ok(Self(ScoopAbiReturnKind::Indirect(storage)))
    }

    fn storage(self) -> Option<CanonicalScoopStorage> {
        match self.0 {
            ScoopAbiReturnKind::UnitVoid => None,
            ScoopAbiReturnKind::ElidedZst(storage)
            | ScoopAbiReturnKind::Direct(storage)
            | ScoopAbiReturnKind::Indirect(storage) => Some(storage),
        }
    }
}

impl WireEncode for ScoopAbiReturn {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self.0 {
            ScoopAbiReturnKind::UnitVoid => encode_empty_sum(encoder, 1),
            ScoopAbiReturnKind::ElidedZst(storage) => encode_value_sum(encoder, 2, &storage),
            ScoopAbiReturnKind::Direct(storage) => encode_value_sum(encoder, 3, &storage),
            ScoopAbiReturnKind::Indirect(storage) => encode_value_sum(encoder, 4, &storage),
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
        if signature.receiver().is_present() {
            return Err(ScoopAbiError::ReceiverPresent);
        }
        if signature.parameters().len() != arguments.len() {
            return Err(ScoopAbiError::ArgumentCountMismatch);
        }
        for (parameter, argument) in signature.parameters().iter().zip(&arguments) {
            if *parameter != argument.storage().exact_type() {
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
    ReceiverPresent,
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
            Self::ReceiverPresent => "Scoop ABI extern signature must not contain a receiver",
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
mod tests {
    use std::num::NonZeroU64;

    use scoop_wire::encode;

    use super::{
        CanonicalScoopAbiFunctionSignature, CanonicalScoopStorage, ScoopAbiArgument, ScoopAbiError,
        ScoopAbiReturn, ScoopAbiValueShape,
    };
    use crate::{ConeIdentity, Effect, ExactCallableSignature, GcEffect, PersistentExactTypeId};

    #[test]
    fn passing_constructors_reject_wrong_size_and_shape() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let zero = storage(exact, 0, ScoopAbiValueShape::Aggregate);
        let scalar = storage(exact, 8, ScoopAbiValueShape::Scalar);
        let aggregate = storage(exact, 8, ScoopAbiValueShape::Aggregate);

        assert_eq!(
            ScoopAbiArgument::direct(zero),
            Err(ScoopAbiError::ExpectedNonZeroSize)
        );
        assert_eq!(
            ScoopAbiArgument::indirect(scalar),
            Err(ScoopAbiError::PassingShapeMismatch)
        );
        assert_eq!(
            ScoopAbiReturn::elided_zst(aggregate),
            Err(ScoopAbiError::ExpectedZeroSize)
        );
    }

    #[test]
    fn signature_rejects_exact_type_mismatch() {
        let parameter = PersistentExactTypeId(ConeIdentity::CORE.0);
        let result = PersistentExactTypeId(ConeIdentity::SINGLE_FILE.0);
        let signature =
            ExactCallableSignature::new(Effect::Ordinary, None, vec![parameter], result);
        let argument =
            ScoopAbiArgument::direct(storage(result, 8, ScoopAbiValueShape::Scalar)).unwrap();
        let result =
            ScoopAbiReturn::direct(storage(result, 8, ScoopAbiValueShape::Scalar)).unwrap();

        assert_eq!(
            CanonicalScoopAbiFunctionSignature::new(
                signature,
                vec![argument],
                result,
                GcEffect::Managed,
            ),
            Err(ScoopAbiError::ArgumentExactTypeMismatch)
        );
    }

    #[test]
    fn canonical_scoop_signature_has_fixed_wire_vector() {
        let parameter = PersistentExactTypeId(ConeIdentity::CORE.0);
        let result_exact = PersistentExactTypeId(ConeIdentity::SINGLE_FILE.0);
        let signature =
            ExactCallableSignature::new(Effect::Ordinary, None, vec![parameter], result_exact);
        let argument =
            ScoopAbiArgument::direct(storage(parameter, 8, ScoopAbiValueShape::Scalar)).unwrap();
        let result_abi =
            ScoopAbiReturn::direct(storage(result_exact, 8, ScoopAbiValueShape::Scalar)).unwrap();
        let signature = CanonicalScoopAbiFunctionSignature::new(
            signature,
            vec![argument],
            result_abi,
            GcEffect::NoGc,
        )
        .unwrap();

        assert_eq!(
            hex(&encode(&signature).unwrap()),
            format!(
                "a401a4010102a1000103815820{parameter}045820{result_exact}0281a2000201a4015820{parameter}02080308040103a2000301a4015820{result_exact}0208030804010402"
            )
        );
    }

    fn storage(
        exact_type: PersistentExactTypeId,
        byte_size: u64,
        shape: ScoopAbiValueShape,
    ) -> CanonicalScoopStorage {
        CanonicalScoopStorage::new(exact_type, byte_size, NonZeroU64::new(8).unwrap(), shape)
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
