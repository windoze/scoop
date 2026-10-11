//! Logical-property queries own initializer and accessor types.
use super::*;
use crate::properties::{PropertyAccessorBodyKind, PropertyAccessorKind};

mod accessors;
mod initializers;
mod signatures;

#[derive(Clone)]
pub(super) struct InferredProperty {
    source: Rc<ast::PropertyDecl>,
    file: usize,
    pub(super) state: InferenceState,
}

impl Lowerer {
    pub(crate) fn resolve_property_annotation(
        &mut self,
        source: &ast::PropertyDecl,
    ) -> Option<TypeId> {
        if let Some(ty) = &source.ty {
            return self.resolve_type_ref(ty);
        }
        let has_source = match &source.body {
            ast::PropertyBodySyntax::Initializer { .. }
            | ast::PropertyBodySyntax::Const(_)
            | ast::PropertyBodySyntax::Delegated { .. } => true,
            ast::PropertyBodySyntax::Computed(accessors) => accessors
                .getter
                .as_ref()
                .is_some_and(|getter| matches!(getter.body, ast::AccessorBodySyntax::Expr(_))),
            _ => false,
        };
        if !has_source {
            self.error(source.name.span, format!("property `{}` requires an explicit type without an initializer, expression getter, or delegate", source.name.text));
            return None;
        }
        // This value is confined to declaration allocation. Every semantic
        // read completes the typed query before using the property's type.
        Some(self.unit)
    }

    pub(crate) fn register_inferred_property(
        &mut self,
        property: hir::PropertyId,
        source: &ast::PropertyDecl,
    ) {
        if source.ty.is_none() {
            self.declaration_inference.properties.insert(
                property,
                InferredProperty {
                    source: Rc::new(source.clone()),
                    file: self.current_file,
                    state: InferenceState::Pending,
                },
            );
        }
    }

    pub(crate) fn property_type_pending(&self, property: hir::PropertyId) -> bool {
        self.declaration_inference
            .properties
            .get(&property)
            .is_some_and(|query| !matches!(query.state, InferenceState::Complete))
    }

    pub(crate) fn complete_inferred_properties(&mut self) {
        let mut properties = self
            .declaration_inference
            .properties
            .keys()
            .copied()
            .collect::<Vec<_>>();
        properties.sort_by_key(|id| id.into_raw().into_u32());
        for property in properties {
            self.infer_property_type(property);
        }
    }

    pub(crate) fn infer_property_type(&mut self, property: hir::PropertyId) -> Option<TypeId> {
        let Some(query) = self
            .declaration_inference
            .properties
            .get(&property)
            .cloned()
        else {
            return Some(self.properties[property].ty);
        };
        if !self.declaration_inference.ready {
            return Some(self.properties[property].ty);
        }
        match query.state {
            InferenceState::Complete => return Some(self.properties[property].ty),
            InferenceState::Failed => return None,
            InferenceState::Active => {
                let names = self.declaration_inference_path(DeclarationKey::Property(property));
                let file = std::mem::replace(&mut self.current_file, query.file);
                self.error(
                    query.source.name.span,
                    format!("recursive declaration type inference: {names}; add an explicit type"),
                );
                self.current_file = file;
                return None;
            }
            InferenceState::Pending => {}
        }
        self.declaration_inference
            .properties
            .get_mut(&property)
            .expect("registered query")
            .state = InferenceState::Active;
        self.declaration_inference
            .active
            .push(DeclarationKey::Property(property));
        let errors = self.diagnostics.len();
        let result =
            self.with_declaration_inference_scope(query.file, |this| match &query.source.body {
                ast::PropertyBodySyntax::Computed(_) => this.infer_property_getter(property),
                ast::PropertyBodySyntax::Initializer { expression, .. } => {
                    this.infer_property_initializer(property, &query.source, expression)
                }
                ast::PropertyBodySyntax::Delegated { expression, .. } => {
                    this.infer_property_initializer(property, &query.source, expression)?;
                    this.infer_property_getter(property)
                }
                _ => unreachable!("only independent property type sources are registered"),
            });
        let valid = result.is_some() && self.diagnostics.len() == errors;
        if let Some(ty) = result {
            self.set_inferred_property_type(property, ty);
        }
        assert_eq!(
            self.declaration_inference.active.pop(),
            Some(DeclarationKey::Property(property))
        );
        self.declaration_inference
            .properties
            .get_mut(&property)
            .expect("registered query")
            .state = if valid {
            InferenceState::Complete
        } else {
            InferenceState::Failed
        };
        valid.then_some(self.properties[property].ty)
    }

    fn property_accessor_source(
        &self,
        property: hir::PropertyId,
        kind: PropertyAccessorKind,
    ) -> crate::properties::PropertyAccessorSource {
        self.property_accessor_sources
            .iter()
            .find(|source| source.property == property && source.kind == kind)
            .expect("a queried accessor has source context")
            .clone()
    }
}
