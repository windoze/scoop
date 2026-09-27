//! Dependency bodies share the current request's type arena, but retain their
//! provider declaration ids. Pending statements are lowering scratch state;
//! the completed Export HIR owns ordinary, structurally complete bodies.

mod prepare;

use std::collections::BTreeMap;
use std::ops::{Deref, Index};

use hir::ImportedCallableSource;
use la_arena::{Arena, Idx, RawIdx};
use scoop_hir as hir;
use scoop_identity::PersistentGenericFunctionId;

use crate::Lowerer;
use crate::imported_core::ImportedTypeBindings;

#[derive(Clone, Default)]
pub(crate) struct ImportedGenericTemplates {
    templates: Vec<PreparedImportedGeneric>,
    by_declaration: BTreeMap<PersistentGenericFunctionId, hir::ImportedGenericCallableTemplateId>,
}

#[derive(Clone)]
pub(crate) struct PreparedImportedGeneric {
    pub(crate) signature: hir::ImportedGenericCallableSignature,
    pub(crate) declaration: hir::ImportedCallableDeclaration,
    pub(crate) bindings: ImportedTypeBindings,
    pub(crate) locals: Arena<hir::Local>,
    statements: Option<Vec<hir::Statement>>,
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
        &self.templates[u32::from(id.into_raw()) as usize]
    }
}

impl ImportedGenericTemplates {
    pub(crate) fn into_completed(self) -> Arena<hir::ImportedGenericCallableTemplate> {
        self.templates
            .into_iter()
            .map(|template| hir::ImportedGenericCallableTemplate {
                signature: template.signature,
                body: hir::Body {
                    locals: template.locals,
                    statements: template
                        .statements
                        .expect("successful lowering completed every queued dependency body"),
                },
            })
            .collect()
    }
}

impl Lowerer {
    pub(crate) fn request_imported_generic_template(
        &mut self,
        declaration: hir::ImportedCallableDeclaration,
    ) -> Result<hir::ImportedGenericCallableTemplateId, String> {
        let scoop_identity::CallableTemplateOrigin::GenericFunction(origin) =
            declaration.interface().declaration()
        else {
            return Err("dependency callable does not name a generic function".into());
        };
        if let Some(id) = self.imported_generic_templates.by_declaration.get(&origin) {
            return Ok(*id);
        }
        let prepared = self.prepare_imported_generic(declaration, origin)?;
        let id = Idx::from_raw(RawIdx::from(
            u32::try_from(self.imported_generic_templates.templates.len())
                .expect("dependency template arena fits u32"),
        ));
        self.imported_generic_templates.templates.push(prepared);
        self.imported_generic_templates
            .by_declaration
            .insert(origin, id);
        Ok(id)
    }

    pub(crate) fn complete_imported_generic_bodies(&mut self) {
        let mut index = 0;
        while index < self.imported_generic_templates.templates.len() {
            let template = self.imported_generic_templates.templates[index].clone();
            match self.materialize_imported_callable_body(&template) {
                Ok(body) => {
                    let target = &mut self.imported_generic_templates.templates[index];
                    target.locals = body.locals;
                    target.statements = Some(body.statements);
                }
                Err(error) => {
                    self.current_file = template.origin.file as usize;
                    self.error(
                        template.span,
                        format!(
                            "cannot instantiate dependency body `{}`: {error}",
                            template.name
                        ),
                    );
                }
            }
            index += 1;
        }
    }
}
