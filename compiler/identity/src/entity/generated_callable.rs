use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncode};

use super::{CallableMaterialization, ExactCallableSignature, StructuralDefinitionPath};
use crate::ids::derive_persistent_id;
use crate::{
    PersistentCallbackApplicationId, PersistentConstructorId, PersistentDispatchSlotId,
    PersistentEnumVariantId, PersistentExactTypeId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentInitializationUnitId,
    PersistentPropertyAccessorId, PersistentTypeId,
};

mod decode;

pub use decode::{
    DecodedGeneratedCallableKey, DecodedLexicalCallableParent, GeneratedCallableResolutionError,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LexicalCallableRole {
    LambdaBody,
    AnonymousFunctionBody,
}

impl WireEncode for LexicalCallableRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::LambdaBody => 1,
            Self::AnonymousFunctionBody => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum InitializationCallableRole {
    Initializer,
    Ensure,
}

impl WireEncode for InitializationCallableRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Initializer => 1,
            Self::Ensure => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ContinuationShellRole {
    Success,
    Failure,
}

impl WireEncode for ContinuationShellRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Success => 1,
            Self::Failure => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CoroutineAdapterRole {
    Success,
    Failure,
}

impl WireEncode for CoroutineAdapterRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Success => 1,
            Self::Failure => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LexicalCallableParent(LexicalCallableParentKind);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum LexicalCallableParentKind {
    Function(PersistentFunctionId),
    GenericFunction(PersistentGenericFunctionId),
    Constructor(PersistentConstructorId),
    Accessor(PersistentPropertyAccessorId),
    Generated(PersistentGeneratedCallableId),
    VariantConstructor(PersistentEnumVariantId),
}

impl LexicalCallableParent {
    pub const fn function(id: PersistentFunctionId) -> Self {
        Self(LexicalCallableParentKind::Function(id))
    }

    pub const fn generic_function(id: PersistentGenericFunctionId) -> Self {
        Self(LexicalCallableParentKind::GenericFunction(id))
    }

    pub const fn constructor(id: PersistentConstructorId) -> Self {
        Self(LexicalCallableParentKind::Constructor(id))
    }

    pub const fn accessor(id: PersistentPropertyAccessorId) -> Self {
        Self(LexicalCallableParentKind::Accessor(id))
    }

    pub const fn variant_constructor(id: PersistentEnumVariantId) -> Self {
        Self(LexicalCallableParentKind::VariantConstructor(id))
    }

    pub fn from_generated_key(key: &GeneratedCallableKey) -> Result<Self, LexicalParentError> {
        if !key.can_be_lexical_parent() {
            return Err(LexicalParentError::GeneratedRoleNotLexical);
        }
        PersistentGeneratedCallableId::from_key(key)
            .map(|id| Self(LexicalCallableParentKind::Generated(id)))
            .map_err(LexicalParentError::Identity)
    }

    pub const fn generated_parent(self) -> Option<PersistentGeneratedCallableId> {
        match self.0 {
            LexicalCallableParentKind::Generated(id) => Some(id),
            _ => None,
        }
    }
}

impl WireEncode for LexicalCallableParent {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self.0 {
            LexicalCallableParentKind::Function(id) => encode_value_sum(encoder, 1, &id),
            LexicalCallableParentKind::GenericFunction(id) => encode_value_sum(encoder, 2, &id),
            LexicalCallableParentKind::Constructor(id) => encode_value_sum(encoder, 3, &id),
            LexicalCallableParentKind::Accessor(id) => encode_value_sum(encoder, 4, &id),
            LexicalCallableParentKind::Generated(id) => encode_value_sum(encoder, 5, &id),
            LexicalCallableParentKind::VariantConstructor(id) => encode_value_sum(encoder, 6, &id),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneratedCallableKey {
    Lexical {
        parent: LexicalCallableParent,
        role: LexicalCallableRole,
        path: StructuralDefinitionPath,
    },
    Initialization {
        unit: PersistentInitializationUnitId,
        role: InitializationCallableRole,
    },
    DerivedEquality {
        exact_owner: PersistentExactTypeId,
    },
    FunctionAdapter {
        source: ExactCallableSignature,
        target: ExactCallableSignature,
    },
    DynamicFunctionAdapter {
        target: ExactCallableSignature,
    },
    CallableReferenceInvoke {
        parent: LexicalCallableParent,
        path: StructuralDefinitionPath,
    },
    StaticNoGcCallbackStorageBridge {
        source: CallableMaterialization,
        signature: ExactCallableSignature,
    },
    ForeignCallbackManagedAdapter {
        application: PersistentCallbackApplicationId,
    },
    CoroutineDriver {
        source_callable: CallableMaterialization,
    },
    ContinuationShell {
        result: PersistentExactTypeId,
        role: ContinuationShellRole,
    },
    CoroutineStart {
        result: PersistentExactTypeId,
    },
    CoroutineAdapter {
        source_callable: CallableMaterialization,
        suspension_site: StructuralDefinitionPath,
        role: CoroutineAdapterRole,
    },
    FunctionBridge {
        environment: PersistentTypeId,
        target: ExactCallableSignature,
    },
    DispatchAdjust {
        slot: PersistentDispatchSlotId,
        implementor: PersistentExactTypeId,
        target: CallableMaterialization,
    },
    BoxingAdjust {
        slot: PersistentDispatchSlotId,
        payload: PersistentExactTypeId,
        interface: PersistentExactTypeId,
    },
    ZeroArgumentConstructorAdapter {
        constructor: PersistentConstructorId,
    },
}

impl GeneratedCallableKey {
    fn can_be_lexical_parent(&self) -> bool {
        matches!(
            self,
            Self::Lexical { .. }
                | Self::Initialization { .. }
                | Self::CallableReferenceInvoke { .. }
                | Self::ZeroArgumentConstructorAdapter { .. }
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

    pub fn generated_callable_dependencies(&self) -> Vec<PersistentGeneratedCallableId> {
        let dependency = match self {
            Self::Lexical { parent, .. } | Self::CallableReferenceInvoke { parent, .. } => {
                parent.generated_parent()
            }
            Self::StaticNoGcCallbackStorageBridge { source, .. }
            | Self::CoroutineDriver {
                source_callable: source,
            }
            | Self::CoroutineAdapter {
                source_callable: source,
                ..
            }
            | Self::DispatchAdjust { target: source, .. } => source.generated_template(),
            _ => None,
        };
        dependency.into_iter().collect()
    }
}

impl WireEncode for GeneratedCallableKey {
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
            Self::ZeroArgumentConstructorAdapter { constructor } => {
                encode_value_sum(encoder, 16, constructor)
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
                "generated lexical parent must be lexical, initialization, callable-reference invoke, or zero-argument constructor adapter",
            ),
            Self::Identity(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for LexicalParentError {}

impl PersistentGeneratedCallableId {
    pub fn from_key(key: &GeneratedCallableKey) -> Result<Self, GeneratedCallableIdentityError> {
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

    use super::{
        ContinuationShellRole, CoroutineAdapterRole, GeneratedCallableIdentityError,
        GeneratedCallableKey, InitializationCallableRole, LexicalCallableParent,
        LexicalCallableRole, LexicalParentError,
    };
    use crate::{
        CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
        ConeIdentity, Effect, ExactCallableSignature, PersistentCallbackApplicationId,
        PersistentConstructorId, PersistentDispatchSlotId, PersistentExactTypeId,
        PersistentFunctionId, PersistentGeneratedCallableId, PersistentInitializationUnitId,
        PersistentTypeId, StructuralDefinitionPath, StructuralDefinitionSiteRole,
        StructuralPathSegment,
    };

    #[test]
    fn generated_callable_has_fixed_identity() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let target = ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), exact);
        let key = GeneratedCallableKey::DynamicFunctionAdapter { target };
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
        let target = ExactCallableSignature::new(Effect::Ordinary, Some(exact), Vec::new(), exact);
        assert_eq!(
            PersistentGeneratedCallableId::from_key(
                &GeneratedCallableKey::DynamicFunctionAdapter { target }
            ),
            Err(GeneratedCallableIdentityError::TargetReceiverMustBeAbsent)
        );
    }

    #[test]
    fn only_template_roles_can_be_generated_lexical_parents() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let key = GeneratedCallableKey::DerivedEquality { exact_owner: exact };
        assert_eq!(
            LexicalCallableParent::from_generated_key(&key),
            Err(LexicalParentError::GeneratedRoleNotLexical)
        );
    }

    #[test]
    fn every_variant_starts_with_its_frozen_tag() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let function = PersistentFunctionId(ConeIdentity::SINGLE_FILE.0);
        let parent = LexicalCallableParent::function(function);
        let path = StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, 0),
            [],
        );
        let target = ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), exact);
        let source_callable = CallableMaterialization::new(
            CallableTemplateOwner::Function(function),
            CallableMaterializationContext::NoSubstitution,
        );
        let cases: Vec<(GeneratedCallableKey, u8, u8)> = vec![
            (
                GeneratedCallableKey::Lexical {
                    parent,
                    role: LexicalCallableRole::LambdaBody,
                    path: path.clone(),
                },
                1,
                0xa4,
            ),
            (
                GeneratedCallableKey::Initialization {
                    unit: PersistentInitializationUnitId(ConeIdentity::CORE.0),
                    role: InitializationCallableRole::Initializer,
                },
                2,
                0xa3,
            ),
            (
                GeneratedCallableKey::DerivedEquality { exact_owner: exact },
                3,
                0xa2,
            ),
            (
                GeneratedCallableKey::FunctionAdapter {
                    source: target.clone(),
                    target: target.clone(),
                },
                4,
                0xa3,
            ),
            (
                GeneratedCallableKey::DynamicFunctionAdapter {
                    target: target.clone(),
                },
                5,
                0xa2,
            ),
            (
                GeneratedCallableKey::CallableReferenceInvoke {
                    parent,
                    path: path.clone(),
                },
                6,
                0xa3,
            ),
            (
                GeneratedCallableKey::StaticNoGcCallbackStorageBridge {
                    source: source_callable,
                    signature: target.clone(),
                },
                7,
                0xa3,
            ),
            (
                GeneratedCallableKey::ForeignCallbackManagedAdapter {
                    application: PersistentCallbackApplicationId(ConeIdentity::CORE.0),
                },
                8,
                0xa2,
            ),
            (
                GeneratedCallableKey::CoroutineDriver { source_callable },
                9,
                0xa2,
            ),
            (
                GeneratedCallableKey::ContinuationShell {
                    result: exact,
                    role: ContinuationShellRole::Success,
                },
                10,
                0xa3,
            ),
            (
                GeneratedCallableKey::CoroutineStart { result: exact },
                11,
                0xa2,
            ),
            (
                GeneratedCallableKey::CoroutineAdapter {
                    source_callable,
                    suspension_site: path,
                    role: CoroutineAdapterRole::Failure,
                },
                12,
                0xa4,
            ),
            (
                GeneratedCallableKey::FunctionBridge {
                    environment: PersistentTypeId(ConeIdentity::CORE.0),
                    target: target.clone(),
                },
                13,
                0xa3,
            ),
            (
                GeneratedCallableKey::DispatchAdjust {
                    slot: PersistentDispatchSlotId(ConeIdentity::CORE.0),
                    implementor: exact,
                    target: source_callable,
                },
                14,
                0xa4,
            ),
            (
                GeneratedCallableKey::BoxingAdjust {
                    slot: PersistentDispatchSlotId(ConeIdentity::CORE.0),
                    payload: exact,
                    interface: exact,
                },
                15,
                0xa4,
            ),
            (
                GeneratedCallableKey::ZeroArgumentConstructorAdapter {
                    constructor: PersistentConstructorId(ConeIdentity::CORE.0),
                },
                16,
                0xa2,
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
        let path = StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, 0),
            [],
        );
        let key = GeneratedCallableKey::Lexical {
            parent: LexicalCallableParent::function(function),
            role: LexicalCallableRole::AnonymousFunctionBody,
            path,
        };
        assert!(LexicalCallableParent::from_generated_key(&key).is_ok());
        assert!(
            LexicalCallableParent::from_generated_key(
                &GeneratedCallableKey::ZeroArgumentConstructorAdapter {
                    constructor: PersistentConstructorId(ConeIdentity::CORE.0),
                }
            )
            .is_ok()
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
