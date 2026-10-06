//! Binding and constant normalization for ordinary source annotations.

use crate::{
    Lowerer, Owner, namespace::TopLevelTypeTarget, persistent_nominals::NominalIdentityInput,
};
mod binding;
mod serialization;
mod targets;
mod values;

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::{
    CborIdentityRecord, PersistentAnnotationId, SourceDeclarationKey, SourceNominalKind,
};

#[derive(Clone)]
pub(crate) struct SourceAnnotationInput {
    pub(crate) identity: CborIdentityRecord<PersistentAnnotationId, SourceDeclarationKey>,
    pub(crate) declaration: ast::AnnotationClassDecl,
    pub(crate) file: usize,
    pub(crate) owner: Option<Owner>,
    pub(crate) access: hir::NominalAccess,
}

impl Lowerer {
    pub(crate) fn declare_annotation_class(
        &mut self,
        declaration: &ast::AnnotationClassDecl,
        file: usize,
        owner: Option<Owner>,
    ) {
        if let Some(kind) = self.type_namespace_conflict(
            owner,
            &declaration.name.text,
            file,
            crate::namespace::is_file_private(declaration.visibility),
        ) {
            self.error(
                declaration.name.span,
                format!(
                    "duplicate type `{}` (already declared as {kind})",
                    declaration.name.text
                ),
            );
            return;
        }
        let access = match owner {
            Some(owner) => self.nested_nominal_access(
                declaration.visibility,
                declaration.name.span,
                "annotation class",
                owner,
                file,
            ),
            None => self.nominal_access(
                declaration.visibility,
                declaration.name.span,
                "annotation class",
                file,
            ),
        };
        let key = match self.source_nominal_declaration_key(&NominalIdentityInput {
            name: &declaration.name.text,
            parent: owner,
            access: access.declared,
            type_parameter_count: 0,
            kind: SourceNominalKind::AnnotationClass,
            file,
            span: declaration.span,
        }) {
            Ok(key) => key,
            Err(error) => {
                self.error(declaration.span, error.to_string());
                return;
            }
        };
        let identity = CborIdentityRecord::<PersistentAnnotationId, _>::from_key(key)
            .expect("a non-generic annotation source key has an annotation identity");
        let annotation = identity.id();
        if let Some(owner) = owner {
            self.nested_annotations_by_owner
                .insert((owner, declaration.name.text.clone()), annotation);
        } else {
            self.top_level_namespaces.register_type(
                file,
                declaration.name.text.clone(),
                TopLevelTypeTarget::Annotation(annotation),
                access.declared == hir::DeclaredVisibility::Private,
            );
        }
        self.source_annotations.insert(
            annotation,
            SourceAnnotationInput {
                identity,
                declaration: declaration.clone(),
                file,
                owner,
                access,
            },
        );
    }
}

impl Lowerer {
    pub(crate) fn lexical_annotation_named(&self, name: &str) -> Option<PersistentAnnotationId> {
        let mut owner = self.current_owner?;
        loop {
            if let Some(id) = self
                .nested_annotations_by_owner
                .get(&(owner, name.to_owned()))
            {
                return Some(*id);
            }
            if self.nested_nominal_target(owner, name).is_some() {
                return None;
            }
            owner = self.nominal_parent(owner)?;
        }
    }

    pub(crate) fn annotation_is_accessible(&self, id: PersistentAnnotationId) -> bool {
        if let Some(declaration) = self.source_annotations.get(&id) {
            return self.access_domain_allows(&declaration.access.lookup.0);
        }
        self.dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.annotation_declaration(id))
            .is_some_and(|declaration| {
                self.access_domain_allows(&self.imported_type_name_access_domain(
                    &declaration.source,
                    declaration.declaration.visibility,
                ))
            })
    }
}
