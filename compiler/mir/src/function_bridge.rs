//! Persistent identity for generated closure function-type bridges.

use std::fmt;

use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableOdrMemberId, CallableOwner,
    CallableTemplateOwner, CborIdentityRecord, ExactCallableSignature,
    GeneratedCallableIdentityError, GeneratedCallableKey, GeneratedNominalKey, OdrGroupId,
    OdrMemberDiscriminator, OdrMemberId, OdrMemberIdentityError, OdrMemberKey, OdrMemberRole,
    PersistentGeneratedCallableId, PersistentTypeId,
};
use scoop_wire::HashError;

use crate::{
    CallableSignatureRecord, CallableSignatureSubject, ClosureClass, ClosureClassId, FunctionId,
    FunctionTypeId,
};

type GeneratedTypeRecord = CborIdentityRecord<PersistentTypeId, GeneratedNominalKey>;
type GeneratedCallableRecord =
    CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>;
type OdrMemberRecord = CborIdentityRecord<OdrMemberId, OdrMemberKey>;

/// Complete persistent identity of one generated function-type forwarding
/// entry. The source signature is already fixed by `environment`; the target
/// signature is retained explicitly by the generated callable key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionBridgeIdentity {
    environment: GeneratedTypeRecord,
    callable: GeneratedCallableRecord,
    odr_member: Option<OdrMemberRecord>,
    signature: CallableSignatureRecord,
}

impl FunctionBridgeIdentity {
    pub fn new(
        environment: &GeneratedTypeRecord,
        target: ExactCallableSignature,
        odr_group: Option<OdrGroupId>,
    ) -> Result<Self, FunctionBridgeIdentityError> {
        let expects_odr = match environment.key() {
            GeneratedNominalKey::ClosureEnvironment { callable, .. } => {
                callable.context() != CallableMaterializationContext::NoSubstitution
            }
            GeneratedNominalKey::CallableAdapterEnvironment { .. } => true,
            _ => return Err(FunctionBridgeIdentityError::ExpectedClosureEnvironment),
        };
        match (expects_odr, odr_group) {
            (true, None) => return Err(FunctionBridgeIdentityError::MissingOdrGroup),
            (false, Some(_)) => return Err(FunctionBridgeIdentityError::UnexpectedOdrGroup),
            _ => {}
        }

        let callable = CborIdentityRecord::from_key(GeneratedCallableKey::FunctionBridge {
            environment: environment.id(),
            target: target.clone(),
        })
        .map_err(FunctionBridgeIdentityError::GeneratedCallable)?;
        let odr_member = odr_group
            .map(|group| {
                let key = OdrMemberKey::new(
                    group,
                    OdrMemberRole::CallableBody,
                    OdrMemberDiscriminator::GeneratedCallable(callable.id()),
                )
                .map_err(FunctionBridgeIdentityError::OdrMember)?;
                CborIdentityRecord::from_key(key)
                    .map_err(FunctionBridgeIdentityError::OdrMemberRecord)
            })
            .transpose()?;
        let subject = match &odr_member {
            Some(member) => CallableSignatureSubject::odr(
                CallableOdrMemberId::from_key(member.key())
                    .map_err(FunctionBridgeIdentityError::OdrMember)?,
            ),
            None => CallableSignatureSubject::strong(CallableOwner::Generated(callable.id())),
        };
        Ok(Self {
            environment: environment.clone(),
            callable,
            odr_member,
            signature: CallableSignatureRecord::new(subject, target),
        })
    }

    pub const fn environment_record(&self) -> &GeneratedTypeRecord {
        &self.environment
    }

    pub const fn callable_record(&self) -> &GeneratedCallableRecord {
        &self.callable
    }

    pub const fn odr_member_record(&self) -> Option<&OdrMemberRecord> {
        self.odr_member.as_ref()
    }

    pub const fn signature_record(&self) -> &CallableSignatureRecord {
        &self.signature
    }

    pub const fn materialization(&self) -> CallableMaterialization {
        CallableMaterialization::new(
            CallableTemplateOwner::Generated(self.callable.id()),
            CallableMaterializationContext::NoSubstitution,
        )
    }
}

/// Exact physical bridge-table entry materializing one generated identity.
/// Direct entries that reuse the source invoke are intentionally excluded.
#[derive(Clone, Debug)]
pub struct FunctionBridgeMaterialization {
    class: ClosureClassId,
    target: FunctionTypeId,
    function: FunctionId,
    identity: FunctionBridgeIdentity,
}

impl FunctionBridgeMaterialization {
    pub fn checked(
        class: ClosureClassId,
        definition: &ClosureClass,
        target: FunctionTypeId,
        function: FunctionId,
        identity: FunctionBridgeIdentity,
    ) -> Option<Self> {
        if target == definition.function_type {
            return None;
        }
        let mut matches = definition
            .bridges
            .iter()
            .filter(|bridge| bridge.target == target && bridge.function == function);
        matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        Some(Self {
            class,
            target,
            function,
            identity,
        })
    }

    pub const fn class(&self) -> ClosureClassId {
        self.class
    }

    pub const fn target(&self) -> FunctionTypeId {
        self.target
    }

    pub const fn function(&self) -> FunctionId {
        self.function
    }

    pub const fn identity(&self) -> &FunctionBridgeIdentity {
        &self.identity
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionBridgeIdentityError {
    ExpectedClosureEnvironment,
    MissingOdrGroup,
    UnexpectedOdrGroup,
    GeneratedCallable(GeneratedCallableIdentityError),
    OdrMember(OdrMemberIdentityError),
    OdrMemberRecord(HashError),
}

impl fmt::Display for FunctionBridgeIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedClosureEnvironment => formatter.write_str(
                "function bridge environment must be a source closure or function adapter",
            ),
            Self::MissingOdrGroup => {
                formatter.write_str("function bridge environment requires an ODR group")
            }
            Self::UnexpectedOdrGroup => formatter
                .write_str("parameter-free source closure bridge must not have an ODR group"),
            Self::GeneratedCallable(error) => error.fmt(formatter),
            Self::OdrMember(error) => error.fmt(formatter),
            Self::OdrMemberRecord(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for FunctionBridgeIdentityError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CallableAdapterEnvironmentKey, CallableApplicationKey, CallableInstantiationOwner,
        CanonicalIdentifier, ConeIdentity, CoreBuiltinNominal, DeclarationScope,
        DefinitionOwnerChain, ExactTypeKey, GeneratedCallableKey, LexicalCallableParent,
        LexicalCallableRole, PackagePath, PersistentExactTypeId, PersistentFunctionId,
        SourceDeclarationKey, SourceDeclarationSite, SpecializationKey, StructuralDefinitionPath,
        StructuralDefinitionSiteRole, StructuralPathSegment,
    };

    use super::*;
    use crate::{ClosureInvokeFunctionId, FunctionBridge};

    fn source_function(name: &str) -> PersistentFunctionId {
        let site = SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
            site,
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            Vec::new(),
        ))
        .unwrap()
    }

    fn unit() -> PersistentExactTypeId {
        CborIdentityRecord::from_key(ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap()
        .id()
    }

    fn target_signature() -> ExactCallableSignature {
        ExactCallableSignature::new(scoop_identity::Effect::Ordinary, None, vec![unit()], unit())
    }

    fn lambda_environment(context: CallableMaterializationContext) -> GeneratedTypeRecord {
        let callable = CborIdentityRecord::from_key(GeneratedCallableKey::Lexical {
            parent: LexicalCallableParent::function(source_function("bridgeOwner")),
            role: LexicalCallableRole::LambdaBody,
            path: StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, 0),
                [],
            ),
        })
        .unwrap()
        .id();
        CborIdentityRecord::from_key(GeneratedNominalKey::ClosureEnvironment {
            callable: CallableMaterialization::new(
                CallableTemplateOwner::Generated(callable),
                context,
            ),
            role: scoop_identity::ClosureEnvironmentRole::Lambda,
        })
        .unwrap()
    }

    fn adapter_environment() -> (GeneratedTypeRecord, OdrGroupId) {
        let target = target_signature();
        let environment =
            CborIdentityRecord::from_key(GeneratedNominalKey::CallableAdapterEnvironment {
                key: CallableAdapterEnvironmentKey::Dynamic {
                    target: target.clone(),
                },
            })
            .unwrap();
        let exact_function = CborIdentityRecord::from_key(ExactTypeKey::Function {
            effect: target.effect(),
            parameters: target.parameters().to_vec(),
            result: target.result(),
        })
        .unwrap()
        .id();
        let group = CborIdentityRecord::from_key(SpecializationKey::StructuralType {
            exact_type: exact_function,
        })
        .unwrap()
        .id();
        (environment, group)
    }

    #[test]
    fn parameter_free_source_bridge_is_a_strong_generated_callable() {
        let environment = lambda_environment(CallableMaterializationContext::NoSubstitution);
        let target = target_signature();
        let identity = FunctionBridgeIdentity::new(&environment, target.clone(), None).unwrap();

        assert_eq!(identity.environment_record(), &environment);
        assert_eq!(
            identity.callable_record().key(),
            &GeneratedCallableKey::FunctionBridge {
                environment: environment.id(),
                target: target.clone(),
            }
        );
        assert_eq!(identity.odr_member_record(), None);
        assert_eq!(
            identity.signature_record().subject(),
            CallableSignatureSubject::strong(CallableOwner::Generated(
                identity.callable_record().id()
            ))
        );
        assert_eq!(identity.signature_record().signature(), &target);
        assert_eq!(
            identity.materialization().generated_template(),
            Some(identity.callable_record().id())
        );
    }

    #[test]
    fn materialized_source_bridge_uses_its_callable_application_group() {
        let application_key = CallableApplicationKey::for_function(
            source_function("applicationOwner"),
            CallableInstantiationOwner::NoOwner,
        );
        let application = CborIdentityRecord::from_key(application_key.clone())
            .unwrap()
            .id();
        let group = CborIdentityRecord::from_key(SpecializationKey::Callable {
            application: application_key,
        })
        .unwrap()
        .id();
        let environment =
            lambda_environment(CallableMaterializationContext::Application(application));
        let identity =
            FunctionBridgeIdentity::new(&environment, target_signature(), Some(group)).unwrap();
        let member = identity.odr_member_record().unwrap();

        assert_eq!(member.key().group(), group);
        assert_eq!(member.key().role(), OdrMemberRole::CallableBody);
        assert_eq!(
            member.key().discriminator(),
            &OdrMemberDiscriminator::GeneratedCallable(identity.callable_record().id())
        );
        assert_eq!(
            identity.signature_record().subject(),
            CallableSignatureSubject::odr(CallableOdrMemberId::from_key(member.key()).unwrap())
        );
    }

    #[test]
    fn environment_kind_and_materialization_root_are_not_optional_guesses() {
        let strong = lambda_environment(CallableMaterializationContext::NoSubstitution);
        let (adapter, group) = adapter_environment();
        assert_eq!(
            FunctionBridgeIdentity::new(&strong, target_signature(), Some(group)),
            Err(FunctionBridgeIdentityError::UnexpectedOdrGroup)
        );
        assert_eq!(
            FunctionBridgeIdentity::new(&adapter, target_signature(), None),
            Err(FunctionBridgeIdentityError::MissingOdrGroup)
        );

        let boxed =
            CborIdentityRecord::from_key(GeneratedNominalKey::BoxedValue { payload: unit() })
                .unwrap();
        assert_eq!(
            FunctionBridgeIdentity::new(&boxed, target_signature(), None),
            Err(FunctionBridgeIdentityError::ExpectedClosureEnvironment)
        );
    }

    #[test]
    fn physical_materialization_excludes_the_direct_invoke_entry() {
        let environment = lambda_environment(CallableMaterializationContext::NoSubstitution);
        let identity = FunctionBridgeIdentity::new(&environment, target_signature(), None).unwrap();
        let source = FunctionTypeId::from_raw(0_u32.into());
        let target = FunctionTypeId::from_raw(1_u32.into());
        let function = FunctionId::from_raw(2_u32.into());
        let definition = ClosureClass {
            name: "$BridgeTest".to_string(),
            function_type: source,
            invoke: ClosureInvokeFunctionId::from_raw(0_u32.into()),
            captures: Vec::new(),
            bridges: vec![FunctionBridge { target, function }],
        };

        assert!(
            FunctionBridgeMaterialization::checked(
                ClosureClassId::from_raw(0_u32.into()),
                &definition,
                target,
                function,
                identity.clone(),
            )
            .is_some()
        );
        assert!(
            FunctionBridgeMaterialization::checked(
                ClosureClassId::from_raw(0_u32.into()),
                &definition,
                source,
                function,
                identity,
            )
            .is_none()
        );
    }
}
