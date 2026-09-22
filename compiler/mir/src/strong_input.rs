//! Sealed MIR input for the single-Cone strong production path.

use scoop_identity::{
    CallableOwner, ConeIdentity, DependencyCallableDeclarationId, ExactCallableSignature,
    ExactTypeKey, PersistentExactTypeId, PersistentTypeId, SourceDeclarationKey,
    StrongCallableDefinitionOwner,
};

use crate::{
    CallableSignatureSubject, CanonicalMirFoundation, CoreBootstrapBridgeSectionV1,
    EntryMirBridgeBranchV1, ExternFunctionId, ExternalCallableUseId, FunctionId,
    GeneratedExactTypeLocation, GeneratedExactTypeOwner, GlobalId, InitializationUnitId, MirOutput,
    Module, ObjectId, OdrFreeMirFoundation, SelectedExternalMirSet, SourceExactTypeOwner,
    StringConstId, Type,
};

mod errors;
pub use errors::*;
mod initialization;
#[cfg(test)]
pub(crate) use initialization::initialization_test_input;
pub use initialization::{
    StrongInitializationUnitError, StrongInitializationUnitMaterializationRoot,
};
mod external;
mod shape_support;
pub(crate) use external::validate_external_callables;
pub use shape_support::{StrongBoxedShapeSupportRoot, StrongSourceShapeSupportRoot};

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

/// One ordinary dependency callable after its request-local selected bridge
/// has been resolved and its caller-side GC protocol has been retained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongExternalCallableRoot {
    callable: ExternalCallableUseId,
    role: crate::CallableRole,
    provider: ConeIdentity,
    declaration: DependencyCallableDeclarationId,
    implementation: StrongCallableDefinitionOwner,
    signature: ExactCallableSignature,
    gc_effect: crate::GcEffect,
}

impl StrongExternalCallableRoot {
    pub const fn role(&self) -> crate::CallableRole {
        self.role
    }

    pub const fn callable(&self) -> ExternalCallableUseId {
        self.callable
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub const fn declaration(&self) -> DependencyCallableDeclarationId {
        self.declaration
    }

    pub const fn implementation(&self) -> StrongCallableDefinitionOwner {
        self.implementation
    }

    pub const fn signature(&self) -> &ExactCallableSignature {
        &self.signature
    }

    pub const fn gc_effect(&self) -> crate::GcEffect {
        self.gc_effect
    }
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
    external_callable_roots: Vec<StrongExternalCallableRoot>,
    source_nominal_shapes: Vec<StrongSourceNominalShapeRoot>,
    generated_nominal_shapes: Vec<StrongGeneratedNominalShapeRoot>,
    shape_support: Vec<StrongSourceShapeSupportRoot>,
    extern_functions: Vec<ExternFunctionId>,
    globals: Vec<GlobalId>,
    initialization_units: Vec<InitializationUnitId>,
    initialization_roots: Vec<StrongInitializationUnitMaterializationRoot>,
    objects: Vec<ObjectId>,
    strings: Vec<StringConstId>,
}

impl SingleConeStrongMaterializationPlan {
    pub fn callable_roots(&self) -> &[StrongCallableMaterializationRoot] {
        &self.callable_roots
    }

    pub fn external_callable_roots(&self) -> &[StrongExternalCallableRoot] {
        &self.external_callable_roots
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

    pub fn shape_support(&self) -> &[StrongSourceShapeSupportRoot] {
        &self.shape_support
    }

    pub fn extern_functions(&self) -> &[ExternFunctionId] {
        &self.extern_functions
    }

    pub fn globals(&self) -> &[GlobalId] {
        &self.globals
    }

    pub fn initialization_roots(&self) -> &[StrongInitializationUnitMaterializationRoot] {
        &self.initialization_roots
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

/// Complete external callable selection supplied to the strong MIR sealer.
/// `Unused` is valid only when the graph has no external callable entries.
pub enum StrongExternalCallableInput<'a> {
    Unused,
    Selected(&'a SelectedExternalMirSet),
}

impl SingleConeStrongMirInput {
    pub fn try_new(
        module: Module,
        foundation: OdrFreeMirFoundation,
        production: CoreBootstrapBridgeSectionV1,
        shape_support_sources: Vec<SourceDeclarationKey>,
        external_callables: StrongExternalCallableInput<'_>,
    ) -> Result<Self, SingleConeStrongMirInputError> {
        let expected_foundation = CanonicalMirFoundation::from_module(&module)
            .map_err(SingleConeStrongMirInputError::Foundation)?;
        if &expected_foundation != foundation.as_canonical() {
            return Err(SingleConeStrongMirInputError::FoundationMismatch);
        }

        if !production
            .strong_callable_bridges()
            .matches_foundation(&foundation)
        {
            return Err(SingleConeStrongMirInputError::StrongCallableSurfaceMismatch);
        }

        production
            .validate_for_artifact(module.cone)
            .map_err(SingleConeStrongMirInputError::Production)?;
        let external_callable_roots = validate_external_callables(&module, external_callables)?;
        let callable_roots = callable_roots(&module)?;
        validate_callable_roots(&callable_roots, production.strong_callable_bridges())?;
        validate_output(&module, &production, &callable_roots)?;
        let initialization_roots = initialization::validate(&module, &callable_roots)
            .map_err(SingleConeStrongMirInputError::Initialization)?;

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

        let shape_support =
            shape_support::validate(shape_support_sources, &module, &source_nominal_shapes)?;
        let materialization = SingleConeStrongMaterializationPlan {
            callable_roots,
            external_callable_roots,
            source_nominal_shapes,
            generated_nominal_shapes,
            shape_support,
            extern_functions: module.extern_functions.iter().map(|(id, _)| id).collect(),
            globals: module.globals.iter().map(|(id, _)| id).collect(),
            initialization_units: initialization_roots
                .iter()
                .map(|root| root.unit())
                .collect(),
            initialization_roots,
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
