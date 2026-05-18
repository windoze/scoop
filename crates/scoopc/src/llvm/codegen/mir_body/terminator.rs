//! MIR terminator and rvalue codegen entry.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn tuple_element_cg_ty(
        &self,
        tuple_ty: TypeId,
        index: usize,
    ) -> Option<CgTy> {
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = self.types.kind(tuple_ty) else {
            return None;
        };
        let elem_ty = *elements.get(index)?;
        self.cg_ty_of(elem_ty)
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
            let slot = slots.get(param.local.as_u32() as usize).copied().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR param local",
                    at: param.span.into(),
                },
            )?;
            let abi_ty = self
                .equivalent_codegen_type_id(mir_types, param.ty)
                .unwrap_or_else(|| {
                    panic!("bind_mir_params: MIR verifier accepted unsupported param type")
                });
            let abi = self.ordinary_param_abi(param.span, abi_ty)?;
            let init = if let Some(pointee_ty) = abi.pointee_ty() {
                let param_ptr = llvm_fun
                    .get_nth_param(idx as u32 + param_offset)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "missing pass MIR llvm param",
                        at: param.span.into(),
                    })?
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
            let slot = slots.get(param.local.as_u32() as usize).copied().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR param local",
                    at: param.span.into(),
                },
            )?;
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
                    && self.fun_index.contains_key(fqn)
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
                let target_bb = llvm_blocks.get(target.as_u32() as usize).copied().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR goto target",
                        at: terminator.span.into(),
                    },
                )?;
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
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR branch condition",
                        at: terminator.span.into(),
                    })?;
                let then_bb = llvm_blocks
                    .get(then_target.as_u32() as usize)
                    .copied()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR then target",
                        at: terminator.span.into(),
                    })?;
                let else_bb = llvm_blocks
                    .get(else_target.as_u32() as usize)
                    .copied()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR else target",
                        at: terminator.span.into(),
                    })?;
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
            crate::mir::Rvalue::MakeTuple { elements, .. } => {
                self.codegen_mir_make_tuple(span, body, mir_types, elements, target_cg, slots)
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
            crate::mir::Rvalue::StructLit { fields, .. } => {
                self.codegen_mir_make_struct(span, fields, target_cg, slots)
            }
            crate::mir::Rvalue::InterpolatedString { .. } => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "interpolated string after HIR desugar",
                    at: span.into(),
                })
            }
            crate::mir::Rvalue::TupleGet { tuple, index } => {
                self.codegen_mir_tuple_get(span, body, mir_types, tuple, *index, slots)
            }
            crate::mir::Rvalue::MakeClosure {
                env,
                fn_ptr,
                env_contract,
            } => {
                let env_cg = self.mir_operand_cg_ty(body, mir_types, env).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR closure env type",
                        at: span.into(),
                    },
                )?;
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
                    self.mir_class_ctor_layout_key(class_fqn, mir_types, target_source_ty);
                self.codegen_mir_class_ctor_call(span, &class_layout_key, ctor, args, slots)
            }
            crate::mir::Rvalue::UnresolvedName { name } => {
                self.codegen_unresolved_ident(span, name, Some(target_cg))
            }
            crate::mir::Rvalue::Todo(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR rvalue",
                at: span.into(),
            }),
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
            crate::mir::Rvalue::MakeTuple { elements, .. } => {
                self.codegen_mir_make_tuple(span, body, mir_types, elements, target_cg, slots)
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
            crate::mir::Rvalue::StructLit { fields, .. } => {
                self.codegen_mir_make_struct(span, fields, target_cg, slots)
            }
            crate::mir::Rvalue::InterpolatedString { .. } => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "interpolated string after HIR desugar",
                    at: span.into(),
                })
            }
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
            crate::mir::Rvalue::Call { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "value primitive call requires published ABI",
                at: span.into(),
            }),
            crate::mir::Rvalue::MakeClosure { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "value primitive closure carrier requires published ABI",
                at: span.into(),
            }),
            crate::mir::Rvalue::ClassCtor { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "value primitive class construction requires published ABI",
                at: span.into(),
            }),
            crate::mir::Rvalue::PerformResult { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "value primitive boundary payload requires published contract",
                at: span.into(),
            }),
            crate::mir::Rvalue::UnresolvedName { name } => {
                self.codegen_unresolved_ident(span, name, Some(target_cg))
            }
            crate::mir::Rvalue::Todo(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "value primitive rvalue",
                at: span.into(),
            }),
        }
    }
}
