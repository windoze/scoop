//! Closure expression/env/body lowering split out of `codegen/mod.rs`.

use super::*;
/// Closure lowering 后，codegen 需要分别知道：
/// - receiver lambda 的隐式 `this` 绑定（来自 LLVM receiver 参数，而非 capture env）；
/// - 普通显式参数 / 隐式 `it` 绑定；
/// - 剩余真正需要进 env 的 captures。
struct ClosureParamBindings {
    receiver: Option<(hir::SymbolId, String, TypeId)>,
    params: Vec<(hir::SymbolId, String, TypeId)>,
    captures: Vec<hir::Capture>,
}

#[derive(Clone, Copy)]
struct ClosureBodyCodegenSpec<'ctx, 'spec> {
    receiver_binding: Option<&'spec (hir::SymbolId, String, TypeId)>,
    param_bindings: &'spec [(hir::SymbolId, String, TypeId)],
    capture_bindings: &'spec [(hir::SymbolId, String, TypeId)],
    llvm_fun: FunctionValue<'ctx>,
    callee_suspend_plan: Option<&'spec CalleeSuspendPlan>,
    callee_resume_entry_fn: Option<FunctionValue<'ctx>>,
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn declare_closure_callee_resume_entry(
        &mut self,
        at: crate::span::Span,
        closure: &hir::ClosureExpr,
        return_cg: CgTy,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        self.declare_callee_resume_entry_function(
            at,
            &closure_callee_resume_entry_fn_name(closure.id),
            return_cg,
        )
    }

    fn build_closure_callee_suspend_plan(
        &self,
        closure: &hir::ClosureExpr,
        _return_ty: TypeId,
    ) -> Option<CalleeSuspendPlan> {
        let callable_fqn = format!("scoop.lambda${}", closure.id.as_u32());
        self.callable_needs_callee_resume_shell(&callable_fqn)
            .then_some(CalleeSuspendPlan {
                saved_locals: Vec::new(),
                resume_sites: Vec::new(),
            })
    }

    pub(in crate::llvm::codegen) fn codegen_closure_expr(
        &mut self,
        span: crate::span::Span,
        closure: &hir::ClosureExpr,
        expected_fun_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(expected_fun_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "lambda without expected function type",
                at: span.into(),
            });
        };

        // 1) 确定参数绑定（显式 params 或 Kotlin-like 隐式 `it`）。
        let ClosureParamBindings {
            receiver: receiver_binding,
            params: param_bindings,
            captures,
        } = self.closure_param_bindings(span, closure, fun_ty)?;
        if captures.iter().any(|c| c.mutable) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "mutable capture (not supported yet)",
                at: span.into(),
            });
        }

        let fun_name = format!("scoop.lambda${}", closure.id.as_u32());

        // 2) 确保 closure 函数本体存在（module-level function）。
        //
        // 注意：我们会在“第一次 codegen 到该 lambda 表达式”时生成其函数体；之后复用同名符号。
        let saved_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;

        let llvm_fun = if let Some(existing) = self.module.get_function(&fun_name) {
            existing
        } else {
            let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
            let ret_cg =
                self.cg_ty_of(fun_ty.return_ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "lambda return type",
                        at: span.into(),
                    })?;
            let callee_suspend_plan =
                self.build_closure_callee_suspend_plan(closure, fun_ty.return_ty);
            let callee_resume_entry_fn = if callee_suspend_plan.is_some() {
                Some(self.declare_closure_callee_resume_entry(span, closure, ret_cg)?)
            } else {
                None
            };
            let hidden_sret_result_ty = self.hidden_sret_result_ty(span, ret_cg)?;
            let uses_explicit_effect_hidden_abi =
                self.callable_uses_explicit_effect_hidden_abi(&fun_name);
            let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = Vec::with_capacity(
                1 + fun_ty.params.len()
                    + usize::from(fun_ty.receiver.is_some())
                    + usize::from(hidden_sret_result_ty.is_some())
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
            // env ptr：GC-managed 引用（closure env 是一个 heap object）。
            llvm_param_tys.push(gc_i8_ptr_ty.into());
            if let Some(receiver_ty) = fun_ty.receiver {
                llvm_param_tys.push(self.ordinary_param_abi(span, receiver_ty)?.llvm_param_ty());
            }
            for ty in &fun_ty.params {
                llvm_param_tys.push(self.ordinary_param_abi(span, *ty)?.llvm_param_ty());
            }

            let fn_ty = match (hidden_sret_result_ty, ret_cg) {
                (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                    self.context.void_type().fn_type(&llvm_param_tys, false)
                }
                (None, CgTy::Bool) => self.context.bool_type().fn_type(&llvm_param_tys, false),
                (None, CgTy::Float64) => self.context.f64_type().fn_type(&llvm_param_tys, false),
                (None, CgTy::Float32) => self.context.f32_type().fn_type(&llvm_param_tys, false),
                (None, CgTy::Int(int_ty)) => self.int_type(int_ty).fn_type(&llvm_param_tys, false),
                (None, CgTy::String) => self
                    .llvm_scoop_string_ptr_type()
                    .fn_type(&llvm_param_tys, false),
                (None, CgTy::Ref) => gc_i8_ptr_ty.fn_type(&llvm_param_tys, false),
                (None, CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_)) => unreachable!(
                    "aggregate lambda returns should have been lowered through hidden sret"
                ),
            };

            let llvm_fun = self.module.add_function(&fun_name, fn_ty, None);
            llvm_fun.set_call_conventions(0);
            if let Some(result_ty) = hidden_sret_result_ty {
                self.add_sret_attribute_to_function(llvm_fun, 0, result_ty);
            }

            let mut cg = self.fresh_child_codegen();
            // 说明：closure 捕获信息里没有类型；这里在外层 codegen 阶段用 env 中的 locals 恢复 type id，
            // 再传给 closure fun body 用于 env layout 与绑定。
            let mut capture_bindings: Vec<(hir::SymbolId, String, TypeId)> =
                Vec::with_capacity(captures.len());
            for cap in &captures {
                let Some(local) = self.function_cx.env.get(cap.id) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "capture local not found",
                        at: cap.decl_span.into(),
                    });
                };
                let Some(ty_id) = local.hir_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "capture local type",
                        at: cap.decl_span.into(),
                    });
                };
                capture_bindings.push((cap.id, cap.name.clone(), ty_id));
            }

            cg.codegen_closure_fun_body(
                closure,
                fun_ty,
                ClosureBodyCodegenSpec {
                    receiver_binding: receiver_binding.as_ref(),
                    param_bindings: &param_bindings,
                    capture_bindings: &capture_bindings,
                    llvm_fun,
                    callee_suspend_plan: callee_suspend_plan.as_ref(),
                    callee_resume_entry_fn,
                },
            )?;

            // 恢复外层插入点（closure 函数 codegen 会移动 builder 的 position）。
            self.builder.position_at_end(saved_block);
            llvm_fun
        };

        // 3) 创建 closure object：`{ header, env_ptr, fn_ptr=&lambda }`
        let closure_obj_ty = self.llvm_closure_object_type();
        let obj_size_bytes = self.target_data.get_store_size(&closure_obj_ty);

        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);

        let closure_desc = self.get_or_create_closure_object_type_desc_global(span)?;
        let closure_desc_i8 = self.builder.build_pointer_cast(
            closure_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "closure_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_alloc,
            &[closure_desc_i8.into(), size_v.into()],
            "rt_alloc_closure",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(obj_i8) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return type",
                at: span.into(),
            });
        };

        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let obj_ptr = self
            .builder
            .build_pointer_cast(obj_i8, obj_ptr_ty, "closure_obj_ptr")?;
        let deferred_obj = self.defer_gc_ref_pointer(span, "closure_obj_root", obj_ptr)?;

        let obj_ptr =
            self.reload_deferred_gc_ref_without_clearing(span, "closure_obj_init", &deferred_obj)?;

        let env_gep =
            self.builder
                .build_struct_gep(closure_obj_ty, obj_ptr, 1, "closure_env_gep")?;
        // 重要：先把 env_ptr 初始化为 NULL。
        //
        // 说明：
        // - closure object 的 type descriptor 会把 `env_ptr` 视为 GC pointer slot；
        // - 若在分配 env 期间发生 safepoint/GC，则必须避免扫描到未初始化的垃圾值。
        let _ = self.store_local_value(
            span,
            env_gep,
            CgTy::Ref,
            CgValue {
                ty: CgTy::Ref,
                value: Some(gc_i8_ptr_ty.const_null().into()),
            },
        )?;

        // 若有捕获，则分配 env 并写入捕获值；否则 env_ptr 为 NULL。
        let env_i8 = if captures.is_empty() {
            gc_i8_ptr_ty.const_null()
        } else {
            let mut capture_bindings: Vec<(hir::SymbolId, String, TypeId)> =
                Vec::with_capacity(captures.len());
            for cap in &captures {
                let Some(local) = self.function_cx.env.get(cap.id) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "capture local not found",
                        at: cap.decl_span.into(),
                    });
                };
                let Some(ty_id) = local.hir_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "capture local type",
                        at: cap.decl_span.into(),
                    });
                };
                capture_bindings.push((cap.id, cap.name.clone(), ty_id));
            }

            let env_ty = self.llvm_closure_env_type(span, closure.id, &capture_bindings)?;
            let env_size_bytes = self.target_data.get_store_size(&env_ty);

            let size_v = self.context.i64_type().const_int(env_size_bytes, false);

            let env_desc =
                self.get_or_create_closure_env_type_desc_global(span, closure.id, env_ty)?;
            let env_desc_i8 = self.builder.build_pointer_cast(
                env_desc.as_pointer_value(),
                self.llvm_i8_ptr_type(),
                "closure_env_type_desc_i8",
            )?;
            let rt_alloc = self.declare_runtime_alloc_typed();
            let call = self.build_call_preserving_gc_local_roots(
                span,
                rt_alloc,
                &[env_desc_i8.into(), size_v.into()],
                "rt_alloc_closure_env",
            )?;
            let raw =
                call.try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "scoop_alloc_typed return value",
                        at: span.into(),
                    })?;
            let BasicValueEnum::PointerValue(env_i8) = raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "scoop_alloc_typed return type",
                    at: span.into(),
                });
            };

            let env_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
            let env_ptr = self
                .builder
                .build_pointer_cast(env_i8, env_ptr_ty, "closure_env_ptr")?;
            let deferred_env = self.defer_gc_ref_pointer(span, "closure_env_root", env_ptr)?;

            for (idx, (id, name, ty_id)) in capture_bindings.iter().enumerate() {
                let Some(local) = self.function_cx.env.get(*id) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "capture local not found",
                        at: span.into(),
                    });
                };

                let cg_ty = self
                    .cg_ty_of(*ty_id)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "capture local type",
                        at: span.into(),
                    })?;
                if !matches!(
                    cg_ty,
                    CgTy::Unit | CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref
                ) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "capture local (non-scalar)",
                        at: span.into(),
                    });
                }

                let llvm_ty = self.llvm_basic_type_of(span, cg_ty)?;
                let local_ptr =
                    self.local_ptr_for_use(span, local, &format!("capture_slot_{name}"))?;
                let loaded =
                    self.builder
                        .build_load(llvm_ty, local_ptr, &format!("capture_load_{name}"))?;

                let env_ptr = self.reload_deferred_gc_ref_without_clearing(
                    span,
                    &format!("closure_env_reload_{name}"),
                    &deferred_env,
                )?;
                let field_gep = self.builder.build_struct_gep(
                    env_ty,
                    env_ptr,
                    (idx + 1) as u32,
                    &format!("capture_gep_{name}"),
                )?;
                let v = if cg_ty == CgTy::Unit {
                    CgValue::unit()
                } else {
                    CgValue {
                        ty: cg_ty,
                        value: Some(loaded),
                    }
                };
                let _ = self.store_local_value(span, field_gep, cg_ty, v)?;
            }

            env_i8
        };
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "closure_obj_store_env",
            &deferred_obj,
        )?;
        let env_gep =
            self.builder
                .build_struct_gep(closure_obj_ty, obj_ptr, 1, "closure_env_gep")?;
        let _ = self.store_local_value(
            span,
            env_gep,
            CgTy::Ref,
            CgValue {
                ty: CgTy::Ref,
                value: Some(env_i8.into()),
            },
        )?;

        let fn_ptr = self.callable_carrier_target_fn_ptr(
            CallableCarrierKind::ClosureObject,
            &fun_name,
            llvm_fun.as_global_value().as_pointer_value(),
        )?;
        let fn_i8 = self
            .builder
            .build_pointer_cast(fn_ptr, i8_ptr_ty, "closure_fn_i8")?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "closure_obj_store_fn",
            &deferred_obj,
        )?;
        let fn_gep = self
            .builder
            .build_struct_gep(closure_obj_ty, obj_ptr, 2, "closure_fn_gep")?;
        let _ = self.builder.build_store(fn_gep, fn_i8)?;

        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "closure_obj_return",
            &deferred_obj,
        )?;
        let obj_i8 = self
            .builder
            .build_pointer_cast(obj_ptr, gc_i8_ptr_ty, "closure_obj_i8")?;

        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(obj_i8.into()),
        })
    }

    fn closure_param_bindings(
        &self,
        at: crate::span::Span,
        closure: &hir::ClosureExpr,
        fun_ty: &crate::ty::FunctionType,
    ) -> Result<ClosureParamBindings, LlvmEmitError> {
        let mut captures = closure.captures.clone();
        let mut explicit_params = closure.params.clone();
        let receiver = if let Some(receiver_ty) = fun_ty.receiver {
            if let Some(receiver_idx) = explicit_params.iter().position(|p| p.name == "this") {
                let receiver_param = explicit_params.remove(receiver_idx);
                Some((receiver_param.id, "this".to_string(), receiver_ty))
            } else {
                let Some(receiver_idx) = captures.iter().position(|c| c.name == "this") else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "receiver lambda missing this binder",
                        at: at.into(),
                    });
                };
                let receiver_capture = captures.remove(receiver_idx);
                Some((receiver_capture.id, "this".to_string(), receiver_ty))
            }
        } else {
            None
        };

        // 显式 params：`{ x -> ... }`
        //
        // 说明：
        // - receiver function type 的 receiver 不出现在 lambda params 列表里；
        // - 因此这里始终只按“非 receiver 形参”做显式 params / 隐式 `it` 绑定。
        if explicit_params.len() == fun_ty.params.len() {
            let params = explicit_params
                .iter()
                .zip(fun_ty.params.iter())
                .map(|(p, ty)| (p.id, p.name.clone(), *ty))
                .collect::<Vec<_>>();
            return Ok(ClosureParamBindings {
                receiver,
                params,
                captures,
            });
        }

        // 隐式 `it`：`{ body }` + expected `(T) -> R`
        if explicit_params.is_empty() && fun_ty.params.len() == 1 {
            let Some(it_idx) = captures.iter().position(|c| c.name == "it") else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "implicit it lambda missing it binder",
                    at: at.into(),
                });
            };
            let it_cap = captures.remove(it_idx);

            let params = vec![(it_cap.id, "it".to_string(), fun_ty.params[0])];
            return Ok(ClosureParamBindings {
                receiver,
                params,
                captures,
            });
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "lambda param arity mismatch",
            at: at.into(),
        })
    }

    fn codegen_closure_fun_body(
        &mut self,
        closure: &hir::ClosureExpr,
        fun_ty: &crate::ty::FunctionType,
        spec: ClosureBodyCodegenSpec<'ctx, '_>,
    ) -> Result<(), LlvmEmitError> {
        let entry = self.context.append_basic_block(spec.llvm_fun, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(spec.llvm_fun)?;

        self.function_cx.env.push_scope();

        // 入口的返回类型由期望函数类型决定（用于 Raise 的“早退默认值”）。
        let declared_return_cg =
            self.cg_ty_of(fun_ty.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "lambda return type",
                    at: closure.span.into(),
                })?;
        self.function_cx.current_fun_return_ty = Some(declared_return_cg);
        let uses_hidden_sret = self
            .hidden_sret_result_ty(closure.span, declared_return_cg)?
            .is_some();
        let callable_fqn = format!("scoop.lambda${}", closure.id.as_u32());
        let uses_explicit_effect_hidden_abi =
            self.callable_uses_explicit_effect_hidden_abi(&callable_fqn);
        self.function_cx.current_sret_return_ptr = if uses_hidden_sret {
            Some(
                spec.llvm_fun
                    .get_nth_param(0)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "missing llvm lambda sret param",
                        at: closure.span.into(),
                    })?
                    .into_pointer_value(),
            )
        } else {
            None
        };
        self.bind_explicit_effect_hidden_abi_slots(
            closure.span,
            spec.llvm_fun,
            u32::from(uses_hidden_sret),
            uses_explicit_effect_hidden_abi,
        )?;
        let env_param_index = u32::from(uses_hidden_sret)
            + self.explicit_effect_hidden_abi_param_count(uses_explicit_effect_hidden_abi);
        let (return_bb, return_alloca) =
            self.setup_function_return_context(closure.span, spec.llvm_fun, declared_return_cg)?;

        // captures：从 env（第 0 个 LLVM param）读取并绑定为 locals。
        if !spec.capture_bindings.is_empty() {
            let env_i8 = spec
                .llvm_fun
                .get_nth_param(env_param_index)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "missing llvm lambda env param",
                    at: closure.span.into(),
                })?
                .into_pointer_value();

            let env_ty =
                self.llvm_closure_env_type(closure.span, closure.id, spec.capture_bindings)?;
            let env_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
            let env_ptr = self
                .builder
                .build_pointer_cast(env_i8, env_ptr_ty, "closure_env_ptr")?;

            for (idx, (id, name, ty_id)) in spec.capture_bindings.iter().enumerate() {
                let target_ty =
                    self.cg_ty_of(*ty_id)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "capture type",
                            at: closure.span.into(),
                        })?;
                if !matches!(
                    target_ty,
                    CgTy::Unit | CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref
                ) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "capture local (non-scalar)",
                        at: closure.span.into(),
                    });
                }

                let llvm_ty = self.llvm_basic_type_of(closure.span, target_ty)?;
                let field_gep = self.builder.build_struct_gep(
                    env_ty,
                    env_ptr,
                    (idx + 1) as u32,
                    &format!("capture_gep_{name}"),
                )?;
                let loaded =
                    self.builder
                        .build_load(llvm_ty, field_gep, &format!("capture_{name}"))?;

                let ptr = self.create_entry_alloca(closure.span, name, target_ty)?;
                let init = CgValue {
                    ty: target_ty,
                    value: Some(loaded),
                };
                let _stored = self.store_local_value(closure.span, ptr, target_ty, init)?;

                self.function_cx.env.insert(
                    *id,
                    CgLocal {
                        hir_ty: Some(*ty_id),
                        call_may_suspend: self
                            .function_cx
                            .env
                            .get(*id)
                            .map(|local| local.call_may_suspend)
                            .unwrap_or_else(|| {
                                self.local_call_may_suspend_from_hir_ty(Some(*ty_id))
                            }),
                        ty: target_ty,
                        ptr,
                        frame_backing_ptr: None,
                        mutable: false,
                    },
                );
            }
        }

        if let Some((id, name, ty_id)) = spec.receiver_binding {
            self.bind_ordinary_param_local(OrdinaryParamLocalBinding {
                at: closure.span,
                llvm_fun: spec.llvm_fun,
                param_index: env_param_index + 1,
                name,
                id: *id,
                ty_id: *ty_id,
                call_may_suspend: self.local_call_may_suspend_from_hir_ty(Some(*ty_id)),
                missing_kind: "missing llvm lambda receiver",
            })?;
        }

        // params：env 固定占用第 0 个 LLVM param；若函数类型带 receiver，则第 1 个 LLVM param
        // 为 receiver，用户显式声明的 params 从其后开始。
        let llvm_param_offset = env_param_index + 1 + u32::from(fun_ty.receiver.is_some());
        for (idx, (id, name, ty_id)) in spec.param_bindings.iter().enumerate() {
            self.bind_ordinary_param_local(OrdinaryParamLocalBinding {
                at: closure.span,
                llvm_fun: spec.llvm_fun,
                param_index: idx as u32 + llvm_param_offset,
                name,
                id: *id,
                ty_id: *ty_id,
                call_may_suspend: self.local_call_may_suspend_from_hir_ty(Some(*ty_id)),
                missing_kind: "missing llvm lambda param",
            })?;
        }

        let body_expr = closure.body.as_ref();
        let ret_v = self.with_callee_suspend_lowering(
            spec.callee_suspend_plan.cloned(),
            spec.callee_resume_entry_fn,
            |cg| match &body_expr.kind {
                hir::ExprKind::Block(block) => {
                    cg.codegen_block_as_return_value(block, declared_return_cg)
                }
                _ => {
                    let v =
                        cg.codegen_expr_in_expected_context(body_expr, Some(declared_return_cg))?;
                    if declared_return_cg == CgTy::Unit {
                        Ok(CgValue::unit())
                    } else {
                        cg.coerce_value(body_expr.span, v, declared_return_cg)
                    }
                }
            },
        )?;
        self.finish_function_return_path(closure.span, declared_return_cg, ret_v)?;

        self.emit_function_return_block(
            closure.span,
            declared_return_cg,
            return_bb,
            return_alloca,
        )?;
        self.finish_function_explicit_frame_layout(closure.span)?;
        if let (Some(plan), Some(resume_fun)) =
            (spec.callee_suspend_plan, spec.callee_resume_entry_fn)
        {
            self.codegen_callee_resume_entry_function(
                closure.span,
                resume_fun,
                plan,
                declared_return_cg,
            )?;
        }
        self.clear_explicit_effect_hidden_abi_slots();
        self.function_cx.current_sret_return_ptr = None;
        self.function_cx.env.pop_scope();
        Ok(())
    }

    fn llvm_closure_env_type(
        &mut self,
        at: crate::span::Span,
        closure_id: hir::ClosureId,
        capture_bindings: &[(hir::SymbolId, String, TypeId)],
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let name = format!("scoop.lambda_env${}", closure_id.as_u32());
        if let Some(existing) = self.context.get_struct_type(&name) {
            return Ok(existing);
        }

        let env_ty = self.context.opaque_struct_type(&name);
        // closure env 是 GC-managed heap object：以对象头开头，再跟 capture 字段。
        let header_ty = self.llvm_gc_object_header_type();
        let mut fields: Vec<BasicTypeEnum<'ctx>> = Vec::with_capacity(1 + capture_bindings.len());
        fields.push(header_ty.into());
        for (_id, _name, ty_id) in capture_bindings {
            let cg_ty = self
                .cg_ty_of(*ty_id)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "capture type",
                    at: at.into(),
                })?;
            if !matches!(
                cg_ty,
                CgTy::Unit | CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref
            ) {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "capture local (non-scalar)",
                    at: at.into(),
                });
            }
            fields.push(self.llvm_basic_type_of(at, cg_ty)?);
        }
        env_ty.set_body(&fields, false);
        Ok(env_ty)
    }

    /// 在当前 compilation unit 的 `TypeStore` 中查找 `() -> Unit / Pure` 的函数类型。
    ///
    /// 用途：
    /// - 一些 sysroot API（例如 `scoop.sync.Once.run`）在 early stage 是“只有声明没有 body 的外部落点”，
    ///   因此不在 `fun_index` 中；但 closure codegen 仍需要一个 expected function type 来确定参数绑定。
    pub(in crate::llvm::codegen) fn lookup_pure_unit_closure_type(&self) -> Option<TypeId> {
        let unit = self
            .types
            .iter_ids()
            .find(|id| matches!(self.types.kind(*id), TypeKind::Value(ValueTypeKind::Unit)))?;

        let mut fallback = None;
        for id in self.types.iter_ids() {
            let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(id) else {
                continue;
            };
            if fun_ty.receiver.is_some()
                || !fun_ty.params.is_empty()
                || fun_ty.return_ty != unit
                || !fun_ty.effects.is_pure()
            {
                continue;
            }
            if fun_ty.effects_closed {
                return Some(id);
            }
            fallback.get_or_insert(id);
        }
        fallback
    }
}

pub(in crate::llvm::codegen) fn closure_callee_resume_entry_fn_name(
    closure_id: hir::ClosureId,
) -> String {
    format!("scoop.lambda_resume${}", closure_id.as_u32())
}
