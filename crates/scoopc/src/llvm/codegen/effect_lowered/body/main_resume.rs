//! Resume / surface-resume / continuation-driver codegen entries plus the small helper methods (current_function, refactor_function, LLVM type accessors) that the resume layer relies on.

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn codegen_refactor_resume_method(
        &mut self,
        program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        pass_view: &'a mir::MaterializedMirPassView<'a>,
        abi: &ProgramAbiQuery<'ctx>,
        callable: &'a LateLoweredCallable,
        symbol_name: &str,
        fn_ty: inkwell::types::FunctionType<'ctx>,
        case_tag: CaseTag,
        resume_tuple_ty: TypeId,
    ) -> Result<(), LlvmEmitError> {
        let function =
            self.declare_compiler_private_helper_function(symbol_name, fn_ty, Linkage::Internal);
        if function.count_basic_blocks() > 0 {
            return Ok(());
        }
        let mir_fun = refactor_mir_callable(pass_view, callable.root_fqn())?;
        let body = mir_fun.body.as_ref().ok_or_else(|| {
            frontend_error(format!(
                "refactor resume method `{symbol_name}` owner `{}` 缺少 canonical MIR body",
                callable.root_fqn()
            ))
        })?;
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(function)?;
        RefactorCallableEmitter::new(
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
            RefactorHandleCompletionMode::ReturnFromFunction,
        )?
        .emit_resume_method(case_tag, resume_tuple_ty)?;
        self.finish_function_explicit_frame_layout(mir_fun.span)?;
        Ok(())
    }

    pub(super) fn codegen_refactor_unreachable_resume_method(
        &mut self,
        symbol_name: &str,
        fn_ty: inkwell::types::FunctionType<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let function =
            self.declare_compiler_private_helper_function(symbol_name, fn_ty, Linkage::Internal);
        if function.count_basic_blocks() == 0 {
            let entry = self.context.append_basic_block(function, "entry");
            self.builder.position_at_end(entry);
            self.builder.build_unreachable()?;
        }
        Ok(())
    }

    pub(super) fn codegen_refactor_surface_resume(
        &mut self,
        _program: &LateLoweredProgram,
        abi: &ProgramAbiQuery<'ctx>,
        surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let function = self.declare_compiler_private_helper_function(
            surface.symbol_name(),
            surface.llvm_ty(),
            Linkage::Internal,
        );
        if function.count_basic_blocks() > 0 {
            return Ok(());
        }
        let dispatch = abi.surface_resume_dispatch_layout(surface.continuation_schema())?;
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        let targets = dispatch.target().owner_trampolines();
        if targets.is_empty() {
            self.builder.build_unreachable()?;
            return Ok(());
        }
        let cont = function.get_nth_param(0).ok_or_else(|| {
            frontend_error(format!(
                "refactor surface resume `{}` 缺少 continuation 参数",
                surface.symbol_name()
            ))
        })?;
        let cont_ptr = cont.into_pointer_value();
        let mut args = vec![cont_ptr.into()];
        if surface.param_count() > 1 {
            let payload = function.get_nth_param(1).ok_or_else(|| {
                frontend_error(format!(
                    "refactor surface resume `{}` 缺少 resume payload 参数",
                    surface.symbol_name()
                ))
            })?;
            args.push(payload.into());
        }
        if targets.len() == 1 {
            let trampoline_fun = self.refactor_function(targets[0].symbol_name())?;
            let call =
                self.builder
                    .build_call(trampoline_fun, &args, "refactor_surface_resume_call")?;
            let owner_step = call.try_as_basic_value().basic().ok_or_else(|| {
                frontend_error(format!(
                    "refactor surface resume `{}` 调用 owner dispatch 未返回 Step_F",
                    surface.symbol_name()
                ))
            })?;
            self.builder.build_return(Some(&owner_step))?;
            return Ok(());
        }

        let current_desc = self.load_gc_object_type_desc(cont_ptr, "surface_resume_cont_desc")?;
        let word_ty = self.context.i64_type();
        let current_desc_int =
            self.builder
                .build_ptr_to_int(current_desc, word_ty, "surface_resume_cont_desc_int")?;
        let first_check = self
            .context
            .append_basic_block(function, "surface_resume_check0");
        self.builder.build_unconditional_branch(first_check)?;
        let mut check_bb = first_check;
        for (index, target) in targets.iter().enumerate() {
            let next_bb = self
                .context
                .append_basic_block(function, &format!("surface_resume_check{}", index + 1));
            let hit_bb = self.context.append_basic_block(
                function,
                &format!(
                    "surface_resume_hit_ko{}",
                    target.owner_continuation_object().as_u32()
                ),
            );
            self.builder.position_at_end(check_bb);
            let continuation_layout = abi
                .continuation_layout(target.owner_continuation_object())
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor surface resume `{}` 缺少 owner continuation object ko{} layout",
                        surface.symbol_name(),
                        target.owner_continuation_object().as_u32(),
                    ))
                })?;
            let type_desc = self.get_or_create_refactor_gc_type_descriptor(
                crate::span::Span::new(0, 0),
                continuation_layout.llvm_ty(),
                continuation_layout.layout_anchor_name(),
            )?;
            let type_desc_i8 = self.builder.build_pointer_cast(
                type_desc.as_pointer_value(),
                self.llvm_i8_ptr_type(),
                "surface_resume_target_desc",
            )?;
            let target_desc_int = self.builder.build_ptr_to_int(
                type_desc_i8,
                word_ty,
                "surface_resume_target_desc_int",
            )?;
            let is_match = self.builder.build_int_compare(
                inkwell::IntPredicate::EQ,
                current_desc_int,
                target_desc_int,
                "surface_resume_desc_match",
            )?;
            self.builder
                .build_conditional_branch(is_match, hit_bb, next_bb)?;

            self.builder.position_at_end(hit_bb);
            let trampoline_fun = self.refactor_function(target.symbol_name())?;
            let call = self.builder.build_call(
                trampoline_fun,
                &args,
                "refactor_surface_resume_owner_call",
            )?;
            let owner_step = call.try_as_basic_value().basic().ok_or_else(|| {
                frontend_error(format!(
                    "refactor surface resume `{}` 调用 owner dispatch `{}` 未返回 Step_F",
                    surface.symbol_name(),
                    target.symbol_name(),
                ))
            })?;
            self.builder.build_return(Some(&owner_step))?;
            check_bb = next_bb;
        }

        self.builder.position_at_end(check_bb);
        self.builder.build_unreachable()?;
        Ok(())
    }

    pub(super) fn codegen_refactor_surface_resume_outcome(
        &mut self,
        abi: &ProgramAbiQuery<'ctx>,
        surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let function = self.refactor_surface_resume_outcome_function(surface);
        if function.count_basic_blocks() > 0 {
            return Ok(());
        }
        let dispatch = abi.surface_resume_dispatch_layout(surface.continuation_schema())?;
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        let targets = dispatch.target().owner_trampolines();
        if targets.is_empty() {
            self.builder.build_unreachable()?;
            return Ok(());
        }
        let cont = function.get_nth_param(0).ok_or_else(|| {
            frontend_error(format!(
                "refactor outcome surface resume `{}` 缺少 continuation 参数",
                function.get_name().to_str().unwrap_or("<invalid>")
            ))
        })?;
        let mut args = vec![cont.into_pointer_value().into()];
        if !surface.resume_payload_abi().is_elided() {
            let payload = function.get_nth_param(1).ok_or_else(|| {
                frontend_error(format!(
                    "refactor outcome surface resume `{}` 缺少 resume payload 参数",
                    function.get_name().to_str().unwrap_or("<invalid>")
                ))
            })?;
            args.push(payload.into());
        }
        let outcome_index = if surface.resume_payload_abi().is_elided() {
            1
        } else {
            2
        };
        let outcome_ptr = function.get_nth_param(outcome_index).ok_or_else(|| {
            frontend_error(format!(
                "refactor outcome surface resume `{}` 缺少 explicit outcome 参数",
                function.get_name().to_str().unwrap_or("<invalid>")
            ))
        })?;
        args.push(outcome_ptr.into());
        if targets.len() == 1 {
            let callee = self.refactor_surface_resume_owner_outcome_function(surface, &targets[0]);
            self.build_call_preserving_gc_local_roots(
                crate::span::Span::new(0, 0),
                callee,
                &args,
                "refactor_surface_resume_outcome_call",
            )?;
            self.builder.build_return(None)?;
            return Ok(());
        }

        let cont_ptr = cont.into_pointer_value();
        let current_desc =
            self.load_gc_object_type_desc(cont_ptr, "surface_resume_outcome_desc")?;
        let word_ty = self.context.i64_type();
        let current_desc_int = self.builder.build_ptr_to_int(
            current_desc,
            word_ty,
            "surface_resume_outcome_desc_int",
        )?;
        let first_check = self
            .context
            .append_basic_block(function, "surface_resume_outcome_check0");
        self.builder.build_unconditional_branch(first_check)?;
        let mut check_bb = first_check;
        for (index, target) in targets.iter().enumerate() {
            let next_bb = self.context.append_basic_block(
                function,
                &format!("surface_resume_outcome_check{}", index + 1),
            );
            let hit_bb = self.context.append_basic_block(
                function,
                &format!(
                    "surface_resume_outcome_hit_ko{}",
                    target.owner_continuation_object().as_u32()
                ),
            );
            self.builder.position_at_end(check_bb);
            let continuation_layout = abi
                .continuation_layout(target.owner_continuation_object())
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor outcome surface resume `{}` 缺少 owner continuation object ko{} layout",
                        function.get_name().to_str().unwrap_or("<invalid>"),
                        target.owner_continuation_object().as_u32(),
                    ))
                })?;
            let type_desc = self.get_or_create_refactor_gc_type_descriptor(
                crate::span::Span::new(0, 0),
                continuation_layout.llvm_ty(),
                continuation_layout.layout_anchor_name(),
            )?;
            let type_desc_i8 = self.builder.build_pointer_cast(
                type_desc.as_pointer_value(),
                self.llvm_i8_ptr_type(),
                "surface_resume_outcome_target_desc",
            )?;
            let target_desc_int = self.builder.build_ptr_to_int(
                type_desc_i8,
                word_ty,
                "surface_resume_outcome_target_desc_int",
            )?;
            let is_match = self.builder.build_int_compare(
                inkwell::IntPredicate::EQ,
                current_desc_int,
                target_desc_int,
                "surface_resume_outcome_desc_match",
            )?;
            self.builder
                .build_conditional_branch(is_match, hit_bb, next_bb)?;

            self.builder.position_at_end(hit_bb);
            let callee = self.refactor_surface_resume_owner_outcome_function(surface, target);
            self.build_call_preserving_gc_local_roots(
                crate::span::Span::new(0, 0),
                callee,
                &args,
                "refactor_surface_resume_owner_outcome_call",
            )?;
            self.builder.build_return(None)?;
            check_bb = next_bb;
        }

        self.builder.position_at_end(check_bb);
        self.builder.build_unreachable()?;
        Ok(())
    }

    pub(super) fn codegen_refactor_continuation_drive_outcome(
        &mut self,
        abi: &ProgramAbiQuery<'ctx>,
        surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let function = self.refactor_continuation_drive_outcome_function(surface);
        if function.count_basic_blocks() > 0 {
            return Ok(());
        }
        let dispatch = abi.surface_resume_dispatch_layout(surface.continuation_schema())?;
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        let targets = dispatch.target().owner_trampolines();
        if targets.is_empty() {
            self.builder.build_unreachable()?;
            return Ok(());
        }
        let cont = function.get_nth_param(0).ok_or_else(|| {
            frontend_error(format!(
                "refactor continuation drive outcome `{}` 缺少 continuation 参数",
                function.get_name().to_str().unwrap_or("<invalid>")
            ))
        })?;
        let resume_word = function.get_nth_param(1).ok_or_else(|| {
            frontend_error(format!(
                "refactor continuation drive outcome `{}` 缺少 resume_word 参数",
                function.get_name().to_str().unwrap_or("<invalid>")
            ))
        })?;
        let resume_gc_ref = function.get_nth_param(2).ok_or_else(|| {
            frontend_error(format!(
                "refactor continuation drive outcome `{}` 缺少 resume_gc_ref 参数",
                function.get_name().to_str().unwrap_or("<invalid>")
            ))
        })?;
        let answer_slot = function.get_nth_param(3).ok_or_else(|| {
            frontend_error(format!(
                "refactor continuation drive outcome `{}` 缺少 answer slot 参数",
                function.get_name().to_str().unwrap_or("<invalid>")
            ))
        })?;
        let outcome_ptr = function.get_nth_param(4).ok_or_else(|| {
            frontend_error(format!(
                "refactor continuation drive outcome `{}` 缺少 outcome 参数",
                function.get_name().to_str().unwrap_or("<invalid>")
            ))
        })?;
        let args = vec![
            cont.into_pointer_value().into(),
            resume_word.into_int_value().into(),
            resume_gc_ref.into_pointer_value().into(),
            answer_slot.into_pointer_value().into(),
            outcome_ptr.into_pointer_value().into(),
        ];
        if targets.len() == 1 {
            let callee =
                self.refactor_continuation_drive_owner_outcome_function(surface, &targets[0]);
            self.build_call_preserving_gc_local_roots(
                crate::span::Span::new(0, 0),
                callee,
                &args,
                "refactor_continuation_drive_outcome_call",
            )?;
            self.builder.build_return(None)?;
            return Ok(());
        }

        let cont_ptr = cont.into_pointer_value();
        let current_desc =
            self.load_gc_object_type_desc(cont_ptr, "continuation_drive_outcome_desc")?;
        let word_ty = self.context.i64_type();
        let current_desc_int = self.builder.build_ptr_to_int(
            current_desc,
            word_ty,
            "continuation_drive_outcome_desc_int",
        )?;
        let first_check = self
            .context
            .append_basic_block(function, "continuation_drive_outcome_check0");
        self.builder.build_unconditional_branch(first_check)?;
        let mut check_bb = first_check;
        for (index, target) in targets.iter().enumerate() {
            let next_bb = self.context.append_basic_block(
                function,
                &format!("continuation_drive_outcome_check{}", index + 1),
            );
            let hit_bb = self.context.append_basic_block(
                function,
                &format!(
                    "continuation_drive_outcome_hit_ko{}",
                    target.owner_continuation_object().as_u32()
                ),
            );
            self.builder.position_at_end(check_bb);
            let continuation_layout = abi
                .continuation_layout(target.owner_continuation_object())
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor continuation drive outcome `{}` 缺少 owner continuation object ko{} layout",
                        function.get_name().to_str().unwrap_or("<invalid>"),
                        target.owner_continuation_object().as_u32(),
                    ))
                })?;
            let type_desc = self.get_or_create_refactor_gc_type_descriptor(
                crate::span::Span::new(0, 0),
                continuation_layout.llvm_ty(),
                continuation_layout.layout_anchor_name(),
            )?;
            let type_desc_i8 = self.builder.build_pointer_cast(
                type_desc.as_pointer_value(),
                self.llvm_i8_ptr_type(),
                "continuation_drive_outcome_target_desc",
            )?;
            let target_desc_int = self.builder.build_ptr_to_int(
                type_desc_i8,
                word_ty,
                "continuation_drive_outcome_target_desc_int",
            )?;
            let is_match = self.builder.build_int_compare(
                inkwell::IntPredicate::EQ,
                current_desc_int,
                target_desc_int,
                "continuation_drive_outcome_desc_match",
            )?;
            self.builder
                .build_conditional_branch(is_match, hit_bb, next_bb)?;

            self.builder.position_at_end(hit_bb);
            let callee = self.refactor_continuation_drive_owner_outcome_function(surface, target);
            self.build_call_preserving_gc_local_roots(
                crate::span::Span::new(0, 0),
                callee,
                &args,
                "refactor_continuation_drive_owner_outcome_call",
            )?;
            self.builder.build_return(None)?;
            check_bb = next_bb;
        }

        self.builder.position_at_end(check_bb);
        self.builder.build_unreachable()?;
        Ok(())
    }

    pub(super) fn codegen_refactor_dynamic_surface_resume_adapter(
        &mut self,
        program: &'a LateLoweredProgram,
        abi: &ProgramAbiQuery<'ctx>,
        surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let function = self.refactor_function(surface.symbol_name())?;
        if function.count_basic_blocks() > 0 {
            return Ok(());
        }
        let wrapper_step = program
            .step_type(surface.return_step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor dynamic surface resume k{} 缺少 wrapper step schema s{}",
                    surface.continuation_schema().as_u32(),
                    surface.return_step_schema().as_u32()
                ))
            })?;
        let wrapper_case = wrapper_step
            .cases()
            .iter()
            .find(|case| {
                case.continuation_contract().continuation_schema() == surface.continuation_schema()
            })
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor dynamic surface resume k{} 无法在 wrapper step s{} 中找到对应 case",
                    surface.continuation_schema().as_u32(),
                    wrapper_step.step_schema().as_u32()
                ))
            })?;

        let mut candidates = Vec::new();
        for callable in program.callables() {
            if !callable.has_control_body() || callable.step_schema() == wrapper_step.step_schema()
            {
                continue;
            }
            let Some(owner_step) = program.step_type(callable.step_schema()) else {
                continue;
            };
            let Some(owner_case) = owner_step.cases().iter().find(|case| {
                case.concrete_op_key() == wrapper_case.concrete_op_key()
                    && case.payload_tuple_ty() == wrapper_case.payload_tuple_ty()
                    && case.answer_ty() == wrapper_case.answer_ty()
            }) else {
                continue;
            };
            let Some(continuation_layout) = abi.continuation_layout(callable.continuation_object())
            else {
                continue;
            };
            let Some(owner_surface) =
                abi.surface_resume_layout(owner_case.continuation_contract().continuation_schema())
            else {
                continue;
            };
            candidates.push((callable, continuation_layout, owner_surface));
        }
        if candidates.is_empty() {
            let entry = self.context.append_basic_block(function, "entry");
            self.builder.position_at_end(entry);
            self.builder.build_unreachable()?;
            return Ok(());
        }

        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        let cont = function.get_nth_param(0).ok_or_else(|| {
            frontend_error(format!(
                "refactor dynamic surface resume `{}` 缺少 continuation 参数",
                surface.symbol_name()
            ))
        })?;
        let cont_ptr = cont.into_pointer_value();
        let payload = if surface.param_count() > 1 {
            Some(function.get_nth_param(1).ok_or_else(|| {
                frontend_error(format!(
                    "refactor dynamic surface resume `{}` 缺少 payload 参数",
                    surface.symbol_name()
                ))
            })?)
        } else {
            None
        };
        let current_desc = self.load_gc_object_type_desc(cont_ptr, "dynamic_surface_cont_desc")?;
        let word_ty = self.context.i64_type();
        let current_desc_int = self.builder.build_ptr_to_int(
            current_desc,
            word_ty,
            "dynamic_surface_cont_desc_int",
        )?;
        let first_check = self
            .context
            .append_basic_block(function, "dynamic_surface_check0");
        self.builder.build_unconditional_branch(first_check)?;

        let mut check_bb = first_check;
        for (index, (callable, continuation_layout, owner_surface)) in
            candidates.into_iter().enumerate()
        {
            let next_bb = self
                .context
                .append_basic_block(function, &format!("dynamic_surface_check{}", index + 1));
            let hit_bb = self.context.append_basic_block(
                function,
                &format!("dynamic_surface_hit_s{}", callable.step_schema().as_u32()),
            );
            self.builder.position_at_end(check_bb);
            let type_desc = self.get_or_create_refactor_gc_type_descriptor(
                crate::span::Span::new(0, 0),
                continuation_layout.llvm_ty(),
                continuation_layout.layout_anchor_name(),
            )?;
            let type_desc_i8 = self.builder.build_pointer_cast(
                type_desc.as_pointer_value(),
                self.llvm_i8_ptr_type(),
                "dynamic_surface_target_desc",
            )?;
            let target_desc_int = self.builder.build_ptr_to_int(
                type_desc_i8,
                word_ty,
                "dynamic_surface_target_desc_int",
            )?;
            let is_match = self.builder.build_int_compare(
                inkwell::IntPredicate::EQ,
                current_desc_int,
                target_desc_int,
                "dynamic_surface_desc_match",
            )?;
            self.builder
                .build_conditional_branch(is_match, hit_bb, next_bb)?;

            self.builder.position_at_end(hit_bb);
            let owner_fun = self.refactor_function(owner_surface.symbol_name())?;
            let mut args = Vec::<BasicMetadataValueEnum<'ctx>>::from([cont_ptr.into()]);
            if owner_surface.param_count() > 1 {
                args.push(
                    payload
                        .ok_or_else(|| {
                            frontend_error(format!(
                                "refactor dynamic surface resume `{}` target `{}` 需要 payload",
                                surface.symbol_name(),
                                owner_surface.symbol_name()
                            ))
                        })?
                        .into(),
                );
            }
            let call = self.build_call_preserving_gc_local_roots(
                crate::span::Span::new(0, 0),
                owner_fun,
                &args,
                "dynamic_surface_owner_resume",
            )?;
            let owner_step = call.try_as_basic_value().basic().ok_or_else(|| {
                frontend_error(format!(
                    "refactor dynamic surface resume `{}` target `{}` 未返回 Step_F",
                    surface.symbol_name(),
                    owner_surface.symbol_name()
                ))
            })?;
            let projected = self.project_refactor_step_to_schema(
                abi,
                owner_step,
                callable.step_schema(),
                surface.return_step_schema(),
            )?;
            self.builder.build_return(Some(&projected))?;
            check_bb = next_bb;
        }

        self.builder.position_at_end(check_bb);
        self.builder.build_unreachable()?;
        Ok(())
    }

    pub(super) fn collect_surface_resume_handle_sites(
        &self,
        target: &super::super::types::RefactorContinuationSurfaceResumeOwnerTrampolineLayout<'ctx>,
    ) -> Option<BTreeSet<SiteId>> {
        let mut surface_handle_sites = target
            .handle_binder_routes()
            .iter()
            .map(|route| route.site_id())
            .collect::<BTreeSet<_>>();
        if let Some(projection) = target.wrapper_projection()
            && let LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                site_id,
                ..
            } = projection.underlying_route().publication()
        {
            surface_handle_sites.insert(*site_id);
        }
        (!surface_handle_sites.is_empty()).then_some(surface_handle_sites)
    }

    pub(super) fn codegen_refactor_surface_resume_owner_core(
        &mut self,
        program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        pass_view: &'a mir::MaterializedMirPassView<'a>,
        abi: &ProgramAbiQuery<'ctx>,
        surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
        target: &super::super::types::RefactorContinuationSurfaceResumeOwnerTrampolineLayout<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let core_fun = self.refactor_surface_resume_owner_core_function(surface, target);
        if core_fun.count_basic_blocks() > 0 {
            return Ok(());
        }
        let callable = program
            .callable_by_version_key(target.owner_version_key())
            .or_else(|| {
                program.callables().iter().find(|candidate| {
                    candidate.body_version_key().surface_instance()
                        == target.owner_version_key().surface_instance()
                })
            })
            .or_else(|| {
                program.callables().iter().find(|candidate| {
                    candidate.root_fqn()
                        == target.owner_version_key().surface_instance().template.fqn
                })
            })
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor outcome owner core k{} 缺少 owner callable {:?}",
                    surface.continuation_schema().as_u32(),
                    target.owner_version_key()
                ))
            })?;
        let mir_fun = refactor_mir_callable(pass_view, callable.root_fqn())?;
        let body = mir_fun.body.as_ref().ok_or_else(|| {
            frontend_error(format!(
                "refactor outcome owner core `{}` owner `{}` 缺少 canonical MIR body",
                core_fun.get_name().to_str().unwrap_or("<invalid>"),
                callable.root_fqn()
            ))
        })?;
        let entry = self.context.append_basic_block(core_fun, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(core_fun)?;
        RefactorCallableEmitter::new(
            self,
            program,
            source_types,
            pass_view,
            abi,
            callable,
            mir_fun,
            body,
            core_fun,
            None,
            None,
            self.collect_surface_resume_handle_sites(target),
            RefactorHandleCompletionMode::ReturnFromFunction,
        )?
        .emit_resume_outcome_core(surface.resume_tuple_ty())?;
        self.finish_function_explicit_frame_layout(mir_fun.span)?;
        Ok(())
    }

    pub(super) fn codegen_refactor_surface_resume_owner_outcome(
        &mut self,
        program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        pass_view: &'a mir::MaterializedMirPassView<'a>,
        abi: &ProgramAbiQuery<'ctx>,
        surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
        target: &super::super::types::RefactorContinuationSurfaceResumeOwnerTrampolineLayout<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let outcome_fun = self.refactor_surface_resume_owner_outcome_function(surface, target);
        if outcome_fun.count_basic_blocks() > 0 {
            return Ok(());
        }
        let core_fun = self.refactor_surface_resume_owner_core_function(surface, target);
        {
            let mut child = self.fresh_child_codegen();
            child.codegen_refactor_surface_resume_owner_core(
                program,
                source_types,
                pass_view,
                abi,
                surface,
                target,
            )?;
        }
        let callable = program
            .callable_by_version_key(target.owner_version_key())
            .or_else(|| {
                program.callables().iter().find(|candidate| {
                    candidate.body_version_key().surface_instance()
                        == target.owner_version_key().surface_instance()
                })
            })
            .or_else(|| {
                program.callables().iter().find(|candidate| {
                    candidate.root_fqn()
                        == target.owner_version_key().surface_instance().template.fqn
                })
            })
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor outcome owner wrapper k{} 缺少 owner callable {:?}",
                    surface.continuation_schema().as_u32(),
                    target.owner_version_key()
                ))
            })?;
        let mir_fun = refactor_mir_callable(pass_view, callable.root_fqn())?;
        let body = mir_fun.body.as_ref().ok_or_else(|| {
            frontend_error(format!(
                "refactor outcome owner wrapper `{}` owner `{}` 缺少 canonical MIR body",
                outcome_fun.get_name().to_str().unwrap_or("<invalid>"),
                callable.root_fqn()
            ))
        })?;
        let entry = self.context.append_basic_block(outcome_fun, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(outcome_fun)?;
        RefactorCallableEmitter::new(
            self,
            program,
            source_types,
            pass_view,
            abi,
            callable,
            mir_fun,
            body,
            outcome_fun,
            None,
            None,
            self.collect_surface_resume_handle_sites(target),
            RefactorHandleCompletionMode::ReturnFromFunction,
        )?
        .emit_resume_outcome_wrapper(core_fun, surface.resume_tuple_ty())?;
        self.finish_function_explicit_frame_layout(mir_fun.span)?;
        Ok(())
    }

    pub(super) fn codegen_refactor_continuation_step(
        &mut self,
        program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        pass_view: &'a mir::MaterializedMirPassView<'a>,
        abi: &ProgramAbiQuery<'ctx>,
        surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
        target: &super::super::types::RefactorContinuationSurfaceResumeOwnerTrampolineLayout<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let function = self.refactor_continuation_step_function(target);
        if function.count_basic_blocks() > 0 {
            return Ok(());
        }
        let callable = program
            .callable_by_version_key(target.owner_version_key())
            .or_else(|| {
                program.callables().iter().find(|candidate| {
                    candidate.body_version_key().surface_instance()
                        == target.owner_version_key().surface_instance()
                })
            })
            .or_else(|| {
                program.callables().iter().find(|candidate| {
                    candidate.root_fqn()
                        == target.owner_version_key().surface_instance().template.fqn
                })
            })
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor continuation step k{} 缺少 owner callable {:?}",
                    surface.continuation_schema().as_u32(),
                    target.owner_version_key()
                ))
            })?;
        let mir_fun = refactor_mir_callable(pass_view, callable.root_fqn())?;
        let body = mir_fun.body.as_ref().ok_or_else(|| {
            frontend_error(format!(
                "refactor continuation step `{}` owner `{}` 缺少 canonical MIR body",
                function.get_name().to_str().unwrap_or("<invalid>"),
                callable.root_fqn()
            ))
        })?;
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(function)?;
        RefactorCallableEmitter::new(
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
            self.collect_surface_resume_handle_sites(target),
            RefactorHandleCompletionMode::ReturnFromFunction,
        )?
        .emit_generated_continuation_step(surface.resume_tuple_ty())?;
        self.finish_function_explicit_frame_layout(mir_fun.span)?;
        Ok(())
    }

    pub(super) fn codegen_refactor_continuation_drive_owner_outcome(
        &mut self,
        program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        pass_view: &'a mir::MaterializedMirPassView<'a>,
        abi: &ProgramAbiQuery<'ctx>,
        surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
        target: &super::super::types::RefactorContinuationSurfaceResumeOwnerTrampolineLayout<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let outcome_fun = self.refactor_continuation_drive_owner_outcome_function(surface, target);
        if outcome_fun.count_basic_blocks() > 0 {
            return Ok(());
        }
        {
            let mut child = self.fresh_child_codegen();
            child.codegen_refactor_continuation_step(
                program,
                source_types,
                pass_view,
                abi,
                surface,
                target,
            )?;
        }
        let callable = program
            .callable_by_version_key(target.owner_version_key())
            .or_else(|| {
                program.callables().iter().find(|candidate| {
                    candidate.body_version_key().surface_instance()
                        == target.owner_version_key().surface_instance()
                })
            })
            .or_else(|| {
                program.callables().iter().find(|candidate| {
                    candidate.root_fqn()
                        == target.owner_version_key().surface_instance().template.fqn
                })
            })
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor continuation drive owner outcome k{} 缺少 owner callable {:?}",
                    surface.continuation_schema().as_u32(),
                    target.owner_version_key()
                ))
            })?;
        let mir_fun = refactor_mir_callable(pass_view, callable.root_fqn())?;
        let body = mir_fun.body.as_ref().ok_or_else(|| {
            frontend_error(format!(
                "refactor continuation drive owner outcome `{}` owner `{}` 缺少 canonical MIR body",
                outcome_fun.get_name().to_str().unwrap_or("<invalid>"),
                callable.root_fqn()
            ))
        })?;
        let entry = self.context.append_basic_block(outcome_fun, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(outcome_fun)?;
        RefactorCallableEmitter::new(
            self,
            program,
            source_types,
            pass_view,
            abi,
            callable,
            mir_fun,
            body,
            outcome_fun,
            None,
            None,
            self.collect_surface_resume_handle_sites(target),
            RefactorHandleCompletionMode::ReturnFromFunction,
        )?
        .emit_generated_continuation_resume_driver(surface)?;
        self.finish_function_explicit_frame_layout(mir_fun.span)?;
        Ok(())
    }

    pub(super) fn load_gc_object_type_desc(
        &mut self,
        obj: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let header_ty = self.llvm_gc_object_header_type();
        let header_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let header_ptr =
            self.builder
                .build_pointer_cast(obj, header_ptr_ty, &format!("{name}_hdr"))?;
        let type_desc_ptr =
            self.builder
                .build_struct_gep(header_ty, header_ptr, 1, &format!("{name}_gep"))?;
        Ok(self
            .builder
            .build_load(self.llvm_i8_ptr_type(), type_desc_ptr, name)?
            .into_pointer_value())
    }

    pub(super) fn codegen_refactor_surface_resume_owner_trampoline(
        &mut self,
        program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        pass_view: &'a mir::MaterializedMirPassView<'a>,
        abi: &ProgramAbiQuery<'ctx>,
        surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
        target: &super::super::types::RefactorContinuationSurfaceResumeOwnerTrampolineLayout<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let function = self
            .module
            .get_function(target.symbol_name())
            .unwrap_or_else(|| {
                self.declare_compiler_private_helper_function(
                    target.symbol_name(),
                    target.llvm_ty(),
                    Linkage::Internal,
                )
            });
        if function.count_basic_blocks() > 0 {
            return Ok(());
        }
        let callable = program
            .callable_by_version_key(target.owner_version_key())
            .or_else(|| {
                program.callables().iter().find(|candidate| {
                    candidate.body_version_key().surface_instance()
                        == target.owner_version_key().surface_instance()
                })
            })
            .or_else(|| {
                program.callables().iter().find(|candidate| {
                    candidate.root_fqn()
                        == target.owner_version_key().surface_instance().template.fqn
                })
            })
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor surface resume owner dispatch k{} 缺少 owner callable {:?}",
                    surface.continuation_schema().as_u32(),
                    target.owner_version_key()
                ))
            })?;
        if callable.step_schema() != target.owner_step_schema()
            && callable.body_version_key().surface_instance()
                != target.owner_version_key().surface_instance()
        {
            return Err(frontend_error(format!(
                "refactor surface resume owner dispatch k{} owner step schema 漂移：callable=s{} target=s{}",
                surface.continuation_schema().as_u32(),
                callable.step_schema().as_u32(),
                target.owner_step_schema().as_u32()
            )));
        }
        let mir_fun = refactor_mir_callable(pass_view, callable.root_fqn())?;
        let body = mir_fun.body.as_ref().ok_or_else(|| {
            frontend_error(format!(
                "refactor surface resume owner dispatch `{}` owner `{}` 缺少 canonical MIR body",
                target.symbol_name(),
                callable.root_fqn()
            ))
        })?;
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(function)?;
        let mut surface_handle_sites = target
            .handle_binder_routes()
            .iter()
            .map(|route| route.site_id())
            .collect::<BTreeSet<_>>();
        if let Some(projection) = target.wrapper_projection()
            && let LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                site_id,
                ..
            } = projection.underlying_route().publication()
        {
            surface_handle_sites.insert(*site_id);
        }
        let surface_handle_sites =
            (!surface_handle_sites.is_empty()).then_some(surface_handle_sites);
        if abi.frame_layout(target.owner_step_schema()).is_none()
            && target.resume_boundary_sites().is_empty()
            && target.handle_binder_routes().is_empty()
            && target.wrapper_projection().is_none()
        {
            self.builder.build_unreachable()?;
            return Ok(());
        }
        let return_step_schema = (target.wrapper_projection().is_none()
            && target.owner_step_schema() != surface.return_step_schema())
        .then_some(surface.return_step_schema());
        RefactorCallableEmitter::new(
            self,
            program,
            source_types,
            pass_view,
            abi,
            callable,
            mir_fun,
            body,
            function,
            target.wrapper_projection(),
            return_step_schema,
            surface_handle_sites,
            RefactorHandleCompletionMode::ReturnFromFunction,
        )?
        .emit_resume_entry(surface.resume_tuple_ty())?;
        self.finish_function_explicit_frame_layout(mir_fun.span)?;
        Ok(())
    }

    pub(super) fn current_function(&self) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        self.builder
            .get_insert_block()
            .and_then(|bb| bb.get_parent())
            .ok_or_else(|| {
                frontend_error(
                    "refactor body lowering 当前 builder 没有 active function".to_string(),
                )
            })
    }

    pub(super) fn refactor_function(
        &self,
        symbol_name: &str,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        self.module.get_function(symbol_name).ok_or_else(|| {
            frontend_error(format!(
                "refactor body lowering 缺少已发布 function shell `{symbol_name}`"
            ))
        })
    }

    pub(super) fn refactor_surface_resume_outcome_llvm_ty(
        &self,
        surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
    ) -> inkwell::types::FunctionType<'ctx> {
        let mut params = vec![self.llvm_gc_i8_ptr_type().into()];
        if !surface.resume_payload_abi().is_elided() {
            params.push(surface.resume_payload_abi().llvm_ty().into());
        }
        params.push(self.context.ptr_type(AddressSpace::default()).into());
        self.context.void_type().fn_type(&params, false)
    }

    pub(super) fn refactor_surface_resume_owner_outcome_llvm_ty(
        &self,
        surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
    ) -> inkwell::types::FunctionType<'ctx> {
        self.refactor_surface_resume_outcome_llvm_ty(surface)
    }

    pub(super) fn refactor_surface_resume_owner_core_llvm_ty(
        &self,
        surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
    ) -> inkwell::types::FunctionType<'ctx> {
        let mut params = vec![self.llvm_gc_i8_ptr_type().into()];
        if !surface.resume_payload_abi().is_elided() {
            params.push(surface.resume_payload_abi().llvm_ty().into());
        }
        params.push(self.llvm_gc_i8_ptr_type().into());
        params.push(self.llvm_gc_i8_ptr_type().into());
        params.push(self.context.ptr_type(AddressSpace::default()).into());
        self.context.void_type().fn_type(&params, false)
    }

    pub(super) fn refactor_surface_resume_outcome_function(
        &mut self,
        surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
    ) -> FunctionValue<'ctx> {
        let symbol_name = refactor_surface_resume_outcome_symbol_name(surface);
        let llvm_ty = self.refactor_surface_resume_outcome_llvm_ty(surface);
        self.declare_compiler_private_helper_function(&symbol_name, llvm_ty, Linkage::Internal)
    }

    pub(super) fn refactor_surface_resume_owner_outcome_function(
        &mut self,
        surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
        target: &super::super::types::RefactorContinuationSurfaceResumeOwnerTrampolineLayout<'ctx>,
    ) -> FunctionValue<'ctx> {
        let symbol_name = refactor_surface_resume_owner_outcome_symbol_name(target);
        let llvm_ty = self.refactor_surface_resume_owner_outcome_llvm_ty(surface);
        self.declare_compiler_private_helper_function(&symbol_name, llvm_ty, Linkage::Internal)
    }

    pub(super) fn refactor_surface_resume_owner_core_function(
        &mut self,
        surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
        target: &super::super::types::RefactorContinuationSurfaceResumeOwnerTrampolineLayout<'ctx>,
    ) -> FunctionValue<'ctx> {
        let symbol_name = refactor_surface_resume_owner_core_symbol_name(target);
        let llvm_ty = self.refactor_surface_resume_owner_core_llvm_ty(surface);
        self.declare_compiler_private_helper_function(&symbol_name, llvm_ty, Linkage::Internal)
    }

    pub(super) fn refactor_continuation_drive_outcome_llvm_ty(
        &self,
    ) -> inkwell::types::FunctionType<'ctx> {
        let params = [
            self.llvm_gc_i8_ptr_type().into(),
            self.context.i64_type().into(),
            self.llvm_gc_i8_ptr_type().into(),
            self.llvm_i8_ptr_type().into(),
            self.context.ptr_type(AddressSpace::default()).into(),
        ];
        self.context.void_type().fn_type(&params, false)
    }

    pub(super) fn refactor_continuation_step_llvm_ty(&self) -> inkwell::types::FunctionType<'ctx> {
        let params = [
            self.llvm_gc_i8_ptr_type().into(),
            self.context.i64_type().into(),
            self.llvm_gc_i8_ptr_type().into(),
            self.llvm_gc_i8_ptr_type().into(),
            self.llvm_gc_i8_ptr_type().into(),
            self.context.ptr_type(AddressSpace::default()).into(),
        ];
        self.context.void_type().fn_type(&params, false)
    }

    pub(super) fn refactor_continuation_drive_outcome_function(
        &mut self,
        surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
    ) -> FunctionValue<'ctx> {
        let symbol_name = refactor_continuation_drive_outcome_symbol_name(surface);
        let llvm_ty = self.refactor_continuation_drive_outcome_llvm_ty();
        self.declare_compiler_private_helper_function(&symbol_name, llvm_ty, Linkage::Internal)
    }

    pub(super) fn refactor_continuation_drive_owner_outcome_function(
        &mut self,
        _surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
        target: &super::super::types::RefactorContinuationSurfaceResumeOwnerTrampolineLayout<'ctx>,
    ) -> FunctionValue<'ctx> {
        let symbol_name = refactor_continuation_drive_owner_outcome_symbol_name(target);
        let llvm_ty = self.refactor_continuation_drive_outcome_llvm_ty();
        self.declare_compiler_private_helper_function(&symbol_name, llvm_ty, Linkage::Internal)
    }

    pub(super) fn refactor_continuation_step_function(
        &mut self,
        target: &super::super::types::RefactorContinuationSurfaceResumeOwnerTrampolineLayout<'ctx>,
    ) -> FunctionValue<'ctx> {
        let symbol_name = refactor_continuation_step_symbol_name(target);
        let llvm_ty = self.refactor_continuation_step_llvm_ty();
        self.declare_compiler_private_helper_function(&symbol_name, llvm_ty, Linkage::Internal)
    }
}
