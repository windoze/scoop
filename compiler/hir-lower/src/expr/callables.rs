//! Calls, callable references, anonymous functions, lambdas and callbacks.

use super::*;

mod values;

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
        let required = RequiredCallableModifiers {
            operator: Some(hir::OperatorKind::Invoke),
            infix: require_infix,
            ..Default::default()
        };
        if self
            .methods_by_name(ty, "invoke")
            .into_iter()
            .any(|candidate| {
                Self::matches_required_modifiers(
                    self.signatures[&candidate.function].modifiers,
                    required,
                )
            })
        {
            return true;
        }
        // Lookup failures enter ordinary member resolution for their diagnostic.
        if self.resolve_imported_member_receiver_type(ty).is_err()
            || self
                .imported_member_candidates(
                    ty,
                    hir::ImportedMemberLookup::Operator(hir::CallableOperatorRoleV1::Language(
                        crate::imports::lookup::calls::wire_operator(hir::OperatorKind::Invoke),
                    )),
                )
                .map_or(true, |candidates| {
                    use hir::ImportedCallableSource;
                    candidates.iter().any(|candidate| {
                        !require_infix
                            || candidate.interface().effects().infix()
                                == hir::CallableInfixV1::Infix
                    })
                })
        {
            return true;
        }
        self.named_executable_extension_operator_layers(hir::OperatorKind::Invoke)
            .into_iter()
            .flat_map(|layer| layer.candidates)
            .any(|target| self.extension_call_target_matches_required(&target, required))
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
        if call.callee.text.contains('.') {
            let constructor = self.classify_constructor(&call.callee)?;
            return self.lower_nominal_constructor_call(constructor, call, sink, expected);
        }
        self.lower_layered_named_call(call, sink, expected)
    }

    pub(in crate::expr) fn lower_nominal_constructor_call(
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
}
