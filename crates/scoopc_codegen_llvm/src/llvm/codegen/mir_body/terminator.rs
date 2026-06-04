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

    pub(in crate::llvm::codegen) fn bind_mir_params_without_hir(
        &mut self,
        mir_fun: &mir_source::FunDecl,
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

    pub(in crate::llvm::codegen) fn codegen_lir_statement(
        &mut self,
        stmt: &crate::effect_lowered::LirStatement,
        body: &LirExecutableBody,
        source_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        used_locals: &HashSet<crate::effect_lowered::mir_source::LocalId>,
        abi: Option<&ProgramAbiQuery<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        if self
            .builder
            .get_insert_block()
            .is_some_and(|bb| bb.get_terminator().is_some())
        {
            return Ok(());
        }

        match &stmt.kind {
            LirStatementKind::Nop => Ok(()),
            LirStatementKind::Assign { target, value } => {
                if !used_locals.contains(target)
                    && self.lir_top_level_ref_can_skip_when_unused(value)
                {
                    return Ok(());
                }
                if let LirRvalue::MemberAccess { member, .. } = value
                    && matches!(
                        member.resolved,
                        LirMemberTarget::Fun { .. } | LirMemberTarget::ExtensionFun { .. }
                    )
                {
                    return Ok(());
                }
                let slot = self.mir_local_slot(stmt.span, slots, *target)?;
                let value = self.codegen_lir_rvalue(
                    stmt.span,
                    value,
                    body,
                    source_types,
                    slots,
                    slot.cg_ty,
                    Some(*target),
                    abi,
                )?;
                let value_ty = value.ty;
                if slot.cg_ty == CgTy::Never {
                    if value_ty == CgTy::Never
                        && self
                            .builder
                            .get_insert_block()
                            .is_some_and(|bb| bb.get_terminator().is_none())
                    {
                        self.builder.build_unreachable()?;
                    }
                    return Ok(());
                }
                let _ = self.store_local_value(stmt.span, slot.ptr, slot.cg_ty, value)?;
                if value_ty == CgTy::Never
                    && self
                        .builder
                        .get_insert_block()
                        .is_some_and(|bb| bb.get_terminator().is_none())
                {
                    self.builder.build_unreachable()?;
                }
                Ok(())
            }
            LirStatementKind::StoreMember {
                receiver,
                member,
                value,
                value_ty,
                continuation_route,
            } => self.codegen_lir_store_member(
                stmt.span,
                receiver,
                member,
                value,
                *value_ty,
                continuation_route,
                body,
                source_types,
                slots,
            ),
            LirStatementKind::StoreGlobal {
                root,
                value,
                value_ty,
            } => self.codegen_lir_store_global(stmt.span, root, value, *value_ty, slots),
        }
    }

    pub(in crate::llvm::codegen) fn codegen_lir_plain_terminator(
        &mut self,
        terminator: &LateLoweredStateTerminator,
        slots: &[MirLocalSlot<'ctx>],
        llvm_blocks: &std::collections::HashMap<StateId, inkwell::basic_block::BasicBlock<'ctx>>,
        declared_return_cg: CgTy,
    ) -> Result<(), LlvmEmitError> {
        if self
            .builder
            .get_insert_block()
            .is_some_and(|bb| bb.get_terminator().is_some())
        {
            return Ok(());
        }

        match terminator {
            LateLoweredStateTerminator::Return { payload_source, .. } => {
                let span = payload_source
                    .operand_source()
                    .and_then(|source| source.span())
                    .unwrap_or(crate::span::Span::new(0, 0));
                let value = match payload_source {
                    LateLoweredCompletionPayloadSource::Unit { .. } => {
                        mir_empty_return_contract_is_lowerable(span, declared_return_cg)?;
                        CgValue::unit()
                    }
                    LateLoweredCompletionPayloadSource::Operand(source) => match source.value() {
                        LateLoweredOperandValueSource::Local(local) => {
                            let slot = self.mir_local_slot(span, slots, *local)?;
                            self.load_mir_local(span, slot)?
                        }
                        LateLoweredOperandValueSource::Const(value) => {
                            self.codegen_mir_const(span, value, Some(declared_return_cg))?
                        }
                    },
                };
                let value = self.coerce_value(span, value, declared_return_cg)?;
                self.finish_function_return_path(span, declared_return_cg, value)
            }
            LateLoweredStateTerminator::Goto { target } => {
                let target_bb = llvm_blocks.get(target).copied().unwrap_or_else(|| {
                    std::panic::panic_any(
                        "codegen_lir_plain_terminator: LIR verifier accepted invalid goto target",
                    )
                });
                self.builder.build_unconditional_branch(target_bb)?;
                Ok(())
            }
            LateLoweredStateTerminator::Branch {
                cond_local,
                then_state,
                else_state,
            } => {
                let cond = self
                    .codegen_lir_operand(
                        crate::span::Span::new(0, 0),
                        &LirOperand::Local(*cond_local),
                        slots,
                    )?
                    .as_bool()
                    .unwrap_or_else(|| {
                        std::panic::panic_any(
                            "codegen_lir_plain_terminator: LIR verifier accepted non-Bool branch condition",
                        )
                    });
                let then_bb = llvm_blocks.get(then_state).copied().unwrap_or_else(|| {
                    std::panic::panic_any(
                        "codegen_lir_plain_terminator: LIR verifier accepted invalid then target",
                    )
                });
                let else_bb = llvm_blocks.get(else_state).copied().unwrap_or_else(|| {
                    std::panic::panic_any(
                        "codegen_lir_plain_terminator: LIR verifier accepted invalid else target",
                    )
                });
                self.builder
                    .build_conditional_branch(cond, then_bb, else_bb)?;
                Ok(())
            }
            LateLoweredStateTerminator::Unreachable | LateLoweredStateTerminator::Abandon => {
                self.builder.build_unreachable()?;
                Ok(())
            }
            LateLoweredStateTerminator::Suspend { .. }
            | LateLoweredStateTerminator::HandleDispatch { .. }
            | LateLoweredStateTerminator::LocalRuntimeError { .. }
            | LateLoweredStateTerminator::ResumeUnwind => panic!(
                "codegen_lir_plain_terminator: effect/control terminator reached plain callable lowering"
            ),
        }
    }

    fn lir_top_level_ref_can_skip_when_unused(&self, value: &LirRvalue) -> bool {
        match value {
            LirRvalue::TopLevelRef(crate::effect_lowered::LirTopLevelRef {
                target: LirTopLevelRefTarget::Callable(_),
                ..
            }) => true,
            LirRvalue::TopLevelRef(crate::effect_lowered::LirTopLevelRef {
                target: LirTopLevelRefTarget::Global(root),
                ..
            }) => {
                let key = root.as_str();
                self.enum_layouts.contains_key(key) || self.nominal_kinds.contains_key(key)
            }
            _ => false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_lir_rvalue(
        &mut self,
        span: crate::span::Span,
        value: &LirRvalue,
        body: &LirExecutableBody,
        source_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        target_cg: CgTy,
        target_local: Option<crate::effect_lowered::mir_source::LocalId>,
        abi: Option<&ProgramAbiQuery<'ctx>>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if let Some(abi) = abi
            && let LirRvalue::Use(LirOperand::Local(source_local))
            | LirRvalue::Transport {
                value: LirOperand::Local(source_local),
                ..
            } = value
            && let Some((env, fn_ptr, env_contract)) =
                self.lir_local_make_closure_source(body, *source_local)
            && let Some(adapter) = self.maybe_build_lir_effect_typed_closure_target_fn_ptr(
                span,
                abi,
                source_types,
                body,
                target_local,
                fn_ptr,
            )?
        {
            let env_cg = self
                .lir_operand_cg_ty(body, source_types, &env)
                .unwrap_or_else(|| {
                    panic!(
                        "codegen_lir_rvalue: LIR verifier accepted propagated closure env without codegen type"
                    )
                });
            return self.codegen_lir_make_closure_with_target_fn_ptr(
                span,
                &env,
                fn_ptr,
                &env_contract,
                source_types,
                env_cg,
                target_cg,
                slots,
                adapter,
            );
        }
        match value {
            LirRvalue::Use(operand) => {
                self.codegen_lir_operand_expected(span, operand, slots, Some(target_cg))
            }
            LirRvalue::Transport { value, transport } => self.codegen_lir_value_transport(
                span,
                value,
                transport,
                body,
                source_types,
                slots,
                target_cg,
            ),
            LirRvalue::TopLevelRef(top) => match &top.target {
                LirTopLevelRefTarget::Global(root) => {
                    let key = root.as_str();
                    if let Some(value) =
                        self.try_codegen_qualified_enum_unit_variant_value(span, key)?
                    {
                        Ok(value)
                    } else {
                        self.codegen_top_level_value_ref(span, key)
                    }
                }
                LirTopLevelRefTarget::Callable(id) => {
                    let program = self.published_late_lowered_program().unwrap_or_else(|| {
                        panic!("codegen_lir_rvalue: missing published LIR program")
                    });
                    let callable = program.callable_by_id(*id).unwrap_or_else(|| {
                        panic!("codegen_lir_rvalue: LIR verifier accepted unknown callable ref")
                    });
                    self.codegen_top_level_value_ref(span, callable.root_fqn())
                }
            },
            LirRvalue::TypeCheck {
                value,
                op,
                test_ty,
                metadata,
            } => self.codegen_lir_type_check(
                span,
                value,
                *op,
                *test_ty,
                metadata,
                source_types,
                slots,
            ),
            LirRvalue::Cast {
                value,
                op,
                target_ty,
                metadata,
            } => self.codegen_lir_cast(
                span,
                value,
                *op,
                *target_ty,
                metadata,
                source_types,
                slots,
                target_cg,
            ),
            LirRvalue::Call {
                site_id,
                kind,
                args,
                transport,
            } => self.codegen_lir_call(
                span,
                *site_id,
                kind,
                args,
                transport,
                body,
                source_types,
                slots,
                abi,
            ),
            LirRvalue::PatternMatch { subject, pattern } => {
                self.codegen_lir_pattern_match(span, source_types, subject, pattern, slots)
            }
            LirRvalue::PatternExtract { subject, path } => {
                self.codegen_lir_pattern_extract(span, subject, path, slots, target_cg)
            }
            LirRvalue::MakeTuple {
                elements,
                transport: _,
            } => self.codegen_lir_make_tuple(span, elements, target_cg, slots),
            LirRvalue::SizeOf { value_ty, .. } => {
                self.codegen_mir_size_of(span, source_types, *value_ty)
            }
            LirRvalue::KindOf { value_ty, .. } => {
                self.codegen_mir_kind_of(span, source_types, *value_ty)
            }
            LirRvalue::AlignOf { value_ty, .. } => {
                self.codegen_mir_align_of(span, source_types, *value_ty)
            }
            LirRvalue::DescOf { value_ty, .. } => {
                self.codegen_mir_desc_of(span, source_types, *value_ty)
            }
            LirRvalue::TypeMetadataLiteral(metadata) => {
                self.codegen_lir_type_metadata_literal(span, metadata, source_types)
            }
            LirRvalue::StructLit { fields, transport } => {
                if let Some(abi) = abi {
                    self.install_lir_effect_typed_closure_target_overrides_for_struct_fields(
                        span,
                        abi,
                        source_types,
                        body,
                        fields,
                        target_cg,
                        slots,
                    )?;
                }
                self.codegen_lir_make_struct(
                    span,
                    source_types,
                    fields,
                    transport,
                    target_cg,
                    slots,
                )
            }
            LirRvalue::InterpolatedString { .. } => std::panic::panic_any(
                "codegen_lir_rvalue: LIR verifier accepted residual interpolated string",
            ),
            LirRvalue::TupleGet { tuple, index } => {
                self.codegen_lir_tuple_get(span, body, source_types, tuple, *index, slots)
            }
            LirRvalue::MakeClosure {
                env,
                fn_ptr,
                env_contract,
            } => {
                let env_cg = self.lir_operand_cg_ty(body, source_types, env).unwrap_or_else(|| {
                    panic!("codegen_lir_rvalue: LIR verifier accepted closure env without codegen type")
                });
                if let Some(abi) = abi
                    && let Some(adapter) = self.maybe_build_lir_effect_typed_closure_target_fn_ptr(
                        span,
                        abi,
                        source_types,
                        body,
                        target_local,
                        *fn_ptr,
                    )?
                {
                    return self.codegen_lir_make_closure_with_target_fn_ptr(
                        span,
                        env,
                        *fn_ptr,
                        env_contract,
                        source_types,
                        env_cg,
                        target_cg,
                        slots,
                        adapter,
                    );
                }
                self.codegen_lir_make_closure(
                    span,
                    env,
                    *fn_ptr,
                    env_contract,
                    source_types,
                    env_cg,
                    target_cg,
                    slots,
                )
            }
            LirRvalue::PerformResult { .. } => std::panic::panic_any(
                "codegen_lir_rvalue: perform result reached plain callable lowering",
            ),
            LirRvalue::MemberAccess {
                receiver, member, ..
            } => self.codegen_lir_member_access(
                span,
                receiver,
                member,
                LirBodyCodegenCtx {
                    body,
                    source_types,
                    slots,
                },
                target_cg,
            ),
            LirRvalue::EnumVariant {
                enum_ty,
                variant_name,
                args,
                payload,
            } => self.codegen_lir_enum_variant_ctor_call(
                span,
                *enum_ty,
                variant_name,
                args,
                payload,
                source_types,
                slots,
            ),
            LirRvalue::ClassCtor {
                site_id,
                class,
                ctor,
                args,
                ..
            } => {
                let class_layout_key =
                    self.lir_class_ctor_layout_key(span, *site_id, class, source_types)?;
                self.codegen_lir_class_ctor_call(
                    span,
                    *site_id,
                    &class_layout_key,
                    ctor,
                    args,
                    slots,
                )
            }
        }
    }

    pub(in crate::llvm::codegen) fn codegen_mir_effect_neutral_rvalue(
        &mut self,
        span: crate::span::Span,
        value: &mir_source::Rvalue,
        body: &mir_source::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match value {
            mir_source::Rvalue::Use(operand) => {
                self.codegen_mir_operand_expected(span, operand, slots, Some(target_cg))
            }
            mir_source::Rvalue::Transport { value, transport } => self.codegen_mir_value_transport(
                span, value, transport, body, mir_types, slots, target_cg,
            ),
            mir_source::Rvalue::TopLevelRef(mir_source::TopLevelRef { fqn, .. }) => {
                if let Some(value) =
                    self.try_codegen_qualified_enum_unit_variant_value(span, fqn)?
                {
                    Ok(value)
                } else {
                    self.codegen_top_level_value_ref(span, fqn)
                }
            }
            mir_source::Rvalue::TypeCheck {
                value,
                op,
                test_ty,
                metadata,
            } => {
                self.codegen_mir_type_check(span, value, *op, *test_ty, metadata, mir_types, slots)
            }
            mir_source::Rvalue::Cast {
                value,
                op,
                target_ty,
                metadata,
            } => self.codegen_mir_cast(
                span, value, *op, *target_ty, metadata, mir_types, slots, target_cg,
            ),
            mir_source::Rvalue::PatternMatch { subject, pattern } => {
                self.codegen_mir_pattern_match(span, mir_types, subject, pattern, slots)
            }
            mir_source::Rvalue::PatternExtract { subject, path } => {
                self.codegen_mir_pattern_extract(span, subject, path, slots, target_cg)
            }
            mir_source::Rvalue::MakeTuple {
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
            mir_source::Rvalue::SizeOf { value_ty, .. } => {
                self.codegen_mir_size_of(span, mir_types, *value_ty)
            }
            mir_source::Rvalue::KindOf { value_ty, .. } => {
                self.codegen_mir_kind_of(span, mir_types, *value_ty)
            }
            mir_source::Rvalue::AlignOf { value_ty, .. } => {
                self.codegen_mir_align_of(span, mir_types, *value_ty)
            }
            mir_source::Rvalue::DescOf { value_ty, .. } => {
                self.codegen_mir_desc_of(span, mir_types, *value_ty)
            }
            mir_source::Rvalue::TypeMetadataLiteral(metadata) => {
                self.codegen_mir_type_metadata_literal(span, metadata, mir_types)
            }
            mir_source::Rvalue::StructLit { fields, transport } => {
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
            mir_source::Rvalue::InterpolatedString { .. } => std::panic::panic_any(
                "codegen_mir_effect_neutral_rvalue: MIR verifier accepted residual interpolated string",
            ),
            mir_source::Rvalue::TupleGet { tuple, index } => {
                self.codegen_mir_tuple_get(span, body, mir_types, tuple, *index, slots)
            }
            mir_source::Rvalue::MemberAccess {
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
            mir_source::Rvalue::EnumVariant {
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
            mir_source::Rvalue::Call { .. } => std::panic::panic_any(
                "codegen_mir_effect_neutral_rvalue: value primitive call must publish ABI before codegen",
            ),
            mir_source::Rvalue::MakeClosure {
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
            mir_source::Rvalue::ClassCtor { .. } => std::panic::panic_any(
                "codegen_mir_effect_neutral_rvalue: value primitive class construction must publish ABI before codegen",
            ),
            mir_source::Rvalue::PerformResult { .. } => std::panic::panic_any(
                "codegen_mir_effect_neutral_rvalue: value primitive boundary payload must publish contract before codegen",
            ),
            mir_source::Rvalue::UnresolvedName { name } => {
                self.codegen_unresolved_ident(span, name, Some(target_cg))
            }
            mir_source::Rvalue::Todo(_) => std::panic::panic_any(
                "codegen_mir_effect_neutral_rvalue: MIR verifier accepted Todo value primitive rvalue",
            ),
        }
    }
}
