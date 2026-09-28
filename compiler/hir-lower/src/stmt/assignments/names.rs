use super::*;

impl Lowerer {
    /// `name = value`: the target must be a declared, mutable local and
    /// the value type must match the local's type. Inside a class
    /// or interface method a bare name may also denote a member property
    /// (`y = v` meaning `this.y = v`): `var` properties store through
    /// `this`, `val` properties are immutable (diagnostic).
    /// Value-type fields stay unwritable as before.
    pub(super) fn lower_local_assign(
        &mut self,
        assign: &ast::Assign,
        name: &ast::Ident,
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        if name.text == "field" && self.backing_field_context.is_some() {
            let (read, write) = self.contextual_backing_field(name.span)?;
            let Some(target) = write else {
                self.error(
                    name.span,
                    "cannot assign to an immutable backing field".to_string(),
                );
                return None;
            };
            let mut sink = Vec::new();
            let value = self.lower_expr(&assign.value, &mut sink, Some(read.ty))?;
            if !self.is_subtype(value.ty, read.ty) {
                self.error(
                    assign.value.span(),
                    format!(
                        "cannot assign value of type {} to backing field of type {}",
                        self.type_name(value.ty),
                        self.type_name(read.ty)
                    ),
                );
                return None;
            }
            let value = self.adapt_to(value, read.ty);
            out.extend(sink);
            return Some(hir::StatementKind::Assign { target, value });
        }
        let Some(local) = self.scopes.lookup(&name.text) else {
            if self.constructor_params_in_scope.contains_key(&name.text) {
                self.error(
                    name.span,
                    format!(
                        "cannot assign to immutable constructor parameter `{}`",
                        name.text
                    ),
                );
                return None;
            }
            if self.initialization_context.is_some()
                && self.initializing_receiver_has_field(&name.text)
            {
                return self.lower_initializing_field_assign(assign, name, out);
            }
            if let Some(capture) = self.available_capture(&name.text) {
                if let Some(plan) = self.local_delegate_plans.get(&capture.binding).cloned() {
                    if !plan.mutable {
                        self.error(
                            name.span,
                            format!("cannot assign to immutable property `{}`", name.text),
                        );
                        return None;
                    }
                    let storage = self.lower_capture(name)?;
                    return self.lower_local_delegate_assign(
                        assign,
                        name,
                        storage,
                        capture.binding,
                        plan.property_ty,
                        out,
                    );
                }
                if capture.mutable {
                    self.error(
                        name.span,
                        format!(
                            "cannot capture mutable local `{}`; bind its current value to a `val` snapshot or capture explicit reference state",
                            name.text
                        ),
                    );
                } else {
                    self.error(
                        name.span,
                        format!("cannot assign to immutable variable `{}`", name.text),
                    );
                }
                return None;
            }
            match self.current_this_ty().map(|ty| self.types[ty].clone()) {
                Some(Type::Class(_)) | Some(Type::Interface(_)) => {
                    let receiver_ty = self.current_this_ty().expect("member receiver type");
                    if self
                        .find_accessible_nominal_property(receiver_ty, &name.text)
                        .is_some()
                    {
                        let receiver = self
                            .lower_current_this(name.span)
                            .expect("a receiver callable body always has a lexical `this`");
                        let mut sink = Vec::new();
                        let kind = self.assign_class_field(assign, receiver, name, &mut sink)?;
                        out.extend(sink);
                        return Some(kind);
                    }
                    if let Some(property) = self
                        .resolve_imported_member_property(receiver_ty, name)
                        .ok()?
                    {
                        let receiver = self.lower_current_this(name.span)?;
                        let mut sink = Vec::new();
                        let kind = self.assign_imported_member_property(
                            assign, property, receiver, name, &mut sink,
                        )?;
                        out.extend(sink);
                        return Some(kind);
                    }
                }
                Some(Type::Struct(_)) | Some(Type::Enum(_)) => {
                    let receiver_ty = self.current_this_ty().expect("member receiver type");
                    if self
                        .find_accessible_nominal_property(receiver_ty, &name.text)
                        .is_some()
                    {
                        self.error(
                            name.span,
                            format!("cannot assign to immutable property `{}`", name.text),
                        );
                        return None;
                    }
                }
                _ => {}
            }
            let mut selected_value = None;
            if self.initialization_context.is_none()
                && let Some(receiver) = self.lower_current_this(name.span)
            {
                let mut sink = Vec::new();
                match self.resolve_implicit_value_write(receiver, name, &mut sink) {
                    crate::properties::ImplicitValueResolution::ExtensionProperty(property) => {
                        let expected = property.value_type();
                        let value = self.lower_expr(&assign.value, &mut sink, Some(expected))?;
                        if !self.is_subtype(value.ty, expected) {
                            let message = self.with_nominal_invariance_detail(
                                format!(
                                    "cannot assign value of type {} to property `{}` of type {}",
                                    self.type_name(value.ty),
                                    name.text,
                                    self.type_name(expected)
                                ),
                                value.ty,
                                expected,
                            );
                            self.error(assign.value.span(), message);
                            return None;
                        }
                        let value = self.adapt_to(value, expected);
                        out.extend(sink);
                        return self.lower_extension_property_write(*property, value, name.span);
                    }
                    crate::properties::ImplicitValueResolution::Value { target, .. } => {
                        selected_value = Some(target);
                    }
                    crate::properties::ImplicitValueResolution::NoApplicable(failure) => {
                        self.commit_layer_diagnostics(*failure);
                        return None;
                    }
                    crate::properties::ImplicitValueResolution::Failed => return None,
                    crate::properties::ImplicitValueResolution::NoCandidate => {}
                }
            }
            let target = match selected_value {
                Some(value) => Some(value),
                None => self.resolve_value_name(name).ok()?,
            };
            if let Some(target) = target {
                let target = match target {
                    crate::imports::lookup::values::ResolvedValueTarget::Dependency(binding) => {
                        return self.lower_imported_dependency_property_assignment(
                            &binding,
                            name,
                            &assign.value,
                            out,
                        );
                    }
                    crate::imports::lookup::values::ResolvedValueTarget::Materialized(target) => {
                        target
                    }
                };
                let crate::imports::lookup::values::ValueTarget::Property(property) = target else {
                    self.error(
                        name.span,
                        format!("cannot assign to immutable value `{}`", name.text),
                    );
                    return None;
                };
                let (owner, receiver) = self.named_property_receiver(property, name.span)?.parts();
                let expected = self.properties[property].ty;
                let mut sink = Vec::new();
                let receiver = receiver.map(|receiver| {
                    self.materialize_place_expr(receiver, "property_receiver", name.span, &mut sink)
                });
                let value = self.lower_expr(&assign.value, &mut sink, Some(expected))?;
                if !self.is_subtype(value.ty, expected) {
                    let message = self.with_nominal_invariance_detail(
                        format!(
                            "cannot assign value of type {} to property `{}` of type {}",
                            self.type_name(value.ty),
                            name.text,
                            self.type_name(expected)
                        ),
                        value.ty,
                        expected,
                    );
                    self.error(assign.value.span(), message);
                    return None;
                }
                let value = self.adapt_to(value, expected);
                let write =
                    self.lower_property_write(property, owner, receiver, value, name.span)?;
                out.extend(sink);
                return Some(write);
            }
            self.error(name.span, format!("unknown variable `{}`", name.text));
            return None;
        };
        let binding = self.locals[local].binding;
        if let Some(plan) = self.local_delegate_plans.get(&binding).cloned() {
            if !plan.mutable {
                self.error(
                    name.span,
                    format!("cannot assign to immutable property `{}`", name.text),
                );
                return None;
            }
            let storage = hir::Expr {
                kind: hir::ExprKind::Local(local),
                ty: self.locals[local].ty,
                span: name.span,
                origin: self.expression_origin(name.span),
            };
            return self.lower_local_delegate_assign(
                assign,
                name,
                storage,
                binding,
                plan.property_ty,
                out,
            );
        }
        if !self.locals[local].mutable {
            self.error(
                name.span,
                format!("cannot assign to immutable variable `{}`", name.text),
            );
            return None;
        }
        let expected = self.locals[local].ty;
        let mut sink = Vec::new();
        let value = self.lower_expr(&assign.value, &mut sink, Some(expected))?;
        if !self.is_subtype(value.ty, expected) {
            let expected_name = self.type_name(expected);
            let found = self.type_name(value.ty);
            let message = self.with_nominal_invariance_detail(
                format!(
                    "cannot assign value of type {found} to `{}` of type {expected_name}",
                    name.text
                ),
                value.ty,
                expected,
            );
            self.error(assign.value.span(), message);
            return None;
        }
        let value = self.adapt_to(value, expected);
        out.extend(sink);
        Some(hir::StatementKind::Assign {
            target: hir::AssignTarget::Local(local),
            value,
        })
    }

    fn lower_local_delegate_assign(
        &mut self,
        assign: &ast::Assign,
        name: &ast::Ident,
        storage: hir::Expr,
        binding: hir::BindingId,
        expected: TypeId,
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        let mut sink = Vec::new();
        let value = self.lower_expr(&assign.value, &mut sink, Some(expected))?;
        if !self.is_subtype(value.ty, expected) {
            let message = self.with_nominal_invariance_detail(
                format!(
                    "cannot assign value of type {} to property `{}` of type {}",
                    self.type_name(value.ty),
                    name.text,
                    self.type_name(expected)
                ),
                value.ty,
                expected,
            );
            self.error(assign.value.span(), message);
            return None;
        }
        let value = self.adapt_to(value, expected);
        let call = self.local_delegate_write(storage, binding, value, name.span)?;
        out.extend(sink);
        Some(hir::StatementKind::Expr(call))
    }
}
