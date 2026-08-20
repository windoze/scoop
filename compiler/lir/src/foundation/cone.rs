use std::fmt;
use std::rc::Rc;

use scoop_identity::{
    ConeIdentity, DecodedCallableBodyKey, DecodedCallableBodyKeyKind, DefinitionAtomRole,
    DefinitionOwner, GeneratedBridgeAtomId, LinkageClass, ObjectDefinitionAtomId,
    ObjectDefinitionPlanId, ObjectDefinitionPlanOwner, OdrGroupId, OdrMemberId,
    PersistentCallableBodyId, PersistentExactTypeId, PersistentStaticStorageId,
    PersistentSymbolKey, PersistentSymbolRequest, SafepointId, StorageRole,
};
use scoop_wire::{Encoder, RuntimeDecodeError, WireEncode, decode_runtime};

use super::{CanonicalLirFoundation, LirFoundationBuildError};
use crate::ValidatedLirFoundation;

mod atoms;
mod codegen;
mod native;

use atoms::DefinitionAtomIndex;

/// The physical producer and its single canonical LIR identity foundation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConeLirFoundation {
    producer: ConeIdentity,
    canonical: Rc<CanonicalLirFoundation>,
    atom_index: Rc<DefinitionAtomIndex>,
}

impl ConeLirFoundation {
    pub fn from_module(module: &crate::Module) -> Result<Self, ConeLirFoundationProjectionError> {
        let mut foundation = CanonicalLirFoundation::from_module(module)
            .map_err(ConeLirFoundationProjectionError::Foundation)?;
        foundation
            .project_definitions(module)
            .map_err(ConeLirFoundationProjectionError::Foundation)?;
        Self::try_new(module.cone, foundation).map_err(ConeLirFoundationProjectionError::Ownership)
    }

    pub fn try_new(
        producer: ConeIdentity,
        foundation: CanonicalLirFoundation,
    ) -> Result<Self, ConeLirFoundationError> {
        for record in &foundation.definition_plans {
            if let ObjectDefinitionPlanOwner::Strong {
                producer: actual, ..
            } = record.key().owner()
                && actual != producer
            {
                return Err(ConeLirFoundationError::ForeignStrongDefinitionPlan {
                    plan: record.id(),
                    expected: producer,
                    actual,
                });
            }
        }
        if let Some(record) = foundation
            .bridge_atoms
            .iter()
            .find(|record| record.key().producer() != producer)
        {
            return Err(ConeLirFoundationError::ForeignGeneratedBridgeAtom {
                atom: record.id(),
                expected: producer,
                actual: record.key().producer(),
            });
        }
        Ok(Self::from_canonical(producer, foundation))
    }

    /// Applies the historical Strong profile restriction at its artifact boundary.
    pub fn require_strong(&self) -> Result<(), ConeLirFoundationError> {
        let foundation = &self.canonical;
        if let Some(record) = foundation.odr_groups.first() {
            return Err(ConeLirFoundationError::OdrGroup(record.id()));
        }
        if let Some(record) = foundation.odr_members.first() {
            return Err(ConeLirFoundationError::OdrMember(record.id()));
        }
        for record in &foundation.callable_bodies {
            let key =
                decode_runtime::<DecodedCallableBodyKey>(record.key_bytes()).map_err(|error| {
                    ConeLirFoundationError::InvalidCallableBodyKey {
                        body: record.id(),
                        error,
                    }
                })?;
            if matches!(key.kind(), DecodedCallableBodyKeyKind::Odr(_)) {
                return Err(ConeLirFoundationError::OdrCallableBody(record.id()));
            }
        }
        if let Some(request) = foundation
            .symbol_requests
            .requests()
            .iter()
            .find(|request| request.linkage() != LinkageClass::ConeStrong)
        {
            return Err(ConeLirFoundationError::NonStrongSymbolRequest {
                key: request.key(),
                linkage: request.linkage(),
            });
        }
        if let Some(record) = foundation
            .definition_plans
            .iter()
            .find(|record| matches!(record.key().owner(), ObjectDefinitionPlanOwner::Odr { .. }))
        {
            return Err(ConeLirFoundationError::OdrDefinitionPlan(record.id()));
        }
        Ok(())
    }

    pub fn from_validated(foundation: ValidatedLirFoundation) -> Self {
        Self::from_canonical(foundation.producer(), foundation.into_canonical())
    }

    fn from_canonical(producer: ConeIdentity, foundation: CanonicalLirFoundation) -> Self {
        Self {
            producer,
            atom_index: Rc::new(DefinitionAtomIndex::new(&foundation.definition_atoms)),
            canonical: Rc::new(foundation),
        }
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn as_canonical(&self) -> &CanonicalLirFoundation {
        &self.canonical
    }

    #[doc(hidden)]
    pub fn native_contracts(&self) -> &[scoop_identity::NativeExternalContractRecord] {
        &self.canonical.native_contracts
    }

    #[doc(hidden)]
    pub fn c_abi_signatures(&self) -> &[scoop_identity::CanonicalCAbiSignatureFingerprintRecord] {
        &self.canonical.c_abi_signatures
    }

    #[doc(hidden)]
    pub fn c_abi_layouts(&self) -> &[scoop_identity::CanonicalCAbiLayoutFingerprintRecord] {
        &self.canonical.c_abi_layouts
    }

    #[doc(hidden)]
    pub fn callback_bridges(&self) -> &[crate::CallbackBridgeRecord] {
        &self.canonical.callback_bridges
    }

    pub fn resolve_definition_atom(
        &self,
        definition: ObjectDefinitionPlanId,
        atom_role: DefinitionAtomRole,
    ) -> Result<(ObjectDefinitionPlanId, ObjectDefinitionAtomId), DefinitionAtomResolutionError>
    {
        self.atom_index
            .resolve(definition, atom_role)
            .map(|atom| (definition, atom))
    }

    pub fn definition_plan_count(&self) -> usize {
        self.definition_plans().len()
    }

    pub(crate) fn definition_plans(&self) -> &[super::DefinitionPlanRecord] {
        &self.canonical.definition_plans
    }

    pub(crate) fn definition_plan(
        &self,
        id: ObjectDefinitionPlanId,
    ) -> Option<&super::DefinitionPlanRecord> {
        self.definition_plans()
            .binary_search_by_key(&id, |record| record.id())
            .ok()
            .map(|index| &self.definition_plans()[index])
    }

    pub(crate) fn odr_member(&self, id: OdrMemberId) -> Option<&super::OdrMemberRecord> {
        self.canonical
            .odr_members
            .iter()
            .find(|record| record.id() == id)
    }

    pub(crate) fn callable_bodies(&self) -> &[super::CallableBodyRecord] {
        &self.canonical.callable_bodies
    }

    pub(crate) fn definition_atoms(&self) -> &[super::DefinitionAtomRecord] {
        &self.canonical.definition_atoms
    }

    pub(crate) fn definition_atoms_for_plan(
        &self,
        plan: ObjectDefinitionPlanId,
    ) -> impl Iterator<Item = &super::DefinitionAtomRecord> {
        self.atom_index
            .for_plan(plan)
            .map(|index| &self.canonical.definition_atoms[index])
    }

    pub(crate) fn definition_atom(
        &self,
        id: ObjectDefinitionAtomId,
    ) -> Option<&super::DefinitionAtomRecord> {
        self.definition_atoms()
            .binary_search_by_key(&id, |record| record.id())
            .ok()
            .map(|index| &self.definition_atoms()[index])
    }

    pub fn materialized_exact_types(&self) -> &[PersistentExactTypeId] {
        &self.canonical.materialized_exact_types
    }

    pub(crate) fn layouts(&self) -> &[super::LayoutRecord] {
        &self.canonical.layouts
    }

    pub(crate) fn scans(&self) -> &[super::ScanRecord] {
        &self.canonical.scans
    }

    pub(crate) fn dispatch_tables(&self) -> &[super::DispatchTableRecord] {
        &self.canonical.dispatch_tables
    }

    pub(crate) fn static_storages(&self) -> &[super::StaticStorageRecord] {
        &self.canonical.static_storages
    }

    pub(crate) fn safepoint_sites(&self) -> &[super::SafepointSiteRecord] {
        &self.canonical.safepoint_sites
    }

    pub(crate) fn safepoint_mappings(&self) -> &[crate::SafepointMappingRecord] {
        &self.canonical.safepoints
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

    pub(crate) fn contains_immortal_object(
        &self,
        id: scoop_identity::PersistentImmortalObjectId,
    ) -> bool {
        self.canonical
            .immortal_objects
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

    pub(crate) fn native_link_requirements(&self) -> &[super::NativeLinkRequirementRecord] {
        &self.canonical.native_link_requirements
    }

    pub fn into_canonical(self) -> CanonicalLirFoundation {
        Rc::unwrap_or_clone(self.canonical)
    }

    pub(crate) fn into_shared(self) -> Rc<CanonicalLirFoundation> {
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

impl WireEncode for ConeLirFoundation {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.canonical.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConeLirFoundationError {
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

impl ConeLirFoundationError {
    pub const CODE: &'static str = "SCOOPC_CAPABILITY_ODR_UNAVAILABLE";
}

impl fmt::Display for ConeLirFoundationError {
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

impl std::error::Error for ConeLirFoundationError {}

#[derive(Debug)]
pub enum ConeLirFoundationProjectionError {
    Foundation(LirFoundationBuildError),
    Ownership(ConeLirFoundationError),
}

impl fmt::Display for ConeLirFoundationProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Foundation(error) => error.fmt(formatter),
            Self::Ownership(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ConeLirFoundationProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Foundation(error) => Some(error),
            Self::Ownership(error) => Some(error),
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
mod tests;
