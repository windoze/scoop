//! Persistent identities for source-owned coroutine state-machine artifacts.

use std::fmt;

use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
    CborIdentityRecord, GeneratedCallableIdentityError, GeneratedCallableKey, OdrGroupId,
    OdrMemberDiscriminator, OdrMemberId, OdrMemberIdentityError, OdrMemberKey, OdrMemberRole,
    PersistentGeneratedCallableId,
};
use scoop_wire::HashError;

type GeneratedCallableRecord =
    CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>;
type OdrMemberRecord = CborIdentityRecord<OdrMemberId, OdrMemberKey>;

/// Complete persistent identity of the state-machine driver generated for one
/// suspend callable that can actually suspend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoroutineDriverIdentity {
    source: CallableMaterialization,
    callable: GeneratedCallableRecord,
    odr_member: Option<OdrMemberRecord>,
}

impl CoroutineDriverIdentity {
    pub fn new(
        source: CallableMaterialization,
        odr_group: Option<OdrGroupId>,
    ) -> Result<Self, CoroutineDriverIdentityError> {
        if source.context() != CallableMaterializationContext::NoSubstitution && odr_group.is_none()
        {
            return Err(CoroutineDriverIdentityError::MissingOdrGroup);
        }
        let callable = CborIdentityRecord::from_key(GeneratedCallableKey::CoroutineDriver {
            source_callable: source,
        })
        .map_err(CoroutineDriverIdentityError::GeneratedCallable)?;
        let odr_member = odr_group
            .map(|group| {
                let key = OdrMemberKey::new(
                    group,
                    OdrMemberRole::CallableBody,
                    OdrMemberDiscriminator::GeneratedCallable(callable.id()),
                )
                .map_err(CoroutineDriverIdentityError::OdrMember)?;
                CborIdentityRecord::from_key(key)
                    .map_err(CoroutineDriverIdentityError::OdrMemberRecord)
            })
            .transpose()?;
        Ok(Self {
            source,
            callable,
            odr_member,
        })
    }

    pub const fn source(&self) -> CallableMaterialization {
        self.source
    }

    pub const fn callable_record(&self) -> &GeneratedCallableRecord {
        &self.callable
    }

    pub const fn odr_member_record(&self) -> Option<&OdrMemberRecord> {
        self.odr_member.as_ref()
    }

    pub const fn materialization(&self) -> CallableMaterialization {
        CallableMaterialization::new(
            CallableTemplateOwner::Generated(self.callable.id()),
            CallableMaterializationContext::NoSubstitution,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoroutineDriverIdentityError {
    MissingOdrGroup,
    GeneratedCallable(GeneratedCallableIdentityError),
    OdrMember(OdrMemberIdentityError),
    OdrMemberRecord(HashError),
}

impl fmt::Display for CoroutineDriverIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingOdrGroup => formatter
                .write_str("a specialized coroutine source requires an ODR group for its driver"),
            Self::GeneratedCallable(error) => error.fmt(formatter),
            Self::OdrMember(error) => error.fmt(formatter),
            Self::OdrMemberRecord(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CoroutineDriverIdentityError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CallableApplicationKey, CallableInstantiationOwner, CanonicalIdentifier, ConeIdentity,
        DeclarationScope, DefinitionOwnerChain, PackagePath, PersistentFunctionId,
        SourceDeclarationKey, SourceDeclarationSite, SpecializationKey,
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
            CanonicalIdentifier::new("suspendSource").unwrap(),
            0,
            None,
            Vec::new(),
        ))
        .unwrap()
    }

    #[test]
    fn parameter_free_source_has_one_strong_driver() {
        let source = CallableMaterialization::new(
            CallableTemplateOwner::Function(source_function()),
            CallableMaterializationContext::NoSubstitution,
        );
        let identity = CoroutineDriverIdentity::new(source, None).unwrap();

        assert_eq!(
            identity.callable_record().key(),
            &GeneratedCallableKey::CoroutineDriver {
                source_callable: source
            }
        );
        assert_eq!(identity.odr_member_record(), None);
    }

    #[test]
    fn specialized_source_driver_is_a_callable_group_member() {
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
        let identity = CoroutineDriverIdentity::new(source, Some(group)).unwrap();
        let member = identity.odr_member_record().unwrap();

        assert_eq!(member.key().group(), group);
        assert_eq!(member.key().role(), OdrMemberRole::CallableBody);
        assert_eq!(
            member.key().discriminator(),
            &OdrMemberDiscriminator::GeneratedCallable(identity.callable_record().id())
        );
    }

    #[test]
    fn specialized_source_cannot_lose_its_group() {
        let application = CborIdentityRecord::from_key(CallableApplicationKey::for_function(
            source_function(),
            CallableInstantiationOwner::NoOwner,
        ))
        .unwrap()
        .id();
        let source = CallableMaterialization::new(
            CallableTemplateOwner::Function(source_function()),
            CallableMaterializationContext::Application(application),
        );

        assert_eq!(
            CoroutineDriverIdentity::new(source, None),
            Err(CoroutineDriverIdentityError::MissingOdrGroup)
        );
    }
}
