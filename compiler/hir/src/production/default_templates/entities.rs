//! Stable identity projection for entities referenced by default templates.

mod callables;
mod owners;
mod targets;
mod values;

use scoop_identity::{
    CallableTemplateOrigin, LexicalCallableParent, PersistentGeneratedCallableId, SignatureTypeKey,
};

use super::super::signatures::HirInterfaceSignatureProjector;
use super::errors::DefaultEntityProjectionError;
use crate::{
    DefaultCallableDeclarationV1, DefaultClassConstructorIdV1, ExportHir, HirFunctionIdentity,
    HirPropertyAccessorFunction, HirSignatureBinder, LexicalDefinitionRoot,
    PersistentLexicalRootV1, SelectedImportedCoreSet, SelectedImportedDependencySet, TypeId,
};

pub(super) struct DefaultEntityProjector<'a, 'core, 'meter> {
    export: &'a ExportHir,
    imported_core: Option<&'a SelectedImportedCoreSet<'core>>,
    imported_dependencies: Option<&'a SelectedImportedDependencySet>,
    signatures: HirInterfaceSignatureProjector<'a>,
    pub(super) resources: super::resources::ProjectionResources<'meter>,
}

impl<'a, 'core, 'meter> DefaultEntityProjector<'a, 'core, 'meter> {
    pub(super) fn new(
        export: &'a ExportHir,
        imported_core: Option<&'a SelectedImportedCoreSet<'core>>,
        imported_dependencies: Option<&'a SelectedImportedDependencySet>,
        meter: &'meter mut scoop_wire::BudgetMeter,
    ) -> Self {
        Self {
            export,
            imported_core,
            imported_dependencies,
            signatures: HirInterfaceSignatureProjector::new(export),
            resources: super::resources::ProjectionResources::new(meter),
        }
    }

    pub(super) const fn export(&self) -> &'a ExportHir {
        self.export
    }

    pub(super) fn type_key(
        &self,
        ty: TypeId,
        binders: &[HirSignatureBinder],
    ) -> Result<SignatureTypeKey, DefaultEntityProjectionError> {
        self.resources.with_meter(|meter, depth| {
            crate::production::signatures::resources::ty(
                self.export,
                ty,
                binders.len(),
                depth + 1,
                meter,
            )
        })?;
        self.signatures
            .map_type(ty, binders)
            .map_err(DefaultEntityProjectionError::Type)
    }

    pub(super) fn charge_origin(
        &self,
        origin: crate::DefinitionOrigin,
    ) -> Result<(), scoop_wire::WireError> {
        if let Some(source) = self.export.source_files.get(origin.file as usize) {
            // One source-path copy and repeated source/context hash comparisons.
            let length = source.identity.logical_path().as_str().len();
            self.resources.leaf(length)?;
            self.resources.with_meter(|meter, _| {
                meter.charge_work(
                    (length as u64).saturating_mul(4).saturating_add(256),
                    &scoop_wire::WirePath::root(),
                )
            })?;
        }
        Ok(())
    }

    pub(super) fn lexical_root(
        &self,
        root: LexicalDefinitionRoot,
    ) -> Result<PersistentLexicalRootV1, DefaultEntityProjectionError> {
        match root {
            LexicalDefinitionRoot::Function(function) => {
                match self.source_callable_declaration(function)? {
                    CallableTemplateOrigin::Function(id) => {
                        Ok(PersistentLexicalRootV1::Function(id))
                    }
                    CallableTemplateOrigin::GenericFunction(id) => {
                        Ok(PersistentLexicalRootV1::GenericFunction(id))
                    }
                    CallableTemplateOrigin::Constructor(_)
                    | CallableTemplateOrigin::Accessor(_)
                    | CallableTemplateOrigin::VariantConstructor(_) => {
                        Err(DefaultEntityProjectionError::ExpectedSourceDeclaration {
                            function: super::raw_index(function),
                        })
                    }
                }
            }
            LexicalDefinitionRoot::ClassConstructor(constructor) => {
                match self.class_constructor_id(constructor)? {
                    DefaultClassConstructorIdV1::Source(id) => {
                        Ok(PersistentLexicalRootV1::Constructor(id))
                    }
                    DefaultClassConstructorIdV1::Generated(_) => {
                        Err(DefaultEntityProjectionError::MissingIdentity {
                            kind: "source class constructor",
                            index: super::raw_index(constructor),
                        })
                    }
                }
            }
            LexicalDefinitionRoot::StructConstructor(constructor) => self
                .struct_constructor_id(constructor)
                .map(PersistentLexicalRootV1::Constructor),
            LexicalDefinitionRoot::VariantConstructor(variant) => self
                .variant_id(variant)
                .map(PersistentLexicalRootV1::EnumVariantConstructor),
        }
    }

    pub(super) fn lexical_parent(
        &self,
        root: LexicalDefinitionRoot,
    ) -> Result<LexicalCallableParent, DefaultEntityProjectionError> {
        Ok(match self.lexical_root(root)? {
            PersistentLexicalRootV1::Function(id) => LexicalCallableParent::function(id),
            PersistentLexicalRootV1::GenericFunction(id) => {
                LexicalCallableParent::generic_function(id)
            }
            PersistentLexicalRootV1::Constructor(id) => LexicalCallableParent::constructor(id),
            PersistentLexicalRootV1::EnumVariantConstructor(id) => {
                LexicalCallableParent::variant_constructor(id)
            }
        })
    }

    pub(super) fn source_callable_declaration(
        &self,
        function: crate::FunctionId,
    ) -> Result<CallableTemplateOrigin, DefaultEntityProjectionError> {
        match self.function_identity(function)? {
            HirFunctionIdentity::Source(crate::HirSourceFunctionIdentity::Plain(record)) => {
                Ok(CallableTemplateOrigin::Function(record.id()))
            }
            HirFunctionIdentity::Source(crate::HirSourceFunctionIdentity::Generic(record)) => {
                Ok(CallableTemplateOrigin::GenericFunction(record.id()))
            }
            HirFunctionIdentity::PropertyAccessor(_)
            | HirFunctionIdentity::LexicalGenerated(_)
            | HirFunctionIdentity::Initialization { .. }
            | HirFunctionIdentity::DerivedEquality(_) => {
                Err(DefaultEntityProjectionError::ExpectedSourceDeclaration {
                    function: super::raw_index(function),
                })
            }
        }
    }

    pub(super) fn callable_declaration(
        &self,
        function: crate::FunctionId,
    ) -> Result<DefaultCallableDeclarationV1, DefaultEntityProjectionError> {
        match self.function_identity(function)? {
            HirFunctionIdentity::Source(crate::HirSourceFunctionIdentity::Plain(record)) => {
                Ok(DefaultCallableDeclarationV1::Function(record.id()))
            }
            HirFunctionIdentity::Source(crate::HirSourceFunctionIdentity::Generic(record)) => {
                Ok(DefaultCallableDeclarationV1::GenericFunction(record.id()))
            }
            HirFunctionIdentity::PropertyAccessor(accessor) => {
                let id = match accessor {
                    HirPropertyAccessorFunction::Getter(id) => self
                        .export
                        .property_accessor_identities
                        .get_getter(*id)
                        .map(|identity| identity.id()),
                    HirPropertyAccessorFunction::Setter(id) => self
                        .export
                        .property_accessor_identities
                        .get_setter(*id)
                        .map(|identity| identity.id()),
                }
                .ok_or(DefaultEntityProjectionError::MissingIdentity {
                    kind: "property accessor",
                    index: super::raw_index(function),
                })?;
                Ok(DefaultCallableDeclarationV1::PropertyAccessor(id))
            }
            HirFunctionIdentity::LexicalGenerated(record)
            | HirFunctionIdentity::Initialization { record, .. } => {
                Ok(DefaultCallableDeclarationV1::Generated(record.id()))
            }
            HirFunctionIdentity::DerivedEquality(_) => {
                Err(DefaultEntityProjectionError::UnsupportedFunctionIdentity {
                    function: super::raw_index(function),
                })
            }
        }
    }

    pub(super) fn generated_function_id(
        &self,
        function: crate::FunctionId,
    ) -> Result<PersistentGeneratedCallableId, DefaultEntityProjectionError> {
        match self.function_identity(function)? {
            HirFunctionIdentity::LexicalGenerated(record)
            | HirFunctionIdentity::Initialization { record, .. } => Ok(record.id()),
            HirFunctionIdentity::Source(_)
            | HirFunctionIdentity::PropertyAccessor(_)
            | HirFunctionIdentity::DerivedEquality(_) => {
                Err(DefaultEntityProjectionError::UnsupportedFunctionIdentity {
                    function: super::raw_index(function),
                })
            }
        }
    }

    fn function_identity(
        &self,
        function: crate::FunctionId,
    ) -> Result<&crate::HirFunctionIdentity, DefaultEntityProjectionError> {
        arena_get(&self.export.functions, function).ok_or(
            DefaultEntityProjectionError::Unknown {
                kind: "function",
                index: super::raw_index(function),
            },
        )?;
        self.export.function_identities.get(function).ok_or(
            DefaultEntityProjectionError::MissingIdentity {
                kind: "function",
                index: super::raw_index(function),
            },
        )
    }
}

fn arena_get<T>(arena: &la_arena::Arena<T>, id: la_arena::Idx<T>) -> Option<&T> {
    ((super::raw_index(id) as usize) < arena.len()).then(|| &arena[id])
}

fn unknown<T>(kind: &'static str, id: la_arena::Idx<T>) -> DefaultEntityProjectionError {
    DefaultEntityProjectionError::Unknown {
        kind,
        index: super::raw_index(id),
    }
}
