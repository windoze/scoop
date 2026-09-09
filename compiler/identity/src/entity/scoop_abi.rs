use std::fmt;
use std::num::NonZeroU64;

use scoop_wire::{Encoder, WireEncodeV1};

use super::{ExactCallableSignatureV1, GcEffectV1};
use crate::PersistentExactTypeId;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ScoopAbiValueShapeV1 {
    Scalar,
    Aggregate,
}

impl WireEncodeV1 for ScoopAbiValueShapeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Scalar => 1,
            Self::Aggregate => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalScoopStorageV1 {
    exact_type: PersistentExactTypeId,
    byte_size: u64,
    alignment: NonZeroU64,
    shape: ScoopAbiValueShapeV1,
}

impl CanonicalScoopStorageV1 {
    pub const fn new(
        exact_type: PersistentExactTypeId,
        byte_size: u64,
        alignment: NonZeroU64,
        shape: ScoopAbiValueShapeV1,
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

    pub const fn shape(self) -> ScoopAbiValueShapeV1 {
        self.shape
    }
}

impl WireEncodeV1 for CanonicalScoopStorageV1 {
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
enum ScoopAbiArgumentKindV1 {
    ElidedZst,
    Direct,
    Indirect,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScoopAbiArgumentV1 {
    kind: ScoopAbiArgumentKindV1,
    storage: CanonicalScoopStorageV1,
}

impl ScoopAbiArgumentV1 {
    pub fn elided_zst(storage: CanonicalScoopStorageV1) -> Result<Self, ScoopAbiError> {
        require_zero_size(storage)?;
        Ok(Self {
            kind: ScoopAbiArgumentKindV1::ElidedZst,
            storage,
        })
    }

    pub fn direct(storage: CanonicalScoopStorageV1) -> Result<Self, ScoopAbiError> {
        require_nonzero_shape(storage, ScoopAbiValueShapeV1::Scalar)?;
        Ok(Self {
            kind: ScoopAbiArgumentKindV1::Direct,
            storage,
        })
    }

    pub fn indirect(storage: CanonicalScoopStorageV1) -> Result<Self, ScoopAbiError> {
        require_nonzero_shape(storage, ScoopAbiValueShapeV1::Aggregate)?;
        Ok(Self {
            kind: ScoopAbiArgumentKindV1::Indirect,
            storage,
        })
    }

    pub const fn storage(self) -> CanonicalScoopStorageV1 {
        self.storage
    }
}

impl WireEncodeV1 for ScoopAbiArgumentV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let tag = match self.kind {
            ScoopAbiArgumentKindV1::ElidedZst => 1,
            ScoopAbiArgumentKindV1::Direct => 2,
            ScoopAbiArgumentKindV1::Indirect => 3,
        };
        encode_value_sum(encoder, tag, &self.storage)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum ScoopAbiReturnKindV1 {
    UnitVoid,
    ElidedZst(CanonicalScoopStorageV1),
    Direct(CanonicalScoopStorageV1),
    Indirect(CanonicalScoopStorageV1),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScoopAbiReturnV1(ScoopAbiReturnKindV1);

impl ScoopAbiReturnV1 {
    pub const fn unit_void() -> Self {
        Self(ScoopAbiReturnKindV1::UnitVoid)
    }

    pub fn elided_zst(storage: CanonicalScoopStorageV1) -> Result<Self, ScoopAbiError> {
        require_zero_size(storage)?;
        Ok(Self(ScoopAbiReturnKindV1::ElidedZst(storage)))
    }

    pub fn direct(storage: CanonicalScoopStorageV1) -> Result<Self, ScoopAbiError> {
        require_nonzero_shape(storage, ScoopAbiValueShapeV1::Scalar)?;
        Ok(Self(ScoopAbiReturnKindV1::Direct(storage)))
    }

    pub fn indirect(storage: CanonicalScoopStorageV1) -> Result<Self, ScoopAbiError> {
        require_nonzero_shape(storage, ScoopAbiValueShapeV1::Aggregate)?;
        Ok(Self(ScoopAbiReturnKindV1::Indirect(storage)))
    }

    fn storage(self) -> Option<CanonicalScoopStorageV1> {
        match self.0 {
            ScoopAbiReturnKindV1::UnitVoid => None,
            ScoopAbiReturnKindV1::ElidedZst(storage)
            | ScoopAbiReturnKindV1::Direct(storage)
            | ScoopAbiReturnKindV1::Indirect(storage) => Some(storage),
        }
    }
}

impl WireEncodeV1 for ScoopAbiReturnV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self.0 {
            ScoopAbiReturnKindV1::UnitVoid => encode_empty_sum(encoder, 1),
            ScoopAbiReturnKindV1::ElidedZst(storage) => encode_value_sum(encoder, 2, &storage),
            ScoopAbiReturnKindV1::Direct(storage) => encode_value_sum(encoder, 3, &storage),
            ScoopAbiReturnKindV1::Indirect(storage) => encode_value_sum(encoder, 4, &storage),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalScoopAbiFunctionSignatureV1 {
    signature: ExactCallableSignatureV1,
    arguments: Vec<ScoopAbiArgumentV1>,
    result: ScoopAbiReturnV1,
    gc_effect: GcEffectV1,
}

impl CanonicalScoopAbiFunctionSignatureV1 {
    pub fn new(
        signature: ExactCallableSignatureV1,
        arguments: Vec<ScoopAbiArgumentV1>,
        result: ScoopAbiReturnV1,
        gc_effect: GcEffectV1,
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

    pub fn signature(&self) -> &ExactCallableSignatureV1 {
        &self.signature
    }

    pub const fn gc_effect(&self) -> GcEffectV1 {
        self.gc_effect
    }
}

impl WireEncodeV1 for CanonicalScoopAbiFunctionSignatureV1 {
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

fn require_zero_size(storage: CanonicalScoopStorageV1) -> Result<(), ScoopAbiError> {
    if storage.byte_size() == 0 {
        Ok(())
    } else {
        Err(ScoopAbiError::ExpectedZeroSize)
    }
}

fn require_nonzero_shape(
    storage: CanonicalScoopStorageV1,
    expected: ScoopAbiValueShapeV1,
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
    value: &impl WireEncodeV1,
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
        CanonicalScoopAbiFunctionSignatureV1, CanonicalScoopStorageV1, ScoopAbiArgumentV1,
        ScoopAbiError, ScoopAbiReturnV1, ScoopAbiValueShapeV1,
    };
    use crate::{
        ConeIdentity, EffectV1, ExactCallableSignatureV1, GcEffectV1, PersistentExactTypeId,
    };

    #[test]
    fn passing_constructors_reject_wrong_size_and_shape() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let zero = storage(exact, 0, ScoopAbiValueShapeV1::Aggregate);
        let scalar = storage(exact, 8, ScoopAbiValueShapeV1::Scalar);
        let aggregate = storage(exact, 8, ScoopAbiValueShapeV1::Aggregate);

        assert_eq!(
            ScoopAbiArgumentV1::direct(zero),
            Err(ScoopAbiError::ExpectedNonZeroSize)
        );
        assert_eq!(
            ScoopAbiArgumentV1::indirect(scalar),
            Err(ScoopAbiError::PassingShapeMismatch)
        );
        assert_eq!(
            ScoopAbiReturnV1::elided_zst(aggregate),
            Err(ScoopAbiError::ExpectedZeroSize)
        );
    }

    #[test]
    fn signature_rejects_exact_type_mismatch() {
        let parameter = PersistentExactTypeId(ConeIdentity::CORE.0);
        let result = PersistentExactTypeId(ConeIdentity::SINGLE_FILE.0);
        let signature =
            ExactCallableSignatureV1::new(EffectV1::Ordinary, None, vec![parameter], result);
        let argument =
            ScoopAbiArgumentV1::direct(storage(result, 8, ScoopAbiValueShapeV1::Scalar)).unwrap();
        let result =
            ScoopAbiReturnV1::direct(storage(result, 8, ScoopAbiValueShapeV1::Scalar)).unwrap();

        assert_eq!(
            CanonicalScoopAbiFunctionSignatureV1::new(
                signature,
                vec![argument],
                result,
                GcEffectV1::Managed,
            ),
            Err(ScoopAbiError::ArgumentExactTypeMismatch)
        );
    }

    #[test]
    fn canonical_scoop_signature_has_fixed_wire_vector() {
        let parameter = PersistentExactTypeId(ConeIdentity::CORE.0);
        let result_exact = PersistentExactTypeId(ConeIdentity::SINGLE_FILE.0);
        let signature =
            ExactCallableSignatureV1::new(EffectV1::Ordinary, None, vec![parameter], result_exact);
        let argument =
            ScoopAbiArgumentV1::direct(storage(parameter, 8, ScoopAbiValueShapeV1::Scalar))
                .unwrap();
        let result_abi =
            ScoopAbiReturnV1::direct(storage(result_exact, 8, ScoopAbiValueShapeV1::Scalar))
                .unwrap();
        let signature = CanonicalScoopAbiFunctionSignatureV1::new(
            signature,
            vec![argument],
            result_abi,
            GcEffectV1::NoGc,
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
        shape: ScoopAbiValueShapeV1,
    ) -> CanonicalScoopStorageV1 {
        CanonicalScoopStorageV1::new(exact_type, byte_size, NonZeroU64::new(8).unwrap(), shape)
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
