use super::*;

impl Lowerer {
    pub(crate) fn lower_imported_member_property_write(
        &mut self,
        property: ResolvedImportedMemberProperty,
        receiver: hir::Expr,
        value: hir::Expr,
        name: &ast::Ident,
        span: ast::Span,
    ) -> Option<hir::StatementKind> {
        let Some(setter) = property.capability.setter() else {
            self.error(
                name.span,
                format!("cannot assign to immutable property `{}`", name.text),
            );
            return None;
        };
        if property.capability.setter_access() != Some(hir::PropertySetterPublicAccessV1::Public) {
            self.error(
                name.span,
                format!("setter of property `{}` is not accessible", name.text),
            );
            return None;
        }
        if !self.is_subtype(value.ty, property.value_type) {
            let message = self.with_nominal_invariance_detail(
                format!(
                    "cannot assign value of type {} to property `{}` of type {}",
                    self.type_name(value.ty),
                    name.text,
                    self.type_name(property.value_type),
                ),
                value.ty,
                property.value_type,
            );
            self.error(value.span, message);
            return None;
        }
        let candidate = self
            .dependencies
            .as_ref()
            .expect("an imported property carries its dependency declarations")
            .callable_declaration(scoop_identity::CallableTemplateOrigin::Accessor(setter))
            .map_err(|error| {
                self.error(
                    name.span,
                    format!("invalid dependency property setter: {error}"),
                );
            })
            .ok()?;
        let value = self.adapt_to(value, property.value_type);
        self.emit_imported_member_accessor(
            candidate,
            receiver,
            vec![value],
            self.unit,
            span,
            "writing an unsafe dependency property",
        )
        .map(hir::StatementKind::Expr)
    }
}
