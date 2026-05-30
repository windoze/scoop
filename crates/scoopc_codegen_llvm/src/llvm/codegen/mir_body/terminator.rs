//! MIR terminator and rvalue codegen entry.

#![allow(dead_code)]

use super::ty::CodegenMonoInput;
use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn tuple_element_cg_ty<T: CodegenMonoInput>(
        &self,
        tuple_ty: T,
        index: usize,
    ) -> Option<CgTy> {
        let tuple_ty = tuple_ty.try_into_mono_type_id(self)?;
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = self.types.kind(tuple_ty.inner())
        else {
            return None;
        };
        let elem_ty = *elements.get(index)?;
        self.try_cg_ty_of_type_id(elem_ty)
    }

    pub(in crate::llvm::codegen) fn bind_mir_params(
        &mut self,
        hir_fun: &hir::FunDecl,
        mir_fun: &crate::mir::FunDecl,
        mir_types: &TypeStore,
        llvm_fun: FunctionValue<'ctx>,
        param_offset: u32,
        slots: &mut [MirLocalSlot<'ctx>],
    ) -> Result<(), LlvmEmitError> {
        for (idx, param) in mir_fun.params.iter().enumerate() {
            let _hir_param = hir_fun.params.get(idx).unwrap_or_else(|| {
                panic!("bind_mir_params: MIR verifier accepted param arity drift")
            });
            let slot = slots
                .get(param.local.as_u32() as usize)
                .copied()
                .unwrap_or_else(|| {
                    std::panic::panic_any(
                        "bind_mir_params: MIR verifier accepted param local outside slot table",
                    )
                });
            let abi_ty = self
                .equivalent_codegen_type_id(mir_types, param.ty)
                .unwrap_or_else(|| {
                    panic!("bind_mir_params: MIR verifier accepted unsupported param type")
                });
            let abi = self.ordinary_param_abi(param.span, abi_ty)?;
            let init = if let Some(pointee_ty) = abi.pointee_ty() {
                let param_ptr = llvm_fun
                    .get_nth_param(idx as u32 + param_offset)
                    .unwrap_or_else(|| {
                        std::panic::panic_any(
                            "bind_mir_params: ABI declaration missing lowered LLVM parameter",
                        )
                    })
                    .into_pointer_value();
                let loaded =
                    self.builder
                        .build_load(pointee_ty, param_ptr, "pass_mir_param_load")?;
                self.cg_value_from_loaded(param.span, slot.cg_ty, loaded)?
            } else {
                self.cg_value_from_llvm_param(
                    param.span,
                    llvm_fun,
                    idx as u32 + param_offset,
                    slot.cg_ty,
                    "missing pass MIR llvm param",
                )?
            };
            let _ = self.store_local_value(param.span, slot.ptr, slot.cg_ty, init)?;
        }
        Ok(())
    }

    pub(in crate::llvm::codegen) fn bind_mir_params_without_hir(
        &mut self,
        mir_fun: &crate::mir::FunDecl,
        llvm_fun: FunctionValue<'ctx>,
        param_offset: u32,
        slots: &mut [MirLocalSlot<'ctx>],
    ) -> Result<(), LlvmEmitError> {
        for (idx, param) in mir_fun.params.iter().enumerate() {
            let slot = slots.get(param.local.as_u32() as usize).copied().unwrap_or_else(|| {
                std::panic::panic_any("bind_mir_params_without_hir: MIR verifier accepted param local outside slot table")
            });
            let init = if slot.cg_ty == CgTy::Unit {
                CgValue::unit()
            } else {
                self.cg_value_from_llvm_param(
                    param.span,
                    llvm_fun,
                    idx as u32 + param_offset,
                    slot.cg_ty,
                    "missing plain MIR llvm param",
                )?
            };
            let _ = self.store_local_value(param.span, slot.ptr, slot.cg_ty, init)?;
        }
        Ok(())
    }

    pub(in crate::llvm::codegen) fn bind_lir_source_params(
        &mut self,
        source_fun: &crate::effect_lowered::ir::LateLoweredSourceCallable,
        source_types: &TypeStore,
        llvm_fun: FunctionValue<'ctx>,
        param_offset: u32,
        slots: &mut [MirLocalSlot<'ctx>],
    ) -> Result<(), LlvmEmitError> {
        for (idx, param) in source_fun.params.iter().enumerate() {
            let slot = slots.get(param.local.as_u32() as usize).copied().unwrap_or_else(|| {
                std::panic::panic_any(
                    "bind_lir_source_params: LIR verifier accepted param local outside slot table",
                )
            });
            let abi_ty = self
                .equivalent_codegen_type_id(source_types, param.ty)
                .unwrap_or_else(|| {
                    panic!("bind_lir_source_params: LIR verifier accepted unsupported param type")
                });
            let abi = self.ordinary_param_abi(param.span, abi_ty)?;
            let init = if let Some(pointee_ty) = abi.pointee_ty() {
                let param_ptr = llvm_fun
                    .get_nth_param(idx as u32 + param_offset)
                    .unwrap_or_else(|| {
                        std::panic::panic_any(
                            "bind_lir_source_params: ABI declaration missing lowered LLVM parameter",
                        )
                    })
                    .into_pointer_value();
                let loaded = self
                    .builder
                    .build_load(pointee_ty, param_ptr, "lir_param_load")?;
                self.cg_value_from_loaded(param.span, slot.cg_ty, loaded)?
            } else {
                self.cg_value_from_llvm_param(
                    param.span,
                    llvm_fun,
                    idx as u32 + param_offset,
                    slot.cg_ty,
                    "missing LIR source llvm param",
                )?
            };
            let _ = self.store_local_value(param.span, slot.ptr, slot.cg_ty, init)?;
        }
        Ok(())
    }

    pub(in crate::llvm::codegen) fn codegen_mir_statement(
        &mut self,
        stmt: &crate::mir::Statement,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        used_locals: &HashSet<crate::mir::LocalId>,
    ) -> Result<(), LlvmEmitError> {
        if self
            .builder
            .get_insert_block()
            .is_some_and(|bb| bb.get_terminator().is_some())
        {
            return Ok(());
        }

        match &stmt.kind {
            crate::mir::StatementKind::Nop => Ok(()),
            crate::mir::StatementKind::Assign { target, value } => {
                if !used_locals.contains(target)
                    && let crate::mir::Rvalue::TopLevelRef(crate::mir::TopLevelRef { fqn, .. }) =
                        value
                    && self.published_codegen_callable_signature(fqn).is_some()
                {
                    return Ok(());
                }
                let slot = self.mir_local_slot(stmt.span, slots, *target)?;
                let target_source_ty = body
                    .locals
                    .get(target.as_u32() as usize)
                    .map(|local| local.ty);
                let value = self.codegen_mir_rvalue(
                    stmt.span,
                    value,
                    body,
                    mir_types,
                    slots,
                    slot.cg_ty,
                    target_source_ty,
                )?;
                let _ = self.store_local_value(stmt.span, slot.ptr, slot.cg_ty, value)?;
                Ok(())
            }
            crate::mir::StatementKind::StoreMember {
                receiver,
                member,
                value,
                value_ty,
                continuation_route,
            } => self.codegen_mir_store_member(
                stmt.span,
                receiver,
                member,
                value,
                *value_ty,
                continuation_route,
                body,
                mir_types,
                slots,
            ),
            crate::mir::StatementKind::StoreTopLevelVar {
                fqn,
                value,
                value_ty,
            } => self.codegen_mir_store_top_level_var(stmt.span, fqn, value, *value_ty, slots),
            crate::mir::StatementKind::Todo(_) => std::panic::panic_any(
                "MIR verifier must reject Todo statements before LLVM codegen",
            ),
        }
    }

    pub(in crate::llvm::codegen) fn codegen_mir_terminator(
        &mut self,
        terminator: &crate::mir::Terminator,
        _body: &crate::mir::Body,
        _mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        llvm_blocks: &[inkwell::basic_block::BasicBlock<'ctx>],
        declared_return_cg: CgTy,
    ) -> Result<(), LlvmEmitError> {
        if self
            .builder
            .get_insert_block()
            .is_some_and(|bb| bb.get_terminator().is_some())
        {
            return Ok(());
        }

        match &terminator.kind {
            crate::mir::TerminatorKind::Return { value } => {
                let value = match value {
                    Some(operand) => self.codegen_mir_operand_expected(
                        terminator.span,
                        operand,
                        slots,
                        Some(declared_return_cg),
                    )?,
                    None => {
                        mir_empty_return_contract_is_lowerable(
                            terminator.span,
                            declared_return_cg,
                        )?;
                        CgValue::unit()
                    }
                };
                let value = self.coerce_value(terminator.span, value, declared_return_cg)?;
                self.finish_function_return_path(terminator.span, declared_return_cg, value)
            }
            crate::mir::TerminatorKind::Goto { target } => {
                let target_bb = llvm_blocks
                    .get(target.as_u32() as usize)
                    .copied()
                    .unwrap_or_else(|| {
                        std::panic::panic_any(
                            "codegen_mir_terminator: MIR verifier accepted invalid goto target",
                        )
                    });
                self.builder.build_unconditional_branch(target_bb)?;
                Ok(())
            }
            crate::mir::TerminatorKind::CondBr {
                cond,
                then_target,
                else_target,
            } => {
                let cond = self
                    .codegen_mir_operand(terminator.span, cond, slots)?
                    .as_bool()
                    .unwrap_or_else(|| {
                        std::panic::panic_any("codegen_mir_terminator: MIR verifier accepted non-Bool branch condition")
                    });
                let then_bb = llvm_blocks
                    .get(then_target.as_u32() as usize)
                    .copied()
                    .unwrap_or_else(|| {
                        std::panic::panic_any(
                            "codegen_mir_terminator: MIR verifier accepted invalid then target",
                        )
                    });
                let else_bb = llvm_blocks
                    .get(else_target.as_u32() as usize)
                    .copied()
                    .unwrap_or_else(|| {
                        std::panic::panic_any(
                            "codegen_mir_terminator: MIR verifier accepted invalid else target",
                        )
                    });
                self.builder
                    .build_conditional_branch(cond, then_bb, else_bb)?;
                Ok(())
            }
            crate::mir::TerminatorKind::Unreachable => {
                self.builder.build_unreachable()?;
                Ok(())
            }
            crate::mir::TerminatorKind::Perform { .. } => Err(raw_mir_route_gate_error(
                self.function_cx
                    .current_callable_fqn
                    .as_deref()
                    .unwrap_or("<unknown raw mir body>"),
                terminator.span,
                "PIPELINE_GAPS §3.2",
                RAW_MIR_PERFORM_TERMINATOR_DETAIL,
            )),
            crate::mir::TerminatorKind::ResumeUnwind
            | crate::mir::TerminatorKind::Handle { .. } => Err(raw_mir_route_gate_error(
                self.function_cx
                    .current_callable_fqn
                    .as_deref()
                    .unwrap_or("<unknown raw mir body>"),
                terminator.span,
                "PIPELINE_GAPS §3.1",
                RAW_MIR_EFFECT_CONTROL_TERMINATOR_DETAIL,
            )),
            crate::mir::TerminatorKind::Todo(_) => Err(raw_mir_route_gate_error(
                self.function_cx
                    .current_callable_fqn
                    .as_deref()
                    .unwrap_or("<unknown raw mir body>"),
                terminator.span,
                "PIPELINE_GAPS §2.3",
                RAW_MIR_TODO_TERMINATOR_DETAIL,
            )),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_rvalue(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Rvalue,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        target_cg: CgTy,
        target_source_ty: Option<TypeId>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match value {
            crate::mir::Rvalue::Use(operand) => {
                self.codegen_mir_operand_expected(span, operand, slots, Some(target_cg))
            }
            crate::mir::Rvalue::Transport { value, transport } => self.codegen_mir_value_transport(
                span, value, transport, body, mir_types, slots, target_cg,
            ),
            crate::mir::Rvalue::TopLevelRef(crate::mir::TopLevelRef { fqn, .. }) => {
                if let Some(value) =
                    self.try_codegen_qualified_enum_unit_variant_value(span, fqn)?
                {
                    Ok(value)
                } else {
                    self.codegen_top_level_value_ref(span, fqn)
                }
            }
            crate::mir::Rvalue::TypeCheck {
                value,
                op,
                test_ty,
                metadata,
            } => {
                self.codegen_mir_type_check(span, value, *op, *test_ty, metadata, mir_types, slots)
            }
            crate::mir::Rvalue::Cast {
                value,
                op,
                target_ty,
                metadata,
            } => self.codegen_mir_cast(
                span, value, *op, *target_ty, metadata, mir_types, slots, target_cg,
            ),
            crate::mir::Rvalue::Call {
                kind,
                args,
                transport,
                ..
            } => self.codegen_mir_call(span, kind, args, transport, body, mir_types, slots),
            crate::mir::Rvalue::PatternMatch { subject, pattern } => {
                self.codegen_mir_pattern_match(span, mir_types, subject, pattern, slots)
            }
            crate::mir::Rvalue::PatternExtract { subject, path } => {
                self.codegen_mir_pattern_extract(span, subject, path, slots, target_cg)
            }
            crate::mir::Rvalue::MakeTuple {
                elements,
                transport,
            } => {
                if let Some(value) =
                    self.try_emit_immortal_tuple(span, mir_types, elements, transport, target_cg)?
                {
                    Ok(value)
                } else {
                    self.codegen_mir_make_tuple(span, body, mir_types, elements, target_cg, slots)
                }
            }
            crate::mir::Rvalue::SizeOf { value_ty } => {
                self.codegen_mir_size_of(span, mir_types, *value_ty)
            }
            crate::mir::Rvalue::KindOf { value_ty } => {
                self.codegen_mir_kind_of(span, mir_types, *value_ty)
            }
            crate::mir::Rvalue::AlignOf { value_ty } => {
                self.codegen_mir_align_of(span, mir_types, *value_ty)
            }
            crate::mir::Rvalue::DescOf { value_ty } => {
                self.codegen_mir_desc_of(span, mir_types, *value_ty)
            }
            crate::mir::Rvalue::TypeMetadataLiteral(metadata) => {
                self.codegen_mir_type_metadata_literal(span, metadata, mir_types)
            }
            crate::mir::Rvalue::StructLit { fields, transport } => {
                if let Some(value) =
                    self.try_emit_immortal_struct(span, mir_types, fields, transport, target_cg)?
                {
                    Ok(value)
                } else {
                    self.codegen_mir_make_struct(
                        span, mir_types, fields, transport, target_cg, slots,
                    )
                }
            }
            crate::mir::Rvalue::InterpolatedString { .. } => std::panic::panic_any(
                "codegen_mir_rvalue: MIR verifier accepted residual interpolated string",
            ),
            crate::mir::Rvalue::TupleGet { tuple, index } => {
                self.codegen_mir_tuple_get(span, body, mir_types, tuple, *index, slots)
            }
            crate::mir::Rvalue::MakeClosure {
                env,
                fn_ptr,
                env_contract,
            } => {
                let env_cg = self.mir_operand_cg_ty(body, mir_types, env).unwrap_or_else(|| {
                        panic!("codegen_mir_rvalue: MIR verifier accepted closure env without codegen type")
                    });
                self.codegen_mir_make_closure(
                    span,
                    env,
                    fn_ptr,
                    env_contract,
                    mir_types,
                    env_cg,
                    target_cg,
                    slots,
                )
            }
            crate::mir::Rvalue::PerformResult { .. } => Err(raw_mir_route_gate_error(
                self.function_cx
                    .current_callable_fqn
                    .as_deref()
                    .unwrap_or("<unknown raw mir body>"),
                span,
                "PIPELINE_GAPS §3.3",
                RAW_MIR_PERFORM_RESULT_DETAIL,
            )),
            crate::mir::Rvalue::MemberAccess {
                receiver, member, ..
            } => self.codegen_mir_member_access(
                span,
                receiver,
                member,
                MirBodyCodegenCtx {
                    body,
                    mir_types,
                    slots,
                },
                target_cg,
            ),
            crate::mir::Rvalue::EnumVariant {
                enum_ty,
                variant_name,
                args,
                payload,
            } => self.codegen_mir_enum_variant_ctor_call(
                span,
                *enum_ty,
                variant_name,
                args,
                payload,
                body,
                mir_types,
                slots,
            ),
            crate::mir::Rvalue::ClassCtor {
                class_fqn,
                ctor,
                args,
                ..
            } => {
                let class_layout_key =
                    self.mir_class_ctor_layout_key(span, class_fqn, mir_types, target_source_ty)?;
                self.codegen_mir_class_ctor_call(span, &class_layout_key, ctor, args, slots)
            }
            crate::mir::Rvalue::UnresolvedName { name } => {
                self.codegen_unresolved_ident(span, name, Some(target_cg))
            }
            crate::mir::Rvalue::Todo(_) => {
                std::panic::panic_any("codegen_mir_rvalue: MIR verifier accepted Todo rvalue")
            }
        }
    }

    pub(in crate::llvm::codegen) fn codegen_mir_effect_neutral_rvalue(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Rvalue,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match value {
            crate::mir::Rvalue::Use(operand) => {
                self.codegen_mir_operand_expected(span, operand, slots, Some(target_cg))
            }
            crate::mir::Rvalue::Transport { value, transport } => self.codegen_mir_value_transport(
                span, value, transport, body, mir_types, slots, target_cg,
            ),
            crate::mir::Rvalue::TopLevelRef(crate::mir::TopLevelRef { fqn, .. }) => {
                if let Some(value) =
                    self.try_codegen_qualified_enum_unit_variant_value(span, fqn)?
                {
                    Ok(value)
                } else {
                    self.codegen_top_level_value_ref(span, fqn)
                }
            }
            crate::mir::Rvalue::TypeCheck {
                value,
                op,
                test_ty,
                metadata,
            } => {
                self.codegen_mir_type_check(span, value, *op, *test_ty, metadata, mir_types, slots)
            }
            crate::mir::Rvalue::Cast {
                value,
                op,
                target_ty,
                metadata,
            } => self.codegen_mir_cast(
                span, value, *op, *target_ty, metadata, mir_types, slots, target_cg,
            ),
            crate::mir::Rvalue::PatternMatch { subject, pattern } => {
                self.codegen_mir_pattern_match(span, mir_types, subject, pattern, slots)
            }
            crate::mir::Rvalue::PatternExtract { subject, path } => {
                self.codegen_mir_pattern_extract(span, subject, path, slots, target_cg)
            }
            crate::mir::Rvalue::MakeTuple {
                elements,
                transport,
            } => {
                if let Some(value) =
                    self.try_emit_immortal_tuple(span, mir_types, elements, transport, target_cg)?
                {
                    Ok(value)
                } else {
                    self.codegen_mir_make_tuple(span, body, mir_types, elements, target_cg, slots)
                }
            }
            crate::mir::Rvalue::SizeOf { value_ty } => {
                self.codegen_mir_size_of(span, mir_types, *value_ty)
            }
            crate::mir::Rvalue::KindOf { value_ty } => {
                self.codegen_mir_kind_of(span, mir_types, *value_ty)
            }
            crate::mir::Rvalue::AlignOf { value_ty } => {
                self.codegen_mir_align_of(span, mir_types, *value_ty)
            }
            crate::mir::Rvalue::DescOf { value_ty } => {
                self.codegen_mir_desc_of(span, mir_types, *value_ty)
            }
            crate::mir::Rvalue::TypeMetadataLiteral(metadata) => {
                self.codegen_mir_type_metadata_literal(span, metadata, mir_types)
            }
            crate::mir::Rvalue::StructLit { fields, transport } => {
                if let Some(value) =
                    self.try_emit_immortal_struct(span, mir_types, fields, transport, target_cg)?
                {
                    Ok(value)
                } else {
                    self.codegen_mir_make_struct(
                        span, mir_types, fields, transport, target_cg, slots,
                    )
                }
            }
            crate::mir::Rvalue::InterpolatedString { .. } => std::panic::panic_any(
                "codegen_mir_effect_neutral_rvalue: MIR verifier accepted residual interpolated string",
            ),
            crate::mir::Rvalue::TupleGet { tuple, index } => {
                self.codegen_mir_tuple_get(span, body, mir_types, tuple, *index, slots)
            }
            crate::mir::Rvalue::MemberAccess {
                receiver, member, ..
            } => self.codegen_mir_member_access(
                span,
                receiver,
                member,
                MirBodyCodegenCtx {
                    body,
                    mir_types,
                    slots,
                },
                target_cg,
            ),
            crate::mir::Rvalue::EnumVariant {
                enum_ty,
                variant_name,
                args,
                payload,
            } => self.codegen_mir_enum_variant_ctor_call(
                span,
                *enum_ty,
                variant_name,
                args,
                payload,
                body,
                mir_types,
                slots,
            ),
            crate::mir::Rvalue::Call { .. } => std::panic::panic_any(
                "codegen_mir_effect_neutral_rvalue: value primitive call must publish ABI before codegen",
            ),
            crate::mir::Rvalue::MakeClosure {
                env,
                fn_ptr,
                env_contract,
            } => {
                let env_cg = self.mir_operand_cg_ty(body, mir_types, env).unwrap_or_else(|| {
                    panic!("codegen_mir_effect_neutral_rvalue: MIR verifier accepted closure env without codegen type")
                });
                self.codegen_mir_make_closure(
                    span,
                    env,
                    fn_ptr,
                    env_contract,
                    mir_types,
                    env_cg,
                    target_cg,
                    slots,
                )
            }
            crate::mir::Rvalue::ClassCtor { .. } => std::panic::panic_any(
                "codegen_mir_effect_neutral_rvalue: value primitive class construction must publish ABI before codegen",
            ),
            crate::mir::Rvalue::PerformResult { .. } => std::panic::panic_any(
                "codegen_mir_effect_neutral_rvalue: value primitive boundary payload must publish contract before codegen",
            ),
            crate::mir::Rvalue::UnresolvedName { name } => {
                self.codegen_unresolved_ident(span, name, Some(target_cg))
            }
            crate::mir::Rvalue::Todo(_) => std::panic::panic_any(
                "codegen_mir_effect_neutral_rvalue: MIR verifier accepted Todo value primitive rvalue",
            ),
        }
    }
}
