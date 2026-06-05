//! Top-level body codegen entry: orchestrates per-callable lowering and emits the published main exit-code wrapper plus the plain (effect-neutral) callable shells.

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn codegen_program_bodies(
        &mut self,
        program: &'a LateLoweredProgram,
        abi_program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        abi_source_types: &'a TypeStore,
        abi: &ProgramAbiQuery<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        for callable in program.callables() {
            if callable.effect_step_abi().is_some()
                && !abi.callable_version_is_primary(callable.body_version_key())
            {
                continue;
            }
            if callable.effect_step_abi().is_some()
                && callable
                    .body_step_schema()
                    .is_some_and(|step| abi.frame_layout(step).is_none())
            {
                continue;
            }
            let mut child = self.fresh_child_codegen();
            if callable.plain_abi().is_some() {
                child.codegen_plain_callable_entry(program, source_types, abi, callable)?;
            } else {
                child.codegen_callable_entries(program, source_types, abi, callable)?;
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
                child.codegen_plain_callable_entry(abi_program, abi_source_types, abi, callable)?;
            } else {
                child.codegen_callable_entries(abi_program, abi_source_types, abi, callable)?;
            }
        }
        for (kind, carrier_fqn, target) in abi.callable_carrier_target_layouts() {
            // Cached dep carrier shells are defined by the dep artifact object;
            // the consumer module only needs their declarations through ABI materialization.
            if !abi.callable_version_is_primary(target.body_version_key()) {
                continue;
            }
            let mut child = self.fresh_child_codegen();
            child.codegen_callable_carrier_entry_shell(
                kind,
                carrier_fqn,
                target,
                abi_program,
                abi_source_types,
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
                .ok_or_else(|| {
                    frontend_error(format!(
                        "body lowering 缺少 continuation schema k{} 的 surface-resume layout",
                        entry.continuation_schema().as_u32()
                    ))
                })?;
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
                    abi,
                    surface,
                    target,
                )?;
                let mut child = self.fresh_child_codegen();
                child.codegen_continuation_drive_owner_outcome(
                    abi_program,
                    abi_source_types,
                    abi,
                    surface,
                    target,
                )?;
                let mut child = self.fresh_child_codegen();
                child.codegen_surface_resume_owner_trampoline(
                    abi_program,
                    abi_source_types,
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
        self.codegen_release_trampolines()?;
        Ok(())
    }

    /// Emits the C `main` exit path through the stage-owned direct-entry ABI.
    pub(crate) fn codegen_stage_main_exit_code(
        &mut self,
        entry: &EntryRef,
        entry_argv_array: Option<PointerValue<'ctx>>,
        source_types: &TypeStore,
        program: &LateLoweredProgram,
        abi: &ProgramAbiQuery<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let entry_root_fqn = entry.root_fqn();
        let callable = program.callable_by_id(entry.callable_id()).ok_or_else(|| {
            frontend_error(format!(
                "LLVM main wrapper 缺少入口 `{}` 的 callable body（id={:?})",
                entry_root_fqn,
                entry.callable_id()
            ))
        })?;
        if callable.root_fqn() != entry_root_fqn {
            return Err(frontend_error(format!(
                "LLVM main wrapper 入口 `{}` 的 EntryRef 与 LIR body `{}` 不一致",
                entry_root_fqn,
                callable.root_fqn()
            )));
        }
        if callable.plain_abi().is_some() {
            return self.codegen_plain_main_exit_code(
                entry_root_fqn,
                entry_argv_array,
                source_types,
                callable,
                abi,
            );
        }
        if entry_argv_array.is_some() {
            return Err(frontend_error(
                "LLVM effect-step main wrapper 尚未发布 Array<String> argv Step ABI".to_string(),
            ));
        }

        let layout = abi.callable_layout_by_version_key(callable.body_version_key())?;
        let direct = self.function(layout.direct_entry().symbol_name())?;
        let args = Vec::<BasicMetadataValueEnum<'ctx>>::new();
        if !layout.direct_entry().args_abi().is_elided() {
            return Err(frontend_error(format!(
                "LLVM main wrapper 入口 `{}` 的 direct entry args ABI 非 elided；Array<String> argv tuple ABI 尚未发布或 entry contract 漂移",
                entry_root_fqn
            )));
        }
        let call = self.builder.build_call(direct, &args, "main_step")?;
        let step = call
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| frontend_error("main direct entry 未返回 Step_F".to_string()))?;
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
        let source = callable_source(callable, "LLVM main wrapper")?;
        let exit_value = match self.cg_ty_of_mir_type(source_types, source.return_ty) {
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
                    entry_root_fqn
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
        entry_root_fqn: &str,
        entry_argv_array: Option<PointerValue<'ctx>>,
        source_types: &TypeStore,
        callable: &LateLoweredCallable,
        abi: &ProgramAbiQuery<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let plain = callable.plain_abi().ok_or_else(|| {
            frontend_error(format!(
                "plain main `{entry_root_fqn}` 缺少 plain callable contract"
            ))
        })?;
        let layout = abi.plain_callable_layout_by_version_key(callable.body_version_key())?;
        if layout.root_fqn() != callable.root_fqn() {
            return Err(frontend_error(format!(
                "plain main `{}` 的 ABI layout root 漂移：layout=`{}`",
                callable.root_fqn(),
                layout.root_fqn(),
            )));
        }
        let entry = layout.direct_entry();
        let args = match (plain.param_tys(), entry_argv_array) {
            ([], None) if entry.param_count() == 0 => Vec::new(),
            ([_param], Some(argv_array)) if entry.param_count() == 1 => vec![argv_array.into()],
            ([], Some(_)) => {
                return Err(frontend_error(format!(
                    "plain main `{}` 没有 source argv 参数，但 wrapper 收到了 argv array",
                    entry_root_fqn,
                )));
            }
            ([_], None) => {
                return Err(frontend_error(format!(
                    "plain main `{}` 需要 argv array，但 wrapper 未收到入口 argv",
                    entry_root_fqn,
                )));
            }
            _ => {
                return Err(frontend_error(format!(
                    "plain main `{}` argv ABI 漂移：source_params={} direct_params={}",
                    entry_root_fqn,
                    plain.param_tys().len(),
                    entry.param_count(),
                )));
            }
        };
        let direct = self.function(entry.symbol_name())?;
        let call = self.builder.build_call(direct, &args, "plain_main")?;
        match self.cg_ty_of_mir_type(source_types, plain.return_ty()) {
            Some(CgTy::Unit) => Ok(self.context.i32_type().const_zero()),
            Some(CgTy::Int(_)) => {
                let raw = call.try_as_basic_value().basic().ok_or_else(|| {
                    frontend_error(format!(
                        "plain main `{}` 的普通入口未返回整数值",
                        entry_root_fqn
                    ))
                })?;
                let BasicValueEnum::IntValue(value) = raw else {
                    return Err(frontend_error(format!(
                        "plain main `{}` 的普通入口返回值不是整数",
                        entry_root_fqn
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
                entry_root_fqn
            ))),
        }
    }

    pub(super) fn codegen_plain_callable_entry(
        &mut self,
        program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
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

        let executable_body = callable.executable_body().ok_or_else(|| {
            frontend_error(format!(
                "plain body lowering callable `{}` 缺少 LIR executable body",
                callable.root_fqn()
            ))
        })?;
        let header = executable_body.header();
        let source_span = header.span();
        let is_materialized_closure = header.name().starts_with("$lambda");
        let mir_types = source_types;
        self.verify_lir_body_composite_transport_contract(
            callable.root_fqn(),
            executable_body,
            mir_types,
        )?;

        self.current_source_id = self.source_id_for_path(
            callable
                .body_version_key()
                .surface_instance()
                .template
                .source_path
                .as_path(),
            source_span,
        )?;
        self.function_cx.current_lir_callable_id = self
            .active_lir_program()
            .and_then(|active| active.callable_id_by_root(callable.root_fqn()))
            .or_else(|| program.callable_id_for(callable));
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(function)?;

        let declared_return_cg = self.cg_ty_of_mir_type(mir_types, header.return_ty()).unwrap_or_else(|| {
            panic!(
                "codegen_plain_callable_entry: plain callable verifier accepted non-codegen return type for `{}` at {:?}",
                callable.root_fqn(),
                source_span
            )
        });
        self.function_cx.current_fun_return_ty = Some(declared_return_cg);
        let uses_hidden_sret = self
            .hidden_sret_result_ty(source_span, declared_return_cg)?
            .is_some();
        self.function_cx.current_sret_return_ptr = if uses_hidden_sret {
            Some(
                function
                    .get_nth_param(0)
                    .unwrap_or_else(|| {
                        panic!(
                            "codegen_plain_callable_entry: plain callable ABI accepted missing sret param for `{}` at {:?}",
                            callable.root_fqn(),
                            source_span
                        )
                    })
                    .into_pointer_value(),
            )
        } else {
            None
        };

        let (return_bb, return_alloca) =
            self.setup_function_return_context(source_span, function, declared_return_cg)?;
        if plain.local_effect_control().is_some() {
            let (mir_fun, body) =
                callable_source_body(callable, "plain local effect-control lowering")?;
            body.validate_cfg().unwrap_or_else(|err| {
                panic!(
                    "codegen_plain_callable_entry: plain local effect-control verifier accepted invalid CFG for `{}` at {:?}: {err}",
                    callable.root_fqn(),
                    mir_fun.span
                )
            });
            let emitter = CallableEmitter::new(
                self,
                program,
                source_types,
                abi,
                callable,
                mir_fun,
                function,
                None,
                None,
                None,
                HandleCompletionMode::ContinueToExit,
            )?;
            if is_materialized_closure {
                emitter.emit_plain_direct_mir_params(
                    u32::from(uses_hidden_sret),
                    declared_return_cg,
                )?;
            } else {
                emitter.emit_plain_direct(
                    mir_types,
                    u32::from(uses_hidden_sret),
                    declared_return_cg,
                )?;
            }
            self.emit_function_return_block(
                source_span,
                declared_return_cg,
                return_bb,
                return_alloca,
            )?;
            self.finish_function_explicit_frame_layout(source_span)?;
            self.function_cx.current_sret_return_ptr = None;
            return Ok(());
        }
        let mut slots = self.create_lir_local_slots(executable_body, mir_types)?;
        if is_materialized_closure {
            self.bind_lir_closure_params(
                executable_body.header(),
                mir_types,
                function,
                u32::from(uses_hidden_sret),
                &mut slots,
            )?;
        } else {
            self.bind_lir_header_params(
                executable_body.header(),
                mir_types,
                function,
                u32::from(uses_hidden_sret),
                &mut slots,
            )?;
        }
        let used_locals = collect_lir_local_uses(executable_body, mir_types);
        let llvm_blocks = executable_body
            .states()
            .states()
            .iter()
            .map(|state| {
                (
                    state.state_id(),
                    self.context.append_basic_block(
                        function,
                        &format!("plain.s{}", state.state_id().as_u32()),
                    ),
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        let start_bb = llvm_blocks
            .get(&executable_body.states().entry_state())
            .copied()
            .unwrap_or_else(|| {
                panic!(
                    "codegen_plain_callable_entry: LIR executable body accepted missing entry state s{} for `{}` at {:?}",
                    executable_body.states().entry_state().as_u32(),
                    callable.root_fqn(),
                    source_span
                )
            });
        self.builder.build_unconditional_branch(start_bb)?;

        for state in executable_body.states().states() {
            let llvm_block = llvm_blocks.get(&state.state_id()).copied().unwrap_or_else(|| {
                panic!(
                    "codegen_plain_callable_entry: LIR executable body accepted missing block for state s{}",
                    state.state_id().as_u32()
                )
            });
            self.builder.position_at_end(llvm_block);
            for stmt in state.body().statements() {
                self.codegen_lir_statement(
                    stmt,
                    executable_body,
                    mir_types,
                    &slots,
                    &used_locals,
                    Some(abi),
                )?;
            }
            self.codegen_lir_plain_terminator(
                state.body().terminator(),
                &slots,
                &llvm_blocks,
                declared_return_cg,
            )?;
        }

        self.emit_function_return_block(source_span, declared_return_cg, return_bb, return_alloca)?;
        self.finish_function_explicit_frame_layout(source_span)?;
        self.function_cx.current_sret_return_ptr = None;
        Ok(())
    }
}
