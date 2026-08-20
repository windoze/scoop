//! Constructor requests retain the provider's declaration and applied owner.

mod companions;
mod prepare;

use std::collections::BTreeMap;
use std::ops::Index;
use std::sync::Arc;

use hir::ImportedCallableSource;
use la_arena::{Arena, Idx, RawIdx};
use scoop_hir as hir;
use scoop_identity::{CallableTemplateOrigin, PersistentConstructorId};

use crate::Lowerer;
use crate::imported_core::ImportedTypeBindings;

#[derive(Clone, Default)]
pub(crate) struct ImportedConstructorTemplates {
    pub(crate) templates: Vec<PreparedImportedConstructor>,
    by_declaration: BTreeMap<PersistentConstructorId, hir::ImportedConstructorTemplateId>,
}

#[derive(Clone)]
pub(crate) struct PreparedImportedConstructor {
    pub(crate) signature: hir::ImportedConstructorSignature,
    pub(crate) source: hir::ImportedCallableDeclaration,
    pub(crate) initialization: Arc<hir::ExportGenericNominalInitializationV1>,
    pub(crate) constructor: usize,
    pub(crate) bindings: ImportedTypeBindings,
    kind: Option<hir::ConstructorKind>,
}

impl PreparedImportedConstructor {
    pub(crate) fn expression_origin(
        &self,
        definition: hir::DefinitionOrigin,
    ) -> hir::ExpressionOrigin {
        let mut evaluation: hir::EvaluationOrigin = definition.into();
        evaluation.context = self.signature.evaluation_context;
        hir::ExpressionOrigin::Instantiated(hir::ConcreteExpressionOrigin {
            definition,
            evaluation,
        })
    }
}

impl Index<hir::ImportedConstructorTemplateId> for ImportedConstructorTemplates {
    type Output = PreparedImportedConstructor;
    fn index(&self, id: hir::ImportedConstructorTemplateId) -> &Self::Output {
        &self.templates[id.into_raw().into_u32() as usize]
    }
}

impl ImportedConstructorTemplates {
    pub(crate) fn into_completed(self) -> Arena<hir::ImportedConstructorTemplate> {
        self.templates
            .into_iter()
            .map(|template| hir::ImportedConstructorTemplate {
                signature: template.signature,
                kind: template
                    .kind
                    .expect("successful lowering completed every requested constructor"),
            })
            .collect()
    }
}

pub(crate) fn source_constructor(
    reference: &hir::DefaultConstructorRefV1,
) -> Option<PersistentConstructorId> {
    match reference {
        hir::DefaultConstructorRefV1::Struct { declaration, .. }
        | hir::DefaultConstructorRefV1::Class {
            declaration: hir::DefaultClassConstructorIdV1::Source(declaration),
            ..
        } => Some(*declaration),
        hir::DefaultConstructorRefV1::Class {
            declaration: hir::DefaultClassConstructorIdV1::Generated(_),
            ..
        }
        | hir::DefaultConstructorRefV1::Variant { .. } => None,
    }
}

impl Lowerer {
    pub(crate) fn constructor_template_application(
        &mut self,
        template: hir::ImportedConstructorTemplateId,
        owner: hir::TypeId,
    ) -> hir::ConstructorApplicationRef {
        match self.types[owner] {
            hir::Type::Class(owner) => {
                hir::ConstructorApplicationRef::Class(self.class_constructor_application(
                    hir::ClassConstructorDefinition::Template(template),
                    owner,
                ))
            }
            hir::Type::Struct(owner) => {
                hir::ConstructorApplicationRef::Struct(self.struct_constructor_application(
                    hir::StructConstructorDefinition::Template(template),
                    owner,
                ))
            }
            _ => unreachable!("a selected constructor has a complete class or struct owner"),
        }
    }

    pub(crate) fn request_imported_constructor_template(
        &mut self,
        source: hir::ImportedCallableDeclaration,
    ) -> Result<hir::ImportedConstructorTemplateId, String> {
        let CallableTemplateOrigin::Constructor(declaration) = source.interface().declaration()
        else {
            return Err("dependency declaration is not a source constructor".into());
        };
        if let Some(id) = self
            .imported_constructor_templates
            .by_declaration
            .get(&declaration)
        {
            return Ok(*id);
        }
        let prepared = self.prepare_imported_constructor(source, declaration)?;
        let id = Idx::from_raw(RawIdx::from(
            u32::try_from(self.imported_constructor_templates.templates.len())
                .expect("constructor template arena fits u32"),
        ));
        self.imported_constructor_templates.templates.push(prepared);
        self.imported_constructor_templates
            .by_declaration
            .insert(declaration, id);
        Ok(id)
    }

    pub(crate) fn complete_imported_constructor(&mut self, index: usize) {
        let template = self.imported_constructor_templates.templates[index].clone();
        let id = Idx::from_raw(RawIdx::from(index as u32));
        let result = self
            .materialize_imported_constructor(&template)
            .map_err(|error| error.to_string())
            .and_then(|kind| {
                self.complete_companion_dependencies(id, &template)?;
                Ok(kind)
            });
        match result {
            Ok(kind) => self.imported_constructor_templates.templates[index].kind = Some(kind),
            Err(error) => {
                self.current_file = template.signature.origin.file as usize;
                self.error(
                    template.signature.origin.span,
                    format!(
                        "cannot instantiate dependency constructor `{}`: {error}",
                        template.signature.name
                    ),
                );
            }
        }
    }
}
