use std::fmt;

use scoop_identity::{
    DefinitionOriginRecord, DefinitionOriginSubject, OdrGroupId, OdrMemberId,
    PersistentCallableApplicationId, PersistentSourceContextId, SourceContextKey, SourceIdentity,
};
use scoop_wire::{Encoder, WireEncode};

use super::{CanonicalHirFoundation, HirFoundationBuildError};
use crate::ValidatedHirFoundation;

/// A canonical HIR identity foundation proven to satisfy the M23-3
/// `SingleConeStrong` profile's `RejectAll` ODR policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OdrFreeHirFoundation(CanonicalHirFoundation);

impl OdrFreeHirFoundation {
    pub fn from_output(
        output: &crate::Output,
    ) -> Result<Self, OdrFreeHirFoundationProjectionError> {
        Self::from_modules(&output.export, &output.local, &output.native_boundary_types)
    }

    pub fn from_modules(
        export: &crate::ExportHir,
        local: &crate::LocalConcreteHir,
        native_boundary_types: &crate::HirNativeBoundaryTypeDefinitions,
    ) -> Result<Self, OdrFreeHirFoundationProjectionError> {
        let foundation = CanonicalHirFoundation::from_modules(export, local, native_boundary_types)
            .map_err(OdrFreeHirFoundationProjectionError::Foundation)?;
        Self::try_new(foundation).map_err(OdrFreeHirFoundationProjectionError::Odr)
    }

    pub fn try_new(foundation: CanonicalHirFoundation) -> Result<Self, OdrFreeHirFoundationError> {
        if let Some(record) = foundation.callable_applications.first() {
            return Err(OdrFreeHirFoundationError::CallableApplication(record.id()));
        }
        if let Some(record) = foundation.odr_groups.first() {
            return Err(OdrFreeHirFoundationError::OdrGroup(record.id()));
        }
        if let Some(record) = foundation.odr_members.first() {
            return Err(OdrFreeHirFoundationError::OdrMember(record.id()));
        }
        Ok(Self(foundation))
    }

    pub fn from_validated(
        foundation: ValidatedHirFoundation,
    ) -> Result<Self, OdrFreeHirFoundationError> {
        Self::try_new(foundation.into_canonical())
    }

    pub const fn as_canonical(&self) -> &CanonicalHirFoundation {
        &self.0
    }

    /// Returns the canonical definition origin for an exact typed subject.
    pub fn definition_origin(
        &self,
        subject: DefinitionOriginSubject,
    ) -> Option<&DefinitionOriginRecord> {
        self.0.definition_origin(subject)
    }

    /// Returns the canonical source record for an exact source identity.
    pub fn source_record(&self, source: &SourceIdentity) -> Option<&crate::SourceRecord> {
        self.0
            .sources
            .binary_search_by(|record| record.identity().cmp(source))
            .ok()
            .map(|index| &self.0.sources[index])
    }

    /// Returns all canonical source records retained by this artifact.
    #[doc(hidden)]
    pub fn source_records(&self) -> &[crate::SourceRecord] {
        &self.0.sources
    }

    /// Returns the canonical key for an exact source-context identity.
    pub fn source_context_key(
        &self,
        context: PersistentSourceContextId,
    ) -> Option<&SourceContextKey> {
        self.0
            .source_contexts
            .binary_search_by_key(&context, |record| record.id())
            .ok()
            .map(|index| self.0.source_contexts[index].key())
    }

    /// Iterates every canonical source-context identity declared by this
    /// artifact together with its validated key.
    #[doc(hidden)]
    pub fn source_context_records(
        &self,
    ) -> impl ExactSizeIterator<Item = (PersistentSourceContextId, &SourceContextKey)> {
        self.0
            .source_contexts
            .iter()
            .map(|record| (record.id(), record.key()))
    }

    #[doc(hidden)]
    pub fn source_native_contracts(&self) -> &[scoop_identity::SourceNativeExternalContractRecord] {
        &self.0.source_native_contracts
    }

    #[doc(hidden)]
    pub fn native_boundary_types(&self) -> &[crate::NativeBoundaryTypeDefinitionRecord] {
        &self.0.native_boundary_types
    }

    pub fn into_canonical(self) -> CanonicalHirFoundation {
        self.0
    }
}

impl TryFrom<CanonicalHirFoundation> for OdrFreeHirFoundation {
    type Error = OdrFreeHirFoundationError;

    fn try_from(foundation: CanonicalHirFoundation) -> Result<Self, Self::Error> {
        Self::try_new(foundation)
    }
}

impl WireEncode for OdrFreeHirFoundation {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OdrFreeHirFoundationError {
    CallableApplication(PersistentCallableApplicationId),
    OdrGroup(OdrGroupId),
    OdrMember(OdrMemberId),
}

impl OdrFreeHirFoundationError {
    pub const CODE: &'static str = "SCOOPC_CAPABILITY_ODR_UNAVAILABLE";
}

impl fmt::Display for OdrFreeHirFoundationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CallableApplication(id) => write!(
                formatter,
                "{}: HIR callable application {} requires unavailable ODR materialization",
                Self::CODE,
                HexIdentity(id.as_array())
            ),
            Self::OdrGroup(id) => write!(
                formatter,
                "{}: HIR ODR group {} is forbidden by the SingleConeStrong profile",
                Self::CODE,
                HexIdentity(id.as_array())
            ),
            Self::OdrMember(id) => write!(
                formatter,
                "{}: HIR ODR member {} is forbidden by the SingleConeStrong profile",
                Self::CODE,
                HexIdentity(id.as_array())
            ),
        }
    }
}

impl std::error::Error for OdrFreeHirFoundationError {}

#[derive(Debug)]
pub enum OdrFreeHirFoundationProjectionError {
    Foundation(HirFoundationBuildError),
    Odr(OdrFreeHirFoundationError),
}

impl fmt::Display for OdrFreeHirFoundationProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Foundation(error) => error.fmt(formatter),
            Self::Odr(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for OdrFreeHirFoundationProjectionError {
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
        CallableApplicationKey, CallableInstantiationOwner, CanonicalIdentifier,
        CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOrigin,
        DefinitionOriginRecord, DefinitionOriginSubject, DefinitionOwnerChain, ExactTypeKey,
        NormalizedSourcePath, OdrMemberDiscriminator, OdrMemberKey, OdrMemberRole, PackagePath,
        PersistentExactTypeId, PersistentFunctionId, PersistentTypeId, SourceContextKey,
        SourceDeclarationKey, SourceDeclarationSite, SourceIdentity, SourceNominalKind, SourceSpan,
        SpecializationKey,
    };
    use scoop_wire::encode;

    use super::{OdrFreeHirFoundation, OdrFreeHirFoundationError};
    use crate::CanonicalHirFoundation;

    #[test]
    fn accepts_and_preserves_an_empty_foundation() {
        let canonical = CanonicalHirFoundation::empty();
        let expected = encode(&canonical).unwrap();

        let proven = OdrFreeHirFoundation::try_new(canonical).unwrap();

        assert_eq!(encode(&proven).unwrap(), expected);
        assert_eq!(proven.as_canonical().counts().callable_applications, 0);
    }

    #[test]
    fn exposes_definition_origins_by_typed_subject() {
        let subject = DefinitionOriginSubject::Type(
            PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
                declaration_site(),
                CanonicalIdentifier::new("Payload").unwrap(),
                SourceNominalKind::Class,
                0,
            ))
            .unwrap(),
        );
        let source = SourceIdentity::new(
            ConeIdentity::CORE,
            NormalizedSourcePath::new("src/Payload.scoop").unwrap(),
        )
        .unwrap();
        let origin = DefinitionOrigin::new(
            source.clone(),
            SourceSpan::new(0, 7).unwrap(),
            &SourceContextKey::File { source },
        )
        .unwrap();
        let expected = DefinitionOriginRecord::new(subject, origin);
        let mut canonical = CanonicalHirFoundation::empty();
        canonical
            .set_definition_origins(vec![expected.clone()])
            .unwrap();
        let proven = OdrFreeHirFoundation::try_new(canonical).unwrap();

        assert_eq!(proven.definition_origin(subject), Some(&expected));
        assert_eq!(
            proven.definition_origin(DefinitionOriginSubject::Type(source_type("Missing"))),
            None
        );
    }

    #[test]
    fn rejects_callable_applications_before_odr_tables() {
        let function = source_function("genericUse");
        let key =
            CallableApplicationKey::for_function(function, CallableInstantiationOwner::NoOwner);
        let application = CborIdentityRecord::from_key(key).unwrap();
        let expected = application.id();
        let mut canonical = CanonicalHirFoundation::empty();
        canonical
            .set_callable_applications(vec![application])
            .unwrap();

        assert_eq!(
            OdrFreeHirFoundation::try_new(canonical),
            Err(OdrFreeHirFoundationError::CallableApplication(expected))
        );
    }

    #[test]
    fn rejects_odr_group_and_member_tables_independently() {
        let (group, member) = odr_records();
        let expected_group = group.id();
        let expected_member = member.id();
        let mut with_group = CanonicalHirFoundation::empty();
        with_group.set_odr_groups(vec![group]).unwrap();
        assert_eq!(
            OdrFreeHirFoundation::try_new(with_group),
            Err(OdrFreeHirFoundationError::OdrGroup(expected_group))
        );

        let mut with_member = CanonicalHirFoundation::empty();
        with_member.set_odr_members(vec![member]).unwrap();
        assert_eq!(
            OdrFreeHirFoundation::try_new(with_member),
            Err(OdrFreeHirFoundationError::OdrMember(expected_member))
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

    fn nominal_exact(name: &str) -> PersistentExactTypeId {
        PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(source_type(name))).unwrap()
    }

    fn source_type(name: &str) -> PersistentTypeId {
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
