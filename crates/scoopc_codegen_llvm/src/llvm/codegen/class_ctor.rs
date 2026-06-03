//! Class constructor selection and initialization lowering split out of `codegen/mod.rs`.

use crate::effect_lowered::ir::{
    LateLoweredClassCtorDelegation, LateLoweredClassCtorInitBody, LateLoweredClassCtorInitStep,
    LateLoweredClassCtorParam, LateLoweredClassCtorSuperCall, LateLoweredSourceClassCtorBlock,
    LateLoweredSourceClassCtorExpr,
};

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    /// 生成 class 构造调用（Appendix B.2.2，Kotlin-like 初始化顺序）。
    ///
    /// 当前阶段的约束：
    /// - normal class ctor call 必须消费 LIR facts 发布的 ctor call-site contract；
    /// - named/default args 的选择与顺序必须由 `arg_mapping` 固化；
    /// - backend 不再按参数个数或缺失 call-site contract 猜测 ctor 目标；
    /// - class 单继承初始化链：会从最基类到派生类逐层执行 init steps；
    /// - super ctor args 与 secondary ctor delegation args 同样优先走 `CtorCallInfo` 映射，
    ///   并按源码顺序求值。
    #[allow(clippy::too_many_arguments, dead_code)]
    pub(in crate::llvm::codegen) fn codegen_class_ctor_call(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        class_fqn: &str,
        args: &[hir::CallArg],
        ctor_span: Option<crate::span::Span>,
        arg_mapping: &[Option<usize>],
        result_ty: Option<TypeId>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let base_fqn = class_fqn;
        let result_ty = result_ty.ok_or_else(|| LlvmEmitError::Frontend {
            message: format!("class ctor `{base_fqn}` reached LLVM without a typed result target"),
        })?;
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.types.kind(result_ty) else {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "class ctor `{base_fqn}` result type t{} is not a nominal class reference",
                    result_ty.as_u32()
                ),
            });
        };
        if nominal.fqn != base_fqn {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "class ctor `{base_fqn}` result type resolves to mismatched nominal `{}`",
                    nominal.fqn
                ),
            });
        }
        let mono_result_ty =
            self.types
                .as_mono(result_ty)
                .map_err(|leak| LlvmEmitError::Frontend {
                    message: format!(
                        "class ctor `{base_fqn}` result type t{} is not fully monomorphic: {:?}",
                        result_ty.as_u32(),
                        leak.leak_path
                    ),
                })?;
        let class_key = hir::ClassInstanceKey::from_mono_nominal(self.types, mono_result_ty)
            .expect("nominal result type must produce ClassInstanceKey");
        if !self.class_inits.contains_key(&class_key) {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "class ctor `{base_fqn}` resolved to missing class layout key `{class_key}`"
                ),
            });
        }
        let class = self.class_init_layout(callee_span, &class_key)?;

        let selected_ctor = self.pick_class_ctor_by_target(
            callee_span,
            &class,
            ctor_span,
            args.len(),
            None,
            "class ctor selected/ordered args contract",
        )?;
        let init_body =
            self.class_ctor_init_body_for_selected(callee_span, &class, selected_ctor)?;
        let ctor_params = init_body.params();
        // 4) 分配对象（header 由 runtime 初始化）；payload 先清零，避免读取未初始化字段导致的非确定性。
        let obj_ty = self.llvm_class_object_type(span, &class)?;
        let obj_size_bytes = self.target_data.get_store_size(&obj_ty);

        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);

        // 分配点统一走 typed alloc：在 runtime 内部写入对象头 `type_desc`。
        let type_desc = self.get_or_create_class_type_desc_global(span, &class_key)?;
        let type_desc_i8 = self.builder.build_pointer_cast(
            type_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "class_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_alloc,
            &[type_desc_i8.into(), size_v.into()],
            "rt_alloc_class",
        )?;
        let raw = self.expect_basic_value(call, "scoop_alloc_typed class allocation");
        let obj_ptr = self.expect_pointer_value(raw, "scoop_alloc_typed class allocation");

        let obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let typed_obj = self
            .builder
            .build_pointer_cast(obj_ptr, obj_ptr_ty, "class_obj_ptr")?;

        let payload_ptr =
            self.builder
                .build_struct_gep(obj_ty, typed_obj, 1, "class_payload_gep")?;
        let payload_ty = self.llvm_class_payload_type(span, &class)?;
        let payload_size_bytes = self.target_data.get_store_size(&payload_ty);
        if payload_size_bytes > 0 {
            let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
            let payload_i8 = self
                .builder
                .build_bit_cast(payload_ptr, i8_ptr_ty, "class_payload_i8")?
                .into_pointer_value();
            let size_ty = self.llvm_ptr_sized_int_type(None);
            let size_v = size_ty.const_int(payload_size_bytes, false);
            let zero = self.context.i8_type().const_int(0, false);
            let _ = self.builder.build_memset(payload_i8, 1, zero, size_v)?;
        }

        let deferred_obj = self.defer_gc_sensitive_cg_value(
            span,
            "class_ctor_obj_root",
            CgValue {
                ty: CgTy::Ref,
                value: Some(obj_ptr.into()),
            },
        )?;

        // 6) 执行构造调用：支持 super ctor args + secondary ctor delegation（T1327c）。
        //
        // 语义（Kotlin-like，Appendix B.2.2）：
        // - 调用点先按源码顺序求值 ctor 实参；
        // - 进入 ctor 后：
        //   - 若是 `: this(...)`，先执行被委托 ctor，再执行当前 ctor body；
        //   - 否则先执行 super ctor call，再执行本类的参数属性赋值、property initializer、init blocks，
        //     最后执行 secondary ctor body（若有）。

        let evaluated_args = self.codegen_class_ctor_eval_args(
            callee_span,
            callee_span,
            args,
            Some(arg_mapping),
            ctor_params,
            "class ctor call arg eval",
        )?;

        let current_obj = self.reload_deferred_gc_ref_without_clearing(
            span,
            "class_ctor_obj_before_invoke",
            &deferred_obj,
        )?;

        self.codegen_class_ctor_invoke(
            span,
            callee_span,
            &class,
            &init_body,
            evaluated_args.as_slice(),
            current_obj,
        )?;

        self.emit_ordinary_call_effect_propagation_check(span, "class_ctor_call_effect")?;

        if !self.ordinary_effect_propagation_enabled()
            && let Some(outcome_ptr) = self.function_cx.current_effect_outcome_ptr
        {
            let current_fn = self.expect_current_function("class ctor call current function");
            let active_bb = self
                .context
                .append_basic_block(current_fn, "class_ctor_call_active");
            let inactive_bb = self
                .context
                .append_basic_block(current_fn, "class_ctor_call_inactive");
            let merge_bb = self
                .context
                .append_basic_block(current_fn, "class_ctor_call_merge");
            let is_propagating =
                self.effect_outcome_is_propagating(span, outcome_ptr, "class_ctor_call_effect")?;
            self.builder
                .build_conditional_branch(is_propagating, active_bb, inactive_bb)?;

            self.builder.position_at_end(active_bb);
            self.clear_deferred_cg_value_root_homes(
                span,
                "class_ctor_obj_active_drop",
                &deferred_obj,
            )?;
            let active_bb_end = self.expect_insert_block("class ctor call active branch");
            self.builder.build_unconditional_branch(merge_bb)?;

            self.builder.position_at_end(inactive_bb);
            let current_obj = self.reload_deferred_gc_ref_without_clearing(
                span,
                "class_ctor_obj_return",
                &deferred_obj,
            )?;
            let inactive_bb_end = self.expect_insert_block("class ctor call inactive branch");
            self.builder.build_unconditional_branch(merge_bb)?;

            self.builder.position_at_end(merge_bb);
            let result_phi = self
                .builder
                .build_phi(self.llvm_gc_i8_ptr_type(), "class_ctor_call_result")?;
            result_phi.add_incoming(&[
                (&self.llvm_gc_i8_ptr_type().const_null(), active_bb_end),
                (&current_obj, inactive_bb_end),
            ]);

            return Ok(CgValue {
                ty: CgTy::Ref,
                value: Some(result_phi.as_basic_value()),
            });
        }

        let current_obj = self.reload_deferred_gc_ref_without_clearing(
            span,
            "class_ctor_obj_return",
            &deferred_obj,
        )?;

        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(current_obj.into()),
        })
    }

    #[allow(dead_code)]
    pub(in crate::llvm::codegen) fn pick_class_ctor_by_target<'b>(
        &self,
        _at: crate::span::Span,
        class: &'b hir::MonoClassInit,
        target_ctor_span: Option<crate::span::Span>,
        arg_count: usize,
        exclude_ctor_span: Option<crate::span::Span>,
        kind: &'static str,
    ) -> Result<Option<&'b hir::ClassCtor<MonoTypeId>>, LlvmEmitError> {
        if let Some(target_span) = target_ctor_span {
            let mut matching: Vec<&hir::ClassCtor<MonoTypeId>> = class
                .ctors
                .iter()
                .filter(|ctor| ctor.span == target_span)
                .collect();
            if let Some(exclude) = exclude_ctor_span {
                matching.retain(|ctor| ctor.span != exclude);
            }
            if matching.len() != 1 {
                panic!("pick_class_ctor_by_target: verifier accepted {kind}");
            }
            return Ok(Some(matching[0]));
        }

        if class.ctors.is_empty() {
            return if arg_count == 0 {
                Ok(None)
            } else {
                panic!("pick_class_ctor_by_target: verifier accepted {kind}")
            };
        }

        let matching = class
            .ctors
            .iter()
            .filter(|ctor| {
                ctor.params.len() >= arg_count
                    && ctor.params[arg_count..]
                        .iter()
                        .all(|param| param.has_default)
            })
            .collect::<Vec<_>>();
        if matching.len() == 1 {
            return Ok(Some(matching[0]));
        }

        panic!("pick_class_ctor_by_target: verifier accepted {kind}")
    }

    pub(in crate::llvm::codegen) fn class_ctor_init_body_for_selected(
        &self,
        at: crate::span::Span,
        class: &hir::MonoClassInit,
        ctor: Option<&hir::ClassCtor<MonoTypeId>>,
    ) -> Result<LateLoweredClassCtorInitBody, LlvmEmitError> {
        let key = LirClassCtorInitKey::for_ctor(
            class.fqn.as_str(),
            ctor.map(|ctor| (ctor.span.start, ctor.span.end)),
        );
        self.class_ctor_init_body_for_key(at, &key)
    }

    pub(in crate::llvm::codegen) fn class_ctor_init_body_for_key(
        &self,
        _at: crate::span::Span,
        key: &LirClassCtorInitKey,
    ) -> Result<LateLoweredClassCtorInitBody, LlvmEmitError> {
        let program =
            self.published_late_lowered_program()
                .ok_or_else(|| LlvmEmitError::Frontend {
                    message: "class ctor init contract requires published LateLoweredProgram"
                        .to_string(),
                })?;
        program
            .class_ctor_init_body(key)
            .cloned()
            .or_else(|| self.class_ctor_init_bodies.get(key.as_str()).cloned())
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "class ctor init body `{}` is missing from LIR facts and LLVM base context",
                    key.as_str()
                ),
            })
    }

    fn codegen_class_ctor_eval_args(
        &mut self,
        _at: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
        arg_mapping: Option<&[Option<usize>]>,
        ctor_params: &[LateLoweredClassCtorParam],
        kind: &'static str,
    ) -> Result<Vec<CgValue<'ctx>>, LlvmEmitError> {
        let mapping: Vec<Option<usize>> = if let Some(arg_mapping) = arg_mapping {
            if arg_mapping.len() != ctor_params.len() {
                panic!("codegen_class_ctor_eval_args: verifier accepted {kind}");
            }
            arg_mapping.to_vec()
        } else {
            if !args.is_empty() || !ctor_params.is_empty() {
                panic!("codegen_class_ctor_eval_args: verifier accepted {kind}");
            }

            Vec::new()
        };

        let mut arg_to_param: Vec<Option<usize>> = vec![None; args.len()];
        for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
            let Some(arg_idx) = arg_idx else {
                continue;
            };
            let slot = arg_to_param.get_mut(arg_idx).unwrap_or_else(|| {
                panic!("codegen_class_ctor_eval_args: verifier accepted {kind}")
            });
            if slot.is_some() {
                panic!("codegen_class_ctor_eval_args: verifier accepted {kind}");
            }
            *slot = Some(param_idx);
        }
        if arg_to_param.iter().any(|slot| slot.is_none()) {
            panic!("codegen_class_ctor_eval_args: verifier accepted {kind}");
        }

        let mut param_values: Vec<Option<CgValue<'ctx>>> = vec![None; ctor_params.len()];
        let mut explicit_values: Vec<Option<(crate::span::Span, DeferredCgValue<'ctx>)>> =
            vec![None; ctor_params.len()];

        for (arg_idx, arg) in args.iter().enumerate() {
            let param_idx = arg_to_param[arg_idx].unwrap_or_else(|| {
                panic!("codegen_class_ctor_eval_args: verifier accepted {kind}")
            });
            let param = &ctor_params[param_idx];
            let param_cg = self.cg_ty_of_type_id(param.ty().inner(), "class ctor param type");
            let expr = match arg {
                hir::CallArg::Positional(expr) => expr,
                hir::CallArg::Named { value, .. } => value,
            };
            let v = match &expr.kind {
                hir::ExprKind::Closure(closure) => {
                    self.codegen_closure_expr(expr.span, closure, param.ty().inner())?
                }
                _ => self.codegen_expr_in_expected_context(expr, Some(param_cg))?,
            };
            let v = self.coerce_value(expr.span, v, param_cg)?;
            let deferred = self.defer_gc_sensitive_cg_value(
                expr.span,
                &format!("class_ctor_arg_{param_idx}"),
                v,
            )?;
            explicit_values[param_idx] = Some((expr.span, deferred));
        }

        self.function_cx.env.push_scope();

        let result = (|| {
            // 先在“ctor 参数作用域”里绑定所有显式实参，再计算默认值。
            // 这样默认值表达式仍可读取已提供的参数，同时不会在显式实参求值阶段
            // 用 side-table `ClassCtorParam` 的 `SymbolId` 污染调用者的局部环境。
            for (param_idx, param) in ctor_params.iter().enumerate() {
                let Some((expr_span, explicit)) = explicit_values[param_idx].clone() else {
                    continue;
                };
                let explicit = self.materialize_deferred_cg_value(
                    expr_span,
                    &format!("class_ctor_arg_reload_{param_idx}"),
                    explicit,
                )?;
                let stored = self.bind_class_ctor_call_param_value(
                    callee_span,
                    kind,
                    param,
                    expr_span,
                    explicit,
                )?;
                param_values[param_idx] = Some(stored);
            }

            for (param_idx, param) in ctor_params.iter().enumerate() {
                if param_values[param_idx].is_some() {
                    continue;
                }
                let default_value =
                    param
                        .default_value()
                        .unwrap_or_else(|| panic!("codegen_class_ctor_eval_args: verifier accepted missing default value for {kind}"));
                let param_cg =
                    self.cg_ty_of_type_id(param.ty().inner(), "class ctor default param type");
                let v = match &default_value.kind {
                    hir::ExprKind::Closure(closure) => {
                        self.codegen_closure_expr(default_value.span, closure, param.ty().inner())?
                    }
                    _ => self.codegen_expr_in_expected_context(default_value, Some(param_cg))?,
                };
                let v = self.coerce_value(default_value.span, v, param_cg)?;
                let stored = self.bind_class_ctor_call_param_value(
                    callee_span,
                    kind,
                    param,
                    default_value.span,
                    v,
                )?;
                param_values[param_idx] = Some(stored);
            }

            param_values
                .into_iter()
                .map(|value| {
                    Ok(value.unwrap_or_else(|| {
                        panic!("codegen_class_ctor_eval_args: verifier accepted {kind}")
                    }))
                })
                .collect::<Result<Vec<_>, _>>()
        })();

        self.clear_gc_locals_in_current_scope(callee_span, "class_ctor_arg_scope_drop")?;
        self.function_cx.env.pop_scope();
        result
    }

    fn bind_class_ctor_call_param_value(
        &mut self,
        _callee_span: crate::span::Span,
        _kind: &'static str,
        param: &LateLoweredClassCtorParam,
        expr_span: crate::span::Span,
        value: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let param_cg = self.cg_ty_of_type_id(param.ty().inner(), "class ctor bound param type");
        let ptr = self.create_entry_alloca(param.decl_span(), param.name(), param_cg)?;
        let stored = self.store_local_value(expr_span, ptr, param_cg, value)?;
        self.function_cx.env.insert(
            param.id(),
            CgLocal {
                hir_ty: Some(param.ty().inner()),
                call_may_suspend: self.local_call_may_suspend_from_hir_ty(Some(param.ty().inner())),
                ty: param_cg,
                ptr,
                frame_backing_ptr: None,
                mutable: false,
            },
        );
        Ok(stored)
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_class_ctor_call_target(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        target_class_fqn: &str,
        target_key: &LirClassCtorInitKey,
        target_args: &[hir::CallArg],
        target_arg_mapping: Option<&[Option<usize>]>,
        obj_ptr: PointerValue<'ctx>,
        stack: &mut HashSet<String>,
        kind: &'static str,
    ) -> Result<(), LlvmEmitError> {
        let class_key = self
            .registered_class_instance_key(target_class_fqn)
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "class ctor target `{target_class_fqn}` lacks ClassInstanceKey metadata"
                ),
            })?;
        let target_class = self.class_init_layout(callee_span, &class_key)?;
        let target_init = self.class_ctor_init_body_for_key(callee_span, target_key)?;
        let deferred_obj =
            self.defer_gc_ref_pointer(span, "class_ctor_target_obj_root", obj_ptr)?;
        let target_values = self.codegen_class_ctor_eval_args(
            callee_span,
            callee_span,
            target_args,
            target_arg_mapping,
            target_init.params(),
            kind,
        )?;
        let current_obj = self.reload_deferred_gc_ref_without_clearing(
            span,
            "class_ctor_target_obj_before_invoke",
            &deferred_obj,
        )?;

        self.codegen_class_ctor_invoke_inner(
            span,
            callee_span,
            &target_class,
            &target_init,
            target_values.as_slice(),
            current_obj,
            stack,
        )?;
        self.clear_deferred_cg_value_root_homes(span, "class_ctor_target_obj_drop", &deferred_obj)
    }

    fn codegen_class_ctor_delegation(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        delegation: &LateLoweredClassCtorDelegation,
        obj_ptr: PointerValue<'ctx>,
        stack: &mut HashSet<String>,
    ) -> Result<(), LlvmEmitError> {
        let kind = match delegation.kind() {
            LirClassCtorDelegationKind::This => {
                "class this delegation selected/ordered args contract"
            }
            LirClassCtorDelegationKind::Super => {
                "class super delegation selected/ordered args contract"
            }
        };
        self.codegen_class_ctor_call_target(
            span,
            callee_span,
            delegation.class_fqn(),
            delegation.target(),
            delegation.args(),
            delegation.call().map(|call| call.arg_mapping.as_slice()),
            obj_ptr,
            stack,
            kind,
        )?;
        let effect_label = match delegation.kind() {
            LirClassCtorDelegationKind::This => "class_ctor_this_delegation_effect",
            LirClassCtorDelegationKind::Super => "class_ctor_super_effect",
        };
        self.emit_current_local_effect_escape_check(callee_span, effect_label)
    }

    fn codegen_class_ctor_super_contract(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        super_call: &LateLoweredClassCtorSuperCall,
        obj_ptr: PointerValue<'ctx>,
        stack: &mut HashSet<String>,
    ) -> Result<(), LlvmEmitError> {
        self.codegen_class_ctor_call_target(
            span,
            callee_span,
            super_call.class_fqn(),
            super_call.target(),
            super_call.args(),
            super_call.call().map(|call| call.arg_mapping.as_slice()),
            obj_ptr,
            stack,
            "class super ctor selected/ordered args contract",
        )?;
        self.emit_current_local_effect_escape_check(callee_span, "class_ctor_super_effect")
    }

    fn codegen_class_ctor_run_init_steps(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        class: &hir::MonoClassInit,
        init_body: &LateLoweredClassCtorInitBody,
    ) -> Result<(), LlvmEmitError> {
        for step in init_body.steps() {
            match step {
                LateLoweredClassCtorInitStep::PropertyParamAssignment {
                    param_index,
                    field_fqn,
                    ..
                } => {
                    let param = init_body.params().get(*param_index).unwrap_or_else(|| {
                        panic!("codegen_class_ctor_run_init_steps: verifier accepted property param index drift")
                    });
                    let param_cg =
                        self.cg_ty_of_type_id(param.ty().inner(), "class ctor property param type");
                    let Some(field_idx) = class.field_indices.get(field_fqn).copied() else {
                        panic!(
                            "codegen_class_ctor_run_init_steps: verifier accepted property param field index drift"
                        );
                    };
                    let local = self.function_cx.env.get(param.id()).unwrap_or_else(|| {
                        panic!("codegen_class_ctor_run_init_steps: verifier accepted missing ctor param local slot")
                    });
                    let local_ptr = self.local_ptr_for_use(
                        span,
                        local,
                        &format!("class_ctor_param_{}", param.name()),
                    )?;
                    let loaded = self.builder.build_load(
                        self.llvm_basic_type_of(span, param_cg)?,
                        local_ptr,
                        &format!("load_class_ctor_param_{}", param.name()),
                    )?;
                    let arg_v = self.cg_value_from_loaded(span, param_cg, loaded)?;
                    let obj_ptr = self.current_class_ctor_this_ptr(span, class)?;
                    let field_ptr =
                        self.codegen_class_field_ptr(span, class, obj_ptr, field_idx)?;
                    let _ = self.store_local_value_exact(span, field_ptr, param_cg, arg_v)?;
                }
                LateLoweredClassCtorInitStep::PropertyInitializer { field_fqn, init } => {
                    let Some(field_idx) = class.field_indices.get(field_fqn).copied() else {
                        panic!(
                            "codegen_class_ctor_run_init_steps: verifier accepted property init field index drift"
                        );
                    };
                    let field = class.fields.get(field_idx as usize).unwrap_or_else(|| {
                        panic!("codegen_class_ctor_run_init_steps: verifier accepted property init field drift")
                    });
                    let field_cg =
                        self.cg_ty_of_type_id(field.ty.inner(), "class property init field type");

                    let v = self.codegen_lir_class_ctor_expr(init, Some(field_cg))?;
                    let obj_ptr = self.current_class_ctor_this_ptr(init.span, class)?;
                    let field_ptr =
                        self.codegen_class_field_ptr(init.span, class, obj_ptr, field_idx)?;
                    let _ = self.store_local_value(init.span, field_ptr, field_cg, v)?;
                }
                LateLoweredClassCtorInitStep::InitBlock { block }
                | LateLoweredClassCtorInitStep::SecondaryBody { block } => {
                    self.codegen_lir_class_ctor_block(block)?;
                }
            }
        }

        Ok(())
    }

    fn codegen_lir_class_ctor_expr(
        &mut self,
        expr: &LateLoweredSourceClassCtorExpr,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_expr_in_expected_context(expr, expected)
    }

    fn codegen_lir_class_ctor_block(
        &mut self,
        block: &LateLoweredSourceClassCtorBlock,
    ) -> Result<(), LlvmEmitError> {
        let _ = self.codegen_block_value(block)?;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn codegen_class_ctor_invoke(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        class: &hir::MonoClassInit,
        init_body: &LateLoweredClassCtorInitBody,
        args: &[CgValue<'ctx>],
        obj_ptr: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let mut stack: HashSet<String> = HashSet::new();
        self.codegen_class_ctor_invoke_inner(
            span,
            callee_span,
            class,
            init_body,
            args,
            obj_ptr,
            &mut stack,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_class_ctor_invoke_inner(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        class: &hir::MonoClassInit,
        init_body: &LateLoweredClassCtorInitBody,
        args: &[CgValue<'ctx>],
        obj_ptr: PointerValue<'ctx>,
        stack: &mut HashSet<String>,
    ) -> Result<(), LlvmEmitError> {
        let key = init_body.key().as_str().to_string();
        if !stack.insert(key.clone()) {
            panic!(
                "codegen_class_ctor_invoke_inner: typecheck accepted class ctor delegation cycle"
            );
        }

        let saved_source_id = self.current_source_id;
        let saved_callable_fqn = self.function_cx.current_callable_fqn.clone();
        let saved_stable_owner_key = self.function_cx.current_stable_owner_key.clone();
        let saved_stable_closure_path_prefix =
            self.function_cx.current_stable_closure_path_prefix.clone();
        let saved_next_stable_child_closure_index =
            self.function_cx.next_stable_child_closure_index;
        let saved_stable_closure_paths = self.function_cx.stable_closure_paths.clone();
        self.current_source_id =
            self.source_id_for_path(init_body.source_path().as_path(), callee_span)?;
        let class_key = class.key();
        self.function_cx.current_callable_fqn = Some(format!("{}.<init>", class_key.as_str()));
        self.function_cx.current_stable_owner_key = Some(self.stable_def_key_for_source_path(
            class.source_path.as_path(),
            StableDefNamespace::Type,
            class_key.as_str(),
            "class_init",
        ));
        self.function_cx.current_stable_closure_path_prefix =
            Some(format!("{}.$init", class_key.as_str()));
        self.function_cx.next_stable_child_closure_index = 0;
        self.function_cx.stable_closure_paths.clear();
        let result = (|| {
            if self.function_cx.current_effect_outcome_ptr.is_none() {
                let outcome_slot =
                    self.alloc_effect_outcome_slot(callee_span, "class_ctor_effect")?;
                self.function_cx.current_effect_outcome_ptr = Some(outcome_slot);
            }
            let current_fn = self.current_codegen_function(callee_span)?;
            let effect_exit_bb = self
                .context
                .append_basic_block(current_fn, "class_ctor_effect_exit");
            let merge_bb = self
                .context
                .append_basic_block(current_fn, "class_ctor_effect_merge");
            self.function_cx.env.push_scope();
            let inner_result = self.with_local_effect_escape_target(effect_exit_bb, |cg| {
                // this local（注意：每一层都有独立的 this SymbolId）。
                let this_ptr = cg.create_entry_alloca(span, "this", CgTy::Ref)?;
                let _ = cg.store_local_value_exact(
                    span,
                    this_ptr,
                    CgTy::Ref,
                    CgValue {
                        ty: CgTy::Ref,
                        value: Some(obj_ptr.into()),
                    },
                )?;
                cg.function_cx.env.insert(
                    init_body.this_id(),
                    CgLocal {
                        hir_ty: None,
                        call_may_suspend: false,
                        ty: CgTy::Ref,
                        ptr: this_ptr,
                        frame_backing_ptr: None,
                        mutable: false,
                    },
                );

                // ctor params locals（先写 locals；参数属性赋值延后到 super ctor call 之后）。
                if args.len() != init_body.params().len() {
                    panic!("codegen_class_ctor_invoke_inner: verifier accepted ctor arg/param length mismatch");
                }

                for (param, arg_v) in init_body.params().iter().zip(args.iter()) {
                    let param_cg = cg
                        .cg_ty_of_type_id(param.ty().inner(), "class ctor invoke param type");
                    let param_ptr =
                        cg.create_entry_alloca(param.decl_span(), param.name(), param_cg)?;
                    let _ =
                        cg.store_local_value_exact(param.decl_span(), param_ptr, param_cg, *arg_v)?;
                    cg.function_cx.env.insert(
                        param.id(),
                        CgLocal {
                            hir_ty: Some(param.ty().inner()),
                            call_may_suspend: cg
                                .local_call_may_suspend_from_hir_ty(Some(param.ty().inner())),
                            ty: param_cg,
                            ptr: param_ptr,
                            frame_backing_ptr: None,
                            mutable: false,
                        },
                    );
                }

                if let Some(delegation) = init_body.delegation() {
                    cg.codegen_class_ctor_delegation(
                        span,
                        callee_span,
                        delegation,
                        obj_ptr,
                        stack,
                    )?;
                } else if let Some(super_call) = init_body.implicit_super() {
                    cg.codegen_class_ctor_super_contract(
                        span,
                        callee_span,
                        super_call,
                        obj_ptr,
                        stack,
                    )?;
                }

                cg.codegen_class_ctor_run_init_steps(span, callee_span, class, init_body)?;

                cg.clear_gc_locals_in_current_scope(callee_span, "class_ctor_invoke_scope_drop")?;
                cg.builder.build_unconditional_branch(merge_bb)?;
                Ok(())
            });

            if let Err(err) = inner_result {
                self.function_cx.env.pop_scope();
                return Err(err);
            }

            self.builder.position_at_end(effect_exit_bb);
            self.clear_gc_locals_in_current_scope(callee_span, "class_ctor_invoke_scope_drop")?;
            self.builder.build_unconditional_branch(merge_bb)?;
            self.function_cx.env.pop_scope();
            self.builder.position_at_end(merge_bb);
            Ok(())
        })();

        self.current_source_id = saved_source_id;
        self.function_cx.current_callable_fqn = saved_callable_fqn;
        self.function_cx.current_stable_owner_key = saved_stable_owner_key;
        self.function_cx.current_stable_closure_path_prefix = saved_stable_closure_path_prefix;
        self.function_cx.next_stable_child_closure_index = saved_next_stable_child_closure_index;
        self.function_cx.stable_closure_paths = saved_stable_closure_paths;
        stack.remove(&key);
        result
    }

    fn current_class_ctor_this_ptr(
        &mut self,
        at: crate::span::Span,
        class: &hir::MonoClassInit,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let this_local = self.function_cx.env.get(class.this_id).unwrap_or_else(|| {
            panic!("current_class_ctor_this_ptr: verifier accepted missing this local")
        });
        let this_slot = self.local_ptr_for_use(at, this_local, "class_ctor_this_reload")?;
        Ok(self
            .builder
            .build_load(self.llvm_gc_i8_ptr_type(), this_slot, "class_ctor_this_obj")?
            .into_pointer_value())
    }
}
