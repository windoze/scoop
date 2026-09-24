//! Owned dependency transport and shared recursive graph replay.

use super::*;

/// Identity-resolved transport, without source-use or machine capability.
/// Graph replay borrows the same records and never manufactures a Selected ref.
pub struct DependencyResolvedCrossConeMirTypeBridgeSectionV1 {
    pub(super) provider: ConeIdentity,
    pub(super) exports: MirTypeBridgeExportConstituentsV1,
    pub(super) legacy: Vec<StrongCallableDefinitionOwner>,
    pub(super) selected: Vec<MirTypeBridgeDependencyV1>,
}

impl DependencyResolvedCrossConeMirTypeBridgeSectionV1 {
    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub const fn exports(&self) -> &MirTypeBridgeExportConstituentsV1 {
        &self.exports
    }

    pub fn selected_relations(&self) -> &[MirTypeBridgeDependencyV1] {
        &self.selected
    }

    pub fn dependency_view<'a>(
        &'a self,
        units: &'a [MirTypeBridgeInitializationUnitV1],
    ) -> MirTypeBridgeDependencyViewV1<'a> {
        view::LocalView {
            provider: self.provider,
            exports: &self.exports,
            units,
            legacy: &self.legacy,
        }
    }

    /// Compares the entire candidate selection with independently supplied
    /// roots and all semantic edges. Source/access replay remains a separate
    /// requirement of the containing artifact closure.
    pub fn replay_dependency_closure<E>(
        &self,
        units: &[MirTypeBridgeInitializationUnitV1],
        dependencies: &[MirTypeBridgeDependencyViewV1<'_>],
        committed: &[MirTypeBridgeDependencyV1],
        graph: &ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<(), MirTypeBridgeSectionError<E>> {
        let count = dependencies
            .len()
            .checked_add(1)
            .ok_or(MirTypeBridgeSectionError::ArithmeticOverflow)?;
        let mut tables = reserve(count, meter)?;
        tables.push(self.exports.types());
        tables.extend(dependencies.iter().map(|view| view.exports.types()));
        let types = MirTypeBridgeTypeIndexV1::try_new(&tables, meter)?;
        let expected = closure::close_views(
            self.dependency_view(units),
            dependencies,
            committed,
            graph,
            &types,
            meter,
        )?;
        meter.charge_work(
            self.selected.len() as u64 + expected.len() as u64,
            &WirePath::root(),
        )?;
        if !self
            .selected
            .iter()
            .copied()
            .eq(expected.iter().map(|entry| entry.relation))
        {
            return Err(MirTypeBridgeSectionError::SelectedClosure);
        }
        Ok(())
    }
}
