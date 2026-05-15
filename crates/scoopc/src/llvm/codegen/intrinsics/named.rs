//! Named intrinsic table lowering.

use inkwell::AddressSpace;
use inkwell::IntPredicate;
use inkwell::types::BasicMetadataTypeEnum;
use inkwell::types::BasicTypeEnum;
use inkwell::values::BasicMetadataValueEnum;
use inkwell::values::FunctionValue;
use inkwell::values::PointerValue;

use super::super::mir_body::MirLocalSlot;
use super::super::*;
use crate::intrinsics::{
    NamedIntrinsicAuditEntry, NamedIntrinsicLoweringMode, NamedIntrinsicRuntimeTy,
    named_intrinsic_audit_entry,
};
use crate::mir;
use crate::ty::{RefTypeKind, TypeId, TypeKind};

#[derive(Clone)]
struct LoweredNamedIntrinsicOperand<'ctx> {
    span: crate::span::Span,
    source_ty: Option<TypeId>,
    value: CgValue<'ctx>,
}

struct LoweredNamedIntrinsicCall<'ctx> {
    span: crate::span::Span,
    callee_span: crate::span::Span,
    operands: Vec<LoweredNamedIntrinsicOperand<'ctx>>,
    array_element_source_ty: Option<TypeId>,
}

type NamedIntrinsicIrEmissionLowerer = for<'a, 'ctx> fn(
    &mut MainCodegen<'a, 'ctx>,
    LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError>;

struct NamedIntrinsicIrRuleEntry {
    name: &'static str,
    lower: NamedIntrinsicIrEmissionLowerer,
}

const NAMED_INTRINSIC_IR_RULES: &[NamedIntrinsicIrRuleEntry] = &[
    NamedIntrinsicIrRuleEntry {
        name: "dummy_ir",
        lower: lower_dummy_ir,
    },
    NamedIntrinsicIrRuleEntry {
        name: "array_size",
        lower: lower_array_size,
    },
    NamedIntrinsicIrRuleEntry {
        name: "array_get",
        lower: lower_array_get,
    },
    NamedIntrinsicIrRuleEntry {
        name: "array_set",
        lower: lower_array_set,
    },
    NamedIntrinsicIrRuleEntry {
        name: "array_data_ptr",
        lower: lower_array_data_ptr,
    },
];

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

fn lower_array_size<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_array_size(call)
}

fn lower_array_get<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_array_get(call)
}

fn lower_array_set<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_array_set(call)
}

fn lower_array_data_ptr<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    call: LoweredNamedIntrinsicCall<'ctx>,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    cg.codegen_named_intrinsic_array_data_ptr(call)
}

fn normalize_array_like_fqn(fqn: &str) -> Option<&'static str> {
    match fqn {
        "scoop.core.Array"
        | "scoop.core.List"
        | "scoop.collections.Set"
        | "scoop.collections.MapView" => Some("scoop.core.Array"),
        "scoop.core.MutableArray"
        | "scoop.core.MutableList"
        | "scoop.collections.MutableSet"
        | "scoop.collections.MutableMap" => Some("scoop.core.MutableArray"),
        _ => None,
    }
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

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn try_codegen_named_intrinsic_mir_direct_call(
        &mut self,
        span: crate::span::Span,
        entry_name: &str,
        args: &[mir::CallArg],
        body: &mir::Body,
        mir_types: &TypeStore,
        array_transport: Option<&mir::ArrayElementTransportMetadata>,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let Some(entry) = named_intrinsic_audit_entry(entry_name) else {
            return Ok(None);
        };
        let call = self.lower_named_intrinsic_mir_call(
            span,
            args,
            body,
            mir_types,
            array_transport,
            slots,
        )?;
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
            array_element_source_ty: None,
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
            source_ty: Some(expr.ty),
            value,
        })
    }

    fn lower_named_intrinsic_mir_call(
        &mut self,
        span: crate::span::Span,
        args: &[mir::CallArg],
        body: &mir::Body,
        mir_types: &TypeStore,
        array_transport: Option<&mir::ArrayElementTransportMetadata>,
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
            array_element_source_ty: array_transport
                .and_then(|metadata| {
                    self.equivalent_codegen_type_id(mir_types, metadata.element_ty)
                })
                .or_else(|| {
                    array_transport.and_then(|metadata| {
                        self.equivalent_codegen_type_id(mir_types, metadata.element.source_ty)
                    })
                }),
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
            source_ty: self.equivalent_codegen_type_id(mir_types, source_ty),
            value,
        })
    }

    fn codegen_named_intrinsic_array_size(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if call.operands.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "named intrinsic array_size operand arity",
                at: call.callee_span.into(),
            });
        }
        let receiver = &call.operands[0];
        let arr_ptr = self.named_intrinsic_array_receiver_ptr(receiver, "array_size receiver")?;
        let len_i64 = self.named_intrinsic_array_len_value(call.span, arr_ptr)?;
        let len_word = self.cast_int(
            len_i64,
            IntTy {
                bits: 64,
                signed: false,
            },
            IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            },
        )?;
        Ok(CgValue::int(
            len_word,
            IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            },
        ))
    }

    fn codegen_named_intrinsic_array_get(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if call.operands.len() != 2 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "named intrinsic array_get operand arity",
                at: call.callee_span.into(),
            });
        }
        let receiver = &call.operands[0];
        let index = &call.operands[1];
        let arr_ptr = self.named_intrinsic_array_receiver_ptr(receiver, "array_get receiver")?;
        let index_i64 = self.named_intrinsic_array_index_i64(index, "array_get index")?;
        let (_elem_ty, elem_cg) = self.named_intrinsic_array_element_cg_ty(
            call.callee_span,
            receiver,
            call.array_element_source_ty,
            "array_get element type",
        )?;
        if elem_cg == CgTy::Unit {
            return Ok(CgValue::unit());
        }

        let len_i64 = self.named_intrinsic_array_len_value(call.span, arr_ptr)?;
        let current_fn = self
            .builder
            .get_insert_block()
            .and_then(|block| block.get_parent())
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "named intrinsic array_get parent function",
                at: call.callee_span.into(),
            })?;
        let in_bounds_bb = self
            .context
            .append_basic_block(current_fn, "array_get_in_bounds");
        let out_of_bounds_bb = self
            .context
            .append_basic_block(current_fn, "array_get_out_of_bounds");
        let not_negative_bb = self
            .context
            .append_basic_block(current_fn, "array_get_not_negative");
        let merge_bb = self
            .context
            .append_basic_block(current_fn, "array_get_merge");

        let is_negative = self.builder.build_int_compare(
            IntPredicate::SLT,
            index_i64,
            self.context.i64_type().const_zero(),
            "array_get_negative",
        )?;
        self.builder
            .build_conditional_branch(is_negative, out_of_bounds_bb, not_negative_bb)?;

        self.builder.position_at_end(not_negative_bb);
        let is_ge_len = self.builder.build_int_compare(
            IntPredicate::SGE,
            index_i64,
            len_i64,
            "array_get_ge_len",
        )?;
        self.builder
            .build_conditional_branch(is_ge_len, out_of_bounds_bb, in_bounds_bb)?;

        let llvm_elem_ty = self.llvm_basic_type_of(call.span, elem_cg)?;
        let oob_value = match elem_cg {
            CgTy::Ref => self.llvm_gc_i8_ptr_type().const_null().into(),
            CgTy::String => self.llvm_scoop_string_ptr_type().const_null().into(),
            _ => llvm_elem_ty.const_zero(),
        };

        self.builder.position_at_end(out_of_bounds_bb);
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(in_bounds_bb);
        let slot_ptr =
            self.named_intrinsic_array_slot_ptr(call.span, arr_ptr, receiver, elem_cg, index_i64)?;
        let loaded = self
            .builder
            .build_load(llvm_elem_ty, slot_ptr, "array_get_load")?;
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(merge_bb);
        let phi = self.builder.build_phi(llvm_elem_ty, "array_get_result")?;
        phi.add_incoming(&[(&oob_value, out_of_bounds_bb), (&loaded, in_bounds_bb)]);
        let loaded = phi.as_basic_value();
        match elem_cg {
            CgTy::String => {
                let ptr = loaded.into_pointer_value();
                let str_ptr = self.builder.build_pointer_cast(
                    ptr,
                    self.llvm_scoop_string_ptr_type(),
                    "array_get_string",
                )?;
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(str_ptr.into()),
                })
            }
            _ => self.cg_value_from_loaded(call.span, elem_cg, loaded),
        }
    }

    fn codegen_named_intrinsic_array_set(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if call.operands.len() != 3 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "named intrinsic array_set operand arity",
                at: call.callee_span.into(),
            });
        }
        let receiver = &call.operands[0];
        let index = &call.operands[1];
        let value_operand = &call.operands[2];
        let arr_ptr = self.named_intrinsic_array_receiver_ptr(receiver, "array_set receiver")?;
        let index_i64 = self.named_intrinsic_array_index_i64(index, "array_set index")?;
        let (elem_ty, elem_cg) = self.named_intrinsic_array_element_cg_ty(
            call.callee_span,
            receiver,
            call.array_element_source_ty,
            "array_set element type",
        )?;
        let len_i64 = self.named_intrinsic_array_len_value(call.span, arr_ptr)?;
        let current_fn = self
            .builder
            .get_insert_block()
            .and_then(|block| block.get_parent())
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "named intrinsic array_set parent function",
                at: call.callee_span.into(),
            })?;
        let in_bounds_bb = self
            .context
            .append_basic_block(current_fn, "array_set_in_bounds");
        let out_of_bounds_bb = self
            .context
            .append_basic_block(current_fn, "array_set_out_of_bounds");
        let not_negative_bb = self
            .context
            .append_basic_block(current_fn, "array_set_not_negative");
        let merge_bb = self
            .context
            .append_basic_block(current_fn, "array_set_merge");

        let is_negative = self.builder.build_int_compare(
            IntPredicate::SLT,
            index_i64,
            self.context.i64_type().const_zero(),
            "array_set_negative",
        )?;
        self.builder
            .build_conditional_branch(is_negative, out_of_bounds_bb, not_negative_bb)?;

        self.builder.position_at_end(not_negative_bb);
        let is_ge_len = self.builder.build_int_compare(
            IntPredicate::SGE,
            index_i64,
            len_i64,
            "array_set_ge_len",
        )?;
        self.builder
            .build_conditional_branch(is_ge_len, out_of_bounds_bb, in_bounds_bb)?;

        self.builder.position_at_end(out_of_bounds_bb);
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(in_bounds_bb);
        if elem_cg != CgTy::Unit {
            let slot_ptr = self
                .named_intrinsic_array_slot_ptr(call.span, arr_ptr, receiver, elem_cg, index_i64)?;
            if matches!(elem_cg, CgTy::Ref | CgTy::String) {
                let value = self.coerce_value(value_operand.span, value_operand.value, elem_cg)?;
                let value = self.coerce_value(value_operand.span, value, CgTy::Ref)?;
                let Some(BasicValueEnum::PointerValue(value_ptr)) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "named intrinsic array_set ref value",
                        at: value_operand.span.into(),
                    });
                };
                self.store_gc_pointer_slot_with_write_barrier(call.span, slot_ptr, value_ptr)?;
            } else if matches!(elem_cg, CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_)) {
                let value = self.coerce_value(value_operand.span, value_operand.value, elem_cg)?;
                let value_ptr = self.named_intrinsic_materialize_value_ptr(
                    value_operand.span,
                    "array_set_composite_value",
                    elem_cg,
                    value,
                )?;
                let descriptor =
                    self.named_intrinsic_array_composite_descriptor(value_operand.span, elem_ty)?;
                let slot_i8_gc = self.named_intrinsic_array_slot_i8_ptr(
                    call.span, arr_ptr, receiver, elem_cg, index_i64,
                )?;
                let slot_i8 = self.builder.build_address_space_cast(
                    slot_i8_gc,
                    self.llvm_i8_ptr_type(),
                    "array_set_composite_dst",
                )?;
                let src_i8 = self.builder.build_pointer_cast(
                    value_ptr,
                    self.llvm_i8_ptr_type(),
                    "array_set_composite_src",
                )?;
                let drop = self.declare_runtime_composite_drop();
                let copy = self.declare_runtime_composite_copy();
                let _ = self.build_call_preserving_gc_local_roots(
                    value_operand.span,
                    drop,
                    &[descriptor.into(), slot_i8.into()],
                    "array_set_composite_drop",
                )?;
                let _ = self.build_call_preserving_gc_local_roots(
                    value_operand.span,
                    copy,
                    &[descriptor.into(), slot_i8.into(), src_i8.into()],
                    "array_set_composite_copy",
                )?;
                for offset in self
                    .named_intrinsic_array_composite_gc_slot_offsets(value_operand.span, elem_cg)?
                {
                    let slot_gc = unsafe {
                        self.builder.build_in_bounds_gep(
                            self.context.i8_type(),
                            slot_i8_gc,
                            &[self.context.i64_type().const_int(offset, false)],
                            "array_set_composite_gc_slot_i8",
                        )?
                    };
                    let slot_gc_ptr = self.builder.build_pointer_cast(
                        slot_gc,
                        self.llvm_ptr_type(self.gc_address_space()),
                        "array_set_composite_gc_slot_ptr",
                    )?;
                    let loaded = self
                        .builder
                        .build_load(
                            self.llvm_gc_i8_ptr_type(),
                            slot_gc_ptr,
                            "array_set_composite_gc_slot_load",
                        )?
                        .into_pointer_value();
                    self.store_gc_pointer_slot_with_write_barrier(
                        value_operand.span,
                        slot_gc_ptr,
                        loaded,
                    )?;
                }
            } else {
                let value = self.coerce_value(value_operand.span, value_operand.value, elem_cg)?;
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "named intrinsic array_set scalar value",
                        at: value_operand.span.into(),
                    });
                };
                let _ = self.builder.build_store(slot_ptr, raw)?;
            }
        }
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(merge_bb);
        Ok(CgValue::unit())
    }

    fn codegen_named_intrinsic_array_data_ptr(
        &mut self,
        call: LoweredNamedIntrinsicCall<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if call.operands.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "named intrinsic array_data_ptr operand arity",
                at: call.callee_span.into(),
            });
        }
        let receiver = &call.operands[0];
        let arr_ptr =
            self.named_intrinsic_array_receiver_ptr(receiver, "array_data_ptr receiver")?;
        let data_ptr_gc = self.named_intrinsic_array_data_base_ptr(call.span, arr_ptr)?;
        let data_ptr = self.builder.build_address_space_cast(
            data_ptr_gc,
            self.llvm_i8_ptr_type(),
            "array_data_ptr_native",
        )?;
        let ptr_int_ty = self.llvm_ptr_sized_int_type(Some(AddressSpace::default()));
        let raw = self
            .builder
            .build_ptr_to_int(data_ptr, ptr_int_ty, "array_data_ptr_word")?;
        Ok(CgValue::int(
            raw,
            IntTy {
                bits: self.host.word_bit_width(),
                signed: false,
            },
        ))
    }

    fn named_intrinsic_array_receiver_ptr(
        &mut self,
        operand: &LoweredNamedIntrinsicOperand<'ctx>,
        kind: &'static str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let value = self.coerce_value(operand.span, operand.value, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(ptr)) = value.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: operand.span.into(),
            });
        };
        Ok(ptr)
    }

    fn named_intrinsic_array_index_i64(
        &mut self,
        operand: &LoweredNamedIntrinsicOperand<'ctx>,
        kind: &'static str,
    ) -> Result<inkwell::values::IntValue<'ctx>, LlvmEmitError> {
        let value = self.coerce_value(
            operand.span,
            operand.value,
            CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            }),
        )?;
        let Some((raw, from)) = value.as_int() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: operand.span.into(),
            });
        };
        self.cast_int(
            raw,
            from,
            IntTy {
                bits: 64,
                signed: true,
            },
        )
    }

    fn named_intrinsic_array_element_cg_ty(
        &self,
        span: crate::span::Span,
        receiver: &LoweredNamedIntrinsicOperand<'ctx>,
        fallback_elem_ty: Option<TypeId>,
        kind: &'static str,
    ) -> Result<(TypeId, CgTy), LlvmEmitError> {
        let elem_ty = receiver
            .source_ty
            .and_then(|receiver_ty| match self.types.kind(receiver_ty) {
                TypeKind::Ref(RefTypeKind::Nominal(nominal))
                    if normalize_array_like_fqn(nominal.fqn.as_str()).is_some() =>
                {
                    nominal.args.first().copied()
                }
                _ => None,
            })
            .or(fallback_elem_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: span.into(),
            })?;
        let elem_cg = self
            .cg_ty_of(elem_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: span.into(),
            })?;
        Ok((elem_ty, elem_cg))
    }

    fn named_intrinsic_array_len_value(
        &mut self,
        _span: crate::span::Span,
        arr_ptr: PointerValue<'ctx>,
    ) -> Result<inkwell::values::IntValue<'ctx>, LlvmEmitError> {
        let array_ty = self.llvm_scoop_array_type();
        let len_ptr = self
            .builder
            .build_struct_gep(array_ty, arr_ptr, 1, "array_len_gep")?;
        Ok(self
            .builder
            .build_load(self.context.i64_type(), len_ptr, "array_len")?
            .into_int_value())
    }

    fn named_intrinsic_array_data_base_ptr(
        &mut self,
        _span: crate::span::Span,
        arr_ptr: PointerValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let array_ty = self.llvm_scoop_array_type();
        let data_offset_ptr =
            self.builder
                .build_struct_gep(array_ty, arr_ptr, 3, "array_data_offset_gep")?;
        let data_offset = self
            .builder
            .build_load(
                self.context.i64_type(),
                data_offset_ptr,
                "array_data_offset",
            )?
            .into_int_value();
        let array_i8_gc =
            self.builder
                .build_pointer_cast(arr_ptr, self.llvm_gc_i8_ptr_type(), "array_i8_gc")?;
        Ok(unsafe {
            self.builder.build_in_bounds_gep(
                self.context.i8_type(),
                array_i8_gc,
                &[data_offset],
                "array_data_base_gc",
            )?
        })
    }

    fn named_intrinsic_array_stride_bytes(
        &mut self,
        span: crate::span::Span,
        elem_cg: CgTy,
    ) -> Result<u64, LlvmEmitError> {
        match elem_cg {
            CgTy::Ref | CgTy::String => Ok(self.target_layout().pointer_size.max(1)),
            CgTy::Unit | CgTy::Bool | CgTy::Float64 | CgTy::Float32 | CgTy::Int(_) => {
                Ok(self.target_layout().pointer_size.max(1))
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                let llvm_ty = self.llvm_basic_type_of(span, elem_cg)?;
                Ok(self.store_size_bytes_of_basic_type(llvm_ty))
            }
            CgTy::Never => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "named intrinsic array element stride",
                at: span.into(),
            }),
        }
    }

    fn named_intrinsic_array_slot_i8_ptr(
        &mut self,
        span: crate::span::Span,
        arr_ptr: PointerValue<'ctx>,
        receiver: &LoweredNamedIntrinsicOperand<'ctx>,
        elem_cg: CgTy,
        index_i64: inkwell::values::IntValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let data_base = self.named_intrinsic_array_data_base_ptr(span, arr_ptr)?;
        let stride = self.named_intrinsic_array_stride_bytes(span, elem_cg)?;
        let byte_offset = if stride == 1 {
            index_i64
        } else {
            self.builder.build_int_mul(
                index_i64,
                self.context.i64_type().const_int(stride, false),
                "array_elem_byte_offset",
            )?
        };
        let _ = receiver;
        Ok(unsafe {
            self.builder.build_in_bounds_gep(
                self.context.i8_type(),
                data_base,
                &[byte_offset],
                "array_elem_i8_gc",
            )?
        })
    }

    fn named_intrinsic_array_slot_ptr(
        &mut self,
        span: crate::span::Span,
        arr_ptr: PointerValue<'ctx>,
        receiver: &LoweredNamedIntrinsicOperand<'ctx>,
        elem_cg: CgTy,
        index_i64: inkwell::values::IntValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let slot_i8 =
            self.named_intrinsic_array_slot_i8_ptr(span, arr_ptr, receiver, elem_cg, index_i64)?;
        Ok(self.builder.build_pointer_cast(
            slot_i8,
            self.llvm_ptr_type(self.gc_address_space()),
            "array_elem_ptr_gc",
        )?)
    }

    fn named_intrinsic_materialize_value_ptr(
        &mut self,
        span: crate::span::Span,
        name: &str,
        cg_ty: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let slot = self.create_entry_alloca(span, name, cg_ty)?;
        let _ = self.store_local_value(span, slot, cg_ty, value)?;
        Ok(slot)
    }

    fn named_intrinsic_array_composite_descriptor(
        &mut self,
        span: crate::span::Span,
        elem_ty: TypeId,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let body_fqn = self
            .function_cx
            .current_callable_fqn
            .clone()
            .unwrap_or_else(|| "<named-intrinsic-array>".to_string());
        let metadata =
            mir::ValueTransportMetadata::plain(elem_ty, mir::MirTransportKind::ArrayElement);
        let descriptor = self.get_or_create_value_composite_transport_descriptor_global(
            &body_fqn, span, self.types, &metadata,
        )?;
        Ok(descriptor.as_pointer_value())
    }

    fn named_intrinsic_array_composite_gc_slot_offsets(
        &mut self,
        span: crate::span::Span,
        elem_cg: CgTy,
    ) -> Result<Vec<u64>, LlvmEmitError> {
        let llvm_ty = self.llvm_basic_type_of(span, elem_cg)?;
        let mut offsets = Vec::new();
        self.collect_gc_ptr_offsets_in_basic_type(span, llvm_ty, 0, &mut offsets)?;
        offsets.sort_unstable();
        offsets.dedup();
        Ok(offsets)
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
        // P4-T01j：named intrinsic runtime symbol 也属于 runtime/native import surface，
        // 必须经过 [`declare_runtime_or_native_import_function`] 的分类入口完成 surface
        // assertions（External linkage、未来一旦改写不会绕开 classification 检查）；
        // wrapper 内部已包含 "已存在则复用" 语义，所以这里不再重复 `module.get_function` early-return。
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
        Ok(self.declare_runtime_or_native_import_function(symbol, fn_ty))
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
            NamedIntrinsicRuntimeTy::I32 => Some(self.context.i32_type().into()),
            NamedIntrinsicRuntimeTy::I64 => Some(self.context.i64_type().into()),
            NamedIntrinsicRuntimeTy::WordInt | NamedIntrinsicRuntimeTy::WordUInt => Some(
                self.context
                    .custom_width_int_type(self.host.word_bit_width())
                    .into(),
            ),
            NamedIntrinsicRuntimeTy::Bool => Some(self.context.bool_type().into()),
            NamedIntrinsicRuntimeTy::Float32 => Some(self.context.f32_type().into()),
            NamedIntrinsicRuntimeTy::Float64 => Some(self.context.f64_type().into()),
            NamedIntrinsicRuntimeTy::StringRef => Some(self.llvm_scoop_string_ptr_type().into()),
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
            NamedIntrinsicRuntimeTy::I32 => {
                let target = CgTy::Int(IntTy {
                    bits: 32,
                    signed: true,
                });
                let coerced = self.coerce_value(operand.span, operand.value, target)?;
                coerced.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "named runtime intrinsic i32 operand value",
                    at: operand.span.into(),
                })?
            }
            NamedIntrinsicRuntimeTy::I64 => {
                let target = CgTy::Int(IntTy {
                    bits: 64,
                    signed: true,
                });
                let coerced = self.coerce_value(operand.span, operand.value, target)?;
                coerced.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "named runtime intrinsic i64 operand value",
                    at: operand.span.into(),
                })?
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
            NamedIntrinsicRuntimeTy::Float32 => {
                let coerced = self.coerce_value(operand.span, operand.value, CgTy::Float32)?;
                coerced.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "named runtime intrinsic f32 operand value",
                    at: operand.span.into(),
                })?
            }
            NamedIntrinsicRuntimeTy::Float64 => {
                let coerced = self.coerce_value(operand.span, operand.value, CgTy::Float64)?;
                coerced.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "named runtime intrinsic f64 operand value",
                    at: operand.span.into(),
                })?
            }
            NamedIntrinsicRuntimeTy::StringRef => {
                let coerced = self.coerce_value(operand.span, operand.value, CgTy::String)?;
                coerced.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "named runtime intrinsic string operand value",
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
            NamedIntrinsicRuntimeTy::I32 => {
                let value = call_site
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "named runtime intrinsic i32 return value",
                        at: span.into(),
                    })?
                    .into_int_value();
                Ok(CgValue::int(
                    value,
                    IntTy {
                        bits: 32,
                        signed: true,
                    },
                ))
            }
            NamedIntrinsicRuntimeTy::I64 => {
                let value = call_site
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "named runtime intrinsic i64 return value",
                        at: span.into(),
                    })?
                    .into_int_value();
                Ok(CgValue::int(
                    value,
                    IntTy {
                        bits: 64,
                        signed: true,
                    },
                ))
            }
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
            NamedIntrinsicRuntimeTy::Float32 => {
                let value = call_site
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "named runtime intrinsic f32 return value",
                        at: span.into(),
                    })?
                    .into_float_value();
                Ok(CgValue::float(value, CgTy::Float32))
            }
            NamedIntrinsicRuntimeTy::Float64 => {
                let value = call_site
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "named runtime intrinsic f64 return value",
                        at: span.into(),
                    })?
                    .into_float_value();
                Ok(CgValue::float(value, CgTy::Float64))
            }
            NamedIntrinsicRuntimeTy::StringRef => {
                let value = call_site
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "named runtime intrinsic string return value",
                        at: span.into(),
                    })?
                    .into_pointer_value();
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(value.into()),
                })
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
