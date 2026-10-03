//! Persistent identities for exact-result coroutine support callables.

use std::fmt;

use scoop_identity::{
    CallableOdrMemberId, CallableOwner, CborIdentityRecord, Effect, ExactCallableSignature,
    ExactTypeKey, GeneratedCallableIdentityError, GeneratedCallableKey, OdrGroupId,
    OdrMemberDiscriminator, OdrMemberIdentityError, OdrMemberRole, PersistentExactTypeId,
    PersistentGeneratedCallableId, SpecializationKey,
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
    InvalidStartSignature,
    GeneratedCallable(GeneratedCallableIdentityError),
    Root(ExactOwnerRootError),
    OdrMember(OdrMemberIdentityError),
}

impl fmt::Display for CoroutineSupportIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidStartSignature => formatter.write_str(
                "coroutine start must be an ordinary receiver-free two-parameter callable",
            ),
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
    fn source_nominal_start_belongs_to_its_definition_cone() {
        let start = CoroutineStartIdentity::new(&unit(), None, start_signature()).unwrap();
        assert!(matches!(
            start.callable_record().key(),
            GeneratedCallableKey::CoroutineStart { result } if *result == unit().id()
        ));
        assert!(start.root().member_record().is_none());
        assert!(matches!(
            start.signature_record().subject(),
            CallableSignatureSubject::Strong(CallableOwner::Generated(id))
                if id == start.callable_record().id()
        ));
    }

    #[test]
    fn nominal_application_start_belongs_to_the_nominal_group() {
        let origin = generic_origin();
        let arguments = NonEmptyVec::from_first(unit().id(), []);
        let exact = CborIdentityRecord::from_key(ExactTypeKey::NominalApplication {
            origin,
            arguments: arguments.clone(),
        })
        .unwrap();
        let group =
            CborIdentityRecord::from_key(SpecializationKey::Nominal { origin, arguments }).unwrap();
        let start = CoroutineStartIdentity::new(&exact, Some(&group), start_signature()).unwrap();
        let member = start.root().member_record().unwrap();
        assert_eq!(member.key().group(), group.id());
        assert!(matches!(
            start.signature_record().subject(),
            CallableSignatureSubject::Odr(subject) if subject.member() == member.id()
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
