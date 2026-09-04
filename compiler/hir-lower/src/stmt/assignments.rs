use super::*;
use crate::expr::{CallSite, RequiredCallableModifiers};

impl Lowerer {
    /// Assignment. A `Local` target names a declared, mutable local; an
    /// `Index` target (`array[index] = value`, spec 10.5) requires a
    /// `MutableArray<T>` receiver, an `Int` index and a value of
    /// exactly the element type `T`; a `Field` target
    /// (`obj.field = value`, M6) requires a class receiver and a `var`
    /// constructor property. In all cases the target type is the
    /// value's expected-type hint.
    pub(super) fn lower_assign(
        &mut self,
        assign: &ast::Assign,
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        if let ast::AssignmentOp::Compound(op) = assign.op {
            return self.lower_compound_assign(assign, op, out);
        }
        match &assign.target {
            ast::AssignTarget::Name(name) => self.lower_local_assign(assign, name, out),
            ast::AssignTarget::Index {
                receiver, indices, ..
            } => self.lower_index_assign(assign, receiver, indices.as_slice(), out),
            ast::AssignTarget::Field { receiver, name, .. } => {
                if matches!(&**receiver, ast::Expr::This { .. })
                    && self.initialization_context.is_some()
                {
                    return self.lower_initializing_field_assign(assign, name, out);
                }
                let mut sink = Vec::new();
                let Some(receiver) = self.lower_expr(receiver, &mut sink, None) else {
                    return None; // diagnostic already recorded
                };
                let kind = self.assign_class_field(assign, receiver, name, &mut sink)?;
                out.extend(sink);
                Some(kind)
            }
            ast::AssignTarget::QualifiedInterfaceSuperProperty {
                qualifier,
                name,
                span,
            } => {
                let property =
                    self.resolve_qualified_interface_super_property(qualifier, name, *span)?;
                let mut sink = Vec::new();
                let value = self.lower_expr(&assign.value, &mut sink, Some(property.ty))?;
                if !self.is_subtype(value.ty, property.ty) {
                    self.error(
                        assign.value.span(),
                        format!(
                            "cannot assign value of type {} to property `{}` of type {}",
                            self.type_name(value.ty),
                            name.text,
                            self.type_name(property.ty)
                        ),
                    );
                    return None;
                }
                let expected = property.ty;
                let value = self.adapt_to(value, expected);
                out.extend(sink);
                self.lower_direct_interface_property_write(property, value, *span)
            }
        }
    }

    fn lower_initializing_field_assign(
        &mut self,
        assign: &ast::Assign,
        name: &ast::Ident,
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        let field = self.initializing_field(name, assign.span)?;
        let Some(target) = field.write else {
            self.error(
                name.span,
                format!("cannot assign to immutable property `{}`", name.text),
            );
            return None;
        };
        let mut sink = Vec::new();
        let value = self.lower_expr(&assign.value, &mut sink, Some(field.read.ty))?;
        if !self.is_subtype(value.ty, field.read.ty) {
            let message = self.with_nominal_invariance_detail(
                format!(
                    "cannot assign value of type {} to property `{}` of type {}",
                    self.type_name(value.ty),
                    name.text,
                    self.type_name(field.read.ty)
                ),
                value.ty,
                field.read.ty,
            );
            self.error(assign.value.span(), message);
            return None;
        }
        out.extend(sink);
        Some(hir::StatementKind::Assign {
            target,
            value: self.adapt_to(value, field.read.ty),
        })
    }

    /// `receiver.name = value` (M6): the receiver must be a class and
    /// `name` a `var` constructor property on it or its base chain
    /// (resolved exactly like a field read — absolute layout index,
    /// declaring class in the `FieldRef`). Value types are immutable
    /// and reject the assignment outright. The value's desugaring
    /// statements append to the receiver's sink (evaluation order:
    /// receiver, then value, then the store).
    fn assign_class_field(
        &mut self,
        assign: &ast::Assign,
        receiver: hir::Expr,
        name: &ast::Ident,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        let receiver_ty = receiver.ty;
        if let Some((property, owner, property_ty)) =
            self.find_accessible_nominal_property(receiver_ty, &name.text)
        {
            let value = self.lower_expr(&assign.value, sink, Some(property_ty))?;
            if !self.is_subtype(value.ty, property_ty) {
                let expected = self.type_name(property_ty);
                let found = self.type_name(value.ty);
                let message = self.with_nominal_invariance_detail(
                    format!(
                        "cannot assign value of type {found} to property `{}` of type {expected}",
                        name.text
                    ),
                    value.ty,
                    property_ty,
                );
                self.error(assign.value.span(), message);
                return None;
            }
            let value = self.adapt_to(value, property_ty);
            return self.lower_property_write(
                property,
                Some(owner),
                Some(receiver),
                value,
                name.span,
            );
        }
        let resolved = match self.resolve_extension_property(receiver, name, sink, false) {
            crate::properties::ExtensionPropertyResolution::Resolved(property) => property,
            crate::properties::ExtensionPropertyResolution::Failed => return None,
            crate::properties::ExtensionPropertyResolution::NoCandidate => {
                match self.types[receiver_ty] {
                    Type::Class(application) => {
                        let class = self.classes[self.class_applications[application].template]
                            .name
                            .clone();
                        self.error(
                            name.span,
                            format!("class `{class}` has no property `{}`", name.text),
                        );
                    }
                    _ if self.is_value_ty(receiver_ty) => self.error(
                        name.span,
                        "field assignment is not supported (value types are immutable)".to_string(),
                    ),
                    _ => {
                        let found = self.type_name(receiver_ty);
                        self.error(name.span, format!("type `{found}` has no fields"));
                    }
                }
                return None;
            }
        };
        let property_ty = resolved.read.ty;
        let value = self.lower_expr(&assign.value, sink, Some(property_ty))?;
        if !self.is_subtype(value.ty, property_ty) {
            let expected = self.type_name(property_ty);
            let found = self.type_name(value.ty);
            let message = self.with_nominal_invariance_detail(
                format!(
                    "cannot assign value of type {found} to property `{}` of type {expected}",
                    name.text
                ),
                value.ty,
                property_ty,
            );
            self.error(assign.value.span(), message);
            return None;
        }
        let value = self.adapt_to(value, property_ty);
        self.lower_extension_property_write(*resolved, value, name.span)
    }

    /// `name = value`: the target must be a declared, mutable local and
    /// the value type must match the local's type. Inside a class
    /// method a bare name may also denote a constructor property
    /// (`y = v` meaning `this.y = v`): `var` properties store through
    /// `this`, `val` properties are immutable (diagnostic).
    /// Value-type fields stay unwritable as before.
    fn lower_local_assign(
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
                Some(Type::Class(application)) => {
                    if self
                        .find_accessible_class_application_property(
                            application,
                            &name.text,
                            self.current_this_ty().expect("member receiver type"),
                        )
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
            if self.initialization_context.is_none()
                && let Some(receiver) = self.lower_current_this(name.span)
            {
                let mut sink = Vec::new();
                match self.resolve_extension_property(receiver, name, &mut sink, false) {
                    crate::properties::ExtensionPropertyResolution::Resolved(property) => {
                        let expected = property.read.ty;
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
                    crate::properties::ExtensionPropertyResolution::Failed => return None,
                    crate::properties::ExtensionPropertyResolution::NoCandidate => {}
                }
            }
            if let Some(property) = self.visible_property(&name.text, None) {
                let expected = self.properties[property].ty;
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
                out.extend(sink);
                return self.lower_property_write(property, None, None, value, name.span);
            }
            self.error(name.span, format!("unknown variable `{}`", name.text));
            return None;
        };
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

    /// Indexed assignment uses the same typed `operator set` resolver as user
    /// types. A winning MutableArray core declaration is normalized to the
    /// dedicated array store expression after applicability succeeds.
    fn lower_index_assign(
        &mut self,
        assign: &ast::Assign,
        receiver: &ast::Expr,
        indices: &[ast::Expr],
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        let mut sink = Vec::new();
        let array = self.lower_expr(receiver, &mut sink, None)?;
        let mut arguments = indices
            .iter()
            .cloned()
            .map(ast::CallArgument::positional)
            .collect::<Vec<_>>();
        arguments.push(ast::CallArgument::positional(assign.value.clone()));
        let call = self.lower_named_call_on_receiver(
            array,
            &ast::Ident {
                text: "set".to_string(),
                span: assign.span,
            },
            CallSite {
                type_args: &[],
                args: &arguments,
                span: assign.span,
            },
            &mut sink,
            Some(self.unit),
            RequiredCallableModifiers {
                operator: Some(hir::OperatorKind::Set),
                infix: false,
            },
        )?;
        out.extend(sink);
        Some(hir::StatementKind::Expr(call))
    }
}
