//! Class constructor selection and initialization lowering split out of `codegen/mod.rs`.

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    /// 生成 class 构造调用（Appendix B.2.2，Kotlin-like 初始化顺序）。
    ///
    /// 当前阶段的约束：
    /// - normal class ctor call 必须消费前端准备好的 `CtorCallInfo`；
    /// - named/default args 的选择与顺序必须由 `CtorCallInfo.arg_mapping` 固化；
    /// - backend 不再按参数个数或缺失 `CtorCallInfo` 猜测 ctor 目标；
    /// - class 单继承初始化链：会从最基类到派生类逐层执行 init steps；
    /// - super ctor args 与 secondary ctor delegation args 同样优先走 `CtorCallInfo` 映射，
    ///   并按源码顺序求值。
    pub(in crate::llvm::codegen) fn codegen_class_ctor_call(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        _name: &str,
        args: &[hir::CallArg],
        site: &hir::CtorCallInfo,
        result_ty: Option<TypeId>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let base_fqn = site.class_fqn.clone();
        let class_fqn = if let Some(rty) = result_ty {
            if let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.types.kind(rty) {
                if !nominal.args.is_empty() {
                    let mangled = self.nominal_layout_key(nominal);
                    if self.class_inits.contains_key(&mangled) {
                        mangled
                    } else {
                        base_fqn
                    }
                } else {
                    base_fqn
                }
            } else {
                base_fqn
            }
        } else {
            base_fqn
        };
        if !self.class_inits.contains_key(&class_fqn) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "class ctor call candidate class",
                at: callee_span.into(),
            });
        }
        let class = self.class_init_layout(callee_span, &class_fqn)?;

        let selected_ctor = self.pick_class_ctor_by_target(
            callee_span,
            &class,
            site.ctor_span,
            args.len(),
            None,
            "class ctor selected/ordered args contract",
        )?;
        let ctor_params: &[hir::ClassCtorParam] = match selected_ctor {
            Some(ctor) => ctor.params.as_slice(),
            None => &[][..],
        };
        // 4) 分配对象（header 由 runtime 初始化）；payload 先清零，避免读取未初始化字段导致的非确定性。
        let obj_ty = self.llvm_class_object_type(span, &class)?;
        let obj_size_bytes = self.target_data.get_store_size(&obj_ty);

        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);

        // 分配点统一走 typed alloc：在 runtime 内部写入对象头 `type_desc`。
        let type_desc = self.get_or_create_class_type_desc_global(span, &class_fqn)?;
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
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(obj_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return type",
                at: span.into(),
            });
        };

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
            Some(site),
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
            selected_ctor,
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
            let active_bb_end =
                self.builder
                    .get_insert_block()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "class ctor call active block",
                        at: span.into(),
                    })?;
            self.builder.build_unconditional_branch(merge_bb)?;

            self.builder.position_at_end(inactive_bb);
            let current_obj = self.reload_deferred_gc_ref_without_clearing(
                span,
                "class_ctor_obj_return",
                &deferred_obj,
            )?;
            let inactive_bb_end =
                self.builder
                    .get_insert_block()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "class ctor call inactive block",
                        at: span.into(),
                    })?;
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

    pub(in crate::llvm::codegen) fn pick_class_ctor_by_target<'b>(
        &self,
        at: crate::span::Span,
        class: &'b hir::ClassInit,
        target_ctor_span: Option<crate::span::Span>,
        arg_count: usize,
        exclude_ctor_span: Option<crate::span::Span>,
        kind: &'static str,
    ) -> Result<Option<&'b hir::ClassCtor>, LlvmEmitError> {
        if let Some(target_span) = target_ctor_span {
            let mut matching: Vec<&hir::ClassCtor> = class
                .ctors
                .iter()
                .filter(|ctor| ctor.span == target_span)
                .collect();
            if let Some(exclude) = exclude_ctor_span {
                matching.retain(|ctor| ctor.span != exclude);
            }
            if matching.len() != 1 {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind,
                    at: at.into(),
                });
            }
            return Ok(Some(matching[0]));
        }

        if class.ctors.is_empty() {
            return if arg_count == 0 {
                Ok(None)
            } else {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind,
                    at: at.into(),
                })
            };
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind,
            at: at.into(),
        })
    }

    fn codegen_class_ctor_eval_args(
        &mut self,
        at: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
        call_info: Option<&hir::CtorCallInfo>,
        ctor_params: &[hir::ClassCtorParam],
        kind: &'static str,
    ) -> Result<Vec<CgValue<'ctx>>, LlvmEmitError> {
        let mapping: Vec<Option<usize>> = if let Some(info) = call_info {
            if info.arg_mapping.len() != ctor_params.len() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind,
                    at: at.into(),
                });
            }
            info.arg_mapping.clone()
        } else {
            if !args.is_empty() || !ctor_params.is_empty() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind,
                    at: at.into(),
                });
            }

            Vec::new()
        };

        let mut arg_to_param: Vec<Option<usize>> = vec![None; args.len()];
        for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
            let Some(arg_idx) = arg_idx else {
                continue;
            };
            let slot = arg_to_param
                .get_mut(arg_idx)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind,
                    at: at.into(),
                })?;
            if slot.is_some() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind,
                    at: at.into(),
                });
            }
            *slot = Some(param_idx);
        }
        if arg_to_param.iter().any(|slot| slot.is_none()) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: at.into(),
            });
        }

        let mut param_values: Vec<Option<CgValue<'ctx>>> = vec![None; ctor_params.len()];
        let mut explicit_values: Vec<Option<(crate::span::Span, DeferredCgValue<'ctx>)>> =
            vec![None; ctor_params.len()];

        for (arg_idx, arg) in args.iter().enumerate() {
            let param_idx = arg_to_param[arg_idx].ok_or(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: at.into(),
            })?;
            let param = &ctor_params[param_idx];
            let param_cg = self
                .cg_ty_of(param.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "class ctor param type",
                    at: callee_span.into(),
                })?;
            let expr = match arg {
                hir::CallArg::Positional(expr) => expr,
                hir::CallArg::Named { value, .. } => value,
            };
            let v = match &expr.kind {
                hir::ExprKind::Closure(closure) => {
                    self.codegen_closure_expr(expr.span, closure, param.ty)?
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
                        .default_value
                        .as_ref()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind,
                            at: callee_span.into(),
                        })?;
                let param_cg =
                    self.cg_ty_of(param.ty)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "class ctor param type",
                            at: callee_span.into(),
                        })?;
                let v = match &default_value.kind {
                    hir::ExprKind::Closure(closure) => {
                        self.codegen_closure_expr(default_value.span, closure, param.ty)?
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
                    value.ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind,
                        at: at.into(),
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        })();

        self.clear_gc_locals_in_current_scope(callee_span, "class_ctor_arg_scope_drop")?;
        self.function_cx.env.pop_scope();
        result
    }

    fn bind_class_ctor_call_param_value(
        &mut self,
        callee_span: crate::span::Span,
        _kind: &'static str,
        param: &hir::ClassCtorParam,
        expr_span: crate::span::Span,
        value: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let param_cg = self
            .cg_ty_of(param.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "class ctor param type",
                at: callee_span.into(),
            })?;
        let ptr = self.create_entry_alloca(param.decl_span, &param.name, param_cg)?;
        let stored = self.store_local_value(expr_span, ptr, param_cg, value)?;
        self.function_cx.env.insert(
            param.id,
            CgLocal {
                hir_ty: Some(param.ty),
                call_may_suspend: self.local_call_may_suspend_from_hir_ty(Some(param.ty)),
                ty: param_cg,
                ptr,
                frame_backing_ptr: None,
                mutable: false,
            },
        );
        Ok(stored)
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_class_ctor_call_super(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        class: &hir::ClassInit,
        super_args: &[hir::CallArg],
        super_call: Option<&hir::CtorCallInfo>,
        stack: &mut HashSet<(String, crate::span::Span)>,
        kind: &'static str,
    ) -> Result<(), LlvmEmitError> {
        let Some(super_fqn) = class.super_class_fqn.as_deref() else {
            return Ok(());
        };

        let super_class = self.class_init_layout(callee_span, super_fqn)?;
        let super_ctor = self.pick_class_ctor_by_target(
            callee_span,
            &super_class,
            super_call.and_then(|call| call.ctor_span),
            super_args.len(),
            None,
            kind,
        )?;

        let super_ctor_params: &[hir::ClassCtorParam] = match super_ctor {
            Some(ctor) => ctor.params.as_slice(),
            None => &[][..],
        };
        let super_values = self.codegen_class_ctor_eval_args(
            callee_span,
            callee_span,
            super_args,
            super_call,
            super_ctor_params,
            kind,
        )?;
        let obj_ptr = self.current_class_ctor_this_ptr(callee_span, class)?;

        self.codegen_class_ctor_invoke_inner(
            span,
            callee_span,
            &super_class,
            super_ctor,
            super_values.as_slice(),
            obj_ptr,
            stack,
        )?;

        self.emit_current_local_effect_escape_check(callee_span, "class_ctor_super_effect")?;

        Ok(())
    }

    fn codegen_class_ctor_run_init_steps(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        class: &hir::ClassInit,
        ctor_params: &[hir::ClassCtorParam],
    ) -> Result<(), LlvmEmitError> {
        // primary ctor 参数属性赋值（在 super ctor 之后执行，Kotlin-like）。
        for param in ctor_params {
            if !param.is_property {
                continue;
            }
            let param_cg = self
                .cg_ty_of(param.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "class ctor param type",
                    at: callee_span.into(),
                })?;

            let Some(field_fqn) = param.property_field_fqn.as_deref() else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "class ctor param property fqn",
                    at: callee_span.into(),
                });
            };
            let Some(field_idx) = class.field_indices.get(field_fqn).copied() else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "class ctor param property field index",
                    at: callee_span.into(),
                });
            };
            let local =
                self.function_cx
                    .env
                    .get(param.id)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "class ctor param local slot",
                        at: callee_span.into(),
                    })?;
            let local_ptr =
                self.local_ptr_for_use(span, local, &format!("class_ctor_param_{}", param.name))?;
            let loaded = self.builder.build_load(
                self.llvm_basic_type_of(span, param_cg)?,
                local_ptr,
                &format!("load_class_ctor_param_{}", param.name),
            )?;
            let arg_v = self.cg_value_from_loaded(span, param_cg, loaded)?;
            let obj_ptr = self.current_class_ctor_this_ptr(span, class)?;
            let field_ptr = self.codegen_class_field_ptr(span, class, obj_ptr, field_idx)?;
            let _ = self.store_local_value_exact(span, field_ptr, param_cg, arg_v)?;
        }

        // property initializer / init blocks（按源码顺序）
        for step in &class.steps {
            match step {
                hir::ClassInitStep::PropertyInit { field_fqn, init } => {
                    let Some(field_idx) = class.field_indices.get(field_fqn).copied() else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "class property init field index",
                            at: init.span.into(),
                        });
                    };
                    let field = class.fields.get(field_idx as usize).ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "class property init field",
                            at: init.span.into(),
                        },
                    )?;
                    let field_cg =
                        self.cg_ty_of(field.ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "class property init field type",
                                at: init.span.into(),
                            })?;

                    let v = self.codegen_expr_in_expected_context(init, Some(field_cg))?;
                    let obj_ptr = self.current_class_ctor_this_ptr(init.span, class)?;
                    let field_ptr =
                        self.codegen_class_field_ptr(init.span, class, obj_ptr, field_idx)?;
                    let _ = self.store_local_value(init.span, field_ptr, field_cg, v)?;
                }
                hir::ClassInitStep::InitBlock { block } => {
                    let _ = self.codegen_block_value(block)?;
                }
            }
        }

        Ok(())
    }

    pub(in crate::llvm::codegen) fn codegen_class_ctor_invoke(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        class: &hir::ClassInit,
        ctor: Option<&hir::ClassCtor>,
        args: &[CgValue<'ctx>],
        obj_ptr: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let mut stack: HashSet<(String, crate::span::Span)> = HashSet::new();
        self.codegen_class_ctor_invoke_inner(
            span,
            callee_span,
            class,
            ctor,
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
        class: &hir::ClassInit,
        ctor: Option<&hir::ClassCtor>,
        args: &[CgValue<'ctx>],
        obj_ptr: PointerValue<'ctx>,
        stack: &mut HashSet<(String, crate::span::Span)>,
    ) -> Result<(), LlvmEmitError> {
        let (ctor_kind, ctor_span, ctor_params, ctor_body, delegation) = match ctor {
            Some(ctor) => (
                ctor.kind,
                ctor.span,
                ctor.params.as_slice(),
                ctor.body.as_ref(),
                ctor.delegation.as_ref(),
            ),
            None => (
                hir::ClassCtorKind::Primary,
                callee_span,
                &[][..],
                None,
                None,
            ),
        };

        let key = (class.fqn.clone(), ctor_span);
        if !stack.insert(key.clone()) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "class ctor delegation cycle",
                at: callee_span.into(),
            });
        }

        let saved_source_id = self.current_source_id;
        self.current_source_id =
            self.source_id_for_path(class.source_path.as_path(), callee_span)?;
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
                    class.this_id,
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
                if args.len() != ctor_params.len() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "class ctor call arg/param len mismatch",
                        at: callee_span.into(),
                    });
                }

                for (param, arg_v) in ctor_params.iter().zip(args.iter()) {
                    let param_cg =
                        cg.cg_ty_of(param.ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "class ctor param type",
                                at: callee_span.into(),
                            })?;
                    let param_ptr =
                        cg.create_entry_alloca(param.decl_span, &param.name, param_cg)?;
                    let _ =
                        cg.store_local_value_exact(param.decl_span, param_ptr, param_cg, *arg_v)?;
                    cg.function_cx.env.insert(
                        param.id,
                        CgLocal {
                            hir_ty: Some(param.ty),
                            call_may_suspend: cg.local_call_may_suspend_from_hir_ty(Some(param.ty)),
                            ty: param_cg,
                            ptr: param_ptr,
                            frame_backing_ptr: None,
                            mutable: false,
                        },
                    );
                }

                // secondary ctor delegation（T1327c）
                if ctor_kind == hir::ClassCtorKind::Secondary
                    && let Some(deleg) = delegation
                {
                    match deleg.kind {
                        ast::CtorDelegationKind::This => {
                            let target = cg.pick_class_ctor_by_target(
                                callee_span,
                                class,
                                deleg.call.as_ref().and_then(|call| call.ctor_span),
                                deleg.args.len(),
                                Some(ctor_span),
                                "class this delegation selected/ordered args contract",
                            )?;

                            let target_params: &[hir::ClassCtorParam] = match target {
                                Some(c) => c.params.as_slice(),
                                None => &[][..],
                            };
                            let target_values = cg.codegen_class_ctor_eval_args(
                                callee_span,
                                callee_span,
                                deleg.args.as_slice(),
                                deleg.call.as_ref(),
                                target_params,
                                "class this delegation arg eval",
                            )?;
                            let current_obj = cg.current_class_ctor_this_ptr(callee_span, class)?;

                            cg.codegen_class_ctor_invoke_inner(
                                span,
                                callee_span,
                                class,
                                target,
                                target_values.as_slice(),
                                current_obj,
                                stack,
                            )?;
                            cg.emit_current_local_effect_escape_check(
                                callee_span,
                                "class_ctor_this_delegation_effect",
                            )?;

                            if let Some(body) = ctor_body {
                                let _ = cg.codegen_block_value(body)?;
                            }

                            cg.clear_gc_locals_in_current_scope(
                                callee_span,
                                "class_ctor_invoke_scope_drop",
                            )?;
                            cg.builder.build_unconditional_branch(merge_bb)?;
                            return Ok(());
                        }
                        ast::CtorDelegationKind::Super => {
                            cg.codegen_class_ctor_call_super(
                                span,
                                callee_span,
                                class,
                                deleg.args.as_slice(),
                                deleg.call.as_ref(),
                                stack,
                                "class super delegation selected/ordered args contract",
                            )?;

                            cg.codegen_class_ctor_run_init_steps(
                                span,
                                callee_span,
                                class,
                                ctor_params,
                            )?;

                            if let Some(body) = ctor_body {
                                let _ = cg.codegen_block_value(body)?;
                            }

                            cg.clear_gc_locals_in_current_scope(
                                callee_span,
                                "class_ctor_invoke_scope_drop",
                            )?;
                            cg.builder.build_unconditional_branch(merge_bb)?;
                            return Ok(());
                        }
                    }
                }

                // primary ctor / secondary ctor（无 delegation）路径：使用 class header 的 super ctor args。
                cg.codegen_class_ctor_call_super(
                    span,
                    callee_span,
                    class,
                    class.super_ctor_args.as_slice(),
                    class.super_ctor_call.as_ref(),
                    stack,
                    "class super ctor selected/ordered args contract",
                )?;

                cg.codegen_class_ctor_run_init_steps(span, callee_span, class, ctor_params)?;

                if ctor_kind == hir::ClassCtorKind::Secondary
                    && let Some(body) = ctor_body
                {
                    let _ = cg.codegen_block_value(body)?;
                }

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
        stack.remove(&key);
        result
    }

    fn current_class_ctor_this_ptr(
        &mut self,
        at: crate::span::Span,
        class: &hir::ClassInit,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let this_local =
            self.function_cx
                .env
                .get(class.this_id)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "class ctor this local",
                    at: at.into(),
                })?;
        let this_slot = self.local_ptr_for_use(at, this_local, "class_ctor_this_reload")?;
        Ok(self
            .builder
            .build_load(self.llvm_gc_i8_ptr_type(), this_slot, "class_ctor_this_obj")?
            .into_pointer_value())
    }
}
