//! Final source/export/selection join for a replayed V2 production section.

use super::*;
use scoop_identity::{
    CallableBodyKey, ConeIdentity, PersistentCallableBodyId, PersistentDispatchTableId,
    PersistentExactTypeId, PersistentInitializationUnitId, PersistentLayoutId, PersistentScanId,
    PersistentTypeId, StrongCallableDefinitionOwner,
};
use scoop_wire::{BudgetMeter, HashError, WireError};

mod local;
mod selected;

/// Strong V2 production authority bound to one complete layout/ABI section.
///
/// This view deliberately has no wire encoder or conversion to the raw section.
/// Construction consumes the replayed physical section and snapshots the
/// checked local exports, so later Link-contract replay cannot downgrade it or
/// substitute tables from another source/selection closure.
pub struct ValidatedStrongProductionSectionV2 {
    replayed: ReplayedStrongProductionSectionV2,
    layouts: crate::CanonicalExactLayoutExportsV1,
    descriptors: crate::CanonicalExactDescriptorExportsV1,
    dispatch: crate::CanonicalExactDispatchExportsV1,
    callables: crate::CanonicalExactCallableAbiExportsV1,
    shape_support: crate::CanonicalParamFreeShapeSupportExportsV1,
}

impl std::fmt::Debug for ValidatedStrongProductionSectionV2 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ValidatedStrongProductionSectionV2")
            .field("provider", &self.provider())
            .finish_non_exhaustive()
    }
}

impl ReplayedStrongProductionSectionV2 {
    pub fn validate_layout_abi(
        self,
        layout_abi: &crate::CrossConeLayoutAbiSectionV1<'_>,
        meter: &mut BudgetMeter,
    ) -> Result<ValidatedStrongProductionSectionV2, StrongProductionLayoutJoinError> {
        validate_layout_abi(self, layout_abi, meter)
    }
}

impl crate::StrongProductionSectionV2 {
    /// Joins a freshly produced section with its complete layout/ABI section.
    /// The returned authority is the only producer result that may be handed
    /// to publication and code-fingerprint projection.
    pub fn validate_layout_abi(
        self,
        layout_abi: &crate::CrossConeLayoutAbiSectionV1<'_>,
        meter: &mut BudgetMeter,
    ) -> Result<ValidatedStrongProductionSectionV2, StrongProductionLayoutJoinError> {
        validate_layout_abi(
            ReplayedStrongProductionSectionV2 { section: self },
            layout_abi,
            meter,
        )
    }
}

impl ValidatedStrongProductionSectionV2 {
    /// Releases the canonical wire section only after the layout/ABI join has
    /// succeeded. A replayed but unjoined section has no equivalent method.
    pub fn into_section(self) -> crate::StrongProductionSectionV2 {
        self.replayed.section
    }

    pub fn provider(&self) -> ConeIdentity {
        self.replayed.type_registrations().producer()
    }

    pub const fn layouts(&self) -> &crate::CanonicalExactLayoutExportsV1 {
        &self.layouts
    }

    pub const fn descriptors(&self) -> &crate::CanonicalExactDescriptorExportsV1 {
        &self.descriptors
    }

    pub const fn dispatch(&self) -> &crate::CanonicalExactDispatchExportsV1 {
        &self.dispatch
    }

    pub const fn callables(&self) -> &crate::CanonicalExactCallableAbiExportsV1 {
        &self.callables
    }

    pub const fn shape_support(&self) -> &crate::CanonicalParamFreeShapeSupportExportsV1 {
        &self.shape_support
    }

    pub fn external_bridges(&self) -> &crate::StrongExternalLirBridgeSurfaceV1 {
        self.replayed.external_bridges()
    }

    pub fn canonical_definitions(&self) -> &crate::StrongObjectSymbolSurfaceV1 {
        self.replayed.canonical_definitions()
    }

    pub fn object_definition_plans(&self) -> &crate::StrongObjectDefinitionPlanSurfaceV1 {
        self.replayed.object_definition_plans()
    }

    pub fn digest_finalization_plan(&self) -> &crate::StrongDigestFinalizationPlanV1 {
        self.replayed.digest_finalization_plan()
    }

    pub fn registration_identities(&self) -> &crate::StrongRegistrationIdentitySurfaceV1 {
        self.replayed.registration_identities()
    }

    pub fn type_registrations(&self) -> &crate::StrongTypeRegistrationPlanSetV2 {
        self.replayed.type_registrations()
    }

    pub fn callable_registrations(&self) -> &crate::StrongCallableRegistrationPlanSetV1 {
        self.replayed.callable_registrations()
    }

    pub fn safepoint_registrations(&self) -> &crate::StrongSafepointRegistrationPlanSetV1 {
        self.replayed.safepoint_registrations()
    }

    pub fn immortal_registrations(&self) -> &crate::StrongImmortalObjectRegistrationPlanSetV1 {
        self.replayed.immortal_registrations()
    }

    pub fn static_storage_registrations(&self) -> &crate::StrongStaticStorageRegistrationPlanSetV1 {
        self.replayed.static_storage_registrations()
    }

    pub fn initialization_registrations(
        &self,
    ) -> &crate::StrongInitializationUnitRegistrationPlanSetV2 {
        self.replayed.initialization_registrations()
    }

    pub fn image_plan(&self) -> &crate::ConeImagePlanV1 {
        self.replayed.image_plan()
    }

    pub fn entry_plan(&self) -> &crate::EntryProductionPlanV1 {
        self.replayed.entry_plan()
    }

    pub fn shape_support_plan(&self) -> &crate::ParamFreeShapeSupportPlanSetV1 {
        self.replayed.shape_support_plan()
    }

    pub fn generated_bridge_plan(&self) -> &crate::GeneratedBridgePlanSetV1 {
        self.replayed.generated_bridge_plan()
    }

    pub fn initialization_cycle_abi(&self) -> Option<&crate::CallableAbiRecordV1> {
        self.replayed.initialization_cycle_abi()
    }
}

fn validate_layout_abi(
    replayed: ReplayedStrongProductionSectionV2,
    layout_abi: &crate::CrossConeLayoutAbiSectionV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<ValidatedStrongProductionSectionV2, StrongProductionLayoutJoinError> {
    local::validate(&replayed, layout_abi, meter)?;
    selected::validate(&replayed, layout_abi.selected(), meter)?;
    Ok(ValidatedStrongProductionSectionV2 {
        replayed,
        layouts: layout_abi.layouts().clone(),
        descriptors: layout_abi.descriptors().clone(),
        dispatch: layout_abi.dispatch().clone(),
        callables: layout_abi.callables().clone(),
        shape_support: layout_abi.shape_support().clone(),
    })
}

#[derive(Debug)]
pub enum StrongProductionLayoutJoinError {
    Resource(WireError),
    CallableBodyIdentity(HashError),
    Provider {
        expected: ConeIdentity,
        actual: ConeIdentity,
        component: &'static str,
    },
    Target,
    LayoutProduction(PersistentLayoutId),
    ScanProduction(PersistentScanId),
    MissingDescriptorExport(PersistentExactTypeId),
    MissingDescriptorProduction(PersistentExactTypeId),
    DescriptorProduction(PersistentExactTypeId),
    MissingDispatchExport(PersistentDispatchTableId),
    MissingDispatchProduction(PersistentDispatchTableId),
    DispatchProduction(PersistentDispatchTableId),
    MissingCallableProduction(StrongCallableDefinitionOwner),
    CallableProduction(StrongCallableDefinitionOwner),
    ShapeSupportProduction(PersistentTypeId),
    MissingSelectedDescriptor {
        provider: ConeIdentity,
        exact: PersistentExactTypeId,
    },
    MissingPhysicalDescriptor {
        provider: ConeIdentity,
        exact: PersistentExactTypeId,
    },
    MissingSelectedCallable {
        provider: ConeIdentity,
        body: PersistentCallableBodyId,
    },
    MissingPhysicalCallable {
        provider: ConeIdentity,
        body: PersistentCallableBodyId,
    },
    AmbiguousPhysicalCallable {
        provider: ConeIdentity,
        body: PersistentCallableBodyId,
    },
    MissingPhysicalInitialization {
        provider: ConeIdentity,
        unit: PersistentInitializationUnitId,
    },
    InitializationDefinition {
        provider: ConeIdentity,
        unit: PersistentInitializationUnitId,
    },
}

impl From<WireError> for StrongProductionLayoutJoinError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl From<HashError> for StrongProductionLayoutJoinError {
    fn from(error: HashError) -> Self {
        Self::CallableBodyIdentity(error)
    }
}

impl std::fmt::Display for StrongProductionLayoutJoinError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid Strong V2 layout/ABI join: {self:?}")
    }
}

impl std::error::Error for StrongProductionLayoutJoinError {}
