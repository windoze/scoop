//! Trusted-core HIR, MIR, and LIR selection projections.

use super::*;

impl<'input> ValidatedTrustedCoreArtifact<'input> {
    /// Projects one checked param-free callable candidate through this exact
    /// trusted artifact's HIR and MIR production surfaces.
    pub fn project_core_callable_to_mir<'artifact>(
        &'artifact self,
        selected: SelectedImportedCoreTarget<'_>,
    ) -> Result<SelectedImportedMirCallable<'artifact>, TrustedCoreCallableProjectionError> {
        let ImportedCorePreludeTarget::Callable(selected_target) = selected.target() else {
            return Err(TrustedCoreCallableProjectionError::NotCallable {
                binding: selected.binding().persistent(),
            });
        };
        let binding = selected.binding();
        if !selected.belongs_to(
            self.compile().hir(),
            &self.core_interface.interface,
            &self.core_interface.strong_callable_bindings,
        ) {
            return Err(TrustedCoreCallableProjectionError::ForeignHirSelection {
                binding: binding.persistent(),
            });
        }
        let own_target = self
            .core_interface
            .interface
            .callable_targets()
            .targets()
            .iter()
            .find(|target| target.binding() == binding.persistent())
            .ok_or(TrustedCoreCallableProjectionError::MissingHirCallable {
                binding: binding.persistent(),
            })?;
        if own_target != selected_target {
            return Err(TrustedCoreCallableProjectionError::ForeignHirSelection {
                binding: binding.persistent(),
            });
        }
        let CoreCallableDefinitionV1::Function(definition) = own_target.definition() else {
            return Err(
                TrustedCoreCallableProjectionError::InvalidHirCallableDefinition {
                    binding: binding.persistent(),
                },
            );
        };
        let CoreHirCallableCapabilityV1::ParamFreeCandidate(signature) = own_target.capability()
        else {
            return Err(TrustedCoreCallableProjectionError::UnavailableHirCallable {
                binding: binding.persistent(),
            });
        };
        self.compile()
            .mir()
            .project_core_callable(
                self.compile().production().mir_core(),
                binding.persistent(),
                definition,
                signature.clone(),
            )
            .map_err(TrustedCoreCallableProjectionError::Mir)
    }

    /// Atomically projects every HIR-selected public callable and, when
    /// requested, the initialization-cycle compiler protocol through this
    /// artifact's MIR bridge. A foreign or partially projectable set does not
    /// yield a MIR sidecar.
    pub fn project_core_callables_to_mir<'artifact>(
        &'artifact self,
        selected: &SelectedImportedCoreSet<'_>,
        include_initialization_cycle_thrower: bool,
    ) -> Result<SelectedImportedMirSet<'artifact>, TrustedCoreCallableSetProjectionError> {
        if !selected.belongs_to(
            self.compile().hir(),
            &self.core_interface.interface,
            &self.core_interface.strong_callable_bindings,
        ) {
            return Err(TrustedCoreCallableSetProjectionError::ForeignHirSet);
        }
        let mut projected = SelectedImportedMirSet::new(
            self.compile().mir(),
            self.compile().production().mir_core(),
        );
        for selected in selected.callable_selections() {
            let callable = self
                .project_core_callable_to_mir(selected)
                .map_err(TrustedCoreCallableSetProjectionError::Callable)?;
            projected
                .insert(callable)
                .map_err(TrustedCoreCallableSetProjectionError::MirSet)?;
        }
        if include_initialization_cycle_thrower {
            let cycle = self
                .core_interface
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
                vec![
                    self.core_interface
                        .interface
                        .string_capability()
                        .exact_type(),
                ],
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
            projected
                .insert(callable)
                .map_err(TrustedCoreCallableSetProjectionError::MirSet)?;
        }
        Ok(projected)
    }

    /// Projects an MIR selection from this artifact into the exact LIR body,
    /// symbol request, and strong definition authority supplied by the same
    /// artifact. A value borrowed from another proof is rejected before any
    /// persistent identity is reused, even if both artifacts have equal bytes.
    pub fn project_core_callable_to_lir<'artifact>(
        &'artifact self,
        selected: &SelectedImportedMirCallable<'_>,
    ) -> Result<SelectedImportedLirCallable<'artifact>, TrustedCoreCallableProjectionError> {
        if !selected.belongs_to(self.compile().mir(), self.compile().production().mir_core()) {
            return Err(TrustedCoreCallableProjectionError::ForeignMirSelection {
                kind: selected.kind(),
            });
        }
        let bridge = self
            .compile()
            .production()
            .lir_strong()
            .core_lir_bridge()
            .core()
            .expect("a validated trusted core artifact has a core LIR bridge");
        let definitions = self
            .compile()
            .production()
            .lir_strong()
            .canonical_definitions();
        match selected.kind() {
            CoreImportedCallableKind::Prelude(binding) => self
                .compile()
                .lir()
                .project_core_callable(
                    bridge,
                    definitions,
                    binding,
                    selected.implementation(),
                    selected.signature().clone(),
                )
                .map_err(TrustedCoreCallableProjectionError::Lir),
            CoreImportedCallableKind::InitializationCycleThrower => self
                .compile()
                .lir()
                .project_initialization_cycle_thrower(
                    bridge,
                    definitions,
                    selected.implementation(),
                    selected.signature().clone(),
                )
                .map_err(TrustedCoreCallableProjectionError::Lir),
        }
    }

    /// Atomically projects a MIR selection set into the exact LIR external
    /// body/definition/symbol authority retained by this artifact.
    pub fn project_core_callables_to_lir<'artifact>(
        &'artifact self,
        selected: &SelectedImportedMirSet<'_>,
    ) -> Result<SelectedImportedLirSet<'artifact>, TrustedCoreLirSetProjectionError> {
        if !selected.belongs_to(self.compile().mir(), self.compile().production().mir_core()) {
            return Err(TrustedCoreLirSetProjectionError::ForeignMirSet);
        }
        let definitions = self
            .compile()
            .production()
            .lir_strong()
            .canonical_definitions();
        let core_bridge = self
            .compile()
            .production()
            .lir_strong()
            .core_lir_bridge()
            .core()
            .expect("a validated trusted core artifact has a core LIR bridge");
        let string_exact = self
            .core_interface
            .interface
            .string_capability()
            .exact_type();
        let shape_support = self
            .compile()
            .production()
            .lir_strong()
            .core_shape_support()
            .core()
            .ok_or(TrustedCoreLirSetProjectionError::MissingRuntimeStringShapeSupport)?;
        let string_shape = shape_support
            .closures()
            .binary_search_by_key(&string_exact, |closure| closure.owner())
            .ok()
            .map(|index| &shape_support.closures()[index])
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
            .project_core_type_descriptor(definitions, string_exact)
            .map_err(TrustedCoreLirSetProjectionError::RuntimeString)?;
        if runtime_string.expected_symbol() != string_descriptor.symbol()
            || runtime_string.required_definition().persistent()
                != string_descriptor.definition_plan()
        {
            return Err(TrustedCoreLirSetProjectionError::RuntimeStringShapeSupportMismatch);
        }
        let mut projected = SelectedImportedLirSet::try_new(
            self.compile().lir(),
            definitions,
            core_bridge,
            runtime_string,
        )
        .map_err(TrustedCoreLirSetProjectionError::LirSet)?;
        for selected in selected.callable_selections() {
            let callable = self
                .project_core_callable_to_lir(selected)
                .map_err(TrustedCoreLirSetProjectionError::Callable)?;
            projected
                .insert(callable)
                .map_err(TrustedCoreLirSetProjectionError::LirSet)?;
        }
        Ok(projected)
    }
}

#[derive(Debug)]
pub enum TrustedCoreCallableSetProjectionError {
    ForeignHirSet,
    InvalidInitializationCycleThrower,
    InitializationCycleUnitIdentity(scoop_wire::HashError),
    InitializationCycleMir(ImportedMirCallableProjectionError),
    Callable(TrustedCoreCallableProjectionError),
    MirSet(ImportedMirSelectionError),
}

impl fmt::Display for TrustedCoreCallableSetProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignHirSet => {
                formatter.write_str("selected core HIR set belongs to another artifact projection")
            }
            Self::InvalidInitializationCycleThrower => formatter
                .write_str("trusted core initialization-cycle protocol is not a source function"),
            Self::InitializationCycleUnitIdentity(error) => error.fmt(formatter),
            Self::InitializationCycleMir(error) => error.fmt(formatter),
            Self::Callable(error) => error.fmt(formatter),
            Self::MirSet(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for TrustedCoreCallableSetProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ForeignHirSet | Self::InvalidInitializationCycleThrower => None,
            Self::InitializationCycleUnitIdentity(error) => Some(error),
            Self::InitializationCycleMir(error) => Some(error),
            Self::Callable(error) => Some(error),
            Self::MirSet(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub enum TrustedCoreLirSetProjectionError {
    ForeignMirSet,
    MissingRuntimeStringShapeSupport,
    RuntimeStringShapeSupportMismatch,
    RuntimeString(ImportedLirTypeDescriptorProjectionError),
    Callable(TrustedCoreCallableProjectionError),
    LirSet(ImportedLirSelectionError),
}

impl fmt::Display for TrustedCoreLirSetProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignMirSet => {
                formatter.write_str("selected core MIR set belongs to another artifact projection")
            }
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
            Self::ForeignMirSet
            | Self::MissingRuntimeStringShapeSupport
            | Self::RuntimeStringShapeSupportMismatch => None,
            Self::RuntimeString(error) => Some(error),
            Self::Callable(error) => Some(error),
            Self::LirSet(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub enum TrustedCoreCallableProjectionError {
    NotCallable { binding: PersistentExportBindingId },
    ForeignHirSelection { binding: PersistentExportBindingId },
    MissingHirCallable { binding: PersistentExportBindingId },
    InvalidHirCallableDefinition { binding: PersistentExportBindingId },
    UnavailableHirCallable { binding: PersistentExportBindingId },
    Mir(ImportedMirCallableProjectionError),
    ForeignMirSelection { kind: CoreImportedCallableKind },
    Lir(ImportedLirCallableProjectionError),
}

impl fmt::Display for TrustedCoreCallableProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotCallable { binding } => {
                write!(formatter, "selected core binding {binding} is not callable")
            }
            Self::ForeignHirSelection { binding } => write!(
                formatter,
                "selected core HIR binding {binding} belongs to another artifact projection"
            ),
            Self::MissingHirCallable { binding } => {
                write!(formatter, "trusted core HIR callable {binding} is missing")
            }
            Self::InvalidHirCallableDefinition { binding } => write!(
                formatter,
                "trusted core HIR callable {binding} is not a param-free function definition"
            ),
            Self::UnavailableHirCallable { binding } => write!(
                formatter,
                "trusted core HIR callable {binding} is unavailable to this production profile"
            ),
            Self::Mir(error) => error.fmt(formatter),
            Self::ForeignMirSelection { kind } => write!(
                formatter,
                "selected core MIR callable {kind:?} belongs to another artifact projection"
            ),
            Self::Lir(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for TrustedCoreCallableProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Mir(error) => Some(error),
            Self::Lir(error) => Some(error),
            _ => None,
        }
    }
}
