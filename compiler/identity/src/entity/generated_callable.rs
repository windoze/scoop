use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncodeV1};

use super::{CallableMaterializationV1, ExactCallableSignatureV1, StructuralDefinitionPathV1};
use crate::ids::derive_persistent_id;
use crate::{
    PersistentCallbackApplicationId, PersistentConstructorId, PersistentDispatchSlotId,
    PersistentExactTypeId, PersistentFunctionId, PersistentGeneratedCallableId,
    PersistentGenericFunctionId, PersistentInitializationUnitId, PersistentPropertyAccessorId,
    PersistentTypeId,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LexicalCallableRoleV1 {
    LambdaBody,
    AnonymousFunctionBody,
}

impl WireEncodeV1 for LexicalCallableRoleV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::LambdaBody => 1,
            Self::AnonymousFunctionBody => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum InitializationCallableRoleV1 {
    Initializer,
    Ensure,
}

impl WireEncodeV1 for InitializationCallableRoleV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Initializer => 1,
            Self::Ensure => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ContinuationShellRoleV1 {
    Success,
    Failure,
}

impl WireEncodeV1 for ContinuationShellRoleV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Success => 1,
            Self::Failure => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CoroutineAdapterRoleV1 {
    Success,
    Failure,
}

impl WireEncodeV1 for CoroutineAdapterRoleV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Success => 1,
            Self::Failure => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LexicalCallableParentV1(LexicalCallableParentKindV1);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum LexicalCallableParentKindV1 {
    Function(PersistentFunctionId),
    GenericFunction(PersistentGenericFunctionId),
    Constructor(PersistentConstructorId),
    Accessor(PersistentPropertyAccessorId),
    Generated(PersistentGeneratedCallableId),
}

impl LexicalCallableParentV1 {
    pub const fn function(id: PersistentFunctionId) -> Self {
        Self(LexicalCallableParentKindV1::Function(id))
    }

    pub const fn generic_function(id: PersistentGenericFunctionId) -> Self {
        Self(LexicalCallableParentKindV1::GenericFunction(id))
    }

    pub const fn constructor(id: PersistentConstructorId) -> Self {
        Self(LexicalCallableParentKindV1::Constructor(id))
    }

    pub const fn accessor(id: PersistentPropertyAccessorId) -> Self {
        Self(LexicalCallableParentKindV1::Accessor(id))
    }

    pub fn from_generated_key(key: &GeneratedCallableKeyV1) -> Result<Self, LexicalParentError> {
        if !key.can_be_lexical_parent() {
            return Err(LexicalParentError::GeneratedRoleNotLexical);
        }
        PersistentGeneratedCallableId::from_key(key)
            .map(|id| Self(LexicalCallableParentKindV1::Generated(id)))
            .map_err(LexicalParentError::Identity)
    }
}

impl WireEncodeV1 for LexicalCallableParentV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self.0 {
            LexicalCallableParentKindV1::Function(id) => encode_value_sum(encoder, 1, &id),
            LexicalCallableParentKindV1::GenericFunction(id) => encode_value_sum(encoder, 2, &id),
            LexicalCallableParentKindV1::Constructor(id) => encode_value_sum(encoder, 3, &id),
            LexicalCallableParentKindV1::Accessor(id) => encode_value_sum(encoder, 4, &id),
            LexicalCallableParentKindV1::Generated(id) => encode_value_sum(encoder, 5, &id),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneratedCallableKeyV1 {
    Lexical {
        parent: LexicalCallableParentV1,
        role: LexicalCallableRoleV1,
        path: StructuralDefinitionPathV1,
    },
    Initialization {
        unit: PersistentInitializationUnitId,
        role: InitializationCallableRoleV1,
    },
    DerivedEquality {
        exact_owner: PersistentExactTypeId,
    },
    FunctionAdapter {
        source: ExactCallableSignatureV1,
        target: ExactCallableSignatureV1,
    },
    DynamicFunctionAdapter {
        target: ExactCallableSignatureV1,
    },
    CallableReferenceInvoke {
        parent: LexicalCallableParentV1,
        path: StructuralDefinitionPathV1,
    },
    StaticNoGcCallbackStorageBridge {
        source: CallableMaterializationV1,
        signature: ExactCallableSignatureV1,
    },
    ForeignCallbackManagedAdapter {
        application: PersistentCallbackApplicationId,
    },
    CoroutineDriver {
        source_callable: CallableMaterializationV1,
    },
    ContinuationShell {
        result: PersistentExactTypeId,
        role: ContinuationShellRoleV1,
    },
    CoroutineStart {
        result: PersistentExactTypeId,
    },
    CoroutineAdapter {
        source_callable: CallableMaterializationV1,
        suspension_site: StructuralDefinitionPathV1,
        role: CoroutineAdapterRoleV1,
    },
    FunctionBridge {
        environment: PersistentTypeId,
        target: ExactCallableSignatureV1,
    },
    DispatchAdjust {
        slot: PersistentDispatchSlotId,
        implementor: PersistentExactTypeId,
        target: CallableMaterializationV1,
    },
    BoxingAdjust {
        slot: PersistentDispatchSlotId,
        payload: PersistentExactTypeId,
        interface: PersistentExactTypeId,
    },
}

impl GeneratedCallableKeyV1 {
    fn can_be_lexical_parent(&self) -> bool {
        matches!(
            self,
            Self::Lexical { .. }
                | Self::Initialization { .. }
                | Self::CallableReferenceInvoke { .. }
        )
    }

    fn validate(&self) -> Result<(), GeneratedCallableIdentityError> {
        match self {
            Self::FunctionAdapter { target, .. }
            | Self::DynamicFunctionAdapter { target }
            | Self::FunctionBridge { target, .. }
                if target.receiver().is_present() =>
            {
                Err(GeneratedCallableIdentityError::TargetReceiverMustBeAbsent)
            }
            _ => Ok(()),
        }
    }
}

impl WireEncodeV1 for GeneratedCallableKeyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Lexical { parent, role, path } => {
                encoder.map(4)?;
                encode_tag(encoder, 1)?;
                encoder.field(1)?;
                parent.encode(encoder)?;
                encoder.field(2)?;
                role.encode(encoder)?;
                encoder.field(3)?;
                path.encode(encoder)
            }
            Self::Initialization { unit, role } => {
                encoder.map(3)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                unit.encode(encoder)?;
                encoder.field(2)?;
                role.encode(encoder)
            }
            Self::DerivedEquality { exact_owner } => encode_value_sum(encoder, 3, exact_owner),
            Self::FunctionAdapter { source, target } => {
                encoder.map(3)?;
                encode_tag(encoder, 4)?;
                encoder.field(1)?;
                source.encode(encoder)?;
                encoder.field(2)?;
                target.encode(encoder)
            }
            Self::DynamicFunctionAdapter { target } => encode_value_sum(encoder, 5, target),
            Self::CallableReferenceInvoke { parent, path } => {
                encoder.map(3)?;
                encode_tag(encoder, 6)?;
                encoder.field(1)?;
                parent.encode(encoder)?;
                encoder.field(2)?;
                path.encode(encoder)
            }
            Self::StaticNoGcCallbackStorageBridge { source, signature } => {
                encoder.map(3)?;
                encode_tag(encoder, 7)?;
                encoder.field(1)?;
                source.encode(encoder)?;
                encoder.field(2)?;
                signature.encode(encoder)
            }
            Self::ForeignCallbackManagedAdapter { application } => {
                encode_value_sum(encoder, 8, application)
            }
            Self::CoroutineDriver { source_callable } => {
                encode_value_sum(encoder, 9, source_callable)
            }
            Self::ContinuationShell { result, role } => {
                encoder.map(3)?;
                encode_tag(encoder, 10)?;
                encoder.field(1)?;
                result.encode(encoder)?;
                encoder.field(2)?;
                role.encode(encoder)
            }
            Self::CoroutineStart { result } => encode_value_sum(encoder, 11, result),
            Self::CoroutineAdapter {
                source_callable,
                suspension_site,
                role,
            } => {
                encoder.map(4)?;
                encode_tag(encoder, 12)?;
                encoder.field(1)?;
                source_callable.encode(encoder)?;
                encoder.field(2)?;
                suspension_site.encode(encoder)?;
                encoder.field(3)?;
                role.encode(encoder)
            }
            Self::FunctionBridge {
                environment,
                target,
            } => {
                encoder.map(3)?;
                encode_tag(encoder, 13)?;
                encoder.field(1)?;
                environment.encode(encoder)?;
                encoder.field(2)?;
                target.encode(encoder)
            }
            Self::DispatchAdjust {
                slot,
                implementor,
                target,
            } => {
                encoder.map(4)?;
                encode_tag(encoder, 14)?;
                encoder.field(1)?;
                slot.encode(encoder)?;
                encoder.field(2)?;
                implementor.encode(encoder)?;
                encoder.field(3)?;
                target.encode(encoder)
            }
            Self::BoxingAdjust {
                slot,
                payload,
                interface,
            } => {
                encoder.map(4)?;
                encode_tag(encoder, 15)?;
                encoder.field(1)?;
                slot.encode(encoder)?;
                encoder.field(2)?;
                payload.encode(encoder)?;
                encoder.field(3)?;
                interface.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneratedCallableIdentityError {
    TargetReceiverMustBeAbsent,
    Hash(HashError),
}

impl fmt::Display for GeneratedCallableIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetReceiverMustBeAbsent => {
                formatter.write_str("generated function-shape target must not have a receiver")
            }
            Self::Hash(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedCallableIdentityError {}

impl From<HashError> for GeneratedCallableIdentityError {
    fn from(error: HashError) -> Self {
        Self::Hash(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LexicalParentError {
    GeneratedRoleNotLexical,
    Identity(GeneratedCallableIdentityError),
}

impl fmt::Display for LexicalParentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GeneratedRoleNotLexical => formatter.write_str(
                "generated lexical parent must be lexical, initialization, or callable-reference invoke",
            ),
            Self::Identity(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for LexicalParentError {}

impl PersistentGeneratedCallableId {
    pub fn from_key(key: &GeneratedCallableKeyV1) -> Result<Self, GeneratedCallableIdentityError> {
        key.validate()?;
        derive_persistent_id("scoop-generated-callable-id-v1", key).map_err(Into::into)
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

    use super::{
        ContinuationShellRoleV1, CoroutineAdapterRoleV1, GeneratedCallableIdentityError,
        GeneratedCallableKeyV1, InitializationCallableRoleV1, LexicalCallableParentV1,
        LexicalCallableRoleV1, LexicalParentError,
    };
    use crate::{
        CallableMaterializationContextV1, CallableMaterializationV1, CallableTemplateOwnerV1,
        ConeIdentity, EffectV1, ExactCallableSignatureV1, PersistentCallbackApplicationId,
        PersistentDispatchSlotId, PersistentExactTypeId, PersistentFunctionId,
        PersistentGeneratedCallableId, PersistentInitializationUnitId, PersistentTypeId,
        StructuralDefinitionPathV1, StructuralDefinitionSiteRoleV1, StructuralPathSegmentV1,
    };

    #[test]
    fn generated_callable_has_fixed_identity() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let target = ExactCallableSignatureV1::new(EffectV1::Ordinary, None, Vec::new(), exact);
        let key = GeneratedCallableKeyV1::DynamicFunctionAdapter { target };
        assert_eq!(
            hex(&encode(&key).unwrap()),
            format!("a2000501a4010102a100010380045820{exact}")
        );
        assert_eq!(
            PersistentGeneratedCallableId::from_key(&key)
                .unwrap()
                .to_string(),
            "c57a69c85a9103908286eafb4e06ae42968689e0afab3ffeb0f49c138d9f7b69"
        );
    }

    #[test]
    fn function_shape_target_rejects_receiver() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let target =
            ExactCallableSignatureV1::new(EffectV1::Ordinary, Some(exact), Vec::new(), exact);
        assert_eq!(
            PersistentGeneratedCallableId::from_key(
                &GeneratedCallableKeyV1::DynamicFunctionAdapter { target }
            ),
            Err(GeneratedCallableIdentityError::TargetReceiverMustBeAbsent)
        );
    }

    #[test]
    fn only_template_roles_can_be_generated_lexical_parents() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let key = GeneratedCallableKeyV1::DerivedEquality { exact_owner: exact };
        assert_eq!(
            LexicalCallableParentV1::from_generated_key(&key),
            Err(LexicalParentError::GeneratedRoleNotLexical)
        );
    }

    #[test]
    fn every_variant_starts_with_its_frozen_tag() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let function = PersistentFunctionId(ConeIdentity::SINGLE_FILE.0);
        let parent = LexicalCallableParentV1::function(function);
        let path = StructuralDefinitionPathV1::from_first(
            StructuralPathSegmentV1::new(StructuralDefinitionSiteRoleV1::Lambda, 0),
            [],
        );
        let target = ExactCallableSignatureV1::new(EffectV1::Ordinary, None, Vec::new(), exact);
        let source_callable = CallableMaterializationV1::new(
            CallableTemplateOwnerV1::Function(function),
            CallableMaterializationContextV1::NoSubstitution,
        );
        let cases: Vec<(GeneratedCallableKeyV1, u8, u8)> = vec![
            (
                GeneratedCallableKeyV1::Lexical {
                    parent,
                    role: LexicalCallableRoleV1::LambdaBody,
                    path: path.clone(),
                },
                1,
                0xa4,
            ),
            (
                GeneratedCallableKeyV1::Initialization {
                    unit: PersistentInitializationUnitId(ConeIdentity::CORE.0),
                    role: InitializationCallableRoleV1::Initializer,
                },
                2,
                0xa3,
            ),
            (
                GeneratedCallableKeyV1::DerivedEquality { exact_owner: exact },
                3,
                0xa2,
            ),
            (
                GeneratedCallableKeyV1::FunctionAdapter {
                    source: target.clone(),
                    target: target.clone(),
                },
                4,
                0xa3,
            ),
            (
                GeneratedCallableKeyV1::DynamicFunctionAdapter {
                    target: target.clone(),
                },
                5,
                0xa2,
            ),
            (
                GeneratedCallableKeyV1::CallableReferenceInvoke {
                    parent,
                    path: path.clone(),
                },
                6,
                0xa3,
            ),
            (
                GeneratedCallableKeyV1::StaticNoGcCallbackStorageBridge {
                    source: source_callable,
                    signature: target.clone(),
                },
                7,
                0xa3,
            ),
            (
                GeneratedCallableKeyV1::ForeignCallbackManagedAdapter {
                    application: PersistentCallbackApplicationId(ConeIdentity::CORE.0),
                },
                8,
                0xa2,
            ),
            (
                GeneratedCallableKeyV1::CoroutineDriver { source_callable },
                9,
                0xa2,
            ),
            (
                GeneratedCallableKeyV1::ContinuationShell {
                    result: exact,
                    role: ContinuationShellRoleV1::Success,
                },
                10,
                0xa3,
            ),
            (
                GeneratedCallableKeyV1::CoroutineStart { result: exact },
                11,
                0xa2,
            ),
            (
                GeneratedCallableKeyV1::CoroutineAdapter {
                    source_callable,
                    suspension_site: path,
                    role: CoroutineAdapterRoleV1::Failure,
                },
                12,
                0xa4,
            ),
            (
                GeneratedCallableKeyV1::FunctionBridge {
                    environment: PersistentTypeId(ConeIdentity::CORE.0),
                    target: target.clone(),
                },
                13,
                0xa3,
            ),
            (
                GeneratedCallableKeyV1::DispatchAdjust {
                    slot: PersistentDispatchSlotId(ConeIdentity::CORE.0),
                    implementor: exact,
                    target: source_callable,
                },
                14,
                0xa4,
            ),
            (
                GeneratedCallableKeyV1::BoxingAdjust {
                    slot: PersistentDispatchSlotId(ConeIdentity::CORE.0),
                    payload: exact,
                    interface: exact,
                },
                15,
                0xa4,
            ),
        ];
        for (key, tag, map_header) in cases {
            let encoded = encode(&key).unwrap();
            assert_eq!(encoded[0], map_header);
            assert_eq!(encoded[1..3], [0, tag]);
        }
    }

    #[test]
    fn lexical_parent_accepts_only_generated_template_roles() {
        let function = PersistentFunctionId(ConeIdentity::CORE.0);
        let path = StructuralDefinitionPathV1::from_first(
            StructuralPathSegmentV1::new(StructuralDefinitionSiteRoleV1::Lambda, 0),
            [],
        );
        let key = GeneratedCallableKeyV1::Lexical {
            parent: LexicalCallableParentV1::function(function),
            role: LexicalCallableRoleV1::AnonymousFunctionBody,
            path,
        };
        assert!(LexicalCallableParentV1::from_generated_key(&key).is_ok());
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
