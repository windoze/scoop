//! Source-side access snapshots, preserving symbolic generic class regions.

use crate::*;
use scoop_wire::{BudgetMeter, WirePath};

mod domains;
mod errors;
use DefaultSourceAccessProductionError as Error;
pub use errors::DefaultSourceAccessProductionError;

impl DefaultSourceAccessWitnessV1 {
    pub fn from_export_hir(
        export: &ExportHir,
        witness: &ExportDefaultAccessWitness,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let path = WirePath::root();
        meter.check_semantic_depth(4, &path)?;
        meter.charge_nodes(1, &path)?;
        meter.charge_work(1, &path)?;
        let owner = super::entities::DefaultEntityProjector::new(export, None, None, meter)
            .parameter_owner(witness.owner)
            .map_err(Error::Owner)?;
        let direct = DefaultSourceAccessDomainV1::from_export_hir(
            export,
            &witness.call_domain.direct.0,
            meter,
        )?;
        let slot = match &witness.call_domain.slot {
            None => OptionalDefaultSourceSlotDomainV1::Absent,
            Some(slot) => OptionalDefaultSourceSlotDomainV1::Present(
                DefaultSourceAccessDomainV1::from_export_hir(export, &slot.0, meter)?,
            ),
        };
        let target =
            DefaultSourceAccessDomainV1::from_export_hir(export, &witness.target_domain, meter)?;
        Self::try_new(owner, direct, slot, target).map_err(Error::Build)
    }
}
