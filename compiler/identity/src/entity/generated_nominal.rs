use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncode};

use super::{CallableMaterialization, ExactCallableSignature, StructuralDefinitionPath};
use crate::ids::derive_persistent_id;
use crate::{PersistentExactTypeId, PersistentTypeId};

mod decode;

pub use decode::{
    DecodedCallableAdapterEnvironmentKey, DecodedGeneratedNominalKey,
    GeneratedNominalResolutionError,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ClosureEnvironmentRole {
    Lambda,
    AnonymousFunction,
    CallableReference,
}

impl WireEncode for ClosureEnvironmentRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Lambda => 1,
            Self::AnonymousFunction => 2,
            Self::CallableReference => 3,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableAdapterEnvironmentKey {
    Static {
        source: ExactCallableSignature,
        target: ExactCallableSignature,
    },
    Dynamic {
        target: ExactCallableSignature,
    },
}

impl WireEncode for CallableAdapterEnvironmentKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Static { source, target } => {
                encoder.map(3)?;
                encode_tag(encoder, 1)?;
                encoder.field(1)?;
                source.encode(encoder)?;
                encoder.field(2)?;
                target.encode(encoder)
            }
            Self::Dynamic { target } => {
                encoder.map(2)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                target.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneratedNominalKey {
    ClosureEnvironment {
        callable: CallableMaterialization,
        role: ClosureEnvironmentRole,
    },
    CallableAdapterEnvironment {
        key: CallableAdapterEnvironmentKey,
    },
    CoroutineFrame {
        source_callable: CallableMaterialization,
    },
    ContinuationAdapterEnvironment {
        source_callable: CallableMaterialization,
        suspension_site: StructuralDefinitionPath,
    },
    CoroutineStep {
        result: PersistentExactTypeId,
    },
    BoxedValue {
        payload: PersistentExactTypeId,
    },
    CoroutineSlot {
        value: PersistentExactTypeId,
    },
    ObjectBackingClass {
        object: PersistentTypeId,
    },
}

impl WireEncode for GeneratedNominalKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ClosureEnvironment { callable, role } => {
                encoder.map(3)?;
                encode_tag(encoder, 1)?;
                encoder.field(1)?;
                callable.encode(encoder)?;
                encoder.field(2)?;
                role.encode(encoder)
            }
            Self::CallableAdapterEnvironment { key } => encode_value_sum(encoder, 2, key),
            Self::CoroutineFrame { source_callable } => {
                encode_value_sum(encoder, 3, source_callable)
            }
            Self::ContinuationAdapterEnvironment {
                source_callable,
                suspension_site,
            } => {
                encoder.map(3)?;
                encode_tag(encoder, 4)?;
                encoder.field(1)?;
                source_callable.encode(encoder)?;
                encoder.field(2)?;
                suspension_site.encode(encoder)
            }
            Self::CoroutineStep { result } => encode_value_sum(encoder, 5, result),
            Self::BoxedValue { payload } => encode_value_sum(encoder, 6, payload),
            Self::CoroutineSlot { value } => encode_value_sum(encoder, 7, value),
            Self::ObjectBackingClass { object } => encode_value_sum(encoder, 8, object),
        }
    }
}

impl PersistentTypeId {
    pub fn from_generated_key(
        key: &GeneratedNominalKey,
    ) -> Result<Self, GeneratedNominalIdentityError> {
        validate_key(key)?;
        derive_persistent_id("scoop-type-id-v1", &GeneratedTypeIdentityKey(key)).map_err(Into::into)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneratedNominalIdentityError {
    TargetReceiverMustBeAbsent,
    Hash(HashError),
}

impl fmt::Display for GeneratedNominalIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetReceiverMustBeAbsent => {
                formatter.write_str("generated function-shape target must not have a receiver")
            }
            Self::Hash(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedNominalIdentityError {}

impl From<HashError> for GeneratedNominalIdentityError {
    fn from(error: HashError) -> Self {
        Self::Hash(error)
    }
}

fn validate_key(key: &GeneratedNominalKey) -> Result<(), GeneratedNominalIdentityError> {
    let target = match key {
        GeneratedNominalKey::CallableAdapterEnvironment {
            key:
                CallableAdapterEnvironmentKey::Static { target, .. }
                | CallableAdapterEnvironmentKey::Dynamic { target },
        } => Some(target),
        _ => None,
    };
    if target.is_some_and(|target| target.receiver().is_present()) {
        Err(GeneratedNominalIdentityError::TargetReceiverMustBeAbsent)
    } else {
        Ok(())
    }
}

struct GeneratedTypeIdentityKey<'key>(&'key GeneratedNominalKey);

impl WireEncode for GeneratedTypeIdentityKey<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encode_tag(encoder, 2)?;
        encoder.field(1)?;
        self.0.encode(encoder)
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

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::GeneratedNominalKey;
    use crate::{
        CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
        ConeIdentity, PersistentExactTypeId, PersistentFunctionId, PersistentTypeId,
    };

    #[test]
    fn generated_nominal_uses_the_type_domain_with_generated_tag() {
        let payload = PersistentExactTypeId(ConeIdentity::CORE.0);
        let key = GeneratedNominalKey::BoxedValue { payload };
        assert_eq!(
            hex(&encode(&key).unwrap()),
            format!("a20006015820{payload}")
        );
        assert_eq!(
            PersistentTypeId::from_generated_key(&key)
                .unwrap()
                .to_string(),
            "b50d260f83c9bf2d7b8b678a332f1092aa8424a5c1bfd24f047f5e93e4ee8f03"
        );
    }

    #[test]
    fn adapter_environment_rejects_target_receiver() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let target = crate::ExactCallableSignature::new(
            crate::Effect::Ordinary,
            Some(exact),
            Vec::new(),
            exact,
        );
        let key = GeneratedNominalKey::CallableAdapterEnvironment {
            key: super::CallableAdapterEnvironmentKey::Dynamic { target },
        };
        assert_eq!(
            PersistentTypeId::from_generated_key(&key),
            Err(super::GeneratedNominalIdentityError::TargetReceiverMustBeAbsent)
        );
    }

    #[test]
    fn closure_environment_keeps_materialization_context() {
        let function = PersistentFunctionId(ConeIdentity::CORE.0);
        let key = GeneratedNominalKey::ClosureEnvironment {
            callable: CallableMaterialization::new(
                CallableTemplateOwner::Function(function),
                CallableMaterializationContext::NoSubstitution,
            ),
            role: super::ClosureEnvironmentRole::Lambda,
        };
        let encoded = encode(&key).unwrap();
        assert_eq!(&encoded[..3], b"\xa3\x00\x01");
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
