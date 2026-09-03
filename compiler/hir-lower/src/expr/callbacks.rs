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
        explicit_type_args: &[TypeId],
        args: &[ast::CallArgument],
        span: Span,
    ) -> Result<Option<TypeId>, ()> {
        let Some(core) = self.foreign_callback_core else {
            return Ok(None);
        };
        if function != core.register {
            return Ok(None);
        }
        let [native_ty] = explicit_type_args else {
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
        let [_, context_index, _] = args else {
            unreachable!("registration candidate shape requires exactly three arguments")
        };
        let ast::Expr::IntLiteral {
            value: context_index,
            span: context_span,
        } = &context_index.expression
        else {
            self.error(
                context_index.span,
                "foreign callback `contextIndex` must be a compile-time integer literal"
                    .to_string(),
            );
            return Err(());
        };
        let native_signature = self.function_types[native_function_type].clone();
        if *context_index < 0 || *context_index as usize >= native_signature.parameter_types.len() {
            self.error(
                *context_span,
                "foreign callback `contextIndex` is outside the native signature".to_string(),
            );
            return Err(());
        }
        let context_type = native_signature.parameter_types[*context_index as usize];
        if !matches!(self.types[context_type], hir::Type::Ptr(pointee) if pointee == self.unit) {
            self.error(
                *context_span,
                "foreign callback context parameter must be exactly `Ptr<Unit>`".to_string(),
            );
            return Err(());
        }

        let managed_parameters = native_signature
            .parameter_types
            .iter()
            .enumerate()
            .filter_map(|(index, ty)| (index != *context_index as usize).then_some(*ty))
            .collect();
        Ok(Some(self.intern_function_type(
            false,
            managed_parameters,
            native_signature.return_type,
        )))
    }

    pub(super) fn lower_foreign_callback_registration(
        &mut self,
        core: hir::ForeignCallbackCore,
        function: hir::FunctionId,
        call: &ast::CallExpr,
        resolved: crate::overload::ResolvedCallee,
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
        let ExprKind::IntLiteral(context_index) = context_index.kind else {
            unreachable!("registration candidate validation requires a literal context index")
        };

        let managed_parameters = native_signature
            .parameter_types
            .iter()
            .enumerate()
            .filter_map(|(index, ty)| (index != context_index as usize).then_some(*ty))
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

        let ExprKind::VariantConstruct {
            application,
            variant,
            args,
        } = mode.kind
        else {
            self.error(
                mode.span,
                "foreign callback mode must be the constant `Reusable` or `OneShot`".to_string(),
            );
            return None;
        };
        if self.enum_applications[application].template != core.mode
            || !args.is_empty()
            || variant > 1
        {
            self.error(
                mode.span,
                "foreign callback mode must be the constant `Reusable` or `OneShot`".to_string(),
            );
            return None;
        }
        let mode = if variant == 0 {
            hir::ForeignCallbackMode::Reusable
        } else {
            hir::ForeignCallbackMode::OneShot
        };
        self.check_call_effects(hir::Callable::Function(function), call.span);
        let registration =
            self.foreign_callback_registrations
                .alloc(hir::ForeignCallbackRegistration {
                    native_function_type,
                    managed_function_type,
                    context_index: context_index as u32,
                    mode,
                });
        Some(hir::Expr {
            kind: ExprKind::ForeignCallbackRegister {
                registration,
                closure: Box::new(closure),
            },
            ty: resolved.return_ty,
            span: call.span,
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
        })
    }
}
