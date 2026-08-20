//! Dependency bodies share the current request's type arena, but retain their
//! provider declaration ids. Pending statements are lowering scratch state;
//! the completed Export HIR owns ordinary, structurally complete bodies.

mod abstract_members;
mod closures;
mod completion;
mod context;
mod delegates;
pub(crate) mod intrinsic;
mod local;
mod methods;
mod prepare;
mod protocols;
mod release;

use std::collections::BTreeMap;
use std::ops::{Deref, Index};

use hir::ImportedCallableSource;
use la_arena::{Arena, Idx, RawIdx};
use scoop_hir as hir;
use scoop_identity::CallableTemplateOrigin;

use crate::Lowerer;
use crate::imported_core::ImportedTypeBindings;

#[derive(Clone, Default)]
pub(crate) struct ImportedGenericTemplates {
    templates: Vec<Option<PreparedImportedGeneric>>,
    by_declaration: BTreeMap<CallableTemplateOrigin, hir::ImportedGenericCallableTemplateId>,
    local_functions: BTreeMap<CallableTemplateOrigin, ImportedLocalFunctionSource>,
    closures: BTreeMap<
        scoop_identity::PersistentGeneratedCallableId,
        hir::ImportedGenericCallableTemplateId,
    >,
    delegates: BTreeMap<
        scoop_identity::PersistentExtensionPropertyId,
        hir::ImportedGenericDelegateTemplateId,
    >,
}

#[derive(Clone)]
struct ImportedLocalFunctionSource {
    descriptor: hir::DefaultLocalFunctionV1,
    capture_bindings: Vec<hir::BindingId>,
}

#[derive(Clone)]
pub(crate) struct PreparedImportedGeneric {
    pub(crate) signature: hir::ImportedGenericCallableSignature,
    pub(crate) source: PreparedImportedCallableSource,
    pub(crate) bindings: ImportedTypeBindings,
    pub(crate) locals: Arena<hir::Local>,
    statements: Option<Vec<hir::Statement>>,
}

#[derive(Clone)]
pub(crate) enum PreparedImportedCallableSource {
    Declaration(Box<hir::ImportedCallableDeclaration>),
    Body(hir::ImportedCallableBody),
    InitializationEnsure,
}

impl PreparedImportedCallableSource {
    pub(crate) fn source_location(
        &self,
        source: &scoop_identity::SourceIdentity,
        context: scoop_identity::PersistentSourceContextId,
    ) -> Option<hir::ImportedDependencyDefinitionSource<'_>> {
        match self {
            Self::Declaration(declaration) => declaration.source_location(source, context),
            Self::Body(body) => body.source_location(source, context),
            Self::InitializationEnsure => None,
        }
    }
    pub(crate) fn body(&self) -> &hir::ExportGenericCallableBodyV1 {
        match self {
            Self::Declaration(declaration) => declaration
                .callable_body()
                .expect("prepared source declaration has an implementation"),
            Self::Body(body) => body.body(),
            Self::InitializationEnsure => {
                panic!("initialization ensure has no portable source body")
            }
        }
    }

    pub(crate) fn declaration(&self) -> &hir::ImportedCallableDeclaration {
        match self {
            Self::Declaration(declaration) => declaration,
            Self::Body(_) | Self::InitializationEnsure => {
                panic!("implementation bodies are not source lookup candidates")
            }
        }
    }

    pub(crate) fn definition_source(
        &self,
        source: &hir::ExportDefinitionSourceV1,
    ) -> Option<hir::ImportedDependencyDefinitionSource<'_>> {
        match self {
            Self::Declaration(declaration) => declaration.definition_source(source),
            Self::Body(body) => body.definition_source(source),
            Self::InitializationEnsure => None,
        }
    }
}

impl Deref for PreparedImportedGeneric {
    type Target = hir::ImportedGenericCallableSignature;
    fn deref(&self) -> &Self::Target {
        &self.signature
    }
}

impl Index<hir::ImportedGenericCallableTemplateId> for ImportedGenericTemplates {
    type Output = PreparedImportedGeneric;
    fn index(&self, id: hir::ImportedGenericCallableTemplateId) -> &Self::Output {
        self.templates[u32::from(id.into_raw()) as usize]
            .as_ref()
            .expect("a referenced template has a complete signature")
    }
}

impl ImportedGenericTemplates {
    pub(crate) fn into_completed(self) -> Arena<hir::ImportedGenericCallableTemplate> {
        self.templates
            .into_iter()
            .map(|template| {
                let template =
                    template.expect("successful lowering completed every dependency signature");
                let provider = template.origin.provider;
                hir::ImportedGenericCallableTemplate {
                    signature: template.signature,
                    implementation: match template.source {
                        PreparedImportedCallableSource::InitializationEnsure => {
                            hir::FunctionKind::InitializationEnsure
                        }
                        PreparedImportedCallableSource::Declaration(ref declaration)
                            if matches!(
                                declaration.interface().effects().implementation(),
                                hir::CallableImplementationV1::Intrinsic(_)
                            ) =>
                        {
                            let hir::CallableImplementationV1::Intrinsic(kind) =
                                declaration.interface().effects().implementation()
                            else {
                                unreachable!()
                            };
                            hir::FunctionKind::Intrinsic(hir::IntrinsicFunction { kind, provider })
                        }
                        PreparedImportedCallableSource::Declaration(declaration)
                            if declaration.interface().modality()
                                == hir::CallableModalityV1::Abstract =>
                        {
                            hir::FunctionKind::Abstract {
                                locals: template.locals,
                            }
                        }
                        PreparedImportedCallableSource::Declaration(_)
                        | PreparedImportedCallableSource::Body(_) => {
                            hir::FunctionKind::User(hir::Body {
                                locals: template.locals,
                                statements: template.statements.expect(
                                    "successful lowering completed every queued dependency body",
                                ),
                            })
                        }
                    },
                }
            })
            .collect()
    }
}

impl Lowerer {
    pub(crate) fn request_imported_generic_template(
        &mut self,
        declaration: hir::ImportedCallableDeclaration,
    ) -> Result<hir::ImportedGenericCallableTemplateId, String> {
        let key = declaration.interface().declaration();
        if let Some(id) = self.imported_generic_templates.by_declaration.get(&key) {
            return Ok(*id);
        }
        let origin = if let hir::PublicDeclarationOwnerV1::Nominal(owner) =
            declaration.interface().owner()
        {
            let nominal = self
                .dependencies
                .as_ref()
                .expect("member bodies have a dependency catalog")
                .nominal_declaration(owner)
                .cloned()
                .ok_or("member owner is missing")?;
            let member = match key {
                CallableTemplateOrigin::Function(id) => {
                    hir::DefaultCallableDeclarationV1::Function(id)
                }
                CallableTemplateOrigin::GenericFunction(id) => {
                    hir::DefaultCallableDeclarationV1::GenericFunction(id)
                }
                CallableTemplateOrigin::Accessor(id) => {
                    hir::DefaultCallableDeclarationV1::PropertyAccessor(id)
                }
                CallableTemplateOrigin::Constructor(_)
                | CallableTemplateOrigin::VariantConstructor(_) => {
                    return Err(
                        "dependency constructor has a distinct initialization template".into(),
                    );
                }
            };
            let (modifier, dispatch) =
                self.imported_template_method_dispatch(&declaration, &nominal)?;
            hir::ImportedCallableTemplateOrigin::Nominal {
                declaration: member,
                owner,
                owner_parameter_count: nominal.interface.type_parameters().binders().len(),
                modifier,
                dispatch,
            }
        } else if let CallableTemplateOrigin::GenericFunction(origin) = key {
            hir::ImportedCallableTemplateOrigin::Generic(origin)
        } else if let CallableTemplateOrigin::Function(origin) = key
            && matches!(
                declaration.interface().effects().implementation(),
                hir::CallableImplementationV1::Intrinsic(_)
            )
        {
            hir::ImportedCallableTemplateOrigin::Intrinsic(origin)
        } else if let CallableTemplateOrigin::Accessor(origin) = key
            && declaration.interface().owner() == hir::PublicDeclarationOwnerV1::Extension
            && self
                .dependencies
                .as_ref()
                .and_then(|dependencies| dependencies.property_for_accessor(origin))
                .is_some_and(|property| !property.type_parameters().is_empty())
        {
            hir::ImportedCallableTemplateOrigin::ExtensionAccessor(origin)
        } else {
            return Err("dependency callable does not name a generic source body".into());
        };
        let id = Idx::from_raw(RawIdx::from(
            u32::try_from(self.imported_generic_templates.templates.len())
                .expect("dependency template arena fits u32"),
        ));
        self.imported_generic_templates.templates.push(None);
        self.imported_generic_templates
            .by_declaration
            .insert(key, id);
        let prepared = if matches!(
            declaration.interface().effects().implementation(),
            hir::CallableImplementationV1::Intrinsic(_)
        ) {
            self.prepare_imported_intrinsic(declaration, origin)?
        } else if declaration.interface().modality() == hir::CallableModalityV1::Abstract {
            self.prepare_imported_abstract_member(declaration, origin)?
        } else {
            self.prepare_imported_generic(declaration, origin)?
        };
        self.imported_generic_templates.templates[id.into_raw().into_u32() as usize] =
            Some(prepared);
        Ok(id)
    }

    fn insert_imported_template(
        &mut self,
        key: CallableTemplateOrigin,
        prepared: PreparedImportedGeneric,
    ) -> hir::ImportedGenericCallableTemplateId {
        let id = self.allocate_imported_template(prepared);
        self.imported_generic_templates
            .by_declaration
            .insert(key, id);
        id
    }

    fn allocate_imported_template(
        &mut self,
        prepared: PreparedImportedGeneric,
    ) -> hir::ImportedGenericCallableTemplateId {
        let id = Idx::from_raw(RawIdx::from(
            u32::try_from(self.imported_generic_templates.templates.len())
                .expect("dependency template arena fits u32"),
        ));
        self.imported_generic_templates
            .templates
            .push(Some(prepared));
        id
    }
}
