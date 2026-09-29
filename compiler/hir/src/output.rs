//! Closed HIR products for one current Cone.

use std::fmt;
use std::ops::Deref;

use scoop_identity::{
    CallableMaterializationContext, CallableTemplateOwner, ExecutableSourceEntryIdentity,
    PersistentFunctionId,
};

use crate::{
    ConeOutputKind, CoreProtocols, ExportHir, HirExportBindingSurfaceValidationError,
    HirNativeBoundaryTypeDefinitions, LocalConcreteHir, LocalExecutableEntry,
    LocalExecutableEntryError, concrete,
};

mod dependencies;
pub use dependencies::*;

/// Export HIR paired with its validated library/executable contract.
#[derive(Debug, Clone)]
pub struct ExportHirOutput {
    module: ExportHir,
    output_kind: ConeOutputKind,
    shared_source: std::sync::Arc<crate::production::ExportSharedSource>,
}

impl ExportHirOutput {
    pub fn try_new(
        module: ExportHir,
        output_kind: ConeOutputKind,
    ) -> Result<Self, ExportHirOutputError> {
        Self::with_dependencies(module, output_kind, None)
    }

    pub fn try_new_with_dependencies(
        module: ExportHir,
        output_kind: ConeOutputKind,
        dependencies: &crate::SelectedImportedDependencySet,
    ) -> Result<Self, ExportHirOutputError> {
        Self::with_dependencies(module, output_kind, Some(dependencies))
    }

    fn with_dependencies(
        module: ExportHir,
        output_kind: ConeOutputKind,
        dependencies: Option<&crate::SelectedImportedDependencySet>,
    ) -> Result<Self, ExportHirOutputError> {
        module
            .export_binding_identities
            .validate_public_bindings(&module.public_export_bindings)
            .map_err(ExportHirOutputError::PublicBindings)?;
        if let ConeOutputKind::Executable { local_entry } = &output_kind {
            let expected =
                LocalExecutableEntry::try_new(&module, local_entry.local_function().function())
                    .map_err(ExportHirOutputError::InvalidEntry)?;
            if &expected != local_entry.as_ref() {
                return Err(ExportHirOutputError::EntryMismatch);
            }
        }
        let shared_source =
            crate::production::ExportSharedSource::from_export(&module, dependencies)
                .map_err(|error| ExportHirOutputError::Templates(Box::new(error)))?;
        Ok(Self {
            module,
            output_kind,
            shared_source: std::sync::Arc::new(shared_source),
        })
    }

    pub(crate) fn shared_source(&self) -> &crate::production::ExportSharedSource {
        &self.shared_source
    }

    pub const fn module(&self) -> &ExportHir {
        &self.module
    }

    pub const fn output_kind(&self) -> &ConeOutputKind {
        &self.output_kind
    }

    pub fn into_module(self) -> ExportHir {
        self.module
    }
}

impl Deref for ExportHirOutput {
    type Target = ExportHir;

    fn deref(&self) -> &Self::Target {
        self.module()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CurrentConcreteFunctionId(concrete::FunctionId);

impl CurrentConcreteFunctionId {
    pub const fn function(self) -> concrete::FunctionId {
        self.0
    }
}

/// Local-concrete projection of the validated executable entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConcreteExecutableEntry {
    identity: ExecutableSourceEntryIdentity,
    local_function: CurrentConcreteFunctionId,
}

impl ConcreteExecutableEntry {
    pub fn try_new(
        module: &LocalConcreteHir,
        source: &LocalExecutableEntry,
        function_id: concrete::FunctionId,
    ) -> Result<Self, ConcreteExecutableEntryError> {
        if source.identity().root_cone() != module.cone {
            return Err(ConcreteExecutableEntryError::NotCurrentCone);
        }
        let unit = module
            .exact_type_identities
            .get(module.unit)
            .ok_or(ConcreteExecutableEntryError::UnitHasNoExactIdentity)?
            .id();
        if source.source_signature().unit() != unit {
            return Err(ConcreteExecutableEntryError::UnitIdentityMismatch);
        }
        validate_concrete_entry(module, source.declaration(), function_id)?;
        Ok(Self {
            identity: source.identity().clone(),
            local_function: CurrentConcreteFunctionId(function_id),
        })
    }

    fn validate(&self, module: &LocalConcreteHir) -> Result<(), ConcreteExecutableEntryError> {
        if self.identity.root_cone() != module.cone {
            return Err(ConcreteExecutableEntryError::NotCurrentCone);
        }
        let unit = module
            .exact_type_identities
            .get(module.unit)
            .ok_or(ConcreteExecutableEntryError::UnitHasNoExactIdentity)?
            .id();
        if self.identity.source_signature().unit() != unit {
            return Err(ConcreteExecutableEntryError::UnitIdentityMismatch);
        }
        validate_concrete_entry(
            module,
            self.identity.declaration(),
            self.local_function.function(),
        )
    }

    pub const fn identity(&self) -> &ExecutableSourceEntryIdentity {
        &self.identity
    }

    pub const fn declaration(&self) -> PersistentFunctionId {
        self.identity.declaration()
    }

    pub const fn local_function(&self) -> CurrentConcreteFunctionId {
        self.local_function
    }
}

/// Output branch translated into the LocalConcrete HIR id domain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalConeOutputKind {
    Library,
    Executable {
        local_entry: Box<ConcreteExecutableEntry>,
    },
}

mod shape_support;
pub use shape_support::*;

mod materialized_types;
pub use materialized_types::MaterializedTypeClosureError;

/// LocalConcrete HIR paired with its closed output branch.
#[derive(Debug, Clone)]
pub struct LocalConcreteHirOutput {
    module: LocalConcreteHir,
    output_kind: LocalConeOutputKind,
    materialization: LocalShapeSupportPlan,
}

impl LocalConcreteHirOutput {
    pub fn try_new(
        module: LocalConcreteHir,
        output_kind: LocalConeOutputKind,
        materialization: LocalShapeSupportPlan,
    ) -> Result<Self, LocalConcreteHirOutputError> {
        if let LocalConeOutputKind::Executable { local_entry } = &output_kind {
            local_entry
                .validate(&module)
                .map_err(LocalConcreteHirOutputError::InvalidEntry)?;
        }
        Ok(Self {
            module,
            output_kind,
            materialization,
        })
    }

    pub const fn module(&self) -> &LocalConcreteHir {
        &self.module
    }

    pub const fn output_kind(&self) -> &LocalConeOutputKind {
        &self.output_kind
    }

    pub const fn materialization(&self) -> &LocalShapeSupportPlan {
        &self.materialization
    }

    pub fn into_module(self) -> LocalConcreteHir {
        self.module
    }
}

impl Deref for LocalConcreteHirOutput {
    type Target = LocalConcreteHir;

    fn deref(&self) -> &Self::Target {
        self.module()
    }
}

impl crate::Output {
    pub fn try_new(
        mut export: ExportHirOutput,
        mut local: LocalConcreteHirOutput,
        native_boundary_types: HirNativeBoundaryTypeDefinitions,
        warnings: Vec<scoop_ast::Diagnostic>,
        dependencies: Option<&crate::SelectedImportedDependencySet>,
    ) -> Result<Self, HirOutputError> {
        if !matches!(
            (
                &export.module().core_protocols,
                &local.module().core_protocols
            ),
            (
                CoreProtocols::Defined(_),
                concrete::ConcreteCoreProtocols::Defined(_)
            ) | (
                CoreProtocols::Imported(_),
                concrete::ConcreteCoreProtocols::Imported(_)
            )
        ) {
            return Err(HirOutputError::CoreProtocolBranchMismatch);
        }
        match (export.output_kind(), local.output_kind()) {
            (ConeOutputKind::Library, LocalConeOutputKind::Library) => {}
            (
                ConeOutputKind::Executable {
                    local_entry: export_entry,
                },
                LocalConeOutputKind::Executable {
                    local_entry: concrete_entry,
                },
            ) if export_entry.identity() == concrete_entry.identity() => {}
            (ConeOutputKind::Executable { .. }, LocalConeOutputKind::Executable { .. }) => {
                return Err(HirOutputError::EntryMismatch);
            }
            _ => return Err(HirOutputError::OutputKindMismatch),
        }
        let types = local
            .shared_declaration_type_closure()
            .map_err(HirOutputError::MaterializedTypes)?;
        if let Some(shared_source) = export
            .shared_source
            .with_materialized_types(export.module(), local.module(), &types, dependencies)
            .map_err(|error| HirOutputError::Templates(Box::new(error)))?
        {
            export.shared_source = std::sync::Arc::new(shared_source);
            let requirements = crate::PublicNominalShapeRequirementsV1::from_export_hir(&export)
                .map_err(|error| HirOutputError::ShapeRequirements(Box::new(error)))?;
            local.materialization = LocalShapeSupportPlan::try_new(local.module(), &requirements)
                .map_err(HirOutputError::ShapeSupport)?;
        }
        Ok(Self {
            export,
            local,
            native_boundary_types,
            warnings,
        })
    }

    pub const fn output_kind(&self) -> &ConeOutputKind {
        self.export.output_kind()
    }
}

fn validate_concrete_entry(
    module: &LocalConcreteHir,
    declaration: PersistentFunctionId,
    function_id: concrete::FunctionId,
) -> Result<(), ConcreteExecutableEntryError> {
    let function = module
        .functions
        .iter()
        .find_map(|(id, function)| (id == function_id).then_some(function))
        .ok_or(ConcreteExecutableEntryError::MissingFunction)?;
    if !module.top_level.contains(&function_id) {
        return Err(ConcreteExecutableEntryError::NotTopLevel);
    }
    if function.name != "main" {
        return Err(ConcreteExecutableEntryError::NotMain);
    }
    if function.receiver.value_type().is_some() {
        return Err(ConcreteExecutableEntryError::HasReceiver);
    }
    if function.materialization.template() != CallableTemplateOwner::Function(declaration) {
        return Err(ConcreteExecutableEntryError::DeclarationMismatch);
    }
    if function.materialization.context() != CallableMaterializationContext::NoSubstitution {
        return Err(ConcreteExecutableEntryError::Generic);
    }
    if function.is_suspend {
        return Err(ConcreteExecutableEntryError::Suspend);
    }
    if !function.params.is_empty() {
        return Err(ConcreteExecutableEntryError::HasParameters);
    }
    if function.return_ty != module.unit {
        return Err(ConcreteExecutableEntryError::NotUnitResult);
    }
    if !matches!(function.kind, concrete::FunctionKind::User(_)) {
        return Err(ConcreteExecutableEntryError::NotScoopDefined);
    }
    Ok(())
}

#[derive(Debug)]
pub enum ExportHirOutputError {
    PublicBindings(HirExportBindingSurfaceValidationError),
    InvalidEntry(LocalExecutableEntryError),
    EntryMismatch,
    Templates(Box<crate::GenericTemplateProductionError>),
}

impl fmt::Display for ExportHirOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PublicBindings(error) => error.fmt(formatter),
            Self::InvalidEntry(error) => error.fmt(formatter),
            Self::Templates(error) => error.fmt(formatter),
            Self::EntryMismatch => formatter
                .write_str("the executable entry identity does not match its Export HIR function"),
        }
    }
}

impl std::error::Error for ExportHirOutputError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConcreteExecutableEntryError {
    MissingFunction,
    NotTopLevel,
    NotMain,
    HasReceiver,
    NotCurrentCone,
    DeclarationMismatch,
    Generic,
    Suspend,
    HasParameters,
    NotUnitResult,
    NotScoopDefined,
    UnitHasNoExactIdentity,
    UnitIdentityMismatch,
}

impl fmt::Display for ConcreteExecutableEntryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingFunction => "the executable entry does not belong to LocalConcrete HIR",
            Self::NotTopLevel => "the executable entry is not a top-level concrete function",
            Self::NotMain => "the executable entry is not named `main`",
            Self::HasReceiver => "the executable entry has a concrete receiver",
            Self::NotCurrentCone => "the executable entry does not belong to the current Cone",
            Self::DeclarationMismatch => {
                "the concrete entry does not materialize the selected source declaration"
            }
            Self::Generic => "the concrete entry has a substitution context",
            Self::Suspend => "the concrete entry is suspend",
            Self::HasParameters => "the concrete entry has parameters",
            Self::NotUnitResult => "the concrete entry does not return `Unit`",
            Self::NotScoopDefined => "the concrete entry has no ordinary Scoop body",
            Self::UnitHasNoExactIdentity => "the concrete HIR `Unit` type has no exact identity",
            Self::UnitIdentityMismatch => {
                "the concrete HIR `Unit` identity differs from the executable source signature"
            }
        })
    }
}

impl std::error::Error for ConcreteExecutableEntryError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalConcreteHirOutputError {
    InvalidEntry(ConcreteExecutableEntryError),
}

impl fmt::Display for LocalConcreteHirOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEntry(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for LocalConcreteHirOutputError {}

#[derive(Debug)]
pub enum HirOutputError {
    OutputKindMismatch,
    EntryMismatch,
    CoreProtocolBranchMismatch,
    MaterializedTypes(crate::MaterializedTypeClosureError),
    Templates(Box<crate::GenericTemplateProductionError>),
    ShapeRequirements(Box<crate::PublicNominalShapeProjectionError>),
    ShapeSupport(LocalShapeSupportPlanError),
}

impl fmt::Display for HirOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::OutputKindMismatch => "Export HIR and LocalConcrete HIR output kinds disagree",
            Self::EntryMismatch => {
                "Export HIR and LocalConcrete HIR executable entry identities disagree"
            }
            Self::CoreProtocolBranchMismatch => {
                "Export HIR and LocalConcrete HIR compiler-protocol branches disagree"
            }
            Self::MaterializedTypes(error) => return error.fmt(formatter),
            Self::Templates(error) => return error.fmt(formatter),
            Self::ShapeRequirements(error) => return error.fmt(formatter),
            Self::ShapeSupport(error) => return error.fmt(formatter),
        })
    }
}

impl std::error::Error for HirOutputError {}
