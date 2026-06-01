//! State-machine emission: walks every state, binds direct call arguments, lowers each state's source slices and terminator, and handles the suspend / resume-unwind / abandon flavors of state exit. Also covers state-block bookkeeping (sync slots, branch, seal unterminated).

use super::*;

impl<'cg, 'a, 'ctx> CallableEmitter<'cg, 'a, 'ctx> {
    pub(super) fn emit_states(&mut self) -> Result<(), LlvmEmitError> {
        for state in self.callable.state_graph().states() {
            let bb = self.state_block(state.state_id())?;
            self.codegen.builder.position_at_end(bb);
            self.lower_state_source_slices(state).map_err(|err| {
                frontend_error(format!(
                    "callable `{}` state st{} source-slice lowering failed: {err}",
                    self.callable.root_fqn(),
                    state.state_id().as_u32()
                ))
            })?;
            self.lower_state_terminator(state).map_err(|err| {
                frontend_error(format!(
                    "callable `{}` step schema s{} (ABI s{}) state st{} terminator lowering failed: {err}",
                    self.callable.root_fqn(),
                    self.callable.step_schema().as_u32(),
                    self.abi_step_schema.as_u32(),
                    state.state_id().as_u32()
                ))
            })?;
            if bb.get_terminator().is_none() {
                return Err(frontend_error(format!(
                    "callable `{}` state st{} lowering 完成后仍未生成 terminator；不能把该 state 留给后续 LLVM verifier 兜底",
                    self.callable.root_fqn(),
                    state.state_id().as_u32(),
                )));
            }
        }
        Ok(())
    }

    pub(super) fn bind_direct_args(
        &mut self,
        entry_layout: &CallableEntryLayout<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let args_layout = self
            .abi
            .source_value_layout(entry_layout.invoke_args_tuple_ty())?;
        let raw_arg = if entry_layout.param_count() == 0 {
            None
        } else {
            Some(self.function.get_nth_param(0).ok_or_else(|| {
                frontend_error(format!(
                    "direct entry `{}` 缺少 args tuple 参数",
                    entry_layout.symbol_name()
                ))
            })?)
        };
        let lambda_env_component_count = self.lambda_env_component_count(entry_layout)?;
        for (index, param) in self.mir_fun.params.iter().enumerate() {
            let param_cg = self
                .codegen
                .cg_ty_of_mir_type(self.source_types, param.ty)
                .unwrap_or_else(|| {
                    panic!(
                        "bind_direct_args: direct entry ABI verifier accepted non-codegen param type at {:?}",
                        param.span
                    )
                });
            let value = if let Some(env_component_count) = lambda_env_component_count {
                if index == 0 {
                    if env_component_count == 0 {
                        self.codegen.default_value(param.span, param_cg)?
                    } else if self.mir_fun.params.len() == 1
                        && param.ty == entry_layout.invoke_args_tuple_ty()
                        && matches!(param_cg, CgTy::Tuple(_))
                    {
                        self.bind_direct_tuple_param_from_components(
                            entry_layout.symbol_name(),
                            param.span,
                            param.ty,
                            param_cg,
                            args_layout,
                            raw_arg,
                            0,
                        )?
                    } else {
                        self.bind_direct_param_from_component(
                            entry_layout.symbol_name(),
                            param.span,
                            param_cg,
                            args_layout,
                            raw_arg,
                            0,
                        )?
                    }
                } else {
                    // The non-flattened lambda invoke-args tuple is `(env, explicit_params...)`:
                    // the env parameter always occupies exactly one leading source slot
                    // (component 0), even when it is `Unit` (env_component_count == 0) and thus
                    // elided. Explicit param at mir index `index` therefore maps 1:1 to source
                    // component `index`.
                    self.bind_direct_param_from_component(
                        entry_layout.symbol_name(),
                        param.span,
                        param_cg,
                        args_layout,
                        raw_arg,
                        index,
                    )?
                }
            } else if self.mir_fun.params.len() == 1
                && param.ty == entry_layout.invoke_args_tuple_ty()
                && matches!(param_cg, CgTy::Tuple(_))
            {
                self.bind_direct_tuple_param_from_components(
                    entry_layout.symbol_name(),
                    param.span,
                    param.ty,
                    param_cg,
                    args_layout,
                    raw_arg,
                    0,
                )?
            } else {
                self.bind_direct_param_from_component(
                    entry_layout.symbol_name(),
                    param.span,
                    param_cg,
                    args_layout,
                    raw_arg,
                    index,
                )?
            };
            let _ = self.store_local_value(param.span, param.local, value)?;
        }
        Ok(())
    }

    pub(super) fn lambda_env_component_count(
        &self,
        entry_layout: &CallableEntryLayout<'ctx>,
    ) -> Result<Option<usize>, LlvmEmitError> {
        if !self.mir_fun.name.starts_with("$lambda") {
            return Ok(None);
        }
        let Some(env_param) = self.mir_fun.params.first() else {
            return Ok(None);
        };
        match self.source_types.kind(env_param.ty) {
            TypeKind::Value(ValueTypeKind::Unit) => Ok(Some(0)),
            TypeKind::Value(ValueTypeKind::Tuple(elements))
                if self.mir_fun.params.len() == 1
                    && env_param.ty == entry_layout.invoke_args_tuple_ty() =>
            {
                Ok(Some(elements.len()))
            }
            TypeKind::Value(ValueTypeKind::Tuple(_)) => Ok(Some(1)),
            _ => Err(frontend_error(format!(
                "direct entry `{}` 的 lambda env 参数不是 Unit 或 tuple",
                self.mir_fun.fqn,
            ))),
        }
    }

    pub(super) fn bind_direct_param_from_component(
        &mut self,
        entry_symbol: &str,
        span: crate::span::Span,
        param_cg: CgTy,
        args_layout: &SourceAbiLayout<'ctx>,
        raw_arg: Option<BasicValueEnum<'ctx>>,
        source_index: usize,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match self.extract_direct_arg_component(entry_symbol, args_layout, raw_arg, source_index)? {
            Some(raw) => self.codegen.cg_value_from_loaded(span, param_cg, raw),
            None => self.codegen.default_value(span, param_cg),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn bind_direct_tuple_param_from_components(
        &mut self,
        entry_symbol: &str,
        span: crate::span::Span,
        tuple_ty: TypeId,
        tuple_cg: CgTy,
        args_layout: &SourceAbiLayout<'ctx>,
        raw_arg: Option<BasicValueEnum<'ctx>>,
        source_start: usize,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = self.source_types.kind(tuple_ty)
        else {
            return Err(frontend_error(format!(
                "direct entry `{entry_symbol}` 不能从 components 组装非 tuple 参数 t{}",
                tuple_ty.as_u32(),
            )));
        };
        let BasicTypeEnum::StructType(tuple_struct_ty) =
            self.codegen.llvm_basic_type_of(span, tuple_cg)?
        else {
            return Err(frontend_error(format!(
                "direct entry `{entry_symbol}` tuple 参数 t{} 的 LLVM type 不是 struct",
                tuple_ty.as_u32(),
            )));
        };
        let mut aggregate = tuple_struct_ty.get_undef();
        for (offset, elem_ty) in elements.iter().enumerate() {
            let elem_cg = self
                .codegen
                .cg_ty_of_mir_type(self.source_types, *elem_ty)
                .unwrap_or_else(|| {
                    panic!(
                        "bind_direct_tuple_param_from_components: direct entry ABI verifier accepted non-codegen tuple element type at {span:?}"
                    )
                });
            let raw = match self.extract_direct_arg_component(
                entry_symbol,
                args_layout,
                raw_arg,
                source_start + offset,
            )? {
                Some(raw) => raw,
                None => {
                    let llvm_ty = self.codegen.llvm_basic_type_of(span, elem_cg)?;
                    self.codegen.zero_initializer_for_basic_type(llvm_ty)
                }
            };
            aggregate = self
                .codegen
                .builder
                .build_insert_value(
                    aggregate,
                    raw,
                    offset as u32,
                    &format!("direct_tuple_param{source_start}_{offset}"),
                )?
                .into_struct_value();
        }
        self.codegen
            .cg_value_from_loaded(span, tuple_cg, aggregate.into())
    }

    pub(super) fn extract_direct_arg_component(
        &mut self,
        entry_symbol: &str,
        args_layout: &SourceAbiLayout<'ctx>,
        raw_arg: Option<BasicValueEnum<'ctx>>,
        source_index: usize,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        match args_layout.kind() {
            SourceAbiLayoutKind::Scalar if source_index == 0 => {
                if args_layout.abi().is_elided() {
                    Ok(None)
                } else {
                    raw_arg.map(Some).ok_or_else(|| {
                        frontend_error(format!(
                            "direct entry `{entry_symbol}` scalar args ABI 缺少 raw 参数"
                        ))
                    })
                }
            }
            SourceAbiLayoutKind::Scalar => Err(frontend_error(format!(
                "direct entry `{entry_symbol}` scalar args ABI 不能绑定 source component {source_index}；不能用默认值掩盖 contract 漂移"
            ))),
            SourceAbiLayoutKind::Tuple => {
                let field = args_layout.field(source_index).ok_or_else(|| {
                    frontend_error(format!(
                        "direct entry `{entry_symbol}` args tuple ABI 缺少 source component {source_index} 的 field；不能用默认值掩盖 contract 漂移"
                    ))
                })?;
                if field.is_elided() {
                    return Ok(None);
                }
                let tuple = raw_arg.ok_or_else(|| {
                    frontend_error(format!(
                        "direct entry `{entry_symbol}` args tuple ABI 缺少 raw 参数"
                    ))
                })?;
                let struct_value = tuple.into_struct_value();
                let raw = self.codegen.builder.build_extract_value(
                    struct_value,
                    field
                        .abi_field_index()
                        .expect("non-elided field has ABI index"),
                    &format!("arg_field{source_index}"),
                )?;
                Ok(Some(raw))
            }
        }
    }

    pub(super) fn lower_state_source_slices(
        &mut self,
        state: &LateLoweredState,
    ) -> Result<(), LlvmEmitError> {
        for slice in state.source_slices() {
            let block = self
                .body
                .blocks
                .get(slice.block_id().as_u32() as usize)
                .unwrap_or_else(|| {
                    panic!(
                        "lower_state_source_slices: late-lowered verifier accepted missing source block bb{} in `{}`",
                        slice.block_id().as_u32(),
                        self.mir_fun.fqn
                    )
                });
            for stmt_index in slice.start_statement_index()..slice.end_statement_index() {
                let stmt = block.stmts.get(stmt_index as usize).unwrap_or_else(|| {
                    panic!(
                        "lower_state_source_slices: late-lowered verifier accepted missing source statement bb{} stmt{} in `{}`",
                        slice.block_id().as_u32(),
                        stmt_index,
                        self.mir_fun.fqn
                    )
                });
                let classification = self
                    .callable
                    .source_statement_classification(*slice, stmt_index)
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "source-slice statement bb{} stmt{} 缺少 published classification",
                            slice.block_id().as_u32(),
                            stmt_index,
                        ))
                    })?;
                match classification.kind() {
                    LateLoweredSourceStatementClassificationKind::EffectNeutralValue
                    | LateLoweredSourceStatementClassificationKind::DynamicInvokeCall { .. }
                    | LateLoweredSourceStatementClassificationKind::BoundaryResultInjection { .. }
                    | LateLoweredSourceStatementClassificationKind::CompletionPayloadInjection { .. } => {
                        if !self.lower_published_call_statement(stmt)? {
                            self.lower_effect_neutral_statement(stmt)?;
                        }
                    }
                    LateLoweredSourceStatementClassificationKind::BoundaryConsumedAnchor { .. }
                    | LateLoweredSourceStatementClassificationKind::ResumePayloadInjection { .. }
                    | LateLoweredSourceStatementClassificationKind::HandleSyntheticCarrierBinder { .. }
                    | LateLoweredSourceStatementClassificationKind::ElidedUnreachable => {}
                    LateLoweredSourceStatementClassificationKind::Unsupported { reason } => {
                        return Err(frontend_error(format!(
                            "source-slice statement bb{} stmt{} classified unsupported: {reason}",
                            slice.block_id().as_u32(),
                            stmt_index,
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn lower_state_terminator(
        &mut self,
        state: &LateLoweredState,
    ) -> Result<(), LlvmEmitError> {
        if self.current_block_is_terminated() {
            return Ok(());
        }
        match state.terminator() {
            LateLoweredStateTerminator::Goto { target } => {
                if self.try_return_handle_completion_from_resume_entry(state, *target)?
                    || self.try_return_wrapper_complete_from_handle_completion(state, *target)?
                    || self.try_route_handle_completion_goto(state, *target)?
                {
                    Ok(())
                } else {
                    self.branch_to_state(*target)
                }
            }
            LateLoweredStateTerminator::Branch {
                cond_local,
                then_state,
                else_state,
            } => {
                let cond = self
                    .load_local_value(self.mir_fun.span, *cond_local)?
                    .as_bool()
                    .unwrap_or_else(|| {
                        panic!(
                            "lower_state_terminator: late-lowered verifier accepted non-bool state branch condition local{} in `{}`",
                            cond_local.as_u32(),
                            self.mir_fun.fqn
                        )
                    });
                self.codegen.builder.build_conditional_branch(
                    cond,
                    self.state_block(*then_state)?,
                    self.state_block(*else_state)?,
                )?;
                Ok(())
            }
            LateLoweredStateTerminator::Return {
                payload_source: _,
                complete_state,
            } => {
                let binding = self
                    .abi
                    .completion_payload_binding_for_state(self.abi_step_schema, state.state_id())?;
                let _ = self
                    .abi
                    .completion_payload_binding_layout(self.abi_step_schema, binding.binding())?;
                let payload_source = binding.payload_source();
                let payload = self
                    .lower_completion_payload_as(
                        payload_source,
                        self.step_layout.complete_variant().payload_source_ty(),
                    )
                    .map_err(|err| {
                        frontend_error(format!(
                            "return state st{} completion payload {:?} lowering failed: {err}",
                            state.state_id().as_u32(),
                            payload_source,
                        ))
                    })?;
                if payload.is_none() && !self.step_layout.complete_variant().payload_is_elided() {
                    return Err(frontend_error(format!(
                        "return state st{} payload source {:?} produced no payload for non-elided Complete layout {}",
                        state.state_id().as_u32(),
                        payload_source,
                        self.step_layout.complete_variant().payload_anchor_name()
                    )));
                }
                match self.return_mode {
                    CallableReturnMode::Step => {
                        let step = self
                            .codegen
                            .build_step_complete(self.step_layout, payload)
                            .map_err(|err| {
                                frontend_error(format!(
                                    "return state st{} build Complete step failed: {err}",
                                    state.state_id().as_u32(),
                                ))
                            })?;
                        self.cleanup_handle_contexts_before_function_return(
                            state.state_id(),
                            *complete_state,
                        )?;
                        self.return_step(step).map_err(|err| {
                            frontend_error(format!(
                                "return state st{} return Step failed: {err}",
                                state.state_id().as_u32(),
                            ))
                        })
                    }
                    CallableReturnMode::EffectOutcome => {
                        let payload =
                            self.complete_payload_or_default(self.step_layout, payload)?;
                        let complete_transport = self.encode_effect_transport_parts(
                            self.step_layout.complete_variant().payload_source_ty(),
                            payload,
                            "return_effect_outcome",
                        )?;
                        let zero_signal = self.codegen.build_effect_signal(
                            self.codegen.context.i32_type().const_zero(),
                            self.codegen.context.i32_type().const_zero(),
                            self.zero_transport_parts(),
                            self.codegen.llvm_gc_i8_ptr_type().const_null(),
                        )?;
                        let outcome = self.codegen.build_effect_outcome(
                            EffectOutcomeTag::Complete,
                            complete_transport,
                            zero_signal,
                        )?;
                        self.cleanup_handle_contexts_before_function_return(
                            state.state_id(),
                            *complete_state,
                        )?;
                        self.emit_effect_outcome_return(outcome).map_err(|err| {
                            frontend_error(format!(
                                "return state st{} return EffectOutcome failed: {err}",
                                state.state_id().as_u32(),
                            ))
                        })
                    }
                    CallableReturnMode::Plain { declared_return_cg } => {
                        let value = match payload {
                            Some(raw) => self.codegen.cg_value_from_loaded(
                                self.mir_fun.span,
                                declared_return_cg,
                                raw,
                            )?,
                            None => self
                                .codegen
                                .default_value(self.mir_fun.span, declared_return_cg)?,
                        };
                        let value = self.codegen.coerce_value(
                            self.mir_fun.span,
                            value,
                            declared_return_cg,
                        )?;
                        self.cleanup_handle_contexts_before_function_return(
                            state.state_id(),
                            *complete_state,
                        )?;
                        self.codegen.finish_function_return_path(
                            self.mir_fun.span,
                            declared_return_cg,
                            value,
                        )
                    }
                }
            }
            LateLoweredStateTerminator::Suspend { boundary_ids, .. } => {
                self.lower_suspend(state, boundary_ids)
            }
            LateLoweredStateTerminator::HandleDispatch {
                site_id,
                contract,
                body_state,
                ..
            } => {
                let _ =
                    self.abi
                        .handle_dispatch_layout(self.abi_step_schema, *site_id, contract)?;
                self.enter_handle_dispatch_effect_ctx(*site_id, contract)?;
                self.branch_to_state(*body_state)
            }
            LateLoweredStateTerminator::LocalRuntimeError {
                payload_tuple_ty,
                terminal_action,
            } => {
                let runtime = self.local_runtime_error_runtime_for_target_state(
                    state.state_id(),
                    *payload_tuple_ty,
                    *terminal_action,
                )?;
                let payload =
                    self.lower_runtime_error_boundary_payload(runtime.payload_tuple_ty)?;
                self.emit_local_runtime_error_terminal(&runtime, payload)
            }
            LateLoweredStateTerminator::Unreachable => {
                self.codegen.builder.build_unreachable()?;
                Ok(())
            }
            LateLoweredStateTerminator::ResumeUnwind => self.lower_resume_unwind_terminator(state),
            LateLoweredStateTerminator::Abandon => self.lower_abandon_terminator(state),
        }
    }

    pub(super) fn lower_resume_unwind_terminator(
        &mut self,
        state: &LateLoweredState,
    ) -> Result<(), LlvmEmitError> {
        self.verify_resume_unwind_contract(state)?;
        // The verified cleanup route is consumed by the surrounding HandleDispatch
        // pending-completion contract; reaching the terminal directly would mean the
        // upstream handoff lost the unwind carrier/origin route.
        self.codegen.builder.build_unreachable()?;
        Ok(())
    }

    pub(super) fn lower_abandon_terminator(
        &mut self,
        state: &LateLoweredState,
    ) -> Result<(), LlvmEmitError> {
        self.verify_abandon_contract(state)?;
        // The drop state is entered by the continuation runtime/GC contract; no
        // remaining source-level computation is resumed from this block.
        self.codegen.builder.build_unreachable()?;
        Ok(())
    }

    pub(super) fn lower_suspend(
        &mut self,
        state: &LateLoweredState,
        boundary_ids: &[BoundaryId],
    ) -> Result<(), LlvmEmitError> {
        let boundary = boundary_ids
            .iter()
            .filter_map(|id| self.callable.boundary_map().boundary(*id))
            .find(|boundary| {
                !matches!(
                    boundary.lowering(),
                    Some(LateLoweredBoundaryLowering::RuntimeError(_))
                )
            })
            .or_else(|| {
                boundary_ids.iter().find_map(|id| {
                    self.callable
                        .boundary_map()
                        .boundary(*id)
                        .filter(|boundary| {
                            matches!(
                                boundary.lowering(),
                                Some(LateLoweredBoundaryLowering::RuntimeError(_))
                            )
                        })
                })
            })
            .ok_or_else(|| {
                frontend_error(format!(
                    "suspend state st{} 缺少可 lower 的 primary boundary",
                    state.state_id().as_u32()
                ))
            })?;
        match boundary.lowering().ok_or_else(|| {
            frontend_error(format!(
                "boundary bd{} 缺少 published lowering",
                boundary.boundary_id().as_u32()
            ))
        })? {
            LateLoweredBoundaryLowering::Call(lowering) => {
                let source = boundary_site(boundary, "Call")?;
                let _ = self.abi.call_boundary_operand_layout(
                    self.abi_step_schema,
                    source,
                    lowering.operand_contract(),
                )?;
                let args_payload = self.pack_sources(
                    lowering.facts().invoke_args_tuple_ty(),
                    lowering.operand_contract().arg_sources(),
                    "call_args",
                )?;
                let target =
                    self.abi
                        .call_target_layout(self.abi_step_schema, source, lowering.facts())?;
                let (step, callee_step_schema) = match target {
                    CallTargetQuery::KnownInstance(layout) => (
                        self.emit_known_instance_call_step(
                            source,
                            layout.direct_entry(),
                            args_payload,
                        )?,
                        layout.step_schema(),
                    ),
                    CallTargetQuery::DynamicInvoke(layout) => {
                        let carrier_source = lowering
                            .operand_contract()
                            .carrier_source()
                            .ok_or_else(|| {
                                frontend_error(format!(
                                    "dynamic call boundary site {} 缺少 published carrier source",
                                    source.as_u32()
                                ))
                            })?;
                        let carrier = self
                            .lower_operand_source(carrier_source)?
                            .value
                            .ok_or_else(|| {
                                frontend_error(format!(
                                    "dynamic call boundary site {} carrier source 缺少可传递值",
                                    source.as_u32()
                                ))
                            })?;
                        (
                            self.emit_dynamic_invoke_step(layout, carrier, args_payload)?,
                            layout.return_step_schema(),
                        )
                    }
                };
                self.dispatch_boundary_step(
                    boundary,
                    callee_step_schema,
                    step,
                    lowering.dispatch(),
                    Some(lowering),
                    Some(lowering.continuation_compositions()),
                )
            }
            LateLoweredBoundaryLowering::ClassCtor(lowering) => {
                self.lower_class_ctor_boundary(boundary, lowering)
            }
            LateLoweredBoundaryLowering::Perform(lowering) => {
                let source = boundary_site(boundary, "Perform")?;
                let _ = self.abi.perform_boundary_operand_layout(
                    self.abi_step_schema,
                    source,
                    lowering.operand_contract(),
                )?;
                let payload = self.pack_sources(
                    lowering.emitted_step().payload_tuple_ty(),
                    lowering.operand_contract().payload_sources(),
                    "perform_payload",
                )?;
                self.emit_or_consume_outward_case(
                    boundary,
                    lowering.emitted_step().case_tag(),
                    payload,
                    lowering.emitted_step().payload_tuple_ty(),
                    None,
                    None,
                )
            }
            LateLoweredBoundaryLowering::Resume(lowering) => {
                let source = boundary_site(boundary, "Resume")?;
                let _ = self.abi.resume_boundary_operand_layout(
                    self.abi_step_schema,
                    source,
                    lowering.operand_contract(),
                )?;
                let surface = self
                    .abi
                    .surface_resume_layout(lowering.facts().continuation_schema())
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "resume site {} 缺少 continuation schema k{} surface ABI",
                            source.as_u32(),
                            lowering.facts().continuation_schema().as_u32()
                        ))
                    })?;
                let cont_value =
                    self.lower_operand_source(lowering.operand_contract().continuation_source())?;
                let cont_ptr = cont_value.value.ok_or_else(|| {
                    frontend_error(format!(
                        "resume site {} continuation source 被 elide",
                        source.as_u32()
                    ))
                })?;
                let BasicValueEnum::PointerValue(cont_ptr) = cont_ptr else {
                    return Err(frontend_error(format!(
                        "resume site {} continuation source 不是 pointer",
                        source.as_u32()
                    )));
                };
                let args_payload = self.pack_sources(
                    surface.resume_tuple_ty(),
                    lowering.operand_contract().arg_sources(),
                    "resume_args",
                )?;
                self.sync_frame_slots_from_locals()?;
                if self.should_use_task_transport_dynamic_resume(source, surface, lowering)?
                    && self.lower_task_transport_dynamic_resume_boundary(
                        boundary,
                        lowering,
                        surface,
                        cont_ptr,
                        args_payload,
                    )?
                {
                    return Ok(());
                }
                let callee = self.codegen.function(surface.symbol_name())?;
                let mut args = vec![cont_ptr.into()];
                if !surface.resume_payload_abi().is_elided() {
                    args.push(
                        args_payload
                            .ok_or_else(|| {
                                frontend_error(format!(
                                    "resume site {} 需要 non-elided payload",
                                    source.as_u32()
                                ))
                            })?
                            .into(),
                    );
                }
                let call = self.codegen.build_call_preserving_gc_local_roots(
                    self.mir_fun.span,
                    callee,
                    &args,
                    "resume_step",
                )?;
                let step = call.try_as_basic_value().basic().ok_or_else(|| {
                    frontend_error("resume boundary callee 未返回 Step_F".to_string())
                })?;
                self.dispatch_boundary_step(
                    boundary,
                    lowering.facts().out_step_schema(),
                    step,
                    lowering.dispatch(),
                    None,
                    Some(lowering.continuation_compositions()),
                )
            }
            LateLoweredBoundaryLowering::RuntimeError(lowering) => {
                self.lower_runtime_error_boundary(boundary, lowering)
            }
            LateLoweredBoundaryLowering::Handle(lowering) => {
                self.lower_handle_boundary(boundary, lowering)
            }
        }
    }

    pub(super) fn sync_frame_slots_from_locals(&mut self) -> Result<(), LlvmEmitError> {
        for slot in self.callable.frame_schema().slots() {
            if let Some(local) = frame_slot_local(slot.kind()) {
                self.store_local_to_frame_slot(local, slot.slot_id())?;
            }
        }
        Ok(())
    }

    pub(super) fn restore_frame_slots_to_locals(&mut self) -> Result<(), LlvmEmitError> {
        for slot in self.callable.frame_schema().slots() {
            let Some(local) = frame_slot_local(slot.kind()) else {
                continue;
            };
            let local_slot = self
                .codegen
                .mir_local_slot(self.mir_fun.span, &self.slots, local)?;
            if local_slot.cg_ty == CgTy::Unit || local_slot.cg_ty == CgTy::Never {
                continue;
            }
            let field_index = self
                .frame_layout
                .field_index_for_slot(slot.slot_id())
                .ok_or_else(|| {
                    frontend_error(format!(
                        "frame layout 缺少 slot{} field index",
                        slot.slot_id().as_u32()
                    ))
                })?;
            let field_ptr = self.frame_field_ptr(field_index, "frame_slot_load_gep")?;
            let loaded = self.codegen.builder.build_load(
                self.codegen
                    .llvm_basic_type_of(self.mir_fun.span, local_slot.cg_ty)?,
                field_ptr,
                "frame_slot_load",
            )?;
            let _ = self.store_loaded_raw_local(self.mir_fun.span, local, loaded)?;
        }
        Ok(())
    }

    pub(super) fn store_local_to_frame_slot(
        &mut self,
        local: LocalId,
        frame_slot: crate::effect_lowered::ir::FrameSlotId,
    ) -> Result<(), LlvmEmitError> {
        let local_slot = self
            .codegen
            .mir_local_slot(self.mir_fun.span, &self.slots, local)?;
        if local_slot.cg_ty == CgTy::Unit || local_slot.cg_ty == CgTy::Never {
            return Ok(());
        }
        let field_index = self
            .frame_layout
            .field_index_for_slot(frame_slot)
            .ok_or_else(|| {
                frontend_error(format!(
                    "frame layout 缺少 slot{} field index",
                    frame_slot.as_u32()
                ))
            })?;
        let field_ptr = self.frame_field_ptr(field_index, "frame_slot_store_gep")?;
        let value = self.load_local_value(self.mir_fun.span, local)?;
        if let Some(raw) = value.value {
            self.codegen.store_gc_aware_value(
                self.mir_fun.span,
                field_ptr,
                raw,
                "frame_slot_store",
            )?;
        }
        Ok(())
    }

    pub(super) fn store_gc_ref_to_local(
        &mut self,
        local: LocalId,
        value: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let slot = self
            .codegen
            .mir_local_slot(self.mir_fun.span, &self.slots, local)?;
        let cg = CgValue {
            ty: slot.cg_ty,
            value: Some(value.into()),
        };
        let _ = self.store_local_value(self.mir_fun.span, local, cg)?;
        Ok(())
    }

    pub(super) fn unpack_payload_field(
        &mut self,
        payload: Option<BasicValueEnum<'ctx>>,
        payload_ty: TypeId,
        ordinal: u32,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        self.value_primitives()
            .unpack_payload_field(payload, payload_ty, ordinal)
    }

    pub(super) fn branch_to_state(&mut self, state_id: StateId) -> Result<(), LlvmEmitError> {
        if self.current_block_is_terminated() {
            return Ok(());
        }
        let target = self.state_block(state_id)?;
        self.codegen.builder.build_unconditional_branch(target)?;
        Ok(())
    }

    pub(super) fn state_block(&self, state_id: StateId) -> Result<BasicBlock<'ctx>, LlvmEmitError> {
        self.state_blocks.get(&state_id).copied().ok_or_else(|| {
            frontend_error(format!(
                "state graph 缺少 StateId st{} 的 LLVM block",
                state_id.as_u32()
            ))
        })
    }

    pub(super) fn current_block_is_terminated(&self) -> bool {
        self.codegen
            .builder
            .get_insert_block()
            .is_some_and(|bb| bb.get_terminator().is_some())
    }

    pub(super) fn seal_unterminated_state_blocks_as_unreachable(
        &mut self,
    ) -> Result<(), LlvmEmitError> {
        for bb in self.state_blocks.values().copied() {
            if bb.get_terminator().is_some() {
                continue;
            }
            self.codegen.builder.position_at_end(bb);
            self.codegen.builder.build_unreachable()?;
        }
        Ok(())
    }
}
