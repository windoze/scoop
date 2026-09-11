//! Persistent identities for source-owned coroutine state-machine artifacts.

use std::fmt;

use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableOdrMemberId, CallableOwner,
    CallableTemplateOwner, CborIdentityRecord, Effect, ExactCallableSignature, FieldIdentityError,
    FieldIdentityKey, GeneratedCallableIdentityError, GeneratedCallableKey,
    GeneratedNominalIdentityError, GeneratedNominalKey, LocalValueKey, OdrGroupId,
    OdrMemberDiscriminator, OdrMemberId, OdrMemberIdentityError, OdrMemberKey, OdrMemberRole,
    PersistentFieldId, PersistentGeneratedCallableId, PersistentLocalValueId, PersistentTypeId,
};
use scoop_wire::HashError;

use crate::{CallableSignatureRecord, CallableSignatureSubject};

type GeneratedCallableRecord =
    CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>;
type OdrMemberRecord = CborIdentityRecord<OdrMemberId, OdrMemberKey>;
type GeneratedTypeRecord = CborIdentityRecord<PersistentTypeId, GeneratedNominalKey>;
type LocalValueRecord = CborIdentityRecord<PersistentLocalValueId, LocalValueKey>;
type FieldRecord = CborIdentityRecord<PersistentFieldId, FieldIdentityKey>;

/// Complete persistent identity of the state-machine driver generated for one
/// suspend callable that can actually suspend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoroutineDriverIdentity {
    source: CallableMaterialization,
    callable: GeneratedCallableRecord,
    odr_member: Option<OdrMemberRecord>,
    signature: CallableSignatureRecord,
}

impl CoroutineDriverIdentity {
    pub fn new(
        source: CallableMaterialization,
        odr_group: Option<OdrGroupId>,
        signature: ExactCallableSignature,
    ) -> Result<Self, CoroutineDriverIdentityError> {
        if signature.effect() != Effect::Suspend {
            return Err(CoroutineDriverIdentityError::ExpectedSuspendSignature);
        }
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
        let subject = match &odr_member {
            Some(member) => CallableSignatureSubject::odr(
                CallableOdrMemberId::from_key(member.key())
                    .map_err(CoroutineDriverIdentityError::OdrMember)?,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoroutineDriverIdentityError {
    ExpectedSuspendSignature,
    MissingOdrGroup,
    GeneratedCallable(GeneratedCallableIdentityError),
    OdrMember(OdrMemberIdentityError),
    OdrMemberRecord(HashError),
}

impl fmt::Display for CoroutineDriverIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedSuspendSignature => {
                formatter.write_str("a coroutine driver requires a suspend logical signature")
            }
            Self::MissingOdrGroup => formatter
                .write_str("a specialized coroutine source requires an ODR group for its driver"),
            Self::GeneratedCallable(error) => error.fmt(formatter),
            Self::OdrMember(error) => error.fmt(formatter),
            Self::OdrMemberRecord(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CoroutineDriverIdentityError {}

/// Persistent identity of one source value saved in a coroutine frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoroutineFrameSavedFieldIdentity {
    value: LocalValueRecord,
    field: FieldRecord,
}

impl CoroutineFrameSavedFieldIdentity {
    pub const fn value_record(&self) -> &LocalValueRecord {
        &self.value
    }

    pub const fn field_record(&self) -> &FieldRecord {
        &self.field
    }
}

/// Complete persistent identity projection of one generated coroutine frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoroutineFrameIdentity {
    source: CallableMaterialization,
    generated_type: GeneratedTypeRecord,
    state: FieldRecord,
    completion: FieldRecord,
    saved: Vec<CoroutineFrameSavedFieldIdentity>,
    failure: FieldRecord,
    odr_member: Option<OdrMemberRecord>,
}

impl CoroutineFrameIdentity {
    pub fn new(
        source: CallableMaterialization,
        saved: Vec<LocalValueRecord>,
        odr_group: Option<OdrGroupId>,
    ) -> Result<Self, CoroutineFrameIdentityError> {
        if source.context() != CallableMaterializationContext::NoSubstitution && odr_group.is_none()
        {
            return Err(CoroutineFrameIdentityError::MissingOdrGroup);
        }
        if saved
            .iter()
            .any(|value| value.key().owner().context() != source.context())
        {
            return Err(CoroutineFrameIdentityError::MaterializationContextMismatch);
        }

        let generated_type = CborIdentityRecord::from_key(GeneratedNominalKey::CoroutineFrame {
            source_callable: source,
        })
        .map_err(CoroutineFrameIdentityError::GeneratedType)?;
        let state = field(FieldIdentityKey::coroutine_frame_state(
            generated_type.key(),
        ))?;
        let completion = field(FieldIdentityKey::coroutine_frame_completion(
            generated_type.key(),
        ))?;
        let failure = field(FieldIdentityKey::coroutine_frame_failure(
            generated_type.key(),
        ))?;
        let mut saved = saved
            .into_iter()
            .map(|value| {
                let field = field(FieldIdentityKey::coroutine_frame_saved(
                    generated_type.key(),
                    value.id(),
                ))?;
                Ok(CoroutineFrameSavedFieldIdentity { value, field })
            })
            .collect::<Result<Vec<_>, CoroutineFrameIdentityError>>()?;
        saved.sort_by_key(|entry| entry.value.id());
        if saved
            .windows(2)
            .any(|pair| pair[0].value.id() == pair[1].value.id())
        {
            return Err(CoroutineFrameIdentityError::DuplicateSavedValue);
        }
        let odr_member = odr_group
            .map(|group| {
                let key = OdrMemberKey::new(
                    group,
                    OdrMemberRole::GeneratedNominal,
                    OdrMemberDiscriminator::GeneratedNominal(generated_type.id()),
                )
                .map_err(CoroutineFrameIdentityError::OdrMember)?;
                CborIdentityRecord::from_key(key)
                    .map_err(CoroutineFrameIdentityError::OdrMemberRecord)
            })
            .transpose()?;
        Ok(Self {
            source,
            generated_type,
            state,
            completion,
            saved,
            failure,
            odr_member,
        })
    }

    pub const fn source(&self) -> CallableMaterialization {
        self.source
    }

    pub const fn generated_type_record(&self) -> &GeneratedTypeRecord {
        &self.generated_type
    }

    pub const fn state_field_record(&self) -> &FieldRecord {
        &self.state
    }

    pub const fn completion_field_record(&self) -> &FieldRecord {
        &self.completion
    }

    pub fn saved_fields(&self) -> &[CoroutineFrameSavedFieldIdentity] {
        &self.saved
    }

    pub const fn failure_field_record(&self) -> &FieldRecord {
        &self.failure
    }

    pub const fn odr_member_record(&self) -> Option<&OdrMemberRecord> {
        self.odr_member.as_ref()
    }

    pub fn physical_saved_index(&self, value: PersistentLocalValueId) -> Option<u32> {
        self.saved
            .binary_search_by_key(&value, |entry| entry.value.id())
            .ok()
            .and_then(|index| u32::try_from(index).ok())
    }
}

fn field(
    key: Result<FieldIdentityKey, FieldIdentityError>,
) -> Result<FieldRecord, CoroutineFrameIdentityError> {
    CborIdentityRecord::from_key(key.map_err(CoroutineFrameIdentityError::Field)?)
        .map_err(CoroutineFrameIdentityError::Field)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoroutineFrameIdentityError {
    MissingOdrGroup,
    MaterializationContextMismatch,
    DuplicateSavedValue,
    GeneratedType(GeneratedNominalIdentityError),
    Field(FieldIdentityError),
    OdrMember(OdrMemberIdentityError),
    OdrMemberRecord(HashError),
}

impl fmt::Display for CoroutineFrameIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingOdrGroup => formatter
                .write_str("a specialized coroutine source requires an ODR group for its frame"),
            Self::MaterializationContextMismatch => formatter
                .write_str("coroutine frame and saved values must share a materialization context"),
            Self::DuplicateSavedValue => {
                formatter.write_str("a coroutine frame cannot save the same value twice")
            }
            Self::GeneratedType(error) => error.fmt(formatter),
            Self::Field(error) => error.fmt(formatter),
            Self::OdrMember(error) => error.fmt(formatter),
            Self::OdrMemberRecord(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CoroutineFrameIdentityError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CallableApplicationKey, CallableInstantiationOwner, CanonicalIdentifier, ConeIdentity,
        CoreBuiltinNominal, DeclarationScope, DefinitionOwnerChain, ExactTypeKey,
        LocalValueSelector, PackagePath, PersistentFunctionId, SourceDeclarationKey,
        SourceDeclarationSite, SpecializationKey,
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

    fn saved_value(owner: CallableMaterialization, declaration_index: u32) -> LocalValueRecord {
        CborIdentityRecord::from_key(LocalValueKey::new(
            owner,
            LocalValueSelector::Parameter { declaration_index },
        ))
        .unwrap()
    }

    fn suspend_signature() -> ExactCallableSignature {
        let unit = CborIdentityRecord::from_key(ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap()
        .id();
        ExactCallableSignature::new(Effect::Suspend, None, Vec::new(), unit)
    }

    #[test]
    fn parameter_free_source_has_one_strong_driver() {
        let source = CallableMaterialization::new(
            CallableTemplateOwner::Function(source_function()),
            CallableMaterializationContext::NoSubstitution,
        );
        let signature = suspend_signature();
        let identity = CoroutineDriverIdentity::new(source, None, signature.clone()).unwrap();

        assert_eq!(
            identity.callable_record().key(),
            &GeneratedCallableKey::CoroutineDriver {
                source_callable: source
            }
        );
        assert_eq!(identity.odr_member_record(), None);
        assert_eq!(identity.signature_record().signature(), &signature);
        assert!(matches!(
            identity.signature_record().subject(),
            CallableSignatureSubject::Strong(CallableOwner::Generated(id))
                if id == identity.callable_record().id()
        ));
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
        let identity =
            CoroutineDriverIdentity::new(source, Some(group), suspend_signature()).unwrap();
        let member = identity.odr_member_record().unwrap();

        assert_eq!(member.key().group(), group);
        assert_eq!(member.key().role(), OdrMemberRole::CallableBody);
        assert_eq!(
            member.key().discriminator(),
            &OdrMemberDiscriminator::GeneratedCallable(identity.callable_record().id())
        );
        assert!(matches!(
            identity.signature_record().subject(),
            CallableSignatureSubject::Odr(subject) if subject.member() == member.id()
        ));
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
            CoroutineDriverIdentity::new(source, None, suspend_signature()),
            Err(CoroutineDriverIdentityError::MissingOdrGroup)
        );
    }

    #[test]
    fn driver_rejects_an_ordinary_logical_signature() {
        let source = CallableMaterialization::new(
            CallableTemplateOwner::Function(source_function()),
            CallableMaterializationContext::NoSubstitution,
        );
        let result = suspend_signature().result();

        assert_eq!(
            CoroutineDriverIdentity::new(
                source,
                None,
                ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), result),
            ),
            Err(CoroutineDriverIdentityError::ExpectedSuspendSignature)
        );
    }

    #[test]
    fn frame_fields_follow_persistent_value_order() {
        let source = CallableMaterialization::new(
            CallableTemplateOwner::Function(source_function()),
            CallableMaterializationContext::NoSubstitution,
        );
        let first = saved_value(source, 0);
        let second = saved_value(source, 1);
        let identity =
            CoroutineFrameIdentity::new(source, vec![second.clone(), first.clone()], None).unwrap();

        assert_eq!(
            identity.generated_type_record().key(),
            &GeneratedNominalKey::CoroutineFrame {
                source_callable: source
            }
        );
        assert_eq!(identity.odr_member_record(), None);
        assert_eq!(
            identity
                .saved_fields()
                .iter()
                .map(|field| field.value_record().id())
                .collect::<Vec<_>>(),
            if first.id() < second.id() {
                vec![first.id(), second.id()]
            } else {
                vec![second.id(), first.id()]
            }
        );
        for field in identity.saved_fields() {
            assert_eq!(
                field.field_record().key(),
                &FieldIdentityKey::coroutine_frame_saved(
                    identity.generated_type_record().key(),
                    field.value_record().id(),
                )
                .unwrap()
            );
        }
    }

    #[test]
    fn frame_rejects_a_saved_value_from_another_materialization() {
        let source = CallableMaterialization::new(
            CallableTemplateOwner::Function(source_function()),
            CallableMaterializationContext::NoSubstitution,
        );
        let application = CborIdentityRecord::from_key(CallableApplicationKey::for_function(
            source_function(),
            CallableInstantiationOwner::NoOwner,
        ))
        .unwrap()
        .id();
        let other = CallableMaterialization::new(
            CallableTemplateOwner::Function(source_function()),
            CallableMaterializationContext::Application(application),
        );

        assert_eq!(
            CoroutineFrameIdentity::new(source, vec![saved_value(other, 0)], None).unwrap_err(),
            CoroutineFrameIdentityError::MaterializationContextMismatch
        );
    }
}
