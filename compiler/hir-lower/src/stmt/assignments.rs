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
        let Some((declaring, index, field_ty, mutable)) =
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
            self.error(
                assign.value.span(),
                format!(
                    "cannot assign value of type {found} to property `{}` of type {expected}",
                    name.text
                ),
            );
            return None;
        }
        let value = self.adapt_to(value, field_ty);
        Some(hir::StatementKind::Assign {
            target: hir::AssignTarget::Field {
                receiver: Box::new(receiver),
                field: hir::FieldRef::ClassField {
                    application: declaring,
                    index,
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
                    self.error(
                        assign.value.span(),
                        format!(
                            "cannot assign value of type {} to global `{}` of type {}",
                            self.type_name(value.ty),
                            name.text,
                            self.type_name(expected)
                        ),
                    );
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
            self.error(
                assign.value.span(),
                format!(
                    "cannot assign value of type {found} to `{}` of type {expected_name}",
                    name.text
                ),
            );
            return None;
        }
        let value = self.adapt_to(value, expected);
        out.extend(sink);
        Some(hir::StatementKind::Assign {
            target: hir::AssignTarget::Local(local),
            value,
        })
    }

    /// `array[index] = value` (spec 10.5, milestone5 DESIGN.md 2.2):
    /// only `MutableArray<T>` is assignable (an `Array<T>` receiver
    /// gets its own diagnostic), the index must be `Int` and the value
    /// exactly `T`.
    fn lower_index_assign(
        &mut self,
        assign: &ast::Assign,
        receiver: &ast::Expr,
        indices: &[ast::Expr],
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        let mut sink = Vec::new();
        let array = self.lower_expr(receiver, &mut sink, None)?;
        let element_ty = match self.array_type_info(array.ty) {
            Some(array_info) if array_info.kind == ArrayKind::Mutable => {
                let [index] = indices else {
                    self.error(
                        assign.span,
                        format!(
                            "array assignment takes exactly one index, but {} were supplied",
                            indices.len()
                        ),
                    );
                    return None;
                };
                let index = self.lower_expr(index, &mut sink, Some(self.int))?;
                if index.ty != self.int {
                    let found = self.type_name(index.ty);
                    self.error(
                        index.span,
                        format!("array index must be Int, found {found}"),
                    );
                    return None;
                }
                let value = self.lower_expr(&assign.value, &mut sink, Some(array_info.element))?;
                if !self.is_subtype(value.ty, array_info.element) {
                    let expected = self.type_name(array_info.element);
                    let found = self.type_name(value.ty);
                    self.error(
                        assign.value.span(),
                        format!(
                            "cannot assign value of type {found} to an array element of type {expected}"
                        ),
                    );
                    return None;
                }
                let value = self.adapt_to(value, array_info.element);
                out.extend(sink);
                return Some(hir::StatementKind::Assign {
                    target: hir::AssignTarget::Index {
                        array: Box::new(array),
                        index: Box::new(index),
                    },
                    value,
                });
            }
            Some(_) => {
                let found = self.type_name(array.ty);
                self.error(
                    receiver.span(),
                    format!("cannot assign to an element of immutable {found}"),
                );
                return None;
            }
            _ => self.unit,
        };
        debug_assert_eq!(element_ty, self.unit);
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
