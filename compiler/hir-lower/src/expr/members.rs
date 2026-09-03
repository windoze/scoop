use super::*;

mod extensions;
mod pointers;
mod resolution;

impl Lowerer {
    /// `this` (M6): only inside member functions, where it is
    /// parameter 0 (`lower_body` registers it as a local).
    pub(super) fn lower_this(&mut self, span: Span) -> Option<hir::Expr> {
        let Some(this) = self.lower_current_this(span) else {
            self.error(
                span,
                "`this` is only allowed inside member functions".to_string(),
            );
            return None;
        };
        Some(this)
    }

    /// `receiver.name(args)` (M6/M7): the method overloads are
    /// collected from the receiver's static type — class members (base
    /// chain included), interface methods, or struct / enum methods — and
    /// resolved by the unified M16 algorithm. If that layer has no applicable
    /// candidate, visible extensions are probed in import priority order.
    /// Single and multiple candidates use the same entry.
    /// The dispatch kind (direct / virtual / interface) is decided at
    /// MIR from the receiver's static type (hir docs).
    ///
    /// One receiver shape is not a method call: the M6 parser folds a
    /// qualified enum variant construction `E.V(args)` into this
    /// syntax (`MethodCall { receiver: Var("E"), ... }`). When the
    /// receiver is a bare name that is no in-scope variable and no
    /// property of the current host — but names an enum — it is a
    /// variant path and goes through variant construction (M4 rules:
    /// variant existence, per-field argument checks, type-argument
    /// inference, constructor-style defaults). Variables and host
    /// properties shadow enum names. Core array conversion methods enter the
    /// ordinary member candidate layer and are normalized only after their
    /// typed intrinsic target wins (spec 10.4).
    pub(super) fn lower_method_call(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        if let ast::Expr::Var(enum_name) = receiver {
            if self.scopes.lookup(&enum_name.text).is_none()
                && !self.host_has_property(&enum_name.text)
                && self.enums_by_name.contains_key(&enum_name.text)
            {
                let enum_id = self.enums_by_name[&enum_name.text];
                let Some(variant) = self.find_variant(enum_id, &name.text) else {
                    self.error(
                        name.span,
                        format!("enum `{}` has no variant `{}`", enum_name.text, name.text),
                    );
                    return None;
                };
                return self.lower_variant_construct(enum_id, variant, call, sink, expected);
            }
        }
        let receiver = self.lower_expr(receiver, sink, None)?;
        self.lower_explicit_named_call(
            receiver,
            name,
            call,
            sink,
            expected,
            RequiredCallableModifiers::default(),
        )
    }

    fn lower_explicit_named_call(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
        direct_required: RequiredCallableModifiers,
    ) -> Option<hir::Expr> {
        if name.text == "invoke" && matches!(self.types[receiver.ty], Type::Function(_)) {
            return self.lower_named_call_on_receiver(
                receiver,
                name,
                call,
                sink,
                expected,
                direct_required,
            );
        }
        if name.text == "invoke" && matches!(self.types[receiver.ty], Type::FunPtr(_)) {
            self.error(
                call.span,
                "FunPtr values are not callable in Scoop".to_string(),
            );
            return None;
        }
        let property = self.member_property_read(receiver.clone(), name);
        let mut first_failure = None;
        let mut members = self.methods_by_name(receiver.ty, &name.text);
        members.retain(|candidate| {
            Self::matches_required_modifiers(
                self.signatures[&candidate.function].modifiers,
                direct_required,
            )
        });
        if !members.is_empty() && matches!(self.types[receiver.ty], Type::Interface(..)) {
            let before = members.len();
            members.retain(|candidate| {
                let signature = &self.signatures[&candidate.function];
                signature.type_params.len() == signature.owner_type_param_count
            });
            if members.is_empty() && before != 0 {
                let found = self.type_name(receiver.ty);
                let mut failure = self.clone();
                failure.error(
                    name.span,
                    format!(
                        "generic member function `{}` cannot be called through interface type `{found}`",
                        name.text
                    ),
                );
                first_failure = Some(Box::new(failure));
            }
        }
        if !members.is_empty() {
            match self.probe_expr_layer(|state, layer_sink| {
                state.finish_overloaded_method_call(
                    members,
                    &name.text,
                    receiver.clone(),
                    call,
                    layer_sink,
                    expected,
                )
            }) {
                Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                Err(failure) => first_failure = Some(failure),
            }
        }

        if let Some(property) = &property
            && let Some(layer) = self.probe_property_member_invoke(
                property.clone(),
                call,
                expected,
                direct_required.infix,
            )
        {
            match layer {
                Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                Err(failure) => {
                    first_failure.get_or_insert(failure);
                }
            }
        }

        for same_side in [true, false] {
            let mut extensions = self.extension_candidates_on_side(&name.text, same_side);
            extensions.retain(|function| {
                Self::matches_required_modifiers(
                    self.signatures[function].modifiers,
                    direct_required,
                )
            });
            if !extensions.is_empty() {
                match self.probe_expr_layer(|state, layer_sink| {
                    state.finish_extension_call(
                        &extensions,
                        &name.text,
                        receiver.clone(),
                        call,
                        layer_sink,
                        expected,
                    )
                }) {
                    Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                    Err(failure) => {
                        first_failure.get_or_insert(failure);
                    }
                }
            }
            if let Some(property) = &property
                && let Some(layer) = self.probe_property_extension_invoke(
                    property.clone(),
                    call,
                    expected,
                    direct_required.infix,
                    same_side,
                )
            {
                match layer {
                    Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                    Err(failure) => {
                        first_failure.get_or_insert(failure);
                    }
                }
            }
        }

        if let Some(failure) = first_failure {
            self.commit_layer_diagnostics(*failure);
        } else {
            let found = self.type_name(receiver.ty);
            let capability = if direct_required.infix {
                "infix callable"
            } else {
                "method"
            };
            self.error(
                name.span,
                format!("type `{found}` has no {capability} `{}`", name.text),
            );
        }
        None
    }

    fn matches_required_modifiers(
        modifiers: hir::CallableModifiers,
        required: RequiredCallableModifiers,
    ) -> bool {
        required
            .operator
            .is_none_or(|operator| modifiers.operator == Some(operator))
            && (!required.infix || modifiers.is_infix)
    }

    pub(in crate::expr) fn extension_candidates_on_side(
        &self,
        name: &str,
        same_side: bool,
    ) -> Vec<hir::FunctionId> {
        let call_site_is_core = self.current_file < self.user_file_index;
        self.extensions_by_name
            .get(name)
            .into_iter()
            .flatten()
            .copied()
            .filter(|function| {
                let candidate_is_core = self.function_files[function] < self.user_file_index;
                (candidate_is_core == call_site_is_core) == same_side
            })
            .collect()
    }

    pub(in crate::expr) fn member_property_read(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
    ) -> Option<hir::Expr> {
        let (field, ty) = match self.types[receiver.ty].clone() {
            Type::Class(application) => {
                let (application, index, ty, _) =
                    self.find_class_application_field(application, &name.text)?;
                (hir::FieldRef::ClassField { application, index }, ty)
            }
            Type::Struct(application) => {
                let value = self.struct_applications[application].clone();
                let index = self.structs[value.template]
                    .semantic_fields()
                    .iter()
                    .position(|field| field.name == name.text)?;
                let ty = self.instantiate_ty(
                    self.structs[value.template].semantic_fields()[index].ty,
                    &value.arguments,
                );
                (
                    hir::FieldRef::StructField {
                        application,
                        index: index as u32,
                    },
                    ty,
                )
            }
            _ => return None,
        };
        Some(hir::Expr {
            kind: ExprKind::FieldAccess {
                receiver: Box::new(receiver),
                field,
            },
            ty,
            span: name.span,
            origin: self.expression_origin(name.span),
        })
    }

    pub(in crate::expr) fn probe_property_member_invoke(
        &mut self,
        property: hir::Expr,
        call: CallSite<'_>,
        expected: Option<TypeId>,
        require_infix: bool,
    ) -> Option<Result<SuccessfulExprLayer, Box<Lowerer>>> {
        let mut candidates = self.methods_by_name(property.ty, "invoke");
        candidates.retain(|candidate| {
            Self::matches_required_modifiers(
                self.signatures[&candidate.function].modifiers,
                RequiredCallableModifiers {
                    operator: Some(hir::OperatorKind::Invoke),
                    infix: require_infix,
                },
            )
        });
        if candidates.is_empty() {
            return None;
        }
        Some(self.probe_expr_layer(|state, layer_sink| {
            state.finish_overloaded_method_call(
                candidates, "invoke", property, call, layer_sink, expected,
            )
        }))
    }

    pub(in crate::expr) fn probe_property_extension_invoke(
        &self,
        property: hir::Expr,
        call: CallSite<'_>,
        expected: Option<TypeId>,
        require_infix: bool,
        same_side: bool,
    ) -> Option<Result<SuccessfulExprLayer, Box<Lowerer>>> {
        let mut candidates = self.extension_candidates_on_side("invoke", same_side);
        candidates.retain(|function| {
            Self::matches_required_modifiers(
                self.signatures[function].modifiers,
                RequiredCallableModifiers {
                    operator: Some(hir::OperatorKind::Invoke),
                    infix: require_infix,
                },
            )
        });
        if candidates.is_empty() {
            return None;
        }
        Some(self.probe_expr_layer(|state, layer_sink| {
            state.finish_extension_call(&candidates, "invoke", property, call, layer_sink, expected)
        }))
    }

    pub(crate) fn lower_named_call_on_receiver(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
        required: RequiredCallableModifiers,
    ) -> Option<hir::Expr> {
        if name.text == "invoke" && matches!(self.types[receiver.ty], Type::Function(_)) {
            if required.operator.is_some() || required.infix {
                let found = self.type_name(receiver.ty);
                self.error(
                    name.span,
                    format!(
                        "type `{found}` has no matching callable role `{}`",
                        name.text
                    ),
                );
                return None;
            }
            if !call.type_args.is_empty() {
                self.error(
                    name.span,
                    "function values do not accept explicit type arguments".to_string(),
                );
                return None;
            }
            return self.lower_callable_call(receiver, call.args, call.span, sink);
        }
        let mut candidates = self.methods_by_name(receiver.ty, &name.text);
        candidates.retain(|candidate| {
            let modifiers = self.signatures[&candidate.function].modifiers;
            Self::matches_required_modifiers(modifiers, required)
        });
        let mut first_failure = None;
        if !candidates.is_empty() && matches!(self.types[receiver.ty], Type::Interface(..)) {
            let before = candidates.len();
            candidates.retain(|candidate| {
                let sig = &self.signatures[&candidate.function];
                sig.type_params.len() == sig.owner_type_param_count
            });
            if candidates.is_empty() && before != 0 {
                let found = self.type_name(receiver.ty);
                let mut failure = self.clone();
                failure.error(
                    name.span,
                    format!(
                        "generic member function `{}` cannot be called through interface type `{found}`",
                        name.text
                    ),
                );
                first_failure = Some(Box::new(failure));
            }
        }
        if !candidates.is_empty() {
            match self.probe_expr_layer(|state, layer_sink| {
                state.finish_overloaded_method_call(
                    candidates,
                    &name.text,
                    receiver.clone(),
                    call,
                    layer_sink,
                    expected,
                )
            }) {
                Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                Err(failure) => first_failure = Some(failure),
            }
        }

        for mut extensions in self.extension_candidate_layers(&name.text) {
            extensions.retain(|function| {
                let modifiers = self.signatures[function].modifiers;
                Self::matches_required_modifiers(modifiers, required)
            });
            if extensions.is_empty() {
                continue;
            }
            match self.probe_expr_layer(|state, layer_sink| {
                state.finish_extension_call(
                    &extensions,
                    &name.text,
                    receiver.clone(),
                    call,
                    layer_sink,
                    expected,
                )
            }) {
                Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                Err(failure) => {
                    first_failure.get_or_insert(failure);
                }
            }
        }

        if let Some(failure) = first_failure {
            self.commit_layer_diagnostics(*failure);
        } else {
            let found = self.type_name(receiver.ty);
            self.error(
                name.span,
                format!("type `{found}` has no method `{}`", name.text),
            );
        }
        None
    }

    pub(super) fn lower_infix_call(
        &mut self,
        lhs: &ast::Expr,
        target: &ast::InfixTarget,
        rhs: &ast::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let receiver = self.lower_expr(lhs, sink, None)?;
        let args = [ast::CallArgument::positional(rhs.clone())];
        let call = CallSite {
            type_args: &[],
            args: &args,
            span,
        };
        match target {
            ast::InfixTarget::Named(name) => self.lower_explicit_named_call(
                receiver,
                name,
                call,
                sink,
                expected,
                RequiredCallableModifiers {
                    operator: None,
                    infix: true,
                },
            ),
            ast::InfixTarget::Invoke => {
                self.lower_value_invoke(receiver, call, sink, expected, true)
            }
        }
    }

    pub(super) fn lower_safe_method_call(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let receiver = self.lower_expr(receiver, sink, None)?;
        let Some(inner) = self.as_option(receiver.ty) else {
            let found = self.type_name(receiver.ty);
            self.error(
                call.span,
                format!("`?.` requires an Option receiver, found {found}"),
            );
            return None;
        };
        let option_ty = receiver.ty;
        let origin = self.expression_origin(call.span);
        let receiver_local = self.alloc_hidden("opt", option_ty);
        sink.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding {
                    local: receiver_local,
                },
                init: receiver,
            },
            span: call.span,
        });
        let receiver_ref = hir::Expr {
            kind: ExprKind::Local(receiver_local),
            ty: option_ty,
            span: call.span,
            origin,
        };
        let payload = hir::Expr {
            kind: ExprKind::Unwrap {
                operand: Box::new(receiver_ref.clone()),
                trap_on_none: false,
            },
            ty: inner,
            span: call.span,
            origin,
        };
        let mut then_body = Vec::new();
        let value = self.lower_named_call_on_receiver(
            payload,
            name,
            call,
            &mut then_body,
            expected.and_then(|ty| self.as_option(ty)),
            RequiredCallableModifiers::default(),
        )?;
        let result_ty = self.option_type(value.ty);
        let result = self.alloc_hidden("res", result_ty);
        then_body.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local: result },
                init: hir::Expr {
                    kind: ExprKind::SomeWrap(Box::new(value)),
                    ty: result_ty,
                    span: call.span,
                    origin,
                },
            },
            span: call.span,
        });
        let else_body = vec![hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local: result },
                init: hir::Expr {
                    kind: ExprKind::NoneLiteral,
                    ty: result_ty,
                    span: call.span,
                    origin,
                },
            },
            span: call.span,
        }];
        sink.push(hir::Statement {
            kind: hir::StatementKind::If {
                cond: hir::Expr {
                    kind: ExprKind::IsSome(Box::new(receiver_ref)),
                    ty: self.boolean,
                    span: call.span,
                    origin,
                },
                then_body,
                else_body: Some(else_body),
            },
            span: call.span,
        });
        Some(hir::Expr {
            kind: ExprKind::Local(result),
            ty: result_ty,
            span: call.span,
            origin,
        })
    }
}
