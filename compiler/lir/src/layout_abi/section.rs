use super::*;

mod build;
pub(super) mod dependencies;
mod error;
mod selection;
mod source;
mod wire;

pub use error::LayoutAbiSectionError;
use selection::SelectedLayoutAbiEntryV1;
pub use selection::{SelectedDependencyLayoutAbiRefV1, SelectedDependencyLayoutAbiSetV1};
pub use source::LayoutAbiSectionSourceAuthorityV1;
pub use wire::DecodedCrossConeLayoutAbiSectionV1;

/// Complete target-aware LIR semantic section. Its selected set retains both
/// terminal semantic records and checked physical imports.
pub struct CrossConeLayoutAbiSectionV1<'a> {
    pub(super) exports: LayoutAbiExportConstituentsV1,
    pub(super) dependencies: Vec<&'a CrossConeLayoutAbiSectionV1<'a>>,
    pub(super) selected: SelectedDependencyLayoutAbiSetV1<'a>,
}

impl<'a> CrossConeLayoutAbiSectionV1<'a> {
    pub fn try_new<E>(
        exports: LayoutAbiExportConstituentsV1,
        dependencies: &[&'a CrossConeLayoutAbiSectionV1<'a>],
        physical_imports: Vec<crate::ExternalShapeLinkImportV1<'a>>,
        source: &impl LayoutAbiSectionSourceAuthorityV1<E>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, LayoutAbiSectionError<E>> {
        build::producer(exports, dependencies, physical_imports, source, meter)
    }

    pub fn provider(&self) -> ConeIdentity {
        self.exports.provider()
    }

    pub fn target_profile(&self) -> crate::LirTargetProfile {
        self.exports.target_profile()
    }

    pub const fn layouts(&self) -> &crate::CanonicalExactLayoutExportsV1 {
        self.exports.layouts()
    }

    pub const fn descriptors(&self) -> &crate::CanonicalExactDescriptorExportsV1 {
        self.exports.descriptors()
    }

    pub const fn dispatch(&self) -> &crate::CanonicalExactDispatchExportsV1 {
        self.exports.dispatch()
    }

    pub const fn callables(&self) -> &crate::CanonicalExactCallableAbiExportsV1 {
        self.exports.callables()
    }

    pub const fn shape_support(&self) -> &crate::CanonicalParamFreeShapeSupportExportsV1 {
        self.exports.shape_support()
    }

    pub const fn selected(&self) -> &SelectedDependencyLayoutAbiSetV1<'a> {
        &self.selected
    }

    pub(super) fn record(
        &self,
        target: LayoutAbiSemanticTargetV1,
    ) -> Option<LayoutAbiSemanticRecordV1<'_>> {
        self.exports.record(target)
    }
}

fn reserve<T, E>(
    count: usize,
    meter: &mut BudgetMeter,
) -> Result<Vec<T>, LayoutAbiSectionError<E>> {
    meter.check_table_entries(count as u64, &WirePath::root())?;
    let mut values = Vec::new();
    meter.try_reserve_collection_slots(&mut values, count, &WirePath::root())?;
    Ok(values)
}

fn sort_work<E>(count: usize, meter: &mut BudgetMeter) -> Result<(), LayoutAbiSectionError<E>> {
    meter.check_table_entries(count as u64, &WirePath::root())?;
    let levels = usize::BITS - count.max(1).saturating_sub(1).leading_zeros();
    for _ in 0..levels {
        meter.charge_work(count as u64, &WirePath::root())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
