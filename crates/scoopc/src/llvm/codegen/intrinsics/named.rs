//! Named intrinsic table lowering.

use inkwell::types::BasicMetadataTypeEnum;
use inkwell::types::BasicTypeEnum;
use inkwell::values::BasicMetadataValueEnum;
use inkwell::values::FunctionValue;

use super::super::mir_body::MirLocalSlot;
use super::super::*;
use crate::intrinsics::{
    NamedIntrinsicAuditEntry, NamedIntrinsicLoweringMode, NamedIntrinsicRuntimeTy,
    named_intrinsic_audit_entry,
};
use crate::mir;

#[derive(Clone)]
struct LoweredNamedIntrinsicOperand<'ctx> {
    span: crate::span::Span,
    value: CgValue<'ctx>,
}

struct LoweredNamedIntrinsicCall<'ctx> {
    span: crate::span::Span,
    callee_span: crate::span::Span,
    operands: Vec<LoweredNamedIntrinsicOperand<'ctx>>,
}

type NamedIntrinsicIrEmissionLowerer = for<'a, 'ctx> fn(
    &mut MainCodegen<'a, 'ctx>,
    LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError>;

struct NamedIntrinsicIrRuleEntry {
    name: &'static str,
    lower: NamedIntrinsicIrEmissionLowerer,
}

const NAMED_INTRINSIC_IR_RULES: &[NamedIntrinsicIrRuleEntry] = &[NamedIntrinsicIrRuleEntry {
    name: "dummy_ir",
    lower: lower_dummy_ir,
}];

fn lookup_named_intrinsic_ir_rule(name: &str) -> Option<NamedIntrinsicIrEmissionLowerer> {
    NAMED_INTRINSIC_IR_RULES
        .iter()
        .find(|entry| entry.name == name)
        .map(|entry| entry.lower)
}

fn lower_dummy_ir<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    if call.operands.len() != 1 {
        return Err(LlvmEmitError::UnsupportedMainBody {
            kind: "named intrinsic dummy_ir operand arity",
            at: call.callee_span.into(),
        });
    }
    let word_ty = cg.context.custom_width_int_type(cg.host.word_bit_width());
    let value = word_ty.const_int(41, false);
    Ok(CgValue::int(
        value,
        IntTy {
            bits: cg.host.word_bit_width(),
            signed: true,
        },
    ))
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn try_codegen_named_intrinsic_hir_call(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        callee: &hir::Expr,
        args: &[hir::CallArg],
        entry_name: &str,
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let Some(entry) = named_intrinsic_audit_entry(entry_name) else {
            return Ok(None);
        };
        let call = self.lower_named_intrinsic_hir_call(span, callee_span, callee, args)?;
        self.codegen_named_intrinsic_call(entry, call).map(Some)
    }

    pub(in crate::llvm::codegen) fn try_codegen_named_intrinsic_mir_direct_call(
        &mut self,
        span: crate::span::Span,
        entry_name: &str,
        args: &[mir::CallArg],
        body: &mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let Some(entry) = named_intrinsic_audit_entry(entry_name) else {
            return Ok(None);
        };
        let call = self.lower_named_intrinsic_mir_call(span, args, body, mir_types, slots)?;
        self.codegen_named_intrinsic_call(entry, call).map(Some)
    }

    fn codegen_named_intrinsic_call(
        &mut self,
        entry: &NamedIntrinsicAuditEntry,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match entry.lowering_mode {
            NamedIntrinsicLoweringMode::IrEmission => {
                let lower = lookup_named_intrinsic_ir_rule(entry.name).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "named intrinsic IR rule lookup",
                        at: call.callee_span.into(),
                    },
                )?;
                lower(self, call)
            }
            NamedIntrinsicLoweringMode::RuntimeCall => {
                self.codegen_named_runtime_intrinsic_call(entry, call)
            }
        }
    }

    fn lower_named_intrinsic_hir_call(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        callee: &hir::Expr,
        args: &[hir::CallArg],
    ) -> Result<LoweredNamedIntrinsicCall<'ctx>, LlvmEmitError> {
        let mut operands = Vec::with_capacity(args.len() + 1);
        if let hir::ExprKind::MemberAccess { receiver, .. } = &callee.kind {
            operands.push(self.lower_named_intrinsic_hir_operand(receiver)?);
        }
        for arg in args {
            let value = match arg {
                hir::CallArg::Positional(value) | hir::CallArg::Named { value, .. } => value,
            };
            operands.push(self.lower_named_intrinsic_hir_operand(value)?);
        }
        Ok(LoweredNamedIntrinsicCall {
            span,
            callee_span,
            operands,
        })
    }

    fn lower_named_intrinsic_hir_operand(
        &mut self,
        expr: &hir::Expr,
    ) -> Result<LoweredNamedIntrinsicOperand<'ctx>, LlvmEmitError> {
        let value = self.codegen_expr(expr)?;
        let value = if let Some(cg_ty) = self.resolve_expr_cg_ty(expr) {
            self.coerce_value(expr.span, value, cg_ty)?
        } else {
            value
        };
        Ok(LoweredNamedIntrinsicOperand {
            span: expr.span,
            value,
        })
    }

    fn lower_named_intrinsic_mir_call(
        &mut self,
        span: crate::span::Span,
        args: &[mir::CallArg],
        body: &mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<LoweredNamedIntrinsicCall<'ctx>, LlvmEmitError> {
        let mut operands = Vec::with_capacity(args.len());
        for arg in args {
            operands.push(self.lower_named_intrinsic_mir_operand(arg, body, mir_types, slots)?);
        }
        Ok(LoweredNamedIntrinsicCall {
            span,
            callee_span: span,
            operands,
        })
    }

    fn lower_named_intrinsic_mir_operand(
        &mut self,
        arg: &mir::CallArg,
        body: &mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<LoweredNamedIntrinsicOperand<'ctx>, LlvmEmitError> {
        let source_ty = self.mir_operand_type_id(body, &arg.value).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "named intrinsic MIR operand type",
                at: arg.span.into(),
            },
        )?;
        let operand_cg = self
            .cg_ty_of_mir_type(mir_types, source_ty)
            .or_else(|| {
                self.equivalent_codegen_type_id(mir_types, source_ty)
                    .and_then(|ty| self.cg_ty_of(ty))
            })
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "named intrinsic MIR operand codegen type",
                at: arg.span.into(),
            })?;
        let value =
            self.codegen_mir_operand_expected(arg.span, &arg.value, slots, Some(operand_cg))?;
        let value = self.coerce_value(arg.span, value, operand_cg)?;
        Ok(LoweredNamedIntrinsicOperand {
            span: arg.span,
            value,
        })
    }

    fn codegen_named_runtime_intrinsic_call(
        &mut self,
        entry: &NamedIntrinsicAuditEntry,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let symbol = entry
            .runtime_symbol
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "named runtime intrinsic symbol metadata",
                at: call.callee_span.into(),
            })?;
        let signature = entry
            .runtime_signature
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "named runtime intrinsic signature metadata",
                at: call.callee_span.into(),
            })?;
        let _reason = entry
            .runtime_reason
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "named runtime intrinsic reason metadata",
                at: call.callee_span.into(),
            })?;
        if call.operands.len() != signature.params.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "named runtime intrinsic operand arity",
                at: call.callee_span.into(),
            });
        }

        let runtime = self.declare_named_intrinsic_runtime_symbol(symbol, signature)?;
        let mut llvm_args = Vec::with_capacity(call.operands.len());
        for (operand, &param_ty) in call.operands.iter().zip(signature.params.iter()) {
            llvm_args.push(self.named_intrinsic_runtime_arg(operand, param_ty)?);
        }
        let call_site = self.build_call_preserving_gc_local_roots(
            call.span,
            runtime,
            &llvm_args,
            "named_intrinsic_runtime_call",
        )?;
        self.named_intrinsic_runtime_result(call.span, call_site, signature.return_ty)
    }

    fn declare_named_intrinsic_runtime_symbol(
        &mut self,
        symbol: &str,
        signature: crate::intrinsics::NamedIntrinsicRuntimeSignature,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        if let Some(existing) = self.module.get_function(symbol) {
            return Ok(existing);
        }
        let param_tys = signature
            .params
            .iter()
            .copied()
            .map(|ty| self.named_intrinsic_runtime_metadata_ty(ty))
            .collect::<Result<Vec<_>, _>>()?;
        let fn_ty = match self.named_intrinsic_runtime_basic_ty(signature.return_ty)? {
            Some(ret) => ret.fn_type(&param_tys, false),
            None => self.context.void_type().fn_type(&param_tys, false),
        };
        Ok(self.module.add_function(symbol, fn_ty, None))
    }

    fn named_intrinsic_runtime_metadata_ty(
        &self,
        ty: NamedIntrinsicRuntimeTy,
    ) -> Result<BasicMetadataTypeEnum<'ctx>, LlvmEmitError> {
        Ok(self
            .named_intrinsic_runtime_basic_ty(ty)?
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "named runtime intrinsic void parameter type",
                at: crate::span::Span::new(0, 0).into(),
            })?
            .into())
    }

    fn named_intrinsic_runtime_basic_ty(
        &self,
        ty: NamedIntrinsicRuntimeTy,
    ) -> Result<Option<BasicTypeEnum<'ctx>>, LlvmEmitError> {
        Ok(match ty {
            NamedIntrinsicRuntimeTy::Void => None,
            NamedIntrinsicRuntimeTy::WordInt | NamedIntrinsicRuntimeTy::WordUInt => Some(
                self.context
                    .custom_width_int_type(self.host.word_bit_width())
                    .into(),
            ),
            NamedIntrinsicRuntimeTy::Bool => Some(self.context.bool_type().into()),
            NamedIntrinsicRuntimeTy::GcRef => Some(self.llvm_gc_i8_ptr_type().into()),
            NamedIntrinsicRuntimeTy::RawPtr => Some(self.llvm_i8_ptr_type().into()),
        })
    }

    fn named_intrinsic_runtime_arg(
        &mut self,
        operand: &LoweredNamedIntrinsicOperand<'ctx>,
        target_ty: NamedIntrinsicRuntimeTy,
    ) -> Result<BasicMetadataValueEnum<'ctx>, LlvmEmitError> {
        let value = match target_ty {
            NamedIntrinsicRuntimeTy::Void => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "named runtime intrinsic void operand",
                    at: operand.span.into(),
                });
            }
            NamedIntrinsicRuntimeTy::WordInt | NamedIntrinsicRuntimeTy::WordUInt => {
                let target = CgTy::Int(IntTy {
                    bits: self.host.word_bit_width(),
                    signed: matches!(target_ty, NamedIntrinsicRuntimeTy::WordInt),
                });
                let coerced = self.coerce_value(operand.span, operand.value, target)?;
                coerced.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "named runtime intrinsic word operand value",
                    at: operand.span.into(),
                })?
            }
            NamedIntrinsicRuntimeTy::Bool => {
                let coerced = self.coerce_value(operand.span, operand.value, CgTy::Bool)?;
                coerced.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "named runtime intrinsic bool operand value",
                    at: operand.span.into(),
                })?
            }
            NamedIntrinsicRuntimeTy::GcRef => {
                let coerced = self.coerce_value(operand.span, operand.value, CgTy::Ref)?;
                coerced.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "named runtime intrinsic GC ref operand value",
                    at: operand.span.into(),
                })?
            }
            NamedIntrinsicRuntimeTy::RawPtr => {
                let raw = operand
                    .value
                    .value
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "named runtime intrinsic raw pointer operand value",
                        at: operand.span.into(),
                    })?;
                match raw {
                    inkwell::values::BasicValueEnum::PointerValue(ptr) => self
                        .builder
                        .build_pointer_cast(
                            ptr,
                            self.llvm_i8_ptr_type(),
                            "named_intrinsic_raw_ptr",
                        )?
                        .into(),
                    _ => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "named runtime intrinsic raw pointer operand type",
                            at: operand.span.into(),
                        });
                    }
                }
            }
        };
        Ok(value.into())
    }

    fn named_intrinsic_runtime_result(
        &self,
        span: crate::span::Span,
        call_site: inkwell::values::CallSiteValue<'ctx>,
        result_ty: NamedIntrinsicRuntimeTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match result_ty {
            NamedIntrinsicRuntimeTy::Void => Ok(CgValue::unit()),
            NamedIntrinsicRuntimeTy::WordInt | NamedIntrinsicRuntimeTy::WordUInt => {
                let value = call_site
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "named runtime intrinsic word return value",
                        at: span.into(),
                    })?
                    .into_int_value();
                Ok(CgValue::int(
                    value,
                    IntTy {
                        bits: self.host.word_bit_width(),
                        signed: matches!(result_ty, NamedIntrinsicRuntimeTy::WordInt),
                    },
                ))
            }
            NamedIntrinsicRuntimeTy::Bool => {
                let value = call_site
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "named runtime intrinsic bool return value",
                        at: span.into(),
                    })?
                    .into_int_value();
                Ok(CgValue::bool(value))
            }
            NamedIntrinsicRuntimeTy::GcRef => {
                let value = call_site
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "named runtime intrinsic GC ref return value",
                        at: span.into(),
                    })?
                    .into_pointer_value();
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(value.into()),
                })
            }
            NamedIntrinsicRuntimeTy::RawPtr => {
                let value = call_site
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "named runtime intrinsic raw pointer return value",
                        at: span.into(),
                    })?
                    .into_pointer_value();
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(
                        self.builder
                            .build_pointer_cast(
                                value,
                                self.llvm_gc_i8_ptr_type(),
                                "named_intrinsic_raw_result",
                            )?
                            .into(),
                    ),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intrinsics::named_intrinsic_audit_entries;

    #[test]
    fn ir_rule_table_covers_shared_ir_entries() {
        for entry in named_intrinsic_audit_entries() {
            if entry.lowering_mode != NamedIntrinsicLoweringMode::IrEmission {
                continue;
            }
            assert!(
                lookup_named_intrinsic_ir_rule(entry.name).is_some(),
                "missing IR rule for shared named intrinsic entry {:?}",
                entry.name
            );
        }
    }
}
