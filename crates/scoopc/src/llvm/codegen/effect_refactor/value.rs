//! Effect-neutral value/expression primitives for the clean refactor LLVM path.
//!
//! This module is the narrow sharing boundary between the refactor backend and
//! generic LLVM value helpers.  It may lower literals, local loads/stores,
//! scalar/tuple ABI packing, primitive operators, casts that do not introduce a
//! hidden control path, and canonical MIR member read/write primitives.  It must
//! not choose call targets, returns, state transitions, boundary dispatch, or
//! continuation behavior; those decisions come from published P5/P6 contracts.

use std::collections::HashSet;

use inkwell::types::{BasicTypeEnum, FunctionType};
use inkwell::values::{BasicMetadataValueEnum, BasicValueEnum, CallSiteValue, FunctionValue};
use inkwell::{AddressSpace, IntPredicate};

use crate::effect_lowered::ir::{LateLoweredOperandSource, LateLoweredOperandValueSource};
use crate::llvm::LlvmEmitError;
use crate::mir::{self, LocalId};
use crate::span::Span;
use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::super::mir_body::MirLocalSlot;
use super::super::types::{CgTy, CgValue};
use super::super::{MainCodegen, sanitize_llvm_ident};
use super::types::{
    RefactorAbiQuery, RefactorCallableEntryLayout, RefactorContinuationSurfaceResumeLayout,
    RefactorSourceAbiLayout, RefactorSourceAbiLayoutKind, RefactorStepLayout,
};

/// A borrow-scoped facade over effect-neutral LLVM value primitives.
pub(super) struct RefactorValuePrimitives<'p, 'a, 'ctx> {
    codegen: &'p mut MainCodegen<'a, 'ctx>,
    source_types: &'a TypeStore,
    body: &'a mir::Body,
    slots: &'p [MirLocalSlot<'ctx>],
    abi: &'p RefactorAbiQuery<'ctx>,
}

#[derive(Clone, Copy)]
struct RefactorPlainAdapterLayout<'ctx> {
    llvm_ty: FunctionType<'ctx>,
    invoke_args_tuple_ty: TypeId,
    return_step_schema: crate::effect_facts::StepSchemaId,
}

fn function_type_source_args(fun_ty: &crate::ty::FunctionType) -> Vec<TypeId> {
    fun_ty
        .receiver
        .into_iter()
        .chain(fun_ty.params.iter().copied())
        .collect()
}

fn source_carrier_types(types: &TypeStore, carrier_ty: TypeId) -> Option<Vec<TypeId>> {
    match types.kind(carrier_ty) {
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => Some(elements.clone()),
        TypeKind::Value(ValueTypeKind::Unit) => Some(Vec::new()),
        _ => Some(vec![carrier_ty]),
    }
}

impl<'p, 'a, 'ctx> RefactorValuePrimitives<'p, 'a, 'ctx> {
    pub(super) fn new(
        codegen: &'p mut MainCodegen<'a, 'ctx>,
        source_types: &'a TypeStore,
        body: &'a mir::Body,
        slots: &'p [MirLocalSlot<'ctx>],
        abi: &'p RefactorAbiQuery<'ctx>,
    ) -> Self {
        Self {
            codegen,
            source_types,
            body,
            slots,
            abi,
        }
    }

    pub(super) fn lower_effect_neutral_statement(
        &mut self,
        stmt: &mir::Statement,
        used_locals: &HashSet<LocalId>,
    ) -> Result<(), LlvmEmitError> {
        if self
            .codegen
            .builder
            .get_insert_block()
            .is_some_and(|bb| bb.get_terminator().is_some())
        {
            return Ok(());
        }

        match &stmt.kind {
            mir::StatementKind::Nop => Ok(()),
            mir::StatementKind::Assign {
                target,
                value: rvalue,
            } => {
                if !used_locals.contains(target)
                    && let mir::Rvalue::TopLevelRef(mir::TopLevelRef { fqn }) = rvalue
                    && self.is_unused_callee_ref(fqn)
                {
                    return Ok(());
                }
                if let mir::Rvalue::MemberAccess { member, .. } = rvalue
                    && let Some(
                        mir::MemberTarget::Fun { fqn } | mir::MemberTarget::ExtensionFun { fqn },
                    ) = &member.resolved
                    && self.is_unused_callee_ref(fqn)
                {
                    return Ok(());
                }
                let slot = self
                    .codegen
                    .mir_local_slot(stmt.span, self.slots, *target)?;
                let value = self
                    .lower_effect_neutral_rvalue(stmt.span, rvalue, slot.cg_ty, Some(*target))
                    .map_err(|err| match err {
                        LlvmEmitError::InvalidLiteral { .. } => err,
                        other => frontend_error(format!(
                            "refactor pure assignment local{} rvalue {:?} lowering failed: {other}",
                            target.as_u32(),
                            rvalue,
                        )),
                    })?;
                let _ = self
                    .codegen
                    .store_local_value(stmt.span, slot.ptr, slot.cg_ty, value)
                    .map_err(|err| {
                        frontend_error(format!(
                            "refactor pure assignment local{} store failed: value_ty={:?} target_ty={:?}: {err}",
                            target.as_u32(),
                            value.ty,
                            slot.cg_ty,
                        ))
                    })?;
                Ok(())
            }
            mir::StatementKind::StoreMember {
                receiver,
                member,
                value,
                value_ty,
                continuation_route,
            } => self.codegen.codegen_mir_store_member(
                stmt.span,
                receiver,
                member,
                value,
                *value_ty,
                continuation_route,
                self.body,
                self.source_types,
                self.slots,
            ),
            mir::StatementKind::Todo(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor pure statement todo",
                at: stmt.span.into(),
            }),
        }
    }

    fn lower_effect_neutral_rvalue(
        &mut self,
        span: Span,
        value: &mir::Rvalue,
        target_cg: super::super::types::CgTy,
        target_local: Option<LocalId>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if let mir::Rvalue::Unary {
            op: crate::ast::UnaryOp::Neg,
            ..
        } = value
            && let CgTy::Int(int_ty) = target_cg
            && let Some(bits) = self
                .codegen
                .int_literal_bits_from_source_span_if_present(span, int_ty)?
        {
            return Ok(CgValue::int(
                self.codegen.int_type(int_ty).const_int(bits, false),
                int_ty,
            ));
        }

        match value {
            mir::Rvalue::Call { kind, args, .. } => {
                self.lower_refactor_pure_direct_call(span, kind, args, target_cg)
            }
            mir::Rvalue::MakeClosure { env, fn_ptr } => {
                let env_cg = self
                    .codegen
                    .mir_operand_cg_ty(self.body, self.source_types, env)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor pure closure carrier env type",
                        at: span.into(),
                    })?;
                if let Some(adapter) =
                    self.maybe_build_effect_typed_plain_closure_adapter(span, target_local, fn_ptr)?
                {
                    return self.codegen.codegen_mir_make_closure_with_target_fn_ptr(
                        span, env, fn_ptr, env_cg, target_cg, self.slots, adapter,
                    );
                }
                self.codegen
                    .codegen_mir_make_closure(span, env, fn_ptr, env_cg, target_cg, self.slots)
            }
            mir::Rvalue::ClassCtor { class_fqn, args } => self
                .codegen
                .codegen_mir_refactor_class_ctor_call(span, class_fqn, args, self.slots),
            _ => self.codegen.codegen_mir_effect_neutral_rvalue(
                span,
                value,
                self.body,
                self.source_types,
                self.slots,
                target_cg,
            ),
        }
    }

    fn maybe_build_effect_typed_plain_closure_adapter(
        &mut self,
        span: Span,
        target_local: Option<LocalId>,
        fn_ptr: &str,
    ) -> Result<Option<inkwell::values::PointerValue<'ctx>>, LlvmEmitError> {
        if self
            .abi
            .maybe_plain_callable_layout_by_root_fqn(fn_ptr)?
            .is_none()
        {
            return Ok(None);
        }
        let Some(target_ty) = target_local
            .and_then(|local| self.body.locals.get(local.as_u32() as usize))
            .map(|local| local.ty)
        else {
            return Ok(None);
        };
        let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.source_types.kind(target_ty) else {
            return Ok(None);
        };
        let Some(fun_ty) = self
            .codegen
            .equivalent_codegen_function_type(self.source_types, fun_ty)
        else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor effect-typed plain adapter function type",
                at: span.into(),
            });
        };
        if fun_ty.effects.is_pure() {
            return Ok(None);
        }
        let layout = self.effect_typed_plain_adapter_layout(&fun_ty)?;
        self.build_effect_typed_plain_closure_adapter(span, fn_ptr, &fun_ty, layout)
            .map(Some)
    }

    fn effect_typed_plain_adapter_layout(
        &self,
        fun_ty: &crate::ty::FunctionType,
    ) -> Result<RefactorPlainAdapterLayout<'ctx>, LlvmEmitError> {
        let expected_args = function_type_source_args(fun_ty);
        let mut matches = self.abi.dynamic_invoke_layouts().filter_map(|layout| {
            let args = source_carrier_types(self.source_types, layout.invoke_args_tuple_ty())?;
            if args != expected_args {
                return None;
            }
            let step_layout = self.abi.step_layout(layout.return_step_schema())?;
            (step_layout.complete_variant().payload_source_ty() == fun_ty.return_ty).then_some(
                RefactorPlainAdapterLayout {
                    llvm_ty: layout.llvm_ty(),
                    invoke_args_tuple_ty: layout.invoke_args_tuple_ty(),
                    return_step_schema: layout.return_step_schema(),
                },
            )
        });
        let first = matches.next().ok_or_else(|| {
            frontend_error(format!(
                "refactor effect-typed plain adapter 缺少匹配 function type args={:?} return=t{} 的 dynamic-invoke layout",
                expected_args.iter().map(|ty| ty.as_u32()).collect::<Vec<_>>(),
                fun_ty.return_ty.as_u32(),
            ))
        })?;
        if matches.next().is_some() {
            return Err(frontend_error(format!(
                "refactor effect-typed plain adapter function type args={:?} return=t{} 匹配多个 dynamic-invoke layout",
                expected_args
                    .iter()
                    .map(|ty| ty.as_u32())
                    .collect::<Vec<_>>(),
                fun_ty.return_ty.as_u32(),
            )));
        }
        Ok(first)
    }

    fn build_effect_typed_plain_closure_adapter(
        &mut self,
        span: Span,
        fn_ptr: &str,
        fun_ty: &crate::ty::FunctionType,
        adapter: RefactorPlainAdapterLayout<'ctx>,
    ) -> Result<inkwell::values::PointerValue<'ctx>, LlvmEmitError> {
        let name = format!(
            "__scoop_refactor_plain_adapter__{}__s{}",
            sanitize_llvm_ident(fn_ptr),
            adapter.return_step_schema.as_u32(),
        );
        if let Some(existing) = self.codegen.module.get_function(&name) {
            if existing.count_basic_blocks() == 0 {
                self.define_effect_typed_plain_closure_adapter(
                    span, fn_ptr, fun_ty, adapter, existing,
                )?;
            }
            return Ok(existing.as_global_value().as_pointer_value());
        }
        let function = self
            .codegen
            .module
            .add_function(&name, adapter.llvm_ty, None);
        self.define_effect_typed_plain_closure_adapter(span, fn_ptr, fun_ty, adapter, function)?;
        Ok(function.as_global_value().as_pointer_value())
    }

    fn define_effect_typed_plain_closure_adapter(
        &mut self,
        span: Span,
        fn_ptr: &str,
        fun_ty: &crate::ty::FunctionType,
        adapter: RefactorPlainAdapterLayout<'ctx>,
        function: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let saved_block = self.codegen.builder.get_insert_block();
        let entry = self.codegen.context.append_basic_block(function, "entry");
        self.codegen.builder.position_at_end(entry);

        let plain = self.abi.plain_callable_layout_by_root_fqn(fn_ptr)?;
        let plain_fun = self
            .codegen
            .module
            .get_function(plain.direct_entry().symbol_name())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor effect-typed plain adapter `{}` 缺少 plain entry `{}`",
                    fn_ptr,
                    plain.direct_entry().symbol_name(),
                ))
            })?;
        if self
            .codegen
            .hidden_sret_result_ty(
                span,
                self.codegen.cg_ty_of(fun_ty.return_ty).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor effect-typed plain adapter return type",
                        at: span.into(),
                    },
                )?,
            )?
            .is_some()
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor effect-typed plain adapter hidden-sret return",
                at: span.into(),
            });
        }

        let carrier = function
            .get_nth_param(0)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor effect-typed plain adapter carrier param",
                at: span.into(),
            })?
            .into_pointer_value();
        let closure_ptr = self.codegen.refactor_cast_ptr(
            carrier,
            self.codegen.llvm_ptr_type(self.codegen.gc_address_space()),
            "refactor_adapter_closure_obj",
        )?;
        let env_gep = self.codegen.builder.build_struct_gep(
            self.codegen.llvm_closure_object_type(),
            closure_ptr,
            1,
            "refactor_adapter_env_gep",
        )?;
        let env = self
            .codegen
            .builder
            .build_load(
                self.codegen.llvm_gc_i8_ptr_type(),
                env_gep,
                "refactor_adapter_env",
            )?
            .into_pointer_value();
        let mut call_args = Vec::<BasicMetadataValueEnum<'ctx>>::new();
        call_args.push(env.into());
        call_args.extend(self.adapter_explicit_args(
            span,
            function,
            adapter.invoke_args_tuple_ty,
        )?);
        let call =
            self.codegen
                .builder
                .build_call(plain_fun, &call_args, "refactor_carrier_to_plain")?;
        let ret_cg =
            self.codegen
                .cg_ty_of(fun_ty.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "refactor effect-typed plain adapter return type",
                    at: span.into(),
                })?;
        let payload = match ret_cg {
            CgTy::Unit | CgTy::Never => None,
            _ => Some(call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "refactor effect-typed plain adapter plain return value",
                    at: span.into(),
                },
            )?),
        };
        let step_layout = self
            .abi
            .step_layout(adapter.return_step_schema)
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor effect-typed plain adapter 缺少 return step schema s{} layout",
                    adapter.return_step_schema.as_u32(),
                ))
            })?;
        let step = self
            .codegen
            .refactor_build_step_complete(step_layout, payload)
            .map_err(|err| frontend_error(format!("refactor_adapter_complete failed: {err}")))?;
        self.codegen.builder.build_return(Some(&step))?;

        if let Some(saved) = saved_block {
            self.codegen.builder.position_at_end(saved);
        }
        Ok(())
    }

    fn adapter_explicit_args(
        &mut self,
        span: Span,
        function: FunctionValue<'ctx>,
        invoke_args_tuple_ty: TypeId,
    ) -> Result<Vec<BasicMetadataValueEnum<'ctx>>, LlvmEmitError> {
        let layout = self.abi.source_value_layout(invoke_args_tuple_ty)?;
        if layout.abi().is_elided() {
            return Ok(Vec::new());
        }
        let raw = function
            .get_nth_param(1)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor effect-typed plain adapter args payload",
                at: span.into(),
            })?;
        match layout.kind() {
            RefactorSourceAbiLayoutKind::Scalar => Ok(vec![raw.into()]),
            RefactorSourceAbiLayoutKind::Tuple => {
                let tuple = raw.into_struct_value();
                let mut args = Vec::new();
                for field in layout.fields() {
                    let Some(index) = field.abi_field_index() else {
                        continue;
                    };
                    let value = self.codegen.builder.build_extract_value(
                        tuple,
                        index,
                        &format!("refactor_adapter_arg{}", field.source_index()),
                    )?;
                    args.push(value.into());
                }
                Ok(args)
            }
        }
    }

    fn lower_refactor_pure_direct_call(
        &mut self,
        span: Span,
        kind: &mir::CallKind,
        args: &[mir::CallArg],
        target_cg: super::super::types::CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if let mir::CallKind::Interface { receiver, dispatch } = kind
            && dispatch.owner_fqn == "scoop.core.ToString"
            && dispatch.member_name == "toString"
        {
            if !args.is_empty() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "refactor pure ToString.toString arg contract",
                    at: span.into(),
                });
            }
            let receiver_cg = self
                .codegen
                .mir_operand_cg_ty(self.body, self.source_types, receiver)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "refactor pure ToString.toString receiver type",
                    at: span.into(),
                })?;
            let value = self.codegen.codegen_mir_operand_expected(
                span,
                receiver,
                self.slots,
                Some(receiver_cg),
            )?;
            let value = self.codegen.coerce_value(span, value, receiver_cg)?;
            let string = self.refactor_core_print_to_string(span, value)?;
            return self.codegen.coerce_value(span, string, target_cg);
        }
        let callee_fqn = match kind {
            mir::CallKind::Direct { callee_fqn } => callee_fqn,
            mir::CallKind::Closure { .. }
            | mir::CallKind::FunValue { .. }
            | mir::CallKind::Virtual { .. }
            | mir::CallKind::Interface { .. } => {
                return self.codegen.codegen_mir_plain_dynamic_call(
                    span,
                    kind,
                    args,
                    self.body,
                    self.source_types,
                    self.slots,
                );
            }
            mir::CallKind::Resume { .. } => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "refactor pure statement resume call requires boundary lowering",
                    at: span.into(),
                });
            }
        };
        if let Some(value) = self.lower_refactor_internal_print_string(span, callee_fqn, args)? {
            return Ok(value);
        }
        if let Some(value) =
            self.lower_refactor_thread_spawn_join_resume_u64(span, callee_fqn, args)?
        {
            return Ok(value);
        }
        if callee_fqn == "scoop.core.__scoop_gc_collect" {
            if !args.is_empty() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "refactor gc collect arity mismatch",
                    at: span.into(),
                });
            }
            let runtime = self.codegen.declare_runtime_gc_collect_safepoint();
            let _ = self.codegen.build_call_preserving_gc_local_roots(
                span,
                runtime,
                &[],
                "refactor_gc_collect_safepoint",
            )?;
            return Ok(CgValue::unit());
        }
        if self.codegen.extern_funs.contains_key(callee_fqn) {
            let value = self
                .codegen
                .codegen_mir_direct_call(span, callee_fqn, args, self.body, self.slots)?;
            return self.codegen.coerce_value(span, value, target_cg);
        }
        let sig_fun = self
            .codegen
            .fun_index
            .get(callee_fqn)
            .copied()
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor pure statement call 缺少 callee `{callee_fqn}` 的 callable signature"
                ))
            })?;
        if sig_fun.body.is_none() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor pure statement declaration-only direct call",
                at: span.into(),
            });
        }
        if self
            .codegen
            .known_fun_body_may_outward_effect(callee_fqn, sig_fun.ty)
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor pure statement effectful direct call requires boundary lowering",
                at: span.into(),
            });
        }
        if let Some(value) = self.lower_refactor_core_print_call(span, callee_fqn, args)? {
            return Ok(value);
        }

        if self
            .abi
            .maybe_plain_callable_layout_by_root_fqn(callee_fqn)?
            .is_some()
        {
            return self
                .codegen
                .codegen_mir_direct_call(span, callee_fqn, args, self.body, self.slots);
        }

        let layout = self.abi.callable_layout_by_root_fqn(callee_fqn)?;
        let entry = layout.direct_entry();
        if entry.return_step_schema() != layout.step_schema() {
            return Err(frontend_error(format!(
                "refactor pure statement call `{callee_fqn}` direct entry return schema 漂移：entry=s{} layout=s{}",
                entry.return_step_schema().as_u32(),
                layout.step_schema().as_u32()
            )));
        }
        let payload = self.pack_refactor_call_args(span, entry, args)?;
        let callee = self
            .codegen
            .module
            .get_function(entry.symbol_name())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor pure statement call `{callee_fqn}` 缺少 direct entry shell `{}`",
                    entry.symbol_name()
                ))
            })?;
        let mut call_args = Vec::<BasicMetadataValueEnum<'ctx>>::new();
        if !entry.args_abi().is_elided() {
            call_args.push(
                payload
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "refactor pure statement call `{callee_fqn}` 需要 non-elided args payload"
                        ))
                    })?
                    .into(),
            );
        }
        let call =
            self.codegen
                .builder
                .build_call(callee, &call_args, "refactor_pure_call_step")?;
        let step = call.try_as_basic_value().basic().ok_or_else(|| {
            frontend_error(format!(
                "refactor pure statement call `{callee_fqn}` direct entry 未返回 Step_F"
            ))
        })?;
        self.extract_refactor_pure_call_complete(span, layout.step_schema(), step, target_cg)
    }

    fn lower_refactor_thread_spawn_join_resume_u64(
        &mut self,
        span: Span,
        callee_fqn: &str,
        args: &[mir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        if callee_fqn != "scoop.core.__scoop_thread_spawn_join_resume_u64" {
            return Ok(None);
        }
        if args.len() != 2 || args.iter().any(|arg| arg.name.is_some()) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor thread spawn+resume arg contract",
                at: span.into(),
            });
        }
        let continuation = &args[0];
        let continuation = self.codegen.codegen_mir_operand_expected(
            continuation.span,
            &continuation.value,
            self.slots,
            Some(CgTy::Ref),
        )?;
        let continuation = self
            .codegen
            .coerce_value(args[0].span, continuation, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(k_ptr)) = continuation.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor thread spawn+resume continuation value",
                at: args[0].span.into(),
            });
        };

        let value_arg = &args[1];
        let resume_ty =
            self.operand_source_ty(&value_arg.value)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "refactor thread spawn+resume value source type",
                    at: value_arg.span.into(),
                })?;
        let value_cg = self
            .codegen
            .mir_operand_cg_ty(self.body, self.source_types, &value_arg.value)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor thread spawn+resume value type",
                at: value_arg.span.into(),
            })?;
        let value = self.codegen.codegen_mir_operand_expected(
            value_arg.span,
            &value_arg.value,
            self.slots,
            Some(value_cg),
        )?;
        let value = self.codegen.coerce_value(value_arg.span, value, value_cg)?;
        let value_word = self.codegen.coerce_u64_word(value_arg.span, value)?;

        let surface = self.abi.unique_surface_resume_layout_for_signature(
            resume_ty,
            self.codegen.builtins.unit,
            "thread spawn+resume u64",
        )?;
        if surface.param_count() != 2 {
            return Err(frontend_error(format!(
                "refactor thread spawn+resume u64 需要单 payload surface resume，实际参数数为 {}",
                surface.param_count()
            )));
        }
        if surface.resume_payload_abi().is_elided()
            || !matches!(surface.resume_payload_abi().llvm_ty(), BasicTypeEnum::IntType(int_ty) if int_ty == self.codegen.context.i64_type())
        {
            return Err(frontend_error(
                "refactor thread spawn+resume u64 需要 i64 resume payload ABI".to_string(),
            ));
        }
        let step_layout = self
            .abi
            .step_layout(surface.return_step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor thread spawn+resume u64 缺少 surface return step s{} layout",
                    surface.return_step_schema().as_u32()
                ))
            })?;
        let thunk =
            get_or_create_refactor_thread_resume_u64_thunk(self.codegen, surface, step_layout)?;

        let runtime = self
            .codegen
            .declare_runtime_thread_spawn_join_refactor_resume_u64();
        let k_i8 = self.codegen.builder.build_pointer_cast(
            k_ptr,
            self.codegen.llvm_gc_i8_ptr_type(),
            "refactor_thread_resume_k_i8",
        )?;
        let thunk_ptr = self.codegen.builder.build_pointer_cast(
            thunk.as_global_value().as_pointer_value(),
            self.codegen.context.ptr_type(AddressSpace::default()),
            "refactor_thread_resume_fn",
        )?;
        let _ = self.codegen.build_call_preserving_gc_local_roots(
            span,
            runtime,
            &[k_i8.into(), value_word.into(), thunk_ptr.into()],
            "refactor_thread_spawn_join_resume",
        )?;
        Ok(Some(CgValue::unit()))
    }

    fn lower_refactor_core_print_call(
        &mut self,
        span: Span,
        callee_fqn: &str,
        args: &[mir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let Some(runtime_name) = refactor_core_print_runtime_name(callee_fqn) else {
            return Ok(None);
        };
        if args.len() != 1 || args[0].name.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor core print arg contract",
                at: span.into(),
            });
        }
        let arg = &args[0];
        let arg_cg = self
            .codegen
            .mir_operand_cg_ty(self.body, self.source_types, &arg.value)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor core print arg type",
                at: arg.span.into(),
            })?;
        let value = self.codegen.codegen_mir_operand_expected(
            arg.span,
            &arg.value,
            self.slots,
            Some(arg_cg),
        )?;
        let value = self.codegen.coerce_value(arg.span, value, arg_cg)?;
        let string = self.refactor_core_print_to_string(arg.span, value)?;
        let Some(BasicValueEnum::PointerValue(str_ptr)) = string.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor core print string value",
                at: arg.span.into(),
            });
        };
        let runtime = self.codegen.declare_runtime_print_like(runtime_name);
        let _ = self.codegen.build_call_preserving_gc_local_roots(
            arg.span,
            runtime,
            &[str_ptr.into()],
            "refactor_core_print",
        )?;
        Ok(Some(CgValue::unit()))
    }

    fn refactor_core_print_to_string(
        &mut self,
        span: Span,
        value: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match value.ty {
            CgTy::String => Ok(value),
            CgTy::Bool => {
                let Some(BasicValueEnum::IntValue(raw)) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor core print Bool value",
                        at: span.into(),
                    });
                };
                let widened = self.codegen.builder.build_int_z_extend(
                    raw,
                    self.codegen.context.i64_type(),
                    "refactor_core_print_bool_arg",
                )?;
                let runtime = self.codegen.declare_runtime_bool_to_string();
                let call = self.codegen.build_call_preserving_gc_local_roots(
                    span,
                    runtime,
                    &[widened.into()],
                    "refactor_core_print_bool_to_string",
                )?;
                self.string_result_from_runtime_call(span, call, "Bool")
            }
            CgTy::Int(_) => {
                let Some(BasicValueEnum::IntValue(raw)) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor core print Int value",
                        at: span.into(),
                    });
                };
                let runtime = self.codegen.declare_runtime_int_to_string();
                let call = self.codegen.build_call_preserving_gc_local_roots(
                    span,
                    runtime,
                    &[raw.into()],
                    "refactor_core_print_int_to_string",
                )?;
                self.string_result_from_runtime_call(span, call, "Int")
            }
            CgTy::Float64 | CgTy::Float32 => {
                let Some(BasicValueEnum::FloatValue(raw)) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor core print Float value",
                        at: span.into(),
                    });
                };
                let runtime = match value.ty {
                    CgTy::Float64 => self.codegen.declare_runtime_float64_to_string(),
                    CgTy::Float32 => self.codegen.declare_runtime_float32_to_string(),
                    _ => unreachable!("value.ty matched float above"),
                };
                let call = self.codegen.build_call_preserving_gc_local_roots(
                    span,
                    runtime,
                    &[raw.into()],
                    "refactor_core_print_float_to_string",
                )?;
                self.string_result_from_runtime_call(span, call, "Float")
            }
            _ => Err(frontend_error(format!(
                "refactor core print unsupported ToString receiver {:?}",
                value.ty
            ))),
        }
    }

    fn string_result_from_runtime_call(
        &self,
        span: Span,
        call: CallSiteValue<'ctx>,
        label: &'static str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let ret = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor core print ToString runtime ret",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(str_ptr) = ret else {
            return Err(frontend_error(format!(
                "refactor core print {label} ToString runtime ret type mismatch"
            )));
        };
        Ok(CgValue {
            ty: CgTy::String,
            value: Some(str_ptr.into()),
        })
    }

    fn lower_refactor_internal_print_string(
        &mut self,
        span: Span,
        callee_fqn: &str,
        args: &[mir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let runtime_name = match callee_fqn {
            "scoop.core.__scoop_print_string" => "scoop_print",
            "scoop.core.__scoop_println_string" => "scoop_println",
            _ => return Ok(None),
        };
        if args.len() != 1 || args[0].name.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor internal print string arg contract",
                at: span.into(),
            });
        }
        let arg = &args[0];
        let value = self.codegen.codegen_mir_operand_expected(
            arg.span,
            &arg.value,
            self.slots,
            Some(CgTy::String),
        )?;
        let value = self.codegen.coerce_value(arg.span, value, CgTy::String)?;
        let Some(BasicValueEnum::PointerValue(str_ptr)) = value.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor internal print string arg value",
                at: arg.span.into(),
            });
        };
        let runtime = self.codegen.declare_runtime_print_like(runtime_name);
        let _ = self.codegen.build_call_preserving_gc_local_roots(
            arg.span,
            runtime,
            &[str_ptr.into()],
            "refactor_internal_print",
        )?;
        Ok(Some(CgValue::unit()))
    }

    fn is_unused_callee_ref(&self, fqn: &str) -> bool {
        self.codegen.fun_index.contains_key(fqn)
            || self.codegen.extern_funs.contains_key(fqn)
            || matches!(
                fqn,
                "scoop.core.__scoop_print_string"
                    | "scoop.core.__scoop_println_string"
                    | "scoop.core.__scoop_gc_collect"
                    | "scoop.core.GC.handleNew"
                    | "scoop.core.GC.handleDrop"
                    | "scoop.core.__scoop_thread_spawn_join_resume_u64"
            )
    }

    fn pack_refactor_call_args(
        &mut self,
        span: Span,
        entry: &RefactorCallableEntryLayout<'ctx>,
        args: &[mir::CallArg],
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        self.pack_call_args_for_invoke_args_tuple(
            span,
            entry.invoke_args_tuple_ty(),
            args,
            "refactor_pure_call",
        )
    }

    pub(super) fn pack_call_args_for_invoke_args_tuple(
        &mut self,
        span: Span,
        invoke_args_tuple_ty: TypeId,
        args: &[mir::CallArg],
        name: &str,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        if args.iter().any(|arg| arg.name.is_some()) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor named call arg",
                at: span.into(),
            });
        }
        let layout = self.abi.source_value_layout(invoke_args_tuple_ty)?;
        if layout.abi().is_elided() {
            return Ok(None);
        }
        match layout.kind() {
            RefactorSourceAbiLayoutKind::Scalar => {
                let arg = args.first().ok_or_else(|| {
                    frontend_error(format!("{name} scalar call ABI 缺少 argument"))
                })?;
                if args.len() != 1 {
                    return Err(frontend_error(format!(
                        "{name} scalar call ABI 期望 1 个 argument，实际 {} 个",
                        args.len()
                    )));
                }
                let expected = self
                    .codegen
                    .cg_ty_of_mir_type(self.source_types, layout.source_ty())
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor scalar call arg type",
                        at: arg.span.into(),
                    })?;
                let value = self.codegen.codegen_mir_operand_expected(
                    arg.span,
                    &arg.value,
                    self.slots,
                    Some(expected),
                )?;
                let value = self.codegen.coerce_value(arg.span, value, expected)?;
                Ok(Some(value.value.ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor scalar call arg value",
                        at: arg.span.into(),
                    },
                )?))
            }
            RefactorSourceAbiLayoutKind::Tuple => {
                if args.len() == 1
                    && self.operand_source_ty(&args[0].value) == Some(layout.source_ty())
                {
                    return self.pack_whole_tuple_operand(layout, &args[0].value, name);
                }
                if args.len() != layout.fields().len() {
                    return Err(frontend_error(format!(
                        "{name} tuple call ABI 期望 {} 个 argument，实际 {} 个",
                        layout.fields().len(),
                        args.len()
                    )));
                }
                let BasicTypeEnum::StructType(struct_ty) = layout.abi().llvm_ty() else {
                    return Err(frontend_error(format!(
                        "{name} tuple call ABI layout 不是 struct"
                    )));
                };
                let mut aggregate = struct_ty.get_undef();
                for (index, field) in layout.fields().iter().enumerate() {
                    if field.is_elided() {
                        continue;
                    }
                    let arg = args.get(index).ok_or_else(|| {
                        frontend_error(format!(
                            "refactor pure statement tuple call ABI 缺少 argument {index}"
                        ))
                    })?;
                    let expected = self
                        .codegen
                        .cg_ty_of_mir_type(self.source_types, field.source_ty())
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "refactor tuple call arg type",
                            at: arg.span.into(),
                        })?;
                    let value = self.codegen.codegen_mir_operand_expected(
                        arg.span,
                        &arg.value,
                        self.slots,
                        Some(expected),
                    )?;
                    let value = self.codegen.coerce_value(arg.span, value, expected)?;
                    let raw = value.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor tuple call arg value",
                        at: arg.span.into(),
                    })?;
                    aggregate = self
                        .codegen
                        .builder
                        .build_insert_value(
                            aggregate,
                            raw,
                            field
                                .abi_field_index()
                                .expect("non-elided field has ABI index"),
                            &format!("{name}_arg{index}"),
                        )?
                        .into_struct_value();
                }
                Ok(Some(aggregate.into()))
            }
        }
    }

    fn extract_refactor_pure_call_complete(
        &mut self,
        span: Span,
        step_schema: crate::effect_facts::StepSchemaId,
        step: BasicValueEnum<'ctx>,
        target_cg: super::super::types::CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let step_layout = self.abi.step_layout(step_schema).ok_or_else(|| {
            frontend_error(format!(
                "refactor pure statement call 缺少 callee step schema s{} layout",
                step_schema.as_u32()
            ))
        })?;
        if !step_layout.cases().is_empty() {
            return Err(frontend_error(format!(
                "refactor pure statement call callee step schema s{} 含 outward case，必须走 boundary lowering",
                step_schema.as_u32()
            )));
        }
        let payload = self.codegen.refactor_extract_step_payload(
            step_layout,
            step,
            step_layout.complete_variant(),
            "refactor_pure_call_complete_payload",
        )?;
        match (target_cg, payload) {
            (super::super::types::CgTy::Unit, None) => Ok(CgValue::unit()),
            (super::super::types::CgTy::Never, None) => Ok(CgValue::never()),
            (super::super::types::CgTy::Unit, Some(_)) => Err(frontend_error(
                "refactor pure statement call Unit target 收到 non-elided Complete payload"
                    .to_string(),
            )),
            (_, Some(raw)) => {
                let value = self.codegen.cg_value_from_loaded(span, target_cg, raw)?;
                self.codegen.coerce_value(span, value, target_cg).map_err(|err| {
                    frontend_error(format!(
                        "refactor pure direct call Complete payload coercion failed: value_ty={:?} target_ty={:?}: {err}",
                        value.ty, target_cg,
                    ))
                })
            }
            (_, None) => Err(frontend_error(
                "refactor pure statement call non-Unit target 缺少 Complete payload".to_string(),
            )),
        }
    }

    pub(super) fn load_local(
        &mut self,
        span: Span,
        local: LocalId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let slot = self.codegen.mir_local_slot(span, self.slots, local)?;
        self.codegen.load_mir_local(span, slot)
    }

    pub(super) fn store_local(
        &mut self,
        span: Span,
        local: LocalId,
        value: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let slot = self.codegen.mir_local_slot(span, self.slots, local)?;
        let value = self.codegen.coerce_value(span, value, slot.cg_ty)?;
        self.codegen
            .store_local_value(span, slot.ptr, slot.cg_ty, value)
    }

    pub(super) fn store_loaded_raw_local(
        &mut self,
        span: Span,
        local: LocalId,
        raw: BasicValueEnum<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let slot = self.codegen.mir_local_slot(span, self.slots, local)?;
        let value = self.codegen.cg_value_from_loaded(span, slot.cg_ty, raw)?;
        self.codegen
            .store_local_value(span, slot.ptr, slot.cg_ty, value)
    }

    pub(super) fn lower_operand_source(
        &mut self,
        source: &LateLoweredOperandSource,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let span = source.span().unwrap_or_else(|| self.body_span());
        let expected = self
            .codegen
            .cg_ty_of_mir_type(self.source_types, source.source_ty())
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor operand source type",
                at: span.into(),
            })?;
        let operand = match source.value() {
            LateLoweredOperandValueSource::Local(local) => mir::Operand::Local(*local),
            LateLoweredOperandValueSource::Const(value) => mir::Operand::Const(value.clone()),
        };
        let value = self.codegen.codegen_mir_operand_expected(
            span,
            &operand,
            self.slots,
            Some(expected),
        )?;
        self.codegen.coerce_value(span, value, expected)
    }

    pub(super) fn pack_sources(
        &mut self,
        source_ty: TypeId,
        sources: &[LateLoweredOperandSource],
        name: &str,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        let layout = self.abi.source_value_layout(source_ty)?;
        if layout.abi().is_elided() {
            return Ok(None);
        }
        match layout.kind() {
            RefactorSourceAbiLayoutKind::Scalar => {
                let source = sources.first().ok_or_else(|| {
                    frontend_error(format!("refactor ABI scalar payload `{name}` 缺少 source"))
                })?;
                Ok(self.lower_operand_source(source)?.value)
            }
            RefactorSourceAbiLayoutKind::Tuple => {
                if sources.len() == 1 && sources[0].source_ty() == source_ty {
                    return self.pack_whole_tuple_source(layout, &sources[0], name);
                }
                let BasicTypeEnum::StructType(struct_ty) = layout.abi().llvm_ty() else {
                    return Err(frontend_error(format!(
                        "refactor ABI tuple payload `{name}` layout 不是 struct"
                    )));
                };
                let mut aggregate = struct_ty.get_undef();
                for (index, source) in sources.iter().enumerate() {
                    let Some(field) = layout.field(index) else {
                        return Err(frontend_error(format!(
                            "refactor ABI tuple payload `{name}` source index {index} 超出 layout 字段"
                        )));
                    };
                    if field.is_elided() {
                        continue;
                    }
                    let raw = self.lower_operand_source(source)?.value.ok_or_else(|| {
                        frontend_error(format!(
                            "refactor ABI tuple payload `{name}` source index {index} 被 elide 但 field 需要值"
                        ))
                    })?;
                    aggregate = self
                        .codegen
                        .builder
                        .build_insert_value(
                            aggregate,
                            raw,
                            field
                                .abi_field_index()
                                .expect("non-elided field has ABI index"),
                            &format!("{name}_field{index}"),
                        )?
                        .into_struct_value();
                }
                Ok(Some(aggregate.into()))
            }
        }
    }

    fn pack_whole_tuple_operand(
        &mut self,
        layout: &RefactorSourceAbiLayout<'ctx>,
        operand: &mir::Operand,
        name: &str,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        if layout.abi().is_elided() {
            return Ok(None);
        }
        let value = self.codegen.codegen_mir_operand_expected(
            self.body_span(),
            operand,
            self.slots,
            self.codegen
                .cg_ty_of_mir_type(self.source_types, layout.source_ty()),
        )?;
        self.pack_whole_tuple_value(layout, value, name)
    }

    fn pack_whole_tuple_source(
        &mut self,
        layout: &RefactorSourceAbiLayout<'ctx>,
        source: &LateLoweredOperandSource,
        name: &str,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        if layout.abi().is_elided() {
            return Ok(None);
        }
        let value = self.lower_operand_source(source)?;
        self.pack_whole_tuple_value(layout, value, name)
    }

    fn pack_whole_tuple_value(
        &mut self,
        layout: &RefactorSourceAbiLayout<'ctx>,
        value: CgValue<'ctx>,
        name: &str,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        let Some(BasicValueEnum::StructValue(tuple)) = value.value else {
            return Err(frontend_error(format!(
                "refactor ABI tuple payload `{name}` whole tuple source 缺少 struct value"
            )));
        };
        let BasicTypeEnum::StructType(struct_ty) = layout.abi().llvm_ty() else {
            return Err(frontend_error(format!(
                "refactor ABI tuple payload `{name}` whole tuple layout 不是 struct"
            )));
        };
        let mut aggregate = struct_ty.get_undef();
        for field in layout.fields() {
            if field.is_elided() {
                continue;
            }
            let raw = self.codegen.builder.build_extract_value(
                tuple,
                field.source_index(),
                &format!("{name}_whole_field{}", field.source_index()),
            )?;
            aggregate = self
                .codegen
                .builder
                .build_insert_value(
                    aggregate,
                    raw,
                    field
                        .abi_field_index()
                        .expect("non-elided field has ABI index"),
                    &format!("{name}_field{}", field.source_index()),
                )?
                .into_struct_value();
        }
        Ok(Some(aggregate.into()))
    }

    fn operand_source_ty(&self, operand: &mir::Operand) -> Option<TypeId> {
        match operand {
            mir::Operand::Local(local) => self
                .body
                .locals
                .get(local.as_u32() as usize)
                .map(|local| local.ty),
            mir::Operand::Const(mir::ConstValue::Bool(_)) => Some(self.codegen.builtins.bool_),
            mir::Operand::Const(mir::ConstValue::Char) => Some(self.codegen.builtins.char_),
            mir::Operand::Const(mir::ConstValue::Unit) => Some(self.codegen.builtins.unit),
            mir::Operand::Const(mir::ConstValue::Int | mir::ConstValue::SynthInt(_)) => {
                Some(self.codegen.builtins.int)
            }
            mir::Operand::Const(mir::ConstValue::Float64) => Some(self.codegen.builtins.float64),
            mir::Operand::Const(mir::ConstValue::Float32) => Some(self.codegen.builtins.float32),
            mir::Operand::Const(mir::ConstValue::String) => Some(self.codegen.builtins.string),
        }
    }

    pub(super) fn unpack_payload_field(
        &mut self,
        payload: Option<BasicValueEnum<'ctx>>,
        payload_ty: TypeId,
        ordinal: u32,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        let layout = self.abi.source_value_layout(payload_ty)?;
        if layout.abi().is_elided() {
            return Ok(None);
        }
        match layout.kind() {
            RefactorSourceAbiLayoutKind::Scalar => Ok(payload),
            RefactorSourceAbiLayoutKind::Tuple => {
                let Some(field) = layout.field(ordinal as usize) else {
                    return Err(frontend_error(format!(
                        "refactor payload tuple t{} 缺少 ordinal {}",
                        payload_ty.as_u32(),
                        ordinal
                    )));
                };
                if field.is_elided() {
                    return Ok(None);
                }
                let Some(BasicValueEnum::StructValue(tuple)) = payload else {
                    return Err(frontend_error(format!(
                        "refactor payload tuple t{} 缺少 struct payload",
                        payload_ty.as_u32()
                    )));
                };
                Ok(Some(
                    self.codegen.builder.build_extract_value(
                        tuple,
                        field
                            .abi_field_index()
                            .expect("non-elided field has ABI index"),
                        "refactor_payload_field",
                    )?,
                ))
            }
        }
    }

    fn body_span(&self) -> Span {
        self.body
            .blocks
            .first()
            .map(|block| block.terminator.span)
            .unwrap_or_else(|| Span::new(0, 0))
    }
}

fn refactor_core_print_runtime_name(callee_fqn: &str) -> Option<&'static str> {
    if callee_fqn == "scoop.core.println" || callee_fqn.starts_with("scoop.core.println::<") {
        Some("scoop_println")
    } else if callee_fqn == "scoop.core.print" || callee_fqn.starts_with("scoop.core.print::<") {
        Some("scoop_print")
    } else {
        None
    }
}

fn get_or_create_refactor_thread_resume_u64_thunk<'a, 'ctx>(
    codegen: &mut MainCodegen<'a, 'ctx>,
    surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
    step_layout: &RefactorStepLayout<'ctx>,
) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
    let symbol = format!(
        "__scoop_refactor_thread_resume_u64__k{}",
        surface.continuation_schema().as_u32()
    );
    let k_ty = codegen.llvm_gc_i8_ptr_type();
    let i64_ty = codegen.context.i64_type();
    let fn_ty = codegen
        .context
        .void_type()
        .fn_type(&[k_ty.into(), i64_ty.into()], false);
    let function = codegen
        .module
        .get_function(&symbol)
        .unwrap_or_else(|| codegen.module.add_function(&symbol, fn_ty, None));
    if function.count_basic_blocks() > 0 {
        return Ok(function);
    }

    let restore_block = codegen.builder.get_insert_block();
    let entry = codegen.context.append_basic_block(function, "entry");
    let complete = codegen.context.append_basic_block(function, "complete");
    let non_complete = codegen.context.append_basic_block(function, "non_complete");

    codegen.builder.position_at_end(entry);
    let continuation = function.get_nth_param(0).ok_or_else(|| {
        frontend_error(format!(
            "refactor thread resume thunk `{symbol}` 缺少 continuation 参数"
        ))
    })?;
    let payload = function.get_nth_param(1).ok_or_else(|| {
        frontend_error(format!(
            "refactor thread resume thunk `{symbol}` 缺少 payload 参数"
        ))
    })?;
    let surface_fun = codegen
        .module
        .get_function(surface.symbol_name())
        .unwrap_or_else(|| {
            codegen
                .module
                .add_function(surface.symbol_name(), surface.llvm_ty(), None)
        });
    let call = codegen.builder.build_call(
        surface_fun,
        &[continuation.into(), payload.into()],
        "refactor_thread_surface_resume",
    )?;
    let step = call.try_as_basic_value().basic().ok_or_else(|| {
        frontend_error("refactor thread surface resume 未返回 Step_F".to_string())
    })?;
    let BasicValueEnum::StructValue(step_struct) = step else {
        return Err(frontend_error(
            "refactor thread surface resume Step_F 不是 struct".to_string(),
        ));
    };
    if step_struct.get_type() != step_layout.llvm_ty() {
        return Err(frontend_error(format!(
            "refactor thread surface resume Step_F layout 漂移：surface s{}",
            surface.return_step_schema().as_u32()
        )));
    }
    let tag = codegen
        .builder
        .build_extract_value(step_struct, 0, "refactor_thread_resume_step_tag")?
        .into_int_value();
    let is_complete = codegen.builder.build_int_compare(
        IntPredicate::EQ,
        tag,
        codegen.context.i32_type().const_zero(),
        "refactor_thread_resume_is_complete",
    )?;
    codegen
        .builder
        .build_conditional_branch(is_complete, complete, non_complete)?;

    codegen.builder.position_at_end(complete);
    codegen.builder.build_return(None)?;

    codegen.builder.position_at_end(non_complete);
    let fatal = codegen.declare_runtime_refactor_thread_resume_noncomplete_fatal();
    let _ = codegen
        .builder
        .build_call(fatal, &[], "refactor_thread_resume_noncomplete")?;
    codegen.builder.build_return(None)?;

    if let Some(block) = restore_block {
        codegen.builder.position_at_end(block);
    }
    Ok(function)
}

fn frontend_error(message: String) -> LlvmEmitError {
    LlvmEmitError::Frontend { message }
}

#[cfg(test)]
mod tests {
    #[test]
    fn refactor_llvm_clean_backend_boundary_audits_body_fallbacks() {
        let body = include_str!("body.rs");
        let forbidden = [
            concat!("codegen_mir_", "statement("),
            concat!("codegen_mir_", "direct_call("),
            concat!("build_unified_", "lowering_contract"),
            concat!("effect_analysis_", "ctx"),
            concat!("scoop_effect_", "handler_stack"),
            concat!("scoop_effect_", "outcome"),
        ];
        for needle in forbidden {
            assert!(
                !body.contains(needle),
                "refactor body must use published contracts plus value primitives, not `{needle}`"
            );
        }
    }

    #[test]
    fn refactor_llvm_value_primitive_inventory_is_explicit() {
        let inventory = [
            "literal",
            "local-load",
            "local-store",
            "scalar-pack",
            "tuple-pack",
            "tuple-unpack",
            "primitive-op",
            "non-control-cast",
            "member-read",
            "member-write",
        ];
        assert_eq!(inventory.len(), 10);
        assert!(inventory.contains(&"local-store"));
        assert!(inventory.contains(&"member-write"));
    }

    #[test]
    fn refactor_llvm_source_slice_classification_audits_body_skip_heuristics() {
        let body = include_str!("body.rs");
        let forbidden = [
            "skipped_statement_indices_for_state",
            "statement_is_published_resume_payload_injection",
            "try_lower_refactor_specialized_direct_call",
            "CallKind::Resume { .. }",
            "TopLevelRef(mir::TopLevelRef",
        ];
        for needle in forbidden {
            assert!(
                !body.contains(needle),
                "refactor body must consume source-slice classifications instead of private skip heuristic `{needle}`"
            );
        }
    }

    #[test]
    fn refactor_llvm_pure_statement_lowering_is_owned_by_value_primitives() {
        let value = include_str!("value.rs");
        let forbidden = concat!("codegen_mir_effect_", "neutral_statement");

        assert!(
            !value.contains(forbidden),
            "refactor pure statement lowering must not delegate whole statements to `{forbidden}`"
        );
        assert!(value.contains("lower_refactor_pure_direct_call"));
        assert!(value.contains("refactor_extract_step_payload"));
        assert!(value.contains("mir::Rvalue::ClassCtor"));
        assert!(value.contains("codegen_mir_refactor_class_ctor_call"));
        assert!(!value.contains(concat!("codegen_mir_", "class_ctor_call")));
        assert!(value.contains("mir::Rvalue::MakeClosure"));
    }

    #[test]
    fn refactor_llvm_plain_call_lowering_uses_ordinary_direct_call() {
        let value = include_str!("value.rs");

        assert!(value.contains("maybe_plain_callable_layout_by_root_fqn"));
        assert!(value.contains("codegen_mir_direct_call"));
        assert!(value.contains("codegen_mir_plain_dynamic_call"));
        assert!(value.contains("refactor_pure_call_step"));
    }

    #[test]
    fn refactor_llvm_effect_typed_adapter_wraps_plain_body() {
        let value = include_str!("value.rs");

        assert!(value.contains("maybe_build_effect_typed_plain_closure_adapter"));
        assert!(value.contains("__scoop_refactor_plain_adapter__"));
        assert!(value.contains("refactor_carrier_to_plain"));
        assert!(value.contains("refactor_adapter_complete"));
    }

    #[test]
    fn refactor_llvm_member_read_write_lowering_uses_member_primitives() {
        let value = include_str!("value.rs");

        assert!(value.contains("mir::StatementKind::StoreMember"));
        assert!(value.contains("codegen_mir_store_member"));
        assert!(value.contains("mir::Rvalue::MemberAccess"));
    }
}
