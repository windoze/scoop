use super::*;

mod build;
pub(super) mod dependencies;
mod dispatch_inventory;
mod error;
mod selection;
mod source;
mod wire;

pub use error::LayoutAbiSectionError;
use selection::SelectedLayoutAbiEntryV1;
pub use selection::{SelectedDependencyLayoutAbiRefV1, SelectedDependencyLayoutAbiSetV1};
pub use source::LayoutAbiSectionSourceAuthorityV1;
pub use wire::{
    CallablesResolvedCrossConeLayoutAbiSectionV1, DecodedCrossConeLayoutAbiSectionV1,
    DependencyResolvedCrossConeLayoutAbiSectionV1, DescriptorsResolvedCrossConeLayoutAbiSectionV1,
    DispatchResolvedCrossConeLayoutAbiSectionV1, ExportsResolvedCrossConeLayoutAbiSectionV1,
    LayoutsResolvedCrossConeLayoutAbiSectionV1, PhysicalImportsReplayedLayoutAbiSectionV1,
};

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
        physical_imports: Vec<crate::ExternalShapeLinkImportV1>,
        source: &impl LayoutAbiSectionSourceAuthorityV1<E>,
    ) -> Result<Self, LayoutAbiSectionError<E>> {
        build::producer(exports, dependencies, physical_imports, source)
    }

    pub fn provider(&self) -> ConeIdentity {
        self.exports.provider()
    }

    pub fn target_profile(&self) -> crate::LirTargetProfile {
        self.exports.target_profile()
    }

    pub const fn exports(&self) -> &LayoutAbiExportConstituentsV1 {
        &self.exports
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

fn reserve<T, E>(count: usize) -> Result<Vec<T>, LayoutAbiSectionError<E>> {
    let mut values = Vec::new();
    scoop_wire::allocation::try_reserve(&mut values, count, &WirePath::root())?;
    Ok(values)
}

#[cfg(test)]
mod tests;
