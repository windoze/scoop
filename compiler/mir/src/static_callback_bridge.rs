//! Persistent identity for static no-GC callback storage bridges.

use std::fmt;

use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableOdrMemberId, CallableOwner,
    CallableTemplateOwner, CborIdentityRecord, Effect, ExactCallableSignature,
    GeneratedCallableIdentityError, GeneratedCallableKey, OdrGroupId, OdrMemberDiscriminator,
    OdrMemberId, OdrMemberIdentityError, OdrMemberKey, OdrMemberRole,
    PersistentGeneratedCallableId, StaticNoGcCallbackStorageBridgeId,
};
use scoop_wire::HashError;

use crate::{
    CallableSignatureRecord, CallableSignatureSubject, CallbackBridge, FunctionId, FunctionTypeId,
};

type GeneratedCallableRecord =
    CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>;
type OdrMemberRecord = CborIdentityRecord<OdrMemberId, OdrMemberKey>;

/// Complete persistent identity of one static no-GC callback storage bridge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticCallbackBridgeIdentity {
    source: CallableMaterialization,
    callable: GeneratedCallableRecord,
    odr_member: Option<OdrMemberRecord>,
    signature: CallableSignatureRecord,
}

impl StaticCallbackBridgeIdentity {
    pub fn new(
        source: CallableMaterialization,
        signature: ExactCallableSignature,
        odr_group: Option<OdrGroupId>,
    ) -> Result<Self, StaticCallbackBridgeIdentityError> {
        if signature.effect() != Effect::Ordinary {
            return Err(StaticCallbackBridgeIdentityError::SuspendSignature);
        }
        if signature.receiver().is_present() {
            return Err(StaticCallbackBridgeIdentityError::ReceiverPresent);
        }
        let expects_odr = source.context() != CallableMaterializationContext::NoSubstitution;
        match (expects_odr, odr_group) {
            (true, None) => return Err(StaticCallbackBridgeIdentityError::MissingOdrGroup),
            (false, Some(_)) => return Err(StaticCallbackBridgeIdentityError::UnexpectedOdrGroup),
            _ => {}
        }

        let callable =
            CborIdentityRecord::from_key(GeneratedCallableKey::StaticNoGcCallbackStorageBridge {
                source,
                signature: signature.clone(),
            })
            .map_err(StaticCallbackBridgeIdentityError::GeneratedCallable)?;
        let odr_member = odr_group
            .map(|group| {
                let key = OdrMemberKey::new(
                    group,
                    OdrMemberRole::CallableBody,
                    OdrMemberDiscriminator::GeneratedCallable(callable.id()),
                )
                .map_err(StaticCallbackBridgeIdentityError::OdrMember)?;
                CborIdentityRecord::from_key(key)
                    .map_err(StaticCallbackBridgeIdentityError::OdrMemberRecord)
            })
            .transpose()?;
        let subject = match &odr_member {
            Some(member) => CallableSignatureSubject::odr(
                CallableOdrMemberId::from_key(member.key())
                    .map_err(StaticCallbackBridgeIdentityError::OdrMember)?,
            ),
            None => CallableSignatureSubject::strong(CallableOwner::Generated(callable.id())),
        };
        Ok(Self {
            source,
            callable,
            odr_member,
            signature: CallableSignatureRecord::new(subject, signature),
        })
    }

    pub const fn source(&self) -> CallableMaterialization {
        self.source
    }

    pub const fn callable_record(&self) -> &GeneratedCallableRecord {
        &self.callable
    }

    pub fn storage_bridge(&self) -> StaticNoGcCallbackStorageBridgeId {
        StaticNoGcCallbackStorageBridgeId::from_key(self.callable.key())
            .expect("static callback identities always own a storage bridge")
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

impl CallbackBridge {
    pub fn new(
        source: FunctionId,
        signature: FunctionTypeId,
        bridge_function: FunctionId,
        source_materialization: CallableMaterialization,
        exact_signature: ExactCallableSignature,
        odr_group: Option<OdrGroupId>,
    ) -> Result<Self, StaticCallbackBridgeIdentityError> {
        Ok(Self {
            source,
            signature,
            bridge_function,
            identity: StaticCallbackBridgeIdentity::new(
                source_materialization,
                exact_signature,
                odr_group,
            )?,
        })
    }

    pub const fn identity(&self) -> &StaticCallbackBridgeIdentity {
        &self.identity
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticCallbackBridgeIdentityError {
    SuspendSignature,
    ReceiverPresent,
    MissingOdrGroup,
    UnexpectedOdrGroup,
    GeneratedCallable(GeneratedCallableIdentityError),
    OdrMember(OdrMemberIdentityError),
    OdrMemberRecord(HashError),
}

impl fmt::Display for StaticCallbackBridgeIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SuspendSignature => {
                formatter.write_str("a static callback bridge cannot use a suspend signature")
            }
            Self::ReceiverPresent => {
                formatter.write_str("a static callback bridge signature cannot have a receiver")
            }
            Self::MissingOdrGroup => {
                formatter.write_str("a materialized static callback bridge requires an ODR group")
            }
            Self::UnexpectedOdrGroup => formatter
                .write_str("a parameter-free static callback bridge must not have an ODR group"),
            Self::GeneratedCallable(error) => error.fmt(formatter),
            Self::OdrMember(error) => error.fmt(formatter),
            Self::OdrMemberRecord(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for StaticCallbackBridgeIdentityError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CallableApplicationKey, CallableInstantiationOwner, CanonicalIdentifier, ConeIdentity,
        CoreBuiltinNominal, DeclarationScope, DefinitionOwnerChain, ExactTypeKey, PackagePath,
        PersistentExactTypeId, PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite,
        SpecializationKey,
    };

    use super::*;

    fn source_function() -> PersistentFunctionId {
        let site = SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
            site,
            CanonicalIdentifier::new("staticCallback").unwrap(),
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

    fn signature() -> ExactCallableSignature {
        ExactCallableSignature::new(Effect::Ordinary, None, vec![unit()], unit())
    }

    #[test]
    fn parameter_free_bridge_is_one_strong_generated_callable() {
        let source = CallableMaterialization::new(
            CallableTemplateOwner::Function(source_function()),
            CallableMaterializationContext::NoSubstitution,
        );
        let signature = signature();
        let identity = StaticCallbackBridgeIdentity::new(source, signature.clone(), None).unwrap();

        assert_eq!(identity.source(), source);
        assert_eq!(
            identity.callable_record().key(),
            &GeneratedCallableKey::StaticNoGcCallbackStorageBridge {
                source,
                signature: signature.clone(),
            }
        );
        assert_eq!(identity.odr_member_record(), None);
        assert_eq!(
            identity.signature_record().subject(),
            CallableSignatureSubject::strong(CallableOwner::Generated(
                identity.callable_record().id()
            ))
        );
        assert_eq!(identity.signature_record().signature(), &signature);
    }

    #[test]
    fn materialized_bridge_is_a_member_of_its_callable_group() {
        let application_key = CallableApplicationKey::for_function(
            source_function(),
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
        let source = CallableMaterialization::new(
            CallableTemplateOwner::Function(source_function()),
            CallableMaterializationContext::Application(application),
        );
        let identity = StaticCallbackBridgeIdentity::new(source, signature(), Some(group)).unwrap();
        let member = identity.odr_member_record().unwrap();

        assert_eq!(member.key().group(), group);
        assert_eq!(member.key().role(), OdrMemberRole::CallableBody);
        assert_eq!(
            member.key().discriminator(),
            &OdrMemberDiscriminator::GeneratedCallable(identity.callable_record().id())
        );
    }
}
