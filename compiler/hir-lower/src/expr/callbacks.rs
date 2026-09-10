use super::*;

impl Lowerer {
    /// The registration intrinsic's source declaration uses `Any` for its
    /// callback because Scoop types cannot express "remove the context
    /// parameter from F". Once the ordinary candidate prefilter has fixed the
    /// unique typed intrinsic target and explicit `F`, derive that candidate's
    /// real postponed-argument expectation inside its probe transaction.
    pub(crate) fn foreign_callback_argument_expected(
        &mut self,
        function: hir::FunctionId,
        explicit_type_args: &[ResolvedCallTypeArgument],
        argument_map: &crate::call_resolution::arguments::CandidateArgumentMap,
        args: &[ast::CallArgument],
        span: Span,
    ) -> Result<Option<(usize, TypeId)>, ()> {
        let Some(core) = self.foreign_callback_core else {
            return Ok(None);
        };
        if function != core.register {
            return Ok(None);
        }
        let [ResolvedCallTypeArgument::Explicit { ty: native_ty, .. }] = explicit_type_args else {
            self.error(
                span,
                "`foreignCallback` requires one explicit function type".to_string(),
            );
            return Err(());
        };
        let hir::Type::Function(native_function_type) = self.types[*native_ty] else {
            self.error(
                span,
                "`foreignCallback` type argument must be an ordinary concrete function type"
                    .to_string(),
            );
            return Err(());
        };
        if !self.validate_foreign_callback_signature(native_function_type, span) {
            return Err(());
        }
        let crate::call_resolution::arguments::ResolvedParameterInput::Explicit(context_source) =
            argument_map.parameters[1].input
        else {
            unreachable!("registration context index is a required explicit argument")
        };
        let context_index = &args[context_source.index()];
        let ast::Expr::IntLiteral(context_literal) = &context_index.expression else {
            self.error(
                context_index.span,
                "foreign callback `contextIndex` must be a compile-time integer literal"
                    .to_string(),
            );
            return Err(());
        };
        let native_signature = self.function_types[native_function_type].clone();
        let context_index = usize::try_from(context_literal.magnitude).ok();
        if context_index.is_none_or(|index| index >= native_signature.parameter_types.len()) {
            self.error(
                context_literal.span,
                "foreign callback `contextIndex` is outside the native signature".to_string(),
            );
            return Err(());
        }
        let context_index = context_index.expect("checked context index");
        let context_type = native_signature.parameter_types[context_index];
        if !matches!(self.types[context_type], hir::Type::Ptr(pointee) if pointee == self.unit) {
            self.error(
                context_literal.span,
                "foreign callback context parameter must be exactly `Ptr<Unit>`".to_string(),
            );
            return Err(());
        }

        let managed_parameters = native_signature
            .parameter_types
            .iter()
            .enumerate()
            .filter_map(|(index, ty)| (index != context_index).then_some(*ty))
            .collect();
        let crate::call_resolution::arguments::ResolvedParameterInput::Explicit(callback_source) =
            argument_map.parameters[0].input
        else {
            unreachable!("registration callback is a required explicit argument")
        };
        let expected =
            self.intern_function_type(false, managed_parameters, native_signature.return_type);
        Ok(Some((callback_source.index(), expected)))
    }

    pub(super) fn lower_foreign_callback_registration(
        &mut self,
        core: hir::ForeignCallbackCore,
        function: hir::FunctionId,
        call: &ast::CallExpr,
        resolved: crate::overload::ResolvedCallee,
        sink: &[hir::Statement],
    ) -> Option<hir::Expr> {
        debug_assert_eq!(function, core.register);
        let [native_ty] = resolved.type_args.as_slice() else {
            unreachable!("validated registration intrinsic has one concrete type argument")
        };
        let hir::Type::Function(native_function_type) = self.types[*native_ty] else {
            unreachable!("registration candidate validation requires a function type")
        };
        let native_signature = self.function_types[native_function_type].clone();
        let [closure, context_index, mode]: [hir::Expr; 3] = resolved
            .args
            .try_into()
            .expect("validated registration intrinsic has three arguments");
        let ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed64(context_index)) =
            materialized_source(&context_index, sink).kind
        else {
            unreachable!("registration candidate validation requires a literal context index")
        };
        let context_index = i64::from_ne_bytes(context_index.to_ne_bytes());
        debug_assert!(context_index >= 0);
        let context_index = context_index as usize;

        let managed_parameters = native_signature
            .parameter_types
            .iter()
            .enumerate()
            .filter_map(|(index, ty)| (index != context_index).then_some(*ty))
            .collect();
        let managed_ty =
            self.intern_function_type(false, managed_parameters, native_signature.return_type);
        let hir::Type::Function(managed_function_type) = self.types[managed_ty] else {
            unreachable!("interned managed callback signature is a function type")
        };
        if !self.types_equal(closure.ty, managed_ty) {
            self.error(
                closure.span,
                format!(
                    "foreign callback closure must have type {}, found {}",
                    self.type_name(managed_ty),
                    self.type_name(closure.ty)
                ),
            );
            return None;
        }

        let ExprKind::VariantConstruct { variant, args } = &materialized_source(&mode, sink).kind
        else {
            self.error(
                mode.span,
                "foreign callback mode must be the constant `Reusable` or `OneShot`".to_string(),
            );
            return None;
        };
        if !core.modes.contains(*variant) || !args.is_empty() {
            self.error(
                mode.span,
                "foreign callback mode must be the constant `Reusable` or `OneShot`".to_string(),
            );
            return None;
        }
        let mode = *variant;
        self.check_call_effects(hir::Callable::Function(function), call.span);
        let definition_path = self
            .definition_paths
            .next(scoop_identity::StructuralDefinitionSiteRole::CallbackConversion);
        let registration =
            self.foreign_callback_registrations
                .alloc(hir::ForeignCallbackRegistration {
                    definition_root: self.current_definition_root(),
                    definition_path,
                    native_function_type,
                    managed_function_type,
                    context_index: context_index as u32,
                    mode,
                    span: call.span,
                });
        Some(hir::Expr {
            kind: ExprKind::ForeignCallbackRegister {
                registration,
                closure: Box::new(closure),
            },
            ty: resolved.return_ty,
            span: call.span,
            origin: self.expression_origin(call.span),
        })
    }

    pub(super) fn lower_foreign_callback_call(
        &mut self,
        core: hir::ForeignCallbackCore,
        function: hir::FunctionId,
        operation: hir::ForeignCallbackOperation,
        call: &ast::CallExpr,
        resolved: crate::overload::ResolvedCallee,
    ) -> Option<hir::Expr> {
        let expected_function = match operation {
            hir::ForeignCallbackOperation::Retain => core.retain,
            hir::ForeignCallbackOperation::Release => core.release,
            hir::ForeignCallbackOperation::State => core.query_state,
            hir::ForeignCallbackOperation::Failure => core.failure,
        };
        debug_assert_eq!(function, expected_function);
        let [callback]: [hir::Expr; 1] = resolved
            .args
            .try_into()
            .expect("validated callback operation has one argument");
        let [function_ty] = resolved.type_args.as_slice() else {
            unreachable!("validated callback operation has one concrete type argument")
        };
        if !matches!(self.types[*function_ty], hir::Type::Function(_)) {
            self.error(
                callback.span,
                "`ForeignCallback` type argument must be one concrete function type".to_string(),
            );
            return None;
        }
        self.check_call_effects(hir::Callable::Function(function), call.span);
        Some(hir::Expr {
            kind: ExprKind::ForeignCallbackOperation {
                operation,
                callback: Box::new(callback),
            },
            ty: resolved.return_ty,
            span: call.span,
            origin: self.expression_origin(call.span),
        })
    }
}

fn materialized_source<'expr>(
    expr: &'expr hir::Expr,
    sink: &'expr [hir::Statement],
) -> &'expr hir::Expr {
    let mut current = expr;
    while let ExprKind::Local(local) = current.kind {
        let Some(init) = sink.iter().find_map(|statement| {
            let hir::StatementKind::ValDecl { pattern, init } = &statement.kind else {
                return None;
            };
            matches!(pattern, hir::Pattern::Binding { local: bound } if *bound == local)
                .then_some(init)
        }) else {
            break;
        };
        current = init;
    }
    current
}
