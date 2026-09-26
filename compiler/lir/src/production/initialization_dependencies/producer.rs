use scoop_identity::{ConeIdentity, PersistentInitializationUnitId};

use super::StrongInitializationUnitDefinitionRefV2;
use crate::{
    ExternalStrongShapeSubjectV1, ShapeLinkContractV1, StrongProductionDependencySelectionV2,
};

/// An actual local unit and its dependency definition. Symbol/definition
/// consistency is checked once when the physical import is resolved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongExternalInitializationUseV2 {
    local_unit: PersistentInitializationUnitId,
    dependency: StrongInitializationUnitDefinitionRefV2,
}

impl StrongExternalInitializationUseV2 {
    pub fn try_new(
        local_unit: PersistentInitializationUnitId,
        dependency: StrongInitializationUnitDefinitionRefV2,
        selected: &StrongProductionDependencySelectionV2<'_>,
    ) -> Result<Self, StrongExternalInitializationUseErrorV2> {
        let provider = dependency.provider();
        let unit = dependency.unit();
        if selected.consumer() == provider {
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
            || import.expected_symbol() != dependency.descriptor().symbol()
            || import.required_definition() != dependency.descriptor().plan()
        {
            return Err(StrongExternalInitializationUseErrorV2::DefinitionMismatch {
                provider,
                unit,
            });
        }
        Ok(Self {
            local_unit,
            dependency,
        })
    }

    pub const fn local_unit(self) -> PersistentInitializationUnitId {
        self.local_unit
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

impl std::fmt::Display for StrongExternalInitializationUseErrorV2 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid external initialization use: {self:?}")
    }
}

impl std::error::Error for StrongExternalInitializationUseErrorV2 {}
