use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncodeV1};

use super::{CallableMaterializationV1, ExactCallableSignatureV1, StructuralDefinitionPathV1};
use crate::ids::derive_persistent_id;
use crate::{PersistentExactTypeId, PersistentTypeId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ClosureEnvironmentRoleV1 {
    Lambda,
    AnonymousFunction,
    CallableReference,
}

impl WireEncodeV1 for ClosureEnvironmentRoleV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Lambda => 1,
            Self::AnonymousFunction => 2,
            Self::CallableReference => 3,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableAdapterEnvironmentKeyV1 {
    Static {
        source: ExactCallableSignatureV1,
        target: ExactCallableSignatureV1,
    },
    Dynamic {
        target: ExactCallableSignatureV1,
    },
}

impl WireEncodeV1 for CallableAdapterEnvironmentKeyV1 {
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
pub enum GeneratedNominalKeyV1 {
    ClosureEnvironment {
        callable: CallableMaterializationV1,
        role: ClosureEnvironmentRoleV1,
    },
    CallableAdapterEnvironment {
        key: CallableAdapterEnvironmentKeyV1,
    },
    CoroutineFrame {
        source_callable: CallableMaterializationV1,
    },
    ContinuationAdapterEnvironment {
        source_callable: CallableMaterializationV1,
        suspension_site: StructuralDefinitionPathV1,
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

impl WireEncodeV1 for GeneratedNominalKeyV1 {
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
        key: &GeneratedNominalKeyV1,
    ) -> Result<Self, GeneratedNominalIdentityError> {
        validate_key(key)?;
        derive_persistent_id("scoop-type-id-v1", &GeneratedTypeIdentityKeyV1(key))
            .map_err(Into::into)
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

fn validate_key(key: &GeneratedNominalKeyV1) -> Result<(), GeneratedNominalIdentityError> {
    let target = match key {
        GeneratedNominalKeyV1::CallableAdapterEnvironment {
            key:
                CallableAdapterEnvironmentKeyV1::Static { target, .. }
                | CallableAdapterEnvironmentKeyV1::Dynamic { target },
        } => Some(target),
        _ => None,
    };
    if target.is_some_and(|target| target.receiver().is_present()) {
        Err(GeneratedNominalIdentityError::TargetReceiverMustBeAbsent)
    } else {
        Ok(())
    }
}

struct GeneratedTypeIdentityKeyV1<'key>(&'key GeneratedNominalKeyV1);

impl WireEncodeV1 for GeneratedTypeIdentityKeyV1<'_> {
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
    value: &impl WireEncodeV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::GeneratedNominalKeyV1;
    use crate::{
        CallableMaterializationContextV1, CallableMaterializationV1, CallableTemplateOwnerV1,
        ConeIdentity, PersistentExactTypeId, PersistentFunctionId, PersistentTypeId,
    };

    #[test]
    fn generated_nominal_uses_the_type_domain_with_generated_tag() {
        let payload = PersistentExactTypeId(ConeIdentity::CORE.0);
        let key = GeneratedNominalKeyV1::BoxedValue { payload };
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
        let target = crate::ExactCallableSignatureV1::new(
            crate::EffectV1::Ordinary,
            Some(exact),
            Vec::new(),
            exact,
        );
        let key = GeneratedNominalKeyV1::CallableAdapterEnvironment {
            key: super::CallableAdapterEnvironmentKeyV1::Dynamic { target },
        };
        assert_eq!(
            PersistentTypeId::from_generated_key(&key),
            Err(super::GeneratedNominalIdentityError::TargetReceiverMustBeAbsent)
        );
    }

    #[test]
    fn closure_environment_keeps_materialization_context() {
        let function = PersistentFunctionId(ConeIdentity::CORE.0);
        let key = GeneratedNominalKeyV1::ClosureEnvironment {
            callable: CallableMaterializationV1::new(
                CallableTemplateOwnerV1::Function(function),
                CallableMaterializationContextV1::NoSubstitution,
            ),
            role: super::ClosureEnvironmentRoleV1::Lambda,
        };
        let encoded = encode(&key).unwrap();
        assert_eq!(&encoded[..3], b"\xa3\x00\x01");
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
