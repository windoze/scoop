use std::fmt;

use scoop_identity::{
    DecodedCallableBodyKey, DecodedCallableBodyKeyKind, LinkageClass, OdrGroupId, OdrMemberId,
    PersistentCallableBodyId, PersistentSymbolKey, PersistentSymbolKind,
};
use scoop_wire::{Encoder, RuntimeDecodeError, WireEncode, decode_runtime};

use super::{CanonicalLirFoundation, LirFoundationBuildError};

/// A canonical LIR identity foundation proven to satisfy the M23-3
/// `SingleConeStrong` profile's `RejectAll` ODR policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OdrFreeLirFoundation(CanonicalLirFoundation);

impl OdrFreeLirFoundation {
    pub fn from_module(
        module: &crate::Module,
    ) -> Result<Self, OdrFreeLirFoundationProjectionError> {
        let foundation = CanonicalLirFoundation::from_module(module)
            .map_err(OdrFreeLirFoundationProjectionError::Foundation)?;
        Self::try_new(foundation).map_err(OdrFreeLirFoundationProjectionError::Odr)
    }

    pub fn try_new(foundation: CanonicalLirFoundation) -> Result<Self, OdrFreeLirFoundationError> {
        if let Some(record) = foundation.odr_groups.first() {
            return Err(OdrFreeLirFoundationError::OdrGroup(record.id()));
        }
        if let Some(record) = foundation.odr_members.first() {
            return Err(OdrFreeLirFoundationError::OdrMember(record.id()));
        }
        for record in &foundation.callable_bodies {
            let key =
                decode_runtime::<DecodedCallableBodyKey>(record.key_bytes()).map_err(|error| {
                    OdrFreeLirFoundationError::InvalidCallableBodyKey {
                        body: record.id(),
                        error,
                    }
                })?;
            if matches!(key.kind(), DecodedCallableBodyKeyKind::Odr(_)) {
                return Err(OdrFreeLirFoundationError::OdrCallableBody(record.id()));
            }
        }
        if let Some(request) = foundation
            .symbol_requests
            .requests()
            .iter()
            .find(|request| {
                request.linkage() == LinkageClass::OdrWeak
                    || request.key().kind() == PersistentSymbolKind::OdrMember
            })
        {
            return Err(OdrFreeLirFoundationError::OdrSymbolRequest(request.key()));
        }
        Ok(Self(foundation))
    }

    pub const fn as_canonical(&self) -> &CanonicalLirFoundation {
        &self.0
    }

    pub fn into_canonical(self) -> CanonicalLirFoundation {
        self.0
    }
}

impl TryFrom<CanonicalLirFoundation> for OdrFreeLirFoundation {
    type Error = OdrFreeLirFoundationError;

    fn try_from(foundation: CanonicalLirFoundation) -> Result<Self, Self::Error> {
        Self::try_new(foundation)
    }
}

impl WireEncode for OdrFreeLirFoundation {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OdrFreeLirFoundationError {
    OdrGroup(OdrGroupId),
    OdrMember(OdrMemberId),
    OdrCallableBody(PersistentCallableBodyId),
    OdrSymbolRequest(PersistentSymbolKey),
    InvalidCallableBodyKey {
        body: PersistentCallableBodyId,
        error: RuntimeDecodeError,
    },
}

impl OdrFreeLirFoundationError {
    pub const CODE: &'static str = "SCOOPC_CAPABILITY_ODR_UNAVAILABLE";
}

impl fmt::Display for OdrFreeLirFoundationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OdrGroup(id) => write!(
                formatter,
                "{}: LIR ODR group {} is forbidden by the SingleConeStrong profile",
                Self::CODE,
                HexIdentity(id.as_array())
            ),
            Self::OdrMember(id) => write!(
                formatter,
                "{}: LIR ODR member {} is forbidden by the SingleConeStrong profile",
                Self::CODE,
                HexIdentity(id.as_array())
            ),
            Self::OdrCallableBody(id) => write!(
                formatter,
                "{}: LIR ODR callable body {} is forbidden by the SingleConeStrong profile",
                Self::CODE,
                HexIdentity(id.as_array())
            ),
            Self::OdrSymbolRequest(key) => write!(
                formatter,
                "{}: LIR {:?} symbol request is forbidden by the SingleConeStrong profile",
                Self::CODE,
                key.kind()
            ),
            Self::InvalidCallableBodyKey { body, error } => write!(
                formatter,
                "invalid canonical LIR callable body {} while applying the SingleConeStrong profile: {error}",
                HexIdentity(body.as_array())
            ),
        }
    }
}

impl std::error::Error for OdrFreeLirFoundationError {}

#[derive(Debug)]
pub enum OdrFreeLirFoundationProjectionError {
    Foundation(LirFoundationBuildError),
    Odr(OdrFreeLirFoundationError),
}

impl fmt::Display for OdrFreeLirFoundationProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Foundation(error) => error.fmt(formatter),
            Self::Odr(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for OdrFreeLirFoundationProjectionError {
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
        CallableBodyKey, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
        DefinitionOwnerChain, ExactTypeKey, GeneratedCallableKey, LinkageClass,
        OdrMemberDiscriminator, OdrMemberKey, OdrMemberRole, PackagePath, PersistentCallableBodyId,
        PersistentExactTypeId, PersistentFunctionId, PersistentSymbolKey, PersistentSymbolRequest,
        PersistentSymbolRequestTable, PersistentTypeId, RuntimeIdentityRecord,
        SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, SpecializationKey,
        StrongCallableDefinitionOwner,
    };
    use scoop_wire::encode;

    use super::{OdrFreeLirFoundation, OdrFreeLirFoundationError};
    use crate::CanonicalLirFoundation;

    #[test]
    fn accepts_and_preserves_strong_bodies_and_symbols() {
        let body = strong_body("entry");
        let request = PersistentSymbolRequest::new(
            PersistentSymbolKey::CallableBody(body.id()),
            LinkageClass::ConeStrong,
        )
        .unwrap();
        let mut canonical = CanonicalLirFoundation::empty();
        canonical.set_callable_bodies(vec![body]).unwrap();
        canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![request]).unwrap());
        let expected = encode(&canonical).unwrap();

        let proven = OdrFreeLirFoundation::try_new(canonical).unwrap();

        assert_eq!(encode(&proven).unwrap(), expected);
        assert_eq!(proven.as_canonical().counts().odr_members, 0);
    }

    #[test]
    fn rejects_odr_group_and_member_tables_independently() {
        let (group, member) = odr_records();
        let expected_group = group.id();
        let mut with_group = CanonicalLirFoundation::empty();
        with_group.set_odr_groups(vec![group]).unwrap();
        assert_eq!(
            OdrFreeLirFoundation::try_new(with_group),
            Err(OdrFreeLirFoundationError::OdrGroup(expected_group))
        );

        let expected_member = member.id();
        let mut with_member = CanonicalLirFoundation::empty();
        with_member.set_odr_members(vec![member]).unwrap();
        assert_eq!(
            OdrFreeLirFoundation::try_new(with_member),
            Err(OdrFreeLirFoundationError::OdrMember(expected_member))
        );
    }

    #[test]
    fn rejects_odr_callable_body_without_relying_on_odr_tables() {
        let member = callable_odr_member();
        let body = RuntimeIdentityRecord::from_key(&CallableBodyKey::odr(member)).unwrap();
        let expected = body.id();
        let mut canonical = CanonicalLirFoundation::empty();
        canonical.set_callable_bodies(vec![body]).unwrap();

        assert_eq!(
            OdrFreeLirFoundation::try_new(canonical),
            Err(OdrFreeLirFoundationError::OdrCallableBody(expected))
        );
    }

    #[test]
    fn rejects_odr_weak_symbol_without_relying_on_body_or_odr_tables() {
        let body = strong_body("weakBody");
        let key = PersistentSymbolKey::CallableBody(body.id());
        let request = PersistentSymbolRequest::new(key, LinkageClass::OdrWeak).unwrap();
        let mut canonical = CanonicalLirFoundation::empty();
        canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![request]).unwrap());

        assert_eq!(
            OdrFreeLirFoundation::try_new(canonical),
            Err(OdrFreeLirFoundationError::OdrSymbolRequest(key))
        );
    }

    #[test]
    fn rejects_explicit_odr_member_symbol_even_with_its_required_linkage() {
        let member = callable_odr_member().member();
        let key = PersistentSymbolKey::OdrMember(member);
        let request = PersistentSymbolRequest::new(key, LinkageClass::OdrWeak).unwrap();
        let mut canonical = CanonicalLirFoundation::empty();
        canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![request]).unwrap());

        assert_eq!(
            OdrFreeLirFoundation::try_new(canonical),
            Err(OdrFreeLirFoundationError::OdrSymbolRequest(key))
        );
    }

    fn odr_records() -> (
        CborIdentityRecord<scoop_identity::OdrGroupId, SpecializationKey>,
        CborIdentityRecord<scoop_identity::OdrMemberId, OdrMemberKey>,
    ) {
        let exact = nominal_exact("Shape");
        let group =
            CborIdentityRecord::from_key(SpecializationKey::StructuralType { exact_type: exact })
                .unwrap();
        let member = CborIdentityRecord::from_key(
            OdrMemberKey::new(
                group.id(),
                OdrMemberRole::TypeDescriptor,
                OdrMemberDiscriminator::ExactType(exact),
            )
            .unwrap(),
        )
        .unwrap();
        (group, member)
    }

    fn callable_odr_member() -> scoop_identity::CallableOdrMemberId {
        let exact = nominal_exact("CallableShape");
        let generated =
            CborIdentityRecord::from_key(GeneratedCallableKey::CoroutineStart { result: exact })
                .unwrap();
        let group = scoop_identity::OdrGroupId::from_key(&SpecializationKey::StructuralType {
            exact_type: exact,
        })
        .unwrap();
        let member = OdrMemberKey::new(
            group,
            OdrMemberRole::CallableBody,
            OdrMemberDiscriminator::GeneratedCallable(generated.id()),
        )
        .unwrap();
        scoop_identity::CallableOdrMemberId::from_key(&member).unwrap()
    }

    fn strong_body(name: &str) -> RuntimeIdentityRecord<PersistentCallableBodyId> {
        let function = source_function(name);
        RuntimeIdentityRecord::from_key(&CallableBodyKey::strong(
            StrongCallableDefinitionOwner::Function(function),
        ))
        .unwrap()
    }

    fn nominal_exact(name: &str) -> PersistentExactTypeId {
        let declaration = SourceDeclarationKey::nominal(
            declaration_site(),
            CanonicalIdentifier::new(name).unwrap(),
            SourceNominalKind::Class,
            0,
        );
        let nominal = PersistentTypeId::from_source_declaration(&declaration).unwrap();
        PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap()
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
