use super::*;

impl Lowerer {
    pub(crate) fn lower_imported_dependency_property_assignment(
        &mut self,
        binding: &hir::DirectImportedTargetBinding,
        name: &ast::Ident,
        value: &ast::Expr,
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        let mut prepared = self.prepare_imported_property_setter(binding, None, name)?;
        let mut sink = Vec::new();
        prepared.receiver = prepared.receiver.map(|receiver| PropertyCallReceiver {
            value: self.materialize_place_expr(
                receiver.value,
                "property_receiver",
                name.span,
                &mut sink,
            ),
            static_type: receiver.static_type,
        });
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
        receiver: Option<PropertyCallReceiver>,
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
        receiver: Option<PropertyCallReceiver>,
        name: &ast::Ident,
    ) -> Option<PreparedImportedPropertySetter> {
        let property = self.imported_dependency_property_candidate(binding, name.span)?;
        let capability = property.interface().accessors();
        if capability.setter().is_none() {
            self.error(
                name.span,
                format!("cannot assign to immutable property `{}`", name.text),
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
        if !self.imported_callable_is_accessible(
            candidate.interface(),
            receiver.as_ref().map(|receiver| receiver.static_type),
        ) {
            self.error(
                name.span,
                format!("setter of property `{}` is not accessible", name.text),
            );
            return None;
        }
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
        let source_receiver = PropertyCallReceiver::source_type(&prepared.receiver);
        args.extend(prepared.receiver.map(|receiver| receiver.value));
        args.push(value);
        self.emit_imported_property_accessor(
            prepared.candidate,
            args,
            source_receiver,
            self.unit,
            span,
            "writing an unsafe dependency property",
        )
        .map(hir::StatementKind::Expr)
    }
}
