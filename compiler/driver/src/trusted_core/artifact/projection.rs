//! Initialization service selection from the frontend language declarations.

use super::*;

impl<'input> ValidatedTrustedCoreArtifact<'input> {
    pub fn project_initialization_protocol_to_mir(
        &self,
        selected: scoop_mir::SelectedExternalMirSet,
        include_initialization_cycle_thrower: bool,
    ) -> Result<scoop_mir::SelectedExternalMirSet, TrustedCoreCallableSetProjectionError> {
        if include_initialization_cycle_thrower {
            let cycle = self
                .interface
                .compiler_protocols()
                .initialization_cycle_thrower();
            let scoop_hir::CoreProtocolCallableDefinitionV1::Function(definition) =
                cycle.definition()
            else {
                return Err(
                    TrustedCoreCallableSetProjectionError::InvalidInitializationCycleThrower,
                );
            };
            let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
                CoreBuiltinNominal::Unit.identity_record().id(),
            ))
            .map_err(TrustedCoreCallableSetProjectionError::InitializationCycleUnitIdentity)?;
            let signature = ExactCallableSignature::new(
                Effect::Ordinary,
                None,
                vec![self.interface.string_capability().exact_type()],
                unit,
            );
            let callable = self
                .compile()
                .mir()
                .project_initialization_cycle_thrower(
                    self.compile().production().mir_core(),
                    definition,
                    signature,
                )
                .map_err(TrustedCoreCallableSetProjectionError::InitializationCycleMir)?;
            return selected
                .with_initialization_cycle(callable)
                .map_err(TrustedCoreCallableSetProjectionError::MirSet);
        }
        Ok(selected)
    }
}

#[derive(Debug)]
pub enum TrustedCoreCallableSetProjectionError {
    InvalidInitializationCycleThrower,
    InitializationCycleUnitIdentity(scoop_wire::HashError),
    InitializationCycleMir(ImportedMirCallableProjectionError),
    MirSet(scoop_mir::SelectedExternalMirSetBuildError),
}

impl fmt::Display for TrustedCoreCallableSetProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInitializationCycleThrower => formatter
                .write_str("trusted core initialization-cycle protocol is not a source function"),
            Self::InitializationCycleUnitIdentity(error) => error.fmt(formatter),
            Self::InitializationCycleMir(error) => error.fmt(formatter),
            Self::MirSet(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for TrustedCoreCallableSetProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidInitializationCycleThrower => None,
            Self::InitializationCycleUnitIdentity(error) => Some(error),
            Self::InitializationCycleMir(error) => Some(error),
            Self::MirSet(error) => Some(error),
        }
    }
}
