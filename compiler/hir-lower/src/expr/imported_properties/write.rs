use super::*;

impl Lowerer {
    pub(crate) fn lower_imported_dependency_property_assignment(
        &mut self,
        binding: &hir::DirectImportedTargetBinding,
        name: &ast::Ident,
        value: &ast::Expr,
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        let prepared = self.prepare_imported_property_setter(binding, None, name)?;
        let mut sink = Vec::new();
        let value = self.lower_expr(value, &mut sink, Some(prepared.value_type))?;
        if !self.is_subtype(value.ty, prepared.value_type) {
            let message = self.with_nominal_invariance_detail(
                format!(
                    "cannot assign value of type {} to property `{}` of type {}",
                    self.type_name(value.ty),
                    name.text,
                    self.type_name(prepared.value_type)
                ),
                value.ty,
                prepared.value_type,
            );
            self.error(value.span, message);
            return None;
        }
        let value = self.adapt_to(value, prepared.value_type);
        let statement = self.emit_imported_property_setter(prepared, value, name.span)?;
        out.extend(sink);
        Some(statement)
    }

    pub(crate) fn lower_imported_dependency_property_write(
        &mut self,
        binding: &hir::DirectImportedTargetBinding,
        receiver: Option<hir::Expr>,
        value: hir::Expr,
        name: &ast::Ident,
        span: ast::Span,
    ) -> Option<hir::StatementKind> {
        let prepared = self.prepare_imported_property_setter(binding, receiver, name)?;
        if !self.is_subtype(value.ty, prepared.value_type) {
            self.error(
                value.span,
                format!(
                    "cannot assign value of type {} to property `{}` of type {}",
                    self.type_name(value.ty),
                    name.text,
                    self.type_name(prepared.value_type)
                ),
            );
            return None;
        }
        let value = self.adapt_to(value, prepared.value_type);
        self.emit_imported_property_setter(prepared, value, span)
    }

    fn prepare_imported_property_setter(
        &mut self,
        binding: &hir::DirectImportedTargetBinding,
        receiver: Option<hir::Expr>,
        name: &ast::Ident,
    ) -> Option<PreparedImportedPropertySetter> {
        let property = self.imported_dependency_property_candidate(binding, name.span)?;
        let capability = property.interface().capability();
        if capability.setter().is_none() {
            self.error(
                name.span,
                format!("cannot assign to immutable property `{}`", name.text),
            );
            return None;
        }
        if capability.setter_access() != Some(hir::PropertySetterPublicAccessV1::Public) {
            self.error(
                name.span,
                format!("setter of property `{}` is not accessible", name.text),
            );
            return None;
        }
        let candidate = self.imported_dependency_property_accessor(
            &property,
            hir::ImportedDependencyPropertyAccessorKind::Setter,
            "dependency property setter",
            name.span,
        )?;
        let receiver = self.validate_imported_property_receiver(&property, receiver, name.span)?;
        let value_type = self.imported_property_value_type(&property, name.span)?;
        Some(PreparedImportedPropertySetter {
            candidate,
            receiver,
            value_type,
        })
    }

    fn emit_imported_property_setter(
        &mut self,
        prepared: PreparedImportedPropertySetter,
        value: hir::Expr,
        span: ast::Span,
    ) -> Option<hir::StatementKind> {
        let mut args = Vec::with_capacity(1 + usize::from(prepared.receiver.is_some()));
        args.extend(prepared.receiver);
        args.push(value);
        self.emit_imported_property_accessor(
            prepared.candidate,
            args,
            self.unit,
            span,
            "writing an unsafe dependency property",
        )
        .map(hir::StatementKind::Expr)
    }
}
