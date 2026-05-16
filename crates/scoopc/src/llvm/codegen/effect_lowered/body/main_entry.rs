//! Top-level body codegen entry: orchestrates per-callable lowering and emits the published main exit-code wrapper plus the plain (effect-neutral) callable shells.

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn codegen_program_bodies(
        &mut self,
        program: &'a LateLoweredProgram,
        abi_program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        pass_view: &'a mir::MaterializedMirPassView<'a>,
        abi_source_types: &'a TypeStore,
        abi_pass_view: &'a mir::MaterializedMirPassView<'a>,
        abi: &ProgramAbiQuery<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        for callable in program.callables() {
            let mut child = self.fresh_child_codegen();
            if callable.plain_abi().is_some() {
                child.codegen_plain_callable_entry(
                    program,
                    source_types,
                    pass_view,
                    abi,
                    callable,
                )?;
            } else {
                child.codegen_callable_entries(
                    program,
                    source_types,
                    pass_view,
                    abi,
                    callable,
                )?;
            }
        }
        let primary_roots = program
            .callables()
            .iter()
            .map(|callable| callable.root_fqn())
            .collect::<HashSet<_>>();
        for callable in abi_program.callables() {
            if primary_roots.contains(callable.root_fqn()) {
                continue;
            }
            let mut child = self.fresh_child_codegen();
            if callable.plain_abi().is_some() {
                child.codegen_plain_callable_entry(
                    abi_program,
                    abi_source_types,
                    abi_pass_view,
                    abi,
                    callable,
                )?;
            } else {
                child.codegen_callable_entries(
                    abi_program,
                    abi_source_types,
                    abi_pass_view,
                    abi,
                    callable,
                )?;
            }
        }
        for (kind, carrier_fqn, target) in abi.callable_carrier_target_layouts() {
            let mut child = self.fresh_child_codegen();
            child.codegen_callable_carrier_entry_shell(
                kind,
                carrier_fqn,
                target,
                abi_source_types,
                abi_pass_view,
                abi,
            )?;
        }
        for interface in abi_program.resume_packings() {
            let packing = abi
                .resume_packing_layout(interface.interface_id())
                .ok_or_else(|| {
                    frontend_error(format!(
                        "body lowering 缺少 resume packing ri{} 的 ABI layout",
                        interface.interface_id().as_u32()
                    ))
                })?;
            for method in interface.methods() {
                let method_layout = packing.method(method.case_tag()).ok_or_else(|| {
                    frontend_error(format!(
                        "body lowering 缺少 resume packing ri{} case c{} method layout",
                        interface.interface_id().as_u32(),
                        method.case_tag().as_u32()
                    ))
                })?;
                if !resume_packing_method_is_reachable(
                    abi_program,
                    interface.interface_id(),
                    method.case_tag(),
                ) {
                    let mut child = self.fresh_child_codegen();
                    child.codegen_unreachable_resume_method(
                        method_layout.symbol_name(),
                        method_layout.llvm_ty(),
                    )?;
                    continue;
                }
                let callable = abi_program
                .callables()
                .iter()
                    .find(|callable| callable.body_step_schema() == Some(method.out_step_schema()))
                    .ok_or_else(|| frontend_error(format!(
                        "body lowering 缺少 resume method case c{} 的 owner step schema s{} callable",
                        method.case_tag().as_u32(),
                        method.out_step_schema().as_u32()
                    )))?;
                let mut child = self.fresh_child_codegen();
                child.codegen_resume_method(
                    abi_program,
                    abi_source_types,
                    abi_pass_view,
                    abi,
                    callable,
                    method_layout.symbol_name(),
                    method_layout.llvm_ty(),
                    method.case_tag(),
                    method.resume_tuple_ty(),
                )?;
            }
        }
        for entry in program.surface_resume_dispatch_inventory() {
            let surface = abi
                .surface_resume_layout(entry.continuation_schema())
                .ok_or_else(|| frontend_error(format!(
                    "body lowering 缺少 continuation schema k{} 的 surface-resume layout",
                    entry.continuation_schema().as_u32()
                )))?;
            let mut child = self.fresh_child_codegen();
            child.codegen_surface_resume(program, abi, surface)?;
        }
        for surface in abi.surface_resume_layouts() {
            let mut child = self.fresh_child_codegen();
            child.codegen_surface_resume_outcome(abi, surface)?;
        }
        for dispatch in abi.surface_resume_dispatch_layouts() {
            let surface = abi
                .surface_resume_layout(dispatch.continuation_schema())
                .ok_or_else(|| {
                    frontend_error(format!(
                        "body lowering 缺少 ABI continuation schema k{} 的 surface-resume layout",
                        dispatch.continuation_schema().as_u32()
                    ))
                })?;
            let mut child = self.fresh_child_codegen();
            child.codegen_continuation_drive_outcome(abi, surface)?;
            for target in dispatch.target().owner_trampolines() {
                let mut child = self.fresh_child_codegen();
                child.codegen_surface_resume_owner_outcome(
                    abi_program,
                    abi_source_types,
                    abi_pass_view,
                    abi,
                    surface,
                    target,
                )?;
                let mut child = self.fresh_child_codegen();
                child.codegen_continuation_drive_owner_outcome(
                    abi_program,
                    abi_source_types,
                    abi_pass_view,
                    abi,
                    surface,
                    target,
                )?;
                let mut child = self.fresh_child_codegen();
                child.codegen_surface_resume_owner_trampoline(
                    abi_program,
                    abi_source_types,
                    abi_pass_view,
                    abi,
                    surface,
                    target,
                )?;
            }
            let mut child = self.fresh_child_codegen();
            child.codegen_surface_resume(abi_program, abi, surface)?;
        }
        for callable in program.callables() {
            if !callable.has_control_body() {
                continue;
            }
            for boundary in callable.boundary_map().entries() {
                let compositions = match boundary.lowering() {
                    Some(LateLoweredBoundaryLowering::Call(lowering)) => {
                        lowering.continuation_compositions()
                    }
                    Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                        lowering.continuation_compositions()
                    }
                    _ => continue,
                };
                for composition in compositions {
                    let continuation_schema = composition
                        .callee_continuation_contract()
                        .continuation_schema();
                    let surface = abi.surface_resume_layout(continuation_schema).ok_or_else(|| {
                        frontend_error(format!(
                            "body lowering 缺少 dynamic continuation schema k{} 的 surface-resume layout",
                            continuation_schema.as_u32()
                        ))
                    })?;
                    let Some(function) = self.module.get_function(surface.symbol_name()) else {
                        continue;
                    };
                    if function.count_basic_blocks() > 0 {
                        continue;
                    }
                    let mut child = self.fresh_child_codegen();
                    child.codegen_dynamic_surface_resume_adapter(program, abi, surface)?;
                }
            }
        }
        Ok(())
    }

    /// Emits the C `main` exit path through the stage-owned direct-entry ABI.
    pub(crate) fn codegen_stage_main_exit_code(
        &mut self,
        hir_main: &crate::hir::FunDecl,
        entry_argv_array: Option<PointerValue<'ctx>>,
        program: &LateLoweredProgram,
        abi: &ProgramAbiQuery<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let mut entry_callables = program
            .callables()
            .iter()
            .filter(|callable| callable.root_fqn() == hir_main.fqn);
        let callable = entry_callables.next().ok_or_else(|| {
            frontend_error(format!(
                "LLVM main wrapper 缺少入口 `{}` 的 callable body",
                hir_main.fqn
            ))
        })?;
        if entry_callables.next().is_some() {
            return Err(frontend_error(format!(
                "LLVM main wrapper 发现入口 `{}` 存在多个 callable body version；必须通过 body version key 明确选择入口 shell",
                hir_main.fqn
            )));
        }
        if callable.plain_abi().is_some() {
            return self.codegen_plain_main_exit_code(
                hir_main,
                entry_argv_array,
                callable,
                abi,
            );
        }
        if entry_argv_array.is_some() {
            return Err(frontend_error(
                "LLVM effect-step main wrapper 尚未发布 Array<String> argv Step ABI"
                    .to_string(),
            ));
        }

        let layout = abi.callable_layout_by_version_key(callable.body_version_key())?;
        let direct = self.function(layout.direct_entry().symbol_name())?;
        let args = Vec::<BasicMetadataValueEnum<'ctx>>::new();
        if !layout.direct_entry().args_abi().is_elided() {
            return Err(frontend_error(format!(
                "LLVM main wrapper 入口 `{}` 的 direct entry args ABI 非 elided；Array<String> argv tuple ABI 尚未发布或 entry contract 漂移",
                hir_main.fqn
            )));
        }
        let call = self
            .builder
            .build_call(direct, &args, "main_step")?;
        let step = call.try_as_basic_value().basic().ok_or_else(|| {
            frontend_error("main direct entry 未返回 Step_F".to_string())
        })?;
        let step_layout = abi.step_layout(layout.step_schema()).ok_or_else(|| {
            frontend_error(format!(
                "LLVM main wrapper 缺少入口 step schema s{} layout",
                layout.step_schema().as_u32()
            ))
        })?;
        let tag = self.extract_step_tag(step_layout, step)?;
        let ok_bb = self
            .context
            .append_basic_block(self.current_function()?, "main_complete");
        let bad_bb = self
            .context
            .append_basic_block(self.current_function()?, "main_unhandled");
        let done_bb = self
            .context
            .append_basic_block(self.current_function()?, "main_done");
        let is_complete = self.builder.build_int_compare(
            inkwell::IntPredicate::EQ,
            tag,
            self.context.i32_type().const_int(STEP_TAG_COMPLETE, false),
            "main_is_complete",
        )?;
        self.builder
            .build_conditional_branch(is_complete, ok_bb, bad_bb)?;

        self.builder.position_at_end(bad_bb);
        let unhandled_exit = if step_layout.cases().is_empty() {
            self.builder.build_unreachable()?;
            None
        } else {
            // The process ABI cannot return `Step_F`; an escaped outward case is a
            // terminal program result at this boundary.
            let exit = self
                .context
                .i32_type()
                .const_int(MAIN_UNHANDLED_EXIT_CODE, false);
            self.builder.build_unconditional_branch(done_bb)?;
            Some(exit)
        };

        self.builder.position_at_end(ok_bb);
        let exit_value = match self.cg_ty_of(hir_main.return_ty) {
            Some(CgTy::Unit) => self.context.i32_type().const_zero(),
            Some(CgTy::Int(_)) => {
                let payload = self.extract_step_payload(
                    step_layout,
                    step,
                    step_layout.complete_variant(),
                    "main_complete_payload",
                )?;
                match payload {
                    Some(BasicValueEnum::IntValue(value)) => {
                        self.builder.build_int_truncate_or_bit_cast(
                            value,
                            self.context.i32_type(),
                            "main_exit_i32",
                        )?
                    }
                    Some(_) => {
                        return Err(frontend_error(
                            "main Complete payload 不是整数值".to_string(),
                        ));
                    }
                    None => self.context.i32_type().const_zero(),
                }
            }
            _ => {
                return Err(frontend_error(format!(
                    "main wrapper 不支持入口 `{}` 的返回类型",
                    hir_main.fqn
                )));
            }
        };
        self.builder.build_unconditional_branch(done_bb)?;

        self.builder.position_at_end(done_bb);
        let phi = self
            .builder
            .build_phi(self.context.i32_type(), "main_exit")?;
        phi.add_incoming(&[(&exit_value, ok_bb)]);
        if let Some(exit) = unhandled_exit {
            phi.add_incoming(&[(&exit, bad_bb)]);
        }
        Ok(phi.as_basic_value().into_int_value())
    }

    pub(super) fn codegen_plain_main_exit_code(
        &mut self,
        hir_main: &crate::hir::FunDecl,
        entry_argv_array: Option<PointerValue<'ctx>>,
        callable: &LateLoweredCallable,
        abi: &ProgramAbiQuery<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let layout = abi.plain_callable_layout_by_version_key(callable.body_version_key())?;
        if layout.root_fqn() != callable.root_fqn() {
            return Err(frontend_error(format!(
                "plain main `{}` 的 ABI layout root 漂移：layout=`{}`",
                callable.root_fqn(),
                layout.root_fqn(),
            )));
        }
        let entry = layout.direct_entry();
        let args = match (hir_main.params.as_slice(), entry_argv_array) {
            ([], None) if entry.param_count() == 0 => Vec::new(),
            ([_param], Some(argv_array)) if entry.param_count() == 1 => vec![argv_array.into()],
            ([], Some(_)) => {
                return Err(frontend_error(format!(
                    "plain main `{}` 没有 source argv 参数，但 wrapper 收到了 argv array",
                    hir_main.fqn,
                )));
            }
            ([_], None) => {
                return Err(frontend_error(format!(
                    "plain main `{}` 需要 argv array，但 wrapper 未收到入口 argv",
                    hir_main.fqn,
                )));
            }
            _ => {
                return Err(frontend_error(format!(
                    "plain main `{}` argv ABI 漂移：source_params={} direct_params={}",
                    hir_main.fqn,
                    hir_main.params.len(),
                    entry.param_count(),
                )));
            }
        };
        let direct = self.function(entry.symbol_name())?;
        let call = self
            .builder
            .build_call(direct, &args, "plain_main")?;
        match self.cg_ty_of(hir_main.return_ty) {
            Some(CgTy::Unit) => Ok(self.context.i32_type().const_zero()),
            Some(CgTy::Int(_)) => {
                let raw = call.try_as_basic_value().basic().ok_or_else(|| {
                    frontend_error(format!(
                        "plain main `{}` 的普通入口未返回整数值",
                        hir_main.fqn
                    ))
                })?;
                let BasicValueEnum::IntValue(value) = raw else {
                    return Err(frontend_error(format!(
                        "plain main `{}` 的普通入口返回值不是整数",
                        hir_main.fqn
                    )));
                };
                Ok(self.builder.build_int_truncate_or_bit_cast(
                    value,
                    self.context.i32_type(),
                    "plain_main_exit_i32",
                )?)
            }
            _ => Err(frontend_error(format!(
                "plain main wrapper 不支持入口 `{}` 的返回类型",
                hir_main.fqn
            ))),
        }
    }

    pub(super) fn codegen_plain_callable_entry(
        &mut self,
        program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        pass_view: &'a mir::MaterializedMirPassView<'a>,
        abi: &ProgramAbiQuery<'ctx>,
        callable: &'a LateLoweredCallable,
    ) -> Result<(), LlvmEmitError> {
        let plain = callable.plain_abi().ok_or_else(|| {
            frontend_error(format!(
                "plain body lowering callable `{}` 缺少 plain ABI handoff",
                callable.root_fqn()
            ))
        })?;
        let layout = abi.plain_callable_layout_by_version_key(callable.body_version_key())?;
        validate_plain_callable_layout(callable, layout)?;
        let function = self.function(layout.direct_entry().symbol_name())?;
        if function.count_basic_blocks() > 0 {
            return Ok(());
        }

        let hir_fun = self.hir_fun_for_callable_fqn(callable.root_fqn());
        let mir_fun = mir_callable(pass_view, callable.root_fqn())?;
        let is_materialized_closure = hir_fun.is_none() && mir_fun.name.starts_with("$lambda");
        if hir_fun.is_none() && !is_materialized_closure {
            return Err(frontend_error(format!(
                "plain body lowering callable `{}` 缺少 HIR signature",
                callable.root_fqn()
            )));
        }
        let body = mir_fun.body.as_ref().ok_or_else(|| {
            frontend_error(format!(
                "plain body lowering callable `{}` 缺少 canonical MIR body",
                callable.root_fqn()
            ))
        })?;
        let mir_types = &pass_view.materialized().types;
        body.validate_cfg()
            .map_err(|_| LlvmEmitError::UnsupportedMainBody {
                kind: "plain callable cfg",
                at: mir_fun.span.into(),
            })?;
        self.verify_mir_body_composite_transport_contract(
            callable.root_fqn(),
            mir_fun.span,
            body,
            mir_types,
        )?;
        let body_slices = validate_plain_body_slices(callable.root_fqn(), plain, body)?;

        self.current_source_id = if let Some(hir_fun) = hir_fun {
            self.source_id_for_path(hir_fun.source_path.as_path(), hir_fun.span)?
        } else {
            self.materialized_mir_callable_source_id(callable.root_fqn(), mir_fun.span)?
        };
        self.function_cx.current_callable_fqn = Some(callable.root_fqn().to_string());
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(function)?;

        let declared_return_cg = self.cg_ty_of_mir_type(mir_types, mir_fun.return_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "plain callable return type",
                at: mir_fun.span.into(),
            },
        )?;
        self.function_cx.current_fun_return_ty = Some(declared_return_cg);
        let uses_hidden_sret = self
            .hidden_sret_result_ty(mir_fun.span, declared_return_cg)?
            .is_some();
        self.function_cx.current_sret_return_ptr = if uses_hidden_sret {
            Some(
                function
                    .get_nth_param(0)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "missing plain llvm function sret param",
                        at: mir_fun.span.into(),
                    })?
                    .into_pointer_value(),
            )
        } else {
            None
        };

        let (return_bb, return_alloca) =
            self.setup_function_return_context(mir_fun.span, function, declared_return_cg)?;
        if plain.local_effect_control().is_some() {
            let emitter = CallableEmitter::new(
                self,
                program,
                source_types,
                pass_view,
                abi,
                callable,
                mir_fun,
                body,
                function,
                None,
                None,
                None,
                HandleCompletionMode::ContinueToExit,
            )?;
            if let Some(hir_fun) = hir_fun {
                emitter.emit_plain_direct(
                    hir_fun,
                    mir_types,
                    u32::from(uses_hidden_sret),
                    declared_return_cg,
                )?;
            } else if is_materialized_closure {
                emitter.emit_plain_direct_mir_params(
                    u32::from(uses_hidden_sret),
                    declared_return_cg,
                )?;
            } else {
                return Err(frontend_error(format!(
                    "plain callable `{}` 的 local effect/control path 缺少 HIR function shell",
                    callable.root_fqn(),
                )));
            }
            self.emit_function_return_block(
                mir_fun.span,
                declared_return_cg,
                return_bb,
                return_alloca,
            )?;
            self.finish_function_explicit_frame_layout(mir_fun.span)?;
            self.function_cx.current_sret_return_ptr = None;
            return Ok(());
        }
        let mut slots = self.create_mir_local_slots(body, mir_types)?;
        if let Ok(source_id) =
            self.materialized_mir_callable_source_id(callable.root_fqn(), mir_fun.span)
        {
            self.current_source_id = source_id;
        }
        if let Some(hir_fun) = hir_fun {
            self.bind_mir_params(
                hir_fun,
                mir_fun,
                mir_types,
                function,
                u32::from(uses_hidden_sret),
                &mut slots,
            )?;
        } else {
            self.bind_mir_closure_params(
                mir_fun,
                mir_types,
                function,
                u32::from(uses_hidden_sret),
                &mut slots,
            )?;
        }
        let used_locals = collect_mir_local_uses(body);
        let llvm_blocks = body
            .blocks
            .iter()
            .enumerate()
            .map(|(idx, _)| {
                self.context
                    .append_basic_block(function, &format!("plain.bb{idx}"))
            })
            .collect::<Vec<_>>();
        let start_bb = llvm_blocks
            .get(body.start.as_u32() as usize)
            .copied()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "plain callable start block",
                at: mir_fun.span.into(),
            })?;
        self.builder.build_unconditional_branch(start_bb)?;

        for (idx, block) in body.blocks.iter().enumerate() {
            self.builder.position_at_end(llvm_blocks[idx]);
            let block_id = mir::BasicBlockId::from_raw(idx as u32);
            let slice = body_slices.get(&block_id).ok_or_else(|| {
                frontend_error(format!(
                    "plain body lowering callable `{}` 缺少 bb{} 的 published source slice",
                    callable.root_fqn(),
                    block_id.as_u32(),
                ))
            })?;
            {
                let mut values = ValuePrimitives::new(self, mir_types, body, &slots, abi);
                for stmt in &block.stmts
                    [slice.start_statement_index() as usize..slice.end_statement_index() as usize]
                {
                    values.lower_effect_neutral_statement(stmt, &used_locals)?;
                }
            }
            self.codegen_plain_terminator(
                &block.terminator,
                &slots,
                &llvm_blocks,
                declared_return_cg,
            )?;
        }

        self.emit_function_return_block(
            mir_fun.span,
            declared_return_cg,
            return_bb,
            return_alloca,
        )?;
        self.finish_function_explicit_frame_layout(mir_fun.span)?;
        self.function_cx.current_sret_return_ptr = None;
        Ok(())
    }

    pub(super) fn codegen_plain_terminator(
        &mut self,
        terminator: &mir::Terminator,
        slots: &[MirLocalSlot<'ctx>],
        llvm_blocks: &[BasicBlock<'ctx>],
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
            mir::TerminatorKind::Return { value } => {
                let value = match value {
                    Some(operand) => self.codegen_mir_operand_expected(
                        terminator.span,
                        operand,
                        slots,
                        Some(declared_return_cg),
                    )?,
                    None => self.default_value(terminator.span, declared_return_cg)?,
                };
                let value = self
                    .coerce_value(terminator.span, value, declared_return_cg)
                    .map_err(|err| match err {
                        LlvmEmitError::Frontend { message } => frontend_error(format!(
                            "plain return coercion failed at {:?}: {message}",
                            terminator.span
                        )),
                        other => other,
                    })?;
                self.finish_function_return_path(terminator.span, declared_return_cg, value)
            }
            mir::TerminatorKind::Goto { target } => {
                let target_bb = llvm_blocks.get(target.as_u32() as usize).copied().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "plain goto target",
                        at: terminator.span.into(),
                    },
                )?;
                self.builder.build_unconditional_branch(target_bb)?;
                Ok(())
            }
            mir::TerminatorKind::CondBr {
                cond,
                then_target,
                else_target,
            } => {
                let cond = self
                    .codegen_mir_operand(terminator.span, cond, slots)?
                    .as_bool()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "plain branch condition",
                        at: terminator.span.into(),
                    })?;
                let then_bb = llvm_blocks
                    .get(then_target.as_u32() as usize)
                    .copied()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "plain then target",
                        at: terminator.span.into(),
                    })?;
                let else_bb = llvm_blocks
                    .get(else_target.as_u32() as usize)
                    .copied()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "plain else target",
                        at: terminator.span.into(),
                    })?;
                self.builder
                    .build_conditional_branch(cond, then_bb, else_bb)?;
                Ok(())
            }
            mir::TerminatorKind::Unreachable => {
                self.builder.build_unreachable()?;
                Ok(())
            }
            mir::TerminatorKind::Perform { .. }
            | mir::TerminatorKind::ResumeUnwind
            | mir::TerminatorKind::Handle { .. }
            | mir::TerminatorKind::Todo(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "plain callable effect/control terminator",
                at: terminator.span.into(),
            }),
        }
    }
}
