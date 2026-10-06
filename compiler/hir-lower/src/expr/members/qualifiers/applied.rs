use super::*;

impl Lowerer {
    pub(crate) fn resolve_applied_qualifier(
        &mut self,
        expression: &ast::Expr,
    ) -> Result<Option<(TypeId, NominalTarget)>, ()> {
        if let Some(reference) = expression.applied_qualifier_type() {
            self.check_applied_qualifier_shadowing(&reference)?;
            let ty = self.resolve_type_ref(&reference).ok_or(())?;
            return Ok(self.nominal_target_for_type(ty).map(|target| (ty, target)));
        }
        if let ast::Expr::FieldAccess(access) = expression
            && access.navigation == ast::Navigation::Direct
            && let ast::FieldSelector::Name(name) = &access.selector
            && let Some((host, target)) = self.resolve_applied_qualifier(&access.receiver)?
            && self
                .nested_nominal_target(target.owner(), &name.text)
                .is_some()
        {
            let ty = self
                .resolve_applied_member_type(host, name, &[], access.span)
                .ok_or(())?;
            return Ok(self.nominal_target_for_type(ty).map(|target| (ty, target)));
        }
        self.resolve_direct_type_alias_qualifier(expression)
    }

    pub(crate) fn lower_qualified_singleton(
        &mut self,
        object: hir::ObjectId,
        qualifier: Option<TypeId>,
        span: ast::Span,
    ) -> Option<hir::Expr> {
        let arguments = qualifier
            .and_then(|ty| self.nominal_application(ty))
            .map_or_else(Vec::new, |application| application.arguments);
        self.lower_singleton_application(object, arguments, span)
    }

    pub(in crate::expr) fn check_applied_qualifier_shadowing(
        &mut self,
        reference: &ast::TypeRef,
    ) -> Result<(), ()> {
        let mut root = reference;
        while let ast::TypeRefKind::AppliedMember { owner, .. } = &root.kind {
            root = owner;
        }
        let name = match &root.kind {
            ast::TypeRefKind::Named(name) | ast::TypeRefKind::Generic(name, _) => name,
            ast::TypeRefKind::Qualified { path, .. } => &path[0],
            _ => return Ok(()),
        };
        if self.lexical_or_member_value_blocks_type_qualifier(&name.text)
            || matches!(
                self.lookup_expression_qualifier(name),
                ExpressionQualifierLookup::Value
            )
        {
            self.error(
                name.span,
                format!("value `{}` cannot qualify a type application", name.text),
            );
            return Err(());
        }
        Ok(())
    }

    pub(crate) fn complete_companion_qualifier(
        &mut self,
        expression: &ast::Expr,
    ) -> Result<Option<(NominalTarget, Option<TypeId>)>, ()> {
        let applied = self.resolve_applied_qualifier(expression)?;
        let Some(target) = applied
            .map(|(_, target)| target)
            .or_else(|| self.nominal_qualifier_target(expression))
        else {
            return Ok(None);
        };
        let object = match target {
            NominalTarget::Object(object) => Some(object),
            _ => self.companion_object(target.owner()),
        };
        if applied.is_none()
            && object.is_some_and(|object| {
                !self.classes[self.objects[object].backing_class]
                    .type_params
                    .is_empty()
            })
        {
            self.error(
                expression.span(),
                "generic companion access requires complete host type arguments".into(),
            );
            return Err(());
        }
        Ok(Some((target, applied.map(|(ty, _)| ty))))
    }

    pub(crate) fn lower_qualified_property_receiver(
        &mut self,
        expression: &ast::Expr,
        name: &str,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        if let Some((target, applied)) = self.complete_companion_qualifier(expression).ok()? {
            if let Some(object) = self.companion_forwarding_property_object(target, name) {
                return self.lower_qualified_singleton(object, applied, expression.span());
            }
            if let NominalTarget::Object(object) = target {
                return self.lower_qualified_singleton(object, applied, expression.span());
            }
        }
        if let Some(owner) = self.resolve_imported_nominal_qualifier(expression).ok()?
            && let Some(ty) = self
                .imported_generic_companion_type(owner, expression.span())
                .ok()?
        {
            return self.lower_imported_singleton_type(ty, expression.span());
        }
        self.lower_expr(expression, sink, None)
    }
}
