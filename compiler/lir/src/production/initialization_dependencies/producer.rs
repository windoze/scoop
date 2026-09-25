use scoop_identity::{ConeIdentity, PersistentInitializationUnitId};

use super::StrongInitializationUnitDefinitionRefV2;
use crate::{
    ExternalStrongShapeSubjectV1, ShapeLinkContractV1, StrongProductionDependencySelectionV2,
};

/// One checked final-LIR use of a dependency initialization unit. The use
/// retains the provider's complete Strong definition and the selected
/// physical descriptor import; callers cannot construct it from a unit id.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongExternalInitializationUseV2 {
    consumer: ConeIdentity,
    local_unit: PersistentInitializationUnitId,
    dependency: StrongInitializationUnitDefinitionRefV2,
}

impl StrongExternalInitializationUseV2 {
    pub fn try_new(
        local_unit: PersistentInitializationUnitId,
        dependency: StrongInitializationUnitDefinitionRefV2,
        selected: &StrongProductionDependencySelectionV2<'_>,
    ) -> Result<Self, StrongExternalInitializationUseErrorV2> {
        let value = Self {
            consumer: selected.consumer(),
            local_unit,
            dependency,
        };
        value.validate_against(selected)?;
        Ok(value)
    }

    pub(crate) fn validate_against(
        self,
        selected: &StrongProductionDependencySelectionV2<'_>,
    ) -> Result<(), StrongExternalInitializationUseErrorV2> {
        if self.consumer != selected.consumer() {
            return Err(StrongExternalInitializationUseErrorV2::ConsumerMismatch {
                expected: self.consumer,
                actual: selected.consumer(),
            });
        }
        let provider = self.dependency.provider();
        let unit = self.dependency.unit();
        if self.consumer == provider {
            return Err(StrongExternalInitializationUseErrorV2::CurrentProvider { provider });
        }

        let import = selected
            .physical_imports()
            .records()
            .iter()
            .find(|import| {
                import.provider() == provider
                    && import.subject()
                        == ExternalStrongShapeSubjectV1::InitializationDescriptor(unit)
            })
            .ok_or(
                StrongExternalInitializationUseErrorV2::MissingPhysicalImport { provider, unit },
            )?;
        let ShapeLinkContractV1::Initialization { unit_projection } = import.contract() else {
            return Err(StrongExternalInitializationUseErrorV2::ContractKind { provider, unit });
        };
        if unit_projection.unit() != unit
            || import.expected_symbol() != self.dependency.descriptor().symbol()
            || import.required_definition() != self.dependency.descriptor().plan()
        {
            return Err(StrongExternalInitializationUseErrorV2::DefinitionMismatch {
                provider,
                unit,
            });
        }
        Ok(())
    }

    pub const fn local_unit(self) -> PersistentInitializationUnitId {
        self.local_unit
    }

    pub const fn consumer(self) -> ConeIdentity {
        self.consumer
    }

    pub const fn provider(self) -> ConeIdentity {
        self.dependency.provider()
    }

    pub const fn dependency_unit(self) -> PersistentInitializationUnitId {
        self.dependency.unit()
    }

    pub const fn dependency(self) -> StrongInitializationUnitDefinitionRefV2 {
        self.dependency
    }
}

#[derive(Debug)]
pub enum StrongExternalInitializationUseErrorV2 {
    Resource(scoop_wire::WireError),
    ConsumerMismatch {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    CurrentProvider {
        provider: ConeIdentity,
    },
    MissingPhysicalImport {
        provider: ConeIdentity,
        unit: PersistentInitializationUnitId,
    },
    ContractKind {
        provider: ConeIdentity,
        unit: PersistentInitializationUnitId,
    },
    DefinitionMismatch {
        provider: ConeIdentity,
        unit: PersistentInitializationUnitId,
    },
}

impl From<scoop_wire::WireError> for StrongExternalInitializationUseErrorV2 {
    fn from(error: scoop_wire::WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for StrongExternalInitializationUseErrorV2 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid external initialization use: {self:?}")
    }
}

impl std::error::Error for StrongExternalInitializationUseErrorV2 {}
