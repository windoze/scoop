//! Sealed MIR input for the single-Cone strong production path.

use std::fmt;

use scoop_identity::{
    CallableOwner, ExactTypeKey, PersistentExactTypeId, PersistentTypeId,
    SourceDeclarationIdentityError, SourceDeclarationKey,
};
use scoop_wire::HashError;

use crate::{
    CallableSignatureSubject, CanonicalMirFoundation, CoreBootstrapBridgeSectionV1,
    CoreMirBridgeBranchV1, CoreMirShapeSupportRootV1, EntryMirBridgeBranchV1, ExternFunctionId,
    FunctionId, GeneratedExactTypeLocation, GeneratedExactTypeOwner, GlobalId,
    InitializationUnitId, MirFoundationBuildError, MirOutput, Module, ObjectId,
    OdrFreeMirFoundation, SourceExactTypeOwner, StringConstId, Type,
};

/// One local function selected as a mandatory strong materialization root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongCallableMaterializationRoot {
    function: FunctionId,
    implementation: CallableOwner,
}

impl StrongCallableMaterializationRoot {
    pub const fn function(self) -> FunctionId {
        self.function
    }

    pub const fn implementation(self) -> CallableOwner {
        self.implementation
    }
}

/// One parameter-free source nominal whose local physical shape is owned by
/// the current Cone. Generic and structural exact types never enter this set.
#[derive(Clone, Debug, PartialEq)]
pub struct StrongSourceNominalShapeRoot {
    ty: Type,
    source: PersistentTypeId,
    exact: PersistentExactTypeId,
}

impl StrongSourceNominalShapeRoot {
    pub const fn ty(&self) -> &Type {
        &self.ty
    }

    pub const fn source(&self) -> PersistentTypeId {
        self.source
    }

    pub const fn exact(&self) -> PersistentExactTypeId {
        self.exact
    }
}

/// One MIR-generated nominal whose exact owner closes back to the current
/// Cone. ODR-owned generated nominals are rejected before a plan exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongGeneratedNominalShapeRoot {
    location: GeneratedExactTypeLocation,
    nominal: PersistentTypeId,
    exact: PersistentExactTypeId,
}

impl StrongGeneratedNominalShapeRoot {
    pub const fn location(self) -> GeneratedExactTypeLocation {
        self.location
    }

    pub const fn nominal(self) -> PersistentTypeId {
        self.nominal
    }

    pub const fn exact(self) -> PersistentExactTypeId {
        self.exact
    }
}

/// Complete root projection consumed by strong-profile LIR lowering.
///
/// The vectors contain local typed ids only after their persistent subjects
/// have been checked against the exact MIR foundation and production section.
pub struct SingleConeStrongMaterializationPlan {
    callable_roots: Vec<StrongCallableMaterializationRoot>,
    source_nominal_shapes: Vec<StrongSourceNominalShapeRoot>,
    generated_nominal_shapes: Vec<StrongGeneratedNominalShapeRoot>,
    core_shape_support_roots: Vec<CoreMirShapeSupportRootV1>,
    core_shape_support_sources: Vec<SourceDeclarationKey>,
    extern_functions: Vec<ExternFunctionId>,
    globals: Vec<GlobalId>,
    initialization_units: Vec<InitializationUnitId>,
    objects: Vec<ObjectId>,
    strings: Vec<StringConstId>,
}

impl SingleConeStrongMaterializationPlan {
    pub fn callable_roots(&self) -> &[StrongCallableMaterializationRoot] {
        &self.callable_roots
    }

    pub fn source_nominal_shapes(&self) -> &[StrongSourceNominalShapeRoot] {
        &self.source_nominal_shapes
    }

    pub fn source_nominal_shape(&self, ty: &Type) -> Option<&StrongSourceNominalShapeRoot> {
        self.source_nominal_shapes
            .iter()
            .find(|root| root.ty() == ty)
    }

    pub fn generated_nominal_shapes(&self) -> &[StrongGeneratedNominalShapeRoot] {
        &self.generated_nominal_shapes
    }

    pub fn generated_nominal_shape(
        &self,
        location: GeneratedExactTypeLocation,
    ) -> Option<StrongGeneratedNominalShapeRoot> {
        self.generated_nominal_shapes
            .iter()
            .copied()
            .find(|root| root.location() == location)
    }

    pub fn core_shape_support_roots(&self) -> &[CoreMirShapeSupportRootV1] {
        &self.core_shape_support_roots
    }

    pub fn core_shape_support_sources(&self) -> &[SourceDeclarationKey] {
        &self.core_shape_support_sources
    }

    pub fn extern_functions(&self) -> &[ExternFunctionId] {
        &self.extern_functions
    }

    pub fn globals(&self) -> &[GlobalId] {
        &self.globals
    }

    pub fn initialization_units(&self) -> &[InitializationUnitId] {
        &self.initialization_units
    }

    pub fn objects(&self) -> &[ObjectId] {
        &self.objects
    }

    pub fn strings(&self) -> &[StringConstId] {
        &self.strings
    }
}

/// MIR graph and all proofs required by the only M23-3 LIR producer entry.
pub struct SingleConeStrongMirInput {
    module: Module,
    foundation: OdrFreeMirFoundation,
    production: CoreBootstrapBridgeSectionV1,
    materialization: SingleConeStrongMaterializationPlan,
}

/// HIR-owned source declarations supplied to the MIR strong sealer.
///
/// This is an untrusted boundary input. The sealer re-derives every source
/// and exact identity and requires exact equality with the canonical MIR core
/// bridge before retaining the declarations in its materialization plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreShapeSupportSourceInput {
    NotCore,
    Core(Vec<SourceDeclarationKey>),
}

impl SingleConeStrongMirInput {
    pub fn try_new(
        module: Module,
        foundation: OdrFreeMirFoundation,
        production: CoreBootstrapBridgeSectionV1,
        core_shape_support_sources: CoreShapeSupportSourceInput,
    ) -> Result<Self, SingleConeStrongMirInputError> {
        if !module.meta.imported_core_callables.is_empty() {
            return Err(SingleConeStrongMirInputError::ImportedCoreCallablesRequireOrdinaryInput);
        }
        let expected_foundation = CanonicalMirFoundation::from_module(&module)
            .map_err(SingleConeStrongMirInputError::Foundation)?;
        if &expected_foundation != foundation.as_canonical() {
            return Err(SingleConeStrongMirInputError::FoundationMismatch);
        }

        let expected_bridges =
            crate::StrongCallableBridgeSurfaceV1::from_odr_free_foundation(&foundation);
        if production.strong_callable_bridges() != &expected_bridges {
            return Err(SingleConeStrongMirInputError::StrongCallableSurfaceMismatch);
        }

        validate_core_branch(module.cone, production.core_bridge())?;
        let callable_roots = callable_roots(&module)?;
        validate_callable_roots(&callable_roots, &expected_bridges)?;
        validate_output(&module, &production, &callable_roots)?;

        let mut source_nominal_shapes = module
            .meta
            .source_exact_types
            .iter()
            .filter_map(|identity| {
                let ExactTypeKey::Nominal(source) = identity.identity_record().key() else {
                    return None;
                };
                (identity.owner() == SourceExactTypeOwner::ConeOwned).then(|| {
                    StrongSourceNominalShapeRoot {
                        ty: identity.ty().clone(),
                        source: *source,
                        exact: identity.identity_record().id(),
                    }
                })
            })
            .collect::<Vec<_>>();
        source_nominal_shapes.sort_unstable_by_key(StrongSourceNominalShapeRoot::exact);

        let mut generated_nominal_shapes =
            Vec::with_capacity(module.meta.generated_exact_types.len());
        for identity in module.meta.generated_exact_types.iter() {
            if identity.owner() != &GeneratedExactTypeOwner::ConeOwned {
                return Err(SingleConeStrongMirInputError::OdrGeneratedNominalShape(
                    identity.location(),
                ));
            }
            generated_nominal_shapes.push(StrongGeneratedNominalShapeRoot {
                location: identity.location(),
                nominal: identity.nominal_record().id(),
                exact: identity.exact_record().id(),
            });
        }
        generated_nominal_shapes.sort_unstable_by_key(|root| root.exact());

        let core_shape_support_roots = match production.core_bridge() {
            CoreMirBridgeBranchV1::NotCore => Vec::new(),
            CoreMirBridgeBranchV1::Core(core) => core.shape_support_roots().to_vec(),
        };
        let core_shape_support_sources = validate_core_shape_support_sources(
            core_shape_support_sources,
            production.core_bridge(),
            &core_shape_support_roots,
        )?;
        for root in &core_shape_support_roots {
            let source_shape = source_nominal_shapes
                .iter()
                .find(|shape| shape.source() == root.source() && shape.exact() == root.exact());
            let Some(source_shape) = source_shape else {
                return Err(
                    SingleConeStrongMirInputError::MissingCoreShapeSupportSource {
                        source: root.source(),
                        exact: root.exact(),
                    },
                );
            };
            if !module
                .meta
                .coroutine_steps
                .iter()
                .any(|(_, step)| step.identity().result_record().id() == root.exact())
            {
                return Err(SingleConeStrongMirInputError::MissingCoreCoroutineStep(
                    root.exact(),
                ));
            }
            if !module
                .meta
                .coroutine_slots
                .iter()
                .any(|(_, slot)| slot.identity().value_record().id() == root.exact())
            {
                return Err(SingleConeStrongMirInputError::MissingCoreCoroutineSlot(
                    root.exact(),
                ));
            }
            if matches!(
                source_shape.ty(),
                Type::Unit | Type::Integer(_) | Type::Boolean | Type::Struct(_) | Type::Enum(_, _)
            ) && !module.meta.boxed_types.iter().any(|boxed| {
                matches!(
                    boxed.identity().generated_type_record().key(),
                    scoop_identity::GeneratedNominalKey::BoxedValue { payload }
                        if *payload == root.exact()
                )
            }) {
                return Err(SingleConeStrongMirInputError::MissingCoreBoxedValue(
                    root.exact(),
                ));
            }
        }
        let materialization = SingleConeStrongMaterializationPlan {
            callable_roots,
            source_nominal_shapes,
            generated_nominal_shapes,
            core_shape_support_roots,
            core_shape_support_sources,
            extern_functions: module.extern_functions.iter().map(|(id, _)| id).collect(),
            globals: module.globals.iter().map(|(id, _)| id).collect(),
            initialization_units: module
                .initialization_units
                .iter()
                .map(|(id, _)| id)
                .collect(),
            objects: module.objects.iter().map(|(id, _)| id).collect(),
            strings: module.strings.iter().map(|(id, _)| id).collect(),
        };
        Ok(Self {
            module,
            foundation,
            production,
            materialization,
        })
    }

    pub const fn module(&self) -> &Module {
        &self.module
    }

    pub const fn foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundation
    }

    pub const fn production(&self) -> &CoreBootstrapBridgeSectionV1 {
        &self.production
    }

    pub const fn materialization(&self) -> &SingleConeStrongMaterializationPlan {
        &self.materialization
    }
}

fn validate_core_shape_support_sources(
    input: CoreShapeSupportSourceInput,
    branch: &CoreMirBridgeBranchV1,
    roots: &[CoreMirShapeSupportRootV1],
) -> Result<Vec<SourceDeclarationKey>, SingleConeStrongMirInputError> {
    let sources = match (input, branch) {
        (CoreShapeSupportSourceInput::NotCore, CoreMirBridgeBranchV1::NotCore) => {
            return Ok(Vec::new());
        }
        (CoreShapeSupportSourceInput::Core(sources), CoreMirBridgeBranchV1::Core(_)) => sources,
        (CoreShapeSupportSourceInput::NotCore, CoreMirBridgeBranchV1::Core(_))
        | (CoreShapeSupportSourceInput::Core(_), CoreMirBridgeBranchV1::NotCore) => {
            return Err(SingleConeStrongMirInputError::CoreShapeSupportSourceBranchMismatch);
        }
    };
    if sources.len() != roots.len() {
        return Err(
            SingleConeStrongMirInputError::CoreShapeSupportSourceCountMismatch {
                expected: roots.len(),
                actual: sources.len(),
            },
        );
    }
    for (index, (declaration, root)) in sources.iter().zip(roots).enumerate() {
        if declaration.origin() != scoop_identity::ConeIdentity::CORE
            || !declaration.declaration_kind().is_nominal()
            || declaration.duplicate_signature().type_parameter_count() != 0
        {
            return Err(SingleConeStrongMirInputError::InvalidCoreShapeSupportSource { index });
        }
        let source = PersistentTypeId::from_source_declaration(declaration).map_err(|error| {
            SingleConeStrongMirInputError::CoreShapeSupportSourceIdentity { index, error }
        })?;
        let exact =
            PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(source)).map_err(|error| {
                SingleConeStrongMirInputError::CoreShapeSupportExactIdentity { index, error }
            })?;
        if source != root.source() || exact != root.exact() {
            return Err(
                SingleConeStrongMirInputError::CoreShapeSupportSourceMismatch(Box::new(
                    CoreShapeSupportSourceMismatch {
                        index,
                        expected_source: root.source(),
                        expected_exact: root.exact(),
                        actual_source: source,
                        actual_exact: exact,
                    },
                )),
            );
        }
    }
    Ok(sources)
}

fn validate_core_branch(
    producer: scoop_identity::ConeIdentity,
    branch: &CoreMirBridgeBranchV1,
) -> Result<(), SingleConeStrongMirInputError> {
    match (producer == scoop_identity::ConeIdentity::CORE, branch) {
        (true, CoreMirBridgeBranchV1::Core(_)) | (false, CoreMirBridgeBranchV1::NotCore) => Ok(()),
        (true, CoreMirBridgeBranchV1::NotCore) | (false, CoreMirBridgeBranchV1::Core(_)) => {
            Err(SingleConeStrongMirInputError::CoreBranchMismatch)
        }
    }
}

fn callable_roots(
    module: &Module,
) -> Result<Vec<StrongCallableMaterializationRoot>, SingleConeStrongMirInputError> {
    let mut roots = Vec::with_capacity(module.top_level.len());
    for &function in &module.top_level {
        let subject = module.meta.callable_signature_subject(function).ok_or(
            SingleConeStrongMirInputError::MissingCallableSubject(function),
        )?;
        let CallableSignatureSubject::Strong(implementation) = subject else {
            return Err(SingleConeStrongMirInputError::OdrCallableSubject(function));
        };
        roots.push(StrongCallableMaterializationRoot {
            function,
            implementation,
        });
    }
    roots.sort_unstable_by_key(|root| root.implementation);
    Ok(roots)
}

fn validate_callable_roots(
    roots: &[StrongCallableMaterializationRoot],
    bridges: &crate::StrongCallableBridgeSurfaceV1,
) -> Result<(), SingleConeStrongMirInputError> {
    for (index, root) in roots.iter().enumerate() {
        if !bridges
            .bridges()
            .iter()
            .any(|bridge| bridge.implementation() == root.implementation)
        {
            return Err(SingleConeStrongMirInputError::MissingStrongCallableBridge {
                index,
                implementation: root.implementation,
            });
        }
    }
    Ok(())
}

fn validate_output(
    module: &Module,
    production: &CoreBootstrapBridgeSectionV1,
    callable_roots: &[StrongCallableMaterializationRoot],
) -> Result<(), SingleConeStrongMirInputError> {
    match (module.output, production.entry_bridge()) {
        (MirOutput::Library, EntryMirBridgeBranchV1::Library) => Ok(()),
        (MirOutput::Executable { entry }, EntryMirBridgeBranchV1::Executable(bridge)) => {
            let implementation = callable_roots
                .iter()
                .find(|root| root.function == entry)
                .map(|root| root.implementation)
                .ok_or(SingleConeStrongMirInputError::MissingEntryRoot(entry))?;
            if implementation != bridge.implementation() {
                return Err(SingleConeStrongMirInputError::EntryImplementationMismatch {
                    expected: implementation,
                    actual: bridge.implementation(),
                });
            }
            Ok(())
        }
        (MirOutput::Library, EntryMirBridgeBranchV1::Executable(_))
        | (MirOutput::Executable { .. }, EntryMirBridgeBranchV1::Library) => {
            Err(SingleConeStrongMirInputError::OutputMismatch)
        }
    }
}

#[derive(Debug)]
pub enum SingleConeStrongMirInputError {
    ImportedCoreCallablesRequireOrdinaryInput,
    Foundation(MirFoundationBuildError),
    FoundationMismatch,
    StrongCallableSurfaceMismatch,
    CoreBranchMismatch,
    CoreShapeSupportSourceBranchMismatch,
    CoreShapeSupportSourceCountMismatch {
        expected: usize,
        actual: usize,
    },
    InvalidCoreShapeSupportSource {
        index: usize,
    },
    CoreShapeSupportSourceIdentity {
        index: usize,
        error: SourceDeclarationIdentityError,
    },
    CoreShapeSupportExactIdentity {
        index: usize,
        error: HashError,
    },
    CoreShapeSupportSourceMismatch(Box<CoreShapeSupportSourceMismatch>),
    MissingCallableSubject(FunctionId),
    OdrCallableSubject(FunctionId),
    OdrGeneratedNominalShape(GeneratedExactTypeLocation),
    MissingCoreShapeSupportSource {
        source: PersistentTypeId,
        exact: PersistentExactTypeId,
    },
    MissingCoreBoxedValue(PersistentExactTypeId),
    MissingCoreCoroutineStep(PersistentExactTypeId),
    MissingCoreCoroutineSlot(PersistentExactTypeId),
    MissingStrongCallableBridge {
        index: usize,
        implementation: CallableOwner,
    },
    OutputMismatch,
    MissingEntryRoot(FunctionId),
    EntryImplementationMismatch {
        expected: CallableOwner,
        actual: CallableOwner,
    },
}

impl fmt::Display for SingleConeStrongMirInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoreShapeSupportSourceMismatch(mismatch) => write!(
                formatter,
                "cannot seal single-Cone strong MIR input: core shape source {} expected source {:?} exact {:?}, found source {:?} exact {:?}",
                mismatch.index,
                mismatch.expected_source,
                mismatch.expected_exact,
                mismatch.actual_source,
                mismatch.actual_exact,
            ),
            _ => write!(
                formatter,
                "cannot seal single-Cone strong MIR input: {self:?}"
            ),
        }
    }
}

impl std::error::Error for SingleConeStrongMirInputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Foundation(source) => Some(source),
            Self::CoreShapeSupportSourceIdentity { error, .. } => Some(error),
            Self::CoreShapeSupportExactIdentity { error, .. } => Some(error),
            Self::ImportedCoreCallablesRequireOrdinaryInput
            | Self::FoundationMismatch
            | Self::StrongCallableSurfaceMismatch
            | Self::CoreBranchMismatch
            | Self::CoreShapeSupportSourceBranchMismatch
            | Self::CoreShapeSupportSourceCountMismatch { .. }
            | Self::InvalidCoreShapeSupportSource { .. }
            | Self::CoreShapeSupportSourceMismatch(_)
            | Self::MissingCallableSubject(_)
            | Self::OdrCallableSubject(_)
            | Self::OdrGeneratedNominalShape(_)
            | Self::MissingCoreShapeSupportSource { .. }
            | Self::MissingCoreBoxedValue(_)
            | Self::MissingCoreCoroutineStep(_)
            | Self::MissingCoreCoroutineSlot(_)
            | Self::MissingStrongCallableBridge { .. }
            | Self::OutputMismatch
            | Self::MissingEntryRoot(_)
            | Self::EntryImplementationMismatch { .. } => None,
        }
    }
}

#[derive(Debug)]
pub struct CoreShapeSupportSourceMismatch {
    index: usize,
    expected_source: PersistentTypeId,
    expected_exact: PersistentExactTypeId,
    actual_source: PersistentTypeId,
    actual_exact: PersistentExactTypeId,
}
