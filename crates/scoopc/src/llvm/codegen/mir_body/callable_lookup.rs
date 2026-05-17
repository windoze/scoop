//! Materialized MIR callable lookup and closure body emission.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn has_materialized_instances_for_template(
        &self,
        fqn: &str,
    ) -> bool {
        self.materialized_pass_view().is_some_and(|pass_view| {
            pass_view.instances().any(|family| {
                family.key().template.fqn == fqn
                    && (!family.key().type_args.is_empty() || !family.key().eff_args.is_empty())
            })
        })
    }

    pub(in crate::llvm::codegen) fn materialized_mir_callable(
        &self,
        fqn: &str,
    ) -> Option<(&TypeStore, &crate::mir::FunDecl)> {
        let pass_view = self.materialized_pass_view()?;
        let fqn_is_generic_template_with_instances = pass_view.instances().any(|family| {
            family.key().template.fqn == fqn
                && (!family.key().type_args.is_empty() || !family.key().eff_args.is_empty())
        });
        if fqn_is_generic_template_with_instances {
            return None;
        }
        let mir_fun = pass_view
            .callable(fqn)
            .or_else(|| {
                pass_view
                    .materialized()
                    .file
                    .items
                    .iter()
                    .find_map(|item| match item {
                        crate::mir::Item::Fun(fun) if fun.fqn == fqn && fun.body.is_some() => {
                            Some(fun)
                        }
                        _ => None,
                    })
            })
            .or_else(|| {
                pass_view
                    .materialized()
                    .caller_side_pass_candidate_bodies()
                    .iter()
                    .find(|fun| fun.fqn == fqn && fun.body.is_some())
            })?;
        Some((&pass_view.materialized().types, mir_fun))
    }

    pub(in crate::llvm::codegen) fn materialized_mir_closure_body_symbol(
        &self,
        callable_fqn: &str,
        at: crate::span::Span,
    ) -> Result<String, LlvmEmitError> {
        Ok(private_closure_body_fn_name(
            &self.stable_closure_key_for_materialized_callable(callable_fqn, at)?,
        ))
    }

    pub(in crate::llvm::codegen) fn inferred_materialized_direct_call_fqn(
        &self,
        template_fqn: &str,
        args: &[crate::mir::CallArg],
        result_source_ty: TypeId,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
    ) -> Option<String> {
        let pass_view = self.materialized_pass_view()?;
        let materialized_types = &pass_view.materialized().types;
        let arg_cg_tys = args
            .iter()
            .map(|arg| {
                let source_ty = self.mir_operand_type_id(body, &arg.value)?;
                self.equivalent_codegen_type_id(mir_types, source_ty)
                    .and_then(|ty| self.cg_ty_of(ty))
                    .or_else(|| self.cg_ty_of_mir_type(mir_types, source_ty))
                    .or_else(|| self.cg_ty_of(source_ty))
            })
            .collect::<Option<Vec<_>>>()?;
        let result_cg = self
            .equivalent_codegen_type_id(mir_types, result_source_ty)
            .and_then(|ty| self.cg_ty_of(ty))
            .or_else(|| self.cg_ty_of_mir_type(mir_types, result_source_ty))
            .or_else(|| self.cg_ty_of(result_source_ty))?;
        let mut matched: Option<String> = None;
        for family in pass_view.instances() {
            if family.key().template.fqn != template_fqn {
                continue;
            }
            let Some(fun) = family.root_body() else {
                continue;
            };
            if fun.params.len() != arg_cg_tys.len() {
                continue;
            }
            let params_match =
                fun.params
                    .iter()
                    .zip(arg_cg_tys.iter().copied())
                    .all(|(param, arg_cg)| {
                        self.cg_ty_of_mir_type(materialized_types, param.ty)
                            .is_some_and(|param_cg| param_cg == arg_cg)
                    });
            if !params_match {
                continue;
            }
            if self.cg_ty_of_mir_type(materialized_types, fun.return_ty) != Some(result_cg) {
                continue;
            }
            let candidate = family.root_fqn().to_string();
            if let Some(found) = matched.as_ref() {
                if found != &candidate {
                    return None;
                }
                continue;
            }
            matched = Some(candidate);
        }
        matched
    }

    pub(in crate::llvm::codegen) fn ensure_materialized_mir_closure_callable_defined(
        &mut self,
        span: crate::span::Span,
        fn_ptr: &str,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let (mir_types, mir_fun) =
            self.materialized_mir_callable(fn_ptr)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR closure function",
                    at: span.into(),
                })?;
        if !mir_fun.name.starts_with("$lambda") {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure function",
                at: span.into(),
            });
        }
        let body_symbol = self.materialized_mir_closure_body_symbol(fn_ptr, mir_fun.span)?;
        if let Some(existing) = self.module.get_function(&body_symbol)
            && existing.count_basic_blocks() > 0
        {
            return Ok(existing);
        }

        let saved_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let mut child = self.fresh_child_codegen();
        child.current_source_id = child.materialized_mir_callable_source_id(fn_ptr, span)?;
        let llvm_fun = child.declare_materialized_mir_closure_fun(span, mir_fun, mir_types)?;
        if llvm_fun.count_basic_blocks() == 0 {
            child.codegen_materialized_mir_closure_fun(mir_fun, mir_types, llvm_fun)?;
        }
        self.builder.position_at_end(saved_block);
        Ok(llvm_fun)
    }

    pub(in crate::llvm::codegen) fn hir_fun_for_callable_fqn(
        &self,
        fqn: &str,
    ) -> Option<&'a hir::FunDecl> {
        if let Some(pass_view) = self.materialized_pass_view()
            && let Some(owner) = pass_view.owner_of_callable(fqn)
            && let Some(hir_fun) = self.fun_index.values().copied().find(|fun| {
                fun.fqn == owner.template.fqn
                    && fun.source_path == owner.template.source_path
                    && fun.span == owner.template.decl_span
            })
        {
            return Some(hir_fun);
        }
        if let Some(hir_fun) = self.fun_index.get(fqn).copied() {
            return Some(hir_fun);
        }
        let base = mir_direct_call_base_fqn(fqn);
        if base != fqn {
            if let Some(pass_view) = self.materialized_pass_view()
                && let Some(owner) = pass_view.owner_of_callable(base)
                && let Some(hir_fun) = self.fun_index.values().copied().find(|fun| {
                    fun.fqn == owner.template.fqn
                        && fun.source_path == owner.template.source_path
                        && fun.span == owner.template.decl_span
                })
            {
                return Some(hir_fun);
            }
            if let Some(hir_fun) = self.fun_index.get(base).copied() {
                return Some(hir_fun);
            }
        }
        None
    }

    pub(in crate::llvm::codegen) fn materialized_mir_callable_source_id(
        &self,
        fqn: &str,
        span: crate::span::Span,
    ) -> Result<SourceId, LlvmEmitError> {
        let mut owner_fqn = fqn;
        loop {
            if let Some(hir_fun) = self.hir_fun_for_callable_fqn(owner_fqn) {
                return self.source_id_for_path(hir_fun.source_path.as_path(), span);
            }
            let Some((parent, _)) = owner_fqn.rsplit_once(".$lambda") else {
                break;
            };
            owner_fqn = parent;
        }
        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "pass MIR callable source path",
            at: span.into(),
        })
    }

    pub(in crate::llvm::codegen) fn declare_materialized_mir_closure_fun(
        &mut self,
        span: crate::span::Span,
        mir_fun: &crate::mir::FunDecl,
        mir_types: &TypeStore,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let param_tys = mir_fun
            .params
            .iter()
            .map(|param| param.ty)
            .collect::<Vec<_>>();
        self.declare_materialized_mir_closure_fun_with_signature(
            span,
            mir_fun,
            &param_tys,
            mir_fun.return_ty,
            mir_types,
        )
    }

    pub(in crate::llvm::codegen) fn declare_materialized_mir_closure_fun_with_signature(
        &mut self,
        span: crate::span::Span,
        mir_fun: &crate::mir::FunDecl,
        param_tys: &[TypeId],
        return_ty: TypeId,
        mir_types: &TypeStore,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let body_symbol = self.materialized_mir_closure_body_symbol(mir_fun.fqn.as_str(), span)?;
        if let Some(existing) = self.module.get_function(&body_symbol) {
            return Ok(existing);
        }
        if param_tys.len() != mir_fun.params.len() {
            return Err(frontend_error(format!(
                "materialized closure `{}` 的 plain ABI 参数数量({}) 与 MIR 参数数量({}) 不一致",
                mir_fun.fqn,
                param_tys.len(),
                mir_fun.params.len()
            )));
        }

        let ret_cg = self.cg_ty_of_mir_type(mir_types, return_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure return type",
                at: mir_fun.span.into(),
            },
        )?;
        let hidden_sret_result_ty = self.hidden_sret_result_ty(span, ret_cg)?;
        // 这里发布的是 plain callable ABI 的 closure body symbol；effect-step callable surface
        // 由 stage-owned direct/dynamic entry shell 单独承载，不应再为 plain entry 混入 hidden ABI。
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> =
            Vec::with_capacity(mir_fun.params.len() + usize::from(hidden_sret_result_ty.is_some()));
        if let Some(result_ty) = hidden_sret_result_ty {
            let _ = result_ty;
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        llvm_param_tys.push(self.llvm_gc_i8_ptr_type().into());
        for (param, param_ty) in mir_fun.params.iter().skip(1).zip(param_tys.iter().skip(1)) {
            let param_ty = self
                .equivalent_codegen_type_id(mir_types, *param_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR closure param type",
                    at: param.span.into(),
                })?;
            llvm_param_tys.push(
                self.ordinary_param_abi(param.span, param_ty)?
                    .llvm_param_ty(),
            );
        }

        let fn_ty = match (hidden_sret_result_ty, ret_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_param_tys, false)
            }
            (None, other) => self
                .llvm_basic_type_of(mir_fun.span, other)?
                .fn_type(&llvm_param_tys, false),
        };
        let llvm_fun =
            self.declare_compiler_private_helper_function(&body_symbol, fn_ty, Linkage::Internal);
        llvm_fun.set_call_conventions(0);
        if let Some(result_ty) = hidden_sret_result_ty {
            self.add_sret_attribute_to_function(llvm_fun, 0, result_ty);
        }
        Ok(llvm_fun)
    }

    pub(in crate::llvm::codegen) fn declare_materialized_mir_plain_fun_with_symbol(
        &mut self,
        llvm_name: &str,
        surface: LlvmFunctionDeclarationSurface,
        mir_fun: &crate::mir::FunDecl,
        param_tys: &[TypeId],
        return_ty: TypeId,
        mir_types: &TypeStore,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let llvm_name = match surface {
            LlvmFunctionDeclarationSurface::ExportedAbi => {
                self.exported_abi_symbol_for_materialized_fun(mir_fun, mir_types)?
            }
            LlvmFunctionDeclarationSurface::RuntimeOrNativeImport
            | LlvmFunctionDeclarationSurface::CompilerPrivateHelper => llvm_name.to_string(),
        };
        if let Some(existing) = self.module.get_function(&llvm_name) {
            return Ok(existing);
        }
        if param_tys.len() != mir_fun.params.len() {
            return Err(frontend_error(format!(
                "plain materialized callable `{}` 的 plain ABI 参数数量({}) 与 MIR 参数数量({}) 不一致",
                mir_fun.fqn,
                param_tys.len(),
                mir_fun.params.len()
            )));
        }

        let ret_cg = self.cg_ty_of_mir_type(mir_types, return_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR plain return type",
                at: mir_fun.span.into(),
            },
        )?;
        let hidden_sret_result_ty = self.hidden_sret_result_ty(mir_fun.span, ret_cg)?;
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> =
            Vec::with_capacity(mir_fun.params.len() + usize::from(hidden_sret_result_ty.is_some()));
        if let Some(result_ty) = hidden_sret_result_ty {
            let _ = result_ty;
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        for (param, param_ty) in mir_fun.params.iter().zip(param_tys.iter().copied()) {
            let param_ty = self.equivalent_codegen_type_id(mir_types, param_ty).ok_or_else(|| {
                tracing::warn!(
                    "declare_materialized_mir_plain_fun_with_symbol: unsupported param type for {} param {} -> {}",
                    mir_fun.fqn,
                    param.name,
                    mir_types.display(param_ty)
                );
                LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR plain param type",
                    at: param.span.into(),
                }
            })?;
            llvm_param_tys.push(
                self.ordinary_param_abi(param.span, param_ty)?
                    .llvm_param_ty(),
            );
        }

        let fn_ty = match (hidden_sret_result_ty, ret_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_param_tys, false)
            }
            (None, other) => self
                .llvm_basic_type_of(mir_fun.span, other)?
                .fn_type(&llvm_param_tys, false),
        };
        let llvm_fun = match surface {
            LlvmFunctionDeclarationSurface::ExportedAbi => {
                self.declare_exported_abi_function(&llvm_name, fn_ty)
            }
            LlvmFunctionDeclarationSurface::RuntimeOrNativeImport => {
                self.declare_runtime_or_native_import_function(&llvm_name, fn_ty)
            }
            LlvmFunctionDeclarationSurface::CompilerPrivateHelper => {
                self.declare_compiler_private_helper_function(&llvm_name, fn_ty, Linkage::Internal)
            }
        };
        llvm_fun.set_call_conventions(0);
        if let Some(result_ty) = hidden_sret_result_ty {
            self.add_sret_attribute_to_function(llvm_fun, 0, result_ty);
        }
        Ok(llvm_fun)
    }

    pub(in crate::llvm::codegen) fn codegen_materialized_mir_closure_fun(
        mut self,
        mir_fun: &crate::mir::FunDecl,
        mir_types: &TypeStore,
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let Some(body) = mir_fun.body.as_ref() else {
            return Ok(());
        };
        body.validate_cfg()
            .map_err(|_| LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR cfg",
                at: mir_fun.span.into(),
            })?;
        ensure_raw_mir_body_route_is_safe(&mir_fun.fqn, body)?;
        self.function_cx.current_callable_fqn = Some(mir_fun.fqn.clone());

        let declared_return_cg = self.cg_ty_of_mir_type(mir_types, mir_fun.return_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure return type",
                at: mir_fun.span.into(),
            },
        )?;
        let entry = self.context.append_basic_block(llvm_fun, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(llvm_fun)?;
        self.function_cx.current_fun_return_ty = Some(declared_return_cg);
        let uses_hidden_sret = self
            .hidden_sret_result_ty(mir_fun.span, declared_return_cg)?
            .is_some();
        self.function_cx.current_sret_return_ptr = if uses_hidden_sret {
            Some(
                llvm_fun
                    .get_nth_param(0)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "missing pass MIR llvm function sret param",
                        at: mir_fun.span.into(),
                    })?
                    .into_pointer_value(),
            )
        } else {
            None
        };
        self.clear_explicit_effect_hidden_abi_slots();

        let (return_bb, return_alloca) =
            self.setup_function_return_context(mir_fun.span, llvm_fun, declared_return_cg)?;
        let mut local_slots = self.create_mir_local_slots(body, mir_types)?;
        self.bind_mir_closure_params(
            mir_fun,
            mir_types,
            llvm_fun,
            u32::from(uses_hidden_sret),
            &mut local_slots,
        )?;
        let used_locals = collect_mir_local_uses(body);
        let llvm_blocks = body
            .blocks
            .iter()
            .enumerate()
            .map(|(idx, _)| {
                self.context
                    .append_basic_block(llvm_fun, &format!("mir.bb{idx}"))
            })
            .collect::<Vec<_>>();
        let start_bb = llvm_blocks
            .get(body.start.as_u32() as usize)
            .copied()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR start block",
                at: mir_fun.span.into(),
            })?;
        self.builder.build_unconditional_branch(start_bb)?;

        for (idx, block) in body.blocks.iter().enumerate() {
            self.builder.position_at_end(llvm_blocks[idx]);
            for stmt in &block.stmts {
                self.codegen_mir_statement(stmt, body, mir_types, &local_slots, &used_locals)?;
            }
            self.codegen_mir_terminator(
                &block.terminator,
                body,
                mir_types,
                &local_slots,
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
        self.clear_explicit_effect_hidden_abi_slots();
        self.function_cx.current_sret_return_ptr = None;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn bind_mir_closure_params(
        &mut self,
        mir_fun: &crate::mir::FunDecl,
        mir_types: &TypeStore,
        llvm_fun: FunctionValue<'ctx>,
        param_offset: u32,
        slots: &mut [MirLocalSlot<'ctx>],
    ) -> Result<(), LlvmEmitError> {
        let env_param = mir_fun
            .params
            .first()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure env param",
                at: mir_fun.span.into(),
            })?;
        if env_param.name != "$env" {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure env param",
                at: env_param.span.into(),
            });
        }
        let env_slot = slots
            .get(env_param.local.as_u32() as usize)
            .copied()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure env local",
                at: env_param.span.into(),
            })?;
        let env_init = self.codegen_mir_closure_env_param(
            env_param.span,
            &mir_fun.fqn,
            llvm_fun,
            param_offset,
            env_slot.cg_ty,
        )?;
        let _ = self.store_local_value(env_param.span, env_slot.ptr, env_slot.cg_ty, env_init)?;

        for (idx, param) in mir_fun.params.iter().enumerate().skip(1) {
            let slot = slots.get(param.local.as_u32() as usize).copied().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR param local",
                    at: param.span.into(),
                },
            )?;
            let param_ty = self.equivalent_codegen_type_id(mir_types, param.ty).ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR param type",
                    at: param.span.into(),
                },
            )?;
            let abi = self.ordinary_param_abi(param.span, param_ty)?;
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

    pub(in crate::llvm::codegen) fn codegen_mir_closure_env_param(
        &mut self,
        span: crate::span::Span,
        fn_ptr: &str,
        llvm_fun: FunctionValue<'ctx>,
        param_index: u32,
        env_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match env_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Tuple(tuple_ty) => {
                let capture_field_cgs = self.mir_closure_env_capture_element_cg_tys(env_cg).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR closure env shape",
                        at: span.into(),
                    },
                )?;
                let env_arg = llvm_fun
                    .get_nth_param(param_index)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "missing pass MIR closure env param",
                        at: span.into(),
                    })?
                    .into_pointer_value();
                let closure_key =
                    self.stable_closure_key_for_materialized_callable(fn_ptr, span)?;
                let env_ty =
                    self.mir_closure_env_object_type(span, &closure_key, &capture_field_cgs)?;
                let env_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
                let env_ptr = self.builder.build_pointer_cast(
                    env_arg,
                    env_ptr_ty,
                    "pass_mir_closure_env_ptr",
                )?;
                let llvm_tuple_ty = self.llvm_tuple_type(span, tuple_ty)?;
                let mut agg: AggregateValueEnum<'ctx> = llvm_tuple_ty.get_undef().into();
                for (idx, field_cg) in capture_field_cgs.iter().enumerate() {
                    let field_gep = self.builder.build_struct_gep(
                        env_ty,
                        env_ptr,
                        (idx + 1) as u32,
                        "pass_mir_closure_env_field_gep",
                    )?;
                    let field_raw = self.builder.build_load(
                        self.llvm_basic_type_of(span, *field_cg)?,
                        field_gep,
                        "pass_mir_closure_env_field_load",
                    )?;
                    agg = self.builder.build_insert_value(
                        agg,
                        field_raw,
                        idx as u32,
                        "pass_mir_closure_env_tuple_insert",
                    )?;
                }
                Ok(CgValue {
                    ty: env_cg,
                    value: Some(agg.as_basic_value_enum()),
                })
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure env type",
                at: span.into(),
            }),
        }
    }

    pub(in crate::llvm::codegen) fn create_mir_local_slots(
        &mut self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
    ) -> Result<Vec<MirLocalSlot<'ctx>>, LlvmEmitError> {
        body.locals
            .iter()
            .enumerate()
            .map(|(idx, local)| {
                let local_id = crate::mir::LocalId::from_raw(idx as u32);
                let cg_ty = self.mir_local_storage_cg_ty(body, mir_types, local_id, local)?;
                let ptr = self.create_entry_alloca(
                    local.span,
                    local.name.as_deref().unwrap_or("mir_local"),
                    cg_ty,
                )?;
                Ok(MirLocalSlot { cg_ty, ptr })
            })
            .collect()
    }
}
