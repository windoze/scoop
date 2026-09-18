use super::*;

impl Lowerer {
    pub(crate) fn lower_extension_property_write(
        &mut self,
        resolved: ResolvedExtensionPropertyWrite,
        value: hir::Expr,
        span: ast::Span,
    ) -> Option<hir::StatementKind> {
        let ResolvedExtensionPropertyWrite {
            target,
            receiver,
            value_type: _,
            has_setter: _,
        } = resolved;
        match target {
            ResolvedExtensionPropertyTarget::Current {
                property,
                type_args,
            } => self.lower_current_extension_property_write(
                property, receiver, &type_args, value, span,
            ),
            ResolvedExtensionPropertyTarget::Dependency { binding, name } => self
                .lower_imported_dependency_property_write(
                    &binding,
                    Some(receiver),
                    value,
                    &name,
                    span,
                ),
        }
    }

    fn lower_current_extension_property_write(
        &mut self,
        property: hir::PropertyId,
        receiver: hir::Expr,
        type_args: &[TypeId],
        value: hir::Expr,
        span: ast::Span,
    ) -> Option<hir::StatementKind> {
        let declaration = self.properties[property].clone();
        self.record_property_initialization_dependency(&declaration, span);
        let Some(setter) = declaration.capability.setter() else {
            self.error(
                span,
                format!("cannot assign to immutable property `{}`", declaration.name),
            );
            return None;
        };
        let setter = self.property_setters[setter].clone();
        if !self.access_domain_allows(&setter.access.lookup.0, Some(receiver.ty)) {
            self.error(
                span,
                format!(
                    "setter of property `{}` is not accessible",
                    declaration.name
                ),
            );
            return None;
        }
        let function = match setter.implementation {
            hir::PropertyAccessorImplementation::Body(function) => function,
            hir::PropertyAccessorImplementation::Storage
            | hir::PropertyAccessorImplementation::Constant
            | hir::PropertyAccessorImplementation::AbstractSlot(_) => {
                unreachable!("extension properties use concrete accessor functions")
            }
        };
        let candidate = crate::CallableCandidate::function(
            function,
            Vec::new(),
            self.function_lookup_witness(function),
        );
        let callee = self.materialize_candidate_callable(&candidate, type_args);
        self.check_call_effects(callee, span);
        Some(hir::StatementKind::Expr(hir::Expr {
            kind: hir::ExprKind::Call {
                callee,
                args: vec![receiver, value],
            },
            ty: self.unit,
            span,
            origin: self.expression_origin(span),
        }))
    }
}
