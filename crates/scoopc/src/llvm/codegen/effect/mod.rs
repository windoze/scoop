//! 统一 state-machine effect codegen。
//!
//! - plan builder / segment 投影 / transform / lowering contract 在 `crate::effect::state_machine`
//!   shared 模块
//! - LLVM emitter 在 `state_machine_emitter` 模块
//! - lowering 只从统一合同（`UnifiedHandleLoweringContract`）出发。

use super::*;

mod contract;
use contract::EffectOutcome;

// Backend bridge：从 MainCodegen 收集当前函数级上下文，并调用 shared state-machine skeleton。
mod state_machine_bridge;

// State machine LLVM emitter：从 UnifiedHandleLoweringContract 生成
// LLVM IR（frame type、step function、handle 入口）。
mod state_machine_emitter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EffectTransportKind {
    Word,
    GcRef,
    BoxedComposite,
    TaskTransportTuple,
}

const CALLEE_SUSPEND_STATE_RESUME_WORD_INDEX: u32 = 1;
const CALLEE_SUSPEND_STATE_RESUME_GC_REF_INDEX: u32 = 2;
const CALLEE_SUSPEND_STATE_SITE_TAG_INDEX: u32 = 3;

#[derive(Clone, Copy)]
pub(super) struct CalleeSuspendResumeState<'ctx> {
    pub(super) state_ty: StructType<'ctx>,
    pub(super) state_ptr: PointerValue<'ctx>,
    pub(super) resume_word: IntValue<'ctx>,
    pub(super) resume_gc_ref: PointerValue<'ctx>,
    pub(super) site_tag: IntValue<'ctx>,
}

#[derive(Clone, Copy)]
pub(super) struct ContinuationResumeResultSlots<'ctx> {
    pub(super) out_word_slot: PointerValue<'ctx>,
    pub(super) out_gc_ref_slot: PointerValue<'ctx>,
    pub(super) outcome_slot: PointerValue<'ctx>,
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn clear_continuation_resume_receiver_local_if_any(
        &mut self,
        receiver: &hir::Expr,
    ) -> Result<(), LlvmEmitError> {
        let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &receiver.kind else {
            return Ok(());
        };
        let Some(local) = self.function_cx.env.get(*id) else {
            return Ok(());
        };
        let cleared = self.default_value(receiver.span, local.ty)?;
        let _ = self.store_local_value_exact(
            receiver.span,
            local.ptr,
            local.ty,
            cleared,
        )?;
        Ok(())
    }

    pub(super) fn ordinary_effect_propagation_enabled(&self) -> bool {
        self.function_cx.current_fun_return_ty.is_some()
    }

    fn effect_trace_line_col(&self, span: crate::span::Span) -> Result<(u32, u32), LlvmEmitError> {
        let (line, col) = self
            .current_source()?
            .offset_to_line_col(span.start)
            .map_err(|_| LlvmEmitError::UnsupportedMainBody {
                kind: "effect trace source location",
                at: span.into(),
            })?;
        let line = u32::try_from(line).map_err(|_| LlvmEmitError::UnsupportedMainBody {
            kind: "effect trace line overflow",
            at: span.into(),
        })?;
        let col = u32::try_from(col).map_err(|_| LlvmEmitError::UnsupportedMainBody {
            kind: "effect trace column overflow",
            at: span.into(),
        })?;
        Ok((line, col))
    }

    fn effect_op_call_info_for_span(
        &self,
        span: crate::span::Span,
    ) -> Result<Option<&hir::EffectOpCallInfo>, LlvmEmitError> {
        let site = self.current_call_site(span)?;
        Ok(self.effect_op_call_sites.get(&site))
    }

    pub(super) fn handle_payload_tuple_ty_for_span(
        &self,
        span: crate::span::Span,
    ) -> Result<Option<TypeId>, LlvmEmitError> {
        let site = self.current_call_site(span)?;
        Ok(self.handle_payload_tuple_tys.get(&site).copied())
    }

    fn call_arg_expr(arg: &hir::CallArg) -> &hir::Expr {
        match arg {
            hir::CallArg::Positional(expr) => expr,
            hir::CallArg::Named { value, .. } => value,
        }
    }

    fn build_tuple_cg_value_from_values(
        &mut self,
        at: crate::span::Span,
        tuple_ty: TypeId,
        elements: &[CgValue<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Tuple(element_tys)) = self.types.kind(tuple_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "perform payload tuple type",
                at: at.into(),
            });
        };
        if element_tys.len() != elements.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "perform payload tuple arity mismatch",
                at: at.into(),
            });
        }

        let llvm_tuple_ty = self.llvm_tuple_type(at, tuple_ty)?;
        let mut agg: AggregateValueEnum<'ctx> = llvm_tuple_ty.get_undef().into();
        for (idx, (elem_ty, value)) in element_tys.iter().zip(elements.iter()).enumerate() {
            let elem_cg = self
                .cg_ty_of(*elem_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "perform payload tuple element type",
                    at: at.into(),
                })?;
            let coerced = self.coerce_value(at, *value, elem_cg)?;
            let raw: BasicValueEnum<'ctx> = match elem_cg {
                CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
                _ => coerced.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "perform payload tuple element value",
                    at: at.into(),
                })?,
            };
            agg = self.builder.build_insert_value(
                agg,
                raw,
                idx as u32,
                &format!("perform_payload_elem_{idx}"),
            )?;
        }

        Ok(CgValue {
            ty: CgTy::Tuple(tuple_ty),
            value: Some(agg.as_basic_value_enum()),
        })
    }

    fn codegen_continuation_resume_legacy_payload_value(
        &mut self,
        payload_ty: Option<TypeId>,
        args: &[hir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let [arg] = args else {
            return Ok(None);
        };

        let payload_expr = match arg {
            hir::CallArg::Positional(expr) => expr,
            hir::CallArg::Named { name, value, .. } if name == "value" => value,
            hir::CallArg::Named { .. } => return Ok(None),
        };
        let payload_expected = payload_ty
            .and_then(|ty| self.cg_ty_of(ty))
            .or_else(|| self.resolve_expr_cg_ty(payload_expr));
        let payload = match &payload_expr.kind {
            hir::ExprKind::Closure(closure) if payload_ty.is_some() => self.codegen_closure_expr(
                payload_expr.span,
                closure,
                payload_ty.expect("checked is_some() above"),
            )?,
            _ => self.codegen_expr_in_expected_context(payload_expr, payload_expected)?,
        };
        let payload = if let Some(expected_cg) = payload_expected {
            self.coerce_value(payload_expr.span, payload, expected_cg)?
        } else {
            payload
        };
        Ok(Some(payload))
    }

    fn codegen_continuation_resume_expanded_payload_value(
        &mut self,
        at: crate::span::Span,
        payload_ty: TypeId,
        args: &[hir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        match self.types.kind(payload_ty) {
            TypeKind::Value(ValueTypeKind::Unit) => {
                if !args.is_empty() {
                    return Ok(None);
                }
                Ok(Some(CgValue::unit()))
            }
            TypeKind::Value(ValueTypeKind::Tuple(element_tys)) => {
                if args.len() != element_tys.len() {
                    return Ok(None);
                }

                let param_names = (0..element_tys.len())
                    .map(|idx| format!("a{idx}"))
                    .collect::<Vec<_>>();
                let Some(arg_to_param) = self.map_call_args_to_params_by_name(&param_names, args)
                else {
                    return Ok(None);
                };

                let mut evaluated_args: Vec<Option<CgValue<'ctx>>> = vec![None; args.len()];
                for (arg_idx, arg) in args.iter().enumerate() {
                    let param_idx = arg_to_param.get(arg_idx).copied().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "Continuation.resume payload mapping",
                            at: at.into(),
                        },
                    )?;
                    let expected_ty = element_tys.get(param_idx).copied().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "Continuation.resume payload element type",
                            at: at.into(),
                        },
                    )?;
                    let expected_cg =
                        self.cg_ty_of(expected_ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "Continuation.resume payload element cg type",
                                at: at.into(),
                            })?;
                    let expr = Self::call_arg_expr(arg);
                    let value = match &expr.kind {
                        hir::ExprKind::Closure(closure) => {
                            self.codegen_closure_expr(expr.span, closure, expected_ty)?
                        }
                        _ => self.codegen_expr_in_expected_context(expr, Some(expected_cg))?,
                    };
                    evaluated_args[arg_idx] =
                        Some(self.coerce_value(expr.span, value, expected_cg)?);
                }

                let ordered = arg_to_param
                    .iter()
                    .enumerate()
                    .map(|(arg_idx, param_idx)| {
                        evaluated_args
                            .get(arg_idx)
                            .and_then(|value| *value)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "Continuation.resume ordered payload arg",
                                at: at.into(),
                            })
                            .map(|value| (*param_idx, value))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let mut elements: Vec<Option<CgValue<'ctx>>> = vec![None; element_tys.len()];
                for (param_idx, value) in ordered {
                    let slot =
                        elements
                            .get_mut(param_idx)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "Continuation.resume ordered payload slot",
                                at: at.into(),
                            })?;
                    *slot = Some(value);
                }
                let elements = elements
                    .into_iter()
                    .map(|value| {
                        value.ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "Continuation.resume ordered payload",
                            at: at.into(),
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Some(self.build_tuple_cg_value_from_values(
                    at, payload_ty, &elements,
                )?))
            }
            _ => Ok(None),
        }
    }

    fn codegen_continuation_resume_payload_value(
        &mut self,
        at: crate::span::Span,
        payload_ty: TypeId,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if let Some(payload) =
            self.codegen_continuation_resume_legacy_payload_value(Some(payload_ty), args)?
        {
            return Ok(payload);
        }
        if let Some(payload) =
            self.codegen_continuation_resume_expanded_payload_value(at, payload_ty, args)?
        {
            return Ok(payload);
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "Continuation.resume payload surface",
            at: at.into(),
        })
    }

    fn codegen_perform_payload_value(
        &mut self,
        span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.is_empty() {
            return Ok(CgValue::unit());
        }

        let call_info = self.effect_op_call_info_for_span(span)?.cloned();
        if let Some(tuple_ty) = call_info.as_ref().and_then(|info| info.payload_tuple_ty) {
            let info = call_info.ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "perform payload call info",
                at: span.into(),
            })?;
            let TypeKind::Value(ValueTypeKind::Tuple(element_tys)) = self.types.kind(tuple_ty)
            else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "perform payload tuple type",
                    at: span.into(),
                });
            };
            if element_tys.len() != info.arg_mapping.len() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "perform payload tuple mapping",
                    at: span.into(),
                });
            }

            let mut arg_param_mapping: Vec<Option<usize>> = vec![None; args.len()];
            for (param_idx, arg_idx) in info.arg_mapping.iter().copied().enumerate() {
                let Some(slot) = arg_param_mapping.get_mut(arg_idx) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "perform payload arg mapping index",
                        at: span.into(),
                    });
                };
                *slot = Some(param_idx);
            }

            let mut evaluated_args: Vec<Option<CgValue<'ctx>>> = vec![None; args.len()];
            for (arg_idx, arg) in args.iter().enumerate() {
                let param_idx = arg_param_mapping
                    .get(arg_idx)
                    .and_then(|slot| *slot)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "perform payload arg mapping",
                        at: span.into(),
                    })?;
                let expected_ty = self.cg_ty_of(element_tys[param_idx]).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "perform payload expected type",
                        at: span.into(),
                    },
                )?;
                let expr = Self::call_arg_expr(arg);
                let value = self.codegen_expr_in_expected_context(expr, Some(expected_ty))?;
                evaluated_args[arg_idx] = Some(self.coerce_value(expr.span, value, expected_ty)?);
            }

            let ordered = info
                .arg_mapping
                .iter()
                .copied()
                .map(|arg_idx| {
                    evaluated_args.get(arg_idx).and_then(|value| *value).ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "perform payload ordered arg",
                            at: span.into(),
                        },
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            return self.build_tuple_cg_value_from_values(span, tuple_ty, &ordered);
        }

        if args.len() > 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "perform payload tuple side table",
                at: span.into(),
            });
        }

        let expr = Self::call_arg_expr(&args[0]);
        self.codegen_expr_in_expected_context(expr, None)
    }

    pub(super) fn read_perform_slot_payload(
        &mut self,
        at: crate::span::Span,
        target: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let transport = self.read_current_effect_payload_transport(at, "binder_transport")?;
        self.decode_effect_transport_value(at, transport.word, transport.gc_ref, target)
    }

    pub(super) fn extract_tuple_payload_element(
        &mut self,
        at: crate::span::Span,
        tuple_payload: CgValue<'ctx>,
        tuple_ty: TypeId,
        elem_idx: u32,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let elem_ty = self.lookup_tuple_element(tuple_ty, elem_idx, at)?;
        if elem_ty == CgTy::Unit {
            return Ok(CgValue::unit());
        }
        let raw = tuple_payload
            .value
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "perform payload tuple value",
                at: at.into(),
            })?;
        let tuple_v = raw.into_struct_value();
        let extracted = self.builder.build_extract_value(
            tuple_v,
            elem_idx,
            &format!("perform_payload_tuple_elem_{elem_idx}"),
        )?;
        self.cg_value_from_loaded(at, elem_ty, extracted)
    }

    fn emit_effect_set_active_with_trace(
        &mut self,
        span: crate::span::Span,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        let (line, col) = self.effect_trace_line_col(span)?;
        let i32_ty = self.context.i32_type();
        let set_active = self.declare_runtime_effect_set_active_with_trace();
        self.builder.build_call(
            set_active,
            &[
                i32_ty.const_int(line as u64, false).into(),
                i32_ty.const_int(col as u64, false).into(),
            ],
            name,
        )?;
        Ok(())
    }

    fn current_codegen_function(
        &self,
        at: crate::span::Span,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        self.builder
            .get_insert_block()
            .and_then(|bb| bb.get_parent())
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: at.into(),
            })
    }

    pub(super) fn ptr_is_non_null(
        &mut self,
        _at: crate::span::Span,
        ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let raw =
            self.builder
                .build_ptr_to_int(ptr, self.context.i64_type(), &format!("{name}_int"))?;
        Ok(self.builder.build_int_compare(
            inkwell::IntPredicate::NE,
            raw,
            self.context.i64_type().const_zero(),
            name,
        )?)
    }

    fn llvm_callee_suspend_state_prefix_type(&self) -> StructType<'ctx> {
        const NAME: &str = "scoop.runtime.CalleeSuspendStatePrefix";
        if let Some(existing) = self.context.get_struct_type(NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(NAME);
        ty.set_body(
            &[
                self.llvm_gc_object_header_type().into(),
                self.context.i64_type().into(),
                self.llvm_gc_i8_ptr_type().into(),
                self.context.i32_type().into(),
                self.llvm_i8_ptr_type().into(),
            ],
            false,
        );
        ty
    }

    fn current_callee_suspend_state_names(
        &self,
        llvm_fun: FunctionValue<'ctx>,
    ) -> (String, String) {
        let func_name = llvm_fun.get_name().to_str().unwrap_or("anon");
        let func_name_san = sanitize_llvm_ident(func_name);
        (
            format!("scoop.runtime.CalleeSuspendState__{func_name_san}"),
            format!("__scoop_type_desc_callee_suspend__{func_name_san}"),
        )
    }

    fn get_or_create_current_callee_suspend_state_type(
        &mut self,
        at: crate::span::Span,
        plan: &CalleeSuspendPlan,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let llvm_fun = self.current_codegen_function(at)?;
        let (type_name, _) = self.current_callee_suspend_state_names(llvm_fun);
        if let Some(existing) = self.context.get_struct_type(&type_name) {
            return Ok(existing);
        }

        let ty = self.context.opaque_struct_type(&type_name);
        let mut fields: Vec<BasicTypeEnum<'ctx>> = vec![
            self.llvm_gc_object_header_type().into(),
            self.context.i64_type().into(),
            self.llvm_gc_i8_ptr_type().into(),
            self.context.i32_type().into(),
            self.llvm_i8_ptr_type().into(),
        ];
        for local in &plan.saved_locals {
            let cg_ty = self
                .cg_ty_of(local.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "callee suspend local type",
                    at: at.into(),
                })?;
            fields.push(self.llvm_basic_type_of(at, cg_ty)?);
        }
        ty.set_body(&fields, false);
        Ok(ty)
    }

    fn get_or_create_current_callee_suspend_state_type_desc_global(
        &mut self,
        at: crate::span::Span,
        state_ty: StructType<'ctx>,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let llvm_fun = self.current_codegen_function(at)?;
        let (canonical_name, global_name) = self.current_callee_suspend_state_names(llvm_fun);
        let trace_start_offset_bytes = self.target_data.offset_of_element(&state_ty, 2).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "callee suspend trace_start offset",
                at: at.into(),
            },
        )?;
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            canonical_name: &canonical_name,
            obj_ty: state_ty,
            trace_start_offset_bytes,
            parent: None,
            itable: None,
            vtable: None,
        })
    }

    fn load_existing_local_value(
        &mut self,
        at: crate::span::Span,
        local: CgLocal<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match local.ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                let local_ptr = self.local_ptr_for_use(at, local, "callee_suspend_load_slot")?;
                let llvm_ty = self.llvm_basic_type_of(at, local.ty)?;
                let loaded = self
                    .builder
                    .build_load(llvm_ty, local_ptr, "callee_suspend_load")?;
                self.cg_value_from_loaded(at, local.ty, loaded)
            }
        }
    }

    pub(super) fn emit_callee_suspend_state_save(
        &mut self,
        at: crate::span::Span,
        plan: &CalleeSuspendPlan,
        site: &CalleeSuspendResumeSite,
    ) -> Result<(), LlvmEmitError> {
        let state_ty = self.get_or_create_current_callee_suspend_state_type(at, plan)?;
        let state_desc =
            self.get_or_create_current_callee_suspend_state_type_desc_global(at, state_ty)?;
        let total_size = self
            .context
            .i64_type()
            .const_int(self.target_data.get_store_size(&state_ty), false);
        let desc_i8 = self.builder.build_pointer_cast(
            state_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "callee_suspend_state_desc_i8",
        )?;
        let alloc = self.declare_runtime_alloc_typed();
        let state_raw = self
            .builder
            .build_call(
                alloc,
                &[desc_i8.into(), total_size.into()],
                "callee_suspend_state_alloc",
            )?
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "callee suspend state alloc return value",
                at: at.into(),
            })?
            .into_pointer_value();

        let pin = self.declare_runtime_gc_pin();
        self.builder
            .build_call(pin, &[state_raw.into()], "callee_suspend_state_pin")?;

        let state_ptr = self.builder.build_pointer_cast(
            state_raw,
            self.llvm_ptr_type(self.gc_address_space()),
            "callee_suspend_state_ptr",
        )?;
        let resume_word_gep = self.builder.build_struct_gep(
            state_ty,
            state_ptr,
            CALLEE_SUSPEND_STATE_RESUME_WORD_INDEX,
            "callee_suspend_resume_word",
        )?;
        self.builder
            .build_store(resume_word_gep, self.context.i64_type().const_zero())?;
        let resume_gc_ref_gep = self.builder.build_struct_gep(
            state_ty,
            state_ptr,
            CALLEE_SUSPEND_STATE_RESUME_GC_REF_INDEX,
            "callee_suspend_resume_gc_ref",
        )?;
        self.builder
            .build_store(resume_gc_ref_gep, self.llvm_gc_i8_ptr_type().const_null())?;
        let site_tag_gep = self.builder.build_struct_gep(
            state_ty,
            state_ptr,
            CALLEE_SUSPEND_STATE_SITE_TAG_INDEX,
            "callee_suspend_site_tag",
        )?;
        let site_tag = self
            .context
            .i32_type()
            .const_int(site.site_tag() as u64, false);
        self.builder.build_store(site_tag_gep, site_tag)?;
        let resume_entry_fn =
            self.current_callee_resume_entry_fn()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "callee suspend resume entry fn",
                    at: at.into(),
                })?;
        let resume_entry_gep = self.builder.build_struct_gep(
            state_ty,
            state_ptr,
            CALLEE_SUSPEND_STATE_RESUME_ENTRY_FN_INDEX,
            "callee_suspend_resume_entry_fn",
        )?;
        let resume_entry_ptr = self.builder.build_pointer_cast(
            resume_entry_fn.as_global_value().as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "callee_suspend_resume_entry_fn_i8",
        )?;
        self.builder
            .build_store(resume_entry_gep, resume_entry_ptr)?;

        let active_locals = site
            .saved_locals
            .iter()
            .map(|local| local.id)
            .collect::<HashSet<_>>();

        for (index, local_plan) in plan.saved_locals.iter().enumerate() {
            let field_index = CALLEE_SUSPEND_STATE_USER_FIELD_BASE_INDEX + index as u32;
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_ptr,
                field_index,
                &format!("callee_suspend_save_{}", local_plan.id.as_u32()),
            )?;
            let cg_ty = self
                .cg_ty_of(local_plan.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "callee suspend local type",
                    at: at.into(),
                })?;
            let value = if active_locals.contains(&local_plan.id) {
                let local = self.function_cx.env.get(local_plan.id).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "callee suspend local not found",
                        at: at.into(),
                    },
                )?;
                self.load_existing_local_value(at, local)?
            } else {
                self.default_value(at, cg_ty)?
            };
            self.store_local_value(at, field_ptr, cg_ty, value)?;
        }

        let publish = self.declare_runtime_callee_suspend_state_publish();
        self.builder
            .build_call(publish, &[state_raw.into()], "publish_callee_suspend_state")?;
        Ok(())
    }

    pub(super) fn emit_resume_payload_into_callee_suspend_state(
        &mut self,
        at: crate::span::Span,
        state_raw: PointerValue<'ctx>,
        resume_word: IntValue<'ctx>,
        resume_gc_ref: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let prefix_ty = self.llvm_callee_suspend_state_prefix_type();
        let prefix_ptr = self.builder.build_pointer_cast(
            state_raw,
            self.llvm_ptr_type(self.gc_address_space()),
            "callee_suspend_prefix_ptr",
        )?;
        let resume_word_gep = self.builder.build_struct_gep(
            prefix_ty,
            prefix_ptr,
            CALLEE_SUSPEND_STATE_RESUME_WORD_INDEX,
            "callee_suspend_prefix_resume_word",
        )?;
        self.builder.build_store(resume_word_gep, resume_word)?;
        let resume_gc_ref_gep = self.builder.build_struct_gep(
            prefix_ty,
            prefix_ptr,
            CALLEE_SUSPEND_STATE_RESUME_GC_REF_INDEX,
            "callee_suspend_prefix_resume_gc_ref",
        )?;
        self.builder.build_store(resume_gc_ref_gep, resume_gc_ref)?;
        let _ = at;
        Ok(())
    }

    pub(super) fn begin_callee_suspend_resume(
        &mut self,
        at: crate::span::Span,
        plan: &CalleeSuspendPlan,
        state_raw: PointerValue<'ctx>,
    ) -> Result<CalleeSuspendResumeState<'ctx>, LlvmEmitError> {
        let state_ty = self.get_or_create_current_callee_suspend_state_type(at, plan)?;
        let state_ptr = self.builder.build_pointer_cast(
            state_raw,
            self.llvm_ptr_type(self.gc_address_space()),
            "callee_suspend_resume_ptr",
        )?;

        let resume_word_gep = self.builder.build_struct_gep(
            state_ty,
            state_ptr,
            CALLEE_SUSPEND_STATE_RESUME_WORD_INDEX,
            "callee_resume_word_gep",
        )?;
        let resume_word = self
            .builder
            .build_load(
                self.context.i64_type(),
                resume_word_gep,
                "callee_resume_word",
            )?
            .into_int_value();
        let resume_gc_ref_gep = self.builder.build_struct_gep(
            state_ty,
            state_ptr,
            CALLEE_SUSPEND_STATE_RESUME_GC_REF_INDEX,
            "callee_resume_gc_ref_gep",
        )?;
        let resume_gc_ref = self
            .builder
            .build_load(
                self.llvm_gc_i8_ptr_type(),
                resume_gc_ref_gep,
                "callee_resume_gc_ref",
            )?
            .into_pointer_value();

        let site_tag_gep = self.builder.build_struct_gep(
            state_ty,
            state_ptr,
            CALLEE_SUSPEND_STATE_SITE_TAG_INDEX,
            "callee_resume_site_tag_gep",
        )?;
        let site_tag = self
            .builder
            .build_load(
                self.context.i32_type(),
                site_tag_gep,
                "callee_resume_site_tag",
            )?
            .into_int_value();

        Ok(CalleeSuspendResumeState {
            state_ty,
            state_ptr,
            resume_word,
            resume_gc_ref,
            site_tag,
        })
    }

    pub(super) fn load_callee_suspend_resume_entry_fn_ptr(
        &mut self,
        state_raw: PointerValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let prefix_ty = self.llvm_callee_suspend_state_prefix_type();
        let prefix_ptr = self.builder.build_pointer_cast(
            state_raw,
            self.llvm_ptr_type(self.gc_address_space()),
            "callee_suspend_resume_prefix_ptr",
        )?;
        let resume_entry_gep = self.builder.build_struct_gep(
            prefix_ty,
            prefix_ptr,
            CALLEE_SUSPEND_STATE_RESUME_ENTRY_FN_INDEX,
            "callee_suspend_resume_entry_fn_ptr",
        )?;
        Ok(self
            .builder
            .build_load(
                self.llvm_i8_ptr_type(),
                resume_entry_gep,
                "callee_suspend_resume_entry_fn",
            )?
            .into_pointer_value())
    }

    pub(super) fn emit_callee_suspend_resume_site_prologue(
        &mut self,
        at: crate::span::Span,
        plan: &CalleeSuspendPlan,
        site: &CalleeSuspendResumeSite,
        resume_state: CalleeSuspendResumeState<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        for local_plan in &site.saved_locals {
            let cg_ty = self
                .cg_ty_of(local_plan.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "callee resume local type",
                    at: at.into(),
                })?;
            let field_index = plan
                .saved_local_index(local_plan.id)
                .map(|index| CALLEE_SUSPEND_STATE_USER_FIELD_BASE_INDEX + index)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "callee resume local field index",
                    at: at.into(),
                })?;
            let field_ptr = self.builder.build_struct_gep(
                resume_state.state_ty,
                resume_state.state_ptr,
                field_index,
                &format!("callee_resume_local_{}", local_plan.id.as_u32()),
            )?;
            let restored = match cg_ty {
                CgTy::Unit => CgValue::unit(),
                CgTy::Never => CgValue::never(),
                _ => {
                    let llvm_ty = self.llvm_basic_type_of(at, cg_ty)?;
                    let loaded = self.builder.build_load(
                        llvm_ty,
                        field_ptr,
                        &format!("callee_resume_load_{}", local_plan.id.as_u32()),
                    )?;
                    self.cg_value_from_loaded(at, cg_ty, loaded)?
                }
            };
            let name = if local_plan.name.is_empty() {
                format!("resumed_local_{}", local_plan.id.as_u32())
            } else {
                format!("resumed_{}", local_plan.name)
            };
            let ptr = self.create_entry_alloca(at, &name, cg_ty)?;
            self.store_local_value(at, ptr, cg_ty, restored)?;
            self.function_cx.env.insert(
                local_plan.id,
                CgLocal {
                    hir_ty: Some(local_plan.ty),
                    call_may_suspend: self.local_call_may_suspend_from_hir_ty(Some(local_plan.ty)),
                    ty: cg_ty,
                    ptr,
                    mutable: local_plan.mutable,
                },
            );
        }

        let resume_slot_cg_ty =
            self.cg_ty_of(site.resume_slot_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "callee resume slot type",
                    at: at.into(),
                })?;
        let resume_slot_value = self.decode_effect_transport_value(
            at,
            resume_state.resume_word,
            resume_state.resume_gc_ref,
            resume_slot_cg_ty,
        )?;
        let resume_slot_name = if site.resume_slot_name.is_empty() {
            format!("callee_resume_slot_{}", site.resume_slot_id.as_u32())
        } else {
            format!("resumed_{}", site.resume_slot_name)
        };
        let resume_slot_ptr = self.create_entry_alloca(at, &resume_slot_name, resume_slot_cg_ty)?;
        self.store_local_value(at, resume_slot_ptr, resume_slot_cg_ty, resume_slot_value)?;
        self.function_cx.env.insert(
            site.resume_slot_id,
            CgLocal {
                hir_ty: Some(site.resume_slot_ty),
                call_may_suspend: self
                    .local_call_may_suspend_from_hir_ty(Some(site.resume_slot_ty)),
                ty: resume_slot_cg_ty,
                ptr: resume_slot_ptr,
                mutable: false,
            },
        );
        Ok(())
    }

    /// 当前普通 callee frame 观察到 effect 已 active 时，立即把默认返回值交给 caller。
    ///
    /// 这条路径只用于 ordinary frame 的 effect 传播：
    /// - 若存在 function-level return context，则写入默认返回值并 branch 到 return_bb；
    /// - 否则直接发射函数 return（例如 object init 这类没有 return_bb 的内部函数）；
    /// - 对 `Nothing` 返回类型，必须发射 `ret void`，让 caller 观察 TLS active，而不是走
    ///   普通 `return_bb` 的 `unreachable`。
    fn emit_effect_propagation_return(
        &mut self,
        at: crate::span::Span,
    ) -> Result<(), LlvmEmitError> {
        let Some(declared_return_ty) = self.function_cx.current_fun_return_ty else {
            return Ok(());
        };

        if declared_return_ty != CgTy::Never
            && let Some(return_ctx) = self.function_cx.return_context
        {
            let default = self.default_value(at, declared_return_ty)?;
            if let Some(alloca) = return_ctx.return_alloca
                && let Some(raw) = default.value
            {
                self.builder.build_store(alloca, raw)?;
            }
            self.builder
                .build_unconditional_branch(return_ctx.return_bb)?;
            return Ok(());
        }

        match declared_return_ty {
            CgTy::Never => {
                self.builder.build_return(None)?;
                Ok(())
            }
            _ => {
                let default = self.default_value(at, declared_return_ty)?;
                self.emit_return(at, declared_return_ty, default)
            }
        }
    }

    /// 普通 callee frame 中的 direct non-resuming effect 一定会向 caller 传播：
    /// 当前 frame 立刻结束，并把 builder 移到一个无前驱的 dead block，供后续 dead IR 落点。
    pub(super) fn emit_ordinary_non_resuming_effect_exit(
        &mut self,
        at: crate::span::Span,
        label: &str,
    ) -> Result<(), LlvmEmitError> {
        if !self.ordinary_effect_propagation_enabled() {
            return Ok(());
        }

        let current_fn = self.current_codegen_function(at)?;
        let return_bb = self
            .context
            .append_basic_block(current_fn, &format!("{label}_return"));
        let dead_bb = self
            .context
            .append_basic_block(current_fn, &format!("{label}_dead"));

        self.builder.build_unconditional_branch(return_bb)?;
        self.builder.position_at_end(return_bb);
        self.emit_effect_propagation_return(at)?;

        self.builder.position_at_end(dead_bb);
        Ok(())
    }

    /// 普通 call site 在 callee 返回后统一检查 TLS active。
    ///
    /// 若 callee perform 了 non-resuming effect，则当前 frame 直接向 caller 返回默认值；
    /// 否则落到 continue block，后续 IR 只在 inactive 路径上继续生成。
    pub(super) fn emit_ordinary_call_effect_propagation_check(
        &mut self,
        at: crate::span::Span,
        label: &str,
    ) -> Result<(), LlvmEmitError> {
        if !self.ordinary_effect_propagation_enabled() {
            return Ok(());
        }

        let current_fn = self.current_codegen_function(at)?;
        let return_bb = self
            .context
            .append_basic_block(current_fn, &format!("{label}_return"));
        let continue_bb = self
            .context
            .append_basic_block(current_fn, &format!("{label}_continue"));

        let EffectOutcome { is_propagating, .. } =
            self.read_current_effect_outcome_status(at, label)?;

        self.builder
            .build_conditional_branch(is_propagating, return_bb, continue_bb)?;

        self.builder.position_at_end(return_bb);
        self.emit_effect_propagation_return(at)?;

        self.builder.position_at_end(continue_bb);
        Ok(())
    }

    /// 显式 outcome 版 ordinary call boundary：
    ///
    /// - callee/wrapper 已把当前 boundary 的完成/传播结果写入 `outcome_slot`；
    /// - 若结果为 `Propagate`，当前 legacy ordinary function 仍需把该 outcome 回写到 TLS，
    ///   再沿现有“默认返回值 + 交由更外层 caller/wrapper 继续处理”的路径退出；
    /// - 若结果为 `Complete`，后续 IR 只在 continue block 上继续生成。
    pub(super) fn emit_ordinary_call_effect_propagation_check_from_outcome(
        &mut self,
        at: crate::span::Span,
        outcome_slot: PointerValue<'ctx>,
        label: &str,
    ) -> Result<(), LlvmEmitError> {
        if !self.ordinary_effect_propagation_enabled() {
            return Ok(());
        }

        let current_fn = self.current_codegen_function(at)?;
        let return_bb = self
            .context
            .append_basic_block(current_fn, &format!("{label}_return"));
        let continue_bb = self
            .context
            .append_basic_block(current_fn, &format!("{label}_continue"));

        let is_propagating = self.effect_outcome_is_propagating(at, outcome_slot, label)?;

        self.builder
            .build_conditional_branch(is_propagating, return_bb, continue_bb)?;

        self.builder.position_at_end(return_bb);
        self.publish_effect_outcome_from_slot(at, outcome_slot, label)?;
        self.emit_effect_propagation_return(at)?;

        self.builder.position_at_end(continue_bb);
        Ok(())
    }

    /// Lower the builtin `Continuation.resume(...)` call.
    ///
    /// The authoritative semantic marker is `continuation_resume_call_sites`;
    /// codegen does not infer this builtin from member names or receiver
    /// shapes. Payload transport follows the shared effect transport contract:
    /// scalar / word-sized enums flow through `resume_word`, direct GC refs
    /// flow through `resume_gc_ref`, and non-word composite values use a
    /// typed GC box carried by `resume_gc_ref`.
    pub(super) fn codegen_continuation_resume_builtin(
        &mut self,
        span: crate::span::Span,
        callee: &hir::Expr,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
        result_ty: Option<TypeId>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if matches!(expected, Some(CgTy::Unit)) {
            return self.codegen_continuation_resume_builtin_discard_answer(span, callee, args);
        }

        let hir::ExprKind::MemberAccess { receiver, .. } = &callee.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Continuation.resume callee shape",
                at: callee.span.into(),
            });
        };

        let receiver_ty = self
            .resolve_expr_concrete_type(receiver)
            .unwrap_or(receiver.ty);
        let (payload_ty, receiver_answer_ty) = match self.types.kind(receiver_ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.core.Continuation" && nominal.args.len() >= 2 =>
            {
                (Some(nominal.args[0]), Some(nominal.args[1]))
            }
            _ => (None, None),
        };
        let answer_cg_ty = result_ty
            .and_then(|ty| self.cg_ty_of(ty))
            .or_else(|| receiver_answer_ty.and_then(|ty| self.cg_ty_of(ty)))
            .or(expected)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "Continuation.resume answer type",
                at: span.into(),
            })?;

        if self.current_continuation_resume_replay() {
            let replay = self.current_continuation_resume_replay_context().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "Continuation.resume replay context",
                    at: span.into(),
                },
            )?;
            let (out_word_slot, out_gc_ref_slot) =
                self.alloc_continuation_resume_answer_slots(span, answer_cg_ty, "replay")?;
            let outcome_slot =
                self.alloc_effect_outcome_slot(span, "continuation_resume_replay")?;
            let resume_slots = ContinuationResumeResultSlots {
                out_word_slot,
                out_gc_ref_slot,
                outcome_slot,
            };
            self.resume_continuation_with_encoded_payload(
                span,
                replay.token,
                replay.resume_word,
                replay.resume_gc_ref,
                resume_slots,
                "continuation_resume_replay",
            )?;
            self.maybe_record_active_suspend_site_effect_outcome(span, resume_slots.outcome_slot);
            self.emit_ordinary_call_effect_propagation_check_from_outcome(
                span,
                resume_slots.outcome_slot,
                "continuation_resume_replay_effect",
            )?;
            return self.load_continuation_resume_answer_with_active_fallback(
                span,
                answer_cg_ty,
                resume_slots.out_word_slot,
                resume_slots.out_gc_ref_slot,
                resume_slots.outcome_slot,
                "replay",
            );
        }

        let continuation = self.codegen_expr_in_expected_context(receiver, Some(CgTy::Ref))?;
        let continuation = self.coerce_value(receiver.span, continuation, CgTy::Ref)?;
        let deferred_continuation = self.defer_gc_sensitive_cg_value(
            receiver.span,
            "continuation_resume_receiver",
            continuation,
        )?;

        // `Continuation.resume(...)` 的 authoritative payload / answer type 都来自
        // receiver 的 `Continuation<Resume, Answer, eff E>` 实参。
        let payload = if let Some(payload_ty) = payload_ty {
            self.codegen_continuation_resume_payload_value(span, payload_ty, args)?
        } else {
            self.codegen_continuation_resume_legacy_payload_value(None, args)?
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "Continuation.resume payload type",
                    at: receiver.span.into(),
                })?
        };
        let (out_word_slot, out_gc_ref_slot) =
            self.alloc_continuation_resume_answer_slots(span, answer_cg_ty, "fresh")?;
        let outcome_slot = self.alloc_effect_outcome_slot(span, "continuation_resume")?;
        let resume_slots = ContinuationResumeResultSlots {
            out_word_slot,
            out_gc_ref_slot,
            outcome_slot,
        };
        let continuation = self.materialize_deferred_cg_value(
            receiver.span,
            "continuation_resume_receiver_reload",
            deferred_continuation,
        )?;
        let Some(raw_continuation) = continuation.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Continuation.resume receiver value",
                at: receiver.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(cont_ptr) = raw_continuation else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Continuation.resume receiver type",
                at: receiver.span.into(),
            });
        };
        self.clear_continuation_resume_receiver_local_if_any(receiver)?;
        self.resume_continuation_with_payload(
            span,
            cont_ptr,
            payload,
            resume_slots,
            "continuation_resume",
        )?;

        self.maybe_record_active_suspend_site_effect_outcome(span, resume_slots.outcome_slot);
        self.emit_ordinary_call_effect_propagation_check_from_outcome(
            span,
            resume_slots.outcome_slot,
            "continuation_resume_effect",
        )?;
        self.load_continuation_resume_answer_with_active_fallback(
            span,
            answer_cg_ty,
            resume_slots.out_word_slot,
            resume_slots.out_gc_ref_slot,
            resume_slots.outcome_slot,
            "fresh",
        )
    }

    fn codegen_continuation_resume_builtin_discard_answer(
        &mut self,
        span: crate::span::Span,
        callee: &hir::Expr,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if self.current_continuation_resume_replay() {
            let replay = self.current_continuation_resume_replay_context().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "Continuation.resume replay context",
                    at: span.into(),
                },
            )?;
            let null_slot = self.context.ptr_type(AddressSpace::default()).const_null();
            let outcome_slot =
                self.alloc_effect_outcome_slot(span, "continuation_resume_replay")?;
            let resume_slots = ContinuationResumeResultSlots {
                out_word_slot: null_slot,
                out_gc_ref_slot: null_slot,
                outcome_slot,
            };
            self.resume_continuation_with_encoded_payload(
                span,
                replay.token,
                replay.resume_word,
                replay.resume_gc_ref,
                resume_slots,
                "continuation_resume_replay",
            )?;
            self.maybe_record_active_suspend_site_effect_outcome(span, resume_slots.outcome_slot);
            self.emit_ordinary_call_effect_propagation_check_from_outcome(
                span,
                resume_slots.outcome_slot,
                "continuation_resume_replay_effect",
            )?;
            return Ok(CgValue::unit());
        }

        let hir::ExprKind::MemberAccess { receiver, .. } = &callee.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Continuation.resume callee shape",
                at: callee.span.into(),
            });
        };

        let continuation = self.codegen_expr_in_expected_context(receiver, Some(CgTy::Ref))?;
        let continuation = self.coerce_value(receiver.span, continuation, CgTy::Ref)?;
        let deferred_continuation = self.defer_gc_sensitive_cg_value(
            receiver.span,
            "continuation_resume_receiver",
            continuation,
        )?;

        let receiver_ty = self
            .resolve_expr_concrete_type(receiver)
            .unwrap_or(receiver.ty);
        let payload_ty = match self.types.kind(receiver_ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.core.Continuation" && !nominal.args.is_empty() =>
            {
                Some(nominal.args[0])
            }
            _ => None,
        };
        let payload = if let Some(payload_ty) = payload_ty {
            self.codegen_continuation_resume_payload_value(span, payload_ty, args)?
        } else {
            self.codegen_continuation_resume_legacy_payload_value(None, args)?
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "Continuation.resume payload type",
                    at: receiver.span.into(),
                })?
        };
        let null_slot = self.context.ptr_type(AddressSpace::default()).const_null();
        let outcome_slot = self.alloc_effect_outcome_slot(span, "continuation_resume")?;
        let resume_slots = ContinuationResumeResultSlots {
            out_word_slot: null_slot,
            out_gc_ref_slot: null_slot,
            outcome_slot,
        };
        let continuation = self.materialize_deferred_cg_value(
            receiver.span,
            "continuation_resume_receiver_reload",
            deferred_continuation,
        )?;
        let Some(raw_continuation) = continuation.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Continuation.resume receiver value",
                at: receiver.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(cont_ptr) = raw_continuation else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Continuation.resume receiver type",
                at: receiver.span.into(),
            });
        };
        self.clear_continuation_resume_receiver_local_if_any(receiver)?;
        self.resume_continuation_with_payload(
            span,
            cont_ptr,
            payload,
            resume_slots,
            "continuation_resume",
        )?;

        self.maybe_record_active_suspend_site_effect_outcome(span, resume_slots.outcome_slot);
        self.emit_ordinary_call_effect_propagation_check_from_outcome(
            span,
            resume_slots.outcome_slot,
            "continuation_resume_effect",
        )?;
        Ok(CgValue::unit())
    }

    fn alloc_continuation_resume_answer_slots(
        &mut self,
        span: crate::span::Span,
        answer_cg_ty: CgTy,
        suffix: &str,
    ) -> Result<(PointerValue<'ctx>, PointerValue<'ctx>), LlvmEmitError> {
        if matches!(answer_cg_ty, CgTy::Unit | CgTy::Never) {
            let null_slot = self.context.ptr_type(AddressSpace::default()).const_null();
            return Ok((null_slot, null_slot));
        }

        let out_word_slot = self.create_entry_alloca_raw(
            span,
            &format!("continuation_resume_out_word_{suffix}"),
            self.context.i64_type().into(),
        )?;
        self.builder
            .build_store(out_word_slot, self.context.i64_type().const_zero())?;
        let out_gc_ref_slot = self.create_entry_alloca_raw(
            span,
            &format!("continuation_resume_out_gc_ref_{suffix}"),
            self.llvm_gc_i8_ptr_type().into(),
        )?;
        self.builder
            .build_store(out_gc_ref_slot, self.llvm_gc_i8_ptr_type().const_null())?;
        Ok((out_word_slot, out_gc_ref_slot))
    }

    fn load_continuation_resume_answer(
        &mut self,
        span: crate::span::Span,
        answer_cg_ty: CgTy,
        out_word_slot: PointerValue<'ctx>,
        out_gc_ref_slot: PointerValue<'ctx>,
        suffix: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match answer_cg_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            target => {
                let loaded_word = self
                    .builder
                    .build_load(
                        self.context.i64_type(),
                        out_word_slot,
                        &format!("continuation_resume_word_{suffix}"),
                    )?
                    .into_int_value();
                let loaded_gc_ref = self
                    .builder
                    .build_load(
                        self.llvm_gc_i8_ptr_type(),
                        out_gc_ref_slot,
                        &format!("continuation_resume_gc_ref_{suffix}"),
                    )?
                    .into_pointer_value();
                self.decode_effect_transport_value(span, loaded_word, loaded_gc_ref, target)
            }
        }
    }

    fn load_continuation_resume_answer_with_active_fallback(
        &mut self,
        span: crate::span::Span,
        answer_cg_ty: CgTy,
        out_word_slot: PointerValue<'ctx>,
        out_gc_ref_slot: PointerValue<'ctx>,
        outcome_slot: PointerValue<'ctx>,
        suffix: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if self.ordinary_effect_propagation_enabled()
            || matches!(answer_cg_ty, CgTy::Unit | CgTy::Never)
        {
            return self.load_continuation_resume_answer(
                span,
                answer_cg_ty,
                out_word_slot,
                out_gc_ref_slot,
                suffix,
            );
        }

        let current_fn = self.current_codegen_function(span)?;
        let active_bb = self
            .context
            .append_basic_block(current_fn, &format!("continuation_resume_{suffix}_active"));
        let inactive_bb = self.context.append_basic_block(
            current_fn,
            &format!("continuation_resume_{suffix}_inactive"),
        );
        let merge_bb = self
            .context
            .append_basic_block(current_fn, &format!("continuation_resume_{suffix}_merge"));

        let is_propagating = self.effect_outcome_is_propagating(
            span,
            outcome_slot,
            &format!("continuation_resume_{suffix}"),
        )?;
        self.builder
            .build_conditional_branch(is_propagating, active_bb, inactive_bb)?;

        let result_basic_ty = self.llvm_basic_type_of(span, answer_cg_ty)?;
        let result_slot = self.create_entry_alloca_raw(
            span,
            &format!("continuation_resume_result_{suffix}"),
            result_basic_ty,
        )?;

        self.builder.position_at_end(active_bb);
        let default = self.default_value(span, answer_cg_ty)?;
        let default_raw = default.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "continuation resume active fallback value",
            at: span.into(),
        })?;
        self.builder.build_store(result_slot, default_raw)?;
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(inactive_bb);
        let decoded = self.load_continuation_resume_answer(
            span,
            answer_cg_ty,
            out_word_slot,
            out_gc_ref_slot,
            suffix,
        )?;
        let decoded_raw = decoded.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "continuation resume inactive decoded value",
            at: span.into(),
        })?;
        self.builder.build_store(result_slot, decoded_raw)?;
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(merge_bb);
        let merged = self.builder.build_load(
            result_basic_ty,
            result_slot,
            &format!("continuation_resume_result_{suffix}_merged"),
        )?;
        self.cg_value_from_loaded(span, answer_cg_ty, merged)
    }

    /// Emit code for a standalone `perform` expression (outside of a state
    /// machine step function).  Writes the op_tag + payload to the TLS
    /// perform slot, records the source line/col in the activation hook, and
    /// returns a default value.
    /// The caller's state machine (via SuspendCall + Suspend terminator) will
    /// detect the active flag and handle dispatch.
    pub(super) fn codegen_perform_expr(
        &mut self,
        span: crate::span::Span,
        effect_ty: TypeId,
        op: &hir::EffectOpRef,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let op_tag = self.effect_op_tag(&op.fqn);
        let op_tag_val = self.context.i32_type().const_int(op_tag as u64, false);
        let effect_instance_key =
            self.effect_instance_key(effect_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect instance key",
                    at: span.into(),
                })?;
        let effect_instance_key_val = self
            .context
            .i32_type()
            .const_int(effect_instance_key as u64, false);

        let payload_val = self.codegen_perform_payload_value(span, args)?;

        // Shared effect transport: word / direct GC ref / boxed composite all
        // collapse to the runtime's `(word0, gc_ref)` perform-slot ABI.
        let (word, gc_ref) = self.encode_effect_transport_value(span, payload_val)?;
        let payload = self.build_value_transport(word, gc_ref);
        let signal = self.build_effect_signal(
            op_tag_val,
            effect_instance_key_val,
            payload,
            self.null_effect_resume_token(),
        );
        // Record the originating perform-site before propagation so outer
        // call-boundary suspend sites can preserve the original trace.
        self.emit_current_effect_propagation_with_trace(
            span,
            signal,
            "effect_set_active_with_trace",
        )?;

        if self.ordinary_effect_propagation_enabled() {
            if let Some(plan) = self.current_callee_suspend_plan().cloned() {
                let site =
                    plan.resume_site_for_span(span)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "callee suspend resume site for perform",
                            at: span.into(),
                        })?;
                self.emit_callee_suspend_state_save(span, &plan, site)?;
            }
            self.emit_ordinary_non_resuming_effect_exit(span, "effect_perform")?;
            // After emitting the early-return edge, the builder continues in a
            // dead block so enclosing expression codegen can finish structurally.
            // Feed that dead path a correctly typed dummy value instead of
            // `Never`, otherwise containers like `perform(...) + 1` still fail
            // before the resumed-body path gets a chance to take over.
            return self.default_value(span, expected.unwrap_or(CgTy::Unit));
        }

        // Return a default value for the expected type.  The actual resume
        // value will be provided by the handler; this default propagates
        // through intermediate frames until the state machine catches it.
        let result_ty = expected.unwrap_or(CgTy::Unit);
        self.default_value(span, result_ty)
    }

    pub(super) fn codegen_handle_expr(
        &mut self,
        span: crate::span::Span,
        handle: &hir::HandleExpr,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_handle_expr_via_state_machine(span, handle, expected)
    }

    /// Emit code to raise a runtime error variant through the effect system.
    /// Writes the `Raise.raise` op_tag to the TLS perform slot, records the
    /// source line/col in the activation hook, and returns.  The caller is
    /// responsible for subsequent control flow (dead block / unreachable).
    pub(super) fn emit_raise_runtime_error_variant(
        &mut self,
        span: crate::span::Span,
        variant: &str,
    ) -> Result<(), LlvmEmitError> {
        // Use the well-known Raise.raise FQN (op_tag = 1 by convention).
        let op_tag = self.effect_op_tag("scoop.core.Raise.raise");
        let op_tag_val = self.context.i32_type().const_int(op_tag as u64, false);
        let effect_instance_key_val = self
            .context
            .i32_type()
            .const_int(EFFECT_INSTANCE_KEY_RAISE_RUNTIME_ERROR as u64, false);

        // Synthesize the concrete `RuntimeError.Variant` enum value, then
        // reuse the shared effect transport encoding so synthesized runtime
        // errors and ordinary `Raise.raise(RuntimeError.X)` share one payload
        // contract.
        let variant_fqn = format!("scoop.core.RuntimeError.{variant}");
        let payload = self
            .try_codegen_qualified_enum_unit_variant_value(span, &variant_fqn)?
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "runtime error enum unit variant value",
                at: span.into(),
            })?;
        let (word, gc_ref) = self.encode_effect_transport_value(span, payload)?;
        let signal = self.build_effect_signal(
            op_tag_val,
            effect_instance_key_val,
            self.build_value_transport(word, gc_ref),
            self.null_effect_resume_token(),
        );
        self.emit_current_effect_propagation_with_trace(
            span,
            signal,
            "raise_set_active_with_trace",
        )?;

        Ok(())
    }

    pub(super) fn coerce_u64_word(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let i64_ty = self.context.i64_type();
        match value.ty {
            CgTy::Unit | CgTy::Never => Ok(i64_ty.const_int(0, false)),
            CgTy::Bool => {
                let b = value.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "u64 word from bool",
                    at: at.into(),
                })?;
                Ok(self.builder.build_int_z_extend(b, i64_ty, "bool_to_u64")?)
            }
            CgTy::Int(_) => {
                let (raw, from) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "u64 word from int",
                    at: at.into(),
                })?;
                let to = IntTy {
                    bits: 64,
                    signed: false,
                };
                Ok(self.cast_int(raw, from, to)?)
            }
            CgTy::Float64 => {
                let (raw, _) = value.as_float().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "u64 word from float64",
                    at: at.into(),
                })?;
                Ok(self
                    .builder
                    .build_bit_cast(raw, i64_ty, "f64_to_u64_bits")?
                    .into_int_value())
            }
            CgTy::Float32 => {
                let (raw, _) = value.as_float().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "u64 word from float32",
                    at: at.into(),
                })?;
                let bits32 = self
                    .builder
                    .build_bit_cast(raw, self.context.i32_type(), "f32_to_u32_bits")?
                    .into_int_value();
                Ok(self
                    .builder
                    .build_int_z_extend(bits32, i64_ty, "u32_to_u64_bits")?)
            }
            CgTy::String | CgTy::Ref => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "u64 word from gc pointer (ptr<->int is forbidden)",
                at: at.into(),
            }),
            CgTy::Enum(enum_ty) => {
                // Value-only enums are plain integers — zero-extend to u64.
                let layout = self.cg_enum_layout(at, enum_ty)?;
                match layout.repr {
                    CgEnumRepr::ValueOnly { underlying } => {
                        let raw = value
                            .value
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "u64 word from enum (no value)",
                                at: at.into(),
                            })?
                            .into_int_value();
                        let to = IntTy {
                            bits: 64,
                            signed: false,
                        };
                        Ok(self.cast_int(raw, underlying, to)?)
                    }
                    CgEnumRepr::TaggedUnion => {
                        // Extract tag (field 0) from { tag, payload_word, payload_ptr }.
                        let raw = value.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "u64 word from enum (no value)",
                            at: at.into(),
                        })?;
                        let tag = self
                            .builder
                            .build_extract_value(raw.into_struct_value(), 0, "enum_tag")?
                            .into_int_value();
                        Ok(self
                            .builder
                            .build_int_z_extend(tag, i64_ty, "enum_tag_u64")?)
                    }
                    CgEnumRepr::Niche {
                        storage: crate::ty::layout::NicheStorage::U8,
                        ..
                    } => {
                        let raw = value.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "u64 word from niche enum (no value)",
                            at: at.into(),
                        })?;
                        let BasicValueEnum::IntValue(raw) = raw else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "u64 word from niche enum (u8)",
                                at: at.into(),
                            });
                        };
                        Ok(self
                            .builder
                            .build_int_z_extend(raw, i64_ty, "niche_u8_to_u64")?)
                    }
                    _ => Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "u64 word from niche enum (not yet supported)",
                        at: at.into(),
                    }),
                }
            }
            CgTy::Tuple(_) | CgTy::Struct(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "u64 word from composite value",
                at: at.into(),
            }),
        }
    }

    pub(super) fn narrow_u64_word_to_cg_value(
        &mut self,
        span: crate::span::Span,
        word: IntValue<'ctx>,
        target: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match target {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            CgTy::Bool => {
                let trunc = self.builder.build_int_truncate(
                    word,
                    self.context.bool_type(),
                    "u64_to_bool",
                )?;
                Ok(CgValue::bool(trunc))
            }
            CgTy::Int(int_ty) => {
                let from = IntTy {
                    bits: 64,
                    signed: false,
                };
                let narrowed = self.cast_int(word, from, int_ty)?;
                Ok(CgValue::int(narrowed, int_ty))
            }
            CgTy::Float64 => {
                let f64_ty = self.context.f64_type();
                let bits = self
                    .builder
                    .build_bit_cast(word, f64_ty, "u64_to_f64")?
                    .into_float_value();
                Ok(CgValue::float(bits, CgTy::Float64))
            }
            CgTy::Float32 => {
                let i32_trunc =
                    self.builder
                        .build_int_truncate(word, self.context.i32_type(), "u64_to_u32")?;
                let f32_ty = self.context.f32_type();
                let bits = self
                    .builder
                    .build_bit_cast(i32_trunc, f32_ty, "u32_to_f32")?
                    .into_float_value();
                Ok(CgValue::float(bits, CgTy::Float32))
            }
            CgTy::String | CgTy::Ref => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "narrow u64 to gc ref",
                at: span.into(),
            }),
            CgTy::Enum(enum_ty) => {
                let layout = self.cg_enum_layout(span, enum_ty)?;
                match layout.repr {
                    CgEnumRepr::ValueOnly { underlying } => {
                        let narrowed = self.cast_int(
                            word,
                            IntTy {
                                bits: 64,
                                signed: false,
                            },
                            underlying,
                        )?;
                        Ok(CgValue {
                            ty: CgTy::Enum(enum_ty),
                            value: Some(narrowed.into()),
                        })
                    }
                    CgEnumRepr::TaggedUnion => {
                        // Fieldless tagged-union enums continue to use tag-only
                        // word transport. Rich tagged unions are routed through
                        // boxed transport before reaching this helper.
                        let llvm_ty = self.llvm_enum_value_type(span, enum_ty)?.into_struct_type();
                        let tag_i32 = self.builder.build_int_truncate(
                            word,
                            self.context.i32_type(),
                            "enum_tag_from_u64",
                        )?;
                        let payload_word_ty = self.int_type(self.enum_payload_ty());
                        let payload_ptr_ty = self.llvm_gc_i8_ptr_type();

                        let mut agg: AggregateValueEnum<'_> = llvm_ty.get_undef().into();
                        agg = self
                            .builder
                            .build_insert_value(agg, tag_i32, 0, "enum_tag")?;
                        agg = self.builder.build_insert_value(
                            agg,
                            payload_word_ty.const_int(0, false),
                            1,
                            "enum_payload_word",
                        )?;
                        agg = self.builder.build_insert_value(
                            agg,
                            payload_ptr_ty.const_null(),
                            2,
                            "enum_payload_ptr",
                        )?;
                        Ok(CgValue {
                            ty: CgTy::Enum(enum_ty),
                            value: Some(agg.as_basic_value_enum()),
                        })
                    }
                    CgEnumRepr::Niche {
                        storage: crate::ty::layout::NicheStorage::U8,
                        ..
                    } => {
                        let narrowed = self.builder.build_int_truncate(
                            word,
                            self.context.i8_type(),
                            "u64_to_niche_u8",
                        )?;
                        Ok(CgValue {
                            ty: CgTy::Enum(enum_ty),
                            value: Some(narrowed.into()),
                        })
                    }
                    _ => Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "narrow u64 to niche enum (not yet supported)",
                        at: span.into(),
                    }),
                }
            }
            CgTy::Tuple(_) | CgTy::Struct(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "narrow u64 to composite type (not yet supported)",
                at: span.into(),
            }),
        }
    }

    fn effect_transport_kind(
        &mut self,
        at: crate::span::Span,
        cg_ty: CgTy,
    ) -> Result<EffectTransportKind, LlvmEmitError> {
        Ok(match cg_ty {
            CgTy::Unit
            | CgTy::Never
            | CgTy::Bool
            | CgTy::Float64
            | CgTy::Float32
            | CgTy::Int(_) => EffectTransportKind::Word,
            CgTy::String | CgTy::Ref => EffectTransportKind::GcRef,
            CgTy::Tuple(tuple_ty) => {
                if self.is_task_transport_tuple_ty(tuple_ty) {
                    EffectTransportKind::TaskTransportTuple
                } else {
                    EffectTransportKind::BoxedComposite
                }
            }
            CgTy::Struct(_) => EffectTransportKind::BoxedComposite,
            CgTy::Enum(enum_ty) => {
                let layout = self.cg_enum_layout(at, enum_ty)?;
                match layout.repr {
                    CgEnumRepr::ValueOnly { .. } => EffectTransportKind::Word,
                    CgEnumRepr::Niche {
                        storage: crate::ty::layout::NicheStorage::U8,
                        ..
                    } => EffectTransportKind::Word,
                    CgEnumRepr::Niche {
                        storage: crate::ty::layout::NicheStorage::Pointer,
                        ..
                    } => EffectTransportKind::GcRef,
                    CgEnumRepr::TaggedUnion => {
                        if layout
                            .variants
                            .iter()
                            .any(|variant| !variant.fields.is_empty())
                        {
                            EffectTransportKind::BoxedComposite
                        } else {
                            EffectTransportKind::Word
                        }
                    }
                }
            }
        })
    }

    pub(super) fn is_task_transport_tuple_ty(&self, ty: TypeId) -> bool {
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = self.types.kind(ty) else {
            return false;
        };
        matches!(
            elements.as_slice(),
            [word_ty, gc_ref_ty] if *word_ty == self.builtins.int && *gc_ref_ty == self.builtins.any
        )
    }

    pub(super) fn build_task_transport_tuple_value(
        &mut self,
        at: crate::span::Span,
        tuple_ty: TypeId,
        word: IntValue<'ctx>,
        gc_ref: PointerValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let word_cg =
            self.cg_ty_of(self.builtins.int)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "task transport tuple word type",
                    at: at.into(),
                })?;
        let word_value = self.narrow_u64_word_to_cg_value(at, word, word_cg)?;
        let gc_ref_value = CgValue {
            ty: CgTy::Ref,
            value: Some(gc_ref.into()),
        };
        self.build_tuple_cg_value_from_values(at, tuple_ty, &[word_value, gc_ref_value])
    }

    pub(super) fn split_task_transport_tuple_value(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
    ) -> Result<(IntValue<'ctx>, PointerValue<'ctx>), LlvmEmitError> {
        let CgTy::Tuple(tuple_ty) = value.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task transport tuple value",
                at: at.into(),
            });
        };
        if !self.is_task_transport_tuple_ty(tuple_ty) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task transport tuple type",
                at: at.into(),
            });
        }

        let word_value = self.extract_tuple_payload_element(at, value, tuple_ty, 0)?;
        let gc_ref_value = self.extract_tuple_payload_element(at, value, tuple_ty, 1)?;
        let word = self.coerce_u64_word(at, word_value)?;
        let gc_ref = gc_ref_value
            .value
            .map(BasicValueEnum::into_pointer_value)
            .unwrap_or_else(|| self.llvm_gc_i8_ptr_type().const_null());
        Ok((word, gc_ref))
    }

    fn effect_transport_box_identity(
        &self,
        at: crate::span::Span,
        cg_ty: CgTy,
    ) -> Result<(String, String, String), LlvmEmitError> {
        let (kind, type_id, display) = match cg_ty {
            CgTy::Tuple(type_id) => ("tuple", type_id, self.types.display(type_id).to_string()),
            CgTy::Struct(type_id) => ("struct", type_id, self.types.display(type_id).to_string()),
            CgTy::Enum(type_id) => ("enum", type_id, self.types.display(type_id).to_string()),
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect transport box identity",
                    at: at.into(),
                });
            }
        };
        let suffix = format!(
            "{}_{}_{}",
            kind,
            type_id.as_u32(),
            sanitize_llvm_ident(&display)
        );
        Ok((
            format!("scoop.runtime.EffectValueBox__{suffix}"),
            format!("__scoop_type_desc_runtime__effect_value_box__{suffix}"),
            format!("scoop.runtime.EffectValueBox<{display}>"),
        ))
    }

    fn llvm_effect_transport_box_object_type(
        &mut self,
        at: crate::span::Span,
        cg_ty: CgTy,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let (type_name, _, _) = self.effect_transport_box_identity(at, cg_ty)?;
        if let Some(existing) = self.context.get_struct_type(&type_name) {
            return Ok(existing);
        }

        let payload_ty = self.llvm_basic_type_of(at, cg_ty)?;
        let ty = self.context.opaque_struct_type(&type_name);
        let header_ty = self.llvm_gc_object_header_type();
        ty.set_body(&[header_ty.into(), payload_ty], false);
        Ok(ty)
    }

    fn get_or_create_effect_transport_box_type_desc_global(
        &mut self,
        at: crate::span::Span,
        cg_ty: CgTy,
        obj_ty: StructType<'ctx>,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let (_, global_name, canonical_name) = self.effect_transport_box_identity(at, cg_ty)?;
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(existing);
        }

        let trace_start_offset_bytes = self.target_data.offset_of_element(&obj_ty, 1).unwrap_or(0);
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            canonical_name: &canonical_name,
            obj_ty,
            trace_start_offset_bytes,
            parent: None,
            itable: None,
            vtable: None,
        })
    }

    fn box_effect_transport_value(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let value_ty = value.ty;
        let deferred = self.defer_gc_sensitive_cg_value(at, "effect_transport_box_value", value)?;
        let obj_ty = self.llvm_effect_transport_box_object_type(at, value_ty)?;
        let obj_size_bytes = self.target_data.get_store_size(&obj_ty);
        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);
        let desc =
            self.get_or_create_effect_transport_box_type_desc_global(at, value_ty, obj_ty)?;
        let desc_i8 = self.builder.build_pointer_cast(
            desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "effect_value_box_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            at,
            rt_alloc,
            &[desc_i8.into(), size_v.into()],
            "rt_alloc_effect_value_box",
        )?;
        let raw_ptr =
            call.try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "scoop_alloc_typed return value (effect value box)",
                    at: at.into(),
                })?;
        let BasicValueEnum::PointerValue(raw_ptr) = raw_ptr else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return type (effect value box)",
                at: at.into(),
            });
        };

        let obj_ptr = self.builder.build_pointer_cast(
            raw_ptr,
            self.llvm_ptr_type(self.gc_address_space()),
            "effect_value_box_obj_ptr",
        )?;
        let deferred_box = self.defer_gc_ref_pointer(at, "effect_transport_box_obj", obj_ptr)?;
        let materialized =
            self.materialize_deferred_cg_value(at, "effect_transport_box_value_reload", deferred)?;
        let raw = materialized
            .value
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect transport boxed payload value",
                at: at.into(),
            })?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            at,
            "effect_transport_box_obj_reload",
            &deferred_box,
        )?;
        let obj_ptr = self.builder.build_pointer_cast(
            obj_ptr,
            self.llvm_ptr_type(self.gc_address_space()),
            "effect_value_box_obj_ptr_reload",
        )?;
        let payload_gep =
            self.builder
                .build_struct_gep(obj_ty, obj_ptr, 1, "effect_value_box_payload_gep")?;
        let _ = self.store_local_value_exact(
            at,
            payload_gep,
            materialized.ty,
            CgValue {
                ty: materialized.ty,
                value: Some(raw),
            },
        )?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            at,
            "effect_transport_box_obj_return",
            &deferred_box,
        )?;
        Ok(self.builder.build_pointer_cast(
            obj_ptr,
            self.llvm_gc_i8_ptr_type(),
            "effect_value_box_as_gc_i8",
        )?)
    }

    fn unbox_effect_transport_value(
        &mut self,
        at: crate::span::Span,
        boxed_ref: PointerValue<'ctx>,
        target: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let obj_ty = self.llvm_effect_transport_box_object_type(at, target)?;
        let obj_ptr = self.builder.build_pointer_cast(
            boxed_ref,
            self.llvm_ptr_type(self.gc_address_space()),
            "effect_value_box_obj_ptr",
        )?;
        let payload_gep =
            self.builder
                .build_struct_gep(obj_ty, obj_ptr, 1, "effect_value_box_payload_gep")?;
        let payload_ty = self.llvm_basic_type_of(at, target)?;
        let loaded =
            self.builder
                .build_load(payload_ty, payload_gep, "effect_value_box_payload")?;
        self.cg_value_from_loaded(at, target, loaded)
    }

    pub(super) fn encode_effect_transport_value(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
    ) -> Result<(IntValue<'ctx>, PointerValue<'ctx>), LlvmEmitError> {
        let zero_word = self.context.i64_type().const_zero();
        let null_gc_ref = self.llvm_gc_i8_ptr_type().const_null();
        match self.effect_transport_kind(at, value.ty)? {
            EffectTransportKind::Word => Ok((self.coerce_u64_word(at, value)?, null_gc_ref)),
            EffectTransportKind::GcRef => {
                let gc_ref = match value.value {
                    Some(raw) => raw.into_pointer_value(),
                    None => null_gc_ref,
                };
                Ok((zero_word, gc_ref))
            }
            EffectTransportKind::TaskTransportTuple => {
                self.split_task_transport_tuple_value(at, value)
            }
            EffectTransportKind::BoxedComposite => {
                let boxed = self.box_effect_transport_value(at, value)?;
                Ok((zero_word, boxed))
            }
        }
    }

    pub(super) fn decode_effect_transport_value(
        &mut self,
        at: crate::span::Span,
        word: IntValue<'ctx>,
        gc_ref: PointerValue<'ctx>,
        target: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match target {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => match self.effect_transport_kind(at, target)? {
                EffectTransportKind::Word => self.narrow_u64_word_to_cg_value(at, word, target),
                EffectTransportKind::GcRef => self.cg_value_from_loaded(at, target, gc_ref.into()),
                EffectTransportKind::TaskTransportTuple => {
                    let CgTy::Tuple(tuple_ty) = target else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "task transport tuple decode target",
                            at: at.into(),
                        });
                    };
                    self.build_task_transport_tuple_value(at, tuple_ty, word, gc_ref)
                }
                EffectTransportKind::BoxedComposite => {
                    self.unbox_effect_transport_value(at, gc_ref, target)
                }
            },
        }
    }

    /// 通过共享 continuation runtime helper 提供 resume payload，并可选接收 delimiter answer。
    pub(super) fn resume_continuation_with_payload(
        &mut self,
        span: crate::span::Span,
        cont_ptr: PointerValue<'ctx>,
        val: CgValue<'ctx>,
        result_slots: ContinuationResumeResultSlots<'ctx>,
        call_name: &str,
    ) -> Result<(), LlvmEmitError> {
        let (word, gc_ref) = self.encode_effect_transport_value(span, val)?;
        self.resume_continuation_with_encoded_payload(
            span,
            cont_ptr,
            word,
            gc_ref,
            result_slots,
            call_name,
        )
    }

    pub(super) fn resume_continuation_with_encoded_payload(
        &mut self,
        span: crate::span::Span,
        cont_ptr: PointerValue<'ctx>,
        word: IntValue<'ctx>,
        gc_ref: PointerValue<'ctx>,
        result_slots: ContinuationResumeResultSlots<'ctx>,
        call_name: &str,
    ) -> Result<(), LlvmEmitError> {
        let resume_fn = self.declare_runtime_continuation_resume_with();
        let args = [
            cont_ptr.into(),
            word.into(),
            gc_ref.into(),
            result_slots.out_word_slot.into(),
            result_slots.out_gc_ref_slot.into(),
            result_slots.outcome_slot.into(),
        ];
        let _ = self.build_call_preserving_gc_local_roots(span, resume_fn, &args, call_name)?;
        Ok(())
    }

    fn effect_intrinsic_word_int_ty(&self) -> IntTy {
        IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        }
    }

    fn codegen_sysroot_effect_intrinsic_word_arg(
        &mut self,
        span: crate::span::Span,
        arg: &hir::CallArg,
        kind: &'static str,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let hir::CallArg::Positional(expr) = arg else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: span.into(),
            });
        };

        let word_ty = self.effect_intrinsic_word_int_ty();
        let value = self.codegen_expr_in_expected_context(expr, Some(CgTy::Int(word_ty)))?;
        let value = self.coerce_value(expr.span, value, CgTy::Int(word_ty))?;
        let (raw, _) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind,
            at: expr.span.into(),
        })?;
        Ok(raw)
    }

    pub(super) fn codegen_sysroot_effect_intrinsics(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let word_ty = self.effect_intrinsic_word_int_ty();
        let op_tag_ty = IntTy {
            bits: 32,
            signed: false,
        };
        let slot_word_ty = IntTy {
            bits: 64,
            signed: false,
        };

        match fqn {
            "scoop.core.__scoop_effect_is_active" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect is_active arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_is_active();
                let call =
                    self.build_call_preserving_gc_local_roots(span, rt, &[], "effect_is_active")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect is_active return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(active_i32) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect is_active return type",
                        at: span.into(),
                    });
                };
                let active_word = self.cast_int(active_i32, op_tag_ty, word_ty)?;
                Ok(CgValue::int(active_word, word_ty))
            }
            "scoop.core.__scoop_effect_set_active" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect set_active arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_set_active();
                let _ = self.builder.build_call(rt, &[], "effect_set_active")?;
                Ok(CgValue::unit())
            }
            "scoop.core.__scoop_effect_clear" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect clear arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_clear();
                let _ = self.builder.build_call(rt, &[], "effect_clear")?;
                Ok(CgValue::unit())
            }
            "scoop.core.__scoop_effect_slot_write" => {
                if args.len() != 3 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write arity mismatch",
                        at: span.into(),
                    });
                }

                let op_tag_word = self.codegen_sysroot_effect_intrinsic_word_arg(
                    span,
                    &args[0],
                    "effect slot_write op_tag",
                )?;
                let value_word = self.codegen_sysroot_effect_intrinsic_word_arg(
                    span,
                    &args[2],
                    "effect slot_write value",
                )?;
                let effect_instance_key_word = self.codegen_sysroot_effect_intrinsic_word_arg(
                    span,
                    &args[1],
                    "effect slot_write effect_instance_key",
                )?;
                let op_tag = self.cast_int(op_tag_word, word_ty, op_tag_ty)?;
                let effect_instance_key =
                    self.cast_int(effect_instance_key_word, word_ty, op_tag_ty)?;
                let value = self.cast_int(value_word, word_ty, slot_word_ty)?;

                let rt = self.declare_runtime_effect_perform_slot_write_u64();
                let _ = self.builder.build_call(
                    rt,
                    &[op_tag.into(), effect_instance_key.into(), value.into()],
                    "effect_slot_write_u64",
                )?;
                Ok(CgValue::unit())
            }
            "scoop.core.__scoop_effect_slot_write2" => {
                if args.len() != 4 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 arity mismatch",
                        at: span.into(),
                    });
                }

                let op_tag_word = self.codegen_sysroot_effect_intrinsic_word_arg(
                    span,
                    &args[0],
                    "effect slot_write2 op_tag",
                )?;
                let word0_raw = self.codegen_sysroot_effect_intrinsic_word_arg(
                    span,
                    &args[2],
                    "effect slot_write2 word0",
                )?;
                let word1_raw = self.codegen_sysroot_effect_intrinsic_word_arg(
                    span,
                    &args[3],
                    "effect slot_write2 word1",
                )?;
                let effect_instance_key_word = self.codegen_sysroot_effect_intrinsic_word_arg(
                    span,
                    &args[1],
                    "effect slot_write2 effect_instance_key",
                )?;
                let op_tag = self.cast_int(op_tag_word, word_ty, op_tag_ty)?;
                let effect_instance_key =
                    self.cast_int(effect_instance_key_word, word_ty, op_tag_ty)?;
                let word0 = self.cast_int(word0_raw, word_ty, slot_word_ty)?;
                let word1 = self.cast_int(word1_raw, word_ty, slot_word_ty)?;

                let rt = self.declare_runtime_effect_perform_slot_write_u64_2();
                let _ = self.builder.build_call(
                    rt,
                    &[
                        op_tag.into(),
                        effect_instance_key.into(),
                        word0.into(),
                        word1.into(),
                    ],
                    "effect_slot_write_u64_2",
                )?;
                Ok(CgValue::unit())
            }
            "scoop.core.__scoop_effect_slot_read_effect_instance_key" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_effect_instance_key arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_perform_slot_read_effect_instance_key();
                let call =
                    self.builder
                        .build_call(rt, &[], "effect_slot_read_effect_instance_key")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_effect_instance_key return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(effect_instance_key) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_effect_instance_key return type",
                        at: span.into(),
                    });
                };
                let effect_instance_key_word =
                    self.cast_int(effect_instance_key, op_tag_ty, word_ty)?;
                Ok(CgValue::int(effect_instance_key_word, word_ty))
            }
            "scoop.core.__scoop_effect_slot_read_op_tag" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_op_tag arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_perform_slot_read_op_tag();
                let call = self
                    .builder
                    .build_call(rt, &[], "effect_slot_read_op_tag")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_op_tag return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(op_tag) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_op_tag return type",
                        at: span.into(),
                    });
                };
                let op_tag_word = self.cast_int(op_tag, op_tag_ty, word_ty)?;
                Ok(CgValue::int(op_tag_word, word_ty))
            }
            "scoop.core.__scoop_effect_slot_read_len_words" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_len_words arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self
                    .builder
                    .build_call(rt, &[], "effect_slot_read_len_words")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_len_words return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(len_words) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_len_words return type",
                        at: span.into(),
                    });
                };
                let len_word = self.cast_int(len_words, op_tag_ty, word_ty)?;
                Ok(CgValue::int(len_word, word_ty))
            }
            "scoop.core.__scoop_effect_slot_read_value" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_value arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_perform_slot_read_u64();
                let call = self.builder.build_call(rt, &[], "effect_slot_read_u64")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_value return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(value_u64) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_value return type",
                        at: span.into(),
                    });
                };
                let value_word = self.cast_int(value_u64, slot_word_ty, word_ty)?;
                Ok(CgValue::int(value_word, word_ty))
            }
            "scoop.core.__scoop_effect_slot_read_word" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word arity mismatch",
                        at: span.into(),
                    });
                }

                let index_word = self.codegen_sysroot_effect_intrinsic_word_arg(
                    span,
                    &args[0],
                    "effect slot_read_word index",
                )?;
                let index = self.cast_int(index_word, word_ty, op_tag_ty)?;
                let rt = self.declare_runtime_effect_perform_slot_read_u64_at();
                let call =
                    self.builder
                        .build_call(rt, &[index.into()], "effect_slot_read_u64_at")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(value_u64) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word return type",
                        at: span.into(),
                    });
                };
                let value_word = self.cast_int(value_u64, slot_word_ty, word_ty)?;
                Ok(CgValue::int(value_word, word_ty))
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown sysroot effect intrinsic callee",
                at: callee_span.into(),
            }),
        }
    }
}
