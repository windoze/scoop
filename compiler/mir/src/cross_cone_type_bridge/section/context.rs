use super::*;

impl<'a> MirTypeBridgeLocalAuthorityV1<'a> {
    pub(super) fn validate<E>(
        self,
        meter: &mut BudgetMeter,
    ) -> Result<(), MirTypeBridgeSectionError<E>> {
        use MirTypeBridgeSectionError as Error;
        if let Self::Producer { input, .. } = self
            && input.module().cone != self.provider()
        {
            return Err(Error::ProviderContext);
        }
        if self.ordinary().artifact() != self.provider()
            || matches!(
                self.production().core_bridge(),
                crate::CoreMirBridgeBranchV1::Core(_)
            ) != (self.provider() == ConeIdentity::CORE)
        {
            return Err(Error::ProviderContext);
        }
        let foundation = self.foundation().as_canonical().callable_signatures();
        let surface = self.production().strong_callable_bridges().bridges();
        meter.charge_work(surface.len() as u64, &WirePath::root())?;
        if foundation.len() != surface.len() {
            return Err(Error::FoundationSurface);
        }
        for bridge in surface {
            meter.charge_work(
                scoop_wire::encoded_length(bridge.signature()).map_err(Error::Encoding)?,
                &WirePath::root(),
            )?;
            let actual = foundation
                .binary_search_by(|entry| entry.subject().compare_sort_key(bridge.subject()))
                .ok()
                .map(|index| foundation[index].signature());
            if actual != Some(bridge.signature()) {
                return Err(Error::FoundationSurface);
            }
        }
        Ok(())
    }
    pub(super) fn shape_authority(self) -> MirTypeBridgeShapeRootAuthorityV1<'a> {
        match self.production().core_bridge() {
            crate::CoreMirBridgeBranchV1::Core(core) => {
                MirTypeBridgeShapeRootAuthorityV1::Core(core)
            }
            crate::CoreMirBridgeBranchV1::NotCore => MirTypeBridgeShapeRootAuthorityV1::Ordinary,
        }
    }
    pub(super) fn legacy_callables<E>(
        self,
        meter: &mut BudgetMeter,
    ) -> Result<Vec<StrongCallableDefinitionOwner>, MirTypeBridgeSectionError<E>> {
        let core_count = match self.production().core_bridge() {
            crate::CoreMirBridgeBranchV1::Core(core) => {
                core.callable_targets().len().checked_add(1)
            }
            crate::CoreMirBridgeBranchV1::NotCore => Some(0),
        }
        .ok_or(MirTypeBridgeSectionError::ArithmeticOverflow)?;
        let count = core_count
            .checked_add(self.ordinary().exports().len())
            .ok_or(MirTypeBridgeSectionError::ArithmeticOverflow)?;
        let mut targets = reserve(count, meter)?;
        if let crate::CoreMirBridgeBranchV1::Core(core) = self.production().core_bridge() {
            targets.extend(
                core.callable_targets()
                    .iter()
                    .map(|entry| StrongCallableDefinitionOwner::Function(entry.definition())),
            );
            targets.push(StrongCallableDefinitionOwner::Function(
                core.initialization_cycle_thrower().definition(),
            ));
        }
        targets.extend(
            self.ordinary()
                .exports()
                .iter()
                .map(|record| record.implementation()),
        );
        sort_work(targets.len(), meter)?;
        targets.sort_unstable();
        meter.charge_work(targets.len() as u64, &WirePath::root())?;
        if let Some(pair) = targets.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(MirTypeBridgeSectionError::OldCallablePartition(pair[0]));
        }
        Ok(targets)
    }
}
