use super::*;

/// Existing local MIR definitions used when assembling dependency references.
#[derive(Clone, Copy)]
pub struct MirTypeBridgeLocalInputV1<'a> {
    pub provider: ConeIdentity,
    pub production: &'a crate::CoreBootstrapBridgeSectionV1,
    pub ordinary: &'a crate::CrossConeMirBridgeSectionV1,
}

impl<'a> MirTypeBridgeLocalInputV1<'a> {
    pub const fn provider(self) -> ConeIdentity {
        self.provider
    }

    pub const fn production(self) -> &'a crate::CoreBootstrapBridgeSectionV1 {
        self.production
    }
    pub const fn ordinary(self) -> &'a crate::CrossConeMirBridgeSectionV1 {
        self.ordinary
    }

    pub(super) fn validate(self) -> Result<(), MirTypeBridgeSectionError> {
        if self.ordinary.artifact() != self.provider {
            return Err(MirTypeBridgeSectionError::ProviderContext);
        }
        Ok(())
    }
    pub(super) fn legacy_callables(
        self,
    ) -> Result<Vec<StrongCallableDefinitionOwner>, MirTypeBridgeSectionError> {
        let mut targets = reserve(self.ordinary().exports().len())?;
        targets.extend(
            self.ordinary()
                .exports()
                .iter()
                .map(|record| record.implementation()),
        );

        targets.sort_unstable();

        Ok(targets)
    }
}
