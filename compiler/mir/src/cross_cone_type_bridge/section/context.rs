use super::*;

impl<'a> MirTypeBridgeLocalAuthorityV1<'a> {
    pub(super) fn validate<E>(self) -> Result<(), MirTypeBridgeSectionError<E>> {
        use MirTypeBridgeSectionError as Error;
        if let Self::Producer { input, .. } = self
            && input.module().cone != self.provider()
        {
            return Err(Error::ProviderContext);
        }
        if self.ordinary().artifact() != self.provider() {
            return Err(Error::ProviderContext);
        }
        self.production()
            .validate_for_artifact(self.provider())
            .map_err(|_| Error::ProviderContext)?;
        let foundation = self.foundation().as_canonical().callable_signatures();
        let surface = self.production().strong_callable_bridges().bridges();

        if foundation.len() != surface.len() {
            return Err(Error::FoundationSurface);
        }
        for bridge in surface {
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
    pub(super) fn legacy_callables<E>(
        self,
    ) -> Result<Vec<StrongCallableDefinitionOwner>, MirTypeBridgeSectionError<E>> {
        let cycle = self
            .production()
            .strong_callable_bridges()
            .initialization_cycle();
        let count = usize::from(cycle.is_some())
            .checked_add(self.ordinary().exports().len())
            .ok_or(MirTypeBridgeSectionError::ArithmeticOverflow)?;
        let mut targets = reserve(count)?;
        if let Some(cycle) = cycle {
            let scoop_identity::CallableOwner::Function(definition) = cycle.implementation() else {
                return Err(MirTypeBridgeSectionError::ProviderContext);
            };
            targets.push(StrongCallableDefinitionOwner::Function(definition));
        }
        targets.extend(
            self.ordinary()
                .exports()
                .iter()
                .map(|record| record.implementation()),
        );

        targets.sort_unstable();

        if let Some(pair) = targets.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(MirTypeBridgeSectionError::OldCallablePartition(pair[0]));
        }
        Ok(targets)
    }
}
