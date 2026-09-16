use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use scoop_hir::{
    CoreCallableDefinitionV1, CoreHirCallableCapabilityV1, CoreHirInterfaceBranchV1,
    CoreHirInterfaceV1, CoreInterfaceImportError, ImportedCoreInputs, ImportedCorePreludeTarget,
    SelectedImportedCoreSet, SelectedImportedCoreTarget,
};
use scoop_identity::{
    ConeCoordinate, ConeIdentity, CoreBuiltinNominal, CoreImportedCallableKind, Effect,
    ExactCallableSignature, ExactTypeKey, PersistentExactTypeId, PersistentExportBindingId,
    SemanticIdentitySession,
};
use scoop_lir::{
    ImportedLirCallableProjectionError, ImportedLirSelectionError,
    ImportedLirTypeDescriptorProjectionError, SelectedImportedLirCallable, SelectedImportedLirSet,
    StrongExternalLirBridgeSurfaceV1, ValidatedLirTargetSelection,
};
use scoop_mir::{
    CoreMirBridgeBranchV1, ImportedMirCallableProjectionError, ImportedMirSelectionError,
    SelectedImportedMirCallable, SelectedImportedMirSet,
};
use scoop_slib::{
    CanonicalDefinedLinkSymbolOwnerSetV1, CompositeIdentityAbiFingerprint, ConeKind,
    ConeSourceForm, DecodedSlibEnvelope, GraphValidationError, PublishViewMismatchError,
    PublishableSingleConeArtifact, SingleConeStrongProfile, SlibReadError,
    StrongCompileArtifactValidationError, StrongLinkArtifactValidationError,
    ValidatedCompileArtifact, ValidatedGraphArtifact, ValidatedSingleConeStrongLinkArtifact,
    validate_single_cone_strong_compile_artifact, validate_single_cone_strong_link_artifact,
};
use scoop_toolchain::ResolvedTargetProfile;
use scoop_wire::DecodeLimits;

use super::TrustedCoreArtifactInput;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustedCoreArtifactLoadOperation {
    Open,
    Inspect,
    Read,
}

impl fmt::Display for TrustedCoreArtifactLoadOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Open => "open",
            Self::Inspect => "inspect",
            Self::Read => "read",
        })
    }
}

#[derive(Debug)]
pub enum TrustedCoreArtifactLoadError {
    Io {
        operation: TrustedCoreArtifactLoadOperation,
        path: PathBuf,
        source: std::io::Error,
    },
    NotRegularFile(PathBuf),
    ArtifactTooLarge {
        path: PathBuf,
        actual: u64,
        limit: u64,
    },
}

impl fmt::Display for TrustedCoreArtifactLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "cannot {operation} trusted core artifact {}: {source}",
                path.display()
            ),
            Self::NotRegularFile(path) => write!(
                formatter,
                "trusted core artifact {} is not a regular file",
                path.display()
            ),
            Self::ArtifactTooLarge {
                path,
                actual,
                limit,
            } => write!(
                formatter,
                "trusted core artifact {} has {actual} bytes, exceeding the {limit}-byte input limit",
                path.display()
            ),
        }
    }
}

impl std::error::Error for TrustedCoreArtifactLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::NotRegularFile(_) | Self::ArtifactTooLarge { .. } => None,
        }
    }
}

#[derive(Debug)]
pub struct LoadedTrustedCoreArtifact {
    input: TrustedCoreArtifactInput,
    bytes: Vec<u8>,
    limits: DecodeLimits,
}

impl TrustedCoreArtifactInput {
    pub fn load(
        self,
        limits: DecodeLimits,
    ) -> Result<LoadedTrustedCoreArtifact, TrustedCoreArtifactLoadError> {
        let path = self.path.clone();
        let file = File::open(&path).map_err(|source| TrustedCoreArtifactLoadError::Io {
            operation: TrustedCoreArtifactLoadOperation::Open,
            path: path.clone(),
            source,
        })?;
        let metadata = file
            .metadata()
            .map_err(|source| TrustedCoreArtifactLoadError::Io {
                operation: TrustedCoreArtifactLoadOperation::Inspect,
                path: path.clone(),
                source,
            })?;
        if !metadata.is_file() {
            return Err(TrustedCoreArtifactLoadError::NotRegularFile(path));
        }
        require_input_size(&path, metadata.len(), limits.owned_bytes)?;

        let mut bytes = Vec::new();
        file.take(limits.owned_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|source| TrustedCoreArtifactLoadError::Io {
                operation: TrustedCoreArtifactLoadOperation::Read,
                path: path.clone(),
                source,
            })?;
        let actual = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        require_input_size(&path, actual, limits.owned_bytes)?;
        Ok(LoadedTrustedCoreArtifact {
            input: self,
            bytes,
            limits,
        })
    }
}

fn require_input_size(
    path: &Path,
    actual: u64,
    limit: u64,
) -> Result<(), TrustedCoreArtifactLoadError> {
    if actual > limit {
        Err(TrustedCoreArtifactLoadError::ArtifactTooLarge {
            path: path.to_path_buf(),
            actual,
            limit,
        })
    } else {
        Ok(())
    }
}

impl LoadedTrustedCoreArtifact {
    pub fn path(&self) -> &Path {
        self.input.path()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn validate<'input>(
        &'input self,
        target: &ResolvedTargetProfile,
    ) -> Result<ValidatedTrustedCoreArtifact<'input>, TrustedCoreArtifactValidationError> {
        self.validate_against(
            target.lir_target_selection(),
            target.c_bridge_toolchain().profile(),
        )
    }

    fn validate_against<'input>(
        &'input self,
        target_selection: ValidatedLirTargetSelection,
        c_bridge_profile: &scoop_lir::CBridgeToolchainProfileV1,
    ) -> Result<ValidatedTrustedCoreArtifact<'input>, TrustedCoreArtifactValidationError> {
        if self.input.target != target_selection {
            return Err(TrustedCoreArtifactValidationError::authority(
                TrustedCoreArtifactAuthorityError::TargetSelection {
                    expected: self.input.target,
                    actual: target_selection,
                },
            ));
        }

        let external_bridges = StrongExternalLirBridgeSurfaceV1::empty_core_bootstrap();
        let empty_core_owners = CanonicalDefinedLinkSymbolOwnerSetV1::empty_core_bootstrap();

        let link_graph = DecodedSlibEnvelope::open(&self.bytes, self.limits, target_selection)
            .map_err(|source| TrustedCoreArtifactValidationError::Envelope {
                view: TrustedCoreArtifactView::Link,
                source: Box::new(source),
            })?
            .validate_graph()
            .map_err(|source| TrustedCoreArtifactValidationError::Graph {
                view: TrustedCoreArtifactView::Link,
                source: Box::new(source),
            })?;
        validate_graph_authority(&link_graph, &self.input)?;
        let link = validate_single_cone_strong_link_artifact(
            link_graph,
            &external_bridges,
            &empty_core_owners,
            c_bridge_profile,
        )
        .map_err(|source| TrustedCoreArtifactValidationError::Link(Box::new(source)))?;

        let compile_graph = DecodedSlibEnvelope::open(&self.bytes, self.limits, target_selection)
            .map_err(|source| TrustedCoreArtifactValidationError::Envelope {
                view: TrustedCoreArtifactView::Compile,
                source: Box::new(source),
            })?
            .validate_graph()
            .map_err(|source| TrustedCoreArtifactValidationError::Graph {
                view: TrustedCoreArtifactView::Compile,
                source: Box::new(source),
            })?;
        validate_graph_authority(&compile_graph, &self.input)?;
        let mut semantic_session = SemanticIdentitySession::new();
        let compile = validate_single_cone_strong_compile_artifact(
            compile_graph,
            &external_bridges,
            &mut semantic_session,
        )
        .map_err(|source| TrustedCoreArtifactValidationError::Compile(Box::new(source)))?;

        let publication = PublishableSingleConeArtifact::from_validated_views(&compile, &link)
            .map_err(TrustedCoreArtifactValidationError::ViewMismatch)?;
        let interface = match compile.production().hir().core_interface() {
            CoreHirInterfaceBranchV1::Core(interface) => interface.as_ref().clone(),
            CoreHirInterfaceBranchV1::NotCore => {
                return Err(TrustedCoreArtifactValidationError::MissingCoreInterface);
            }
        };
        let strong_callable_bindings = match compile.production().mir().core_bridge() {
            CoreMirBridgeBranchV1::Core(bridge) => bridge
                .callable_targets()
                .iter()
                .map(|target| target.binding())
                .collect(),
            CoreMirBridgeBranchV1::NotCore => {
                return Err(TrustedCoreArtifactValidationError::MissingCoreMirBridge);
            }
        };
        let core_interface = ValidatedCoreInterface {
            interface,
            strong_callable_bindings,
        };

        Ok(ValidatedTrustedCoreArtifact {
            compile,
            link,
            authority: TrustedCoreArtifactAuthority::from_input(&self.input),
            core_interface,
            publication,
            _semantic_session: semantic_session,
        })
    }
}

fn validate_graph_authority(
    graph: &ValidatedGraphArtifact<'_>,
    input: &TrustedCoreArtifactInput,
) -> Result<(), TrustedCoreArtifactValidationError> {
    if graph.coordinate() != &input.expected_coordinate {
        return Err(TrustedCoreArtifactValidationError::authority(
            TrustedCoreArtifactAuthorityError::Coordinate {
                expected: input.expected_coordinate.clone(),
                actual: graph.coordinate().clone(),
            },
        ));
    }
    if graph.identity() != ConeIdentity::CORE {
        return Err(TrustedCoreArtifactValidationError::authority(
            TrustedCoreArtifactAuthorityError::Identity {
                actual: graph.identity(),
            },
        ));
    }
    if graph.kind() != ConeKind::Library {
        return Err(TrustedCoreArtifactValidationError::authority(
            TrustedCoreArtifactAuthorityError::Kind {
                actual: graph.kind(),
            },
        ));
    }
    if graph.source_form() != ConeSourceForm::Manifest {
        return Err(TrustedCoreArtifactValidationError::authority(
            TrustedCoreArtifactAuthorityError::SourceForm {
                actual: graph.source_form(),
            },
        ));
    }
    if !graph.direct_dependencies().is_empty() {
        return Err(TrustedCoreArtifactValidationError::authority(
            TrustedCoreArtifactAuthorityError::Dependencies {
                actual: graph.direct_dependencies().len(),
            },
        ));
    }
    let actual_abi = graph.compatibility().composite_identity_abi();
    if actual_abi != input.toolchain_compatibility {
        return Err(TrustedCoreArtifactValidationError::authority(
            TrustedCoreArtifactAuthorityError::ToolchainCompatibility {
                expected: input.toolchain_compatibility,
                actual: actual_abi,
            },
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedCoreArtifactAuthority {
    path: PathBuf,
    coordinate: ConeCoordinate,
    target: ValidatedLirTargetSelection,
    toolchain_compatibility: CompositeIdentityAbiFingerprint,
}

impl TrustedCoreArtifactAuthority {
    fn from_input(input: &TrustedCoreArtifactInput) -> Self {
        Self {
            path: input.path.clone(),
            coordinate: input.expected_coordinate.clone(),
            target: input.target,
            toolchain_compatibility: input.toolchain_compatibility,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn coordinate(&self) -> &ConeCoordinate {
        &self.coordinate
    }

    pub const fn target(&self) -> ValidatedLirTargetSelection {
        self.target
    }

    pub const fn toolchain_compatibility(&self) -> CompositeIdentityAbiFingerprint {
        self.toolchain_compatibility
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ValidatedCoreInterface {
    interface: CoreHirInterfaceV1,
    strong_callable_bindings: Vec<PersistentExportBindingId>,
}

pub struct ValidatedTrustedCoreArtifact<'input> {
    compile: ValidatedCompileArtifact<'input, SingleConeStrongProfile>,
    link: ValidatedSingleConeStrongLinkArtifact<'input>,
    authority: TrustedCoreArtifactAuthority,
    core_interface: ValidatedCoreInterface,
    publication: PublishableSingleConeArtifact,
    // Retained as the owner of the session-local identity world. It is not a
    // lookup surface and deliberately has no projection getter.
    _semantic_session: SemanticIdentitySession,
}

impl<'input> ValidatedTrustedCoreArtifact<'input> {
    pub const fn authority(&self) -> &TrustedCoreArtifactAuthority {
        &self.authority
    }

    pub fn dependency_record(&self) -> scoop_slib::DependencyRecord {
        self.compile.dependency_record()
    }

    /// Atomically projects the only HIR lookup and compiler-protocol
    /// capabilities authorized for an M23-3 consumer from this artifact's
    /// own Compile proof and core interface.
    pub fn import_core_inputs(&self) -> Result<ImportedCoreInputs<'_>, CoreInterfaceImportError> {
        self.compile.hir().import_core_inputs(
            &self.core_interface.interface,
            &self.core_interface.strong_callable_bindings,
        )
    }

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
            self.compile.hir(),
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
        self.compile
            .mir()
            .project_core_callable(
                self.compile.production().mir(),
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
            self.compile.hir(),
            &self.core_interface.interface,
            &self.core_interface.strong_callable_bindings,
        ) {
            return Err(TrustedCoreCallableSetProjectionError::ForeignHirSet);
        }
        let mut projected =
            SelectedImportedMirSet::new(self.compile.mir(), self.compile.production().mir());
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
                .compile
                .mir()
                .project_initialization_cycle_thrower(
                    self.compile.production().mir(),
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
        if !selected.belongs_to(self.compile.mir(), self.compile.production().mir()) {
            return Err(TrustedCoreCallableProjectionError::ForeignMirSelection {
                kind: selected.kind(),
            });
        }
        let bridge = self
            .compile
            .production()
            .lir()
            .core_lir_bridge()
            .core()
            .expect("a validated trusted core artifact has a core LIR bridge");
        let definitions = self.compile.production().lir().canonical_definitions();
        match selected.kind() {
            CoreImportedCallableKind::Prelude(binding) => self
                .compile
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
                .compile
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
        if !selected.belongs_to(self.compile.mir(), self.compile.production().mir()) {
            return Err(TrustedCoreLirSetProjectionError::ForeignMirSet);
        }
        let definitions = self.compile.production().lir().canonical_definitions();
        let core_bridge = self
            .compile
            .production()
            .lir()
            .core_lir_bridge()
            .core()
            .expect("a validated trusted core artifact has a core LIR bridge");
        let string_exact = self
            .core_interface
            .interface
            .string_capability()
            .exact_type();
        let shape_support = self
            .compile
            .production()
            .lir()
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
            .compile
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
            self.compile.lir(),
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

    pub const fn publication(&self) -> &PublishableSingleConeArtifact {
        &self.publication
    }

    pub const fn defined_symbols(&self) -> &CanonicalDefinedLinkSymbolOwnerSetV1 {
        self.link.link_identity_closure().defined_symbols()
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustedCoreArtifactView {
    Compile,
    Link,
}

impl fmt::Display for TrustedCoreArtifactView {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Compile => "Compile",
            Self::Link => "Link",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrustedCoreArtifactAuthorityError {
    Coordinate {
        expected: ConeCoordinate,
        actual: ConeCoordinate,
    },
    Identity {
        actual: ConeIdentity,
    },
    Kind {
        actual: ConeKind,
    },
    SourceForm {
        actual: ConeSourceForm,
    },
    Dependencies {
        actual: usize,
    },
    TargetSelection {
        expected: ValidatedLirTargetSelection,
        actual: ValidatedLirTargetSelection,
    },
    ToolchainCompatibility {
        expected: CompositeIdentityAbiFingerprint,
        actual: CompositeIdentityAbiFingerprint,
    },
}

impl fmt::Display for TrustedCoreArtifactAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Coordinate { expected, actual } => write!(
                formatter,
                "trusted core coordinate mismatch: expected {expected}, found {actual}"
            ),
            Self::Identity { actual } => write!(
                formatter,
                "trusted core identity mismatch: expected {}, found {actual}",
                ConeIdentity::CORE
            ),
            Self::Kind { actual } => write!(
                formatter,
                "trusted core must be a library, found {actual:?}"
            ),
            Self::SourceForm { actual } => write!(
                formatter,
                "trusted core must use Manifest source form, found {actual:?}"
            ),
            Self::Dependencies { actual } => write!(
                formatter,
                "trusted core must have no direct dependencies, found {actual}"
            ),
            Self::TargetSelection { expected, actual } => write!(
                formatter,
                "trusted core target mismatch: expected {expected:?}, requested {actual:?}"
            ),
            Self::ToolchainCompatibility { expected, actual } => write!(
                formatter,
                "trusted core identity ABI mismatch: expected {expected}, found {actual}"
            ),
        }
    }
}

impl std::error::Error for TrustedCoreArtifactAuthorityError {}

#[derive(Debug)]
pub enum TrustedCoreArtifactValidationError {
    Authority(Box<TrustedCoreArtifactAuthorityError>),
    Envelope {
        view: TrustedCoreArtifactView,
        source: Box<SlibReadError>,
    },
    Graph {
        view: TrustedCoreArtifactView,
        source: Box<GraphValidationError>,
    },
    Compile(Box<StrongCompileArtifactValidationError>),
    Link(Box<StrongLinkArtifactValidationError>),
    ViewMismatch(PublishViewMismatchError),
    MissingCoreInterface,
    MissingCoreMirBridge,
}

impl TrustedCoreArtifactValidationError {
    fn authority(source: TrustedCoreArtifactAuthorityError) -> Self {
        Self::Authority(Box::new(source))
    }
}

impl fmt::Display for TrustedCoreArtifactValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authority(error) => error.fmt(formatter),
            Self::Envelope { view, source } => {
                write!(
                    formatter,
                    "trusted core {view} envelope validation failed: {source}"
                )
            }
            Self::Graph { view, source } => {
                write!(
                    formatter,
                    "trusted core {view} graph validation failed: {source}"
                )
            }
            Self::Compile(error) => {
                write!(formatter, "trusted core Compile validation failed: {error}")
            }
            Self::Link(error) => write!(formatter, "trusted core Link validation failed: {error}"),
            Self::ViewMismatch(error) => error.fmt(formatter),
            Self::MissingCoreInterface => {
                formatter.write_str("trusted core Compile proof has no Core HIR interface")
            }
            Self::MissingCoreMirBridge => {
                formatter.write_str("trusted core Compile proof has no Core MIR bridge")
            }
        }
    }
}

impl std::error::Error for TrustedCoreArtifactValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Authority(error) => Some(error),
            Self::Envelope { source, .. } => Some(source.as_ref()),
            Self::Graph { source, .. } => Some(source.as_ref()),
            Self::Compile(error) => Some(error.as_ref()),
            Self::Link(error) => Some(error.as_ref()),
            Self::ViewMismatch(error) => Some(error),
            Self::MissingCoreInterface | Self::MissingCoreMirBridge => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use scoop_hir::CanonicalHirFoundation;
    use scoop_identity::ConeCoordinate;
    use scoop_lir::{
        AppleClangCompilerIdentityV1, CBridgeToolchainProfileV1, CanonicalLirFoundation,
        DarwinCBridgeDeploymentContractV1, DarwinPackedVersionV1,
    };
    use scoop_mir::CanonicalMirFoundation;
    use scoop_slib::{
        ConeRecord, IdentityFoundationArtifact, IdentityFoundationArtifactInput, ProducerRecord,
    };

    use super::*;

    #[test]
    fn loader_binds_the_exact_opened_bytes_and_budget() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("scoop.core.slib");
        std::fs::write(&path, b"trusted bytes").unwrap();
        let input = TrustedCoreArtifactInput::for_test(path.clone());

        let loaded = input.load(DecodeLimits::default()).unwrap();

        assert_eq!(loaded.path(), path);
        assert_eq!(loaded.bytes(), b"trusted bytes");
    }

    #[test]
    fn loader_rejects_an_artifact_larger_than_the_owned_input_budget() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("scoop.core.slib");
        std::fs::write(&path, b"oversized").unwrap();
        let limits = DecodeLimits {
            owned_bytes: 4,
            ..DecodeLimits::default()
        };

        assert!(matches!(
            TrustedCoreArtifactInput::for_test(path.clone()).load(limits),
            Err(TrustedCoreArtifactLoadError::ArtifactTooLarge {
                path: actual_path,
                actual: 9,
                limit: 4,
            }) if actual_path == path
        ));
    }

    #[test]
    fn validation_rejects_a_non_core_artifact_before_profile_decoding() {
        let coordinate = ConeCoordinate::new("test", "ordinary", "0.0.0").unwrap();
        let bytes = foundation_artifact(
            ConeRecord::new(
                coordinate.clone(),
                ConeKind::Library,
                ConeSourceForm::Manifest,
            )
            .unwrap(),
        );
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("scoop.core.slib");
        std::fs::write(&path, bytes).unwrap();
        let loaded = TrustedCoreArtifactInput::for_test(path)
            .load(DecodeLimits::default())
            .unwrap();

        assert!(matches!(
            loaded.validate_against(selection(), &c_bridge_profile()),
            Err(TrustedCoreArtifactValidationError::Authority(error))
                if matches!(error.as_ref(),
                    TrustedCoreArtifactAuthorityError::Coordinate { actual, .. }
                        if actual == &coordinate)
        ));
    }

    #[test]
    fn validation_requires_the_complete_strong_profile_after_core_authority() {
        let bytes = foundation_artifact(
            ConeRecord::new(
                ConeCoordinate::reserved_core(),
                ConeKind::Library,
                ConeSourceForm::Manifest,
            )
            .unwrap(),
        );
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("scoop.core.slib");
        std::fs::write(&path, bytes).unwrap();
        let loaded = TrustedCoreArtifactInput::for_test(path)
            .load(DecodeLimits::default())
            .unwrap();

        assert!(matches!(
            loaded.validate_against(selection(), &c_bridge_profile()),
            Err(TrustedCoreArtifactValidationError::Link(_))
        ));
    }

    fn foundation_artifact(cone: ConeRecord) -> Vec<u8> {
        IdentityFoundationArtifact::write(IdentityFoundationArtifactInput::new(
            ProducerRecord::new("trusted-core-test").unwrap(),
            cone,
            selection(),
            &CanonicalHirFoundation::empty(),
            &CanonicalMirFoundation::empty(),
            &CanonicalLirFoundation::empty(),
        ))
        .unwrap()
        .as_bytes()
        .to_vec()
    }

    fn selection() -> ValidatedLirTargetSelection {
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
    }

    fn c_bridge_profile() -> CBridgeToolchainProfileV1 {
        CBridgeToolchainProfileV1::new_darwin_aarch64_apple_clang(
            DarwinCBridgeDeploymentContractV1::new(
                DarwinPackedVersionV1::new(0x000d_0100).unwrap(),
                DarwinPackedVersionV1::new(0x000e_0200).unwrap(),
                Vec::new(),
            )
            .unwrap(),
            AppleClangCompilerIdentityV1::new(21, 0, 0, "clang-2100.1.1.101").unwrap(),
        )
        .unwrap()
    }
}
