//! Closed HIR products for one current Cone.

use std::collections::HashSet;
use std::fmt;
use std::ops::Deref;

use scoop_identity::{
    CallableMaterializationContext, CallableTemplateOwner, CoreBuiltinNominal, ExactTypeKey,
    ExecutableSourceEntryIdentity, PersistentExactTypeId, PersistentFunctionId, PersistentTypeId,
    SourceDeclarationKey, SourceDeclarationKind,
};

use crate::{
    ConeOutputKind, CoreProtocols, CoreShapeSupportRequirementsV1, ExportHir,
    HirNativeBoundaryTypeDefinitions, LocalConcreteHir, LocalExecutableEntry,
    LocalExecutableEntryError, concrete,
};

/// Export HIR paired with its validated library/executable contract.
#[derive(Debug, Clone)]
pub struct ExportHirOutput {
    module: ExportHir,
    output_kind: ConeOutputKind,
}

impl ExportHirOutput {
    pub fn try_new(
        module: ExportHir,
        output_kind: ConeOutputKind,
    ) -> Result<Self, ExportHirOutputError> {
        if let ConeOutputKind::Executable { local_entry } = &output_kind {
            let expected =
                LocalExecutableEntry::try_new(&module, local_entry.local_function().function())
                    .map_err(ExportHirOutputError::InvalidEntry)?;
            if &expected != local_entry.as_ref() {
                return Err(ExportHirOutputError::EntryMismatch);
            }
        }
        Ok(Self {
            module,
            output_kind,
        })
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

/// One core shape-support requirement translated into the LocalConcrete HIR
/// type-id domain without discarding its persistent HIR authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalCoreShapeSupportRoot {
    declaration: SourceDeclarationKey,
    source: PersistentTypeId,
    exact: PersistentExactTypeId,
    ty: concrete::TypeId,
    boxed_value: LocalCoreBoxedValueRequirement,
}

impl LocalCoreShapeSupportRoot {
    pub const fn declaration(&self) -> &SourceDeclarationKey {
        &self.declaration
    }

    pub const fn source(&self) -> PersistentTypeId {
        self.source
    }

    pub const fn exact(&self) -> PersistentExactTypeId {
        self.exact
    }

    pub const fn ty(&self) -> concrete::TypeId {
        self.ty
    }

    pub const fn boxed_value(&self) -> LocalCoreBoxedValueRequirement {
        self.boxed_value
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalCoreBoxedValueRequirement {
    Required,
    NotApplicable,
}

/// Complete local projection of the trusted core's strong shape-support set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalCoreShapeSupportPlan {
    roots: Vec<LocalCoreShapeSupportRoot>,
}

impl LocalCoreShapeSupportPlan {
    pub fn try_new(
        module: &LocalConcreteHir,
        requirements: &CoreShapeSupportRequirementsV1,
    ) -> Result<Self, LocalCoreShapeSupportPlanError> {
        let mut roots = Vec::with_capacity(requirements.roots().len());
        for requirement in requirements.roots() {
            let ty = module
                .exact_type_identities
                .type_for_identity(requirement.exact())
                .ok_or(LocalCoreShapeSupportPlanError::MissingExactType(
                    requirement.exact(),
                ))?;
            let record = &module.exact_type_identities[ty];
            if record.key() != &ExactTypeKey::Nominal(requirement.source()) {
                return Err(LocalCoreShapeSupportPlanError::IdentityMismatch {
                    source: requirement.source(),
                    exact: requirement.exact(),
                });
            }
            let source = requirement.source();
            let declaration = if source == CoreBuiltinNominal::Unit.identity_record().id() {
                CoreBuiltinNominal::Unit.identity_record().key().clone()
            } else if source == CoreBuiltinNominal::Any.identity_record().id() {
                CoreBuiltinNominal::Any.identity_record().key().clone()
            } else {
                let mut declarations = module
                    .structs
                    .iter()
                    .map(|(_, declaration)| &declaration.origin)
                    .chain(
                        module
                            .enums
                            .iter()
                            .map(|(_, declaration)| &declaration.origin),
                    )
                    .chain(
                        module
                            .classes
                            .iter()
                            .map(|(_, declaration)| &declaration.origin),
                    )
                    .chain(
                        module
                            .interfaces
                            .iter()
                            .map(|(_, declaration)| &declaration.origin),
                    )
                    .chain(
                        module
                            .objects
                            .iter()
                            .map(|(_, declaration)| &declaration.origin),
                    )
                    .filter_map(|origin| {
                        (origin.concrete_type_id() == Some(source))
                            .then(|| origin.source())
                            .flatten()
                            .map(|identity| identity.declaration())
                    });
                let declaration = declarations
                    .next()
                    .ok_or(LocalCoreShapeSupportPlanError::MissingSourceNominal(source))?;
                if declarations.next().is_some() {
                    return Err(LocalCoreShapeSupportPlanError::AmbiguousSourceNominal(
                        source,
                    ));
                }
                declaration.clone()
            };
            let boxed_value = match declaration.declaration_kind() {
                SourceDeclarationKind::Struct | SourceDeclarationKind::Enum => {
                    LocalCoreBoxedValueRequirement::Required
                }
                SourceDeclarationKind::Class
                | SourceDeclarationKind::Interface
                | SourceDeclarationKind::Object
                | SourceDeclarationKind::AnnotationClass => {
                    LocalCoreBoxedValueRequirement::NotApplicable
                }
                _ => {
                    return Err(LocalCoreShapeSupportPlanError::InvalidSourceNominal(source));
                }
            };
            roots.push(LocalCoreShapeSupportRoot {
                declaration,
                source,
                exact: requirement.exact(),
                ty,
                boxed_value,
            });
        }
        Ok(Self { roots })
    }

    pub fn roots(&self) -> &[LocalCoreShapeSupportRoot] {
        &self.roots
    }
}

/// The lowering contract carried by a closed LocalConcrete HIR product.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalConcreteMaterializationContract {
    Ordinary,
    CoreShapeSupport(LocalCoreShapeSupportPlan),
}

/// LocalConcrete HIR paired with its closed output branch.
#[derive(Debug, Clone)]
pub struct LocalConcreteHirOutput {
    module: LocalConcreteHir,
    output_kind: LocalConeOutputKind,
    materialization: LocalConcreteMaterializationContract,
}

impl LocalConcreteHirOutput {
    pub fn try_new(
        module: LocalConcreteHir,
        output_kind: LocalConeOutputKind,
        materialization: LocalConcreteMaterializationContract,
    ) -> Result<Self, LocalConcreteHirOutputError> {
        if matches!(
            (&module.core_protocols, &materialization),
            (
                concrete::ConcreteCoreProtocols::Imported(_),
                LocalConcreteMaterializationContract::CoreShapeSupport(_)
            )
        ) {
            return Err(LocalConcreteHirOutputError::ImportedCoreShapeSupport);
        }
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

    pub const fn materialization(&self) -> &LocalConcreteMaterializationContract {
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
        export: ExportHirOutput,
        local: LocalConcreteHirOutput,
        native_boundary_types: HirNativeBoundaryTypeDefinitions,
        warnings: Vec<scoop_ast::Diagnostic>,
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

/// Closed ordinary HIR product whose imported-core uses are bound to the
/// exact selected set that admitted them.
///
/// This wrapper is the only ordinary product accepted by the imported-core
/// MIR path. Keeping the sidecar here prevents a caller from pairing the HIR
/// graph with a selection projected from another trusted artifact.
pub struct OrdinaryHirOutput<'a> {
    output: crate::Output,
    imported_core: crate::SelectedImportedCoreSet<'a>,
}

impl<'a> OrdinaryHirOutput<'a> {
    pub fn try_new(
        output: crate::Output,
        imported_core: crate::SelectedImportedCoreSet<'a>,
    ) -> Result<Self, OrdinaryHirOutputError> {
        if output.export.module().cone == scoop_identity::ConeIdentity::CORE {
            return Err(OrdinaryHirOutputError::CurrentConeIsCore);
        }
        if !matches!(
            (
                &output.export.module().core_protocols,
                &output.local.module().core_protocols
            ),
            (
                CoreProtocols::Imported(_),
                concrete::ConcreteCoreProtocols::Imported(_)
            )
        ) {
            return Err(OrdinaryHirOutputError::DefinedCoreProtocols);
        }
        if !matches!(
            output.local.materialization(),
            LocalConcreteMaterializationContract::Ordinary
        ) {
            return Err(OrdinaryHirOutputError::CoreShapeSupportMaterialization);
        }

        let export = output.export.module();
        let local = output.local.module();
        let selected_count = imported_core.callable_count()
            + imported_core.type_count()
            + imported_core.value_count();
        let mut bindings = HashSet::with_capacity(selected_count);
        validate_imported_core_projection(
            ImportedCoreUseKind::Callable,
            export
                .imported_core_callables
                .iter()
                .map(|(id, use_)| (id.into_raw().into_u32(), use_.reference())),
            local
                .imported_core_callables
                .iter()
                .map(|(id, use_)| (id.into_raw().into_u32(), use_.reference())),
            imported_core.callable_count(),
            |reference| imported_core.resolve_callable(reference),
            &mut bindings,
        )?;
        validate_imported_core_projection(
            ImportedCoreUseKind::Type,
            export
                .imported_core_types
                .iter()
                .map(|(id, use_)| (id.into_raw().into_u32(), use_.reference())),
            local
                .imported_core_types
                .iter()
                .map(|(id, use_)| (id.into_raw().into_u32(), use_.reference())),
            imported_core.type_count(),
            |reference| imported_core.resolve_type(reference),
            &mut bindings,
        )?;
        validate_imported_core_projection(
            ImportedCoreUseKind::Value,
            export
                .imported_core_values
                .iter()
                .map(|(id, use_)| (id.into_raw().into_u32(), use_.reference())),
            local
                .imported_core_values
                .iter()
                .map(|(id, use_)| (id.into_raw().into_u32(), use_.reference())),
            imported_core.value_count(),
            |reference| imported_core.resolve_value(reference),
            &mut bindings,
        )?;

        Ok(Self {
            output,
            imported_core,
        })
    }

    pub const fn output(&self) -> &crate::Output {
        &self.output
    }

    pub const fn imported_core(&self) -> &crate::SelectedImportedCoreSet<'a> {
        &self.imported_core
    }

    pub fn into_parts(self) -> (crate::Output, crate::SelectedImportedCoreSet<'a>) {
        (self.output, self.imported_core)
    }
}

fn validate_imported_core_projection<'a, Reference: Copy + Eq>(
    kind: ImportedCoreUseKind,
    export: impl ExactSizeIterator<Item = (u32, Reference)>,
    local: impl ExactSizeIterator<Item = (u32, Reference)>,
    selected_count: usize,
    mut resolve: impl FnMut(Reference) -> Option<crate::SelectedImportedCoreTarget<'a>>,
    bindings: &mut HashSet<scoop_identity::PersistentExportBindingId>,
) -> Result<(), OrdinaryHirOutputError> {
    let export_count = export.len();
    let local_count = local.len();
    if export_count != local_count {
        return Err(OrdinaryHirOutputError::ProjectionCountMismatch {
            kind,
            export: export_count,
            local: local_count,
        });
    }
    if export_count != selected_count {
        return Err(OrdinaryHirOutputError::SelectionCountMismatch {
            kind,
            hir: export_count,
            selected: selected_count,
        });
    }
    for ((export_index, export_reference), (local_index, local_reference)) in export.zip(local) {
        if export_index != local_index || export_reference != local_reference {
            return Err(OrdinaryHirOutputError::ProjectionMismatch {
                kind,
                index: export_index,
            });
        }
        let Some(selected) = resolve(export_reference) else {
            return Err(OrdinaryHirOutputError::ForeignImportedCoreUse {
                kind,
                index: export_index,
            });
        };
        if !bindings.insert(selected.binding().persistent()) {
            return Err(OrdinaryHirOutputError::DuplicateImportedCoreBinding {
                kind,
                index: export_index,
            });
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportedCoreUseKind {
    Callable,
    Type,
    Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrdinaryHirOutputError {
    CurrentConeIsCore,
    DefinedCoreProtocols,
    CoreShapeSupportMaterialization,
    ProjectionCountMismatch {
        kind: ImportedCoreUseKind,
        export: usize,
        local: usize,
    },
    SelectionCountMismatch {
        kind: ImportedCoreUseKind,
        hir: usize,
        selected: usize,
    },
    ProjectionMismatch {
        kind: ImportedCoreUseKind,
        index: u32,
    },
    ForeignImportedCoreUse {
        kind: ImportedCoreUseKind,
        index: u32,
    },
    DuplicateImportedCoreBinding {
        kind: ImportedCoreUseKind,
        index: u32,
    },
}

impl fmt::Display for OrdinaryHirOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot seal ordinary imported-core HIR: {self:?}"
        )
    }
}

impl std::error::Error for OrdinaryHirOutputError {}

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
    if function.method.is_some() {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportHirOutputError {
    InvalidEntry(LocalExecutableEntryError),
    EntryMismatch,
}

impl fmt::Display for ExportHirOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEntry(error) => error.fmt(formatter),
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
    ImportedCoreShapeSupport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalCoreShapeSupportPlanError {
    MissingExactType(PersistentExactTypeId),
    MissingSourceNominal(PersistentTypeId),
    AmbiguousSourceNominal(PersistentTypeId),
    InvalidSourceNominal(PersistentTypeId),
    IdentityMismatch {
        source: PersistentTypeId,
        exact: PersistentExactTypeId,
    },
}

impl fmt::Display for LocalCoreShapeSupportPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot project core shape support into LocalConcrete HIR: {self:?}"
        )
    }
}

impl std::error::Error for LocalCoreShapeSupportPlanError {}

impl fmt::Display for LocalConcreteHirOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEntry(error) => error.fmt(formatter),
            Self::ImportedCoreShapeSupport => formatter
                .write_str("imported-core LocalConcrete HIR cannot materialize core shape support"),
        }
    }
}

impl std::error::Error for LocalConcreteHirOutputError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirOutputError {
    OutputKindMismatch,
    EntryMismatch,
    CoreProtocolBranchMismatch,
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
        })
    }
}

impl std::error::Error for HirOutputError {}
