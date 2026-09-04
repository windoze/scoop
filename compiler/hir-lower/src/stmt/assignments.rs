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
        let application = match self.types[receiver.ty] {
            Type::Class(application) => application,
            _ if self.is_value_ty(receiver.ty) => {
                self.error(
                    name.span,
                    "field assignment is not supported (value types are immutable)".to_string(),
                );
                return None;
            }
            _ => {
                let found = self.type_name(receiver.ty);
                self.error(name.span, format!("type `{found}` has no fields"));
                return None;
            }
        };
        let class_id = self.class_applications[application].template;
        let Some((declaring, field, field_ty, mutable)) =
            self.find_class_application_field(application, &name.text)
        else {
            let class_name = self.classes[class_id].name.clone();
            self.error(
                name.span,
                format!("class `{class_name}` has no field `{}`", name.text),
            );
            return None;
        };
        if !mutable {
            self.error(
                name.span,
                format!("cannot assign to immutable property `{}`", name.text),
            );
            return None;
        }
        let value = self.lower_expr(&assign.value, sink, Some(field_ty))?;
        if !self.is_subtype(value.ty, field_ty) {
            let expected = self.type_name(field_ty);
            let found = self.type_name(value.ty);
            let message = self.with_nominal_invariance_detail(
                format!(
                    "cannot assign value of type {found} to property `{}` of type {expected}",
                    name.text
                ),
                value.ty,
                field_ty,
            );
            self.error(assign.value.span(), message);
            return None;
        }
        let value = self.adapt_to(value, field_ty);
        Some(hir::StatementKind::Assign {
            target: hir::AssignTarget::Field {
                receiver: Box::new(receiver),
                field: hir::FieldRef::ClassField {
                    application: declaring,
                    field,
                },
            },
            value,
        })
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
                    if let Some((_, _, _, mutable)) =
                        self.find_class_application_field(application, &name.text)
                    {
                        if !mutable {
                            self.error(
                                name.span,
                                format!("cannot assign to immutable property `{}`", name.text),
                            );
                            return None;
                        }
                        let receiver = self
                            .lower_current_this(name.span)
                            .expect("a receiver callable body always has a lexical `this`");
                        let mut sink = Vec::new();
                        let kind = self.assign_class_field(assign, receiver, name, &mut sink)?;
                        out.extend(sink);
                        return Some(kind);
                    }
                }
                Some(Type::Struct(application))
                    if self.structs[self.struct_applications[application].template]
                        .semantic_fields()
                        .iter()
                        .any(|field| field.name == name.text) =>
                {
                    self.error(
                        name.span,
                        format!("cannot assign to immutable property `{}`", name.text),
                    );
                    return None;
                }
                _ => {}
            }
            if let Some(&global) = self.globals_by_name.get(&name.text) {
                if !self.globals[global].mutable {
                    self.error(
                        name.span,
                        format!("cannot assign to immutable global `{}`", name.text),
                    );
                    return None;
                }
                if matches!(
                    self.globals[global].storage,
                    hir::GlobalStorage::Extern { .. }
                ) {
                    self.require_unsafe_operation(name.span, "writing an extern global");
                }
                let expected = self.globals[global].ty;
                let mut sink = Vec::new();
                let value = self.lower_expr(&assign.value, &mut sink, Some(expected))?;
                if !self.is_subtype(value.ty, expected) {
                    let message = self.with_nominal_invariance_detail(
                        format!(
                            "cannot assign value of type {} to global `{}` of type {}",
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
                return Some(hir::StatementKind::Assign {
                    target: hir::AssignTarget::Global(global),
                    value,
                });
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
