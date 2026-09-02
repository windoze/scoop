use super::*;

impl Lowerer {
    pub(super) fn foreign_callback_operation(
        &self,
        core: hir::ForeignCallbackCore,
        name: &str,
    ) -> Option<hir::ForeignCallbackOperation> {
        match name {
            "foreignCallback" => None,
            "retainForeignCallback" => Some(hir::ForeignCallbackOperation::Retain),
            "releaseForeignCallback" => Some(hir::ForeignCallbackOperation::Release),
            "foreignCallbackState" => Some(hir::ForeignCallbackOperation::State),
            "foreignCallbackFailure" => Some(hir::ForeignCallbackOperation::Failure),
            _ => return None,
        }
        .filter(|operation| {
            let function = match operation {
                hir::ForeignCallbackOperation::Retain => core.retain,
                hir::ForeignCallbackOperation::Release => core.release,
                hir::ForeignCallbackOperation::State => core.query_state,
                hir::ForeignCallbackOperation::Failure => core.failure,
            };
            self.functions[function].name == name
        })
    }

    pub(super) fn lower_foreign_callback_registration(
        &mut self,
        core: hir::ForeignCallbackCore,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        if call.type_args.len() != 1 || call.args.len() != 3 {
            self.error(
                call.span,
                "`foreignCallback` requires one explicit function type and exactly three arguments"
                    .to_string(),
            );
            return None;
        }
        let native_ty = self.resolve_type_ref(&call.type_args[0])?;
        let hir::Type::Function(native_function_type) = self.types[native_ty] else {
            self.error(
                call.type_args[0].span,
                "`foreignCallback` type argument must be an ordinary concrete function type"
                    .to_string(),
            );
            return None;
        };
        if !self.validate_foreign_callback_signature(native_function_type, call.span) {
            return None;
        }
        let native_signature = self.function_types[native_function_type].clone();
        let ast::Expr::IntLiteral {
            value: context_index,
            span: context_span,
        } = &call.args[1]
        else {
            self.error(
                call.args[1].span(),
                "foreign callback `contextIndex` must be a compile-time integer literal"
                    .to_string(),
            );
            return None;
        };
        if *context_index < 0 || *context_index as usize >= native_signature.parameter_types.len() {
            self.error(
                *context_span,
                "foreign callback `contextIndex` is outside the native signature".to_string(),
            );
            return None;
        }
        let context_type = native_signature.parameter_types[*context_index as usize];
        if !matches!(self.types[context_type], hir::Type::Ptr(pointee) if pointee == self.unit) {
            self.error(
                *context_span,
                "foreign callback context parameter must be exactly `Ptr<Unit>`".to_string(),
            );
            return None;
        }

        let managed_parameters = native_signature
            .parameter_types
            .iter()
            .enumerate()
            .filter_map(|(index, ty)| (index != *context_index as usize).then_some(*ty))
            .collect();
        let managed_ty =
            self.intern_function_type(false, managed_parameters, native_signature.return_type);
        let hir::Type::Function(managed_function_type) = self.types[managed_ty] else {
            unreachable!("interned managed callback signature is a function type")
        };
        let closure = self.lower_expr(&call.args[0], sink, Some(managed_ty))?;
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

        let mode_ty = self.interned_enum_type(core.mode);
        let mode = self.lower_expr(&call.args[2], sink, Some(mode_ty))?;
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
        self.check_call_effects(hir::Callable::Function(core.register), call.span);
        let registration =
            self.foreign_callback_registrations
                .alloc(hir::ForeignCallbackRegistration {
                    native_function_type,
                    managed_function_type,
                    context_index: *context_index as u32,
                    mode,
                });
        let ty = self.struct_application(core.callback, vec![native_ty]);
        Some(hir::Expr {
            kind: ExprKind::ForeignCallbackRegister {
                registration,
                closure: Box::new(closure),
            },
            ty,
            span: call.span,
        })
    }

    pub(super) fn lower_foreign_callback_call(
        &mut self,
        core: hir::ForeignCallbackCore,
        operation: hir::ForeignCallbackOperation,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let function = match operation {
            hir::ForeignCallbackOperation::Retain => core.retain,
            hir::ForeignCallbackOperation::Release => core.release,
            hir::ForeignCallbackOperation::State => core.query_state,
            hir::ForeignCallbackOperation::Failure => core.failure,
        };
        if call.args.len() != 1 || call.type_args.len() > 1 {
            self.error(
                call.span,
                format!("`{}` expects one callback value", call.callee.text),
            );
            return None;
        }
        let explicit = if let Some(ty) = call.type_args.first() {
            Some(self.resolve_type_ref(ty)?)
        } else {
            None
        };
        let expected_callback =
            explicit.map(|function| self.struct_application(core.callback, vec![function]));
        let callback = self.lower_expr(&call.args[0], sink, expected_callback)?;
        let hir::Type::Struct(application) = self.types[callback.ty] else {
            self.error(
                callback.span,
                "managed callback token operation requires `ForeignCallback<F>`".to_string(),
            );
            return None;
        };
        let application = self.struct_applications[application].clone();
        if application.template != core.callback || application.arguments.len() != 1 {
            self.error(
                callback.span,
                "managed callback token operation requires `ForeignCallback<F>`".to_string(),
            );
            return None;
        }
        let function_ty = application.arguments[0];
        if !matches!(self.types[function_ty], hir::Type::Function(_))
            || explicit.is_some_and(|explicit| !self.types_equal(explicit, function_ty))
        {
            self.error(
                callback.span,
                "`ForeignCallback` type argument must be one concrete function type".to_string(),
            );
            return None;
        }
        self.check_call_effects(hir::Callable::Function(function), call.span);
        let return_ty = self.instantiate_ty(self.signatures[&function].return_ty, &[function_ty]);
        Some(hir::Expr {
            kind: ExprKind::ForeignCallbackOperation {
                operation,
                callback: Box::new(callback),
            },
            ty: return_ty,
            span: call.span,
        })
    }
}
