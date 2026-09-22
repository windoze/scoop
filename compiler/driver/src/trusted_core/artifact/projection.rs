//! Trusted-core HIR, MIR, and LIR selection projections.

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

    /// Projects the selected typed service through the provider's canonical LIR ABI.
    fn project_core_callable_to_lir(
        &self,
        selected: &scoop_mir::SelectedExternalMirCallable,
    ) -> Result<scoop_lir::SelectedDependencyLirCallableV1, ImportedLirCallableProjectionError>
    {
        let bridge = self
            .compile()
            .production()
            .lir_strong()
            .initialization_cycle_abi()
            .expect("a validated trusted core artifact has a core LIR bridge");
        let definitions = self
            .compile()
            .production()
            .lir_strong()
            .canonical_definitions();
        self.compile().lir().project_initialization_cycle_thrower(
            bridge,
            definitions,
            selected.implementation(),
            selected.signature().clone(),
        )
    }

    /// Atomically projects a MIR selection set into the exact LIR external
    /// body/definition/symbol authority retained by this artifact.
    pub fn project_core_callables_to_lir(
        &self,
        selected: &scoop_mir::SelectedExternalMirSet,
        mut projected: scoop_lir::SelectedExternalLirSet,
    ) -> Result<
        (
            scoop_lir::ExternalTypeDescriptor,
            scoop_lir::SelectedExternalLirSet,
        ),
        TrustedCoreLirSetProjectionError,
    > {
        let definitions = self
            .compile()
            .production()
            .lir_strong()
            .canonical_definitions();
        let string_exact = self.interface.string_capability().exact_type();
        let shape_support_plan = self
            .compile()
            .production()
            .lir_strong()
            .shape_support_plan();
        let string_shape = shape_support_plan
            .closures()
            .binary_search_by_key(&string_exact, |closure| closure.owner())
            .ok()
            .map(|index| &shape_support_plan.closures()[index])
            .ok_or(TrustedCoreLirSetProjectionError::MissingRuntimeStringShapeSupport)?;
        let string_descriptor = string_shape
            .roles()
            .type_descriptor()
            .available()
            .ok_or(TrustedCoreLirSetProjectionError::MissingRuntimeStringShapeSupport)?;
        if string_descriptor.semantic_id() != string_exact {
            return Err(TrustedCoreLirSetProjectionError::RuntimeStringShapeSupportMismatch);
        }
        let runtime_string = self
            .compile()
            .lir()
            .project_type_descriptor(definitions, string_exact)
            .map_err(TrustedCoreLirSetProjectionError::RuntimeString)?;
        if runtime_string.expected_symbol() != string_descriptor.symbol()
            || runtime_string.required_definition() != string_descriptor.definition_plan()
        {
            return Err(TrustedCoreLirSetProjectionError::RuntimeStringShapeSupportMismatch);
        }
        if let Some(id) = selected.initialization_cycle() {
            let selected = selected.callable(id).expect("the selected service exists");
            let callable = self
                .project_core_callable_to_lir(selected)
                .map_err(TrustedCoreLirSetProjectionError::Callable)?;
            projected = projected
                .with_initialization_cycle(callable)
                .map_err(TrustedCoreLirSetProjectionError::LirSet)?;
        }
        Ok((runtime_string, projected))
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

#[derive(Debug)]
pub enum TrustedCoreLirSetProjectionError {
    MissingRuntimeStringShapeSupport,
    RuntimeStringShapeSupportMismatch,
    RuntimeString(ImportedLirTypeDescriptorProjectionError),
    Callable(ImportedLirCallableProjectionError),
    LirSet(scoop_lir::SelectedExternalLirSetBuildError),
}

impl fmt::Display for TrustedCoreLirSetProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRuntimeStringShapeSupport => formatter.write_str(
                "trusted core LIR shape support is missing the runtime String descriptor",
            ),
            Self::RuntimeStringShapeSupportMismatch => formatter.write_str(
                "trusted core runtime String descriptor disagrees with its LIR shape support",
            ),
            Self::RuntimeString(error) => error.fmt(formatter),
            Self::Callable(error) => error.fmt(formatter),
            Self::LirSet(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for TrustedCoreLirSetProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MissingRuntimeStringShapeSupport | Self::RuntimeStringShapeSupportMismatch => {
                None
            }
            Self::RuntimeString(error) => Some(error),
            Self::Callable(error) => Some(error),
            Self::LirSet(error) => Some(error),
        }
    }
}
