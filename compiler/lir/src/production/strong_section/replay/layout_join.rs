//! Layout and selected-reference consistency for canonical Strong V2 data.

use super::*;
use scoop_identity::{
    CallableBodyKey, ConeIdentity, PersistentCallableBodyId, PersistentDispatchTableId,
    PersistentExactTypeId, PersistentInitializationUnitId, PersistentLayoutId, PersistentScanId,
    PersistentTypeId, StrongCallableDefinitionOwner,
};
use scoop_wire::{HashError, WireError};

mod local;
mod selected;

impl StrongProductionSectionV2 {
    pub fn validate_layout_abi(
        self,
        layout: &crate::CrossConeLayoutAbiSectionV1<'_>,
    ) -> Result<Self, StrongProductionLayoutJoinError> {
        local::validate(&self, layout.exports())?;
        selected::validate(&self, selected::Selection::Complete(layout.selected()))?;
        Ok(self)
    }

    pub fn validate_layout_selection(
        &self,
        layout: &crate::PhysicalImportsReplayedLayoutAbiSectionV1,
    ) -> Result<(), StrongProductionLayoutJoinError> {
        local::validate(self, layout.exports())?;
        selected::validate(self, selected::Selection::Replayed(layout))
    }
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
    MissingDescriptorProduction(PersistentExactTypeId),
    DescriptorProduction(PersistentExactTypeId),
    MissingDispatchProduction(PersistentDispatchTableId),
    DispatchProduction(PersistentDispatchTableId),
    MissingCallableProduction(StrongCallableDefinitionOwner),
    CallableProduction(StrongCallableDefinitionOwner),
    ShapeSupportProduction(PersistentTypeId),
    StaticStorageLayout {
        provider: ConeIdentity,
        layout: PersistentLayoutId,
    },
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
