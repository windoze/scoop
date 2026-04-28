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

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(crate) fn raw_materialized_mir_body_requires_hir_compat_boundary(
        &self,
        hir_fun: &hir::FunDecl,
        mir_fun: &crate::mir::FunDecl,
    ) -> bool {
        if self.build_fun_callee_suspend_plan(hir_fun).is_some() {
            return true;
        }
        let Some(body) = mir_fun.body.as_ref() else {
            return true;
        };
        body.validate_cfg().is_err() || !self.raw_materialized_mir_body_is_supported(body)
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
                self.codegen_mir_statement(stmt, body, &local_slots, &used_locals)?;
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
        self.function_cx.current_sret_return_ptr = None;
        Ok(())
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

    fn raw_materialized_mir_body_is_supported(&self, body: &crate::mir::Body) -> bool {
        let used_locals = collect_mir_local_uses(body);
        body.blocks.iter().all(|block| {
            block
                .stmts
                .iter()
                .all(|stmt| self.raw_materialized_mir_statement_is_supported(stmt, &used_locals))
                && self.raw_materialized_mir_terminator_is_supported(&block.terminator.kind)
        })
    }

    fn raw_materialized_mir_statement_is_supported(
        &self,
        stmt: &crate::mir::Statement,
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
                self.raw_materialized_mir_rvalue_is_supported(value)
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
                .is_none_or(|operand| self.raw_materialized_mir_operand_is_supported(operand)),
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

    fn raw_materialized_mir_rvalue_is_supported(&self, value: &crate::mir::Rvalue) -> bool {
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
                self.raw_materialized_mir_call_kind_is_supported(kind)
                    && args
                        .iter()
                        .all(|arg| self.raw_materialized_mir_operand_is_supported(&arg.value))
            }
            crate::mir::Rvalue::UnresolvedName { .. }
            | crate::mir::Rvalue::TypeCheck { .. }
            | crate::mir::Rvalue::Cast { .. }
            | crate::mir::Rvalue::MemberAccess { .. }
            | crate::mir::Rvalue::MakeTuple { .. }
            | crate::mir::Rvalue::TupleGet { .. }
            | crate::mir::Rvalue::CaptureBoxNew { .. }
            | crate::mir::Rvalue::CaptureBoxGet { .. }
            | crate::mir::Rvalue::CaptureBoxSet { .. }
            | crate::mir::Rvalue::PatternMatch { .. }
            | crate::mir::Rvalue::PatternExtract { .. }
            | crate::mir::Rvalue::MakeClosure { .. }
            | crate::mir::Rvalue::PerformResult { .. }
            | crate::mir::Rvalue::Todo(_) => false,
        }
    }

    fn raw_materialized_mir_call_kind_is_supported(&self, kind: &crate::mir::CallKind) -> bool {
        match kind {
            crate::mir::CallKind::Direct { callee_fqn } => self.fun_index.contains_key(callee_fqn),
            crate::mir::CallKind::Closure { .. }
            | crate::mir::CallKind::FunValue { .. }
            | crate::mir::CallKind::Virtual { .. }
            | crate::mir::CallKind::Interface { .. }
            | crate::mir::CallKind::Resume { .. } => false,
        }
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

    fn bind_mir_params(
        &mut self,
        mir_fun: &crate::mir::FunDecl,
        llvm_fun: FunctionValue<'ctx>,
        param_offset: u32,
        slots: &mut [MirLocalSlot<'ctx>],
    ) -> Result<(), LlvmEmitError> {
        for (idx, param) in mir_fun.params.iter().enumerate() {
            let slot = slots.get(param.local.as_u32() as usize).copied().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR param local",
                    at: param.span.into(),
                },
            )?;
            let init = self.cg_value_from_llvm_param(
                param.span,
                llvm_fun,
                idx as u32 + param_offset,
                slot.cg_ty,
                "missing pass MIR llvm param",
            )?;
            let _ = self.store_local_value(param.span, slot.ptr, slot.cg_ty, init)?;
        }
        Ok(())
    }

    fn codegen_mir_statement(
        &mut self,
        stmt: &crate::mir::Statement,
        body: &crate::mir::Body,
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
                let value = self.codegen_mir_rvalue(stmt.span, value, body, slots, slot.cg_ty)?;
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
                self.codegen_mir_call(span, kind, args, body, slots)
            }
            crate::mir::Rvalue::UnresolvedName { .. }
            | crate::mir::Rvalue::TypeCheck { .. }
            | crate::mir::Rvalue::Cast { .. }
            | crate::mir::Rvalue::MemberAccess { .. }
            | crate::mir::Rvalue::MakeTuple { .. }
            | crate::mir::Rvalue::TupleGet { .. }
            | crate::mir::Rvalue::CaptureBoxNew { .. }
            | crate::mir::Rvalue::CaptureBoxGet { .. }
            | crate::mir::Rvalue::CaptureBoxSet { .. }
            | crate::mir::Rvalue::PatternMatch { .. }
            | crate::mir::Rvalue::PatternExtract { .. }
            | crate::mir::Rvalue::MakeClosure { .. }
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

    fn codegen_mir_call(
        &mut self,
        span: crate::span::Span,
        kind: &crate::mir::CallKind,
        args: &[crate::mir::CallArg],
        body: &crate::mir::Body,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match kind {
            crate::mir::CallKind::Direct { callee_fqn } => {
                self.codegen_mir_direct_call(span, callee_fqn, args, body, slots)
            }
            crate::mir::CallKind::Closure { .. }
            | crate::mir::CallKind::FunValue { .. }
            | crate::mir::CallKind::Virtual { .. }
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
        let sret_root_slot_ids = if let Some(result_ptr) = sret_result_slot {
            self.register_hidden_sret_result_roots(span, ret_cg, result_ptr, "pass_mir_call_sret")?
        } else {
            Vec::new()
        };
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
                let result = if let Some(result_ptr) = sret_result_slot {
                    self.load_sret_result_from_ptr(span, ret_cg, result_ptr)
                } else {
                    let raw = call_site.try_as_basic_value().basic().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR call return value",
                            at: span.into(),
                        },
                    )?;
                    self.cg_value_from_loaded(span, ret_cg, raw)
                };
                self.release_gc_root_slot_ids(&sret_root_slot_ids);
                result
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
                    let (slot_ptr, cleanup_root_slot_ids) = self
                        .deferred_gc_spill_slot_for_call_arg(
                            arg_span,
                            &format!("pass_mir_call_arg_reload_{param_idx}"),
                            deferred,
                        )?;
                    return Ok(EvaluatedCallArg {
                        value: slot_ptr.into(),
                        pointer_value: None,
                        cleanup_root_slot_ids,
                    });
                }

                let (materialized, cleanup_root_slot_ids) = self
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
                    cleanup_root_slot_ids,
                })
            })
            .collect()
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
                let llvm_ty = self.llvm_basic_type_of(span, slot.cg_ty)?;
                let loaded = self
                    .builder
                    .build_load(llvm_ty, slot.ptr, "pass_mir_load")?;
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
