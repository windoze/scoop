//! Same-provider physical view, without complete source/selection authority.

use super::*;
use crate::{LayoutAbiExportConstituentsV1, PhysicalImportsReplayedLayoutAbiSectionV1};

#[derive(Clone, Copy, Debug)]
pub struct ReplayedStrongLayoutExportsV2<'a> {
    pub(crate) production: &'a ReplayedStrongProductionSectionV2,
    pub(crate) exports: &'a LayoutAbiExportConstituentsV1,
}

impl ReplayedStrongProductionSectionV2 {
    /// Joins all five inventories to the actual replayed Strong plans. The
    /// returned borrow cannot be encoded or promoted to a production section.
    pub fn replay_layout_exports<'a>(
        &'a self,
        exports: &'a LayoutAbiExportConstituentsV1,
        meter: &mut BudgetMeter,
    ) -> Result<ReplayedStrongLayoutExportsV2<'a>, StrongProductionLayoutJoinError> {
        local::validate(self, exports, meter)?;
        Ok(ReplayedStrongLayoutExportsV2 {
            production: self,
            exports,
        })
    }

    /// Requires a separately replayed physical table; it never derives source
    /// roots from Strong references or grants a complete selected set.
    pub fn validate_replayed_layout_selection(
        &self,
        layout: &PhysicalImportsReplayedLayoutAbiSectionV1<'_>,
        meter: &mut BudgetMeter,
    ) -> Result<(), StrongProductionLayoutJoinError> {
        local::validate(self, layout.exports(), meter)?;
        selected::validate(self, selected::Selection::Replayed(layout), meter)
    }
}
