//! LIR source callable lookup and closure body emission.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn has_lir_source_instances_for_template(
        &self,
        _fqn: &str,
    ) -> bool {
        false
    }

    pub(in crate::llvm::codegen) fn lir_source_callable(
        &self,
        fqn: &str,
    ) -> Option<(
        &'a TypeStore,
        &'a crate::effect_lowered::LateLoweredSourceCallable,
    )> {
        if self.has_lir_source_instances_for_template(fqn) {
            return None;
        }
        let program = self.published_late_lowered_program()?;
        let source_types = self.published_late_lowered_types()?;
        let callable = program.callable(fqn)?;
        let source_callable = callable.source_callable()?;
        callable.executable_body()?;
        Some((source_types, source_callable))
    }

    pub(in crate::llvm::codegen) fn lir_source_closure_body_symbol(
        &self,
        callable_fqn: &str,
        at: crate::span::Span,
    ) -> Result<String, LlvmEmitError> {
        Ok(private_closure_body_fn_name(
            &self.stable_closure_key_for_lir_source_callable(callable_fqn, at)?,
        ))
    }

    pub(in crate::llvm::codegen) fn ensure_lir_source_closure_callable_defined(
        &mut self,
        span: crate::span::Span,
        fn_ptr: &str,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let (source_types, source_fun) = self.lir_source_callable(fn_ptr).unwrap_or_else(|| {
            panic!(
                "ensure_lir_source_closure_callable_defined: missing LIR source closure callable"
            )
        });
        if !source_fun.name.starts_with("$lambda") {
            panic!("ensure_lir_source_closure_callable_defined: callable is not a closure body")
        }
        let body_symbol = self.lir_source_closure_body_symbol(fn_ptr, source_fun.span)?;
        if let Some(existing) = self.module.get_function(&body_symbol)
            && existing.count_basic_blocks() > 0
        {
            return Ok(existing);
        }

        let saved_block = self.expect_insert_block("LIR source closure body lookup");
        let mut child = self.fresh_child_codegen();
        child.current_source_id = child.lir_source_callable_source_id(fn_ptr, span)?;
        let llvm_fun = child.declare_lir_source_closure_fun(span, source_fun, source_types)?;
        if llvm_fun.count_basic_blocks() == 0 {
            child.codegen_lir_source_closure_fun(source_fun, source_types, llvm_fun)?;
        }
        self.builder.position_at_end(saved_block);
        Ok(llvm_fun)
    }

    pub(in crate::llvm::codegen) fn lir_source_callable_source_id(
        &self,
        fqn: &str,
        span: crate::span::Span,
    ) -> Result<SourceId, LlvmEmitError> {
        let mut owner_fqn = fqn;
        loop {
            if let Some(source) = self.callable_sources.get(owner_fqn) {
                return self.source_id_for_path(source.source_path.as_path(), span);
            }
            let Some((parent, _)) = owner_fqn.rsplit_once(".$lambda") else {
                break;
            };
            owner_fqn = parent;
        }
        panic!(
            "lir_source_callable_source_id: LIR source contract accepted callable without source path"
        )
    }

    pub(in crate::llvm::codegen) fn declare_lir_source_closure_fun(
        &mut self,
        span: crate::span::Span,
        source_fun: &crate::effect_lowered::LateLoweredSourceCallable,
        source_types: &TypeStore,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let param_tys = source_fun
            .params
            .iter()
            .map(|param| param.ty)
            .collect::<Vec<_>>();
        self.declare_lir_source_closure_fun_with_signature(
            span,
            source_fun,
            &param_tys,
            source_fun.return_ty,
            source_types,
        )
    }

    pub(in crate::llvm::codegen) fn declare_lir_source_closure_fun_with_signature(
        &mut self,
        span: crate::span::Span,
        source_fun: &crate::effect_lowered::LateLoweredSourceCallable,
        param_tys: &[TypeId],
        return_ty: TypeId,
        source_types: &TypeStore,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let body_symbol = self.lir_source_closure_body_symbol(source_fun.fqn.as_str(), span)?;
        if let Some(existing) = self.module.get_function(&body_symbol) {
            return Ok(existing);
        }
        if param_tys.len() != source_fun.params.len() {
            return Err(frontend_error(format!(
                "LIR source closure `{}` 的 plain ABI 参数数量({}) 与 source 参数数量({}) 不一致",
                source_fun.fqn,
                param_tys.len(),
                source_fun.params.len()
            )));
        }

        let ret_cg = self.cg_ty_of_mir_type(source_types, return_ty).unwrap_or_else(|| {
            panic!("declare_lir_source_closure_fun_with_signature: LIR source verifier accepted unsupported closure return type")
        });
        let hidden_sret_result_ty = self.hidden_sret_result_ty(span, ret_cg)?;
        // 这里发布的是 plain callable ABI 的 closure body symbol；effect-step callable surface
        // 由 stage-owned direct/dynamic entry shell 单独承载，不应再为 plain entry 混入 hidden ABI。
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = Vec::with_capacity(
            source_fun.params.len() + usize::from(hidden_sret_result_ty.is_some()),
        );
        if let Some(result_ty) = hidden_sret_result_ty {
            let _ = result_ty;
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        llvm_param_tys.push(self.llvm_gc_i8_ptr_type().into());
        for (param, param_ty) in source_fun
            .params
            .iter()
            .skip(1)
            .zip(param_tys.iter().skip(1))
        {
            let param_ty = self
                .equivalent_codegen_type_id(source_types, *param_ty)
                .unwrap_or_else(|| {
                    panic!("declare_lir_source_closure_fun_with_signature: TypeStore equivalence verifier accepted unsupported closure param type")
                });
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
                .llvm_basic_type_of(source_fun.span, other)?
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

    pub(in crate::llvm::codegen) fn declare_lir_source_plain_fun_with_symbol(
        &mut self,
        llvm_name: &str,
        surface: LlvmFunctionDeclarationSurface,
        source_fun: &crate::effect_lowered::LateLoweredSourceCallable,
        param_tys: &[TypeId],
        return_ty: TypeId,
        source_types: &TypeStore,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let llvm_name = match surface {
            LlvmFunctionDeclarationSurface::ExportedAbi => {
                self.exported_abi_symbol_for_lir_callable(&source_fun.fqn)?
            }
            LlvmFunctionDeclarationSurface::RuntimeOrNativeImport
            | LlvmFunctionDeclarationSurface::CompilerPrivateHelper => llvm_name.to_string(),
        };
        if let Some(existing) = self.module.get_function(&llvm_name) {
            return Ok(existing);
        }
        if param_tys.len() != source_fun.params.len() {
            return Err(frontend_error(format!(
                "plain LIR source callable `{}` 的 plain ABI 参数数量({}) 与 source 参数数量({}) 不一致",
                source_fun.fqn,
                param_tys.len(),
                source_fun.params.len()
            )));
        }

        let ret_cg = self.cg_ty_of_mir_type(source_types, return_ty).unwrap_or_else(|| {
            panic!("declare_lir_source_plain_fun_with_symbol: LIR source ABI verifier accepted unsupported plain return type")
        });
        let hidden_sret_result_ty = self.hidden_sret_result_ty(source_fun.span, ret_cg)?;
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = Vec::with_capacity(
            source_fun.params.len() + usize::from(hidden_sret_result_ty.is_some()),
        );
        if let Some(result_ty) = hidden_sret_result_ty {
            let _ = result_ty;
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        for (param, param_ty) in source_fun.params.iter().zip(param_tys.iter().copied()) {
            let param_ty = self.equivalent_codegen_type_id(source_types, param_ty).unwrap_or_else(|| {
                tracing::warn!(
                    "declare_lir_source_plain_fun_with_symbol: unsupported param type for {} param {} -> {}",
                    source_fun.fqn,
                    param.name,
                    source_types.display(param_ty)
                );
                panic!("declare_lir_source_plain_fun_with_symbol: TypeStore equivalence verifier accepted unsupported plain param type")
            });
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
                .llvm_basic_type_of(source_fun.span, other)?
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

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn declare_lir_plain_fun_with_symbol(
        &mut self,
        llvm_name: &str,
        surface: LlvmFunctionDeclarationSurface,
        owner_fqn: &str,
        param_tys: &[TypeId],
        return_ty: TypeId,
        source_types: &TypeStore,
        closure_like: bool,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let span = self
            .callable_sources
            .get(owner_fqn)
            .map(|source| source.span)
            .unwrap_or_else(|| crate::span::Span::new(0, 0));
        let llvm_name = match surface {
            LlvmFunctionDeclarationSurface::ExportedAbi => {
                if owner_fqn == "main" {
                    "main".to_string()
                } else if llvm_name != owner_fqn {
                    llvm_name.to_string()
                } else {
                    self.exported_abi_symbol_for_lir_callable(owner_fqn)?
                }
            }
            LlvmFunctionDeclarationSurface::RuntimeOrNativeImport
            | LlvmFunctionDeclarationSurface::CompilerPrivateHelper => llvm_name.to_string(),
        };
        if let Some(existing) = self.module.get_function(&llvm_name) {
            return Ok(existing);
        }

        let callable_abi = self.direct_call_abi_identity(owner_fqn);
        let codegen_param_tys = param_tys
            .iter()
            .copied()
            .map(|param_ty| {
                self.equivalent_codegen_type_id(source_types, param_ty)
                    .unwrap_or_else(|| {
                        panic!("declare_lir_plain_fun_with_symbol: TypeStore equivalence verifier accepted unsupported plain param type")
                    })
            })
            .collect::<Vec<_>>();
        let codegen_return_ty = self
            .equivalent_codegen_type_id(source_types, return_ty)
            .unwrap_or(return_ty);
        let native_abi = if callable_abi.uses_native_abi() {
            Some(self.classify_direct_extern_native_callable(
                span,
                owner_fqn,
                &codegen_param_tys,
                codegen_return_ty,
            )?)
        } else {
            None
        };
        let ret_cg = native_abi
            .as_ref()
            .map(|abi| abi.return_abi.cg_ty)
            .or_else(|| self.cg_ty_of_mir_type(source_types, return_ty))
            .unwrap_or_else(|| {
                panic!("declare_lir_plain_fun_with_symbol: LIR facts verifier accepted unsupported plain return type")
            });
        let hidden_sret_result_ty = if native_abi.is_some() {
            None
        } else {
            self.hidden_sret_result_ty(span, ret_cg)?
        };
        let uses_explicit_effect_hidden_abi =
            native_abi.is_none() && !closure_like && callable_abi.uses_effect_bridge_abi();
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = Vec::with_capacity(
            param_tys.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + usize::from(closure_like)
                + self.explicit_effect_hidden_abi_param_count(uses_explicit_effect_hidden_abi)
                    as usize,
        );
        if let Some(result_ty) = hidden_sret_result_ty {
            let _ = result_ty;
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        if uses_explicit_effect_hidden_abi {
            self.push_explicit_effect_hidden_abi_param_tys(&mut llvm_param_tys);
        }
        if let Some(native_abi) = native_abi.as_ref() {
            llvm_param_tys.extend(native_abi.param_abis.iter().map(|abi| abi.llvm_param_ty));
        } else {
            let lowered_param_tys = if closure_like {
                llvm_param_tys.push(self.llvm_gc_i8_ptr_type().into());
                codegen_param_tys.into_iter().skip(1).collect::<Vec<_>>()
            } else {
                codegen_param_tys
            };
            for param_ty in lowered_param_tys {
                llvm_param_tys.push(self.ordinary_param_abi(span, param_ty)?.llvm_param_ty());
            }
        }

        let fn_ty = match (hidden_sret_result_ty, ret_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_param_tys, false)
            }
            (None, other) => self
                .llvm_basic_type_of(span, other)?
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
        llvm_fun.set_call_conventions(
            native_abi
                .as_ref()
                .map(|abi| abi.call_convention)
                .unwrap_or(0),
        );
        if let Some(result_ty) = hidden_sret_result_ty {
            self.add_sret_attribute_to_function(llvm_fun, 0, result_ty);
        }
        if native_abi.is_some() {
            self.mark_gc_leaf_function(llvm_fun);
        }
        Ok(llvm_fun)
    }

    pub(in crate::llvm::codegen) fn codegen_lir_source_closure_fun(
        mut self,
        source_fun: &crate::effect_lowered::LateLoweredSourceCallable,
        source_types: &TypeStore,
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let executable_body = self
            .published_late_lowered_program()
            .and_then(|program| program.callable(&source_fun.fqn))
            .and_then(|callable| callable.executable_body())
            .unwrap_or_else(|| {
                panic!("codegen_lir_source_closure_fun: missing LIR executable closure body")
            });
        self.function_cx.current_callable_fqn = Some(source_fun.fqn.clone());

        let declared_return_cg = self
            .cg_ty_of_mir_type(source_types, source_fun.return_ty)
            .unwrap_or_else(|| {
                panic!("codegen_lir_source_closure_fun: LIR source verifier accepted unsupported closure return type")
            });
        let entry = self.context.append_basic_block(llvm_fun, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(llvm_fun)?;
        self.function_cx.current_fun_return_ty = Some(declared_return_cg);
        let uses_hidden_sret = self
            .hidden_sret_result_ty(source_fun.span, declared_return_cg)?
            .is_some();
        self.function_cx.current_sret_return_ptr = if uses_hidden_sret {
            Some(
                llvm_fun
                    .get_nth_param(0)
                    .unwrap_or_else(|| {
                        panic!("codegen_lir_source_closure_fun: LIR source ABI verifier accepted missing sret param")
                    })
                    .into_pointer_value(),
            )
        } else {
            None
        };
        self.clear_explicit_effect_hidden_abi_slots();

        let (return_bb, return_alloca) =
            self.setup_function_return_context(source_fun.span, llvm_fun, declared_return_cg)?;
        let mut local_slots = self.create_lir_local_slots(executable_body, source_types)?;
        self.bind_lir_closure_params(
            executable_body.header(),
            source_types,
            llvm_fun,
            u32::from(uses_hidden_sret),
            &mut local_slots,
        )?;
        let used_locals = collect_lir_local_uses(executable_body, source_types);
        let llvm_blocks = executable_body
            .states()
            .states()
            .iter()
            .map(|state| {
                (
                    state.state_id(),
                    self.context.append_basic_block(
                        llvm_fun,
                        &format!("lir.s{}", state.state_id().as_u32()),
                    ),
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        let start_bb = llvm_blocks
            .get(&executable_body.states().entry_state())
            .copied()
            .unwrap_or_else(|| {
                panic!("codegen_lir_source_closure_fun: LIR executable body accepted missing entry state")
            });
        self.builder.build_unconditional_branch(start_bb)?;

        for state in executable_body.states().states() {
            let llvm_block = llvm_blocks.get(&state.state_id()).copied().unwrap_or_else(|| {
                panic!(
                    "codegen_lir_source_closure_fun: LIR executable body accepted missing block for state s{}",
                    state.state_id().as_u32()
                )
            });
            self.builder.position_at_end(llvm_block);
            for stmt in state.body().statements() {
                self.codegen_lir_statement(
                    stmt,
                    executable_body,
                    source_types,
                    &local_slots,
                    &used_locals,
                    None,
                )?;
            }
            self.codegen_lir_plain_terminator(
                state.body().terminator(),
                &local_slots,
                &llvm_blocks,
                declared_return_cg,
            )?;
        }

        self.emit_function_return_block(
            source_fun.span,
            declared_return_cg,
            return_bb,
            return_alloca,
        )?;
        self.finish_function_explicit_frame_layout(source_fun.span)?;
        self.clear_explicit_effect_hidden_abi_slots();
        self.function_cx.current_sret_return_ptr = None;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn bind_lir_header_params(
        &mut self,
        header: &crate::effect_lowered::LirCallableHeader,
        source_types: &TypeStore,
        llvm_fun: FunctionValue<'ctx>,
        param_offset: u32,
        slots: &mut [MirLocalSlot<'ctx>],
    ) -> Result<(), LlvmEmitError> {
        for (idx, param) in header.params().iter().enumerate() {
            let slot = slots.get(param.local().as_u32() as usize).copied().unwrap_or_else(|| {
                std::panic::panic_any(
                    "bind_lir_header_params: LIR verifier accepted param local outside slot table",
                )
            });
            let abi_ty = self
                .equivalent_codegen_type_id(source_types, param.ty())
                .unwrap_or_else(|| {
                    panic!("bind_lir_header_params: LIR verifier accepted unsupported param type")
                });
            let abi = self.ordinary_param_abi(param.span(), abi_ty)?;
            let init = if let Some(pointee_ty) = abi.pointee_ty() {
                let param_ptr = llvm_fun
                    .get_nth_param(idx as u32 + param_offset)
                    .unwrap_or_else(|| {
                        std::panic::panic_any(
                            "bind_lir_header_params: ABI declaration missing lowered LLVM parameter",
                        )
                    })
                    .into_pointer_value();
                let loaded =
                    self.builder
                        .build_load(pointee_ty, param_ptr, "lir_header_param_load")?;
                self.cg_value_from_loaded(param.span(), slot.cg_ty, loaded)?
            } else {
                self.cg_value_from_llvm_param(
                    param.span(),
                    llvm_fun,
                    idx as u32 + param_offset,
                    slot.cg_ty,
                    "missing LIR header llvm param",
                )?
            };
            let _ = self.store_local_value(param.span(), slot.ptr, slot.cg_ty, init)?;
        }
        Ok(())
    }

    pub(in crate::llvm::codegen) fn bind_lir_closure_params(
        &mut self,
        header: &crate::effect_lowered::LirCallableHeader,
        source_types: &TypeStore,
        llvm_fun: FunctionValue<'ctx>,
        param_offset: u32,
        slots: &mut [MirLocalSlot<'ctx>],
    ) -> Result<(), LlvmEmitError> {
        let env_param = header.params().first().unwrap_or_else(|| {
            panic!("bind_lir_closure_params: LIR verifier accepted closure without env param")
        });
        if env_param.name() != "$env" {
            panic!(
                "bind_lir_closure_params: LIR verifier accepted closure first param not named `$env`"
            )
        }
        let env_slot = slots
            .get(env_param.local().as_u32() as usize)
            .copied()
            .unwrap_or_else(|| {
                panic!("bind_lir_closure_params: LIR verifier accepted missing env local slot")
            });
        let env_init = self.codegen_mir_closure_env_param(
            env_param.span(),
            header.root_fqn(),
            llvm_fun,
            param_offset,
            env_slot.cg_ty,
        )?;
        let _ = self.store_local_value(env_param.span(), env_slot.ptr, env_slot.cg_ty, env_init)?;

        for (idx, param) in header.params().iter().enumerate().skip(1) {
            let slot = slots
                .get(param.local().as_u32() as usize)
                .copied()
                .unwrap_or_else(|| {
                    panic!("bind_lir_closure_params: LIR call ABI verifier accepted missing param local slot")
                });
            let param_ty = self.equivalent_codegen_type_id(source_types, param.ty()).unwrap_or_else(|| {
                panic!("bind_lir_closure_params: TypeStore equivalence verifier accepted unsupported param type")
            });
            let abi = self.ordinary_param_abi(param.span(), param_ty)?;
            let init = if let Some(pointee_ty) = abi.pointee_ty() {
                let param_ptr = llvm_fun
                    .get_nth_param(idx as u32 + param_offset)
                    .unwrap_or_else(|| {
                        panic!("bind_lir_closure_params: LIR call ABI verifier accepted missing LLVM param")
                    })
                    .into_pointer_value();
                let loaded =
                    self.builder
                        .build_load(pointee_ty, param_ptr, "lir_closure_param_load")?;
                self.cg_value_from_loaded(param.span(), slot.cg_ty, loaded)?
            } else {
                self.cg_value_from_llvm_param(
                    param.span(),
                    llvm_fun,
                    idx as u32 + param_offset,
                    slot.cg_ty,
                    "missing LIR closure llvm param",
                )?
            };
            let _ = self.store_local_value(param.span(), slot.ptr, slot.cg_ty, init)?;
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
                let capture_field_cgs = self
                    .mir_closure_env_capture_element_cg_tys(env_cg)
                    .unwrap_or_else(|| {
                        panic!("codegen_mir_closure_env_param: MIR verifier accepted non-tuple closure env")
                    });
                let env_arg = llvm_fun
                    .get_nth_param(param_index)
                    .unwrap_or_else(|| {
                        panic!(
                            "codegen_mir_closure_env_param: declared closure ABI without env param"
                        )
                    })
                    .into_pointer_value();
                let closure_key = self.stable_closure_key_for_lir_source_callable(fn_ptr, span)?;
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
            _ => panic!(
                "codegen_mir_closure_env_param: MIR verifier accepted unsupported closure env type"
            ),
        }
    }

    pub(in crate::llvm::codegen) fn create_mir_local_slots(
        &mut self,
        body: &mir_source::Body,
        mir_types: &TypeStore,
    ) -> Result<Vec<MirLocalSlot<'ctx>>, LlvmEmitError> {
        body.locals
            .iter()
            .enumerate()
            .map(|(idx, local)| {
                let local_id = mir_source::LocalId::from_raw(idx as u32);
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

    pub(in crate::llvm::codegen) fn create_lir_local_slots(
        &mut self,
        body: &LirExecutableBody,
        source_types: &TypeStore,
    ) -> Result<Vec<MirLocalSlot<'ctx>>, LlvmEmitError> {
        body.locals()
            .iter()
            .map(|local| {
                let cg_ty =
                    self.lir_local_storage_cg_ty(body, source_types, local.local(), local)?;
                let ptr = self.create_entry_alloca(
                    local.span(),
                    local.name().unwrap_or("lir_local"),
                    cg_ty,
                )?;
                Ok(MirLocalSlot { cg_ty, ptr })
            })
            .collect()
    }
}
