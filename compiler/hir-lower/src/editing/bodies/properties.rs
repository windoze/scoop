use super::*;
use hir::{EditingPropertyAccess as Access, SemanticProperty as Target};

impl Lowerer {
    pub(crate) fn observe_property_syntax(&mut self, expression: &ast::Expr) {
        if !self.observes_current_body() {
            return;
        }
        let name = match expression {
            ast::Expr::Var(name)
            | ast::Expr::QualifiedInterfaceSuperAccess { name, .. }
            | ast::Expr::MethodCall { name, .. } => name,
            ast::Expr::Call(call) => &call.callee,
            ast::Expr::FieldAccess(access) => match &access.selector {
                ast::FieldSelector::Name(name) => name,
                _ => return,
            },
            ast::Expr::Update { place, span, .. } => {
                self.observe_property_place(place, *span);
                return;
            }
            _ => return,
        };
        self.observe_property_name(expression.span(), name.span);
    }

    pub(crate) fn observe_property_place(&mut self, place: &ast::PlaceExpr, span: ast::Span) {
        if !self.observes_current_body() {
            return;
        }
        let (place_span, name) = match place {
            ast::PlaceExpr::Name(name) => (name.span, name),
            ast::PlaceExpr::Field { name, span, .. }
            | ast::PlaceExpr::QualifiedInterfaceSuperProperty { name, span, .. } => (*span, name),
            ast::PlaceExpr::Index { .. } => return,
        };
        self.observe_property_name(place_span, name.span);
        self.observe_property_name(span, name.span);
    }

    fn observe_property_name(&mut self, span: ast::Span, name: ast::Span) {
        let names = &mut self
            .editing_body
            .as_mut()
            .expect("active query")
            .property_names;
        names.insert(span, name);
        names.insert(name, name);
    }

    pub(crate) fn observe_property_reference(
        &mut self,
        span: ast::Span,
        target: Target,
        value_type: hir::TypeId,
        mut access: Access,
    ) {
        if !self.observes_current_body() {
            return;
        }
        let body = self.editing_body.as_ref().expect("active query");
        let Some(span) = body.property_names.get(&span).copied() else {
            return;
        };
        if let Some(previous) = body.properties.get(&span)
            && previous.target == target
            && previous.access != access
        {
            access = Access::ReadWrite;
        }
        let (name, mutable, definition) = match target {
            Target::Current(property) => {
                let declaration = &self.properties[property];
                let definition = self.property_name_spans.get(&property).and_then(|span| {
                    self.property_definition_file(property).map(|file| {
                        hir::SemanticSourceLocation {
                            source: self.source_identity(file).clone(),
                            span: *span,
                        }
                    })
                });
                (
                    declaration.name.clone(),
                    declaration.capability.setter().is_some(),
                    definition,
                )
            }
            Target::Imported(property) => {
                let dependencies = self
                    .dependencies
                    .as_ref()
                    .expect("an imported property has its declarations");
                let declaration = dependencies
                    .property_declaration(property)
                    .expect("a selected property has its declaration");
                let name = dependencies
                    .property_name(property)
                    .expect("a selected property has its name")
                    .to_owned();
                let definition = dependencies
                    .property_definition_origin(property)
                    .and_then(|origin| crate::semantic_queries::imported_location(origin).ok());
                (name, declaration.accessors().setter().is_some(), definition)
            }
        };
        let context = self.observe_body_context();
        self.editing_body
            .as_mut()
            .expect("active query")
            .properties
            .insert(
                span,
                hir::EditingPropertyReference {
                    span,
                    context,
                    target,
                    name,
                    mutable,
                    value_type,
                    definition,
                    access,
                },
            );
    }
}
