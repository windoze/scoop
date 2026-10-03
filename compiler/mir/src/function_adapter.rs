//! Persistent identity bundle for MIR-generated function-value adapters.

use std::fmt;

use scoop_identity::{
    CallableAdapterEnvironmentKey, CallableMaterialization, CallableMaterializationContext,
    CallableOdrMemberId, CallableTemplateOwner, CborIdentityRecord, ExactCallableSignature,
    ExactTypeKey, FieldIdentityError, FieldIdentityKey, GeneratedCallableIdentityError,
    GeneratedCallableKey, GeneratedNominalIdentityError, GeneratedNominalKey, OdrGroupId,
    OdrMemberDiscriminator, OdrMemberId, OdrMemberIdentityError, OdrMemberKey, OdrMemberRole,
    PersistentExactTypeId, PersistentFieldId, PersistentGeneratedCallableId, PersistentTypeId,
    SpecializationKey,
};
use scoop_wire::HashError;

use crate::{CallableSignatureRecord, CallableSignatureSubject, ClosureClassId, FunctionTypeId};

type GeneratedTypeRecord = CborIdentityRecord<PersistentTypeId, GeneratedNominalKey>;
type GeneratedCallableRecord =
    CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>;
type FieldRecord = CborIdentityRecord<PersistentFieldId, FieldIdentityKey>;
type OdrGroupRecord = CborIdentityRecord<OdrGroupId, SpecializationKey>;
type OdrMemberRecord = CborIdentityRecord<OdrMemberId, OdrMemberKey>;

/// Complete persistent identity projection shared by static and dynamic
/// function-value adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionAdapterIdentity {
    environment: GeneratedTypeRecord,
    source_field: FieldRecord,
    callable: GeneratedCallableRecord,
    odr_group: OdrGroupRecord,
    environment_member: OdrMemberRecord,
    callable_member: OdrMemberRecord,
    callable_signature: CallableSignatureRecord,
}

impl FunctionAdapterIdentity {
    pub fn for_static(
        source: ExactCallableSignature,
        target: ExactCallableSignature,
        target_function_type: PersistentExactTypeId,
    ) -> Result<Self, FunctionAdapterIdentityError> {
        if source.effect() != target.effect() {
            return Err(FunctionAdapterIdentityError::StaticEffectMismatch);
        }
        if source.parameters().len() != target.parameters().len() {
            return Err(FunctionAdapterIdentityError::StaticArityMismatch);
        }
        let environment = GeneratedNominalKey::CallableAdapterEnvironment {
            key: CallableAdapterEnvironmentKey::Static {
                source: source.clone(),
                target: target.clone(),
            },
        };
        let callable = GeneratedCallableKey::FunctionAdapter {
            source,
            target: target.clone(),
        };
        Self::build(environment, callable, target, target_function_type)
    }

    pub fn for_dynamic(
        target: ExactCallableSignature,
        target_function_type: PersistentExactTypeId,
    ) -> Result<Self, FunctionAdapterIdentityError> {
        let environment = GeneratedNominalKey::CallableAdapterEnvironment {
            key: CallableAdapterEnvironmentKey::Dynamic {
                target: target.clone(),
            },
        };
        let callable = GeneratedCallableKey::DynamicFunctionAdapter {
            target: target.clone(),
        };
        Self::build(environment, callable, target, target_function_type)
    }

    fn build(
        environment_key: GeneratedNominalKey,
        callable_key: GeneratedCallableKey,
        target: ExactCallableSignature,
        target_function_type: PersistentExactTypeId,
    ) -> Result<Self, FunctionAdapterIdentityError> {
        if target.receiver().is_present() {
            return Err(FunctionAdapterIdentityError::TargetReceiverPresent);
        }
        let expected_target = PersistentExactTypeId::from_key(&ExactTypeKey::Function {
            effect: target.effect(),
            parameters: target.parameters().to_vec(),
            result: target.result(),
        })
        .map_err(FunctionAdapterIdentityError::TargetShape)?;
        if target_function_type != expected_target {
            return Err(FunctionAdapterIdentityError::TargetShapeMismatch {
                expected: expected_target,
                actual: target_function_type,
            });
        }

        let environment = CborIdentityRecord::from_key(environment_key)
            .map_err(FunctionAdapterIdentityError::GeneratedNominal)?;
        let source_field = CborIdentityRecord::from_key(
            FieldIdentityKey::function_adapter_source(environment.key())
                .map_err(FunctionAdapterIdentityError::Field)?,
        )
        .map_err(FunctionAdapterIdentityError::Field)?;
        let callable = CborIdentityRecord::from_key(callable_key)
            .map_err(FunctionAdapterIdentityError::GeneratedCallable)?;
        let odr_group = CborIdentityRecord::from_key(SpecializationKey::StructuralType {
            exact_type: target_function_type,
        })
        .map_err(FunctionAdapterIdentityError::OdrGroup)?;
        let environment_member = odr_member(
            odr_group.id(),
            OdrMemberRole::GeneratedNominal,
            OdrMemberDiscriminator::GeneratedNominal(environment.id()),
        )?;
        let callable_member = odr_member(
            odr_group.id(),
            OdrMemberRole::CallableBody,
            OdrMemberDiscriminator::GeneratedCallable(callable.id()),
        )?;
        let callable_subject = CallableOdrMemberId::from_key(callable_member.key())
            .map_err(FunctionAdapterIdentityError::OdrMember)?;
        let callable_signature =
            CallableSignatureRecord::new(CallableSignatureSubject::odr(callable_subject), target);
        Ok(Self {
            environment,
            source_field,
            callable,
            odr_group,
            environment_member,
            callable_member,
            callable_signature,
        })
    }

    pub const fn environment_record(&self) -> &GeneratedTypeRecord {
        &self.environment
    }

    pub const fn source_field_record(&self) -> &FieldRecord {
        &self.source_field
    }

    pub const fn callable_record(&self) -> &GeneratedCallableRecord {
        &self.callable
    }

    pub const fn odr_group_record(&self) -> &OdrGroupRecord {
        &self.odr_group
    }

    pub const fn environment_member_record(&self) -> &OdrMemberRecord {
        &self.environment_member
    }

    pub const fn callable_member_record(&self) -> &OdrMemberRecord {
        &self.callable_member
    }

    pub const fn callable_signature_record(&self) -> &CallableSignatureRecord {
        &self.callable_signature
    }

    pub const fn materialization(&self) -> CallableMaterialization {
        CallableMaterialization::new(
            CallableTemplateOwner::Generated(self.callable.id()),
            CallableMaterializationContext::NoSubstitution,
        )
    }
}

/// Static variance adapter from one exact managed function shape to another.
#[derive(Debug)]
pub struct ClosureAdapter {
    class: ClosureClassId,
    source: FunctionTypeId,
    target: FunctionTypeId,
    identity: FunctionAdapterIdentity,
}

impl ClosureAdapter {
    pub fn new(
        class: ClosureClassId,
        source: FunctionTypeId,
        target: FunctionTypeId,
        source_signature: ExactCallableSignature,
        target_signature: ExactCallableSignature,
        target_function_type: PersistentExactTypeId,
    ) -> Result<Self, FunctionAdapterIdentityError> {
        Ok(Self {
            class,
            source,
            target,
            identity: FunctionAdapterIdentity::for_static(
                source_signature,
                target_signature,
                target_function_type,
            )?,
        })
    }

    pub const fn class(&self) -> ClosureClassId {
        self.class
    }

    pub const fn source(&self) -> FunctionTypeId {
        self.source
    }

    pub const fn target(&self) -> FunctionTypeId {
        self.target
    }

    pub const fn identity(&self) -> &FunctionAdapterIdentity {
        &self.identity
    }
}

/// Adapter used after a runtime `Any`/interface-to-function check.
#[derive(Debug)]
pub struct DynamicClosureAdapter {
    class: ClosureClassId,
    target: FunctionTypeId,
    identity: FunctionAdapterIdentity,
}

impl DynamicClosureAdapter {
    pub fn new(
        class: ClosureClassId,
        target: FunctionTypeId,
        target_signature: ExactCallableSignature,
        target_function_type: PersistentExactTypeId,
    ) -> Result<Self, FunctionAdapterIdentityError> {
        Ok(Self {
            class,
            target,
            identity: FunctionAdapterIdentity::for_dynamic(target_signature, target_function_type)?,
        })
    }

    pub const fn class(&self) -> ClosureClassId {
        self.class
    }

    pub const fn target(&self) -> FunctionTypeId {
        self.target
    }

    pub const fn identity(&self) -> &FunctionAdapterIdentity {
        &self.identity
    }
}

fn odr_member(
    group: OdrGroupId,
    role: OdrMemberRole,
    discriminator: OdrMemberDiscriminator,
) -> Result<OdrMemberRecord, FunctionAdapterIdentityError> {
    let key = OdrMemberKey::new(group, role, discriminator)
        .map_err(FunctionAdapterIdentityError::OdrMember)?;
    CborIdentityRecord::from_key(key).map_err(FunctionAdapterIdentityError::OdrMemberRecord)
}

#[derive(Debug)]
pub enum FunctionAdapterIdentityError {
    StaticEffectMismatch,
    StaticArityMismatch,
    TargetReceiverPresent,
    TargetShape(HashError),
    TargetShapeMismatch {
        expected: PersistentExactTypeId,
        actual: PersistentExactTypeId,
    },
    GeneratedNominal(GeneratedNominalIdentityError),
    GeneratedCallable(GeneratedCallableIdentityError),
    Field(FieldIdentityError),
    OdrGroup(HashError),
    OdrMember(OdrMemberIdentityError),
    OdrMemberRecord(HashError),
}

impl fmt::Display for FunctionAdapterIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaticEffectMismatch => {
                formatter.write_str("static function adapter must preserve its effect")
            }
            Self::StaticArityMismatch => {
                formatter.write_str("static function adapter must preserve its arity")
            }
            Self::TargetReceiverPresent => {
                formatter.write_str("function adapter target must not have a receiver")
            }
            Self::TargetShape(error) => write!(formatter, "failed to derive target shape: {error}"),
            Self::TargetShapeMismatch { expected, actual } => write!(
                formatter,
                "function adapter target type {actual} does not match signature shape {expected}"
            ),
            Self::GeneratedNominal(error) => error.fmt(formatter),
            Self::GeneratedCallable(error) => error.fmt(formatter),
            Self::Field(error) => error.fmt(formatter),
            Self::OdrGroup(error) => write!(formatter, "failed to derive ODR group: {error}"),
            Self::OdrMember(error) => error.fmt(formatter),
            Self::OdrMemberRecord(error) => {
                write!(formatter, "failed to record ODR member: {error}")
            }
        }
    }
}

impl std::error::Error for FunctionAdapterIdentityError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{CoreBuiltinNominal, Effect, ExactTypeKey};

    use super::*;

    fn unit() -> PersistentExactTypeId {
        PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap()
    }

    fn any() -> PersistentExactTypeId {
        PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            CoreBuiltinNominal::Any.identity_record().id(),
        ))
        .unwrap()
    }

    fn signature(parameter: PersistentExactTypeId) -> ExactCallableSignature {
        ExactCallableSignature::new(Effect::Ordinary, None, vec![parameter], unit())
    }

    fn function_type(signature: &ExactCallableSignature) -> PersistentExactTypeId {
        PersistentExactTypeId::from_key(&ExactTypeKey::Function {
            effect: signature.effect(),
            parameters: signature.parameters().to_vec(),
            result: signature.result(),
        })
        .unwrap()
    }

    #[test]
    fn static_adapter_records_one_structural_identity_bundle() {
        let source = signature(any());
        let target = signature(unit());
        let identity = FunctionAdapterIdentity::for_static(
            source.clone(),
            target.clone(),
            function_type(&target),
        )
        .unwrap();

        assert!(matches!(
            identity.environment_record().key(),
            GeneratedNominalKey::CallableAdapterEnvironment {
                key: CallableAdapterEnvironmentKey::Static {
                    source: actual_source,
                    target: actual_target,
                },
            } if actual_source == &source && actual_target == &target
        ));
        assert!(matches!(
            identity.callable_record().key(),
            GeneratedCallableKey::FunctionAdapter {
                source: actual_source,
                target: actual_target,
            } if actual_source == &source && actual_target == &target
        ));
        assert_eq!(
            identity.source_field_record().key(),
            &FieldIdentityKey::function_adapter_source(identity.environment_record().key())
                .unwrap()
        );
        assert!(matches!(
            identity.odr_group_record().key(),
            SpecializationKey::StructuralType { exact_type }
                if *exact_type == function_type(&target)
        ));
        assert_eq!(identity.callable_signature_record().signature(), &target);
        assert_eq!(
            identity.callable_signature_record().subject(),
            CallableSignatureSubject::odr(
                CallableOdrMemberId::from_key(identity.callable_member_record().key()).unwrap()
            )
        );
    }

    #[test]
    fn static_adapter_rejects_an_arity_change() {
        let source = signature(unit());
        let target = ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit());
        assert!(matches!(
            FunctionAdapterIdentity::for_static(source, target.clone(), function_type(&target)),
            Err(FunctionAdapterIdentityError::StaticArityMismatch)
        ));
    }

    #[test]
    fn dynamic_adapter_rejects_a_mismatched_target_shape() {
        let target = signature(unit());
        let wrong = PersistentExactTypeId::from_key(&ExactTypeKey::Tuple(
            scoop_identity::NonEmptyVec::from_first(unit(), []),
        ))
        .unwrap();
        assert!(matches!(
            FunctionAdapterIdentity::for_dynamic(target, wrong),
            Err(FunctionAdapterIdentityError::TargetShapeMismatch { actual, .. }) if actual == wrong
        ));
    }
}
