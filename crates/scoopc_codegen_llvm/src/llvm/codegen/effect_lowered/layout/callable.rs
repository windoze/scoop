//! Continuation objects, callable layouts, and surface-resume bindings.
//!
//! Each late-lowered callable surfaces in three flavours: the plain (effect-
//! neutral) shell, the version-indexed family for stateful callables, and
//! the continuation object that ties resume sites to body versions. Surface
//! resume bindings record which body satisfies each resume packing
//! interface.

use super::*;

impl<'cg, 'a, 'ctx> ProgramAbiMaterializer<'cg, 'a, 'ctx> {
    pub(super) fn materialize_continuation_object_layout(
        &mut self,
        object: &LateLoweredContinuationObject,
        surface_resume_layouts: &BTreeMap<
            ContinuationSchemaId,
            ContinuationSurfaceResumeLayout<'ctx>,
        >,
    ) -> Result<ContinuationObjectLayout<'ctx>, LlvmEmitError> {
        let owner_callable = self
            .program
            .callable_by_version_key(object.owner_version_key())
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 continuation object {} 的 owner callable",
                    object.object_id().as_u32()
                ))
            })?;
        let stable_owner_key_text = stable_naming::callable_version_key_text(
            self.stable_cone_key,
            self.source_types,
            self.codegen.stable_type_param_resolver(),
            self.program,
            owner_callable.body_version_key(),
            &format!("continuation object {}", object.object_id().as_u32()),
        )?;
        let cont_type_name = stable_naming::private_type_name_from_key_text(
            "Continuation",
            "continuation_type",
            &stable_owner_key_text,
        )?;
        let cont_anchor_name = stable_naming::private_name_from_key_text(
            "continuation_layout",
            &stable_owner_key_text,
        );
        let header_ty = self.codegen.llvm_gc_object_header_type();
        let gc_ref_ty = self.codegen.llvm_gc_i8_ptr_type();
        let resumed_ty = self.codegen.context.i32_type();
        let resume_state_ty = self.codegen.context.i32_type();
        let step_fn_ty = self.codegen.llvm_i8_ptr_type();
        let resume_word_ty = self.codegen.context.i64_type();
        let vtable_ptr_ty = self.codegen.llvm_i8_ptr_type();
        self.validate_published_resume_packing_ids(
            &format!("continuation object {}", object.object_id().as_u32()),
            owner_callable.step_schema(),
            object.implemented_packings(),
        )?;
        let surface_resume_bindings = self.materialize_surface_resume_bindings(
            object,
            owner_callable,
            surface_resume_layouts,
        )?;
        if object.implemented_packings() != owner_callable.resume_packings() {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 continuation object {} 的 implemented packings {} 与 owner callable `{}` 的 published resume packings {} 不一致",
                object.object_id().as_u32(),
                render_resume_packing_ids(object.implemented_packings()),
                owner_callable.root_fqn(),
                render_resume_packing_ids(owner_callable.resume_packings()),
            )));
        }
        let packing_ids = object.implemented_packings().to_vec();

        let mut llvm_fields: Vec<BasicTypeEnum<'ctx>> = vec![
            header_ty.into(),
            resumed_ty.into(),
            resume_state_ty.into(),
            gc_ref_ty.into(),
            gc_ref_ty.into(),
            step_fn_ty.into(),
            resume_word_ty.into(),
            gc_ref_ty.into(),
            gc_ref_ty.into(),
        ];
        let mut fields = vec![
            ContinuationFieldLayout::new(0, ContinuationFieldKind::Header, header_ty.into()),
            ContinuationFieldLayout::new(1, ContinuationFieldKind::ResumedFlag, resumed_ty.into()),
            ContinuationFieldLayout::new(
                2,
                ContinuationFieldKind::ResumeStateTag,
                resume_state_ty.into(),
            ),
            ContinuationFieldLayout::new(
                3,
                ContinuationFieldKind::CapturedEffectCtxRef,
                gc_ref_ty.into(),
            ),
            ContinuationFieldLayout::new(4, ContinuationFieldKind::StateRef, gc_ref_ty.into()),
            ContinuationFieldLayout::new(5, ContinuationFieldKind::StepFn, step_fn_ty.into()),
            ContinuationFieldLayout::new(
                6,
                ContinuationFieldKind::ResumeWord,
                resume_word_ty.into(),
            ),
            ContinuationFieldLayout::new(7, ContinuationFieldKind::ResumeGcRef, gc_ref_ty.into()),
            ContinuationFieldLayout::new(
                8,
                ContinuationFieldKind::CapturedCalleeSuspendStateRef,
                gc_ref_ty.into(),
            ),
        ];
        let mut packing_field_indices = BTreeMap::new();
        for interface_id in &packing_ids {
            let field_index = llvm_fields.len() as u32;
            llvm_fields.push(vtable_ptr_ty.into());
            fields.push(ContinuationFieldLayout::new(
                field_index,
                ContinuationFieldKind::PackingVtable(*interface_id),
                vtable_ptr_ty.into(),
            ));
            packing_field_indices.insert(*interface_id, field_index);
        }

        let cont_ty = self.define_named_struct(&cont_type_name, &llvm_fields);
        self.ensure_struct_anchor(&cont_anchor_name, cont_ty);
        Ok(ContinuationObjectLayout::new(
            object.object_id(),
            owner_callable.step_schema(),
            cont_ty,
            cont_anchor_name,
            fields,
            packing_field_indices,
            surface_resume_bindings,
        ))
    }

    pub(super) fn materialize_surface_resume_bindings(
        &self,
        object: &LateLoweredContinuationObject,
        owner_callable: &LateLoweredCallable,
        surface_resume_layouts: &BTreeMap<
            ContinuationSchemaId,
            ContinuationSurfaceResumeLayout<'ctx>,
        >,
    ) -> Result<BTreeMap<ContinuationSchemaId, Vec<ContinuationSurfaceResumeBinding>>, LlvmEmitError>
    {
        let mut bindings =
            BTreeMap::<ContinuationSchemaId, Vec<ContinuationSurfaceResumeBinding>>::new();
        let mut register_binding =
            |continuation_schema: ContinuationSchemaId,
             return_step_schema: StepSchemaId,
             case_tag: crate::effect_facts::CaseTag,
             reachability: crate::effect_lowered::ir::LateLoweredContinuationMethodReachability,
             source_label: &str|
             -> Result<(), LlvmEmitError> {
                let layout = surface_resume_layouts
                .get(&continuation_schema)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "LLVM ABI materialization 缺少 continuation object {} 需要的 continuation schema k{} surface-resume layout（来源：{source_label}）",
                        object.object_id().as_u32(),
                        continuation_schema.as_u32(),
                    ))
                })?;
                if layout.return_step_schema() != return_step_schema {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 continuation object {} 的 continuation schema k{} 在 {source_label} 上声明 out_step_schema=s{}，但已发布 surface-resume layout 的 return step schema 为 s{}",
                        object.object_id().as_u32(),
                        continuation_schema.as_u32(),
                        return_step_schema.as_u32(),
                        layout.return_step_schema().as_u32(),
                    )));
                }
                bindings.entry(continuation_schema).or_default().push(
                    ContinuationSurfaceResumeBinding::new(
                        continuation_schema,
                        return_step_schema,
                        case_tag,
                        reachability,
                    ),
                );
                Ok(())
            };

        for surface_resume in object.surface_resumes() {
            register_binding(
                surface_resume.continuation_schema(),
                surface_resume.out_step_schema(),
                surface_resume.case_tag(),
                surface_resume.reachability(),
                &format!(
                    "continuation object {} published surface resume case {}",
                    object.object_id().as_u32(),
                    surface_resume.case_tag().as_u32()
                ),
            )?;
        }

        let owner_step = self
            .program
            .step_type(owner_callable.step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 continuation object {} 的 owner step schema {}",
                    object.object_id().as_u32(),
                    owner_callable.step_schema().as_u32(),
                ))
            })?;
        for case in owner_step.cases() {
            if !bindings.contains_key(&case.continuation_schema()) {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 continuation object {} 缺少 owner step schema {} case {} 所需的 continuation schema k{} surface-resume 发布",
                    object.object_id().as_u32(),
                    owner_callable.step_schema().as_u32(),
                    case.case_tag().as_u32(),
                    case.continuation_schema().as_u32(),
                )));
            }
        }
        Ok(bindings)
    }

    pub(super) fn materialize_callable_layout(
        &mut self,
        callable: &LateLoweredCallable,
        step_layouts: &BTreeMap<StepSchemaId, StepLayout<'ctx>>,
    ) -> Result<CallableLayout<'ctx>, LlvmEmitError> {
        let step_layout = step_layouts.get(&callable.step_schema()).ok_or_else(|| {
            frontend_error(format!(
                "LLVM ABI materialization 缺少 callable `{}` 的 step layout {}",
                callable.root_fqn(),
                callable.step_schema().as_u32()
            ))
        })?;
        let step_ty = step_layout.llvm_ty();
        let args_layout =
            self.source_value_layout(callable.dynamic_invoke_entry().invoke_args_tuple_ty())?;
        let args_abi = *args_layout.abi();
        let mut params: Vec<BasicMetadataTypeEnum<'ctx>> = Vec::new();
        if !args_abi.is_elided() {
            params.push(args_abi.llvm_ty().into());
        }
        let dynamic_ty = step_ty.fn_type(&params, false);
        let direct_ty = step_ty.fn_type(&params, false);
        let stable_callable_key_text = stable_naming::callable_version_key_text(
            self.stable_cone_key,
            self.source_types,
            self.codegen.stable_type_param_resolver(),
            self.program,
            callable.body_version_key(),
            &format!("callable `{}`", callable.root_fqn()),
        )?;
        let callable_hash = callable
            .lir_callable_key()
            .map(scoopc_ids::LirCallableHash::from_stable_key)
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 callable `{}` 的 stable LIR callable key",
                    callable.root_fqn()
                ))
            })?;
        let dynamic_name = stable_naming::private_name_from_key_text(
            "dynamic_invoke",
            step_layout.stable_effect_key_text(),
        );
        let direct_name = stable_naming::private_name_from_key_text(
            "direct_invoke",
            step_layout.stable_effect_key_text(),
        );
        self.ensure_declared_compiler_private_helper_function(&dynamic_name, dynamic_ty);
        self.ensure_declared_compiler_private_helper_function(&direct_name, direct_ty);
        self.validate_published_resume_packing_ids(
            &format!("callable `{}`", callable.root_fqn()),
            callable.step_schema(),
            callable.resume_packings(),
        )?;
        let continuation_object = self
            .program
            .continuation_object(callable.continuation_object())
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 callable `{}` 的 continuation object {}",
                    callable.root_fqn(),
                    callable.continuation_object().as_u32()
                ))
            })?;
        if continuation_object.implemented_packings() != callable.resume_packings() {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{}` 的 published resume packings {} 与 continuation object {} 的 implemented packings {} 不一致",
                callable.root_fqn(),
                render_resume_packing_ids(callable.resume_packings()),
                continuation_object.object_id().as_u32(),
                render_resume_packing_ids(continuation_object.implemented_packings()),
            )));
        }
        let resume_packings = callable.resume_packings().to_vec();

        Ok(CallableLayout::new(
            self.origin,
            callable_hash,
            callable.root_fqn().to_string(),
            callable.body_version_key().clone(),
            stable_callable_key_text,
            callable.step_schema(),
            CallableEntryLayout::new(
                dynamic_name,
                dynamic_ty,
                params.len(),
                callable.dynamic_invoke_entry().invoke_args_tuple_ty(),
                args_abi,
                callable.step_schema(),
            ),
            CallableEntryLayout::new(
                direct_name,
                direct_ty,
                params.len(),
                callable.dynamic_invoke_entry().invoke_args_tuple_ty(),
                args_abi,
                callable.step_schema(),
            ),
            callable.continuation_object(),
            resume_packings,
        ))
    }

    pub(super) fn materialize_plain_callable_layout(
        &mut self,
        callable: &LateLoweredCallable,
    ) -> Result<PlainCallableLayout<'ctx>, LlvmEmitError> {
        let stable_callable_key_text = stable_naming::callable_version_key_text(
            self.stable_cone_key,
            self.source_types,
            self.codegen.stable_type_param_resolver(),
            self.program,
            callable.body_version_key(),
            &format!("plain callable `{}`", callable.root_fqn()),
        )?;
        let callable_hash = callable
            .lir_callable_key()
            .map(scoopc_ids::LirCallableHash::from_stable_key)
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 plain callable `{}` 的 stable LIR callable key",
                    callable.root_fqn()
                ))
            })?;
        let plain = callable.plain_abi().ok_or_else(|| {
            frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{}` 没有 plain ABI handoff",
                callable.root_fqn()
            ))
        })?;
        let plain_facts = self.plain_callable_facts(callable)?;
        let plain_function_ty = plain_facts.function_ty;
        let plain_param_tys = plain_facts.param_tys.clone();
        let plain_return_ty = plain_facts.return_ty;
        if plain_function_ty != plain.function_ty()
            || plain_return_ty != plain.return_ty()
            || plain_param_tys != plain.param_tys()
        {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 plain callable `{}` 的 LIR facts 与 LIR plain ABI handoff 漂移",
                callable.root_fqn()
            )));
        }
        let (symbol_name, surface, closure_like) = if callable.root_fqn().contains("$lambda") {
            let source_callable = callable.source_callable().ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 发现 plain closure `{}` 缺少 source callable contract",
                    callable.root_fqn()
                ))
            })?;
            (
                self.codegen
                    .lir_source_closure_body_symbol(callable.root_fqn(), source_callable.span)?,
                LlvmFunctionDeclarationSurface::CompilerPrivateHelper,
                true,
            )
        } else if callable.root_fqn() == "main" {
            (
                "__scoop_plain_source_main".to_string(),
                LlvmFunctionDeclarationSurface::CompilerPrivateHelper,
                false,
            )
        } else {
            (
                self.exported_plain_callable_symbol(callable)?,
                LlvmFunctionDeclarationSurface::ExportedAbi,
                false,
            )
        };
        let llvm_fun = self.codegen.declare_lir_plain_fun_with_symbol(
            &symbol_name,
            surface,
            callable.root_fqn(),
            &plain_param_tys,
            plain_return_ty,
            self.source_types,
            closure_like,
        )?;
        let symbol_name = llvm_fun
            .get_name()
            .to_str()
            .map_err(|_| {
                frontend_error(format!(
                    "LLVM ABI materialization 发现 plain callable `{}` 的 LLVM symbol 非 UTF-8",
                    callable.root_fqn()
                ))
            })?
            .to_string();
        Ok(PlainCallableLayout::new(
            callable_hash,
            callable.root_fqn().to_string(),
            callable.body_version_key().clone(),
            stable_callable_key_text,
            PlainCallableEntryLayout::new(
                symbol_name,
                llvm_fun.get_type(),
                llvm_fun.count_params() as usize,
                plain_function_ty,
                plain_param_tys,
                plain_return_ty,
            ),
        ))
    }

    fn exported_plain_callable_symbol(
        &self,
        callable: &LateLoweredCallable,
    ) -> Result<String, LlvmEmitError> {
        let id = self.callable_id(callable)?;
        let symbol_facts = self
            .program
            .physical_layout()
            .callable_symbols
            .get(&id)
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 plain callable `{}` 的 LIR symbol facts（id={:?}）",
                    callable.root_fqn(),
                    id
                ))
            })?;
        symbol_facts.exported_symbol.clone().ok_or_else(|| {
            frontend_error(format!(
                "plain callable `{}` 的 LIR symbol facts 缺少 exported ABI symbol",
                callable.root_fqn()
            ))
        })
    }

    pub(super) fn materialize_callable_version_layout_index(
        &self,
        callable_layouts: &BTreeMap<StepSchemaId, CallableLayout<'ctx>>,
    ) -> Result<HashMap<LateLoweredBodyVersionKey, StepSchemaId>, LlvmEmitError> {
        let mut index = HashMap::with_capacity(callable_layouts.len());
        for layout in callable_layouts.values() {
            let version_key = layout.body_version_key().clone();
            if let Some(existing_step_schema) =
                index.insert(version_key.clone(), layout.step_schema())
            {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 body version key {:?} 同时指向 callable step schema s{} 与 s{}",
                    version_key,
                    existing_step_schema.as_u32(),
                    layout.step_schema().as_u32(),
                )));
            }
        }
        Ok(index)
    }

    pub(super) fn materialize_known_instance_callable_versions(
        &self,
        callable_layouts: &BTreeMap<StepSchemaId, CallableLayout<'ctx>>,
    ) -> Result<HashMap<(InstanceKey, StepSchemaId), LateLoweredBodyVersionKey>, LlvmEmitError>
    {
        let mut selectors = HashMap::with_capacity(callable_layouts.len());
        for layout in callable_layouts.values() {
            let selector = (layout.surface_instance().clone(), layout.step_schema());
            let version_key = layout.body_version_key().clone();
            if let Some(existing) = selectors.insert(selector.clone(), version_key.clone()) {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 known-instance selector ({:?}, s{}) 同时指向多个 callable version：已有 {:?}，新值 {:?}",
                    selector.0,
                    selector.1.as_u32(),
                    existing,
                    version_key,
                )));
            }
        }
        Ok(selectors)
    }
}
