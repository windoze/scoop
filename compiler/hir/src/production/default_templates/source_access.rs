//! Source-side access snapshots, preserving symbolic generic class regions.

use crate::*;
use scoop_wire::WirePath;

mod call_domains;
mod domains;
pub(super) use call_domains::SourceCallDomain;
mod errors;
mod shared;
use DefaultSourceAccessProductionError as Error;
pub use errors::DefaultSourceAccessProductionError;

impl DefaultSourceAccessWitnessV1 {
    pub fn from_export_hir(
        export: &ExportHir,
        witness: &ExportDefaultAccessWitness,
    ) -> Result<Self, Error> {
        let owner = super::entities::DefaultEntityProjector::new(export, None)
            .parameter_owner(witness.owner)
            .map_err(Error::Owner)?;
        let direct =
            DefaultSourceAccessDomainV1::from_export_hir(export, &witness.call_domain.direct.0)?;
        let slot = match &witness.call_domain.slot {
            None => OptionalDefaultSourceSlotDomainV1::Absent,
            Some(slot) => OptionalDefaultSourceSlotDomainV1::Present(
                DefaultSourceAccessDomainV1::from_export_hir(export, &slot.0)?,
            ),
        };
        let target = DefaultSourceAccessDomainV1::from_export_hir(export, &witness.target_domain)?;
        Self::try_new(owner, direct, slot, target).map_err(Error::Build)
    }
}
