//! Persistent identities for exact-result coroutine support callables.

use std::fmt;

use scoop_identity::{
    CallableOdrMemberId, CallableOwner, CborIdentityRecord, ContinuationShellRole, Effect,
    ExactCallableSignature, ExactTypeKey, GeneratedCallableIdentityError, GeneratedCallableKey,
    OdrGroupId, OdrMemberDiscriminator, OdrMemberIdentityError, OdrMemberRole,
    PersistentExactTypeId, PersistentGeneratedCallableId, SpecializationKey,
};

use crate::{
    CallableSignatureRecord, CallableSignatureSubject, ExactOwnerRoot, ExactOwnerRootError,
};

type ExactTypeRecord = CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>;
type GeneratedCallableRecord =
    CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>;
type OdrGroupRecord = CborIdentityRecord<OdrGroupId, SpecializationKey>;

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExactCoroutineCallableIdentity {
    result: ExactTypeRecord,
    callable: GeneratedCallableRecord,
    root: ExactOwnerRoot,
    signature: CallableSignatureRecord,
}

impl ExactCoroutineCallableIdentity {
    fn new(
        result: &ExactTypeRecord,
        nominal_group: Option<&OdrGroupRecord>,
        key: GeneratedCallableKey,
        signature: ExactCallableSignature,
    ) -> Result<Self, CoroutineSupportIdentityError> {
        let callable = CborIdentityRecord::from_key(key)
            .map_err(CoroutineSupportIdentityError::GeneratedCallable)?;
        let root = ExactOwnerRoot::for_member(
            result,
            nominal_group,
            OdrMemberRole::CallableBody,
            OdrMemberDiscriminator::GeneratedCallable(callable.id()),
        )
        .map_err(CoroutineSupportIdentityError::Root)?;
        let subject = match root.member_record() {
            Some(member) => CallableSignatureSubject::odr(
                CallableOdrMemberId::from_key(member.key())
                    .map_err(CoroutineSupportIdentityError::OdrMember)?,
            ),
            None => CallableSignatureSubject::strong(CallableOwner::Generated(callable.id())),
        };
        Ok(Self {
            result: result.clone(),
            callable,
            root,
            signature: CallableSignatureRecord::new(subject, signature),
        })
    }
}

/// The two generated `Continuation<R>` dispatch shells for one exact `R`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContinuationShellIdentity {
    success: ExactCoroutineCallableIdentity,
    failure: ExactCoroutineCallableIdentity,
}

impl ContinuationShellIdentity {
    pub fn new(
        result: &ExactTypeRecord,
        nominal_group: Option<&OdrGroupRecord>,
        success_signature: ExactCallableSignature,
        failure_signature: ExactCallableSignature,
    ) -> Result<Self, CoroutineSupportIdentityError> {
        if success_signature.effect() != Effect::Ordinary
            || !success_signature.receiver().is_present()
            || success_signature.parameters() != [result.id()]
        {
            return Err(CoroutineSupportIdentityError::InvalidSuccessSignature);
        }
        if failure_signature.effect() != Effect::Ordinary
            || failure_signature.receiver() != success_signature.receiver()
            || failure_signature.parameters().len() != 1
            || failure_signature.result() != success_signature.result()
        {
            return Err(CoroutineSupportIdentityError::InvalidFailureSignature);
        }
        Ok(Self {
            success: ExactCoroutineCallableIdentity::new(
                result,
                nominal_group,
                GeneratedCallableKey::ContinuationShell {
                    result: result.id(),
                    role: ContinuationShellRole::Success,
                },
                success_signature,
            )?,
            failure: ExactCoroutineCallableIdentity::new(
                result,
                nominal_group,
                GeneratedCallableKey::ContinuationShell {
                    result: result.id(),
                    role: ContinuationShellRole::Failure,
                },
                failure_signature,
            )?,
        })
    }

    pub const fn result_record(&self) -> &ExactTypeRecord {
        &self.success.result
    }

    pub const fn success_callable_record(&self) -> &GeneratedCallableRecord {
        &self.success.callable
    }

    pub const fn failure_callable_record(&self) -> &GeneratedCallableRecord {
        &self.failure.callable
    }

    pub const fn success_root(&self) -> &ExactOwnerRoot {
        &self.success.root
    }

    pub const fn failure_root(&self) -> &ExactOwnerRoot {
        &self.failure.root
    }

    pub const fn success_signature_record(&self) -> &CallableSignatureRecord {
        &self.success.signature
    }

    pub const fn failure_signature_record(&self) -> &CallableSignatureRecord {
        &self.failure.signature
    }
}

/// The generated `startCoroutine<R>` helper for one exact `R`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoroutineStartIdentity(ExactCoroutineCallableIdentity);

impl CoroutineStartIdentity {
    pub fn new(
        result: &ExactTypeRecord,
        nominal_group: Option<&OdrGroupRecord>,
        signature: ExactCallableSignature,
    ) -> Result<Self, CoroutineSupportIdentityError> {
        if signature.effect() != Effect::Ordinary
            || signature.receiver().is_present()
            || signature.parameters().len() != 2
        {
            return Err(CoroutineSupportIdentityError::InvalidStartSignature);
        }
        Ok(Self(ExactCoroutineCallableIdentity::new(
            result,
            nominal_group,
            GeneratedCallableKey::CoroutineStart {
                result: result.id(),
            },
            signature,
        )?))
    }

    pub const fn result_record(&self) -> &ExactTypeRecord {
        &self.0.result
    }

    pub const fn callable_record(&self) -> &GeneratedCallableRecord {
        &self.0.callable
    }

    pub const fn root(&self) -> &ExactOwnerRoot {
        &self.0.root
    }

    pub const fn signature_record(&self) -> &CallableSignatureRecord {
        &self.0.signature
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoroutineSupportIdentityError {
    InvalidSuccessSignature,
    InvalidFailureSignature,
    InvalidStartSignature,
    GeneratedCallable(GeneratedCallableIdentityError),
    Root(ExactOwnerRootError),
    OdrMember(OdrMemberIdentityError),
}

impl fmt::Display for CoroutineSupportIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSuccessSignature => formatter
                .write_str("continuation success must receive the exact result as an ordinary method"),
            Self::InvalidFailureSignature => formatter.write_str(
                "continuation failure must share the success receiver and result with one parameter",
            ),
            Self::InvalidStartSignature => formatter
                .write_str("coroutine start must be an ordinary receiver-free two-parameter callable"),
            Self::GeneratedCallable(error) => error.fmt(formatter),
            Self::Root(error) => error.fmt(formatter),
            Self::OdrMember(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CoroutineSupportIdentityError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CanonicalIdentifier, ConeIdentity, CoreBuiltinNominal, DeclarationScope,
        DefinitionOwnerChain, NonEmptyVec, PackagePath, PersistentGenericTypeId,
        SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
    };

    use super::*;

    fn unit() -> ExactTypeRecord {
        CborIdentityRecord::from_key(ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap()
    }

    fn any() -> ExactTypeRecord {
        CborIdentityRecord::from_key(ExactTypeKey::Nominal(
            CoreBuiltinNominal::Any.identity_record().id(),
        ))
        .unwrap()
    }

    fn shell_signatures(
        result: PersistentExactTypeId,
    ) -> (ExactCallableSignature, ExactCallableSignature) {
        let receiver = Some(any().id());
        (
            ExactCallableSignature::new(Effect::Ordinary, receiver, vec![result], unit().id()),
            ExactCallableSignature::new(Effect::Ordinary, receiver, vec![any().id()], unit().id()),
        )
    }

    fn start_signature() -> ExactCallableSignature {
        ExactCallableSignature::new(
            Effect::Ordinary,
            None,
            vec![any().id(), any().id()],
            unit().id(),
        )
    }

    fn generic_origin() -> PersistentGenericTypeId {
        let site = SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        PersistentGenericTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
            site,
            CanonicalIdentifier::new("Result").unwrap(),
            SourceNominalKind::Struct,
            1,
        ))
        .unwrap()
    }

    #[test]
    fn source_nominal_support_has_three_distinct_strong_callables() {
        let (success, failure) = shell_signatures(unit().id());
        let shells =
            ContinuationShellIdentity::new(&unit(), None, success.clone(), failure.clone())
                .unwrap();
        let start = CoroutineStartIdentity::new(&unit(), None, start_signature()).unwrap();
        assert!(matches!(
            shells.success_callable_record().key(),
            GeneratedCallableKey::ContinuationShell {
                result,
                role: ContinuationShellRole::Success,
            } if *result == unit().id()
        ));
        assert!(matches!(
            shells.failure_callable_record().key(),
            GeneratedCallableKey::ContinuationShell {
                role: ContinuationShellRole::Failure,
                ..
            }
        ));
        assert!(matches!(
            start.callable_record().key(),
            GeneratedCallableKey::CoroutineStart { result } if *result == unit().id()
        ));
        assert_ne!(
            shells.success_callable_record().id(),
            shells.failure_callable_record().id()
        );
        assert_ne!(
            shells.success_callable_record().id(),
            start.callable_record().id()
        );
        assert!(shells.success_root().member_record().is_none());
        assert!(shells.failure_root().member_record().is_none());
        assert!(start.root().member_record().is_none());
        assert_eq!(shells.success_signature_record().signature(), &success);
        assert_eq!(shells.failure_signature_record().signature(), &failure);
        assert!(matches!(
            shells.success_signature_record().subject(),
            CallableSignatureSubject::Strong(CallableOwner::Generated(id))
                if id == shells.success_callable_record().id()
        ));
        assert!(matches!(
            start.signature_record().subject(),
            CallableSignatureSubject::Strong(CallableOwner::Generated(id))
                if id == start.callable_record().id()
        ));
    }

    #[test]
    fn nominal_application_support_reuses_one_group_with_distinct_members() {
        let origin = generic_origin();
        let arguments = NonEmptyVec::from_first(unit().id(), []);
        let exact = CborIdentityRecord::from_key(ExactTypeKey::NominalApplication {
            origin,
            arguments: arguments.clone(),
        })
        .unwrap();
        let group =
            CborIdentityRecord::from_key(SpecializationKey::Nominal { origin, arguments }).unwrap();
        let (success, failure) = shell_signatures(exact.id());
        let shells =
            ContinuationShellIdentity::new(&exact, Some(&group), success, failure).unwrap();
        let start = CoroutineStartIdentity::new(&exact, Some(&group), start_signature()).unwrap();
        let members = [
            shells.success_root().member_record().unwrap(),
            shells.failure_root().member_record().unwrap(),
            start.root().member_record().unwrap(),
        ];
        assert!(
            members
                .iter()
                .all(|member| member.key().group() == group.id())
        );
        assert_ne!(members[0].id(), members[1].id());
        assert_ne!(members[0].id(), members[2].id());
        assert_ne!(members[1].id(), members[2].id());
        assert!(matches!(
            shells.success_signature_record().subject(),
            CallableSignatureSubject::Odr(member) if member.member() == members[0].id()
        ));
        assert!(matches!(
            start.signature_record().subject(),
            CallableSignatureSubject::Odr(member) if member.member() == members[2].id()
        ));
    }

    #[test]
    fn structural_support_creates_the_exact_group() {
        let exact = CborIdentityRecord::from_key(ExactTypeKey::Tuple(NonEmptyVec::from_first(
            unit().id(),
            [],
        )))
        .unwrap();
        let start = CoroutineStartIdentity::new(&exact, None, start_signature()).unwrap();
        let ExactOwnerRoot::Structural(root) = start.root() else {
            panic!("tuple support uses a structural exact root")
        };
        assert_eq!(
            root.group_record().key(),
            &SpecializationKey::StructuralType {
                exact_type: exact.id(),
            }
        );
    }
}
