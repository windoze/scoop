//! Persistent identities for continuation adapters generated at suspension sites.

use std::fmt;

use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
    CborIdentityRecord, CoroutineAdapterRole, FieldIdentityError, FieldIdentityKey,
    GeneratedCallableIdentityError, GeneratedCallableKey, GeneratedNominalIdentityError,
    GeneratedNominalKey, OdrGroupId, OdrMemberDiscriminator, OdrMemberId, OdrMemberIdentityError,
    OdrMemberKey, OdrMemberRole, PersistentFieldId, PersistentGeneratedCallableId,
    PersistentTypeId, StructuralDefinitionPath,
};
use scoop_wire::HashError;

type GeneratedCallableRecord =
    CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>;
type GeneratedTypeRecord = CborIdentityRecord<PersistentTypeId, GeneratedNominalKey>;
type FieldRecord = CborIdentityRecord<PersistentFieldId, FieldIdentityKey>;
type OdrMemberRecord = CborIdentityRecord<OdrMemberId, OdrMemberKey>;

/// Persistent field identities whose presence is fixed by the adapter's
/// concrete continuation protocol.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContinuationAdapterStorageIdentity {
    Direct,
    Latched {
        result: Box<FieldRecord>,
        failure: Box<FieldRecord>,
    },
}

impl ContinuationAdapterStorageIdentity {
    pub const fn result_field_record(&self) -> Option<&FieldRecord> {
        match self {
            Self::Direct => None,
            Self::Latched { result, .. } => Some(result),
        }
    }

    pub const fn failure_field_record(&self) -> Option<&FieldRecord> {
        match self {
            Self::Direct => None,
            Self::Latched { failure, .. } => Some(failure),
        }
    }
}

/// One generated success or failure callback at a suspension site.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoroutineAdapterCallableIdentity {
    callable: GeneratedCallableRecord,
    odr_member: Option<OdrMemberRecord>,
}

impl CoroutineAdapterCallableIdentity {
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

/// Complete persistent identity projection of one continuation adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContinuationAdapterIdentity {
    source: CallableMaterialization,
    suspension_site: StructuralDefinitionPath,
    generated_type: GeneratedTypeRecord,
    frame: FieldRecord,
    state: FieldRecord,
    storage: ContinuationAdapterStorageIdentity,
    success: CoroutineAdapterCallableIdentity,
    failure: CoroutineAdapterCallableIdentity,
    odr_member: Option<OdrMemberRecord>,
}

impl ContinuationAdapterIdentity {
    pub fn direct(
        source: CallableMaterialization,
        suspension_site: StructuralDefinitionPath,
        odr_group: Option<OdrGroupId>,
    ) -> Result<Self, ContinuationAdapterIdentityError> {
        Self::new(source, suspension_site, false, odr_group)
    }

    pub fn latched(
        source: CallableMaterialization,
        suspension_site: StructuralDefinitionPath,
        odr_group: Option<OdrGroupId>,
    ) -> Result<Self, ContinuationAdapterIdentityError> {
        Self::new(source, suspension_site, true, odr_group)
    }

    fn new(
        source: CallableMaterialization,
        suspension_site: StructuralDefinitionPath,
        latched: bool,
        odr_group: Option<OdrGroupId>,
    ) -> Result<Self, ContinuationAdapterIdentityError> {
        if source.context() != CallableMaterializationContext::NoSubstitution && odr_group.is_none()
        {
            return Err(ContinuationAdapterIdentityError::MissingOdrGroup);
        }
        let generated_type =
            CborIdentityRecord::from_key(GeneratedNominalKey::ContinuationAdapterEnvironment {
                source_callable: source,
                suspension_site: suspension_site.clone(),
            })
            .map_err(ContinuationAdapterIdentityError::GeneratedType)?;
        let frame = field(FieldIdentityKey::coroutine_adapter_frame(
            generated_type.key(),
        ))?;
        let state = field(FieldIdentityKey::coroutine_adapter_state(
            generated_type.key(),
        ))?;
        let storage = if latched {
            ContinuationAdapterStorageIdentity::Latched {
                result: Box::new(field(FieldIdentityKey::coroutine_adapter_result(
                    generated_type.key(),
                ))?),
                failure: Box::new(field(FieldIdentityKey::coroutine_adapter_failure(
                    generated_type.key(),
                ))?),
            }
        } else {
            ContinuationAdapterStorageIdentity::Direct
        };
        let success = callable(
            source,
            suspension_site.clone(),
            CoroutineAdapterRole::Success,
            odr_group,
        )?;
        let failure = callable(
            source,
            suspension_site.clone(),
            CoroutineAdapterRole::Failure,
            odr_group,
        )?;
        let odr_member = odr_group
            .map(|group| {
                member(
                    group,
                    OdrMemberRole::GeneratedNominal,
                    OdrMemberDiscriminator::GeneratedNominal(generated_type.id()),
                )
            })
            .transpose()?;
        Ok(Self {
            source,
            suspension_site,
            generated_type,
            frame,
            state,
            storage,
            success,
            failure,
            odr_member,
        })
    }

    pub const fn source(&self) -> CallableMaterialization {
        self.source
    }

    pub const fn suspension_site(&self) -> &StructuralDefinitionPath {
        &self.suspension_site
    }

    pub const fn generated_type_record(&self) -> &GeneratedTypeRecord {
        &self.generated_type
    }

    pub const fn frame_field_record(&self) -> &FieldRecord {
        &self.frame
    }

    pub const fn state_field_record(&self) -> &FieldRecord {
        &self.state
    }

    pub const fn storage(&self) -> &ContinuationAdapterStorageIdentity {
        &self.storage
    }

    pub const fn success(&self) -> &CoroutineAdapterCallableIdentity {
        &self.success
    }

    pub const fn failure(&self) -> &CoroutineAdapterCallableIdentity {
        &self.failure
    }

    pub const fn odr_member_record(&self) -> Option<&OdrMemberRecord> {
        self.odr_member.as_ref()
    }
}

fn field(
    key: Result<FieldIdentityKey, FieldIdentityError>,
) -> Result<FieldRecord, ContinuationAdapterIdentityError> {
    CborIdentityRecord::from_key(key.map_err(ContinuationAdapterIdentityError::Field)?)
        .map_err(ContinuationAdapterIdentityError::Field)
}

fn callable(
    source: CallableMaterialization,
    suspension_site: StructuralDefinitionPath,
    role: CoroutineAdapterRole,
    odr_group: Option<OdrGroupId>,
) -> Result<CoroutineAdapterCallableIdentity, ContinuationAdapterIdentityError> {
    let callable = CborIdentityRecord::from_key(GeneratedCallableKey::CoroutineAdapter {
        source_callable: source,
        suspension_site,
        role,
    })
    .map_err(ContinuationAdapterIdentityError::GeneratedCallable)?;
    let odr_member = odr_group
        .map(|group| {
            member(
                group,
                OdrMemberRole::CallableBody,
                OdrMemberDiscriminator::GeneratedCallable(callable.id()),
            )
        })
        .transpose()?;
    Ok(CoroutineAdapterCallableIdentity {
        callable,
        odr_member,
    })
}

fn member(
    group: OdrGroupId,
    role: OdrMemberRole,
    discriminator: OdrMemberDiscriminator,
) -> Result<OdrMemberRecord, ContinuationAdapterIdentityError> {
    let key = OdrMemberKey::new(group, role, discriminator)
        .map_err(ContinuationAdapterIdentityError::OdrMember)?;
    CborIdentityRecord::from_key(key).map_err(ContinuationAdapterIdentityError::OdrMemberRecord)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContinuationAdapterIdentityError {
    MissingOdrGroup,
    GeneratedType(GeneratedNominalIdentityError),
    GeneratedCallable(GeneratedCallableIdentityError),
    Field(FieldIdentityError),
    OdrMember(OdrMemberIdentityError),
    OdrMemberRecord(HashError),
}

impl fmt::Display for ContinuationAdapterIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingOdrGroup => formatter.write_str(
                "a specialized coroutine source requires an ODR group for its continuation adapter",
            ),
            Self::GeneratedType(error) => error.fmt(formatter),
            Self::GeneratedCallable(error) => error.fmt(formatter),
            Self::Field(error) => error.fmt(formatter),
            Self::OdrMember(error) => error.fmt(formatter),
            Self::OdrMemberRecord(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ContinuationAdapterIdentityError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CallableApplicationKey, CallableInstantiationOwner, CanonicalIdentifier, ConeIdentity,
        DeclarationScope, DefinitionOwnerChain, PackagePath, PersistentFunctionId,
        SourceDeclarationKey, SourceDeclarationSite, SpecializationKey,
        StructuralDefinitionSiteRole, StructuralPathSegment,
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

    fn source() -> CallableMaterialization {
        CallableMaterialization::new(
            CallableTemplateOwner::Function(source_function()),
            CallableMaterializationContext::NoSubstitution,
        )
    }

    fn suspension_site(ordinal: u32) -> StructuralDefinitionPath {
        StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::CoroutineTransform, ordinal),
            [],
        )
    }

    #[test]
    fn direct_adapter_has_one_environment_and_two_callable_identities() {
        let source = source();
        let site = suspension_site(2);
        let identity = ContinuationAdapterIdentity::direct(source, site.clone(), None).unwrap();

        assert_eq!(
            identity.generated_type_record().key(),
            &GeneratedNominalKey::ContinuationAdapterEnvironment {
                source_callable: source,
                suspension_site: site.clone(),
            }
        );
        assert_eq!(
            identity.success().callable_record().key(),
            &GeneratedCallableKey::CoroutineAdapter {
                source_callable: source,
                suspension_site: site.clone(),
                role: CoroutineAdapterRole::Success,
            }
        );
        assert_eq!(
            identity.failure().callable_record().key(),
            &GeneratedCallableKey::CoroutineAdapter {
                source_callable: source,
                suspension_site: site,
                role: CoroutineAdapterRole::Failure,
            }
        );
        assert_eq!(
            identity.storage(),
            &ContinuationAdapterStorageIdentity::Direct
        );
        assert_eq!(identity.odr_member_record(), None);
    }

    #[test]
    fn latched_adapter_has_both_latch_field_identities() {
        let identity =
            ContinuationAdapterIdentity::latched(source(), suspension_site(0), None).unwrap();

        let ContinuationAdapterStorageIdentity::Latched { result, failure } = identity.storage()
        else {
            unreachable!("latched construction creates both latch fields")
        };
        assert_eq!(
            result.key(),
            &FieldIdentityKey::coroutine_adapter_result(identity.generated_type_record().key())
                .unwrap()
        );
        assert_eq!(
            failure.key(),
            &FieldIdentityKey::coroutine_adapter_failure(identity.generated_type_record().key())
                .unwrap()
        );
    }

    #[test]
    fn specialized_adapter_source_links_all_three_generated_members_to_its_group() {
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
        let identity =
            ContinuationAdapterIdentity::direct(source, suspension_site(0), Some(group)).unwrap();
        let members = [
            identity.odr_member_record().unwrap(),
            identity.success().odr_member_record().unwrap(),
            identity.failure().odr_member_record().unwrap(),
        ];

        assert!(members.iter().all(|member| member.key().group() == group));
        assert_eq!(members[0].key().role(), OdrMemberRole::GeneratedNominal);
        assert_eq!(members[1].key().role(), OdrMemberRole::CallableBody);
        assert_eq!(members[2].key().role(), OdrMemberRole::CallableBody);
        assert_ne!(members[0].id(), members[1].id());
        assert_ne!(members[1].id(), members[2].id());
    }
}
