//! Calls, callable references, anonymous functions, lambdas and callbacks.

use super::*;

impl Lowerer {
    pub(super) fn lower_value_invoke(
        &mut self,
        callee: hir::Expr,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
        require_infix: bool,
    ) -> Option<hir::Expr> {
        if matches!(self.types[callee.ty], Type::Function(_)) {
            if require_infix {
                let found = self.type_name(callee.ty);
                self.error(
                    call.span,
                    format!("function value of type {found} does not declare infix `invoke`"),
                );
                return None;
            }
            if !call.type_args.is_empty() {
                self.error(
                    call.span,
                    "function values do not accept explicit type arguments".to_string(),
                );
                return None;
            }
            return self.lower_callable_call(callee, call.args, call.span, sink);
        }
        if matches!(self.types[callee.ty], Type::FunPtr(_)) {
            self.error(
                call.span,
                "FunPtr values are not callable in Scoop".to_string(),
            );
            return None;
        }
        if !self.type_exposes_invoke(callee.ty, require_infix) {
            let found = self.type_name(callee.ty);
            self.error(call.span, format!("value of type {found} is not callable"));
            return None;
        }
        let name = ast::Ident {
            text: "invoke".to_string(),
            span: call.span,
        };
        self.lower_named_call_on_receiver(
            callee,
            &name,
            call,
            sink,
            expected,
            RequiredCallableModifiers {
                operator: Some(hir::OperatorKind::Invoke),
                infix: require_infix,
                ..Default::default()
            },
        )
    }

    pub(super) fn type_exposes_invoke(&mut self, ty: TypeId, require_infix: bool) -> bool {
        if matches!(self.types[ty], Type::Function(_)) {
            return !require_infix;
        }
        let matching = |modifiers: hir::CallableModifiers| {
            modifiers.operator == Some(hir::OperatorKind::Invoke)
                && (!require_infix || modifiers.is_infix)
        };
        if self
            .methods_by_name(ty, "invoke")
            .into_iter()
            .any(|candidate| matching(self.signatures[&candidate.function].modifiers))
        {
            return true;
        }
        self.extension_candidate_layers("invoke")
            .into_iter()
            .flatten()
            .any(|function| matching(self.signatures[&function].modifiers))
    }

    /// `Name(args...)` in call position: a variant or struct
    /// construction when the name resolves as one, a direct function
    /// call otherwise.
    pub(super) fn lower_call(
        &mut self,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        if let Some(local) = self.scopes.lookup(&call.callee.text) {
            let binding = self.locals[local].binding;
            let ty = self
                .local_delegate_plans
                .get(&binding)
                .map(|plan| plan.property_ty)
                .or_else(|| self.smart_casts.get(&local).copied())
                .unwrap_or(self.locals[local].ty);
            if self.type_exposes_invoke(ty, false) || matches!(self.types[ty], Type::FunPtr(_)) {
                let callee = if self.local_delegate_plans.contains_key(&binding) {
                    self.lower_var(&call.callee, sink, None)?
                } else {
                    hir::Expr {
                        kind: ExprKind::Local(local),
                        ty,
                        span: call.callee.span,
                        origin: self.expression_origin(call.callee.span),
                    }
                };
                return self.lower_value_invoke(
                    callee,
                    CallSite {
                        type_args: &call.type_args,
                        args: &call.args,
                        span: call.span,
                    },
                    sink,
                    expected,
                    false,
                );
            }
        }
        if let Some(capture) = self.available_capture(&call.callee.text) {
            let ty = self
                .local_delegate_plans
                .get(&capture.binding)
                .map_or(capture.ty, |plan| plan.property_ty);
            if self.type_exposes_invoke(ty, false) || matches!(self.types[ty], Type::FunPtr(_)) {
                let callee = if self.local_delegate_plans.contains_key(&capture.binding) {
                    self.lower_var(&call.callee, sink, None)?
                } else {
                    self.lower_capture(&call.callee)?
                };
                return self.lower_value_invoke(
                    callee,
                    CallSite {
                        type_args: &call.type_args,
                        args: &call.args,
                        span: call.span,
                    },
                    sink,
                    expected,
                    false,
                );
            }
        }
        let constructor = self.classify_constructor(&call.callee)?;
        if !matches!(&constructor, Constructor::Unmatched) {
            if call.callee.text.contains('.') {
                return self.lower_nominal_constructor_call(constructor, call, sink, expected);
            }
            return match self.probe_expr_layer(move |state, layer_sink| {
                state.lower_nominal_constructor_call(constructor, call, layer_sink, expected)
            }) {
                Ok(layer) => Some(self.commit_expr_layer(layer, sink)),
                Err(failure) => self.lower_function_call(call, sink, expected, Some(failure)),
            };
        }

        let object = self
            .lexical_nested_nominal_target(&call.callee.text)
            .or_else(|| self.top_level_nominal_target(&call.callee.text))
            .and_then(|target| match target {
                crate::NominalTarget::Object(object) => Some(object),
                _ => None,
            });
        let object_failure = object.map(|object| {
            let kind = match self.objects[object].kind {
                hir::ObjectKind::Standalone => "object",
                hir::ObjectKind::Companion(_) => "companion object",
            };
            let mut failure = self.clone();
            failure.error(
                call.span,
                format!("{kind} `{}` cannot be constructed", call.callee.text),
            );
            Box::new(failure)
        });
        self.lower_function_call(call, sink, expected, object_failure)
    }

    fn lower_nominal_constructor_call(
        &mut self,
        constructor: Constructor,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        match constructor {
            Constructor::Variant { target, alias } => {
                let expected =
                    self.alias_fixed_expected(alias.as_ref(), &call.type_args, expected)?;
                self.lower_variant_construct(
                    target,
                    CallSite {
                        type_args: &call.type_args,
                        args: &call.args,
                        span: call.span,
                    },
                    sink,
                    expected,
                )
            }
            Constructor::Struct {
                struct_id,
                ty,
                alias,
            } => {
                if Some(struct_id) == self.ffi_foreign_callback {
                    self.error(
                        call.span,
                        "`ForeignCallback` values can only be produced by `foreignCallback`"
                            .to_string(),
                    );
                    return None;
                }
                let expected =
                    self.alias_fixed_expected(alias.as_ref(), &call.type_args, expected)?;
                let site = CallSite {
                    type_args: &call.type_args,
                    args: &call.args,
                    span: call.span,
                };
                if Some(struct_id) == self.ffi_ptr || Some(struct_id) == self.ffi_fun_ptr {
                    self.lower_ffi_struct_init(struct_id, site, sink, expected)
                } else {
                    self.lower_struct_init(struct_id, ty, site, sink, expected)
                }
            }
            Constructor::Class { class_id, alias } => {
                let expected =
                    self.alias_fixed_expected(alias.as_ref(), &call.type_args, expected)?;
                if let Some(target_kind) = self.array_class_kind(class_id) {
                    self.lower_array_conversion(call, sink, class_id, target_kind, expected)
                } else {
                    self.lower_class_construct(
                        class_id,
                        CallSite {
                            type_args: &call.type_args,
                            args: &call.args,
                            span: call.span,
                        },
                        sink,
                        expected,
                    )
                }
            }
            Constructor::Unmatched => None,
        }
    }

    pub(super) fn lower_callable_call(
        &mut self,
        callee: hir::Expr,
        args: &[ast::CallArgument],
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let Type::Function(function_type) = self.types[callee.ty] else {
            let found = self.type_name(callee.ty);
            self.error(
                callee.span,
                format!("value of type {found} is not callable"),
            );
            return None;
        };
        let signature = self.function_types[function_type].clone();
        if let Some(argument) = args
            .iter()
            .find(|argument| !matches!(argument.name, ast::CallArgumentName::Positional))
        {
            self.error(
                argument.span,
                "function values do not accept named arguments".to_string(),
            );
            return None;
        }
        if let Some(argument) = args
            .iter()
            .find(|argument| matches!(argument.spread, ast::SpreadSyntax::Spread(_)))
        {
            self.error(
                argument.span,
                "function values do not accept spread arguments".to_string(),
            );
            return None;
        }
        if signature.parameter_types.len() != args.len() {
            self.error(
                span,
                format!(
                    "function value takes exactly {} argument(s), but {} were supplied",
                    signature.parameter_types.len(),
                    args.len()
                ),
            );
            return None;
        }
        if signature.is_suspend {
            let context = *self
                .suspension_contexts
                .last()
                .expect("the suspension context stack is initialized");
            if let SuspensionContext::Forbidden(reason) = context {
                let location = match reason {
                    ForbiddenSuspendContext::TopLevel => "a non-suspend declaration".to_string(),
                    ForbiddenSuspendContext::Function => {
                        format!("non-suspend function `{}`", self.current_fn_name)
                    }
                    ForbiddenSuspendContext::DefaultExpression => {
                        format!("non-suspend default expression {}", self.current_fn_name)
                    }
                    ForbiddenSuspendContext::ConstructorDelegation => {
                        "constructor delegation".to_string()
                    }
                    ForbiddenSuspendContext::ConstructorInitialization => {
                        "constructor initialization".to_string()
                    }
                };
                self.error(
                    span,
                    format!("suspend function value cannot be called from {location}"),
                );
                return None;
            }
        }
        let mut lowered = Vec::with_capacity(args.len());
        for (arg, &parameter_ty) in args.iter().zip(&signature.parameter_types) {
            let value = self.lower_expr(&arg.expression, sink, Some(parameter_ty))?;
            if !self.is_subtype(value.ty, parameter_ty) {
                let expected = self.type_name(parameter_ty);
                let found = self.type_name(value.ty);
                self.error(
                    arg.span,
                    format!("function argument must be of type {expected}, found {found}"),
                );
                return None;
            }
            lowered.push(self.adapt_to(value, parameter_ty));
        }
        Some(hir::Expr {
            kind: ExprKind::CallableCall {
                callee: Box::new(callee),
                function_type,
                args: lowered,
            },
            ty: signature.return_type,
            span,
            origin: self.expression_origin(span),
        })
    }
}
