use scoop_wire::{Encoder, HashError, WireEncode};

use super::CallbackParameterIndex;
use crate::ids::derive_persistent_id;
use crate::{
    CanonicalCAbiLayoutFingerprint, CanonicalCAbiSignatureFingerprint, ConeIdentity,
    GeneratedBridgeAtomId, GeneratedBridgeUnitId, NativeExternalContractFingerprint,
};

mod decode;

pub use decode::{
    DecodedGeneratedBridgeAtomKey, DecodedGeneratedBridgeAtomRoleKey,
    DecodedGeneratedBridgeSemanticTarget, DecodedGeneratedBridgeUnitKey,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneratedBridgeUnitKey {
    OutboundFunction(NativeExternalContractFingerprint),
    GlobalRead(NativeExternalContractFingerprint),
    GlobalWrite(NativeExternalContractFingerprint),
    GlobalAddress(NativeExternalContractFingerprint),
    CallbackTrampoline {
        signature: CanonicalCAbiSignatureFingerprint,
        context_index: CallbackParameterIndex,
    },
}

impl WireEncode for GeneratedBridgeUnitKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::OutboundFunction(contract) => encode_value_sum(encoder, 1, contract),
            Self::GlobalRead(contract) => encode_value_sum(encoder, 2, contract),
            Self::GlobalWrite(contract) => encode_value_sum(encoder, 3, contract),
            Self::GlobalAddress(contract) => encode_value_sum(encoder, 4, contract),
            Self::CallbackTrampoline {
                signature,
                context_index,
            } => {
                encoder.map(3)?;
                encode_tag(encoder, 5)?;
                encoder.field(1)?;
                signature.encode(encoder)?;
                encoder.field(2)?;
                context_index.encode(encoder)
            }
        }
    }
}

impl GeneratedBridgeUnitId {
    pub fn from_key(key: &GeneratedBridgeUnitKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-generated-bridge-unit-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneratedBridgeAtomRoleKey {
    PrimaryEntry {
        unit: GeneratedBridgeUnitId,
    },
    SignatureDescriptor {
        unit: GeneratedBridgeUnitId,
        signature: CanonicalCAbiSignatureFingerprint,
    },
    ContextDescriptor {
        unit: GeneratedBridgeUnitId,
        context_index: CallbackParameterIndex,
    },
    StaticAssertSupport {
        unit: GeneratedBridgeUnitId,
        layout: CanonicalCAbiLayoutFingerprint,
    },
}

impl GeneratedBridgeAtomRoleKey {
    pub const fn unit(self) -> GeneratedBridgeUnitId {
        match self {
            Self::PrimaryEntry { unit }
            | Self::SignatureDescriptor { unit, .. }
            | Self::ContextDescriptor { unit, .. }
            | Self::StaticAssertSupport { unit, .. } => unit,
        }
    }

    pub const fn is_materializable(self) -> bool {
        !matches!(self, Self::StaticAssertSupport { .. })
    }
}

impl WireEncode for GeneratedBridgeAtomRoleKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::PrimaryEntry { unit } => encode_value_sum(encoder, 1, unit),
            Self::SignatureDescriptor { unit, signature } => {
                encode_two_value_sum(encoder, 2, unit, signature)
            }
            Self::ContextDescriptor {
                unit,
                context_index,
            } => encode_two_value_sum(encoder, 3, unit, context_index),
            Self::StaticAssertSupport { unit, layout } => {
                encode_two_value_sum(encoder, 4, unit, layout)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GeneratedBridgeAtomKey {
    producer: ConeIdentity,
    atom: GeneratedBridgeAtomRoleKey,
}

impl GeneratedBridgeAtomKey {
    pub const fn new(producer: ConeIdentity, atom: GeneratedBridgeAtomRoleKey) -> Self {
        Self { producer, atom }
    }

    pub const fn producer(self) -> ConeIdentity {
        self.producer
    }

    pub const fn atom(self) -> GeneratedBridgeAtomRoleKey {
        self.atom
    }
}

impl WireEncode for GeneratedBridgeAtomKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.producer.encode(encoder)?;
        encoder.field(2)?;
        self.atom.encode(encoder)
    }
}

impl GeneratedBridgeAtomId {
    pub fn from_key(key: &GeneratedBridgeAtomKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-generated-bridge-atom-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GeneratedBridgeSemanticTarget {
    unit: GeneratedBridgeUnitId,
}

impl GeneratedBridgeSemanticTarget {
    pub const fn new(unit: GeneratedBridgeUnitId) -> Self {
        Self { unit }
    }

    pub const fn unit(self) -> GeneratedBridgeUnitId {
        self.unit
    }
}

impl WireEncode for GeneratedBridgeSemanticTarget {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(1)?;
        self.unit.encode(encoder)
    }
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
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

fn encode_two_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    first: &impl WireEncode,
    second: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    first.encode(encoder)?;
    encoder.field(2)?;
    second.encode(encoder)
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::{
        GeneratedBridgeAtomKey, GeneratedBridgeAtomRoleKey, GeneratedBridgeSemanticTarget,
        GeneratedBridgeUnitKey,
    };
    use crate::{
        CallbackParameterIndex, CanonicalCAbiLayoutFingerprint, CanonicalCAbiSignatureFingerprint,
        ConeIdentity, GeneratedBridgeAtomId, GeneratedBridgeUnitId,
        NativeExternalContractFingerprint,
    };

    #[test]
    fn bridge_unit_kind_is_part_of_the_identity() {
        let contract = NativeExternalContractFingerprint(ConeIdentity::CORE.0);
        let function =
            GeneratedBridgeUnitId::from_key(&GeneratedBridgeUnitKey::OutboundFunction(contract))
                .unwrap();
        let read =
            GeneratedBridgeUnitId::from_key(&GeneratedBridgeUnitKey::GlobalRead(contract)).unwrap();

        assert_ne!(function, read);
        assert_eq!(
            function.to_string(),
            "d199d64f271d9c4038341cea0f1985d5751f6f7128652b9f6b76e81d7f17410e"
        );
    }

    #[test]
    fn callback_unit_keeps_signature_and_context_index() {
        let signature = CanonicalCAbiSignatureFingerprint(ConeIdentity::CORE.0);
        let key = GeneratedBridgeUnitKey::CallbackTrampoline {
            signature,
            context_index: CallbackParameterIndex::new(3),
        };
        assert_eq!(
            hex(&encode(&key).unwrap()),
            format!("a30005015820{signature}0203")
        );
    }

    #[test]
    fn bridge_atom_is_producer_specific_but_semantic_target_is_not() {
        let unit = GeneratedBridgeUnitId(ConeIdentity::CORE.0);
        let role = GeneratedBridgeAtomRoleKey::PrimaryEntry { unit };
        let first =
            GeneratedBridgeAtomId::from_key(&GeneratedBridgeAtomKey::new(ConeIdentity::CORE, role))
                .unwrap();
        let second = GeneratedBridgeAtomId::from_key(&GeneratedBridgeAtomKey::new(
            ConeIdentity::SINGLE_FILE,
            role,
        ))
        .unwrap();

        assert_ne!(first, second);
        assert_eq!(
            first.to_string(),
            "e818924c690d54ff339da47dd03d991c84702e854c168fc9b77f46438daf7b70"
        );
        assert_eq!(GeneratedBridgeSemanticTarget::new(unit).unit(), unit);
        assert!(role.is_materializable());
        assert!(
            !GeneratedBridgeAtomRoleKey::StaticAssertSupport {
                unit,
                layout: CanonicalCAbiLayoutFingerprint(ConeIdentity::CORE.0),
            }
            .is_materializable()
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
