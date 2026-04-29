//! LLVM lowering for production-visible MIR callable bodies.
//!
//! Production emit lowers callable bodies from `MaterializedMirPassView` through this bridge when
//! their MIR shape is inside the currently supported lowering subset. Explicit pass rewrites enter
//! here strictly; raw materialized bodies outside this subset, declaration-only callables, and
//! non-generic bodies that have not been published into the pass view continue to use their
//! existing HIR-compatible boundary.

use std::collections::HashSet;

use inkwell::values::{BasicMetadataValueEnum, FunctionValue, PointerValue};

use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::*;

#[derive(Clone, Copy)]
struct MirLocalSlot<'ctx> {
    cg_ty: CgTy,
    ptr: PointerValue<'ctx>,
}

const MIR_CAPTURE_BOX_FQN: &str = "scoop.__CaptureBox";

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(crate) fn raw_materialized_mir_body_requires_hir_compat_boundary(
        &mut self,
        hir_fun: &hir::FunDecl,
        mir_fun: &crate::mir::FunDecl,
    ) -> bool {
        if self.build_fun_callee_suspend_plan(hir_fun).is_some() {
            return true;
        }
        let Some(body) = mir_fun.body.as_ref() else {
            return true;
        };
        let mir_types = self
            .materialized_pass_view()
            .map(|view| &view.materialized().types)
            .unwrap_or(self.types);
        let supported = self.raw_materialized_mir_body_is_supported(body, mir_types);
        body.validate_cfg().is_err() || !supported
    }

    pub(crate) fn codegen_top_level_mir_fun(
        mut self,
        hir_fun: &hir::FunDecl,
        mir_fun: &crate::mir::FunDecl,
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let Some(body) = mir_fun.body.as_ref() else {
            return Ok(());
        };
        if hir_fun.fqn != mir_fun.fqn {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR callable identity mismatch",
                at: mir_fun.span.into(),
            });
        }
        if hir_fun.params.len() != mir_fun.params.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR callable arity mismatch",
                at: mir_fun.span.into(),
            });
        }
        body.validate_cfg()
            .map_err(|_| LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR cfg",
                at: mir_fun.span.into(),
            })?;

        self.current_source_id =
            self.source_id_for_path(hir_fun.source_path.as_path(), hir_fun.span)?;
        self.function_cx.current_callable_fqn = Some(hir_fun.fqn.clone());

        if self.build_fun_callee_suspend_plan(hir_fun).is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR effect-state-machine body lowering",
                at: mir_fun.span.into(),
            });
        }

        let entry = self.context.append_basic_block(llvm_fun, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(llvm_fun)?;

        let Some(declared_return_cg) = self.cg_ty_of(hir_fun.return_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR function return type",
                at: hir_fun.span.into(),
            });
        };
        self.function_cx.current_fun_return_ty = Some(declared_return_cg);
        let uses_hidden_sret = self
            .hidden_sret_result_ty(hir_fun.span, declared_return_cg)?
            .is_some();
        self.function_cx.current_sret_return_ptr = if uses_hidden_sret {
            Some(
                llvm_fun
                    .get_nth_param(0)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "missing pass MIR llvm function sret param",
                        at: hir_fun.span.into(),
                    })?
                    .into_pointer_value(),
            )
        } else {
            None
        };

        let (return_bb, return_alloca) =
            self.setup_function_return_context(hir_fun.span, llvm_fun, declared_return_cg)?;
        let mir_types = self
            .materialized_pass_view()
            .map(|view| &view.materialized().types)
            .unwrap_or(self.types);
        let mut local_slots = self.create_mir_local_slots(body, mir_types)?;
        self.bind_mir_params(
            hir_fun,
            mir_fun,
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
                &local_slots,
                &llvm_blocks,
                declared_return_cg,
            )?;
        }

        self.emit_function_return_block(
            hir_fun.span,
            declared_return_cg,
            return_bb,
            return_alloca,
        )?;
        self.finish_function_explicit_frame_layout(hir_fun.span)?;
        self.function_cx.current_sret_return_ptr = None;
        Ok(())
    }

    fn materialized_mir_callable(&self, fqn: &str) -> Option<(&TypeStore, &crate::mir::FunDecl)> {
        let pass_view = self.materialized_pass_view()?;
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
                        crate::mir::Item::Fun(_) | crate::mir::Item::Todo { .. } => None,
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

    fn raw_materialized_mir_closure_callable_is_supported(&mut self, fn_ptr: &str) -> bool {
        let Some((mir_types, mir_fun)) = self.materialized_mir_callable(fn_ptr) else {
            return false;
        };
        if !mir_fun.name.starts_with("$lambda") {
            return false;
        }
        let Some(body) = mir_fun.body.as_ref() else {
            return false;
        };
        let mut child = self.fresh_child_codegen();
        body.validate_cfg().is_ok() && child.raw_materialized_mir_body_is_supported(body, mir_types)
    }

    fn ensure_materialized_mir_closure_callable_defined(
        &mut self,
        span: crate::span::Span,
        fn_ptr: &str,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        if let Some(existing) = self.module.get_function(fn_ptr)
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
        let mut child = self.fresh_child_codegen();
        child.current_source_id = child.materialized_mir_callable_source_id(fn_ptr, span)?;
        let llvm_fun = child.declare_materialized_mir_closure_fun(span, mir_fun, mir_types)?;
        if llvm_fun.count_basic_blocks() == 0 {
            child.codegen_materialized_mir_closure_fun(mir_fun, mir_types, llvm_fun)?;
        }
        self.builder.position_at_end(saved_block);
        Ok(llvm_fun)
    }

    fn materialized_mir_callable_source_id(
        &self,
        fqn: &str,
        span: crate::span::Span,
    ) -> Result<SourceId, LlvmEmitError> {
        let mut owner_fqn = fqn;
        loop {
            if let Some(hir_fun) = self.fun_index.get(owner_fqn).copied() {
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

    fn declare_materialized_mir_closure_fun(
        &mut self,
        span: crate::span::Span,
        mir_fun: &crate::mir::FunDecl,
        mir_types: &TypeStore,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        if let Some(existing) = self.module.get_function(&mir_fun.fqn) {
            return Ok(existing);
        }

        let ret_cg = self.cg_ty_of_mir_type(mir_types, mir_fun.return_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure return type",
                at: mir_fun.span.into(),
            },
        )?;
        let hidden_sret_result_ty = self.hidden_sret_result_ty(span, ret_cg)?;
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> =
            Vec::with_capacity(mir_fun.params.len() + usize::from(hidden_sret_result_ty.is_some()));
        if let Some(result_ty) = hidden_sret_result_ty {
            let _ = result_ty;
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        llvm_param_tys.push(self.llvm_gc_i8_ptr_type().into());
        for param in mir_fun.params.iter().skip(1) {
            let param_ty = self.equivalent_codegen_type_id(mir_types, param.ty).ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR closure param type",
                    at: param.span.into(),
                },
            )?;
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
        let llvm_fun = self.module.add_function(&mir_fun.fqn, fn_ty, None);
        llvm_fun.set_call_conventions(0);
        if let Some(result_ty) = hidden_sret_result_ty {
            self.add_sret_attribute_to_function(llvm_fun, 0, result_ty);
        }
        Ok(llvm_fun)
    }

    fn codegen_materialized_mir_closure_fun(
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
        self.function_cx.current_sret_return_ptr = None;
        Ok(())
    }

    fn bind_mir_closure_params(
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

    fn codegen_mir_closure_env_param(
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
                let env_ty = self.mir_closure_env_object_type(span, fn_ptr, &capture_field_cgs)?;
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

    fn create_mir_local_slots(
        &mut self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
    ) -> Result<Vec<MirLocalSlot<'ctx>>, LlvmEmitError> {
        body.locals
            .iter()
            .map(|local| {
                let cg_ty = self.cg_ty_of_mir_type(mir_types, local.ty).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR local type",
                        at: local.span.into(),
                    },
                )?;
                let ptr = self.create_entry_alloca(
                    local.span,
                    local.name.as_deref().unwrap_or("mir_local"),
                    cg_ty,
                )?;
                Ok(MirLocalSlot { cg_ty, ptr })
            })
            .collect()
    }

    fn raw_materialized_mir_body_is_supported(
        &mut self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
    ) -> bool {
        let used_locals = collect_mir_local_uses(body);
        body.blocks.iter().all(|block| {
            block.stmts.iter().all(|stmt| {
                self.raw_materialized_mir_statement_is_supported(
                    stmt,
                    body,
                    mir_types,
                    &used_locals,
                )
            }) && self.raw_materialized_mir_terminator_is_supported(&block.terminator.kind)
        })
    }

    fn raw_materialized_mir_statement_is_supported(
        &mut self,
        stmt: &crate::mir::Statement,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        used_locals: &HashSet<crate::mir::LocalId>,
    ) -> bool {
        match &stmt.kind {
            crate::mir::StatementKind::Nop => true,
            crate::mir::StatementKind::Assign { target, value } => {
                if !used_locals.contains(target)
                    && let crate::mir::Rvalue::TopLevelRef(crate::mir::TopLevelRef { fqn }) = value
                    && self.fun_index.contains_key(fqn)
                {
                    return true;
                }
                let Some(target_cg) = self.mir_local_cg_ty(body, mir_types, *target) else {
                    return false;
                };
                self.raw_materialized_mir_rvalue_is_supported(
                    body,
                    mir_types,
                    value,
                    Some(target_cg),
                )
            }
            crate::mir::StatementKind::Todo(_) => false,
        }
    }

    fn raw_materialized_mir_terminator_is_supported(
        &self,
        terminator: &crate::mir::TerminatorKind,
    ) -> bool {
        match terminator {
            crate::mir::TerminatorKind::Return { value } => value
                .as_ref()
                // 现阶段 generic MIR 仍会把“函数体尾表达式”保留成 `Return { value: None }`
                // 的隐式约定；production raw MIR bridge 还没有独立的 tail-value 契约，
                // 因此这类 body 必须继续留在 HIR-compatible fallback，避免把隐式尾值
                // 误降成类型默认值（例如 Bool -> false）。
                .is_some_and(|operand| self.raw_materialized_mir_operand_is_supported(operand)),
            crate::mir::TerminatorKind::Goto { .. } | crate::mir::TerminatorKind::Unreachable => {
                true
            }
            crate::mir::TerminatorKind::CondBr { cond, .. } => {
                self.raw_materialized_mir_operand_is_supported(cond)
            }
            crate::mir::TerminatorKind::ResumeUnwind
            | crate::mir::TerminatorKind::Perform { .. }
            | crate::mir::TerminatorKind::Handle { .. }
            | crate::mir::TerminatorKind::Todo(_) => false,
        }
    }

    fn raw_materialized_mir_rvalue_is_supported(
        &mut self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        value: &crate::mir::Rvalue,
        target_cg: Option<CgTy>,
    ) -> bool {
        match value {
            crate::mir::Rvalue::Use(operand) | crate::mir::Rvalue::Unary { operand, .. } => {
                self.raw_materialized_mir_operand_is_supported(operand)
            }
            crate::mir::Rvalue::TopLevelRef(crate::mir::TopLevelRef { fqn }) => {
                self.object_inits.contains_key(fqn)
                    || self.top_level_consts.contains_key(fqn)
                    || self.top_level_immutable_values.contains_key(fqn)
                    || self.top_level_vars.contains_key(fqn)
            }
            crate::mir::Rvalue::Binary { lhs, rhs, .. } => {
                self.raw_materialized_mir_operand_is_supported(lhs)
                    && self.raw_materialized_mir_operand_is_supported(rhs)
            }
            crate::mir::Rvalue::Call { kind, args } => {
                self.raw_materialized_mir_call_kind_is_supported(body, mir_types, kind)
                    && args
                        .iter()
                        .all(|arg| self.raw_materialized_mir_operand_is_supported(&arg.value))
            }
            crate::mir::Rvalue::PatternMatch { subject, pattern } => {
                let Some(subject_ty) = self.mir_operand_cg_ty(body, mir_types, subject) else {
                    return false;
                };
                self.raw_materialized_mir_operand_is_supported(subject)
                    && self
                        .raw_materialized_mir_pattern_is_supported(mir_types, pattern, subject_ty)
            }
            crate::mir::Rvalue::PatternExtract { subject, path } => {
                let Some(target_cg) = target_cg else {
                    return false;
                };
                self.raw_materialized_mir_operand_is_supported(subject)
                    && self.raw_materialized_mir_pattern_extract_is_supported(
                        body, mir_types, subject, path, target_cg,
                    )
            }
            crate::mir::Rvalue::MakeTuple { elements } => self
                .raw_materialized_mir_make_tuple_is_supported(body, mir_types, elements, target_cg),
            crate::mir::Rvalue::TupleGet { tuple, index } => {
                self.raw_materialized_mir_tuple_get_is_supported(body, mir_types, tuple, *index)
            }
            crate::mir::Rvalue::MakeClosure { env, fn_ptr } => self
                .raw_materialized_mir_make_closure_is_supported(
                    body, mir_types, env, fn_ptr, target_cg,
                ),
            crate::mir::Rvalue::CaptureBoxNew { value } => self
                .raw_materialized_mir_capture_box_new_is_supported(
                    body, mir_types, value, target_cg,
                ),
            crate::mir::Rvalue::CaptureBoxGet { box_operand } => self
                .raw_materialized_mir_capture_box_get_is_supported(
                    body,
                    mir_types,
                    box_operand,
                    target_cg,
                ),
            crate::mir::Rvalue::CaptureBoxSet { box_operand, value } => self
                .raw_materialized_mir_capture_box_set_is_supported(
                    body,
                    mir_types,
                    box_operand,
                    value,
                    target_cg,
                ),
            crate::mir::Rvalue::UnresolvedName { .. }
            | crate::mir::Rvalue::TypeCheck { .. }
            | crate::mir::Rvalue::Cast { .. }
            | crate::mir::Rvalue::MemberAccess { .. }
            | crate::mir::Rvalue::PerformResult { .. }
            | crate::mir::Rvalue::Todo(_) => false,
        }
    }

    fn raw_materialized_mir_call_kind_is_supported(
        &mut self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        kind: &crate::mir::CallKind,
    ) -> bool {
        match kind {
            crate::mir::CallKind::Direct { callee_fqn } => {
                self.raw_materialized_mir_direct_call_is_supported(callee_fqn)
            }
            crate::mir::CallKind::Closure { callee, fn_ptr } => {
                let callee_supported = self.raw_materialized_mir_operand_is_supported(callee);
                let callee_fun_ty = self
                    .mir_operand_function_type(body, mir_types, callee)
                    .is_some();
                let closure_supported =
                    self.raw_materialized_mir_closure_callable_is_supported(fn_ptr);
                callee_supported && callee_fun_ty && closure_supported
            }
            crate::mir::CallKind::FunValue { callee } => {
                self.raw_materialized_mir_operand_is_supported(callee)
                    && self
                        .mir_operand_function_type(body, mir_types, callee)
                        .is_some()
            }
            crate::mir::CallKind::Virtual { .. }
            | crate::mir::CallKind::Interface { .. }
            | crate::mir::CallKind::Resume { .. } => false,
        }
    }

    fn raw_materialized_mir_capture_box_value_cg_is_supported(cg_ty: CgTy) -> bool {
        matches!(
            cg_ty,
            CgTy::Unit
                | CgTy::Bool
                | CgTy::Float64
                | CgTy::Float32
                | CgTy::Int(_)
                | CgTy::String
                | CgTy::Ref
                | CgTy::Tuple(_)
                | CgTy::Struct(_)
                | CgTy::Enum(_)
        )
    }

    fn raw_materialized_mir_capture_box_new_is_supported(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        value: &crate::mir::Operand,
        target_cg: Option<CgTy>,
    ) -> bool {
        matches!(target_cg, Some(CgTy::Ref))
            && self.raw_materialized_mir_operand_is_supported(value)
            && self
                .mir_operand_cg_ty(body, mir_types, value)
                .is_some_and(Self::raw_materialized_mir_capture_box_value_cg_is_supported)
    }

    fn raw_materialized_mir_capture_box_get_is_supported(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        box_operand: &crate::mir::Operand,
        target_cg: Option<CgTy>,
    ) -> bool {
        self.raw_materialized_mir_operand_is_supported(box_operand)
            && self
                .mir_capture_box_inner_cg_ty_from_operand(body, mir_types, box_operand)
                .zip(target_cg)
                .is_some_and(|(inner_cg, target_cg)| inner_cg == target_cg)
    }

    fn raw_materialized_mir_capture_box_set_is_supported(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        box_operand: &crate::mir::Operand,
        value: &crate::mir::Operand,
        target_cg: Option<CgTy>,
    ) -> bool {
        matches!(target_cg, Some(CgTy::Unit))
            && self.raw_materialized_mir_operand_is_supported(box_operand)
            && self.raw_materialized_mir_operand_is_supported(value)
            && self
                .mir_capture_box_inner_cg_ty_from_operand(body, mir_types, box_operand)
                .zip(self.mir_operand_cg_ty(body, mir_types, value))
                .is_some_and(|(inner_cg, value_cg)| inner_cg == value_cg)
    }

    fn raw_materialized_mir_make_tuple_is_supported(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        elements: &[crate::mir::Operand],
        target_cg: Option<CgTy>,
    ) -> bool {
        let Some(CgTy::Tuple(tuple_ty)) = target_cg else {
            return false;
        };
        let TypeKind::Value(ValueTypeKind::Tuple(tuple_elems)) = self.types.kind(tuple_ty) else {
            return false;
        };
        if tuple_elems.len() != elements.len() {
            return false;
        }
        tuple_elems.iter().zip(elements).all(|(elem_ty, operand)| {
            self.cg_ty_of(*elem_ty).is_some()
                && self.raw_materialized_mir_operand_is_supported(operand)
                && self
                    .mir_operand_cg_ty(body, mir_types, operand)
                    .is_some_and(|cg| {
                        self.cg_ty_of(*elem_ty)
                            .is_some_and(|expected| cg == expected)
                    })
        })
    }

    fn raw_materialized_mir_tuple_get_is_supported(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        tuple: &crate::mir::Operand,
        index: usize,
    ) -> bool {
        self.raw_materialized_mir_operand_is_supported(tuple)
            && self
                .mir_operand_type_id(body, tuple)
                .is_some_and(|tuple_ty| match mir_types.kind(tuple_ty) {
                    TypeKind::Value(ValueTypeKind::Tuple(elements)) => index < elements.len(),
                    _ => false,
                })
    }

    fn raw_materialized_mir_make_closure_is_supported(
        &mut self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        env: &crate::mir::Operand,
        fn_ptr: &str,
        target_cg: Option<CgTy>,
    ) -> bool {
        let target_supported = matches!(target_cg, Some(CgTy::Ref));
        let env_operand_supported = self.raw_materialized_mir_operand_is_supported(env);
        let env_shape_supported =
            self.mir_operand_cg_ty(body, mir_types, env)
                .is_some_and(|env_cg| {
                    self.mir_closure_env_capture_element_cg_tys(env_cg)
                        .is_some()
                });
        let closure_supported = self.raw_materialized_mir_closure_callable_is_supported(fn_ptr);
        target_supported && env_operand_supported && env_shape_supported && closure_supported
    }

    fn raw_materialized_mir_direct_call_is_supported(&self, callee_fqn: &str) -> bool {
        if self.extern_funs.contains_key(callee_fqn) {
            return true;
        }
        self.fun_index
            .get(callee_fqn)
            .is_some_and(|fun| fun.body.is_some())
    }

    fn raw_materialized_mir_operand_is_supported(&self, operand: &crate::mir::Operand) -> bool {
        match operand {
            crate::mir::Operand::Local(_) => true,
            crate::mir::Operand::Const(_) => true,
        }
    }

    fn cg_ty_of_mir_type(&self, mir_types: &TypeStore, ty: TypeId) -> Option<CgTy> {
        match mir_types.kind(ty) {
            TypeKind::Ref(RefTypeKind::String) => Some(CgTy::String),
            TypeKind::Ref(_) => Some(CgTy::Ref),
            TypeKind::StarProjection(star) => self.cg_ty_of_mir_type(mir_types, star.read_ty),
            TypeKind::Value(ValueTypeKind::Nothing) => Some(CgTy::Never),
            TypeKind::Value(ValueTypeKind::Unit) => Some(CgTy::Unit),
            TypeKind::Value(ValueTypeKind::Bool) => Some(CgTy::Bool),
            TypeKind::Value(ValueTypeKind::Char) => Some(CgTy::Int(IntTy {
                bits: 32,
                signed: false,
            })),
            TypeKind::Value(ValueTypeKind::Float64) => Some(CgTy::Float64),
            TypeKind::Value(ValueTypeKind::Float32) => Some(CgTy::Float32),
            TypeKind::Value(ValueTypeKind::Int) => Some(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            })),
            TypeKind::Value(ValueTypeKind::UInt) => Some(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: false,
            })),
            TypeKind::Value(ValueTypeKind::IntN(bits)) => Some(CgTy::Int(IntTy {
                bits: u32::from(*bits),
                signed: true,
            })),
            TypeKind::Value(ValueTypeKind::UIntN(bits)) => Some(CgTy::Int(IntTy {
                bits: u32::from(*bits),
                signed: false,
            })),
            TypeKind::Value(
                ValueTypeKind::Option(_) | ValueTypeKind::Tuple(_) | ValueTypeKind::Nominal(_),
            ) => self
                .equivalent_codegen_type_id(mir_types, ty)
                .and_then(|codegen_ty| self.cg_ty_of(codegen_ty)),
            TypeKind::Param(_) => None,
        }
    }

    fn equivalent_codegen_type_id(
        &self,
        source_types: &TypeStore,
        source_ty: TypeId,
    ) -> Option<TypeId> {
        let source_display = source_types.display(source_ty).to_string();
        self.types
            .iter_ids()
            .find(|&candidate| self.types.display(candidate).to_string() == source_display)
    }

    fn equivalent_codegen_effect_row(
        &self,
        source_types: &TypeStore,
        source_row: &crate::ty::EffectRow,
    ) -> Option<crate::ty::EffectRow> {
        let mut terms = Vec::with_capacity(source_row.terms.len());
        for term in &source_row.terms {
            terms.push(self.equivalent_codegen_type_id(source_types, *term)?);
        }
        Some(crate::ty::EffectRow::new(terms))
    }

    fn equivalent_codegen_function_type(
        &self,
        source_types: &TypeStore,
        fun_ty: &crate::ty::FunctionType,
    ) -> Option<crate::ty::FunctionType> {
        let receiver = match fun_ty.receiver {
            Some(ty) => Some(self.equivalent_codegen_type_id(source_types, ty)?),
            None => None,
        };
        let mut params = Vec::with_capacity(fun_ty.params.len());
        for param in &fun_ty.params {
            params.push(self.equivalent_codegen_type_id(source_types, *param)?);
        }
        Some(crate::ty::FunctionType {
            receiver,
            params,
            return_ty: self.equivalent_codegen_type_id(source_types, fun_ty.return_ty)?,
            effects: self.equivalent_codegen_effect_row(source_types, &fun_ty.effects)?,
            effects_closed: fun_ty.effects_closed,
        })
    }

    fn mir_local_cg_ty(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        local: crate::mir::LocalId,
    ) -> Option<CgTy> {
        let local = body.locals.get(local.as_u32() as usize)?;
        self.cg_ty_of_mir_type(mir_types, local.ty)
    }

    fn mir_operand_cg_ty(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        operand: &crate::mir::Operand,
    ) -> Option<CgTy> {
        match operand {
            crate::mir::Operand::Local(local) => self.mir_local_cg_ty(body, mir_types, *local),
            crate::mir::Operand::Const(value) => self.mir_const_cg_ty(value),
        }
    }

    fn mir_const_cg_ty(&self, value: &crate::mir::ConstValue) -> Option<CgTy> {
        match value {
            crate::mir::ConstValue::Bool(_) => Some(CgTy::Bool),
            crate::mir::ConstValue::Char => Some(CgTy::Int(IntTy {
                bits: 32,
                signed: false,
            })),
            crate::mir::ConstValue::Unit => Some(CgTy::Unit),
            crate::mir::ConstValue::Int | crate::mir::ConstValue::SynthInt(_) => {
                self.cg_ty_of(self.builtins.int)
            }
            crate::mir::ConstValue::Float64 => Some(CgTy::Float64),
            crate::mir::ConstValue::Float32 => Some(CgTy::Float32),
            crate::mir::ConstValue::String => Some(CgTy::String),
        }
    }

    fn raw_materialized_mir_pattern_is_supported(
        &mut self,
        mir_types: &TypeStore,
        pattern: &crate::mir::Pattern,
        subject_ty: CgTy,
    ) -> bool {
        match pattern {
            crate::mir::Pattern::Else
            | crate::mir::Pattern::Wildcard
            | crate::mir::Pattern::Rest
            | crate::mir::Pattern::Bind { .. } => true,
            crate::mir::Pattern::Or { pats } => pats.iter().all(|pat| {
                self.raw_materialized_mir_pattern_is_supported(mir_types, pat, subject_ty)
            }),
            crate::mir::Pattern::Is { ty } => {
                matches!(subject_ty, CgTy::Ref | CgTy::String)
                    && self
                        .equivalent_codegen_type_id(mir_types, *ty)
                        .and_then(|target_ty| self.cg_ty_of(target_ty))
                        .is_some_and(|target_cg| matches!(target_cg, CgTy::Ref | CgTy::String))
            }
            crate::mir::Pattern::Tuple { elements } => {
                let CgTy::Tuple(tuple_ty) = subject_ty else {
                    return false;
                };
                self.raw_materialized_mir_tuple_pattern_is_supported(mir_types, tuple_ty, elements)
            }
            crate::mir::Pattern::Variant { name, args } => {
                let CgTy::Enum(enum_ty) = subject_ty else {
                    return false;
                };
                self.raw_materialized_mir_variant_pattern_is_supported(
                    mir_types, enum_ty, name, args,
                )
            }
            crate::mir::Pattern::IntLit { .. } | crate::mir::Pattern::CharLit { .. } => {
                matches!(subject_ty, CgTy::Int(_))
            }
            crate::mir::Pattern::StringLit { .. } => subject_ty == CgTy::String,
            crate::mir::Pattern::BoolLit { .. } => subject_ty == CgTy::Bool,
        }
    }

    fn raw_materialized_mir_tuple_pattern_is_supported(
        &mut self,
        mir_types: &TypeStore,
        tuple_ty: TypeId,
        elements: &[crate::mir::Pattern],
    ) -> bool {
        let TypeKind::Value(ValueTypeKind::Tuple(tuple_elems)) = self.types.kind(tuple_ty) else {
            return false;
        };

        let (prefix_pats, has_rest) = match elements.last() {
            Some(crate::mir::Pattern::Rest) => {
                (&elements[..elements.len().saturating_sub(1)], true)
            }
            _ => (elements, false),
        };
        let pat_arity = prefix_pats.len();
        if (!has_rest && pat_arity != tuple_elems.len())
            || (has_rest && pat_arity > tuple_elems.len())
        {
            return false;
        }

        prefix_pats.iter().enumerate().all(|(idx, pat)| {
            let Some(elem_ty) = self.tuple_element_cg_ty(tuple_ty, idx) else {
                return false;
            };
            self.raw_materialized_mir_pattern_is_supported(mir_types, pat, elem_ty)
        })
    }

    fn raw_materialized_mir_variant_pattern_is_supported(
        &mut self,
        mir_types: &TypeStore,
        enum_ty: TypeId,
        variant_name: &str,
        args: &[crate::mir::Pattern],
    ) -> bool {
        let dummy_span = crate::span::Span::new(0, 0);
        let Ok(layout) = self.cg_enum_layout(dummy_span, enum_ty) else {
            return false;
        };
        let Some(variant) = layout
            .variants
            .iter()
            .find(|variant| variant.name == variant_name)
        else {
            return false;
        };
        let (prefix_pats, has_rest) = match args.last() {
            Some(crate::mir::Pattern::Rest) => (&args[..args.len().saturating_sub(1)], true),
            _ => (args, false),
        };
        let expected_arity = variant.fields.len();
        let found_arity = prefix_pats.len();
        if (!has_rest && expected_arity != found_arity)
            || (has_rest && found_arity > expected_arity)
        {
            return false;
        }

        prefix_pats.iter().enumerate().all(|(idx, pat)| {
            let Some(field_ty) = variant.fields.get(idx).copied() else {
                return false;
            };
            self.raw_materialized_mir_pattern_is_supported(mir_types, pat, field_ty)
        })
    }

    fn raw_materialized_mir_pattern_extract_is_supported(
        &mut self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        subject: &crate::mir::Operand,
        path: &[crate::mir::PatternBindingStep],
        target_ty: CgTy,
    ) -> bool {
        let Some(mut current_ty) = self.mir_operand_cg_ty(body, mir_types, subject) else {
            return false;
        };
        for step in path {
            let Some(next_ty) = self.mir_pattern_binding_step_result_ty(current_ty, step) else {
                return false;
            };
            current_ty = next_ty;
        }
        current_ty == target_ty
    }

    fn mir_pattern_binding_step_result_ty(
        &mut self,
        current_ty: CgTy,
        step: &crate::mir::PatternBindingStep,
    ) -> Option<CgTy> {
        match step {
            crate::mir::PatternBindingStep::TupleIndex(index) => {
                let CgTy::Tuple(tuple_ty) = current_ty else {
                    return None;
                };
                self.tuple_element_cg_ty(tuple_ty, *index)
            }
            crate::mir::PatternBindingStep::VariantField {
                variant,
                field_index,
            } => {
                let CgTy::Enum(enum_ty) = current_ty else {
                    return None;
                };
                let layout = self
                    .cg_enum_layout(crate::span::Span::new(0, 0), enum_ty)
                    .ok()?;
                let variant = layout.variants.iter().find(|item| item.name == *variant)?;
                variant.fields.get(*field_index).copied()
            }
        }
    }

    fn tuple_element_cg_ty(&self, tuple_ty: TypeId, index: usize) -> Option<CgTy> {
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = self.types.kind(tuple_ty) else {
            return None;
        };
        let elem_ty = *elements.get(index)?;
        self.cg_ty_of(elem_ty)
    }

    fn bind_mir_params(
        &mut self,
        hir_fun: &hir::FunDecl,
        mir_fun: &crate::mir::FunDecl,
        llvm_fun: FunctionValue<'ctx>,
        param_offset: u32,
        slots: &mut [MirLocalSlot<'ctx>],
    ) -> Result<(), LlvmEmitError> {
        for (idx, param) in mir_fun.params.iter().enumerate() {
            let hir_param = hir_fun
                .params
                .get(idx)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR param arity",
                    at: param.span.into(),
                })?;
            let slot = slots.get(param.local.as_u32() as usize).copied().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR param local",
                    at: param.span.into(),
                },
            )?;
            let abi = self.ordinary_param_abi(param.span, hir_param.ty)?;
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

    fn codegen_mir_statement(
        &mut self,
        stmt: &crate::mir::Statement,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        used_locals: &HashSet<crate::mir::LocalId>,
    ) -> Result<(), LlvmEmitError> {
        if self
            .builder
            .get_insert_block()
            .is_some_and(|bb| bb.get_terminator().is_some())
        {
            return Ok(());
        }

        match &stmt.kind {
            crate::mir::StatementKind::Nop => Ok(()),
            crate::mir::StatementKind::Assign { target, value } => {
                if !used_locals.contains(target)
                    && let crate::mir::Rvalue::TopLevelRef(crate::mir::TopLevelRef { fqn }) = value
                    && self.fun_index.contains_key(fqn)
                {
                    return Ok(());
                }
                let slot = self.mir_local_slot(stmt.span, slots, *target)?;
                let value =
                    self.codegen_mir_rvalue(stmt.span, value, body, mir_types, slots, slot.cg_ty)?;
                let _ = self.store_local_value(stmt.span, slot.ptr, slot.cg_ty, value)?;
                Ok(())
            }
            crate::mir::StatementKind::Todo(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR statement todo",
                at: stmt.span.into(),
            }),
        }
    }

    fn codegen_mir_terminator(
        &mut self,
        terminator: &crate::mir::Terminator,
        _body: &crate::mir::Body,
        slots: &[MirLocalSlot<'ctx>],
        llvm_blocks: &[inkwell::basic_block::BasicBlock<'ctx>],
        declared_return_cg: CgTy,
    ) -> Result<(), LlvmEmitError> {
        if self
            .builder
            .get_insert_block()
            .is_some_and(|bb| bb.get_terminator().is_some())
        {
            return Ok(());
        }

        match &terminator.kind {
            crate::mir::TerminatorKind::Return { value } => {
                let value = match value {
                    Some(operand) => self.codegen_mir_operand_expected(
                        terminator.span,
                        operand,
                        slots,
                        Some(declared_return_cg),
                    )?,
                    None => self.default_value(terminator.span, declared_return_cg)?,
                };
                let value = self.coerce_value(terminator.span, value, declared_return_cg)?;
                self.finish_function_return_path(terminator.span, declared_return_cg, value)
            }
            crate::mir::TerminatorKind::Goto { target } => {
                let target_bb = llvm_blocks.get(target.as_u32() as usize).copied().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR goto target",
                        at: terminator.span.into(),
                    },
                )?;
                self.builder.build_unconditional_branch(target_bb)?;
                Ok(())
            }
            crate::mir::TerminatorKind::CondBr {
                cond,
                then_target,
                else_target,
            } => {
                let cond = self
                    .codegen_mir_operand(terminator.span, cond, slots)?
                    .as_bool()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR branch condition",
                        at: terminator.span.into(),
                    })?;
                let then_bb = llvm_blocks
                    .get(then_target.as_u32() as usize)
                    .copied()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR then target",
                        at: terminator.span.into(),
                    })?;
                let else_bb = llvm_blocks
                    .get(else_target.as_u32() as usize)
                    .copied()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR else target",
                        at: terminator.span.into(),
                    })?;
                self.builder
                    .build_conditional_branch(cond, then_bb, else_bb)?;
                Ok(())
            }
            crate::mir::TerminatorKind::Unreachable => {
                self.builder.build_unreachable()?;
                Ok(())
            }
            crate::mir::TerminatorKind::ResumeUnwind
            | crate::mir::TerminatorKind::Perform { .. }
            | crate::mir::TerminatorKind::Handle { .. }
            | crate::mir::TerminatorKind::Todo(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR terminator",
                at: terminator.span.into(),
            }),
        }
    }

    fn codegen_mir_rvalue(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Rvalue,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match value {
            crate::mir::Rvalue::Use(operand) => {
                self.codegen_mir_operand_expected(span, operand, slots, Some(target_cg))
            }
            crate::mir::Rvalue::TopLevelRef(crate::mir::TopLevelRef { fqn }) => {
                self.codegen_top_level_value_ref(span, fqn)
            }
            crate::mir::Rvalue::Unary { op, operand } => {
                let operand = self.codegen_mir_operand(span, operand, slots)?;
                self.codegen_mir_unary(span, *op, operand)
            }
            crate::mir::Rvalue::Binary { lhs, op, rhs } => {
                let lhs = self.codegen_mir_operand(span, lhs, slots)?;
                let rhs = self.codegen_mir_operand(span, rhs, slots)?;
                self.codegen_mir_binary(span, *op, lhs, rhs)
            }
            crate::mir::Rvalue::Call { kind, args } => {
                self.codegen_mir_call(span, kind, args, body, mir_types, slots)
            }
            crate::mir::Rvalue::PatternMatch { subject, pattern } => {
                self.codegen_mir_pattern_match(span, mir_types, subject, pattern, slots)
            }
            crate::mir::Rvalue::PatternExtract { subject, path } => {
                self.codegen_mir_pattern_extract(span, subject, path, slots, target_cg)
            }
            crate::mir::Rvalue::MakeTuple { elements } => {
                self.codegen_mir_make_tuple(span, body, mir_types, elements, target_cg, slots)
            }
            crate::mir::Rvalue::TupleGet { tuple, index } => {
                self.codegen_mir_tuple_get(span, body, mir_types, tuple, *index, slots)
            }
            crate::mir::Rvalue::MakeClosure { env, fn_ptr } => {
                let env_cg = self.mir_operand_cg_ty(body, mir_types, env).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR closure env type",
                        at: span.into(),
                    },
                )?;
                self.codegen_mir_make_closure(span, env, fn_ptr, env_cg, target_cg, slots)
            }
            crate::mir::Rvalue::CaptureBoxNew { value } => {
                self.codegen_mir_capture_box_new(span, value, body, mir_types, target_cg, slots)
            }
            crate::mir::Rvalue::CaptureBoxGet { box_operand } => self.codegen_mir_capture_box_get(
                span,
                box_operand,
                body,
                mir_types,
                target_cg,
                slots,
            ),
            crate::mir::Rvalue::CaptureBoxSet { box_operand, value } => {
                self.codegen_mir_capture_box_set(span, box_operand, value, body, mir_types, slots)
            }
            crate::mir::Rvalue::UnresolvedName { .. }
            | crate::mir::Rvalue::TypeCheck { .. }
            | crate::mir::Rvalue::Cast { .. }
            | crate::mir::Rvalue::MemberAccess { .. }
            | crate::mir::Rvalue::PerformResult { .. }
            | crate::mir::Rvalue::Todo(_) => {
                let _ = target_cg;
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR rvalue",
                    at: span.into(),
                })
            }
        }
    }

    fn codegen_mir_operand(
        &mut self,
        span: crate::span::Span,
        operand: &crate::mir::Operand,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_mir_operand_expected(span, operand, slots, None)
    }

    fn codegen_mir_operand_expected(
        &mut self,
        span: crate::span::Span,
        operand: &crate::mir::Operand,
        slots: &[MirLocalSlot<'ctx>],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match operand {
            crate::mir::Operand::Local(local) => {
                let slot = self.mir_local_slot(span, slots, *local)?;
                self.load_mir_local(span, slot)
            }
            crate::mir::Operand::Const(value) => self.codegen_mir_const(span, value, expected),
        }
    }

    fn codegen_mir_const(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::ConstValue,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match value {
            crate::mir::ConstValue::Bool(v) => Ok(CgValue::bool(
                self.context.bool_type().const_int(*v as u64, false),
            )),
            crate::mir::ConstValue::Char => {
                let text = self.current_source_slice(span)?;
                let value =
                    crate::syntax::char_literal::parse_char_literal(text).map_err(|_| {
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR char literal",
                            at: span.into(),
                        }
                    })?;
                Ok(CgValue::int(
                    self.context.i32_type().const_int(value as u64, false),
                    IntTy {
                        bits: 32,
                        signed: false,
                    },
                ))
            }
            crate::mir::ConstValue::Unit => Ok(CgValue::unit()),
            crate::mir::ConstValue::Int => {
                let int_ty = match expected.or_else(|| self.cg_ty_of(self.builtins.int)) {
                    Some(CgTy::Int(int_ty)) => int_ty,
                    _ => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR int builtin type",
                            at: span.into(),
                        });
                    }
                };
                let bits = self.int_literal_bits_for_ty(span, int_ty)?;
                Ok(CgValue::int(
                    self.int_type(int_ty).const_int(bits, false),
                    int_ty,
                ))
            }
            crate::mir::ConstValue::SynthInt(value) => {
                let int_ty = match expected.or_else(|| self.cg_ty_of(self.builtins.int)) {
                    Some(CgTy::Int(int_ty)) => int_ty,
                    _ => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR synthesized int builtin type",
                            at: span.into(),
                        });
                    }
                };
                Ok(CgValue::int(
                    self.int_type(int_ty)
                        .const_int(*value as u64, int_ty.signed),
                    int_ty,
                ))
            }
            crate::mir::ConstValue::Float64 => {
                let parsed = crate::syntax::float_literal::parse_float_literal(
                    self.current_source_slice(span)?,
                );
                Ok(CgValue::float(
                    self.context.f64_type().const_float(parsed.value),
                    CgTy::Float64,
                ))
            }
            crate::mir::ConstValue::Float32 => {
                let parsed = crate::syntax::float_literal::parse_float_literal(
                    self.current_source_slice(span)?,
                );
                Ok(CgValue::float(
                    self.context.f32_type().const_float(parsed.value),
                    CgTy::Float32,
                ))
            }
            crate::mir::ConstValue::String => self.codegen_string_literal(span),
        }
    }

    fn codegen_mir_pattern_match(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        subject: &crate::mir::Operand,
        pattern: &crate::mir::Pattern,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let subject = self.codegen_mir_operand(span, subject, slots)?;
        let cond = self.codegen_mir_pattern_match_value(span, mir_types, subject, pattern)?;
        Ok(CgValue::bool(cond))
    }

    fn codegen_mir_pattern_match_value(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        subject: CgValue<'ctx>,
        pattern: &crate::mir::Pattern,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match pattern {
            crate::mir::Pattern::Else
            | crate::mir::Pattern::Wildcard
            | crate::mir::Pattern::Rest
            | crate::mir::Pattern::Bind { .. } => Ok(self.context.bool_type().const_int(1, false)),
            crate::mir::Pattern::Or { pats } => {
                let mut cond = self.context.bool_type().const_int(0, false);
                for pat in pats {
                    let pat_cond =
                        self.codegen_mir_pattern_match_value(span, mir_types, subject, pat)?;
                    cond = self
                        .builder
                        .build_or(cond, pat_cond, "pass_mir_pattern_or")?;
                }
                Ok(cond)
            }
            crate::mir::Pattern::Is { ty } => {
                self.codegen_mir_is_pattern_match(span, mir_types, subject, *ty)
            }
            crate::mir::Pattern::Tuple { elements } => {
                self.codegen_mir_tuple_pattern_match(span, mir_types, subject, elements)
            }
            crate::mir::Pattern::Variant { name, args } => {
                self.codegen_mir_variant_pattern_match(span, mir_types, subject, name, args)
            }
            crate::mir::Pattern::IntLit { raw } => {
                let (value, int_ty) =
                    subject.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern int subject",
                        at: span.into(),
                    })?;
                let expected = self.int_literal_bits_from_text_for_ty(span, raw, int_ty)?;
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    value,
                    self.int_type(int_ty).const_int(expected, false),
                    "pass_mir_pattern_int_eq",
                )?)
            }
            crate::mir::Pattern::CharLit { value: expected } => {
                let (value, int_ty) =
                    subject.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern char subject",
                        at: span.into(),
                    })?;
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    value,
                    self.int_type(int_ty).const_int(*expected as u64, false),
                    "pass_mir_pattern_char_eq",
                )?)
            }
            crate::mir::Pattern::StringLit { value } => {
                let CgTy::String = subject.ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern string subject",
                        at: span.into(),
                    });
                };
                let Some(BasicValueEnum::PointerValue(subject_ptr)) = subject.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern string value",
                        at: span.into(),
                    });
                };
                let expected = self.codegen_string_literal_from_text(span, value)?;
                let Some(BasicValueEnum::PointerValue(expected_ptr)) = expected.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern string literal",
                        at: span.into(),
                    });
                };
                let fn_val = self.declare_runtime_string_equals();
                let call = self.builder.build_call(
                    fn_val,
                    &[subject_ptr.into(), expected_ptr.into()],
                    "pass_mir_pattern_str_eq",
                )?;
                let raw_result = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern string equals return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(eq_i64) = raw_result else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern string equals return type",
                        at: span.into(),
                    });
                };
                Ok(self.builder.build_int_compare(
                    IntPredicate::NE,
                    eq_i64,
                    self.context.i64_type().const_zero(),
                    "pass_mir_pattern_str_eq_bool",
                )?)
            }
            crate::mir::Pattern::BoolLit { value: expected } => {
                let value = subject
                    .as_bool()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern bool subject",
                        at: span.into(),
                    })?;
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    value,
                    self.context.bool_type().const_int(*expected as u64, false),
                    "pass_mir_pattern_bool_eq",
                )?)
            }
        }
    }

    fn codegen_mir_is_pattern_match(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        subject: CgValue<'ctx>,
        target_ty: TypeId,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let target_ty = self
            .equivalent_codegen_type_id(mir_types, target_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR pattern is target type",
                at: span.into(),
            })?;
        let target_cg = self
            .cg_ty_of(target_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR pattern is target type",
                at: span.into(),
            })?;
        if !matches!(target_cg, CgTy::Ref | CgTy::String) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR pattern is target runtime type",
                at: span.into(),
            });
        }

        let subject = match subject.ty {
            CgTy::Ref => subject,
            CgTy::String => self.coerce_value(span, subject, CgTy::Ref)?,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR pattern is subject type",
                    at: span.into(),
                });
            }
        };
        let Some(BasicValueEnum::PointerValue(subject_ptr)) = subject.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR pattern is subject value",
                at: span.into(),
            });
        };
        self.codegen_ref_is_instance_of(span, subject_ptr, target_ty)
    }

    fn codegen_mir_tuple_pattern_match(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        subject: CgValue<'ctx>,
        elements: &[crate::mir::Pattern],
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let CgTy::Tuple(tuple_ty) = subject.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple pattern subject type",
                at: span.into(),
            });
        };
        let TypeKind::Value(ValueTypeKind::Tuple(tuple_elems)) = self.types.kind(tuple_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple pattern tuple type",
                at: span.into(),
            });
        };
        let Some(raw) = subject.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple pattern subject value",
                at: span.into(),
            });
        };
        let tuple_v = raw.into_struct_value();
        let (prefix_pats, has_rest) = match elements.last() {
            Some(crate::mir::Pattern::Rest) => {
                (&elements[..elements.len().saturating_sub(1)], true)
            }
            _ => (elements, false),
        };
        let pat_arity = prefix_pats.len();
        if (!has_rest && pat_arity != tuple_elems.len())
            || (has_rest && pat_arity > tuple_elems.len())
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple pattern arity",
                at: span.into(),
            });
        }

        let mut cond = self.context.bool_type().const_int(1, false);
        for (idx, pat) in prefix_pats.iter().enumerate() {
            let elem_ty = self.tuple_element_cg_ty(tuple_ty, idx).ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR tuple pattern element type",
                    at: span.into(),
                },
            )?;
            let elem_value = self.extract_mir_tuple_element_value(span, tuple_v, idx, elem_ty)?;
            let elem_cond =
                self.codegen_mir_pattern_match_value(span, mir_types, elem_value, pat)?;
            cond = self
                .builder
                .build_and(cond, elem_cond, "pass_mir_tuple_pattern_and")?;
        }
        Ok(cond)
    }

    fn codegen_mir_variant_pattern_match(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        subject: CgValue<'ctx>,
        variant_name: &str,
        args: &[crate::mir::Pattern],
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let CgTy::Enum(enum_ty) = subject.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR variant pattern subject type",
                at: span.into(),
            });
        };
        let Some(raw) = subject.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR variant pattern subject value",
                at: span.into(),
            });
        };
        let (repr, variant) = {
            let layout = self.cg_enum_layout(span, enum_ty)?;
            let repr = layout.repr;
            let variant = layout
                .variants
                .iter()
                .find(|variant| variant.name == variant_name)
                .cloned()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR unknown enum variant",
                    at: span.into(),
                })?;
            (repr, variant)
        };
        let (prefix_pats, has_rest) = match args.last() {
            Some(crate::mir::Pattern::Rest) => (&args[..args.len().saturating_sub(1)], true),
            _ => (args, false),
        };
        let expected_arity = variant.fields.len();
        let found_arity = prefix_pats.len();
        if (!has_rest && expected_arity != found_arity)
            || (has_rest && found_arity > expected_arity)
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR variant pattern arity",
                at: span.into(),
            });
        }

        let tag = self.extract_mir_enum_tag_value(span, enum_ty, repr, raw)?;
        let expected = tag.get_type().const_int(variant.tag, false);
        let tag_eq = self.builder.build_int_compare(
            IntPredicate::EQ,
            tag,
            expected,
            "pass_mir_variant_tag_eq",
        )?;
        if !prefix_pats
            .iter()
            .any(Self::mir_pattern_needs_payload_match)
        {
            return Ok(tag_eq);
        }

        let subject_ptr = self.create_entry_alloca(span, "pass_mir_variant_subject", subject.ty)?;
        let _ = self.store_local_value(span, subject_ptr, subject.ty, subject)?;
        let current_bb =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = current_bb
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;
        let payload_bb = self
            .context
            .append_basic_block(func, "pass_mir_variant_payload");
        let merge_bb = self
            .context
            .append_basic_block(func, "pass_mir_variant_merge");
        self.builder
            .build_conditional_branch(tag_eq, payload_bb, merge_bb)?;

        self.builder.position_at_end(payload_bb);
        let mut payload_cond = self.context.bool_type().const_int(1, false);
        for (idx, pat) in prefix_pats.iter().enumerate() {
            if !Self::mir_pattern_needs_payload_match(pat) {
                continue;
            }
            let extracted = self.extract_matched_when_variant_field_value(
                enum_ty,
                repr,
                &variant,
                idx,
                span,
                subject_ptr,
            )?;
            let field_cond =
                self.codegen_mir_pattern_match_value(span, mir_types, extracted, pat)?;
            payload_cond =
                self.builder
                    .build_and(payload_cond, field_cond, "pass_mir_variant_payload_and")?;
        }

        let payload_tail =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR variant payload tail",
                    at: span.into(),
                })?;
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(merge_bb);
        let phi = self
            .builder
            .build_phi(self.context.bool_type(), "pass_mir_variant_match")?;
        let no_match = self.context.bool_type().const_int(0, false);
        phi.add_incoming(&[(&no_match, current_bb), (&payload_cond, payload_tail)]);
        Ok(phi.as_basic_value().into_int_value())
    }

    fn codegen_mir_pattern_extract(
        &mut self,
        span: crate::span::Span,
        subject: &crate::mir::Operand,
        path: &[crate::mir::PatternBindingStep],
        slots: &[MirLocalSlot<'ctx>],
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let mut current = self.codegen_mir_operand(span, subject, slots)?;
        for step in path {
            current = match step {
                crate::mir::PatternBindingStep::TupleIndex(index) => {
                    let CgTy::Tuple(tuple_ty) = current.ty else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR pattern extract tuple subject type",
                            at: span.into(),
                        });
                    };
                    let Some(raw) = current.value else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR pattern extract tuple subject value",
                            at: span.into(),
                        });
                    };
                    let elem_ty = self.tuple_element_cg_ty(tuple_ty, *index).ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR pattern extract tuple field type",
                            at: span.into(),
                        },
                    )?;
                    self.extract_mir_tuple_element_value(
                        span,
                        raw.into_struct_value(),
                        *index,
                        elem_ty,
                    )?
                }
                crate::mir::PatternBindingStep::VariantField {
                    variant,
                    field_index,
                } => {
                    let CgTy::Enum(enum_ty) = current.ty else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR pattern extract variant subject type",
                            at: span.into(),
                        });
                    };
                    let layout = self.cg_enum_layout(span, enum_ty)?;
                    let variant = layout
                        .variants
                        .iter()
                        .find(|item| item.name == *variant)
                        .cloned()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR pattern extract unknown enum variant",
                            at: span.into(),
                        })?;
                    let subject_ptr =
                        self.create_entry_alloca(span, "pass_mir_extract_subject", current.ty)?;
                    let _ = self.store_local_value(span, subject_ptr, current.ty, current)?;
                    self.extract_matched_when_variant_field_value(
                        enum_ty,
                        layout.repr,
                        &variant,
                        *field_index,
                        span,
                        subject_ptr,
                    )?
                }
            };
        }
        self.coerce_value(span, current, target_cg)
    }

    fn extract_mir_tuple_element_value(
        &mut self,
        span: crate::span::Span,
        tuple_v: inkwell::values::StructValue<'ctx>,
        index: usize,
        elem_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if elem_ty == CgTy::Unit {
            return Ok(CgValue::unit());
        }
        let raw = self
            .builder
            .build_extract_value(tuple_v, index as u32, "pass_mir_tuple_elem")?;
        self.cg_value_from_loaded(span, elem_ty, raw)
    }

    fn extract_mir_enum_tag_value(
        &mut self,
        span: crate::span::Span,
        enum_ty: TypeId,
        repr: CgEnumRepr,
        raw: BasicValueEnum<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match repr {
            CgEnumRepr::TaggedUnion => Ok(self
                .builder
                .build_extract_value(raw.into_struct_value(), 0, "pass_mir_when_tag")?
                .into_int_value()),
            CgEnumRepr::Niche {
                storage,
                none_value,
            } => {
                let is_none = match storage {
                    NicheStorage::Pointer => {
                        let ptr = raw.into_pointer_value();
                        if none_value != 0 {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "pass MIR niche pointer none_value",
                                at: span.into(),
                            });
                        }
                        self.builder.build_is_null(ptr, "pass_mir_option_is_none")?
                    }
                    NicheStorage::U8 => {
                        let value = raw.into_int_value();
                        let expected = self.context.i8_type().const_int(none_value, false);
                        self.builder.build_int_compare(
                            IntPredicate::EQ,
                            value,
                            expected,
                            "pass_mir_option_is_none",
                        )?
                    }
                };
                let some_tag = self.context.i32_type().const_int(0, false);
                let none_tag = self.context.i32_type().const_int(1, false);
                Ok(self
                    .builder
                    .build_select(is_none, none_tag, some_tag, "pass_mir_option_tag")?
                    .into_int_value())
            }
            CgEnumRepr::ValueOnly { .. } => {
                let _ = enum_ty;
                Ok(raw.into_int_value())
            }
        }
    }

    fn mir_pattern_needs_payload_match(pattern: &crate::mir::Pattern) -> bool {
        !matches!(
            pattern,
            crate::mir::Pattern::Else
                | crate::mir::Pattern::Wildcard
                | crate::mir::Pattern::Rest
                | crate::mir::Pattern::Bind { .. }
        )
    }

    fn codegen_mir_call(
        &mut self,
        span: crate::span::Span,
        kind: &crate::mir::CallKind,
        args: &[crate::mir::CallArg],
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match kind {
            crate::mir::CallKind::Direct { callee_fqn } => {
                self.codegen_mir_direct_call(span, callee_fqn, args, body, slots)
            }
            crate::mir::CallKind::Closure { callee, fn_ptr } => {
                let fun_ty = self
                    .mir_operand_function_type(body, mir_types, callee)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR closure callee type",
                        at: span.into(),
                    })?;
                self.codegen_mir_closure_call(span, callee, fn_ptr, args, &fun_ty, slots)
            }
            crate::mir::CallKind::FunValue { callee } => {
                let fun_ty = self
                    .mir_operand_function_type(body, mir_types, callee)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR function-value callee type",
                        at: span.into(),
                    })?;
                self.codegen_mir_fun_value_call(span, callee, args, &fun_ty, slots)
            }
            crate::mir::CallKind::Virtual { .. }
            | crate::mir::CallKind::Interface { .. }
            | crate::mir::CallKind::Resume { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR call kind",
                at: span.into(),
            }),
        }
    }

    fn codegen_mir_direct_call(
        &mut self,
        span: crate::span::Span,
        fqn: &str,
        args: &[crate::mir::CallArg],
        _body: &crate::mir::Body,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let is_extern = self.extern_funs.contains_key(fqn);
        let sig_fun =
            self.fun_index
                .get(fqn)
                .copied()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR call callee type",
                    at: span.into(),
                })?;
        if !is_extern && sig_fun.body.is_none() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR declaration-only direct call",
                at: span.into(),
            });
        }
        let call_may_suspend = self.known_fun_body_may_outward_effect(fqn, sig_fun.ty);
        let explicit_effect_call = call_may_suspend && !is_extern;

        if args.len() != sig_fun.params.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR call arity mismatch",
                at: span.into(),
            });
        }

        let ret_cg =
            self.cg_ty_of(sig_fun.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR call return type",
                    at: span.into(),
                })?;
        let hidden_sret_result_ty = if is_extern {
            None
        } else {
            self.hidden_sret_result_ty(span, ret_cg)?
        };
        let evaluated_args =
            self.codegen_bound_mir_call_args(span, sig_fun, args, slots, is_extern)?;

        let (effect_ctx_slot, effect_outcome_slot): (
            Option<PointerValue<'ctx>>,
            Option<PointerValue<'ctx>>,
        ) = if explicit_effect_call {
            let (ctx_slot, outcome_slot) =
                self.prepare_current_effect_call_contract(span, "pass_mir_direct_call")?;
            (Some(ctx_slot), Some(outcome_slot))
        } else {
            (None, None)
        };
        let mut llvm_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(
            evaluated_args.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + usize::from(explicit_effect_call) * 2,
        );
        let sret_result_slot = if hidden_sret_result_ty.is_some() {
            let slot = self.create_entry_alloca(span, "pass_mir_call_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        if let (Some(ctx_slot), Some(outcome_slot)) = (effect_ctx_slot, effect_outcome_slot) {
            llvm_args.push(ctx_slot.into());
            llvm_args.push(outcome_slot.into());
        }
        llvm_args.extend(evaluated_args.iter().map(|slot| slot.value));

        let llvm_name = self
            .extern_funs
            .get(fqn)
            .map(|e| e.symbol.as_str())
            .unwrap_or(fqn);
        let llvm_fun = if explicit_effect_call {
            self.ensure_top_level_fun_effect_call_wrapper_defined(sig_fun)?
        } else {
            match self.module.get_function(llvm_name) {
                Some(f) => f,
                None => self.declare_top_level_fun(sig_fun)?,
            }
        };

        let call_site_result = if is_extern {
            self.emit_extern_native_call(span, fqn, llvm_fun, &llvm_args)
        } else {
            self.with_conservative_gc_local_root_spills(span, |cg| {
                let call_site = cg
                    .builder
                    .build_call(llvm_fun, &llvm_args, "pass_mir_call")?;
                if let Some(result_ty) = hidden_sret_result_ty {
                    cg.add_sret_attribute_to_call(call_site, 0, result_ty);
                }
                call_site.set_call_convention(cg.llvm_call_convention_for_fqn(fqn));
                Ok(call_site)
            })
        };
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        if let Some(result_ptr) = sret_result_slot {
            self.sync_hidden_sret_result_roots(span, ret_cg, result_ptr, "pass_mir_call_sret")?;
        }
        if let Some(outcome_slot) = effect_outcome_slot {
            self.maybe_record_active_suspend_site_effect_outcome(span, outcome_slot);
            self.emit_ordinary_call_effect_propagation_check_from_outcome(
                span,
                outcome_slot,
                "pass_mir_direct_call_effect",
            )?;
        } else if call_may_suspend && !is_extern {
            self.emit_ordinary_call_effect_propagation_check(span, "pass_mir_direct_call_effect")?;
        }

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                if let Some(result_ptr) = sret_result_slot {
                    self.load_hidden_sret_result_from_ptr(
                        span,
                        ret_cg,
                        result_ptr,
                        "pass_mir_call_sret",
                    )
                } else {
                    let raw = call_site.try_as_basic_value().basic().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR call return value",
                            at: span.into(),
                        },
                    )?;
                    self.cg_value_from_loaded(span, ret_cg, raw)
                }
            }
        }
    }

    fn codegen_bound_mir_call_args(
        &mut self,
        span: crate::span::Span,
        sig_fun: &hir::FunDecl,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
        is_extern: bool,
    ) -> Result<Vec<EvaluatedCallArg<'ctx>>, LlvmEmitError> {
        let arg_to_param = map_mir_call_args_to_params(&sig_fun.params, args).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR call arg binding",
                at: span.into(),
            },
        )?;

        let mut evaluated: Vec<Option<(crate::span::Span, DeferredCgValue<'ctx>)>> =
            vec![None; sig_fun.params.len()];
        for (arg_idx, arg) in args.iter().enumerate() {
            let param_idx = arg_to_param[arg_idx];
            let param = &sig_fun.params[param_idx];
            let target_cg = self
                .cg_ty_of(param.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR call arg type",
                    at: arg.span.into(),
                })?;
            let value =
                self.codegen_mir_operand_expected(arg.span, &arg.value, slots, Some(target_cg))?;
            let coerced = self.coerce_value(arg.span, value, target_cg)?;
            let deferred = self.defer_gc_sensitive_cg_value(
                arg.span,
                &format!("pass_mir_call_arg_{param_idx}"),
                coerced,
            )?;
            evaluated[param_idx] = Some((arg.span, deferred));
        }

        evaluated
            .into_iter()
            .enumerate()
            .map(|(param_idx, slot)| {
                let (arg_span, deferred) = slot.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR call arg binding",
                    at: span.into(),
                })?;
                let param = &sig_fun.params[param_idx];
                let param_abi = if is_extern {
                    None
                } else {
                    Some(self.ordinary_param_abi(span, param.ty)?)
                };
                if let Some(abi) = param_abi
                    && abi.pointee_ty().is_some()
                {
                    let (slot_ptr, cleanup_spills) = self.deferred_gc_spill_slot_for_call_arg(
                        arg_span,
                        &format!("pass_mir_call_arg_reload_{param_idx}"),
                        deferred,
                    )?;
                    return Ok(EvaluatedCallArg {
                        value: slot_ptr.into(),
                        pointer_value: None,
                        cleanup_spills,
                    });
                }

                let (materialized, cleanup_spills) = self
                    .materialize_deferred_cg_value_for_call_arg(
                        arg_span,
                        &format!("pass_mir_call_arg_reload_{param_idx}"),
                        deferred,
                    )?;
                let pointer_value = match materialized.value {
                    Some(inkwell::values::BasicValueEnum::PointerValue(ptr)) => Some(ptr),
                    _ => None,
                };
                let param_cg = param_abi
                    .map(OrdinaryParamAbi::cg_ty)
                    .unwrap_or(materialized.ty);
                let value = self.as_llvm_arg_value(arg_span, param_cg, materialized)?;
                Ok(EvaluatedCallArg {
                    value,
                    pointer_value,
                    cleanup_spills,
                })
            })
            .collect()
    }

    fn mir_local_type_id(
        &self,
        body: &crate::mir::Body,
        local: crate::mir::LocalId,
    ) -> Option<TypeId> {
        body.locals
            .get(local.as_u32() as usize)
            .map(|local| local.ty)
    }

    fn mir_operand_type_id(
        &self,
        body: &crate::mir::Body,
        operand: &crate::mir::Operand,
    ) -> Option<TypeId> {
        match operand {
            crate::mir::Operand::Local(local) => self.mir_local_type_id(body, *local),
            crate::mir::Operand::Const(value) => Some(match value {
                crate::mir::ConstValue::Bool(_) => self.builtins.bool_,
                crate::mir::ConstValue::Char => self.builtins.char_,
                crate::mir::ConstValue::Unit => self.builtins.unit,
                crate::mir::ConstValue::Int | crate::mir::ConstValue::SynthInt(_) => {
                    self.builtins.int
                }
                crate::mir::ConstValue::Float64 => self.builtins.float64,
                crate::mir::ConstValue::Float32 => self.builtins.float32,
                crate::mir::ConstValue::String => self.builtins.string,
            }),
        }
    }

    fn mir_operand_function_type(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        operand: &crate::mir::Operand,
    ) -> Option<crate::ty::FunctionType> {
        let ty = self.mir_operand_type_id(body, operand)?;
        match mir_types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Function(fun_ty)) => {
                self.equivalent_codegen_function_type(mir_types, fun_ty)
            }
            _ => None,
        }
    }

    fn mir_closure_env_capture_element_cg_tys(&self, env_cg: CgTy) -> Option<Vec<CgTy>> {
        match env_cg {
            CgTy::Unit => Some(Vec::new()),
            CgTy::Tuple(tuple_ty) => {
                let TypeKind::Value(ValueTypeKind::Tuple(elements)) = self.types.kind(tuple_ty)
                else {
                    return None;
                };
                let mut out = Vec::with_capacity(elements.len());
                for elem_ty in elements {
                    let cg = self.cg_ty_of(*elem_ty)?;
                    if !matches!(
                        cg,
                        CgTy::Unit | CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref
                    ) {
                        return None;
                    }
                    out.push(cg);
                }
                Some(out)
            }
            _ => None,
        }
    }

    fn mir_capture_box_inner_type_id(
        &self,
        mir_types: &TypeStore,
        box_ty: TypeId,
    ) -> Option<TypeId> {
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = mir_types.kind(box_ty) else {
            return None;
        };
        if nominal.fqn != MIR_CAPTURE_BOX_FQN || nominal.args.len() != 1 || nominal.eff.is_some() {
            return None;
        }
        self.equivalent_codegen_type_id(mir_types, nominal.args[0])
    }

    fn mir_capture_box_inner_cg_ty_from_operand(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        box_operand: &crate::mir::Operand,
    ) -> Option<CgTy> {
        let box_ty = self.mir_operand_type_id(body, box_operand)?;
        let inner_ty = self.mir_capture_box_inner_type_id(mir_types, box_ty)?;
        self.cg_ty_of(inner_ty)
    }

    fn codegen_mir_make_tuple(
        &mut self,
        span: crate::span::Span,
        _body: &crate::mir::Body,
        _mir_types: &TypeStore,
        elements: &[crate::mir::Operand],
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let CgTy::Tuple(tuple_ty) = target_cg else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple target type",
                at: span.into(),
            });
        };
        let TypeKind::Value(ValueTypeKind::Tuple(element_tys)) = self.types.kind(tuple_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple type",
                at: span.into(),
            });
        };
        if element_tys.len() != elements.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple arity mismatch",
                at: span.into(),
            });
        }

        let llvm_tuple_ty = self.llvm_tuple_type(span, tuple_ty)?;
        let mut deferred_elements: Vec<(usize, crate::span::Span, DeferredCgValue<'ctx>)> =
            Vec::with_capacity(elements.len());

        for (idx, (operand, elem_ty)) in elements.iter().zip(element_tys.iter()).enumerate() {
            let elem_cg = self
                .cg_ty_of(*elem_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR tuple element type",
                    at: span.into(),
                })?;
            let value = self.codegen_mir_operand_expected(span, operand, slots, Some(elem_cg))?;
            let coerced = self.coerce_value(span, value, elem_cg)?;
            let deferred = self.defer_gc_sensitive_cg_value(
                span,
                &format!("pass_mir_tuple_elem_{idx}"),
                coerced,
            )?;
            deferred_elements.push((idx, span, deferred));
        }

        let mut agg: AggregateValueEnum<'ctx> = llvm_tuple_ty.get_undef().into();
        for (idx, elem_span, deferred) in deferred_elements {
            let materialized = self.materialize_deferred_cg_value(
                elem_span,
                &format!("pass_mir_tuple_elem_reload_{idx}"),
                deferred,
            )?;
            let raw: BasicValueEnum<'ctx> = match materialized.ty {
                CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
                _ => materialized
                    .value
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR tuple element value",
                        at: elem_span.into(),
                    })?,
            };
            agg = self
                .builder
                .build_insert_value(agg, raw, idx as u32, "pass_mir_tuple_insert")?;
        }

        Ok(CgValue {
            ty: target_cg,
            value: Some(agg.as_basic_value_enum()),
        })
    }

    fn codegen_mir_tuple_get(
        &mut self,
        span: crate::span::Span,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        tuple: &crate::mir::Operand,
        index: usize,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let tuple_ty =
            self.mir_operand_type_id(body, tuple)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR tuple operand type",
                    at: span.into(),
                })?;
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = mir_types.kind(tuple_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple operand type",
                at: span.into(),
            });
        };
        let elem_ty = *elements
            .get(index)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple index",
                at: span.into(),
            })?;
        let elem_cg = self.cg_ty_of_mir_type(mir_types, elem_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple element type",
                at: span.into(),
            },
        )?;
        let tuple_cg = self.mir_operand_cg_ty(body, mir_types, tuple).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple operand cg type",
                at: span.into(),
            },
        )?;
        let value = self.codegen_mir_operand_expected(span, tuple, slots, Some(tuple_cg))?;
        let tuple_v = value
            .value
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple operand value",
                at: span.into(),
            })?
            .into_struct_value();
        self.extract_mir_tuple_element_value(span, tuple_v, index, elem_cg)
    }

    fn codegen_mir_make_closure(
        &mut self,
        span: crate::span::Span,
        env: &crate::mir::Operand,
        fn_ptr: &str,
        env_cg: CgTy,
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if target_cg != CgTy::Ref {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure target type",
                at: span.into(),
            });
        }

        let capture_field_cgs = self.mir_closure_env_capture_element_cg_tys(env_cg).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure env shape",
                at: span.into(),
            },
        )?;

        let deferred_env = if capture_field_cgs.is_empty() {
            None
        } else {
            let value = self.codegen_mir_operand_expected(span, env, slots, Some(env_cg))?;
            let coerced = self.coerce_value(span, value, env_cg)?;
            Some(self.defer_gc_sensitive_cg_value(span, "pass_mir_closure_env", coerced)?)
        };

        let closure_obj_ty = self.llvm_closure_object_type();
        let obj_size_bytes = self.target_data.get_store_size(&closure_obj_ty);
        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);
        let closure_desc = self.get_or_create_closure_object_type_desc_global(span)?;
        let closure_desc_i8 = self.builder.build_pointer_cast(
            closure_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "pass_mir_closure_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_alloc,
            &[closure_desc_i8.into(), size_v.into()],
            "rt_alloc_pass_mir_closure",
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
        let obj_ptr =
            self.builder
                .build_pointer_cast(obj_i8, obj_ptr_ty, "pass_mir_closure_obj_ptr")?;
        let deferred_obj = self.defer_gc_ref_pointer(span, "pass_mir_closure_obj_root", obj_ptr)?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "pass_mir_closure_obj_init",
            &deferred_obj,
        )?;
        let env_gep = self.builder.build_struct_gep(
            closure_obj_ty,
            obj_ptr,
            1,
            "pass_mir_closure_env_gep",
        )?;
        let _ = self.store_local_value(
            span,
            env_gep,
            CgTy::Ref,
            CgValue {
                ty: CgTy::Ref,
                value: Some(gc_i8_ptr_ty.const_null().into()),
            },
        )?;

        let env_i8 = if capture_field_cgs.is_empty() {
            gc_i8_ptr_ty.const_null()
        } else {
            let env_ty = self.mir_closure_env_object_type(span, fn_ptr, &capture_field_cgs)?;
            let env_size_bytes = self.target_data.get_store_size(&env_ty);
            let env_size_v = self.context.i64_type().const_int(env_size_bytes, false);
            let env_desc =
                self.get_or_create_mir_closure_env_type_desc_global(span, fn_ptr, env_ty)?;
            let env_desc_i8 = self.builder.build_pointer_cast(
                env_desc.as_pointer_value(),
                self.llvm_i8_ptr_type(),
                "pass_mir_closure_env_desc_i8",
            )?;
            let call = self.build_call_preserving_gc_local_roots(
                span,
                rt_alloc,
                &[env_desc_i8.into(), env_size_v.into()],
                "rt_alloc_pass_mir_closure_env",
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
            let env_ptr =
                self.builder
                    .build_pointer_cast(env_i8, env_ptr_ty, "pass_mir_closure_env_ptr")?;
            let deferred_env_obj =
                self.defer_gc_ref_pointer(span, "pass_mir_closure_env_root", env_ptr)?;
            let env_value = self.materialize_deferred_cg_value(
                span,
                "pass_mir_closure_env_reload",
                deferred_env.expect("non-empty env must have been deferred"),
            )?;
            let tuple_v = env_value
                .value
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR closure env value",
                    at: span.into(),
                })?
                .into_struct_value();
            for (idx, field_cg) in capture_field_cgs.iter().enumerate() {
                let env_ptr = self.reload_deferred_gc_ref_without_clearing(
                    span,
                    "pass_mir_closure_env_field_reload",
                    &deferred_env_obj,
                )?;
                let field_gep = self.builder.build_struct_gep(
                    env_ty,
                    env_ptr,
                    (idx + 1) as u32,
                    "pass_mir_closure_env_field_gep",
                )?;
                let field_value =
                    self.extract_mir_tuple_element_value(span, tuple_v, idx, *field_cg)?;
                let _ = self.store_local_value(span, field_gep, *field_cg, field_value)?;
            }
            env_i8
        };
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "pass_mir_closure_obj_store_env",
            &deferred_obj,
        )?;
        let env_gep = self.builder.build_struct_gep(
            closure_obj_ty,
            obj_ptr,
            1,
            "pass_mir_closure_env_gep",
        )?;
        let _ = self.store_local_value(
            span,
            env_gep,
            CgTy::Ref,
            CgValue {
                ty: CgTy::Ref,
                value: Some(env_i8.into()),
            },
        )?;

        let llvm_fun = self.ensure_materialized_mir_closure_callable_defined(span, fn_ptr)?;
        let fn_i8 = self.builder.build_pointer_cast(
            llvm_fun.as_global_value().as_pointer_value(),
            i8_ptr_ty,
            "pass_mir_closure_fn_i8",
        )?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "pass_mir_closure_obj_store_fn",
            &deferred_obj,
        )?;
        let fn_gep =
            self.builder
                .build_struct_gep(closure_obj_ty, obj_ptr, 2, "pass_mir_closure_fn_gep")?;
        let _ = self.builder.build_store(fn_gep, fn_i8)?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "pass_mir_closure_obj_return",
            &deferred_obj,
        )?;
        let obj_i8 = self.builder.build_pointer_cast(
            obj_ptr,
            gc_i8_ptr_ty,
            "pass_mir_closure_obj_i8",
        )?;
        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(obj_i8.into()),
        })
    }

    fn codegen_mir_capture_box_new(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Operand,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if target_cg != CgTy::Ref {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR capture box target type",
                at: span.into(),
            });
        }

        let value_ty = self
            .mir_operand_type_id(body, value)
            .and_then(|ty| self.equivalent_codegen_type_id(mir_types, ty))
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR capture box value type",
                at: span.into(),
            })?;
        let value_cg = self
            .cg_ty_of(value_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR capture box value type",
                at: span.into(),
            })?;

        let deferred_value = if value_cg == CgTy::Unit {
            None
        } else {
            let value = self.codegen_mir_operand_expected(span, value, slots, Some(value_cg))?;
            let coerced = self.coerce_value(span, value, value_cg)?;
            Some(self.defer_gc_sensitive_cg_value(span, "pass_mir_capture_box_value", coerced)?)
        };

        let box_obj_ty = self.mir_capture_box_object_type(span, value_ty, value_cg)?;
        let obj_size_bytes = self.target_data.get_store_size(&box_obj_ty);
        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);
        let box_desc =
            self.get_or_create_mir_capture_box_type_desc_global(span, value_ty, box_obj_ty)?;
        let box_desc_i8 = self.builder.build_pointer_cast(
            box_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "pass_mir_capture_box_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_alloc,
            &[box_desc_i8.into(), size_v.into()],
            "rt_alloc_pass_mir_capture_box",
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

        let obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let obj_ptr =
            self.builder
                .build_pointer_cast(obj_i8, obj_ptr_ty, "pass_mir_capture_box_obj_ptr")?;
        let deferred_obj =
            self.defer_gc_ref_pointer(span, "pass_mir_capture_box_obj_root", obj_ptr)?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "pass_mir_capture_box_obj_reload",
            &deferred_obj,
        )?;
        let field_gep = self.builder.build_struct_gep(
            box_obj_ty,
            obj_ptr,
            1,
            "pass_mir_capture_box_field_gep",
        )?;
        let stored_value = deferred_value
            .map(|value| {
                self.materialize_deferred_cg_value(span, "pass_mir_capture_box_reload", value)
            })
            .transpose()?
            .unwrap_or_else(CgValue::unit);
        let _ = self.store_local_value(span, field_gep, value_cg, stored_value)?;
        let obj_i8 = self.reload_deferred_gc_ref_without_clearing(
            span,
            "pass_mir_capture_box_return",
            &deferred_obj,
        )?;
        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(obj_i8.into()),
        })
    }

    fn codegen_mir_capture_box_get(
        &mut self,
        span: crate::span::Span,
        box_operand: &crate::mir::Operand,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let value_ty = self
            .mir_operand_type_id(body, box_operand)
            .and_then(|ty| self.mir_capture_box_inner_type_id(mir_types, ty))
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR capture box operand type",
                at: span.into(),
            })?;
        let value_cg = self
            .cg_ty_of(value_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR capture box value type",
                at: span.into(),
            })?;
        let box_value =
            self.codegen_mir_operand_expected(span, box_operand, slots, Some(CgTy::Ref))?;
        let box_value = self.coerce_value(span, box_value, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(box_obj_i8)) = box_value.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR capture box operand value",
                at: span.into(),
            });
        };

        let box_obj_ty = self.mir_capture_box_object_type(span, value_ty, value_cg)?;
        let obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let obj_ptr = self.builder.build_pointer_cast(
            box_obj_i8,
            obj_ptr_ty,
            "pass_mir_capture_box_get_obj_ptr",
        )?;
        let field_gep = self.builder.build_struct_gep(
            box_obj_ty,
            obj_ptr,
            1,
            "pass_mir_capture_box_get_field_gep",
        )?;
        let loaded = if value_cg == CgTy::Unit {
            CgValue::unit()
        } else {
            let llvm_value_ty = self.llvm_basic_type_of(span, value_cg)?;
            let raw =
                self.builder
                    .build_load(llvm_value_ty, field_gep, "pass_mir_capture_box_get")?;
            self.cg_value_from_loaded(span, value_cg, raw)?
        };
        self.coerce_value(span, loaded, target_cg)
    }

    fn codegen_mir_capture_box_set(
        &mut self,
        span: crate::span::Span,
        box_operand: &crate::mir::Operand,
        value: &crate::mir::Operand,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let value_ty = self
            .mir_operand_type_id(body, box_operand)
            .and_then(|ty| self.mir_capture_box_inner_type_id(mir_types, ty))
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR capture box operand type",
                at: span.into(),
            })?;
        let value_cg = self
            .cg_ty_of(value_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR capture box value type",
                at: span.into(),
            })?;
        let box_value =
            self.codegen_mir_operand_expected(span, box_operand, slots, Some(CgTy::Ref))?;
        let box_value = self.coerce_value(span, box_value, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(box_obj_i8)) = box_value.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR capture box operand value",
                at: span.into(),
            });
        };
        let value = self.codegen_mir_operand_expected(span, value, slots, Some(value_cg))?;
        let value = self.coerce_value(span, value, value_cg)?;

        let box_obj_ty = self.mir_capture_box_object_type(span, value_ty, value_cg)?;
        let obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let obj_ptr = self.builder.build_pointer_cast(
            box_obj_i8,
            obj_ptr_ty,
            "pass_mir_capture_box_set_obj_ptr",
        )?;
        let field_gep = self.builder.build_struct_gep(
            box_obj_ty,
            obj_ptr,
            1,
            "pass_mir_capture_box_set_field_gep",
        )?;
        let _ = self.store_local_value(span, field_gep, value_cg, value)?;
        Ok(CgValue::unit())
    }

    fn codegen_mir_fun_value_call(
        &mut self,
        span: crate::span::Span,
        callee: &crate::mir::Operand,
        args: &[crate::mir::CallArg],
        fun_ty: &crate::ty::FunctionType,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let callee_value =
            self.codegen_mir_operand_expected(span, callee, slots, Some(CgTy::Ref))?;
        let callee_value = self.coerce_value(span, callee_value, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(closure_obj_i8)) = callee_value.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR function-value callee value",
                at: span.into(),
            });
        };
        let call_may_suspend = !fun_ty.effects.is_pure();
        self.codegen_mir_function_value_call_from_closure_obj(
            span,
            closure_obj_i8,
            fun_ty,
            call_may_suspend,
            args,
            slots,
        )
    }

    fn codegen_mir_closure_call(
        &mut self,
        span: crate::span::Span,
        callee: &crate::mir::Operand,
        fn_ptr: &str,
        args: &[crate::mir::CallArg],
        fun_ty: &crate::ty::FunctionType,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let callee_value =
            self.codegen_mir_operand_expected(span, callee, slots, Some(CgTy::Ref))?;
        let callee_value = self.coerce_value(span, callee_value, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(closure_obj_i8)) = callee_value.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure callee value",
                at: span.into(),
            });
        };
        let call_may_suspend = self
            .fun_index
            .get(fn_ptr)
            .copied()
            .map_or(!fun_ty.effects.is_pure(), |fun| {
                self.known_fun_body_may_outward_effect(fn_ptr, fun.ty)
            });
        self.codegen_mir_function_value_call_from_closure_obj(
            span,
            closure_obj_i8,
            fun_ty,
            call_may_suspend,
            args,
            slots,
        )
    }

    fn codegen_mir_function_value_call_from_closure_obj(
        &mut self,
        span: crate::span::Span,
        closure_obj_i8: PointerValue<'ctx>,
        fun_ty: &crate::ty::FunctionType,
        call_may_suspend: bool,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let expected_arity = fun_ty.params.len() + usize::from(fun_ty.receiver.is_some());
        if args.len() != expected_arity {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure call arity mismatch",
                at: span.into(),
            });
        }

        let closure_ty = self.llvm_closure_object_type();
        let closure_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let closure_ptr = self.builder.build_pointer_cast(
            closure_obj_i8,
            closure_ptr_ty,
            "pass_mir_closure_call_obj_ptr",
        )?;
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let env_ptr_gep = self.builder.build_struct_gep(
            closure_ty,
            closure_ptr,
            1,
            "pass_mir_closure_call_env_gep",
        )?;
        let fn_ptr_gep = self.builder.build_struct_gep(
            closure_ty,
            closure_ptr,
            2,
            "pass_mir_closure_call_fn_gep",
        )?;
        let env_ptr = self
            .builder
            .build_load(gc_i8_ptr_ty, env_ptr_gep, "pass_mir_closure_env")?
            .into_pointer_value();
        let fn_ptr_raw = self
            .builder
            .build_load(i8_ptr_ty, fn_ptr_gep, "pass_mir_closure_fn")?
            .into_pointer_value();

        let ret_cg = self
            .cg_ty_of(fun_ty.return_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure call return type",
                at: span.into(),
            })?;
        let hidden_sret_result_ty = self.hidden_sret_result_ty(span, ret_cg)?;
        let mut llvm_param_tys: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> =
            Vec::with_capacity(1 + expected_arity + usize::from(hidden_sret_result_ty.is_some()));
        if let Some(result_ty) = hidden_sret_result_ty {
            let _ = result_ty;
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        llvm_param_tys.push(gc_i8_ptr_ty.into());
        if let Some(receiver_ty) = fun_ty.receiver {
            llvm_param_tys.push(self.ordinary_param_abi(span, receiver_ty)?.llvm_param_ty());
        }
        for ty in &fun_ty.params {
            llvm_param_tys.push(self.ordinary_param_abi(span, *ty)?.llvm_param_ty());
        }
        let llvm_fun_ty = match (hidden_sret_result_ty, ret_cg) {
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
                "aggregate closure-call returns should have been lowered through hidden sret"
            ),
        };
        let typed_fn_ptr = self.builder.build_pointer_cast(
            fn_ptr_raw,
            self.llvm_ptr_type(AddressSpace::default()),
            "pass_mir_closure_fn_typed",
        )?;

        let mut llvm_args: Vec<BasicMetadataValueEnum<'ctx>> =
            Vec::with_capacity(1 + args.len() + usize::from(hidden_sret_result_ty.is_some()));
        let sret_result_slot = if hidden_sret_result_ty.is_some() {
            let slot = self.create_entry_alloca(span, "pass_mir_closure_call_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        llvm_args.push(env_ptr.into());
        let evaluated_args = self.codegen_mir_callable_value_args(span, fun_ty, args, slots)?;
        for arg in &evaluated_args {
            llvm_args.push(arg.value);
        }

        let effect_boundary = if call_may_suspend {
            let (ctx_slot, outcome_slot) =
                self.prepare_current_effect_call_contract(span, "pass_mir_closure_call")?;
            let installed_top = self.load_effect_ctx_handler_top_from_slot(
                span,
                ctx_slot,
                "pass_mir_closure_call",
            )?;
            let saved_top =
                self.swap_effect_handler_stack_top(span, installed_top, "pass_mir_closure_call")?;
            Some((outcome_slot, saved_top))
        } else {
            None
        };

        let call_site_result = self.with_conservative_gc_local_root_spills(span, |cg| {
            let call_site = cg.builder.build_indirect_call(
                llvm_fun_ty,
                typed_fn_ptr,
                &llvm_args,
                "pass_mir_call_closure",
            )?;
            if let Some(result_ty) = hidden_sret_result_ty {
                cg.add_sret_attribute_to_call(call_site, 0, result_ty);
            }
            Ok(call_site)
        });
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        if let Some(result_ptr) = sret_result_slot {
            self.sync_hidden_sret_result_roots(
                span,
                ret_cg,
                result_ptr,
                "pass_mir_closure_call_sret",
            )?;
        }
        if let Some((outcome_slot, saved_top)) = effect_boundary {
            self.consume_current_effect_outcome_into(span, outcome_slot, "pass_mir_closure_call")?;
            let _ = self.swap_effect_handler_stack_top(
                span,
                saved_top,
                "pass_mir_closure_call_restore",
            )?;
            self.maybe_record_active_suspend_site_effect_outcome(span, outcome_slot);
            self.emit_ordinary_call_effect_propagation_check_from_outcome(
                span,
                outcome_slot,
                "pass_mir_closure_call_effect",
            )?;
        } else if call_may_suspend {
            self.emit_ordinary_call_effect_propagation_check(span, "pass_mir_closure_call_effect")?;
        }

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                if let Some(result_ptr) = sret_result_slot {
                    self.load_hidden_sret_result_from_ptr(
                        span,
                        ret_cg,
                        result_ptr,
                        "pass_mir_closure_call_sret",
                    )
                } else {
                    let raw = call_site.try_as_basic_value().basic().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR closure call return value",
                            at: span.into(),
                        },
                    )?;
                    self.cg_value_from_loaded(span, ret_cg, raw)
                }
            }
        }
    }

    fn codegen_mir_callable_value_args(
        &mut self,
        span: crate::span::Span,
        fun_ty: &crate::ty::FunctionType,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<Vec<EvaluatedCallArg<'ctx>>, LlvmEmitError> {
        let param_names = self.callable_value_param_names(fun_ty);
        let param_tys = self.callable_value_param_tys(fun_ty);
        let arg_to_param = map_mir_call_args_to_param_names(&param_names, args).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure call arg binding",
                at: span.into(),
            },
        )?;

        let mut evaluated: Vec<Option<(crate::span::Span, DeferredCgValue<'ctx>)>> =
            vec![None; param_tys.len()];
        for (arg_idx, arg) in args.iter().enumerate() {
            let param_idx = arg_to_param[arg_idx];
            let target_cg =
                self.cg_ty_of(param_tys[param_idx])
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR closure call arg type",
                        at: arg.span.into(),
                    })?;
            let value =
                self.codegen_mir_operand_expected(arg.span, &arg.value, slots, Some(target_cg))?;
            let coerced = self.coerce_value(arg.span, value, target_cg)?;
            let deferred = self.defer_gc_sensitive_cg_value(
                arg.span,
                &format!("pass_mir_closure_arg_{param_idx}"),
                coerced,
            )?;
            evaluated[param_idx] = Some((arg.span, deferred));
        }

        evaluated
            .into_iter()
            .enumerate()
            .map(|(param_idx, slot)| {
                let (arg_span, deferred) = slot.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR closure call arg binding",
                    at: span.into(),
                })?;
                let param_ty = param_tys[param_idx];
                let param_abi = self.ordinary_param_abi(span, param_ty)?;
                if param_abi.pointee_ty().is_some() {
                    let (slot_ptr, cleanup_spills) = self.deferred_gc_spill_slot_for_call_arg(
                        arg_span,
                        &format!("pass_mir_closure_arg_reload_{param_idx}"),
                        deferred,
                    )?;
                    return Ok(EvaluatedCallArg {
                        value: slot_ptr.into(),
                        pointer_value: None,
                        cleanup_spills,
                    });
                }

                let (materialized, cleanup_spills) = self
                    .materialize_deferred_cg_value_for_call_arg(
                        arg_span,
                        &format!("pass_mir_closure_arg_reload_{param_idx}"),
                        deferred,
                    )?;
                let pointer_value = match materialized.value {
                    Some(inkwell::values::BasicValueEnum::PointerValue(ptr)) => Some(ptr),
                    _ => None,
                };
                let value = self.as_llvm_arg_value(arg_span, param_abi.cg_ty(), materialized)?;
                Ok(EvaluatedCallArg {
                    value,
                    pointer_value,
                    cleanup_spills,
                })
            })
            .collect()
    }

    fn mir_closure_env_object_type(
        &mut self,
        at: crate::span::Span,
        fn_ptr: &str,
        field_cgs: &[CgTy],
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let name = format!("scoop.mir.lambda_env${}", sanitize_llvm_ident(fn_ptr));
        if let Some(existing) = self.context.get_struct_type(&name) {
            return Ok(existing);
        }
        let env_ty = self.context.opaque_struct_type(&name);
        let mut fields: Vec<BasicTypeEnum<'ctx>> = Vec::with_capacity(1 + field_cgs.len());
        fields.push(self.llvm_gc_object_header_type().into());
        for cg in field_cgs {
            fields.push(self.llvm_basic_type_of(at, *cg)?);
        }
        env_ty.set_body(&fields, false);
        Ok(env_ty)
    }

    fn mir_capture_box_object_type(
        &mut self,
        at: crate::span::Span,
        value_ty: TypeId,
        value_cg: CgTy,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let name = format!(
            "scoop.mir.capture_box${}",
            sanitize_llvm_ident(&self.types.display(value_ty).to_string())
        );
        if let Some(existing) = self.context.get_struct_type(&name) {
            return Ok(existing);
        }
        let box_ty = self.context.opaque_struct_type(&name);
        let fields = [
            self.llvm_gc_object_header_type().into(),
            self.llvm_basic_type_of(at, value_cg)?,
        ];
        box_ty.set_body(&fields, false);
        Ok(box_ty)
    }

    fn get_or_create_mir_closure_env_type_desc_global(
        &mut self,
        at: crate::span::Span,
        fn_ptr: &str,
        env_ty: StructType<'ctx>,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let fn_san = sanitize_llvm_ident(fn_ptr);
        let global_name = format!("__scoop_type_desc_mir_closure_env__{fn_san}");
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(existing);
        }
        let trace_start_offset_bytes = self.target_data.offset_of_element(&env_ty, 1).unwrap_or(0);
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            canonical_name: &global_name,
            obj_ty: env_ty,
            trace_start_offset_bytes,
            parent: None,
            itable: None,
            vtable: None,
        })
    }

    fn get_or_create_mir_capture_box_type_desc_global(
        &mut self,
        at: crate::span::Span,
        value_ty: TypeId,
        box_ty: StructType<'ctx>,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let value_name = sanitize_llvm_ident(&self.types.display(value_ty).to_string());
        let global_name = format!("__scoop_type_desc_mir_capture_box__{value_name}");
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(existing);
        }
        let trace_start_offset_bytes = self.target_data.offset_of_element(&box_ty, 1).unwrap_or(0);
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            canonical_name: &global_name,
            obj_ty: box_ty,
            trace_start_offset_bytes,
            parent: None,
            itable: None,
            vtable: None,
        })
    }

    fn codegen_mir_unary(
        &mut self,
        span: crate::span::Span,
        op: ast::UnaryOp,
        operand: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match op {
            ast::UnaryOp::Not => {
                let value = operand
                    .as_bool()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR bool unary",
                        at: span.into(),
                    })?;
                Ok(CgValue::bool(
                    self.builder.build_not(value, "pass_mir_not")?,
                ))
            }
            ast::UnaryOp::Neg => {
                if let Some((value, int_ty)) = operand.as_int() {
                    return Ok(CgValue::int(
                        self.builder.build_int_neg(value, "pass_mir_neg")?,
                        int_ty,
                    ));
                }
                if let Some((value, float_ty)) = operand.as_float() {
                    return Ok(CgValue::float(
                        self.builder.build_float_neg(value, "pass_mir_fneg")?,
                        float_ty,
                    ));
                }
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR numeric unary",
                    at: span.into(),
                })
            }
            ast::UnaryOp::BitNot => {
                let (value, int_ty) =
                    operand.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR int unary",
                        at: span.into(),
                    })?;
                Ok(CgValue::int(
                    self.builder.build_not(value, "pass_mir_bitnot")?,
                    int_ty,
                ))
            }
        }
    }

    fn codegen_mir_binary(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: CgValue<'ctx>,
        rhs: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if let (Some((l, l_ty)), Some((r, r_ty))) = (lhs.as_int(), rhs.as_int()) {
            if l_ty.bits != r_ty.bits {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR int width mismatch",
                    at: span.into(),
                });
            }
            let value = match op {
                ast::BinaryOp::Add => {
                    return Ok(CgValue::int(
                        self.builder.build_int_add(l, r, "pass_mir_iadd")?,
                        l_ty,
                    ));
                }
                ast::BinaryOp::Sub => {
                    return Ok(CgValue::int(
                        self.builder.build_int_sub(l, r, "pass_mir_isub")?,
                        l_ty,
                    ));
                }
                ast::BinaryOp::Mul => {
                    return Ok(CgValue::int(
                        self.builder.build_int_mul(l, r, "pass_mir_imul")?,
                        l_ty,
                    ));
                }
                ast::BinaryOp::Div if l_ty.signed => {
                    return Ok(CgValue::int(
                        self.builder.build_int_signed_div(l, r, "pass_mir_sdiv")?,
                        l_ty,
                    ));
                }
                ast::BinaryOp::Div => {
                    return Ok(CgValue::int(
                        self.builder.build_int_unsigned_div(l, r, "pass_mir_udiv")?,
                        l_ty,
                    ));
                }
                ast::BinaryOp::Rem if l_ty.signed => {
                    return Ok(CgValue::int(
                        self.builder.build_int_signed_rem(l, r, "pass_mir_srem")?,
                        l_ty,
                    ));
                }
                ast::BinaryOp::Rem => {
                    return Ok(CgValue::int(
                        self.builder.build_int_unsigned_rem(l, r, "pass_mir_urem")?,
                        l_ty,
                    ));
                }
                ast::BinaryOp::Shl => {
                    return Ok(CgValue::int(
                        self.builder.build_left_shift(l, r, "pass_mir_shl")?,
                        l_ty,
                    ));
                }
                ast::BinaryOp::Shr if l_ty.signed => {
                    return Ok(CgValue::int(
                        self.builder
                            .build_right_shift(l, r, true, "pass_mir_ashr")?,
                        l_ty,
                    ));
                }
                ast::BinaryOp::Shr => {
                    return Ok(CgValue::int(
                        self.builder
                            .build_right_shift(l, r, false, "pass_mir_lshr")?,
                        l_ty,
                    ));
                }
                ast::BinaryOp::BitAnd => {
                    return Ok(CgValue::int(
                        self.builder.build_and(l, r, "pass_mir_iand")?,
                        l_ty,
                    ));
                }
                ast::BinaryOp::BitXor => {
                    return Ok(CgValue::int(
                        self.builder.build_xor(l, r, "pass_mir_ixor")?,
                        l_ty,
                    ));
                }
                ast::BinaryOp::BitOr => {
                    return Ok(CgValue::int(
                        self.builder.build_or(l, r, "pass_mir_ior")?,
                        l_ty,
                    ));
                }
                ast::BinaryOp::Lt => self.builder.build_int_compare(
                    int_predicate(l_ty, IntCompareKind::Lt),
                    l,
                    r,
                    "pass_mir_ilt",
                )?,
                ast::BinaryOp::Le => self.builder.build_int_compare(
                    int_predicate(l_ty, IntCompareKind::Le),
                    l,
                    r,
                    "pass_mir_ile",
                )?,
                ast::BinaryOp::Gt => self.builder.build_int_compare(
                    int_predicate(l_ty, IntCompareKind::Gt),
                    l,
                    r,
                    "pass_mir_igt",
                )?,
                ast::BinaryOp::Ge => self.builder.build_int_compare(
                    int_predicate(l_ty, IntCompareKind::Ge),
                    l,
                    r,
                    "pass_mir_ige",
                )?,
                ast::BinaryOp::Eq => {
                    self.builder
                        .build_int_compare(IntPredicate::EQ, l, r, "pass_mir_ieq")?
                }
                ast::BinaryOp::Ne => {
                    self.builder
                        .build_int_compare(IntPredicate::NE, l, r, "pass_mir_ine")?
                }
                _ => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR int binary op",
                        at: span.into(),
                    });
                }
            };
            return Ok(CgValue::bool(value));
        }

        if let (Some(l), Some(r)) = (lhs.as_bool(), rhs.as_bool()) {
            let value = match op {
                ast::BinaryOp::LogAnd | ast::BinaryOp::BitAnd => {
                    self.builder.build_and(l, r, "pass_mir_band")?
                }
                ast::BinaryOp::LogOr | ast::BinaryOp::BitOr => {
                    self.builder.build_or(l, r, "pass_mir_bor")?
                }
                ast::BinaryOp::Eq => {
                    self.builder
                        .build_int_compare(IntPredicate::EQ, l, r, "pass_mir_beq")?
                }
                ast::BinaryOp::Ne => {
                    self.builder
                        .build_int_compare(IntPredicate::NE, l, r, "pass_mir_bne")?
                }
                _ => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR bool binary op",
                        at: span.into(),
                    });
                }
            };
            return Ok(CgValue::bool(value));
        }

        if let (Some((l, l_ty)), Some((r, r_ty))) = (lhs.as_float(), rhs.as_float()) {
            if l_ty != r_ty {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR float width mismatch",
                    at: span.into(),
                });
            }
            let value = match op {
                ast::BinaryOp::Add => {
                    return Ok(CgValue::float(
                        self.builder.build_float_add(l, r, "pass_mir_fadd")?,
                        l_ty,
                    ));
                }
                ast::BinaryOp::Sub => {
                    return Ok(CgValue::float(
                        self.builder.build_float_sub(l, r, "pass_mir_fsub")?,
                        l_ty,
                    ));
                }
                ast::BinaryOp::Mul => {
                    return Ok(CgValue::float(
                        self.builder.build_float_mul(l, r, "pass_mir_fmul")?,
                        l_ty,
                    ));
                }
                ast::BinaryOp::Div => {
                    return Ok(CgValue::float(
                        self.builder.build_float_div(l, r, "pass_mir_fdiv")?,
                        l_ty,
                    ));
                }
                ast::BinaryOp::Rem => {
                    return Ok(CgValue::float(
                        self.builder.build_float_rem(l, r, "pass_mir_frem")?,
                        l_ty,
                    ));
                }
                ast::BinaryOp::Lt => {
                    self.builder
                        .build_float_compare(FloatPredicate::OLT, l, r, "pass_mir_flt")?
                }
                ast::BinaryOp::Le => {
                    self.builder
                        .build_float_compare(FloatPredicate::OLE, l, r, "pass_mir_fle")?
                }
                ast::BinaryOp::Gt => {
                    self.builder
                        .build_float_compare(FloatPredicate::OGT, l, r, "pass_mir_fgt")?
                }
                ast::BinaryOp::Ge => {
                    self.builder
                        .build_float_compare(FloatPredicate::OGE, l, r, "pass_mir_fge")?
                }
                ast::BinaryOp::Eq => {
                    self.builder
                        .build_float_compare(FloatPredicate::OEQ, l, r, "pass_mir_feq")?
                }
                ast::BinaryOp::Ne => {
                    self.builder
                        .build_float_compare(FloatPredicate::ONE, l, r, "pass_mir_fne")?
                }
                _ => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR float binary op",
                        at: span.into(),
                    });
                }
            };
            return Ok(CgValue::bool(value));
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "pass MIR binary operands",
            at: span.into(),
        })
    }

    fn mir_local_slot(
        &self,
        span: crate::span::Span,
        slots: &[MirLocalSlot<'ctx>],
        local: crate::mir::LocalId,
    ) -> Result<MirLocalSlot<'ctx>, LlvmEmitError> {
        slots
            .get(local.as_u32() as usize)
            .copied()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR local",
                at: span.into(),
            })
    }

    fn load_mir_local(
        &mut self,
        span: crate::span::Span,
        slot: MirLocalSlot<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match slot.cg_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                let local_ptr = self.local_ptr_for_use(
                    span,
                    CgLocal {
                        hir_ty: None,
                        call_may_suspend: false,
                        ty: slot.cg_ty,
                        ptr: slot.ptr,
                        mutable: false,
                    },
                    "pass_mir_load_slot",
                )?;
                let llvm_ty = self.llvm_basic_type_of(span, slot.cg_ty)?;
                let loaded = self
                    .builder
                    .build_load(llvm_ty, local_ptr, "pass_mir_load")?;
                self.cg_value_from_loaded(span, slot.cg_ty, loaded)
            }
        }
    }
}

#[derive(Clone, Copy)]
enum IntCompareKind {
    Lt,
    Le,
    Gt,
    Ge,
}

fn int_predicate(ty: IntTy, kind: IntCompareKind) -> IntPredicate {
    match (ty.signed, kind) {
        (true, IntCompareKind::Lt) => IntPredicate::SLT,
        (true, IntCompareKind::Le) => IntPredicate::SLE,
        (true, IntCompareKind::Gt) => IntPredicate::SGT,
        (true, IntCompareKind::Ge) => IntPredicate::SGE,
        (false, IntCompareKind::Lt) => IntPredicate::ULT,
        (false, IntCompareKind::Le) => IntPredicate::ULE,
        (false, IntCompareKind::Gt) => IntPredicate::UGT,
        (false, IntCompareKind::Ge) => IntPredicate::UGE,
    }
}

fn map_mir_call_args_to_params(
    params: &[hir::Param],
    args: &[crate::mir::CallArg],
) -> Option<Vec<usize>> {
    let mut used = vec![false; params.len()];
    let mut next_pos = 0usize;
    let mut out = Vec::with_capacity(args.len());

    for arg in args {
        let param_idx = match arg.name.as_deref() {
            Some(name) => params
                .iter()
                .enumerate()
                .find_map(|(idx, param)| (!used[idx] && param.name == name).then_some(idx))?,
            None => {
                while used.get(next_pos).copied().unwrap_or(false) {
                    next_pos += 1;
                }
                let idx = next_pos;
                if idx >= params.len() {
                    return None;
                }
                next_pos += 1;
                idx
            }
        };
        used[param_idx] = true;
        out.push(param_idx);
    }

    (out.len() == params.len()).then_some(out)
}

fn map_mir_call_args_to_param_names(
    param_names: &[String],
    args: &[crate::mir::CallArg],
) -> Option<Vec<usize>> {
    let mut used = vec![false; param_names.len()];
    let mut next_pos = 0usize;
    let mut out = Vec::with_capacity(args.len());

    for arg in args {
        let param_idx = match arg.name.as_deref() {
            Some(name) => param_names
                .iter()
                .enumerate()
                .find_map(|(idx, param)| (!used[idx] && param == name).then_some(idx))?,
            None => {
                while used.get(next_pos).copied().unwrap_or(false) {
                    next_pos += 1;
                }
                let idx = next_pos;
                if idx >= param_names.len() {
                    return None;
                }
                next_pos += 1;
                idx
            }
        };
        used[param_idx] = true;
        out.push(param_idx);
    }

    (out.len() == param_names.len()).then_some(out)
}

fn collect_mir_local_uses(body: &crate::mir::Body) -> HashSet<crate::mir::LocalId> {
    let mut out = HashSet::new();
    for block in &body.blocks {
        for stmt in &block.stmts {
            if let crate::mir::StatementKind::Assign { value, .. } = &stmt.kind {
                collect_mir_rvalue_uses(value, &mut out);
            }
        }
        collect_mir_terminator_uses(&block.terminator.kind, &mut out);
    }
    out
}

fn collect_mir_operand_use(operand: &crate::mir::Operand, out: &mut HashSet<crate::mir::LocalId>) {
    if let crate::mir::Operand::Local(local) = operand {
        out.insert(*local);
    }
}

fn collect_mir_call_kind_uses(kind: &crate::mir::CallKind, out: &mut HashSet<crate::mir::LocalId>) {
    match kind {
        crate::mir::CallKind::Direct { .. } => {}
        crate::mir::CallKind::Closure { callee, .. }
        | crate::mir::CallKind::FunValue { callee } => collect_mir_operand_use(callee, out),
        crate::mir::CallKind::Virtual { receiver, .. }
        | crate::mir::CallKind::Interface { receiver, .. } => {
            collect_mir_operand_use(receiver, out);
        }
        crate::mir::CallKind::Resume { continuation, .. } => {
            collect_mir_operand_use(continuation, out);
        }
    }
}

fn collect_mir_rvalue_uses(value: &crate::mir::Rvalue, out: &mut HashSet<crate::mir::LocalId>) {
    match value {
        crate::mir::Rvalue::Use(operand)
        | crate::mir::Rvalue::Unary { operand, .. }
        | crate::mir::Rvalue::TypeCheck { value: operand, .. }
        | crate::mir::Rvalue::Cast { value: operand, .. }
        | crate::mir::Rvalue::MemberAccess {
            receiver: operand, ..
        }
        | crate::mir::Rvalue::TupleGet { tuple: operand, .. }
        | crate::mir::Rvalue::CaptureBoxNew { value: operand }
        | crate::mir::Rvalue::CaptureBoxGet {
            box_operand: operand,
        }
        | crate::mir::Rvalue::PatternMatch {
            subject: operand, ..
        }
        | crate::mir::Rvalue::PatternExtract {
            subject: operand, ..
        } => collect_mir_operand_use(operand, out),
        crate::mir::Rvalue::Binary { lhs, rhs, .. } => {
            collect_mir_operand_use(lhs, out);
            collect_mir_operand_use(rhs, out);
        }
        crate::mir::Rvalue::Call { kind, args } => {
            collect_mir_call_kind_uses(kind, out);
            for arg in args {
                collect_mir_operand_use(&arg.value, out);
            }
        }
        crate::mir::Rvalue::MakeTuple { elements } => {
            for element in elements {
                collect_mir_operand_use(element, out);
            }
        }
        crate::mir::Rvalue::CaptureBoxSet { box_operand, value } => {
            collect_mir_operand_use(box_operand, out);
            collect_mir_operand_use(value, out);
        }
        crate::mir::Rvalue::MakeClosure { env, .. } => collect_mir_operand_use(env, out),
        crate::mir::Rvalue::TopLevelRef(_)
        | crate::mir::Rvalue::UnresolvedName { .. }
        | crate::mir::Rvalue::PerformResult { .. }
        | crate::mir::Rvalue::Todo(_) => {}
    }
}

fn collect_mir_terminator_uses(
    terminator: &crate::mir::TerminatorKind,
    out: &mut HashSet<crate::mir::LocalId>,
) {
    match terminator {
        crate::mir::TerminatorKind::Return { value } => {
            if let Some(value) = value {
                collect_mir_operand_use(value, out);
            }
        }
        crate::mir::TerminatorKind::CondBr { cond, .. } => collect_mir_operand_use(cond, out),
        crate::mir::TerminatorKind::Perform { args, .. } => {
            for arg in args {
                collect_mir_operand_use(&arg.value, out);
            }
        }
        crate::mir::TerminatorKind::ResumeUnwind
        | crate::mir::TerminatorKind::Goto { .. }
        | crate::mir::TerminatorKind::Unreachable
        | crate::mir::TerminatorKind::Handle { .. }
        | crate::mir::TerminatorKind::Todo(_) => {}
    }
}
