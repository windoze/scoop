use std::fmt;

use scoop_identity::{
    ConeIdentity, DecodedCallableBodyKey, DecodedCallableBodyKeyKind, DefinitionAtomRole,
    DefinitionOwner, GeneratedBridgeAtomId, LinkageClass, ObjectDefinitionAtomId,
    ObjectDefinitionPlanId, ObjectDefinitionPlanOwner, OdrGroupId, OdrMemberId,
    PersistentCallableBodyId, PersistentStaticStorageId, PersistentSymbolKey,
    PersistentSymbolRequest, SafepointId, StorageRole,
};
use scoop_wire::{Encoder, RuntimeDecodeError, WireEncode, decode_runtime};

use super::{CanonicalLirFoundation, LirFoundationBuildError};
use crate::ValidatedLirFoundation;

/// A canonical LIR identity foundation proven to satisfy the M23-3
/// `SingleConeStrong` profile's `RejectAll` ODR policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OdrFreeLirFoundation {
    producer: ConeIdentity,
    canonical: CanonicalLirFoundation,
}

impl OdrFreeLirFoundation {
    pub fn from_module(
        module: &crate::Module,
    ) -> Result<Self, OdrFreeLirFoundationProjectionError> {
        let foundation = CanonicalLirFoundation::from_module(module)
            .map_err(OdrFreeLirFoundationProjectionError::Foundation)?;
        Self::try_new(module.cone, foundation).map_err(OdrFreeLirFoundationProjectionError::Odr)
    }

    pub fn try_new(
        producer: ConeIdentity,
        foundation: CanonicalLirFoundation,
    ) -> Result<Self, OdrFreeLirFoundationError> {
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
            .find(|request| request.linkage() != LinkageClass::ConeStrong)
        {
            return Err(OdrFreeLirFoundationError::NonStrongSymbolRequest {
                key: request.key(),
                linkage: request.linkage(),
            });
        }
        for record in &foundation.definition_plans {
            match record.key().owner() {
                ObjectDefinitionPlanOwner::Strong {
                    producer: actual, ..
                } if actual != producer => {
                    return Err(OdrFreeLirFoundationError::ForeignStrongDefinitionPlan {
                        plan: record.id(),
                        expected: producer,
                        actual,
                    });
                }
                ObjectDefinitionPlanOwner::Odr { .. } => {
                    return Err(OdrFreeLirFoundationError::OdrDefinitionPlan(record.id()));
                }
                ObjectDefinitionPlanOwner::Strong { .. } => {}
            }
        }
        if let Some(record) = foundation
            .bridge_atoms
            .iter()
            .find(|record| record.key().producer() != producer)
        {
            return Err(OdrFreeLirFoundationError::ForeignGeneratedBridgeAtom {
                atom: record.id(),
                expected: producer,
                actual: record.key().producer(),
            });
        }
        Ok(Self {
            producer,
            canonical: foundation,
        })
    }

    pub fn from_validated(
        foundation: ValidatedLirFoundation,
    ) -> Result<Self, OdrFreeLirFoundationError> {
        let producer = foundation.producer();
        Self::try_new(producer, foundation.into_canonical())
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub const fn as_canonical(&self) -> &CanonicalLirFoundation {
        &self.canonical
    }

    pub fn resolve_definition_atom(
        &self,
        definition: ObjectDefinitionPlanId,
        atom_role: DefinitionAtomRole,
    ) -> Result<(ObjectDefinitionPlanId, ObjectDefinitionAtomId), DefinitionAtomResolutionError>
    {
        let mut targets = self.canonical.definition_atoms.iter().filter_map(|atom| {
            (atom.key().plan() == definition && atom.key().role() == atom_role)
                .then_some((definition, atom.id()))
        });
        let Some(target) = targets.next() else {
            return Err(DefinitionAtomResolutionError::Missing);
        };
        if targets.next().is_some() {
            return Err(DefinitionAtomResolutionError::Ambiguous);
        }
        Ok(target)
    }

    pub(crate) fn definition_plans(&self) -> &[super::DefinitionPlanRecord] {
        &self.canonical.definition_plans
    }

    pub(crate) fn callable_bodies(&self) -> &[super::CallableBodyRecord] {
        &self.canonical.callable_bodies
    }

    pub(crate) fn definition_atoms(&self) -> &[super::DefinitionAtomRecord] {
        &self.canonical.definition_atoms
    }

    pub(crate) fn exact_types(&self) -> &[super::ExactTypeRecord] {
        &self.canonical.exact_types
    }

    pub(crate) fn layouts(&self) -> &[super::LayoutRecord] {
        &self.canonical.layouts
    }

    pub(crate) fn scans(&self) -> &[super::ScanRecord] {
        &self.canonical.scans
    }

    pub(crate) fn runtime_types(&self) -> &[crate::RuntimeTypeMappingRecord] {
        &self.canonical.runtime_types
    }

    pub(crate) fn contains_callable_body(&self, id: PersistentCallableBodyId) -> bool {
        self.canonical
            .callable_bodies
            .iter()
            .any(|record| record.id() == id)
    }

    pub(crate) fn root_gateway_bodies(
        &self,
    ) -> Result<Vec<PersistentCallableBodyId>, RuntimeDecodeError> {
        self.canonical
            .callable_bodies
            .iter()
            .filter_map(|record| {
                match decode_runtime::<DecodedCallableBodyKey>(record.key_bytes()) {
                    Ok(key)
                        if matches!(key.kind(), DecodedCallableBodyKeyKind::RootGateway { .. }) =>
                    {
                        Some(Ok(record.id()))
                    }
                    Ok(_) => None,
                    Err(error) => Some(Err(error)),
                }
            })
            .collect()
    }

    pub(crate) fn contains_static_storage(&self, id: PersistentStaticStorageId) -> bool {
        self.canonical
            .static_storages
            .iter()
            .any(|record| record.id() == id)
    }

    pub(crate) fn root_entry_failure_roots(&self) -> Vec<PersistentStaticStorageId> {
        self.canonical
            .static_storages
            .iter()
            .filter_map(|record| {
                matches!(
                    (record.key().owner(), record.key().role()),
                    (
                        DefinitionOwner::RootEntry { .. },
                        StorageRole::RootEntryFailureRoot
                    )
                )
                .then_some(record.id())
            })
            .collect()
    }

    pub(crate) fn symbol_requests(&self) -> &[PersistentSymbolRequest] {
        self.canonical.symbol_requests.requests()
    }

    pub(crate) fn contains_layout(&self, id: scoop_identity::PersistentLayoutId) -> bool {
        self.canonical
            .layouts
            .iter()
            .any(|record| record.id() == id)
    }

    pub(crate) fn contains_scan(&self, id: scoop_identity::PersistentScanId) -> bool {
        self.canonical.scans.iter().any(|record| record.id() == id)
    }

    pub(crate) fn contains_safepoint_site(
        &self,
        id: scoop_identity::PersistentSafepointSiteId,
    ) -> bool {
        self.canonical
            .safepoint_sites
            .iter()
            .any(|record| record.id() == id)
    }

    pub(crate) fn contains_safepoint_mapping(
        &self,
        site: scoop_identity::PersistentSafepointSiteId,
        safepoint: SafepointId,
    ) -> bool {
        self.canonical
            .safepoints
            .iter()
            .any(|record| record.site() == site && record.safepoint() == safepoint)
    }

    pub(crate) fn contains_symbol_request(&self, request: PersistentSymbolRequest) -> bool {
        self.canonical.symbol_requests.requests().contains(&request)
    }

    pub(crate) fn bridge_units(&self) -> &[super::BridgeUnitRecord] {
        &self.canonical.bridge_units
    }

    pub(crate) fn bridge_atoms(&self) -> &[super::BridgeAtomRecord] {
        &self.canonical.bridge_atoms
    }

    pub(crate) fn native_contracts(&self) -> &[scoop_identity::NativeExternalContractRecord] {
        &self.canonical.native_contracts
    }

    pub(crate) fn native_link_requirements(&self) -> &[super::NativeLinkRequirementRecord] {
        &self.canonical.native_link_requirements
    }

    pub fn into_canonical(self) -> CanonicalLirFoundation {
        self.canonical
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefinitionAtomResolutionError {
    Missing,
    Ambiguous,
}

impl fmt::Display for DefinitionAtomResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "definition atom cannot be resolved uniquely: {self:?}"
        )
    }
}

impl std::error::Error for DefinitionAtomResolutionError {}

impl WireEncode for OdrFreeLirFoundation {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.canonical.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OdrFreeLirFoundationError {
    OdrGroup(OdrGroupId),
    OdrMember(OdrMemberId),
    OdrCallableBody(PersistentCallableBodyId),
    NonStrongSymbolRequest {
        key: PersistentSymbolKey,
        linkage: LinkageClass,
    },
    OdrDefinitionPlan(ObjectDefinitionPlanId),
    ForeignStrongDefinitionPlan {
        plan: ObjectDefinitionPlanId,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    ForeignGeneratedBridgeAtom {
        atom: GeneratedBridgeAtomId,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
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
            Self::NonStrongSymbolRequest { key, linkage } => write!(
                formatter,
                "{}: LIR {:?} symbol request has {:?} linkage; the SingleConeStrong profile requires ConeStrong",
                Self::CODE,
                key.kind(),
                linkage
            ),
            Self::OdrDefinitionPlan(id) => write!(
                formatter,
                "{}: LIR ODR definition plan {} is forbidden by the SingleConeStrong profile",
                Self::CODE,
                HexIdentity(id.as_array())
            ),
            Self::ForeignStrongDefinitionPlan {
                plan,
                expected,
                actual,
            } => write!(
                formatter,
                "strong definition plan {plan} belongs to Cone {actual}, not production Cone {expected}"
            ),
            Self::ForeignGeneratedBridgeAtom {
                atom,
                expected,
                actual,
            } => write!(
                formatter,
                "generated bridge atom {atom} belongs to Cone {actual}, not production Cone {expected}"
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
        DefinitionOwnerChain, DispatchSlotKey, ExactTypeKey, GeneratedCallableKey, LinkageClass,
        ObjectDefinitionPlanKey, OdrMemberDiscriminator, OdrMemberKey, OdrMemberRole, PackagePath,
        PersistentCallableBodyId, PersistentDispatchSlotId, PersistentExactTypeId,
        PersistentFunctionId, PersistentSymbolKey, PersistentSymbolRequest,
        PersistentSymbolRequestTable, PersistentTypeId, RuntimeIdentityRecord,
        SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, SpecializationKey,
        StrongCallableDefinitionOwner, StrongDefinitionEntity, StrongDefinitionRole,
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

        let proven = OdrFreeLirFoundation::try_new(ConeIdentity::CORE, canonical).unwrap();

        assert_eq!(encode(&proven).unwrap(), expected);
        assert_eq!(proven.producer(), ConeIdentity::CORE);
        assert_eq!(proven.as_canonical().counts().odr_members, 0);
    }

    #[test]
    fn rejects_odr_group_and_member_tables_independently() {
        let (group, member) = odr_records();
        let expected_group = group.id();
        let mut with_group = CanonicalLirFoundation::empty();
        with_group.set_odr_groups(vec![group]).unwrap();
        assert_eq!(
            OdrFreeLirFoundation::try_new(ConeIdentity::CORE, with_group),
            Err(OdrFreeLirFoundationError::OdrGroup(expected_group))
        );

        let expected_member = member.id();
        let mut with_member = CanonicalLirFoundation::empty();
        with_member.set_odr_members(vec![member]).unwrap();
        assert_eq!(
            OdrFreeLirFoundation::try_new(ConeIdentity::CORE, with_member),
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
            OdrFreeLirFoundation::try_new(ConeIdentity::CORE, canonical),
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
            OdrFreeLirFoundation::try_new(ConeIdentity::CORE, canonical),
            Err(OdrFreeLirFoundationError::NonStrongSymbolRequest {
                key,
                linkage: LinkageClass::OdrWeak,
            })
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
            OdrFreeLirFoundation::try_new(ConeIdentity::CORE, canonical),
            Err(OdrFreeLirFoundationError::NonStrongSymbolRequest {
                key,
                linkage: LinkageClass::OdrWeak,
            })
        );
    }

    #[test]
    fn rejects_template_support_hidden_symbols() {
        let slot = PersistentDispatchSlotId::from_key(&DispatchSlotKey::virtual_method(
            source_function("hiddenSlot"),
        ))
        .unwrap();
        let key = PersistentSymbolKey::DispatchSlot(slot);
        let request =
            PersistentSymbolRequest::new(key, LinkageClass::TemplateSupportHidden).unwrap();
        let mut canonical = CanonicalLirFoundation::empty();
        canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![request]).unwrap());

        assert_eq!(
            OdrFreeLirFoundation::try_new(ConeIdentity::CORE, canonical),
            Err(OdrFreeLirFoundationError::NonStrongSymbolRequest {
                key,
                linkage: LinkageClass::TemplateSupportHidden,
            })
        );
    }

    #[test]
    fn rejects_odr_and_foreign_strong_definition_plans() {
        let odr_plan = CborIdentityRecord::from_key(ObjectDefinitionPlanKey::odr(
            callable_odr_member().member(),
        ))
        .unwrap();
        let expected_odr = odr_plan.id();
        let mut with_odr = CanonicalLirFoundation::empty();
        with_odr.set_definition_plans(vec![odr_plan]).unwrap();
        assert_eq!(
            OdrFreeLirFoundation::try_new(ConeIdentity::CORE, with_odr),
            Err(OdrFreeLirFoundationError::OdrDefinitionPlan(expected_odr))
        );

        let body = strong_body("foreign");
        let foreign_plan = CborIdentityRecord::from_key(
            ObjectDefinitionPlanKey::strong(
                ConeIdentity::SINGLE_FILE,
                StrongDefinitionEntity::callable_body(body.id()),
                StrongDefinitionRole::CallableBody,
            )
            .unwrap(),
        )
        .unwrap();
        let expected_plan = foreign_plan.id();
        let mut with_foreign = CanonicalLirFoundation::empty();
        with_foreign.set_callable_bodies(vec![body]).unwrap();
        with_foreign
            .set_definition_plans(vec![foreign_plan])
            .unwrap();
        assert_eq!(
            OdrFreeLirFoundation::try_new(ConeIdentity::CORE, with_foreign),
            Err(OdrFreeLirFoundationError::ForeignStrongDefinitionPlan {
                plan: expected_plan,
                expected: ConeIdentity::CORE,
                actual: ConeIdentity::SINGLE_FILE,
            })
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
