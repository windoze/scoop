use std::fmt;

use scoop_identity::{OdrGroupId, OdrMemberId};
use scoop_wire::{Encoder, WireEncode};

use super::{CanonicalMirFoundation, MirFoundationBuildError};
use crate::CallableSignatureSubject;

/// A canonical MIR identity foundation proven to satisfy the M23-3
/// `SingleConeStrong` profile's `RejectAll` ODR policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OdrFreeMirFoundation(CanonicalMirFoundation);

impl OdrFreeMirFoundation {
    pub fn from_module(
        module: &crate::Module,
    ) -> Result<Self, OdrFreeMirFoundationProjectionError> {
        let foundation = CanonicalMirFoundation::from_module(module)
            .map_err(OdrFreeMirFoundationProjectionError::Foundation)?;
        Self::try_new(foundation).map_err(OdrFreeMirFoundationProjectionError::Odr)
    }

    pub fn try_new(foundation: CanonicalMirFoundation) -> Result<Self, OdrFreeMirFoundationError> {
        if let Some(member) = foundation.callable_signatures.iter().find_map(|record| {
            let CallableSignatureSubject::Odr(member) = record.subject() else {
                return None;
            };
            Some(member.member())
        }) {
            return Err(OdrFreeMirFoundationError::CallableSignatureSubject(member));
        }
        if let Some(member) = foundation
            .callback_application_records
            .iter()
            .find_map(|record| {
                let CallableSignatureSubject::Odr(member) = record.managed_adapter() else {
                    return None;
                };
                Some(member.member())
            })
        {
            return Err(OdrFreeMirFoundationError::CallbackAdapterSubject(member));
        }
        if let Some(record) = foundation.odr_groups.first() {
            return Err(OdrFreeMirFoundationError::OdrGroup(record.id()));
        }
        if let Some(record) = foundation.odr_members.first() {
            return Err(OdrFreeMirFoundationError::OdrMember(record.id()));
        }
        Ok(Self(foundation))
    }

    pub const fn as_canonical(&self) -> &CanonicalMirFoundation {
        &self.0
    }

    pub fn into_canonical(self) -> CanonicalMirFoundation {
        self.0
    }
}

impl TryFrom<CanonicalMirFoundation> for OdrFreeMirFoundation {
    type Error = OdrFreeMirFoundationError;

    fn try_from(foundation: CanonicalMirFoundation) -> Result<Self, Self::Error> {
        Self::try_new(foundation)
    }
}

impl WireEncode for OdrFreeMirFoundation {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OdrFreeMirFoundationError {
    CallableSignatureSubject(OdrMemberId),
    CallbackAdapterSubject(OdrMemberId),
    OdrGroup(OdrGroupId),
    OdrMember(OdrMemberId),
}

impl OdrFreeMirFoundationError {
    pub const CODE: &'static str = "SCOOPC_CAPABILITY_ODR_UNAVAILABLE";
}

impl fmt::Display for OdrFreeMirFoundationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CallableSignatureSubject(id) => write!(
                formatter,
                "{}: MIR callable-signature ODR subject {} is forbidden by the SingleConeStrong profile",
                Self::CODE,
                HexIdentity(id.as_array())
            ),
            Self::CallbackAdapterSubject(id) => write!(
                formatter,
                "{}: MIR callback-adapter ODR subject {} is forbidden by the SingleConeStrong profile",
                Self::CODE,
                HexIdentity(id.as_array())
            ),
            Self::OdrGroup(id) => write!(
                formatter,
                "{}: MIR ODR group {} is forbidden by the SingleConeStrong profile",
                Self::CODE,
                HexIdentity(id.as_array())
            ),
            Self::OdrMember(id) => write!(
                formatter,
                "{}: MIR ODR member {} is forbidden by the SingleConeStrong profile",
                Self::CODE,
                HexIdentity(id.as_array())
            ),
        }
    }
}

impl std::error::Error for OdrFreeMirFoundationError {}

#[derive(Debug)]
pub enum OdrFreeMirFoundationProjectionError {
    Foundation(MirFoundationBuildError),
    Odr(OdrFreeMirFoundationError),
}

impl fmt::Display for OdrFreeMirFoundationProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Foundation(error) => error.fmt(formatter),
            Self::Odr(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for OdrFreeMirFoundationProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Foundation(error) => Some(error),
            Self::Odr(error) => Some(error),
        }
    }
}

struct HexIdentity<'a>(&'a [u8; 32]);

impl fmt::Display for HexIdentity<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CallableApplicationKey, CallableInstantiationOwner, CallableMaterializationContext,
        CallbackApplicationKey, CallbackMode, CallbackParameterIndex, CallbackRegistrationKey,
        CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
        DefinitionOwnerChain, Effect, ExactCallableSignature, ExactTypeKey, LexicalCallableParent,
        OdrMemberDiscriminator, OdrMemberKey, OdrMemberRole, PackagePath,
        PersistentCallbackApplicationId, PersistentExactTypeId, PersistentFunctionId,
        PersistentTypeId, SignatureCallableShape, SignatureTypeKey, SourceCAbiFunctionSignature,
        SourceCAbiReturn, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
        SpecializationKey, StructuralDefinitionPath, StructuralDefinitionSiteRole,
        StructuralPathSegment,
    };
    use scoop_wire::encode;

    use super::{OdrFreeMirFoundation, OdrFreeMirFoundationError};
    use crate::{
        CallableSignatureRecord, CallableSignatureSubject, CallbackApplicationRecord,
        CanonicalMirFoundation, ForeignCallbackStorageAbi,
    };

    #[test]
    fn accepts_and_preserves_an_empty_foundation() {
        let canonical = CanonicalMirFoundation::empty();
        let expected = encode(&canonical).unwrap();

        let proven = OdrFreeMirFoundation::try_new(canonical).unwrap();

        assert_eq!(encode(&proven).unwrap(), expected);
        assert_eq!(proven.as_canonical().counts().odr_groups, 0);
    }

    #[test]
    fn rejects_odr_callable_signature_subject() {
        let (_, callable_member) = callable_odr_records();
        let expected = callable_member.member();
        let signature = ExactCallableSignature::new(
            Effect::Ordinary,
            None,
            Vec::new(),
            nominal_exact("Result"),
        );
        let mut canonical = CanonicalMirFoundation::empty();
        canonical
            .set_callable_signatures(vec![CallableSignatureRecord::new(
                CallableSignatureSubject::odr(callable_member),
                signature,
            )])
            .unwrap();

        assert_eq!(
            OdrFreeMirFoundation::try_new(canonical),
            Err(OdrFreeMirFoundationError::CallableSignatureSubject(
                expected
            ))
        );
    }

    #[test]
    fn rejects_odr_callback_adapter_subject_even_without_signature_table_entry() {
        let (_, callable_member) = callable_odr_records();
        let expected = callable_member.member();
        let exact = nominal_exact("CallbackResult");
        let function = source_function("callbackOwner");
        let application = callback_application(function);
        let mut canonical = CanonicalMirFoundation::empty();
        canonical
            .set_callback_application_records(vec![CallbackApplicationRecord::new(
                application,
                CallableSignatureSubject::odr(callable_member),
                ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), exact),
                ForeignCallbackStorageAbi::ClosureResultRootsThrowableToU32,
                CallbackMode::Reusable,
            )])
            .unwrap();

        assert_eq!(
            OdrFreeMirFoundation::try_new(canonical),
            Err(OdrFreeMirFoundationError::CallbackAdapterSubject(expected))
        );
    }

    #[test]
    fn rejects_odr_group_and_member_tables_independently() {
        let (group, member) = callable_odr_records();
        let member_record = CborIdentityRecord::from_key(
            OdrMemberKey::new(
                group.id(),
                OdrMemberRole::CallableBody,
                OdrMemberDiscriminator::CallableApplication(callable_application_identity()),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(member_record.id(), member.member());

        let expected_group = group.id();
        let mut with_group = CanonicalMirFoundation::empty();
        with_group.set_odr_groups(vec![group]).unwrap();
        assert_eq!(
            OdrFreeMirFoundation::try_new(with_group),
            Err(OdrFreeMirFoundationError::OdrGroup(expected_group))
        );

        let expected_member = member_record.id();
        let mut with_member = CanonicalMirFoundation::empty();
        with_member.set_odr_members(vec![member_record]).unwrap();
        assert_eq!(
            OdrFreeMirFoundation::try_new(with_member),
            Err(OdrFreeMirFoundationError::OdrMember(expected_member))
        );
    }

    fn callable_odr_records() -> (
        CborIdentityRecord<scoop_identity::OdrGroupId, SpecializationKey>,
        scoop_identity::CallableOdrMemberId,
    ) {
        let application_key = callable_application_key();
        let application =
            scoop_identity::PersistentCallableApplicationId::from_key(&application_key).unwrap();
        let group = CborIdentityRecord::from_key(SpecializationKey::Callable {
            application: application_key,
        })
        .unwrap();
        let member_key = OdrMemberKey::new(
            group.id(),
            OdrMemberRole::CallableBody,
            OdrMemberDiscriminator::CallableApplication(application),
        )
        .unwrap();
        let member = scoop_identity::CallableOdrMemberId::from_key(&member_key).unwrap();
        (group, member)
    }

    fn callable_application_identity() -> scoop_identity::PersistentCallableApplicationId {
        scoop_identity::PersistentCallableApplicationId::from_key(&callable_application_key())
            .unwrap()
    }

    fn callable_application_key() -> CallableApplicationKey {
        CallableApplicationKey::for_function(
            source_function("specialized"),
            CallableInstantiationOwner::NoOwner,
        )
    }

    fn callback_application(function: PersistentFunctionId) -> PersistentCallbackApplicationId {
        let nominal = source_nominal("CallbackValue");
        let registration = CallbackRegistrationKey::new(
            LexicalCallableParent::function(function),
            StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::CallbackConversion, 0),
                [],
            ),
            SourceCAbiFunctionSignature::new(Vec::new(), SourceCAbiReturn::Void),
            CallbackParameterIndex::new(0),
            SignatureCallableShape::new(
                Effect::Ordinary,
                None,
                Vec::new(),
                SignatureTypeKey::Nominal(nominal),
            ),
            CallbackMode::Reusable,
        );
        let key = CallbackApplicationKey::new(
            &registration,
            CallableMaterializationContext::NoSubstitution,
        )
        .unwrap();
        PersistentCallbackApplicationId::from_key(&key).unwrap()
    }

    fn nominal_exact(name: &str) -> PersistentExactTypeId {
        let nominal = source_nominal(name);
        PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap()
    }

    fn source_nominal(name: &str) -> PersistentTypeId {
        let declaration = SourceDeclarationKey::nominal(
            declaration_site(),
            CanonicalIdentifier::new(name).unwrap(),
            SourceNominalKind::Class,
            0,
        );
        PersistentTypeId::from_source_declaration(&declaration).unwrap()
    }

    fn source_function(name: &str) -> PersistentFunctionId {
        let declaration = SourceDeclarationKey::function(
            declaration_site(),
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            Vec::new(),
        );
        PersistentFunctionId::from_source_declaration(&declaration).unwrap()
    }

    fn declaration_site() -> SourceDeclarationSite {
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap()
    }
}
