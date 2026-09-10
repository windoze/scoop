//! Persistent identities for exact-result coroutine support callables.

use std::fmt;

use scoop_identity::{
    CborIdentityRecord, ContinuationShellRole, ExactTypeKey, GeneratedCallableIdentityError,
    GeneratedCallableKey, OdrGroupId, OdrMemberDiscriminator, OdrMemberRole, PersistentExactTypeId,
    PersistentGeneratedCallableId, SpecializationKey,
};

use crate::{ExactOwnerRoot, ExactOwnerRootError};

type ExactTypeRecord = CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>;
type GeneratedCallableRecord =
    CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>;
type OdrGroupRecord = CborIdentityRecord<OdrGroupId, SpecializationKey>;

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExactCoroutineCallableIdentity {
    result: ExactTypeRecord,
    callable: GeneratedCallableRecord,
    root: ExactOwnerRoot,
}

impl ExactCoroutineCallableIdentity {
    fn new(
        result: &ExactTypeRecord,
        nominal_group: Option<&OdrGroupRecord>,
        key: GeneratedCallableKey,
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
        Ok(Self {
            result: result.clone(),
            callable,
            root,
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
    ) -> Result<Self, CoroutineSupportIdentityError> {
        Ok(Self {
            success: ExactCoroutineCallableIdentity::new(
                result,
                nominal_group,
                GeneratedCallableKey::ContinuationShell {
                    result: result.id(),
                    role: ContinuationShellRole::Success,
                },
            )?,
            failure: ExactCoroutineCallableIdentity::new(
                result,
                nominal_group,
                GeneratedCallableKey::ContinuationShell {
                    result: result.id(),
                    role: ContinuationShellRole::Failure,
                },
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
}

/// The generated `startCoroutine<R>` helper for one exact `R`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoroutineStartIdentity(ExactCoroutineCallableIdentity);

impl CoroutineStartIdentity {
    pub fn new(
        result: &ExactTypeRecord,
        nominal_group: Option<&OdrGroupRecord>,
    ) -> Result<Self, CoroutineSupportIdentityError> {
        Ok(Self(ExactCoroutineCallableIdentity::new(
            result,
            nominal_group,
            GeneratedCallableKey::CoroutineStart {
                result: result.id(),
            },
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoroutineSupportIdentityError {
    GeneratedCallable(GeneratedCallableIdentityError),
    Root(ExactOwnerRootError),
}

impl fmt::Display for CoroutineSupportIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GeneratedCallable(error) => error.fmt(formatter),
            Self::Root(error) => error.fmt(formatter),
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
        let shells = ContinuationShellIdentity::new(&unit(), None).unwrap();
        let start = CoroutineStartIdentity::new(&unit(), None).unwrap();
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
        let shells = ContinuationShellIdentity::new(&exact, Some(&group)).unwrap();
        let start = CoroutineStartIdentity::new(&exact, Some(&group)).unwrap();
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
    }

    #[test]
    fn structural_support_creates_the_exact_group() {
        let exact = CborIdentityRecord::from_key(ExactTypeKey::Tuple(NonEmptyVec::from_first(
            unit().id(),
            [],
        )))
        .unwrap();
        let start = CoroutineStartIdentity::new(&exact, None).unwrap();
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
