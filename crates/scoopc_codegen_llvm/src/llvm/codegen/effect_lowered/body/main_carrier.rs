//! Callable carrier entry shells: builds the dispatch entry that carriers (closures, virtual receivers, interface receivers) jump into, plus the closure-env packing/unpacking that backs them.

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(super) fn codegen_callable_entries(
        &mut self,
        program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        abi: &ProgramAbiQuery<'ctx>,
        callable: &'a LateLoweredCallable,
    ) -> Result<(), LlvmEmitError> {
        let layout = abi.callable_layout_by_version_key(callable.body_version_key())?;
        validate_callable_entry_layout(layout)?;
        let direct_fun = self.function(layout.direct_entry().symbol_name())?;
        if direct_fun.count_basic_blocks() == 0 {
            let (mir_fun, body) = callable_source_body(callable, "body lowering")?;
            let entry = self.context.append_basic_block(direct_fun, "entry");
            self.builder.position_at_end(entry);
            self.begin_function_explicit_frame_layout(direct_fun)?;
            CallableEmitter::new(
                self,
                program,
                source_types,
                abi,
                callable,
                mir_fun,
                body,
                direct_fun,
                None,
                None,
                None,
                HandleCompletionMode::ContinueToExit,
            )?
            .emit_direct(layout.direct_entry())?;
            self.finish_function_explicit_frame_layout(mir_fun.span)?;
        }

        let dynamic_fun = self.function(layout.dynamic_entry().symbol_name())?;
        if dynamic_fun.count_basic_blocks() == 0 {
            let entry = self.context.append_basic_block(dynamic_fun, "entry");
            self.builder.position_at_end(entry);
            let mut args = Vec::<BasicMetadataValueEnum<'ctx>>::new();
            if layout.dynamic_entry().param_count() > 0 {
                let arg = dynamic_fun.get_nth_param(0).ok_or_else(|| {
                    frontend_error(format!(
                        "dynamic entry `{}` 缺少 args tuple 参数",
                        layout.dynamic_entry().symbol_name()
                    ))
                })?;
                args.push(arg.into());
            }
            let call = self
                .builder
                .build_call(direct_fun, &args, "dynamic_to_direct")?;
            let value = call.try_as_basic_value().basic().ok_or_else(|| {
                frontend_error(format!(
                    "direct entry `{}` 未返回 Step_F",
                    layout.direct_entry().symbol_name()
                ))
            })?;
            self.builder.build_return(Some(&value))?;
        }
        Ok(())
    }

    pub(super) fn codegen_callable_carrier_entry_shell(
        &mut self,
        kind: CallableCarrierKind,
        carrier_fqn: &str,
        target: &super::super::types::CallableCarrierTargetLayout,
        program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        abi: &ProgramAbiQuery<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let target_layout = abi.callable_layout_by_version_key(target.body_version_key())?;
        let function = self.function(target.symbol_name())?;
        if function.count_basic_blocks() > 0 {
            return Ok(());
        }
        let target_callable = program.callable(target_layout.root_fqn()).ok_or_else(|| {
            frontend_error(format!(
                "carrier shell `{}` 缺少 target callable `{}` 的 LIR body contract",
                target.symbol_name(),
                target_layout.root_fqn()
            ))
        })?;
        let mir_fun = callable_source(target_callable, "carrier shell")?;
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        let direct_entry = target_layout.direct_entry();
        let args_payload = self.build_carrier_direct_args(
            kind,
            carrier_fqn,
            function,
            mir_fun,
            source_types,
            abi,
            direct_entry,
        )?;
        let direct_fun = self.function(direct_entry.symbol_name())?;
        let mut args = Vec::<BasicMetadataValueEnum<'ctx>>::new();
        if !direct_entry.args_abi().is_elided() {
            args.push(
                args_payload
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "carrier shell `{}` 需要 non-elided direct args payload",
                            target.symbol_name()
                        ))
                    })?
                    .into(),
            );
        }
        let call = self
            .builder
            .build_call(direct_fun, &args, "carrier_to_direct")?;
        let step = call.try_as_basic_value().basic().ok_or_else(|| {
            frontend_error(format!(
                "carrier shell `{}` direct entry 未返回 Step_F",
                target.symbol_name()
            ))
        })?;
        let returned_step = if target.step_schema() == target_layout.step_schema() {
            step
        } else {
            self.project_step_to_schema(
                abi,
                step,
                target_layout.step_schema(),
                target.step_schema(),
            )?
        };
        self.builder.build_return(Some(&returned_step))?;
        Ok(())
    }

    pub(in crate::llvm::codegen::effect_lowered) fn project_step_to_schema(
        &mut self,
        abi: &ProgramAbiQuery<'ctx>,
        owner_step: BasicValueEnum<'ctx>,
        owner_step_schema: StepSchemaId,
        wrapper_step_schema: StepSchemaId,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        let owner_layout = abi.step_layout(owner_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "carrier projection 缺少 owner step schema s{} layout",
                owner_step_schema.as_u32()
            ))
        })?;
        let wrapper_layout = abi.step_layout(wrapper_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "carrier projection 缺少 wrapper step schema s{} layout",
                wrapper_step_schema.as_u32()
            ))
        })?;
        let tag = self.extract_step_tag(owner_layout, owner_step)?;
        let function = self.current_function()?;
        let complete_bb = self
            .context
            .append_basic_block(function, "carrier_project_complete");
        let dispatch_bb = self
            .context
            .append_basic_block(function, "carrier_project_dispatch");
        let unmatched_bb = self
            .context
            .append_basic_block(function, "carrier_project_unmatched");
        let done_bb = self
            .context
            .append_basic_block(function, "carrier_project_done");
        let is_complete = self.builder.build_int_compare(
            inkwell::IntPredicate::EQ,
            tag,
            self.context.i32_type().const_int(STEP_TAG_COMPLETE, false),
            "carrier_project_is_complete",
        )?;
        self.builder
            .build_conditional_branch(is_complete, complete_bb, dispatch_bb)?;

        self.builder.position_at_end(dispatch_bb);
        let mut case_targets = Vec::new();
        for wrapper_case in wrapper_layout.cases() {
            let Some(owner_case) = owner_layout.cases().iter().find(|owner_case| {
                owner_case.1.concrete_op_key() == wrapper_case.1.concrete_op_key()
                    && owner_case.1.payload_tuple_ty() == wrapper_case.1.payload_tuple_ty()
            }) else {
                continue;
            };
            let bb = self.context.append_basic_block(
                function,
                &format!("carrier_project_case{}", wrapper_case.1.case_tag().as_u32()),
            );
            case_targets.push((
                self.context
                    .i32_type()
                    .const_int(owner_case.1.variant().tag_value() as u64, false),
                bb,
                owner_case.1.case_tag(),
                wrapper_case.1.case_tag(),
            ));
        }
        let switch_cases = case_targets
            .iter()
            .map(|(tag, bb, _, _)| (*tag, *bb))
            .collect::<Vec<_>>();
        self.builder
            .build_switch(tag, unmatched_bb, &switch_cases)?;

        let phi_ty = wrapper_layout.llvm_ty();
        self.builder.position_at_end(complete_bb);
        let complete_payload = self.extract_step_payload(
            owner_layout,
            owner_step,
            owner_layout.complete_variant(),
            "carrier_project_complete_payload",
        )?;
        let complete_step = self.build_step_complete(wrapper_layout, complete_payload)?;
        self.builder.build_unconditional_branch(done_bb)?;
        let complete_incoming = self.builder.get_insert_block().ok_or_else(|| {
            frontend_error("carrier projection complete block missing".to_string())
        })?;

        let mut incomings = vec![(complete_step, complete_incoming)];
        for (_, bb, owner_case_tag, wrapper_case_tag) in case_targets {
            self.builder.position_at_end(bb);
            let owner_case = owner_layout.case_layout(owner_case_tag).ok_or_else(|| {
                frontend_error(format!(
                    "carrier projection 缺少 owner case c{}",
                    owner_case_tag.as_u32()
                ))
            })?;
            let wrapper_case = wrapper_layout
                .case_layout(wrapper_case_tag)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "carrier projection 缺少 wrapper case c{}",
                        wrapper_case_tag.as_u32()
                    ))
                })?;
            let (payload, continuation) = self.extract_step_case_parts(
                owner_layout,
                owner_step,
                owner_case,
                "carrier_project_case_payload",
            )?;
            let projected =
                self.build_step_case(wrapper_layout, wrapper_case, payload, continuation)?;
            self.builder.build_unconditional_branch(done_bb)?;
            let incoming_bb = self.builder.get_insert_block().ok_or_else(|| {
                frontend_error("carrier projection case block missing".to_string())
            })?;
            incomings.push((projected, incoming_bb));
        }

        self.builder.position_at_end(unmatched_bb);
        self.builder.build_unreachable()?;

        self.builder.position_at_end(done_bb);
        let phi = self.builder.build_phi(phi_ty, "carrier_projected_step")?;
        for (value, bb) in &incomings {
            phi.add_incoming(&[(value, *bb)]);
        }
        Ok(phi.as_basic_value())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn build_carrier_direct_args(
        &mut self,
        kind: CallableCarrierKind,
        carrier_fqn: &str,
        function: FunctionValue<'ctx>,
        mir_fun: &mir::FunDecl,
        source_types: &TypeStore,
        abi: &ProgramAbiQuery<'ctx>,
        direct_entry: &CallableEntryLayout<'ctx>,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        let direct_args_layout = abi.source_value_layout(direct_entry.invoke_args_tuple_ty())?;
        let direct_component_count = source_layout_component_count(direct_args_layout);
        let receiver = function.get_nth_param(0).ok_or_else(|| {
            frontend_error(format!(
                "carrier shell `{}` 缺少 receiver/carrier 参数",
                carrier_fqn
            ))
        })?;
        let mut components = vec![None; direct_component_count.max(mir_fun.params.len())];
        let (explicit_param_start, explicit_component_start) = match kind {
            CallableCarrierKind::ClosureObject if mir_fun.name.starts_with("$lambda") => {
                let flatten_env = mir_fun.params.first().is_some_and(|param| {
                    mir_fun.params.len() == 1 && param.ty == direct_entry.invoke_args_tuple_ty()
                });
                let env_components = self.load_closure_env_components(
                    receiver.into_pointer_value(),
                    mir_fun,
                    source_types,
                    flatten_env,
                )?;
                let env_component_count = env_components.len();
                components = env_components;
                // In the non-flattened lambda ABI the env parameter always occupies exactly
                // one leading source-tuple slot (component 0), even when it is `Unit` and thus
                // elided from the layout: the invoke-args tuple is `(env, explicit_params...)`.
                // Explicit params therefore start at source slot 1. In the flattened case the
                // single env param *is* the whole invoke-args tuple, so its captures fill all
                // leading slots and there are no explicit params to place.
                let env_slot_count = if flatten_env { env_component_count } else { 1 };
                components.resize(direct_component_count.max(env_slot_count), None);
                (1, env_slot_count)
            }
            CallableCarrierKind::ClosureObject => (0, 0),
            CallableCarrierKind::ClassVtable | CallableCarrierKind::InterfaceItable => {
                if components.is_empty() {
                    return Err(frontend_error(format!(
                        "dispatch carrier `{carrier_fqn}` direct entry 缺少 receiver 参数"
                    )));
                }
                components[0] = Some(receiver);
                (1, 1)
            }
        };
        // Closure carriers for a single tuple-typed parameter already receive the exact
        // invoke-args ABI payload as their explicit args parameter; forwarding it intact
        // preserves the authoritative tuple source layout without dropping components.
        if matches!(kind, CallableCarrierKind::ClosureObject)
            && explicit_param_start == 0
            && explicit_component_start == 0
            && mir_fun.params.len() == 1
            && mir_fun.params[0].ty == direct_entry.invoke_args_tuple_ty()
            && matches!(
                source_types.kind(mir_fun.params[0].ty),
                TypeKind::Value(ValueTypeKind::Tuple(_))
            )
        {
            return if direct_args_layout.abi().is_elided() {
                Ok(None)
            } else {
                function.get_nth_param(1).map(Some).ok_or_else(|| {
                    frontend_error(format!(
                        "carrier shell `{}` 缺少 explicit args payload 参数",
                        mir_fun.fqn
                    ))
                })
            };
        }
        self.unpack_carrier_explicit_args(
            function,
            mir_fun,
            explicit_param_start,
            explicit_component_start,
            source_types,
            &mut components,
        )?;
        self.build_source_payload_from_components(
            direct_args_layout,
            &components,
            "carrier_direct_args",
        )
    }

    pub(super) fn unpack_carrier_explicit_args(
        &mut self,
        function: FunctionValue<'ctx>,
        mir_fun: &mir::FunDecl,
        explicit_param_start: usize,
        explicit_component_start: usize,
        source_types: &TypeStore,
        components: &mut [Option<BasicValueEnum<'ctx>>],
    ) -> Result<(), LlvmEmitError> {
        if explicit_param_start > mir_fun.params.len() {
            return Err(frontend_error(format!(
                "carrier shell `{}` explicit arg 起点越界：start={} params={}",
                mir_fun.fqn,
                explicit_param_start,
                mir_fun.params.len(),
            )));
        }
        let explicit_params = &mir_fun.params[explicit_param_start..];
        if explicit_params.is_empty() {
            return Ok(());
        }
        let needed_components = explicit_component_start + explicit_params.len();
        if needed_components > components.len() {
            return Err(frontend_error(format!(
                "carrier shell `{}` explicit arg component range 越界：start={} count={} components={}",
                mir_fun.fqn,
                explicit_component_start,
                explicit_params.len(),
                components.len(),
            )));
        }
        let elided = explicit_params
            .iter()
            .map(|param| self.source_type_is_elided(param.span, source_types, param.ty))
            .collect::<Result<Vec<_>, _>>()?;
        if elided.iter().all(|is_elided| *is_elided) {
            return Ok(());
        }
        let raw = function.get_nth_param(1).ok_or_else(|| {
            frontend_error(format!(
                "carrier shell `{}` 缺少 explicit args payload 参数",
                mir_fun.fqn
            ))
        })?;
        if explicit_params.len() == 1 {
            components[explicit_component_start] = Some(raw);
            return Ok(());
        }
        let BasicValueEnum::StructValue(tuple) = raw else {
            return Err(frontend_error(format!(
                "carrier shell `{}` explicit args payload 不是 tuple struct",
                mir_fun.fqn
            )));
        };
        let mut abi_field = 0u32;
        for (offset, is_elided) in elided.into_iter().enumerate() {
            if is_elided {
                continue;
            }
            let raw_field = self.builder.build_extract_value(
                tuple,
                abi_field,
                &format!("carrier_arg{offset}"),
            )?;
            components[explicit_component_start + offset] = Some(raw_field);
            abi_field += 1;
        }
        Ok(())
    }

    pub(super) fn source_type_is_elided(
        &mut self,
        span: crate::span::Span,
        source_types: &TypeStore,
        ty: TypeId,
    ) -> Result<bool, LlvmEmitError> {
        let cg_ty =
            self.cg_ty_of_mir_type(source_types, ty)
                .unwrap_or_else(|| {
                    panic!(
                        "source_type_is_elided: carrier ABI verifier accepted non-codegen source type at {span:?}"
                    )
                });
        let llvm_ty = self.llvm_basic_type_of(span, cg_ty)?;
        Ok(self.target_data.get_store_size(&llvm_ty) == 0)
    }

    pub(super) fn load_closure_env_components(
        &mut self,
        closure_obj_i8: PointerValue<'ctx>,
        mir_fun: &mir::FunDecl,
        source_types: &TypeStore,
        flatten_env: bool,
    ) -> Result<Vec<Option<BasicValueEnum<'ctx>>>, LlvmEmitError> {
        let Some(env_param) = mir_fun.params.first() else {
            return Err(frontend_error(format!(
                "closure carrier `{}` 缺少 lambda env 参数",
                mir_fun.fqn
            )));
        };
        let env_cg = self.cg_ty_of_mir_type(source_types, env_param.ty).unwrap_or_else(|| {
            panic!(
                "load_closure_env_components: closure ABI verifier accepted non-codegen env type at {:?}",
                env_param.span
            )
        });
        if env_cg == CgTy::Unit {
            return Ok(Vec::new());
        }
        let CgTy::Tuple(tuple_ty) = env_cg else {
            panic!(
                "load_closure_env_components: closure ABI verifier accepted non-tuple env payload at {:?}",
                env_param.span
            );
        };
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = self.types.kind(tuple_ty.inner())
        else {
            panic!(
                "load_closure_env_components: closure ABI verifier accepted non-tuple env type id at {:?}",
                env_param.span
            );
        };
        let capture_cgs = elements
            .iter()
            .map(|ty| {
                self.try_cg_ty_of_type_id(*ty)
                    .unwrap_or_else(|| {
                        panic!(
                            "load_closure_env_components: closure ABI verifier accepted non-codegen capture type at {:?}",
                            env_param.span
                        )
                    })
            })
            .collect::<Vec<_>>();
        let env_obj_ty =
            self.closure_env_object_type(env_param.span, &mir_fun.fqn, source_types, &capture_cgs)?;
        let env_i8 = self.load_closure_env_ref(closure_obj_i8)?;
        let env_ptr = self.cast_ptr(
            env_i8,
            self.context.ptr_type(self.gc_address_space()),
            "closure_env_obj",
        )?;
        let mut components = Vec::new();
        let mut aggregate = if flatten_env {
            None
        } else {
            let BasicTypeEnum::StructType(env_tuple_ty) =
                self.llvm_basic_type_of(env_param.span, env_cg)?
            else {
                return Err(frontend_error(format!(
                    "closure env `{}` 的 env tuple LLVM type 不是 struct",
                    mir_fun.fqn,
                )));
            };
            Some(env_tuple_ty.get_undef())
        };
        for (index, capture_cg) in capture_cgs.iter().enumerate() {
            let field_ty = self.llvm_basic_type_of(env_param.span, *capture_cg)?;
            let raw = if matches!(capture_cg, CgTy::Unit | CgTy::Never) {
                self.zero_initializer_for_basic_type(field_ty)
            } else {
                let env_field_index = (index + 1) as u32;
                if env_field_index >= env_obj_ty.count_fields() {
                    return Err(frontend_error(format!(
                        "closure env object `{}` 缺少 capture field {}（field_count={}）",
                        mir_fun.fqn,
                        env_field_index,
                        env_obj_ty.count_fields(),
                    )));
                }
                let field_gep = self.builder.build_struct_gep(
                    env_obj_ty,
                    env_ptr,
                    env_field_index,
                    &format!("closure_env_field{index}_gep"),
                )?;
                self.builder.build_load(
                    field_ty,
                    field_gep,
                    &format!("closure_env_field{index}"),
                )?
            };
            if let Some(current) = aggregate.take() {
                aggregate = Some(
                    self.builder
                        .build_insert_value(
                            current,
                            raw,
                            index as u32,
                            &format!("closure_env_tuple_field{index}"),
                        )?
                        .into_struct_value(),
                );
            } else {
                components.push(Some(raw));
            }
        }
        if let Some(aggregate) = aggregate {
            Ok(vec![Some(aggregate.into())])
        } else {
            Ok(components)
        }
    }

    pub(super) fn load_closure_env_ref(
        &mut self,
        closure_obj_i8: PointerValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let closure_ty = self.llvm_closure_object_type();
        if closure_ty.count_fields() <= 1 {
            return Err(frontend_error(format!(
                "closure object layout 缺少 env field（field_count={}）",
                closure_ty.count_fields(),
            )));
        }
        let closure_ptr = self.cast_ptr(
            closure_obj_i8,
            self.context.ptr_type(self.gc_address_space()),
            "closure_obj",
        )?;
        let env_gep =
            self.builder
                .build_struct_gep(closure_ty, closure_ptr, 1, "closure_env_gep")?;
        Ok(self
            .builder
            .build_load(self.llvm_gc_i8_ptr_type(), env_gep, "closure_env")?
            .into_pointer_value())
    }

    pub(super) fn closure_env_object_type(
        &mut self,
        span: crate::span::Span,
        fn_ptr: &str,
        source_types: &TypeStore,
        field_cgs: &[CgTy],
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let program = self.published_late_lowered_program().ok_or_else(|| {
            frontend_error(format!("closure env `{fn_ptr}` 缺少 published LIR program"))
        })?;
        let callable = program.callable(fn_ptr).ok_or_else(|| {
            frontend_error(format!(
                "closure env `{fn_ptr}` 缺少 published LIR callable stable key"
            ))
        })?;
        let stable_callable_key_text = super::stable_naming::callable_version_key_text(
            self.stable_cone_key,
            source_types,
            self.stable_type_param_resolver(),
            program,
            callable.body_version_key(),
            "closure env type",
        )?;
        let name =
            stable_naming::private_name_from_key_text("closure_env", &stable_callable_key_text);
        if let Some(existing) = self.context.get_struct_type(&name) {
            return Ok(existing);
        }
        let env_ty = self.context.opaque_struct_type(&name);
        let mut fields = Vec::with_capacity(1 + field_cgs.len());
        fields.push(self.llvm_gc_object_header_type().into());
        for cg in field_cgs {
            fields.push(self.llvm_basic_type_of(span, *cg)?);
        }
        env_ty.set_body(&fields, false);
        Ok(env_ty)
    }

    pub(super) fn build_source_payload_from_components(
        &mut self,
        layout: &SourceAbiLayout<'ctx>,
        components: &[Option<BasicValueEnum<'ctx>>],
        name: &str,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        if layout.abi().is_elided() {
            return Ok(None);
        }
        match layout.kind() {
            SourceAbiLayoutKind::Scalar => components
                .first()
                .and_then(|value| *value)
                .map(Some)
                .ok_or_else(|| {
                    frontend_error(format!("ABI scalar payload `{name}` 缺少 source component"))
                }),
            SourceAbiLayoutKind::Tuple => {
                let BasicTypeEnum::StructType(struct_ty) = layout.abi().llvm_ty() else {
                    return Err(frontend_error(format!(
                        "ABI tuple payload `{name}` layout 不是 struct"
                    )));
                };
                let mut aggregate = struct_ty.get_undef();
                for field in layout.fields() {
                    if field.is_elided() {
                        continue;
                    }
                    let source_index = field.source_index() as usize;
                    let raw = components
                        .get(source_index)
                        .and_then(|value| *value)
                        .ok_or_else(|| {
                            frontend_error(format!(
                                "ABI tuple payload `{name}` 缺少 source component {source_index}"
                            ))
                        })?;
                    aggregate = self
                        .builder
                        .build_insert_value(
                            aggregate,
                            raw,
                            field
                                .abi_field_index()
                                .expect("non-elided field has ABI index"),
                            &format!("{name}_field{source_index}"),
                        )?
                        .into_struct_value();
                }
                Ok(Some(aggregate.into()))
            }
        }
    }
}
