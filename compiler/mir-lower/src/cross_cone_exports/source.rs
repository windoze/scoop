//! Independent source projection for replaying an untrusted export candidate.

use super::*;
use scoop_identity::{
    ConeIdentity, PersistentExactTypeId, PersistentObjectValueId, PersistentTypeId,
};

mod inventory;
mod queries;
mod units;
mod uses;

/// Expected source records produced without observing the candidate being
/// checked. This is a source-join adapter, not a terminal section or selection.
pub struct MirTypeBridgeSourceProjectionV1 {
    provider: ConeIdentity,
    expected: mir::MirTypeBridgeExportConstituentsV1,
    inventory: inventory::Inventory,
    units: units::InitializationContracts,
    uses: Vec<mir::MirTypeBridgeDependencyV1>,
}

impl MirTypeBridgeSourceProjectionV1 {
    pub fn from_input(
        input: MirTypeBridgeExportInputV1<'_>,
        dependencies: MirTypeBridgeDependencyTablesV1<'_>,
        initialization_uses: mir::CanonicalMirExternalInitializationUsesV1,
        meter: &mut BudgetMeter,
    ) -> Result<Self, MirTypeBridgeSourceProjectionError> {
        let expected = lower_type_bridge_exports(input, dependencies, initialization_uses, meter)
            .map_err(MirTypeBridgeSourceProjectionError::Production)?;
        let inventory = inventory::Inventory::from_source(input, &expected, meter)?;
        let units = units::InitializationContracts::from_input(input, meter)?;
        let uses = uses::project(input, meter)?;
        Ok(Self {
            provider: input.mir.module().cone,
            expected,
            inventory,
            units,
            uses,
        })
    }
}

#[derive(Debug)]
pub enum MirTypeBridgeSourceProjectionError {
    Production(MirTypeBridgeExportProductionError),
    Resource(WireError),
    Shapes(mir::MirShapeSupportError),
    MissingSource(mir::MirTypeBridgeSourceRecordV1),
    MissingInitializationUnit(PersistentInitializationUnitId),
    InitializationInventory,
    MaterializedTypes(hir::MaterializedTypeClosureError),
    Identity(scoop_identity::IdentityReferenceError),
}
impl From<WireError> for MirTypeBridgeSourceProjectionError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for MirTypeBridgeSourceProjectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot project the MIR type bridge source: {self:?}")
    }
}
impl std::error::Error for MirTypeBridgeSourceProjectionError {}
