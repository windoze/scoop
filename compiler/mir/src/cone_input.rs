//! Complete MIR and materialization data for the shared lowering path.

use std::rc::Rc;

use scoop_identity::{
    ConeIdentity, ExactCallableSignature, ExactTypeKey, PersistentExactTypeId, PersistentTypeId,
    SourceDeclarationKey, StrongCallableDefinitionOwner,
};

use crate::{
    CallableSignatureSubject, CanonicalMirFoundation, CoreBootstrapBridgeSectionV1,
    DependencyMirOutput, EntryMirBridgeBranchV1, ExternFunctionId, ExternalCallableUseId,
    FunctionId, GeneratedExactTypeLocation, GlobalId, InitializationUnitId, MirOutput, Module,
    ObjectId, SelectedExternalMirSet, SourceExactTypeOwner, StringConstId, Type,
};

mod errors;
pub use errors::*;
mod generated_shapes;
mod initialization;
pub use generated_shapes::StrongDependencyGeneratedNominalShapeRoot;
pub use initialization::{
    StrongInitializationUnitError, StrongInitializationUnitMaterializationRoot,
};
mod external;
mod shape_support;
pub(crate) use external::validate_external_callables;
pub use shape_support::{StrongBoxedShapeSupportRoot, StrongSourceShapeSupportRoot};

/// One local function with its complete persistent signature subject.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallableMaterializationRoot {
    function: FunctionId,
    subject: CallableSignatureSubject,
}

impl CallableMaterializationRoot {
    pub const fn function(self) -> FunctionId {
        self.function
    }

    pub const fn subject(self) -> CallableSignatureSubject {
        self.subject
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

/// A source nominal that needs physical definitions in this Cone.
/// Applications retain the HIR-owned specialization group across the MIR boundary.
#[derive(Clone, Debug, PartialEq)]
pub enum SourceNominalShapeRoot {
    Cone(StrongSourceNominalShapeRoot),
    Application {
        ty: Type,
        exact: PersistentExactTypeId,
        group: scoop_identity::OdrGroupId,
    },
}

impl SourceNominalShapeRoot {
    pub const fn ty(&self) -> &Type {
        match self {
            Self::Cone(root) => root.ty(),
            Self::Application { ty, .. } => ty,
        }
    }

    pub const fn exact(&self) -> PersistentExactTypeId {
        match self {
            Self::Cone(root) => root.exact(),
            Self::Application { exact, .. } => *exact,
        }
    }

    pub const fn cone_owned(&self) -> Option<&StrongSourceNominalShapeRoot> {
        match self {
            Self::Cone(root) => Some(root),
            Self::Application { .. } => None,
        }
    }
}

/// One MIR-generated nominal owned by its source Cone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongGeneratedNominalShapeRoot {
    location: GeneratedExactTypeLocation,
    nominal: PersistentTypeId,
    exact: PersistentExactTypeId,
}

/// An actual generated shape retains the owner already established by MIR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneratedNominalShapeRoot {
    Cone(StrongGeneratedNominalShapeRoot),
    Odr {
        location: GeneratedExactTypeLocation,
        nominal: PersistentTypeId,
        exact: PersistentExactTypeId,
        group: scoop_identity::OdrGroupId,
    },
}

impl GeneratedNominalShapeRoot {
    pub const fn location(self) -> GeneratedExactTypeLocation {
        match self {
            Self::Cone(shape) => shape.location(),
            Self::Odr { location, .. } => location,
        }
    }
    pub const fn nominal(self) -> PersistentTypeId {
        match self {
            Self::Cone(shape) => shape.nominal(),
            Self::Odr { nominal, .. } => nominal,
        }
    }
    pub const fn exact(self) -> PersistentExactTypeId {
        match self {
            Self::Cone(shape) => shape.exact(),
            Self::Odr { exact, .. } => exact,
        }
    }
}

/// One ordinary dependency callable after its request-local selected bridge
/// has been resolved and its caller-side GC protocol has been retained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongExternalCallableRoot {
    callable: ExternalCallableUseId,
    provider: ConeIdentity,
    implementation: StrongCallableDefinitionOwner,
    signature: ExactCallableSignature,
    gc_effect: crate::GcEffect,
}

impl StrongExternalCallableRoot {
    pub const fn callable(&self) -> ExternalCallableUseId {
        self.callable
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
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

/// Complete root projection consumed by LIR lowering.
///
/// The vectors contain local typed ids only after their persistent subjects
/// have been checked against the exact MIR foundation and production section.
pub struct ConeMirMaterializationPlan {
    callable_roots: Vec<CallableMaterializationRoot>,
    external_callable_roots: Vec<StrongExternalCallableRoot>,
    source_nominal_shapes: Vec<SourceNominalShapeRoot>,
    generated_nominal_shapes: Vec<GeneratedNominalShapeRoot>,
    dependency_generated_nominal_shapes: Vec<StrongDependencyGeneratedNominalShapeRoot>,
    shape_support: Vec<StrongSourceShapeSupportRoot>,
    extern_functions: Vec<ExternFunctionId>,
    globals: Vec<GlobalId>,
    initialization_units: Vec<InitializationUnitId>,
    initialization_roots: Vec<StrongInitializationUnitMaterializationRoot>,
    objects: Vec<ObjectId>,
    strings: Vec<StringConstId>,
}

impl ConeMirMaterializationPlan {
    pub fn callable_roots(&self) -> &[CallableMaterializationRoot] {
        &self.callable_roots
    }

    pub fn external_callable_roots(&self) -> &[StrongExternalCallableRoot] {
        &self.external_callable_roots
    }

    pub fn source_nominal_shapes(&self) -> &[SourceNominalShapeRoot] {
        &self.source_nominal_shapes
    }

    pub fn source_nominal_shape(&self, ty: &Type) -> Option<&SourceNominalShapeRoot> {
        self.source_nominal_shapes
            .iter()
            .find(|root| root.ty() == ty)
    }

    pub fn generated_nominal_shapes(&self) -> &[GeneratedNominalShapeRoot] {
        &self.generated_nominal_shapes
    }

    pub fn dependency_generated_nominal_shapes(
        &self,
    ) -> &[StrongDependencyGeneratedNominalShapeRoot] {
        &self.dependency_generated_nominal_shapes
    }

    pub fn generated_nominal_shape(
        &self,
        location: GeneratedExactTypeLocation,
    ) -> Option<GeneratedNominalShapeRoot> {
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

/// MIR graph, identity records, dependencies, and the complete callable plan.
pub struct ConeMirInput {
    module: Module,
    foundation: Rc<CanonicalMirFoundation>,
    selected_callables: SelectedExternalMirSet,
    production: CoreBootstrapBridgeSectionV1,
    materialization: ConeMirMaterializationPlan,
}

impl ConeMirInput {
    pub fn try_new(
        output: DependencyMirOutput,
        production: CoreBootstrapBridgeSectionV1,
        shape_support_sources: Vec<SourceDeclarationKey>,
    ) -> Result<Self, ConeMirInputError> {
        let (module, foundation, selected_callables, external_callable_roots) = output.into_parts();

        if !production
            .strong_callable_bridges()
            .matches_foundation(&foundation)
        {
            return Err(ConeMirInputError::StrongCallableSurfaceMismatch);
        }

        production
            .validate_for_artifact(module.cone)
            .map_err(ConeMirInputError::Production)?;
        let callable_roots = callable_roots(&module)?;
        validate_output(&module, &production, &callable_roots)?;
        let initialization_roots = initialization::validate(&module, &callable_roots)
            .map_err(ConeMirInputError::Initialization)?;

        let mut source_nominal_shapes = module
            .meta
            .source_exact_types
            .iter()
            .filter_map(
                |identity| match (identity.identity_record().key(), identity.owner()) {
                    (ExactTypeKey::Nominal(source), SourceExactTypeOwner::Cone(provider))
                        if provider == module.cone =>
                    {
                        Some(SourceNominalShapeRoot::Cone(StrongSourceNominalShapeRoot {
                            ty: identity.ty().clone(),
                            source: *source,
                            exact: identity.identity_record().id(),
                        }))
                    }
                    (
                        ExactTypeKey::NominalApplication { .. },
                        SourceExactTypeOwner::NominalApplication(group),
                    ) => Some(SourceNominalShapeRoot::Application {
                        ty: identity.ty().clone(),
                        exact: identity.identity_record().id(),
                        group,
                    }),
                    _ => None,
                },
            )
            .collect::<Vec<_>>();
        source_nominal_shapes.sort_unstable_by_key(SourceNominalShapeRoot::exact);

        let generated_shapes::Partition {
            local: generated_nominal_shapes,
            dependencies: dependency_generated_nominal_shapes,
        } = generated_shapes::partition(&module)?;

        let shape_support =
            shape_support::validate(shape_support_sources, &module, &source_nominal_shapes)?;
        let materialization = ConeMirMaterializationPlan {
            callable_roots,
            external_callable_roots,
            source_nominal_shapes,
            generated_nominal_shapes,
            dependency_generated_nominal_shapes,
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
            selected_callables,
            production,
            materialization,
        })
    }

    pub const fn module(&self) -> &Module {
        &self.module
    }

    pub fn foundation(&self) -> &CanonicalMirFoundation {
        &self.foundation
    }

    pub const fn selected_callables(&self) -> &SelectedExternalMirSet {
        &self.selected_callables
    }

    pub const fn production(&self) -> &CoreBootstrapBridgeSectionV1 {
        &self.production
    }

    pub const fn materialization(&self) -> &ConeMirMaterializationPlan {
        &self.materialization
    }
}

fn callable_roots(module: &Module) -> Result<Vec<CallableMaterializationRoot>, ConeMirInputError> {
    let mut roots = Vec::with_capacity(module.top_level.len());
    for &function in &module.top_level {
        let subject = module
            .meta
            .callable_signature_subject(function)
            .ok_or(ConeMirInputError::MissingCallableSubject(function))?;
        roots.push(CallableMaterializationRoot { function, subject });
    }
    roots.sort_unstable_by(|left, right| left.subject.compare_sort_key(right.subject));
    Ok(roots)
}

fn validate_output(
    module: &Module,
    production: &CoreBootstrapBridgeSectionV1,
    callable_roots: &[CallableMaterializationRoot],
) -> Result<(), ConeMirInputError> {
    match (module.output, production.entry_bridge()) {
        (MirOutput::Library, EntryMirBridgeBranchV1::Library) => Ok(()),
        (MirOutput::Executable { entry, .. }, EntryMirBridgeBranchV1::Executable(bridge)) => {
            let implementation = callable_roots
                .iter()
                .find(|root| root.function == entry)
                .map(|root| root.subject)
                .ok_or(ConeMirInputError::MissingEntryRoot(entry))?;
            if implementation != CallableSignatureSubject::Strong(bridge.implementation()) {
                return Err(ConeMirInputError::EntryImplementationMismatch {
                    expected: bridge.implementation(),
                    actual: implementation,
                });
            }
            Ok(())
        }
        (MirOutput::Library, EntryMirBridgeBranchV1::Executable(_))
        | (MirOutput::Executable { .. }, EntryMirBridgeBranchV1::Library) => {
            Err(ConeMirInputError::OutputMismatch)
        }
    }
}
