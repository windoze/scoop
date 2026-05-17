//! 表达式 codegen（T0102d：从 `codegen/mod.rs` 拆分）。

use super::effect_outcome::{EffectOutcomeTag, ValueTransportParts};
use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(super) fn codegen_expr_in_expected_context(
        &mut self,
        expr: &hir::Expr,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let value = match &expr.kind {
            hir::ExprKind::UnresolvedIdent { name } => {
                self.codegen_unresolved_ident(expr.span, name, expected)
            }
            hir::ExprKind::Closure(closure) => {
                self.codegen_closure_expr(expr.span, closure, expr.ty)
            }
            hir::ExprKind::Call { callee, args } => {
                self.codegen_call(expr.span, callee, args, expected, Some(expr.ty))
            }
            hir::ExprKind::Perform {
                effect_ty,
                op,
                args,
            } => self.codegen_perform_expr(
                expr.span,
                *effect_ty,
                op,
                args,
                expected.or_else(|| self.cg_ty_of(expr.ty)),
            ),
            hir::ExprKind::Handle(handle) => self.codegen_handle_expr(expr.span, handle, expected),
            hir::ExprKind::Block(block) => {
                self.codegen_block_value_in_expected_context(block, expected)
            }
            hir::ExprKind::When { subject, arms } => {
                self.codegen_when_expr(expr.span, subject, arms, expected)
            }
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.codegen_if_expr(
                expr.span,
                expr.ty,
                cond,
                then_branch,
                else_branch.as_deref(),
                expected,
            ),
            _ => self.codegen_expr(expr),
        }?;

        if let Some(target) = expected {
            if target == CgTy::Unit {
                if value.ty == CgTy::Never {
                    Ok(value)
                } else {
                    Ok(CgValue::unit())
                }
            } else {
                self.coerce_value(expr.span, value, target)
            }
        } else {
            Ok(value)
        }
    }

    pub(super) fn codegen_expr(
        &mut self,
        expr: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match &expr.kind {
            hir::ExprKind::Missing | hir::ExprKind::Todo(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "expression",
                    at: expr.span.into(),
                })
            }
            hir::ExprKind::UnresolvedIdent { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unresolved ident (missing expected type context)",
                at: expr.span.into(),
            }),
            hir::ExprKind::Literal(lit) => self.codegen_literal(expr.span, expr.ty, lit),
            hir::ExprKind::ClassLiteral(class_lit) => match class_lit.metadata_kind {
                hir::TypeMetadataLiteralKind::TypeNameString => {
                    let type_name = class_lit
                        .source_fqn
                        .clone()
                        .unwrap_or_else(|| self.types.display(class_lit.source_ty).to_string());
                    self.codegen_string_literal_from_text(expr.span, &type_name)
                }
            },
            hir::ExprKind::VarRef(v) => self.codegen_var_ref(expr.span, v),
            hir::ExprKind::StructLit { ty, fields } => {
                self.codegen_struct_lit(expr.span, *ty, fields)
            }
            hir::ExprKind::TupleLit { elements } => {
                self.codegen_tuple_lit(expr.span, expr.ty, elements)
            }
            hir::ExprKind::InterpolatedString { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "interpolated string after HIR desugar",
                at: expr.span.into(),
            }),
            hir::ExprKind::Unary {
                op, expr: inner, ..
            } => self.codegen_unary(expr.span, expr.ty, *op, inner),
            hir::ExprKind::Binary { lhs, op, rhs, .. } => {
                self.codegen_binary(expr.span, *op, lhs, rhs)
            }
            hir::ExprKind::TypeCheck {
                expr: inner,
                op,
                target_ty,
                ..
            } => self.codegen_type_check_expr(expr.span, *op, inner, *target_ty),
            hir::ExprKind::Cast {
                expr: inner,
                op,
                target_ty,
                ..
            } => self.codegen_cast_expr(expr.span, *op, inner, *target_ty, expr.ty),
            hir::ExprKind::Block(block) => self.codegen_block_value(block),
            hir::ExprKind::Call { callee, args } => {
                self.codegen_call(expr.span, callee, args, None, Some(expr.ty))
            }
            hir::ExprKind::MemberAccess { receiver, member } => {
                self.codegen_member_access(expr.span, receiver, member)
            }
            hir::ExprKind::When { subject, arms } => {
                self.codegen_when_expr(expr.span, subject, arms, self.cg_ty_of(expr.ty))
            }
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.codegen_if_expr(
                expr.span,
                expr.ty,
                cond,
                then_branch,
                else_branch.as_deref(),
                None,
            ),

            // 后续任务接入 MIR/CFG codegen
            hir::ExprKind::Closure(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "expression kind",
                at: expr.span.into(),
            }),
            hir::ExprKind::Perform {
                effect_ty,
                op,
                args,
            } => self.codegen_perform_expr(expr.span, *effect_ty, op, args, self.cg_ty_of(expr.ty)),
            hir::ExprKind::Handle(handle) => {
                // T1611: infer expected type from HIR when not in expected context,
                // so statement-position handles don't need `val _: Unit = ...` workaround.
                let inferred = self.cg_ty_of(expr.ty);
                self.codegen_handle_expr(expr.span, handle, inferred)
            }
        }
    }

    fn codegen_perform_expr(
        &mut self,
        span: crate::span::Span,
        effect_ty: TypeId,
        op: &hir::EffectOpRef,
        args: &[hir::CallArg],
        _expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let outcome_ptr = self.function_cx.current_effect_outcome_ptr.ok_or_else(|| {
            LlvmEmitError::Frontend {
                message: format!(
                    "direct HIR perform `{}`（effect `{}`）缺少当前 explicit EffectOutcome 槽位；该路径应由 published late-lowered/local-effect-control handoff 接管",
                    op.fqn,
                    self.types.display(effect_ty),
                ),
            }
        })?;

        let Some(payload) = self.codegen_hir_perform_payload_transport(span, args)? else {
            return Ok(CgValue::never());
        };

        let effect_instance_key =
            self.effect_instance_key(effect_ty)
                .ok_or_else(|| LlvmEmitError::Frontend {
                    message: format!(
                        "direct HIR perform `{}`（effect `{}`）缺少可发布的 effect_instance_key",
                        op.fqn,
                        self.types.display(effect_ty),
                    ),
                })?;
        let op_tag = self.effect_op_tag(&op.fqn);
        let signal = self.build_effect_signal(
            self.context.i32_type().const_int(u64::from(op_tag), false),
            self.context
                .i32_type()
                .const_int(u64::from(effect_instance_key), false),
            payload,
            self.llvm_gc_i8_ptr_type().const_null(),
        )?;
        let outcome = self.build_effect_outcome(
            EffectOutcomeTag::Propagate,
            self.zero_value_transport_parts(),
            signal,
        )?;
        self.builder.build_store(outcome_ptr, outcome)?;

        if !self.ordinary_effect_propagation_enabled()
            && self.current_local_effect_escape_target().is_none()
        {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "direct HIR perform `{}`（effect `{}`）命中了 suppressed ordinary propagation，但当前 callable 未安装 local effect escape target",
                    op.fqn,
                    self.types.display(effect_ty),
                ),
            });
        }

        self.emit_ordinary_non_resuming_effect_exit(span, "hir_perform_effect")?;
        Ok(CgValue::never())
    }

    fn codegen_handle_expr(
        &mut self,
        _span: crate::span::Span,
        handle: &hir::HandleExpr,
        _expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let callable = self
            .function_cx
            .current_callable_fqn
            .as_deref()
            .unwrap_or("<unknown>");
        Err(LlvmEmitError::Frontend {
            message: format!(
                "LLVM HIR handle 入口已停用；callable `{callable}` 仍命中 direct HIR handle（arms={}, finally={}），应先经 published late-lowered/local-effect-control handoff",
                handle.arms.len(),
                handle.finally.is_some(),
            ),
        })
    }

    fn codegen_hir_perform_payload_transport(
        &mut self,
        span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<Option<ValueTransportParts<'ctx>>, LlvmEmitError> {
        let mut values: Vec<(TypeId, CgValue<'ctx>)> = Vec::with_capacity(args.len());
        for arg in args {
            let expr = match arg {
                hir::CallArg::Positional(expr) => expr,
                hir::CallArg::Named { value, .. } => value,
            };
            let expected = self.cg_ty_of(expr.ty);
            let value = match &expr.kind {
                hir::ExprKind::Closure(closure) => {
                    self.codegen_closure_expr(expr.span, closure, expr.ty)?
                }
                _ => self.codegen_expr_in_expected_context(expr, expected)?,
            };
            if value.ty == CgTy::Never {
                return Ok(None);
            }
            let value = if let Some(target) = expected {
                self.coerce_value(expr.span, value, target)?
            } else {
                value
            };
            values.push((expr.ty, value));
        }

        match values.as_slice() {
            [] => Ok(Some(self.zero_value_transport_parts())),
            [(source_ty, single)] => self
                .encode_effect_transport_value(
                    span,
                    Some(*source_ty),
                    *single,
                    "hir_perform_payload",
                )
                .map(Some),
            many => {
                let call_site = self.current_call_site(span)?;
                let payload_tuple_ty = self
                    .effect_op_call_sites
                    .get(&call_site)
                    .and_then(|info| info.payload_tuple_ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "HIR perform payload tuple contract",
                        at: span.into(),
                    })?;
                let tuple = self.build_tuple_payload_value(span, payload_tuple_ty, many)?;
                if tuple.ty == CgTy::Never {
                    return Ok(None);
                }
                self.encode_effect_transport_value(
                    span,
                    Some(payload_tuple_ty),
                    tuple,
                    "hir_perform_payload",
                )
                .map(Some)
            }
        }
    }

    fn build_tuple_payload_value(
        &mut self,
        span: crate::span::Span,
        tuple_ty: TypeId,
        values: &[(TypeId, CgValue<'ctx>)],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let Some(CgTy::Tuple(tuple_cg)) = self.cg_ty_of(tuple_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "perform payload tuple type",
                at: span.into(),
            });
        };
        let llvm_tuple_ty = self.llvm_tuple_type(span, tuple_cg)?;
        let mut agg = llvm_tuple_ty.get_undef();
        for (idx, (_source_ty, value)) in values.iter().enumerate() {
            if value.ty == CgTy::Never {
                return Ok(CgValue::never());
            }
            let raw = match value.ty {
                CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
                _ => value.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "perform payload tuple value",
                    at: span.into(),
                })?,
            };
            agg = self
                .builder
                .build_insert_value(agg, raw, idx as u32, &format!("perform_payload_{idx}"))?
                .into_struct_value();
        }
        Ok(CgValue {
            ty: CgTy::Tuple(tuple_cg),
            value: Some(agg.into()),
        })
    }
}
