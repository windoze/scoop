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
            let ty = self
                .smart_casts
                .get(&local)
                .copied()
                .unwrap_or(self.locals[local].ty);
            if self.type_exposes_invoke(ty, false) || matches!(self.types[ty], Type::FunPtr(_)) {
                let callee = hir::Expr {
                    kind: ExprKind::Local(local),
                    ty,
                    span: call.callee.span,
                    origin: self.expression_origin(call.callee.span),
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
        if let Some(capture) = self.available_capture(&call.callee.text)
            && (self.type_exposes_invoke(capture.ty, false)
                || matches!(self.types[capture.ty], Type::FunPtr(_)))
        {
            let callee = self.lower_capture(&call.callee)?;
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
        if let Some(&global) = self.globals_by_name.get(&call.callee.text) {
            let ty = self.globals[global].ty;
            if self.type_exposes_invoke(ty, false) || matches!(self.types[ty], Type::FunPtr(_)) {
                if matches!(
                    self.globals[global].storage,
                    hir::GlobalStorage::Extern { .. }
                ) {
                    self.require_unsafe_operation(call.callee.span, "reading an extern global");
                }
                let callee = hir::Expr {
                    kind: ExprKind::GlobalRead(global),
                    ty,
                    span: call.callee.span,
                    origin: self.expression_origin(call.callee.span),
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
        // An intrinsic array class in constructor position denotes the
        // opposite-family snapshot conversion. The class namespace resolves
        // the source name; the typed declaration kind selects the operation.
        if let Some(&(class, _)) = self.classes_by_name.get(&call.callee.text) {
            if let Some(target_kind) = self.array_class_kind(class) {
                return self.lower_array_conversion(call, sink, class, target_kind, expected);
            }
        }
        match self.classify_constructor(&call.callee)? {
            Constructor::Variant { enum_id, variant } => self.lower_variant_construct(
                enum_id,
                variant,
                CallSite {
                    type_args: &call.type_args,
                    args: &call.args,
                    span: call.span,
                },
                sink,
                expected,
            ),
            Constructor::Struct { struct_id, ty } => {
                if Some(struct_id) == self.ffi_foreign_callback {
                    self.error(
                        call.span,
                        "`ForeignCallback` values can only be produced by `foreignCallback`"
                            .to_string(),
                    );
                    return None;
                }
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
            Constructor::Class { class_id } => self.lower_class_construct(
                class_id,
                CallSite {
                    type_args: &call.type_args,
                    args: &call.args,
                    span: call.span,
                },
                sink,
                expected,
            ),
            Constructor::Unmatched => self.lower_function_call(call, sink, expected),
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
