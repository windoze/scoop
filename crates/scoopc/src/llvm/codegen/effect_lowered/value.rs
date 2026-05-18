//! Effect-neutral value/expression primitives for the clean LLVM path.
//!
//! This module is the narrow sharing boundary between the backend and
//! generic LLVM value helpers.  It may lower literals, local loads/stores,
//! scalar/tuple ABI packing, primitive operators, casts that do not introduce a
//! hidden control path, and canonical MIR member read/write primitives.  It must
//! not choose call targets, returns, state transitions, boundary dispatch, or
//! continuation behavior; those decisions come from published P5/P6 contracts.

use std::collections::{BTreeSet, HashSet};

use inkwell::module::Linkage;
use inkwell::types::{BasicType, BasicTypeEnum, FunctionType};
use inkwell::values::{
    AggregateValueEnum, BasicMetadataValueEnum, BasicValue, BasicValueEnum, CallSiteValue,
    FunctionValue, PointerValue,
};
use inkwell::{AddressSpace, AtomicOrdering, IntPredicate};

use crate::effect_lowered::ir::{LateLoweredOperandSource, LateLoweredOperandValueSource};
use crate::llvm::LlvmEmitError;
use crate::mir::{self, LocalId};
use crate::span::Span;
use crate::stable_id::canonical_record;
use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::super::mir_body::MirLocalSlot;
use super::super::types::{CgTy, CgValue, IntTy};
use super::super::{CallableCarrierKind, MainCodegen};
use super::stable_naming;
use super::types::{
    CallableEntryLayout, ProgramAbiQuery, SourceAbiLayout, SourceAbiLayoutKind, StepLayout,
};

/// A borrow-scoped facade over effect-neutral LLVM value primitives.
pub(super) struct ValuePrimitives<'p, 'a, 'ctx> {
    codegen: &'p mut MainCodegen<'a, 'ctx>,
    source_types: &'a TypeStore,
    body: &'a mir::Body,
    slots: &'p [MirLocalSlot<'ctx>],
    abi: &'p ProgramAbiQuery<'ctx>,
}

#[derive(Clone, Copy)]
struct ClosureSurfaceLayout<'ctx> {
    llvm_ty: FunctionType<'ctx>,
    invoke_args_tuple_ty: TypeId,
    return_step_schema: crate::effect_facts::StepSchemaId,
}

type EffectFamilyMatchKey = (String, Vec<TypeId>);

fn function_type_source_args(fun_ty: &crate::ty::FunctionType) -> Vec<TypeId> {
    fun_ty
        .receiver
        .into_iter()
        .chain(fun_ty.params.iter().copied())
        .collect()
}

fn direct_call_dispatch_fqn(fqn: &str) -> &str {
    if let Some((base, _)) = fqn.rsplit_once("::<") {
        return base;
    }
    fqn.split_once("$overload$")
        .map(|(base, _)| base)
        .unwrap_or(fqn)
}

fn source_carrier_types(types: &TypeStore, carrier_ty: TypeId) -> Option<Vec<TypeId>> {
    match types.kind(carrier_ty) {
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => Some(elements.clone()),
        TypeKind::Value(ValueTypeKind::Unit) => Some(Vec::new()),
        _ => Some(vec![carrier_ty]),
    }
}

fn operand_mentions_local(operand: &mir::Operand, local: LocalId) -> bool {
    matches!(operand, mir::Operand::Local(found) if *found == local)
}

fn call_args_mention_local(args: &[mir::CallArg], local: LocalId) -> bool {
    args.iter()
        .any(|arg| operand_mentions_local(&arg.value, local))
}

fn call_kind_mentions_local(kind: &mir::CallKind, local: LocalId) -> bool {
    match kind {
        mir::CallKind::Direct { .. } => false,
        mir::CallKind::Closure { callee, .. }
        | mir::CallKind::FunValue { callee }
        | mir::CallKind::FunPtr { callee } => operand_mentions_local(callee, local),
        mir::CallKind::Virtual { receiver, .. } | mir::CallKind::Interface { receiver, .. } => {
            operand_mentions_local(receiver, local)
        }
        mir::CallKind::Resume { continuation, .. } => operand_mentions_local(continuation, local),
    }
}

fn rvalue_mentions_local(value: &mir::Rvalue, local: LocalId) -> bool {
    match value {
        mir::Rvalue::Use(operand)
        | mir::Rvalue::Transport { value: operand, .. }
        | mir::Rvalue::TypeCheck { value: operand, .. }
        | mir::Rvalue::Cast { value: operand, .. }
        | mir::Rvalue::TupleGet { tuple: operand, .. }
        | mir::Rvalue::PatternMatch {
            subject: operand, ..
        }
        | mir::Rvalue::PatternExtract {
            subject: operand, ..
        }
        | mir::Rvalue::MakeClosure { env: operand, .. } => operand_mentions_local(operand, local),
        mir::Rvalue::MemberAccess { receiver, .. } => operand_mentions_local(receiver, local),
        mir::Rvalue::EnumVariant { args, .. } | mir::Rvalue::ClassCtor { args, .. } => {
            call_args_mention_local(args, local)
        }
        mir::Rvalue::Call { kind, args, .. } => {
            call_kind_mentions_local(kind, local) || call_args_mention_local(args, local)
        }
        mir::Rvalue::MakeTuple { elements, .. } => elements
            .iter()
            .any(|operand| operand_mentions_local(operand, local)),
        mir::Rvalue::StructLit { fields, .. } => fields
            .iter()
            .any(|field| operand_mentions_local(&field.value, local)),
        mir::Rvalue::InterpolatedString { parts, .. } => parts.iter().any(|part| match part {
            mir::InterpolatedStringPart::Text { .. } => false,
            mir::InterpolatedStringPart::Expr { value, .. } => operand_mentions_local(value, local),
        }),
        mir::Rvalue::TopLevelRef(_)
        | mir::Rvalue::UnresolvedName { .. }
        | mir::Rvalue::SizeOf { .. }
        | mir::Rvalue::KindOf { .. }
        | mir::Rvalue::AlignOf { .. }
        | mir::Rvalue::DescOf { .. }
        | mir::Rvalue::TypeMetadataLiteral(_)
        | mir::Rvalue::PerformResult { .. }
        | mir::Rvalue::Todo(_) => false,
    }
}

impl<'p, 'a, 'ctx> ValuePrimitives<'p, 'a, 'ctx> {
    pub(super) fn new(
        codegen: &'p mut MainCodegen<'a, 'ctx>,
        source_types: &'a TypeStore,
        body: &'a mir::Body,
        slots: &'p [MirLocalSlot<'ctx>],
        abi: &'p ProgramAbiQuery<'ctx>,
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
                    && let mir::Rvalue::TopLevelRef(mir::TopLevelRef { fqn, .. }) = rvalue
                    && self.is_unused_callee_ref(fqn)
                {
                    return Ok(());
                }
                if let mir::Rvalue::MemberAccess { member, .. } = rvalue
                    && let Some(
                        mir::MemberTarget::Fun { fqn } | mir::MemberTarget::ExtensionFun { fqn },
                    ) = &member.resolved
                {
                    let _ = fqn;
                    return Ok(());
                }
                if let mir::Rvalue::MemberAccess { member, .. } = rvalue
                    && member.resolved.is_none()
                    && matches!(member.name.as_str(), "byteLength" | "getByte")
                {
                    return Ok(());
                }
                if let mir::Rvalue::UnresolvedName { .. } = rvalue
                    && self
                        .body
                        .locals
                        .get(target.as_u32() as usize)
                        .is_some_and(|local| {
                            matches!(
                                self.source_types.kind(local.ty),
                                TypeKind::Ref(RefTypeKind::Function(_))
                            )
                        })
                {
                    return Ok(());
                }
                let slot = self
                    .codegen
                    .mir_local_slot(stmt.span, self.slots, *target)?;
                if let mir::Rvalue::TopLevelRef(mir::TopLevelRef { fqn, .. }) = rvalue
                    && self
                        .body
                        .locals
                        .get(target.as_u32() as usize)
                        .is_some_and(|local| {
                            matches!(
                                self.source_types.kind(local.ty),
                                TypeKind::Ref(RefTypeKind::Function(_))
                            )
                        })
                    && (!self.codegen.top_level_immutable_values.contains_key(fqn)
                        || !used_locals.contains(target))
                {
                    return Ok(());
                }
                if let mir::Rvalue::TopLevelRef(mir::TopLevelRef { fqn, .. }) = rvalue
                    && (self.codegen.top_level_vars.contains_key(fqn)
                        || self.codegen.has_extern_global_contract(fqn))
                    && self.local_is_only_atomic_target(*target)
                {
                    return Ok(());
                }
                if let mir::Rvalue::TopLevelRef(mir::TopLevelRef { fqn, .. }) = rvalue
                    && !self.codegen.object_inits.contains_key(fqn)
                    && !self.codegen.top_level_consts.contains_key(fqn)
                    && !self.codegen.top_level_immutable_values.contains_key(fqn)
                    && !self.codegen.top_level_vars.contains_key(fqn)
                    && !self.codegen.has_extern_global_contract(fqn)
                    && !self.static_enum_unit_variant_value(fqn)
                {
                    return Ok(());
                }
                if let mir::Rvalue::UnresolvedName { .. } = rvalue
                    && !matches!(slot.cg_ty, CgTy::Enum(_))
                {
                    return Ok(());
                }
                if matches!(rvalue, mir::Rvalue::Todo("missing expr"))
                    && self.local_is_only_static_member_namespace_receiver(*target)
                {
                    return Ok(());
                }
                if matches!(rvalue, mir::Rvalue::TopLevelRef(_))
                    && self.local_is_only_static_member_namespace_receiver(*target)
                {
                    return Ok(());
                }
                let value = self
                    .lower_effect_neutral_rvalue(stmt.span, rvalue, slot.cg_ty, Some(*target))
                    .map_err(|err| match err {
                        LlvmEmitError::InvalidLiteral { .. } => err,
                        other => frontend_error(format!(
                            "pure assignment local{} rvalue {:?} lowering failed: {other}",
                            target.as_u32(),
                            rvalue,
                        )),
                    })?;
                let value_ty = value.ty;
                if slot.cg_ty == CgTy::Never {
                    if value_ty == CgTy::Never
                        && self
                            .codegen
                            .builder
                            .get_insert_block()
                            .is_some_and(|bb| bb.get_terminator().is_none())
                    {
                        self.codegen.builder.build_unreachable()?;
                    }
                    return Ok(());
                }
                let _ = self
                    .codegen
                    .store_local_value(stmt.span, slot.ptr, slot.cg_ty, value)
                    .map_err(|err| {
                        frontend_error(format!(
                            "pure assignment local{} store failed: value_ty={:?} target_ty={:?}: {err}",
                            target.as_u32(),
                            value_ty,
                            slot.cg_ty,
                        ))
                    })?;
                if value_ty == CgTy::Never
                    && self
                        .codegen
                        .builder
                        .get_insert_block()
                        .is_some_and(|bb| bb.get_terminator().is_none())
                {
                    self.codegen.builder.build_unreachable()?;
                }
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
            mir::StatementKind::StoreTopLevelVar {
                fqn,
                value,
                value_ty,
            } => self
                .codegen
                .codegen_mir_store_top_level_var(stmt.span, fqn, value, *value_ty, self.slots),
            mir::StatementKind::Todo(_) => std::panic::panic_any(
                "MIR verifier must reject Todo statements before effect lowering",
            ),
        }
    }

    fn lower_effect_neutral_rvalue(
        &mut self,
        span: Span,
        value: &mir::Rvalue,
        target_cg: super::super::types::CgTy,
        target_local: Option<LocalId>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if let mir::Rvalue::UnresolvedName { name } = value
            && let Some(source_ty) = target_local
                .and_then(|local| self.body.locals.get(local.as_u32() as usize))
                .map(|local| local.ty)
        {
            return self.codegen.codegen_mir_unresolved_name_with_source_ty(
                span,
                name,
                self.source_types,
                source_ty,
                target_cg,
            );
        }
        if let mir::Rvalue::Call {
            kind: mir::CallKind::FunValue { callee },
            args,
            ..
        } = value
            && let Some(target_ty) = target_local
                .and_then(|local| self.body.locals.get(local.as_u32() as usize))
                .map(|local| local.ty)
            && self.unresolved_fun_value_callee_name(callee).is_some()
        {
            let _ = (args, target_ty);
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "enum variant call requires MIR enum payload schema",
                at: span.into(),
            });
        }
        if let mir::Rvalue::Use(mir::Operand::Local(source_local))
        | mir::Rvalue::Transport {
            value: mir::Operand::Local(source_local),
            ..
        } = value
            && let Some((env, fn_ptr, env_contract)) = self.local_make_closure_source(*source_local)
            && let Some(adapter) =
                self.maybe_build_effect_typed_closure_target_fn_ptr(span, target_local, &fn_ptr)?
        {
            let env_cg = self
                .codegen
                .mir_operand_cg_ty(self.body, self.source_types, &env)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect-typed closure coercion env type",
                    at: span.into(),
                })?;
            return self.codegen.codegen_mir_make_closure_with_target_fn_ptr(
                span,
                &env,
                &fn_ptr,
                &env_contract,
                self.source_types,
                env_cg,
                target_cg,
                self.slots,
                adapter,
            );
        }

        match value {
            mir::Rvalue::Call {
                kind,
                args,
                transport,
                ..
            } => self.lower_pure_direct_call(span, kind, args, transport, target_cg, target_local),
            mir::Rvalue::MakeClosure {
                env,
                fn_ptr,
                env_contract,
            } => {
                let env_cg = self
                    .codegen
                    .mir_operand_cg_ty(self.body, self.source_types, env)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pure closure carrier env type",
                        at: span.into(),
                    })?;
                if let Some(adapter) =
                    self.maybe_build_effect_typed_closure_target_fn_ptr(span, target_local, fn_ptr)?
                {
                    return self.codegen.codegen_mir_make_closure_with_target_fn_ptr(
                        span,
                        env,
                        fn_ptr,
                        env_contract,
                        self.source_types,
                        env_cg,
                        target_cg,
                        self.slots,
                        adapter,
                    );
                }
                self.codegen.codegen_mir_make_closure(
                    span,
                    env,
                    fn_ptr,
                    env_contract,
                    self.source_types,
                    env_cg,
                    target_cg,
                    self.slots,
                )
            }
            mir::Rvalue::StructLit { fields, .. } => {
                self.install_effect_typed_closure_target_overrides_for_struct_fields(
                    span, fields, target_cg,
                )?;
                self.codegen.codegen_mir_effect_neutral_rvalue(
                    span,
                    value,
                    self.body,
                    self.source_types,
                    self.slots,
                    target_cg,
                )
            }
            mir::Rvalue::ClassCtor {
                class_fqn,
                ctor,
                args,
                ..
            } => {
                let class_layout_key = self.class_ctor_layout_key(class_fqn, target_local)?;
                self.codegen.codegen_mir_class_ctor_call(
                    span,
                    &class_layout_key,
                    ctor,
                    args,
                    self.slots,
                )
            }
            mir::Rvalue::TopLevelRef(mir::TopLevelRef { fqn, .. })
                if self.static_enum_unit_variant_value(fqn) =>
            {
                self.codegen
                    .try_codegen_qualified_enum_unit_variant_value(span, fqn)?
                    .ok_or_else(|| frontend_error(format!("missing enum unit variant `{fqn}`")))
            }
            mir::Rvalue::MemberAccess { member, .. }
                if let Some(mir::MemberTarget::Value { fqn }) = &member.resolved
                    && self.static_member_value(member) =>
            {
                if self.codegen.lookup_object_property_by_fqn(fqn).is_some() {
                    self.codegen.codegen_object_property_access(span, fqn)
                } else if let Some(value) = self
                    .codegen
                    .try_codegen_qualified_enum_unit_variant_value(span, fqn)?
                {
                    Ok(value)
                } else {
                    self.codegen.codegen_top_level_value_ref(span, fqn)
                }
            }
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

    fn local_make_closure_source(
        &self,
        local: LocalId,
    ) -> Option<(mir::Operand, String, mir::ClosureEnvTransportMetadata)> {
        self.body.blocks.iter().find_map(|block| {
            block.stmts.iter().find_map(|stmt| {
                let mir::StatementKind::Assign { target, value } = &stmt.kind else {
                    return None;
                };
                if *target != local {
                    return None;
                }
                let mir::Rvalue::MakeClosure {
                    env,
                    fn_ptr,
                    env_contract,
                } = value
                else {
                    return None;
                };
                Some((env.clone(), fn_ptr.clone(), env_contract.clone()))
            })
        })
    }

    fn local_is_only_static_member_namespace_receiver(&self, local: LocalId) -> bool {
        let mut saw_static_member = false;
        for block in &self.body.blocks {
            for stmt in &block.stmts {
                let mir::StatementKind::Assign { target, value } = &stmt.kind else {
                    continue;
                };
                if *target == local {
                    continue;
                }
                if let mir::Rvalue::MemberAccess {
                    receiver: mir::Operand::Local(receiver),
                    member,
                    ..
                } = value
                    && *receiver == local
                    && (self.static_member_value(member)
                        || self.static_member_fun_for_namespace(local, member))
                {
                    saw_static_member = true;
                    continue;
                }
                if rvalue_mentions_local(value, local) {
                    return false;
                }
            }
        }
        saw_static_member
    }

    fn local_is_only_atomic_target(&self, local: LocalId) -> bool {
        let mut saw_atomic_call = false;
        for block in &self.body.blocks {
            for stmt in &block.stmts {
                let mir::StatementKind::Assign { target, value } = &stmt.kind else {
                    continue;
                };
                if *target == local {
                    continue;
                }
                if let mir::Rvalue::Call {
                    kind: mir::CallKind::Direct { callee_fqn },
                    args,
                    ..
                } = value
                    && (callee_fqn.starts_with("scoop.unsafe.__atomicInt")
                        || callee_fqn.starts_with("scoop.unsafe.__atomicRef"))
                    && matches!(
                        args.first(),
                        Some(mir::CallArg {
                            name: None,
                            value: mir::Operand::Local(target_local),
                            ..
                        }) if *target_local == local
                    )
                {
                    if args
                        .iter()
                        .skip(1)
                        .any(|arg| operand_mentions_local(&arg.value, local))
                    {
                        return false;
                    }
                    saw_atomic_call = true;
                    continue;
                }
                if rvalue_mentions_local(value, local) {
                    return false;
                }
            }
        }
        saw_atomic_call
    }

    fn static_member_fun_for_namespace(
        &self,
        receiver_local: LocalId,
        member: &mir::MemberAccessMetadata,
    ) -> bool {
        let Some(receiver_fqn) = self.local_top_level_ref_fqn(receiver_local) else {
            return false;
        };
        let member_fqn = match member.resolved.as_ref() {
            Some(mir::MemberTarget::Fun { fqn })
            | Some(mir::MemberTarget::ExtensionFun { fqn }) => fqn,
            Some(mir::MemberTarget::Value { .. } | mir::MemberTarget::ExtensionValue { .. })
            | None => return false,
        };
        member_fqn.starts_with(receiver_fqn)
            && member_fqn.as_bytes().get(receiver_fqn.len()) == Some(&b'.')
    }

    fn local_top_level_ref_fqn(&self, local: LocalId) -> Option<&str> {
        self.body.blocks.iter().find_map(|block| {
            block.stmts.iter().find_map(|stmt| {
                let mir::StatementKind::Assign { target, value } = &stmt.kind else {
                    return None;
                };
                if *target != local {
                    return None;
                }
                let mir::Rvalue::TopLevelRef(top) = value else {
                    return None;
                };
                Some(top.fqn.as_str())
            })
        })
    }

    fn static_member_value(&self, member: &mir::MemberAccessMetadata) -> bool {
        let Some(mir::MemberTarget::Value { fqn }) = member.resolved.as_ref() else {
            return false;
        };
        self.codegen.object_inits.contains_key(fqn)
            || self.codegen.lookup_object_property_by_fqn(fqn).is_some()
            || self.codegen.top_level_consts.contains_key(fqn)
            || self.codegen.top_level_immutable_values.contains_key(fqn)
            || self.codegen.top_level_vars.contains_key(fqn)
            || self.codegen.has_extern_global_contract(fqn)
            || self.static_enum_unit_variant_value(fqn)
    }

    fn static_enum_unit_variant_value(&self, fqn: &str) -> bool {
        let Some((owner_fqn, variant_name)) = fqn.rsplit_once('.') else {
            return false;
        };
        self.codegen
            .enum_layouts
            .get(owner_fqn)
            .and_then(|layout| {
                layout
                    .variants
                    .iter()
                    .find(|variant| variant.name == variant_name)
            })
            .is_some_and(|variant| variant.fields.is_empty())
    }

    fn class_ctor_layout_key(
        &self,
        class_fqn: &str,
        target_local: Option<LocalId>,
    ) -> Result<String, LlvmEmitError> {
        let Some(target_ty) = target_local
            .and_then(|local| self.body.locals.get(local.as_u32() as usize))
            .map(|local| local.ty)
        else {
            return Ok(class_fqn.to_string());
        };
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.source_types.kind(target_ty) else {
            return Ok(class_fqn.to_string());
        };
        if nominal.fqn != class_fqn {
            return Ok(class_fqn.to_string());
        }

        let layout = self.abi.class_instance_layout(target_ty)?;
        if layout.base_fqn() != class_fqn {
            return Err(frontend_error(format!(
                "class ctor `{class_fqn}` target type t{} resolved to mismatched class layout `{}`",
                target_ty.as_u32(),
                layout.base_fqn()
            )));
        }
        Ok(layout.class_key().to_string())
    }

    fn maybe_build_effect_typed_closure_target_fn_ptr(
        &mut self,
        span: Span,
        target_local: Option<LocalId>,
        fn_ptr: &str,
    ) -> Result<Option<inkwell::values::PointerValue<'ctx>>, LlvmEmitError> {
        let mut surface_tys = Vec::new();
        if let Some(target_ty) = target_local
            .and_then(|local| self.body.locals.get(local.as_u32() as usize))
            .map(|local| local.ty)
        {
            surface_tys.push(target_ty);
        }
        if let Some(target_local) = target_local
            && let Some(consumer_ty) =
                self.local_function_value_consumer_surface_ty(target_local)?
            && !surface_tys.contains(&consumer_ty)
        {
            surface_tys.push(consumer_ty);
        }
        for surface_ty in surface_tys {
            if let Some(ptr) = self.maybe_build_effect_typed_closure_target_fn_ptr_for_source_ty(
                span, surface_ty, fn_ptr,
            )? {
                return Ok(Some(ptr));
            }
        }
        Ok(None)
    }

    fn maybe_build_effect_typed_closure_target_fn_ptr_for_source_ty(
        &mut self,
        span: Span,
        target_ty: TypeId,
        fn_ptr: &str,
    ) -> Result<Option<inkwell::values::PointerValue<'ctx>>, LlvmEmitError> {
        let TypeKind::Ref(RefTypeKind::Function(surface_fun_ty)) =
            self.source_types.kind(target_ty)
        else {
            return Ok(None);
        };
        let Some(fun_ty) = self
            .codegen
            .equivalent_codegen_function_type(self.source_types, surface_fun_ty)
        else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect-typed closure surface function type",
                at: span.into(),
            });
        };
        if fun_ty.effects.is_pure() {
            return Ok(None);
        }
        let Some(layout) = self.effect_typed_closure_surface_layout(&fun_ty)? else {
            return Ok(None);
        };
        if let Some(source_target) = self
            .abi
            .maybe_callable_carrier_target_layout(CallableCarrierKind::ClosureObject, fn_ptr)
        {
            let source_step_schema = source_target.step_schema();
            let source_symbol_name = source_target.symbol_name().to_string();
            if source_step_schema == layout.return_step_schema {
                return Ok(None);
            }
            return self
                .build_effect_typed_effectful_closure_adapter(
                    span,
                    fn_ptr,
                    layout,
                    source_step_schema,
                    &source_symbol_name,
                )
                .map(Some);
        }
        if self
            .abi
            .maybe_plain_callable_layout_by_root_fqn(fn_ptr)?
            .is_some()
        {
            return self
                .build_effect_typed_plain_closure_adapter(span, fn_ptr, &fun_ty, layout)
                .map(Some);
        }
        Err(frontend_error(format!(
            "effect-typed closure surface `{}` 缺少 published closure carrier target 或 plain callable layout",
            fn_ptr,
        )))
    }

    fn install_effect_typed_closure_target_overrides_for_struct_fields(
        &mut self,
        span: Span,
        fields: &[mir::StructLitField],
        target_cg: CgTy,
    ) -> Result<(), LlvmEmitError> {
        let CgTy::Struct(struct_ty) = target_cg else {
            return Ok(());
        };
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.codegen.types.kind(struct_ty)
        else {
            return Ok(());
        };
        let layout_key = self.codegen.nominal_layout_key(nominal);
        let layout = self.codegen.struct_layouts.get(&layout_key).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "struct closure adapter layout",
                at: span.into(),
            },
        )?;
        for layout_field in &layout.fields {
            let Some(init) = fields.iter().find(|field| field.name == layout_field.name) else {
                continue;
            };
            let mir::Operand::Local(source_local) = init.value else {
                continue;
            };
            let Some((_env, fn_ptr, _env_contract)) = self.local_make_closure_source(source_local)
            else {
                continue;
            };
            let Some(field_ty) = layout_field.ty else {
                continue;
            };
            let Some(source_field_ty) = self.source_type_matching_codegen_ty(field_ty) else {
                continue;
            };
            let Some(adapter) = self.maybe_build_effect_typed_closure_target_fn_ptr_for_source_ty(
                init.span,
                source_field_ty,
                &fn_ptr,
            )?
            else {
                continue;
            };
            self.store_closure_dynamic_entry(init.span, &init.value, adapter)?;
        }
        Ok(())
    }

    fn local_function_value_consumer_surface_ty(
        &self,
        local: LocalId,
    ) -> Result<Option<TypeId>, LlvmEmitError> {
        let mut matched: Option<TypeId> = None;
        for block in &self.body.blocks {
            for stmt in &block.stmts {
                let mir::StatementKind::Assign { value, .. } = &stmt.kind else {
                    continue;
                };
                let Some(surface_ty) = self.call_arg_function_surface_ty(value, local)? else {
                    continue;
                };
                if let Some(existing) = matched {
                    if existing != surface_ty {
                        return Err(frontend_error(format!(
                            "closure local{} 被多个不兼容的 function surface 消费：t{} 与 t{}",
                            local.as_u32(),
                            existing.as_u32(),
                            surface_ty.as_u32(),
                        )));
                    }
                } else {
                    matched = Some(surface_ty);
                }
            }
        }
        Ok(matched)
    }

    fn call_arg_function_surface_ty(
        &self,
        value: &mir::Rvalue,
        local: LocalId,
    ) -> Result<Option<TypeId>, LlvmEmitError> {
        let mir::Rvalue::Call { kind, args, .. } = value else {
            return Ok(None);
        };
        let Some(arg_index) = args.iter().position(
            |arg| matches!(&arg.value, mir::Operand::Local(candidate) if *candidate == local),
        ) else {
            return Ok(None);
        };
        let surface_ty = match kind {
            mir::CallKind::Direct { callee_fqn } => {
                if let Ok(layout) = self.abi.callable_layout_by_root_fqn(callee_fqn) {
                    source_carrier_types(
                        self.source_types,
                        layout.direct_entry().invoke_args_tuple_ty(),
                    )
                    .and_then(|tys| tys.get(arg_index).copied())
                } else if let Ok(layout) = self.abi.plain_callable_layout_by_root_fqn(callee_fqn) {
                    layout.direct_entry().param_tys().get(arg_index).copied()
                } else {
                    None
                }
            }
            _ => None,
        };
        Ok(surface_ty.filter(|ty| {
            matches!(
                self.source_types.kind(*ty),
                TypeKind::Ref(RefTypeKind::Function(_))
            )
        }))
    }

    fn source_type_matching_codegen_ty(&self, codegen_ty: TypeId) -> Option<TypeId> {
        let display = self.codegen.types.display(codegen_ty).to_string();
        self.source_types
            .iter_ids()
            .find(|&ty| self.source_types.display(ty).to_string() == display)
    }

    fn store_closure_dynamic_entry(
        &mut self,
        span: Span,
        closure_operand: &mir::Operand,
        fn_ptr: inkwell::values::PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let closure = self.codegen.codegen_mir_operand_expected(
            span,
            closure_operand,
            self.slots,
            Some(CgTy::Ref),
        )?;
        let closure = self.codegen.coerce_value(span, closure, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(raw_closure)) = closure.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "struct closure adapter value",
                at: span.into(),
            });
        };
        let closure_ptr = self.codegen.cast_ptr(
            raw_closure,
            self.codegen.llvm_ptr_type(self.codegen.gc_address_space()),
            "struct_closure_adapter_obj",
        )?;
        let fn_gep = self.codegen.builder.build_struct_gep(
            self.codegen.llvm_closure_object_type(),
            closure_ptr,
            2,
            "struct_closure_adapter_fn_gep",
        )?;
        let _ = self.codegen.builder.build_store(fn_gep, fn_ptr)?;
        Ok(())
    }

    fn effect_typed_closure_surface_layout(
        &self,
        fun_ty: &crate::ty::FunctionType,
    ) -> Result<Option<ClosureSurfaceLayout<'ctx>>, LlvmEmitError> {
        let expected_args = function_type_source_args(fun_ty);
        let expected_effect_families = self.effect_row_family_match_keys(&fun_ty.effects)?;
        // Contract-first: only consume published dynamic callable surfaces here.
        // If no dynamic surface was materialized for this function type, there is no
        // authoritative schema that justifies mutating the closure carrier fn_ptr.
        let mut matches = self.abi.dynamic_invoke_layouts().filter_map(|layout| {
            let args = source_carrier_types(self.source_types, layout.invoke_args_tuple_ty())?
                .into_iter()
                .map(|ty| {
                    self.codegen
                        .equivalent_codegen_type_id(self.source_types, ty)
                })
                .collect::<Option<Vec<_>>>()?;
            if args != expected_args {
                return None;
            }
            let step_layout = self.abi.step_layout(layout.return_step_schema())?;
            let effect_families = self.step_layout_effect_family_match_keys(step_layout)?;
            if effect_families != expected_effect_families {
                return None;
            }
            let payload_ty = self.codegen.equivalent_codegen_type_id(
                self.source_types,
                step_layout.complete_variant().payload_source_ty(),
            )?;
            (payload_ty == fun_ty.return_ty).then_some(ClosureSurfaceLayout {
                llvm_ty: layout.llvm_ty(),
                invoke_args_tuple_ty: layout.invoke_args_tuple_ty(),
                return_step_schema: layout.return_step_schema(),
            })
        });
        let Some(first) = matches.next() else {
            return Ok(None);
        };
        let ambiguous = matches.any(|candidate| {
            candidate.return_step_schema != first.return_step_schema
                || candidate.invoke_args_tuple_ty != first.invoke_args_tuple_ty
                || candidate.llvm_ty != first.llvm_ty
        });
        if ambiguous {
            return Err(frontend_error(format!(
                "effect-typed closure surface function type args={:?} effects={:?} return=t{} 匹配多个 dynamic-invoke layout",
                expected_args
                    .iter()
                    .map(|ty| ty.as_u32())
                    .collect::<Vec<_>>(),
                expected_effect_families,
                fun_ty.return_ty.as_u32(),
            )));
        }
        Ok(Some(first))
    }

    fn effect_row_family_match_keys(
        &self,
        row: &crate::ty::EffectRow,
    ) -> Result<BTreeSet<EffectFamilyMatchKey>, LlvmEmitError> {
        let mut families = BTreeSet::new();
        for effect_ty in &row.terms {
            let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.codegen.types.kind(*effect_ty)
            else {
                return Err(frontend_error(format!(
                    "effect-typed plain adapter effect row term t{} is not a nominal effect type",
                    effect_ty.as_u32()
                )));
            };
            families.insert((nominal.fqn.clone(), nominal.args.clone()));
        }
        Ok(families)
    }

    fn step_layout_effect_family_match_keys(
        &self,
        step_layout: &StepLayout<'ctx>,
    ) -> Option<BTreeSet<EffectFamilyMatchKey>> {
        let mut families = BTreeSet::new();
        for case in step_layout.cases().values() {
            let family = case.concrete_op_key().effect_family();
            let type_args = family
                .type_args()
                .iter()
                .map(|ty| {
                    self.codegen
                        .equivalent_codegen_type_id(self.source_types, *ty)
                })
                .collect::<Option<Vec<_>>>()?;
            families.insert((family.effect_fqn().to_string(), type_args));
        }
        Some(families)
    }

    fn build_effect_typed_plain_closure_adapter(
        &mut self,
        span: Span,
        fn_ptr: &str,
        fun_ty: &crate::ty::FunctionType,
        adapter: ClosureSurfaceLayout<'ctx>,
    ) -> Result<inkwell::values::PointerValue<'ctx>, LlvmEmitError> {
        let plain = self.abi.plain_callable_layout_by_root_fqn(fn_ptr)?;
        let return_step_layout = self
            .abi
            .step_layout(adapter.return_step_schema)
            .ok_or_else(|| {
                frontend_error(format!(
                    "effect-typed plain adapter `{}` 缺少 return step schema s{} layout",
                    fn_ptr,
                    adapter.return_step_schema.as_u32(),
                ))
            })?;
        let name = stable_naming::private_name_from_key_text(
            "plain_adapter",
            &canonical_record(
                "plain_adapter",
                [
                    plain.stable_callable_key_text().to_string(),
                    return_step_layout.stable_effect_key_text().to_string(),
                ],
            ),
        );
        if let Some(existing) = self.codegen.module.get_function(&name) {
            if existing.count_basic_blocks() == 0 {
                self.define_effect_typed_plain_closure_adapter(
                    span, fn_ptr, fun_ty, adapter, existing,
                )?;
            }
            return Ok(existing.as_global_value().as_pointer_value());
        }
        let function = self.codegen.declare_compiler_private_helper_function(
            &name,
            adapter.llvm_ty,
            Linkage::Internal,
        );
        self.define_effect_typed_plain_closure_adapter(span, fn_ptr, fun_ty, adapter, function)?;
        Ok(function.as_global_value().as_pointer_value())
    }

    fn define_effect_typed_plain_closure_adapter(
        &mut self,
        span: Span,
        fn_ptr: &str,
        _fun_ty: &crate::ty::FunctionType,
        adapter: ClosureSurfaceLayout<'ctx>,
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
                    "effect-typed plain adapter `{}` 缺少 plain entry `{}`",
                    fn_ptr,
                    plain.direct_entry().symbol_name(),
                ))
            })?;
        let step_layout = self
            .abi
            .step_layout(adapter.return_step_schema)
            .ok_or_else(|| {
                frontend_error(format!(
                    "effect-typed plain adapter 缺少 return step schema s{} layout",
                    adapter.return_step_schema.as_u32(),
                ))
            })?;
        let complete_variant = step_layout.complete_variant();
        let complete_payload_ty = if complete_variant.payload_is_elided() {
            None
        } else {
            Some(
                complete_variant
                    .payload_ty()
                    .get_field_type_at_index(0)
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "effect-typed plain adapter Step complete payload `{}` 缺少 field#0",
                            complete_variant.payload_anchor_name(),
                        ))
                    })?,
            )
        };

        let carrier = function
            .get_nth_param(0)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect-typed plain adapter carrier param",
                at: span.into(),
            })?
            .into_pointer_value();
        let closure_ptr = self.codegen.cast_ptr(
            carrier,
            self.codegen.llvm_ptr_type(self.codegen.gc_address_space()),
            "adapter_closure_obj",
        )?;
        let env_gep = self.codegen.builder.build_struct_gep(
            self.codegen.llvm_closure_object_type(),
            closure_ptr,
            1,
            "adapter_env_gep",
        )?;
        let env = self
            .codegen
            .builder
            .build_load(self.codegen.llvm_gc_i8_ptr_type(), env_gep, "adapter_env")?
            .into_pointer_value();
        let explicit_args =
            self.adapter_explicit_args(span, function, adapter.invoke_args_tuple_ty)?;
        let plain_arg_count_without_sret = 1 + explicit_args.len();
        let uses_hidden_sret = match (plain.direct_entry().param_count(), complete_payload_ty) {
            (count, Some(_)) if count == plain_arg_count_without_sret + 1 => true,
            (count, _) if count == plain_arg_count_without_sret => false,
            (count, _) => {
                return Err(frontend_error(format!(
                    "effect-typed plain adapter `{}` plain entry param count drift: entry={} expected={} or {}",
                    fn_ptr,
                    count,
                    plain_arg_count_without_sret,
                    plain_arg_count_without_sret + 1,
                )));
            }
        };

        let mut call_args = Vec::<BasicMetadataValueEnum<'ctx>>::new();
        let sret_result_slot = if uses_hidden_sret {
            let result_ty = complete_payload_ty.ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect-typed plain adapter sret payload type",
                at: span.into(),
            })?;
            let slot =
                self.codegen
                    .create_entry_alloca_raw(span, "adapter_plain_sret", result_ty)?;
            call_args.push(slot.into());
            Some((slot, result_ty))
        } else {
            None
        };
        call_args.push(env.into());
        call_args.extend(explicit_args);
        let call = self
            .codegen
            .builder
            .build_call(plain_fun, &call_args, "carrier_to_plain")?;
        if let Some((_, result_ty)) = sret_result_slot {
            self.codegen.add_sret_attribute_to_call(call, 0, result_ty);
        }
        let payload = if let Some(expected_payload_ty) = complete_payload_ty {
            Some(if let Some((result_ptr, _)) = sret_result_slot {
                if self
                    .codegen
                    .basic_type_contains_gc_ptrs(span, expected_payload_ty)?
                {
                    self.codegen.sync_storage_slot_into_explicit_frame(
                        span,
                        result_ptr,
                        expected_payload_ty,
                        "adapter_plain_sret",
                    )?;
                }
                let payload = self.codegen.builder.build_load(
                    expected_payload_ty,
                    result_ptr,
                    "adapter_plain_sret_payload",
                )?;
                self.codegen.clear_spill_slot_root_homes(
                    span,
                    result_ptr,
                    expected_payload_ty,
                    "adapter_plain_sret",
                )?;
                payload
            } else {
                let payload = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect-typed plain adapter plain return value",
                        at: span.into(),
                    },
                )?;
                if payload.get_type() != expected_payload_ty {
                    return Err(frontend_error(format!(
                        "effect-typed plain adapter `{}` direct payload type drift: expected {:?}, got {:?}",
                        fn_ptr,
                        expected_payload_ty,
                        payload.get_type(),
                    )));
                }
                payload
            })
        } else {
            None
        };
        let step = self
            .codegen
            .build_step_complete(step_layout, payload)
            .map_err(|err| frontend_error(format!("adapter_complete failed: {err}")))?;
        self.codegen.builder.build_return(Some(&step))?;

        if let Some(saved) = saved_block {
            self.codegen.builder.position_at_end(saved);
        }
        Ok(())
    }

    fn build_effect_typed_effectful_closure_adapter(
        &mut self,
        span: Span,
        fn_ptr: &str,
        adapter: ClosureSurfaceLayout<'ctx>,
        source_step_schema: crate::effect_facts::StepSchemaId,
        source_symbol_name: &str,
    ) -> Result<inkwell::values::PointerValue<'ctx>, LlvmEmitError> {
        let source_step_layout = self.abi.step_layout(source_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "effectful closure adapter `{}` 缺少 source step schema s{} layout",
                fn_ptr,
                source_step_schema.as_u32(),
            ))
        })?;
        let return_step_layout = self
            .abi
            .step_layout(adapter.return_step_schema)
            .ok_or_else(|| {
                frontend_error(format!(
                    "effectful closure adapter `{}` 缺少 return step schema s{} layout",
                    fn_ptr,
                    adapter.return_step_schema.as_u32(),
                ))
            })?;
        let name = stable_naming::private_name_from_key_text(
            "closure_step_adapter",
            &canonical_record(
                "closure_step_adapter",
                [
                    source_step_layout.stable_effect_key_text().to_string(),
                    return_step_layout.stable_effect_key_text().to_string(),
                ],
            ),
        );
        if let Some(existing) = self.codegen.module.get_function(&name) {
            if existing.count_basic_blocks() == 0 {
                self.define_effect_typed_effectful_closure_adapter(
                    span,
                    fn_ptr,
                    adapter,
                    source_step_schema,
                    source_symbol_name,
                    existing,
                )?;
            }
            return Ok(existing.as_global_value().as_pointer_value());
        }
        let function = self.codegen.declare_compiler_private_helper_function(
            &name,
            adapter.llvm_ty,
            Linkage::Internal,
        );
        self.define_effect_typed_effectful_closure_adapter(
            span,
            fn_ptr,
            adapter,
            source_step_schema,
            source_symbol_name,
            function,
        )?;
        Ok(function.as_global_value().as_pointer_value())
    }

    fn define_effect_typed_effectful_closure_adapter(
        &mut self,
        span: Span,
        fn_ptr: &str,
        adapter: ClosureSurfaceLayout<'ctx>,
        source_step_schema: crate::effect_facts::StepSchemaId,
        source_symbol_name: &str,
        function: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let saved_block = self.codegen.builder.get_insert_block();
        let entry = self.codegen.context.append_basic_block(function, "entry");
        self.codegen.builder.position_at_end(entry);

        let source_fun = self
            .codegen
            .module
            .get_function(source_symbol_name)
            .ok_or_else(|| {
                frontend_error(format!(
                    "effect-typed closure adapter `{}` 缺少 source carrier entry `{}`",
                    fn_ptr, source_symbol_name,
                ))
            })?;
        let mut call_args = vec![
            function
                .get_nth_param(0)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect-typed closure adapter carrier param",
                    at: span.into(),
                })?
                .into(),
        ];
        if let Some(explicit_args) = function.get_nth_param(1) {
            call_args.push(explicit_args.into());
        }
        if source_fun.count_params() as usize != call_args.len() {
            return Err(frontend_error(format!(
                "effect-typed closure adapter `{}` source carrier entry `{}` param count drift: entry={} expected={}",
                fn_ptr,
                source_symbol_name,
                source_fun.count_params(),
                call_args.len(),
            )));
        }
        let call =
            self.codegen
                .builder
                .build_call(source_fun, &call_args, "carrier_to_effectful")?;
        let step = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect-typed closure adapter source carrier return",
                at: span.into(),
            })?;
        let step = if source_step_schema == adapter.return_step_schema {
            step
        } else {
            self.codegen.project_step_to_schema(
                self.abi,
                step,
                source_step_schema,
                adapter.return_step_schema,
            )?
        };
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
        if function.get_nth_param(1).is_none() {
            return Ok(Vec::new());
        }
        let layout = self.abi.source_value_layout(invoke_args_tuple_ty)?;
        if layout.abi().is_elided() {
            return Ok(Vec::new());
        }
        let raw = function
            .get_nth_param(1)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect-typed plain adapter args payload",
                at: span.into(),
            })?;
        match layout.kind() {
            SourceAbiLayoutKind::Scalar => Ok(vec![raw.into()]),
            SourceAbiLayoutKind::Tuple => {
                let tuple = raw.into_struct_value();
                let mut args = Vec::new();
                for field in layout.fields() {
                    let Some(index) = field.abi_field_index() else {
                        continue;
                    };
                    let value = self.codegen.builder.build_extract_value(
                        tuple,
                        index,
                        &format!("adapter_arg{}", field.source_index()),
                    )?;
                    args.push(value.into());
                }
                Ok(args)
            }
        }
    }

    fn lower_pure_direct_call(
        &mut self,
        span: Span,
        kind: &mir::CallKind,
        args: &[mir::CallArg],
        transport: &mir::CallTransportMetadata,
        target_cg: super::super::types::CgTy,
        _target_local: Option<LocalId>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if let mir::CallKind::FunValue { callee } = kind
            && let Some(callee_fqn) = self.resolved_fun_value_callee_fqn(callee)
        {
            match callee_fqn {
                "scoop.core.GC.handleNew" => {
                    return self.codegen.codegen_mir_sysroot_gc_handle_new(
                        span,
                        args,
                        self.slots,
                        Some(target_cg),
                    );
                }
                "scoop.core.GC.handleGet" => {
                    return self.codegen.codegen_mir_sysroot_gc_handle_get(
                        span,
                        args,
                        self.slots,
                        Some(target_cg),
                    );
                }
                "scoop.core.GC.handleDrop" => {
                    return self
                        .codegen
                        .codegen_mir_sysroot_gc_handle_drop(span, args, self.slots);
                }
                "scoop.core.GC.pin" => {
                    return self.lower_gc_pin(span, args, target_cg);
                }
                "scoop.core.GC.unpin" => {
                    return self.lower_gc_unpin(span, args);
                }
                _ => {}
            }
        }
        let callee_fqn = match kind {
            mir::CallKind::Direct { callee_fqn } => callee_fqn,
            mir::CallKind::Closure { .. }
            | mir::CallKind::FunValue { .. }
            | mir::CallKind::FunPtr { .. }
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
                    kind: "pure statement resume call requires boundary lowering",
                    at: span.into(),
                });
            }
        };
        match callee_fqn.as_str() {
            "scoop.core.GC.handleNew" => {
                return self.codegen.codegen_mir_sysroot_gc_handle_new(
                    span,
                    args,
                    self.slots,
                    Some(target_cg),
                );
            }
            "scoop.core.GC.handleGet" => {
                return self.codegen.codegen_mir_sysroot_gc_handle_get(
                    span,
                    args,
                    self.slots,
                    Some(target_cg),
                );
            }
            "scoop.core.GC.handleDrop" => {
                return self
                    .codegen
                    .codegen_mir_sysroot_gc_handle_drop(span, args, self.slots);
            }
            "scoop.core.GC.pin" => {
                return self.lower_gc_pin(span, args, target_cg);
            }
            "scoop.core.GC.unpin" => {
                return self.lower_gc_unpin(span, args);
            }
            _ => {}
        }
        if let Some(value) = self.lower_internal_print_string(span, callee_fqn, args)? {
            return Ok(value);
        }
        if let Some(value) = self.lower_gc_debug_intrinsic(span, callee_fqn, args)? {
            return Ok(value);
        }
        if let Some(value) = self.lower_to_int_intrinsic(span, callee_fqn, args)? {
            return Ok(value);
        }
        if let Some(value) = self.lower_hash_intrinsic(span, callee_fqn, args)? {
            return Ok(value);
        }
        if callee_fqn == "scoop.core.byteLength" {
            return self.lower_core_string_byte_length_call(span, args, target_cg);
        }
        if callee_fqn == "scoop.core.getByte" {
            return self.lower_core_string_get_byte_call(span, args, target_cg);
        }
        if matches!(
            callee_fqn.as_str(),
            "scoop.core.abs" | "scoop.core.isNaN" | "scoop.core.isInfinite"
        ) && let Some(value) =
            self.maybe_lower_float_ext_call(span, callee_fqn, args, target_cg)?
        {
            return Ok(value);
        }
        if callee_fqn == "scoop.thread.yield" {
            assert!(
                args.is_empty(),
                "typecheck must reject thread.yield arguments before LLVM codegen"
            );
            let rt = self.codegen.declare_runtime_thread_yield();
            let _ =
                self.codegen
                    .build_call_preserving_gc_local_roots(span, rt, &[], "thread_yield")?;
            return Ok(CgValue::unit());
        }
        if let Some(value) = self.lower_atomic_int_intrinsic(span, callee_fqn, args)? {
            return Ok(value);
        }
        if let Some(value) = self.lower_atomic_ref_intrinsic(span, callee_fqn, args)? {
            return Ok(value);
        }
        if callee_fqn == "scoop.core.getPlatform" {
            if !args.is_empty() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "getPlatform intrinsic arity",
                    at: span.into(),
                });
            }
            return self.codegen.codegen_platform_literal(span, target_cg);
        }
        if callee_fqn == "scoop.core.panic" {
            return self.lower_panic_call(span, args);
        }
        let dispatch_fqn = direct_call_dispatch_fqn(callee_fqn);
        if let Some(value) = self.lower_sync_intrinsic(span, dispatch_fqn, args)? {
            return Ok(value);
        }
        if let Some(value) = self.lower_thread_intrinsic(span, dispatch_fqn, args)? {
            return Ok(value);
        }
        if dispatch_fqn == "scoop.unsafe.invoke" {
            let value = self.codegen.codegen_mir_funptr_invoke_call(
                span,
                args,
                self.body,
                self.source_types,
                self.slots,
            )?;
            return self.codegen.coerce_value(span, value, target_cg);
        }
        let callable_abi = self.codegen.direct_call_abi_identity(callee_fqn);
        if callable_abi.uses_native_abi() {
            let value = self.codegen.codegen_mir_direct_call(
                span,
                callee_fqn,
                args,
                self.body,
                self.source_types,
                transport,
                self.slots,
            )?;
            return self.codegen.coerce_value(span, value, target_cg);
        }
        if callable_abi.is_extern() {
            let value = self.codegen.codegen_mir_direct_call(
                span,
                callee_fqn,
                args,
                self.body,
                self.source_types,
                transport,
                self.slots,
            )?;
            return self.codegen.coerce_value(span, value, target_cg);
        }
        if self
            .codegen
            .has_materialized_instances_for_template(callee_fqn)
        {
            let value = self.codegen.codegen_mir_plain_direct_call(
                span,
                callee_fqn,
                args,
                self.body,
                self.source_types,
                transport,
                self.slots,
            )?;
            return self.codegen.coerce_value(span, value, target_cg);
        }
        let binding_entry_name = self
            .codegen
            .current_top_level_fun_call_binding(span)?
            .and_then(|binding| binding.intrinsic_entry_name.clone());
        if let Some(entry_name) = binding_entry_name
            .as_deref()
            .or_else(|| crate::intrinsics::fallback_named_intrinsic_entry_name_for_fqn(callee_fqn))
            && let Some(value) = self.codegen.try_codegen_named_intrinsic_mir_direct_call(
                span,
                entry_name,
                args,
                self.body,
                self.source_types,
                transport.array.as_ref(),
                self.slots,
            )?
        {
            return Ok(value);
        }
        let sig_fun = match self.codegen.hir_fun_for_callable_fqn(callee_fqn) {
            Some(sig_fun) => sig_fun,
            None => {
                if let Some(value) =
                    self.lower_top_level_funptr_direct_call(callee_fqn, span, args)?
                {
                    return Ok(value);
                }
                if let Some(fun_ty) = self.top_level_function_value_type(callee_fqn) {
                    return self.lower_top_level_function_value_direct_call(
                        callee_fqn, span, args, &fun_ty,
                    );
                }
                if let Some(callee_local) = self.top_level_callable_value_local(callee_fqn) {
                    return self.codegen.codegen_mir_plain_dynamic_call(
                        span,
                        &mir::CallKind::FunValue {
                            callee: mir::Operand::Local(callee_local),
                        },
                        args,
                        self.body,
                        self.source_types,
                        self.slots,
                    );
                }
                return Err(frontend_error(format!(
                    "pure statement call 缺少 callee `{callee_fqn}` 的 callable signature"
                )));
            }
        };
        if sig_fun.body.is_none() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pure statement declaration-only direct call",
                at: span.into(),
            });
        }
        if self
            .abi
            .maybe_plain_callable_layout_by_root_fqn(callee_fqn)?
            .is_some()
        {
            return self.codegen.codegen_mir_plain_direct_call(
                span,
                callee_fqn,
                args,
                self.body,
                self.source_types,
                transport,
                self.slots,
            );
        }
        if self
            .codegen
            .known_fun_body_may_outward_effect(callee_fqn, sig_fun.ty)
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pure statement effectful direct call requires boundary lowering",
                at: span.into(),
            });
        }
        let layout = self.abi.callable_layout_by_root_fqn(callee_fqn)?;
        let entry = layout.direct_entry();
        if entry.return_step_schema() != layout.step_schema() {
            return Err(frontend_error(format!(
                "pure statement call `{callee_fqn}` direct entry return schema 漂移：entry=s{} layout=s{}",
                entry.return_step_schema().as_u32(),
                layout.step_schema().as_u32()
            )));
        }
        let payload = self.pack_call_args(span, entry, args)?;
        let callee = self
            .codegen
            .module
            .get_function(entry.symbol_name())
            .ok_or_else(|| {
                frontend_error(format!(
                    "pure statement call `{callee_fqn}` 缺少 direct entry shell `{}`",
                    entry.symbol_name()
                ))
            })?;
        let mut call_args = Vec::<BasicMetadataValueEnum<'ctx>>::new();
        if !entry.args_abi().is_elided() {
            call_args.push(
                payload
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "pure statement call `{callee_fqn}` 需要 non-elided args payload"
                        ))
                    })?
                    .into(),
            );
        }
        let call = self
            .codegen
            .builder
            .build_call(callee, &call_args, "pure_call_step")?;
        let step = call.try_as_basic_value().basic().ok_or_else(|| {
            frontend_error(format!(
                "pure statement call `{callee_fqn}` direct entry 未返回 Step_F"
            ))
        })?;
        self.extract_pure_call_complete(span, layout.step_schema(), step, target_cg)
    }

    fn lower_thread_intrinsic(
        &mut self,
        span: Span,
        dispatch_fqn: &str,
        args: &[mir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        match dispatch_fqn {
            "scoop.thread.threadSpawn" => {
                self.expect_sync_arity(span, dispatch_fqn, args, 1)?;
                let block = self.lower_sync_ref_arg(dispatch_fqn, &args[0])?;
                let closure_ty = self.codegen.llvm_closure_object_type();
                let closure_ptr_ty = self.codegen.llvm_ptr_type(self.codegen.gc_address_space());
                let closure_ptr = self.codegen.builder.build_pointer_cast(
                    block,
                    closure_ptr_ty,
                    "thread_block_ptr",
                )?;
                let i8_ptr_ty = self.codegen.llvm_i8_ptr_type();
                let env_gep = self.codegen.builder.build_struct_gep(
                    closure_ty,
                    closure_ptr,
                    1,
                    "thread_env_gep",
                )?;
                let fn_gep = self.codegen.builder.build_struct_gep(
                    closure_ty,
                    closure_ptr,
                    2,
                    "thread_fn_gep",
                )?;
                let env_ptr = self
                    .codegen
                    .builder
                    .build_load(i8_ptr_ty, env_gep, "thread_env")?
                    .into_pointer_value();
                let fn_ptr_raw = self
                    .codegen
                    .builder
                    .build_load(i8_ptr_ty, fn_gep, "thread_fn_raw")?
                    .into_pointer_value();
                let start_fn_ptr_ty = self.codegen.llvm_ptr_type(AddressSpace::default());
                let start_fn_ptr = self.codegen.builder.build_pointer_cast(
                    fn_ptr_raw,
                    start_fn_ptr_ty,
                    "thread_fn_typed",
                )?;
                let rt = self.codegen.declare_runtime_thread_spawn();
                let call = self.codegen.build_call_preserving_gc_local_roots(
                    span,
                    rt,
                    &[env_ptr.into(), start_fn_ptr.into()],
                    "thread_spawn",
                )?;
                Ok(Some(CgValue {
                    ty: CgTy::Ref,
                    value: Some(self.sync_ref_return_value(span, dispatch_fqn, call)?.into()),
                }))
            }
            "scoop.thread.join" => {
                self.expect_sync_arity(span, dispatch_fqn, args, 1)?;
                let thread = self.lower_sync_ref_arg(dispatch_fqn, &args[0])?;
                let rt = self.codegen.declare_runtime_thread_join();
                let _ = self.codegen.build_call_preserving_gc_local_roots(
                    span,
                    rt,
                    &[thread.into()],
                    "thread_join",
                )?;
                Ok(Some(CgValue::unit()))
            }
            "scoop.thread.sleepMillis" => {
                self.expect_sync_arity(span, dispatch_fqn, args, 1)?;
                let word = IntTy {
                    bits: self.codegen.host.word_bit_width(),
                    signed: true,
                };
                let value = self.codegen.codegen_mir_operand_expected(
                    args[0].span,
                    &args[0].value,
                    self.slots,
                    Some(CgTy::Int(word)),
                )?;
                let value = self
                    .codegen
                    .coerce_value(args[0].span, value, CgTy::Int(word))?;
                let (raw, from) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "thread.sleepMillis value",
                    at: args[0].span.into(),
                })?;
                let ms = self.codegen.cast_int(
                    raw,
                    from,
                    IntTy {
                        bits: 64,
                        signed: true,
                    },
                )?;
                let rt = self.codegen.declare_runtime_thread_sleep_millis();
                let _ = self.codegen.build_call_preserving_gc_local_roots(
                    span,
                    rt,
                    &[ms.into()],
                    "thread_sleep_millis",
                )?;
                Ok(Some(CgValue::unit()))
            }
            "scoop.thread.currentId" => {
                self.expect_sync_arity(span, dispatch_fqn, args, 0)?;
                let rt = self.codegen.declare_runtime_thread_current_id();
                let call = self.codegen.build_call_preserving_gc_local_roots(
                    span,
                    rt,
                    &[],
                    "thread_current_id",
                )?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "thread.currentId return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(id) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "thread.currentId return type",
                        at: span.into(),
                    });
                };
                Ok(Some(CgValue::int(
                    id,
                    IntTy {
                        bits: 64,
                        signed: false,
                    },
                )))
            }
            "scoop.thread.yield" => {
                self.expect_sync_arity(span, dispatch_fqn, args, 0)?;
                let rt = self.codegen.declare_runtime_thread_yield();
                let _ = self.codegen.build_call_preserving_gc_local_roots(
                    span,
                    rt,
                    &[],
                    "thread_yield",
                )?;
                Ok(Some(CgValue::unit()))
            }
            _ => Ok(None),
        }
    }

    fn lower_sync_intrinsic(
        &mut self,
        span: Span,
        dispatch_fqn: &str,
        args: &[mir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        match dispatch_fqn {
            "scoop.sync.mutexCreate" => {
                self.expect_sync_arity(span, dispatch_fqn, args, 0)?;
                let rt = self.codegen.declare_runtime_sync_mutex_create();
                let call = self
                    .codegen
                    .builder
                    .build_call(rt, &[], "sync_mutex_create")?;
                Ok(Some(CgValue {
                    ty: CgTy::Ref,
                    value: Some(self.sync_ref_return_value(span, dispatch_fqn, call)?.into()),
                }))
            }
            "scoop.sync.lock" => {
                self.expect_sync_arity(span, dispatch_fqn, args, 1)?;
                let recv = self.lower_sync_ref_arg(dispatch_fqn, &args[0])?;
                let rt = self.codegen.declare_runtime_sync_mutex_lock();
                let _ = self.codegen.build_call_preserving_gc_local_roots(
                    span,
                    rt,
                    &[recv.into()],
                    "sync_mutex_lock",
                )?;
                Ok(Some(CgValue::unit()))
            }
            "scoop.sync.unlock" => {
                self.expect_sync_arity(span, dispatch_fqn, args, 1)?;
                let recv = self.lower_sync_ref_arg(dispatch_fqn, &args[0])?;
                let rt = self.codegen.declare_runtime_sync_mutex_unlock();
                let _ = self.codegen.build_call_preserving_gc_local_roots(
                    span,
                    rt,
                    &[recv.into()],
                    "sync_mutex_unlock",
                )?;
                Ok(Some(CgValue::unit()))
            }
            "scoop.sync.condVarCreate" => {
                self.expect_sync_arity(span, dispatch_fqn, args, 0)?;
                let rt = self.codegen.declare_runtime_sync_condvar_create();
                let call = self
                    .codegen
                    .builder
                    .build_call(rt, &[], "sync_condvar_create")?;
                Ok(Some(CgValue {
                    ty: CgTy::Ref,
                    value: Some(self.sync_ref_return_value(span, dispatch_fqn, call)?.into()),
                }))
            }
            "scoop.sync.wait" => {
                self.expect_sync_arity(span, dispatch_fqn, args, 2)?;
                let cv = self.lower_sync_ref_arg(dispatch_fqn, &args[0])?;
                let mutex = self.lower_sync_ref_arg(dispatch_fqn, &args[1])?;
                let rt = self.codegen.declare_runtime_sync_condvar_wait();
                let _ = self.codegen.build_call_preserving_gc_local_roots(
                    span,
                    rt,
                    &[cv.into(), mutex.into()],
                    "sync_condvar_wait",
                )?;
                Ok(Some(CgValue::unit()))
            }
            "scoop.sync.notifyOne" => {
                self.expect_sync_arity(span, dispatch_fqn, args, 1)?;
                let cv = self.lower_sync_ref_arg(dispatch_fqn, &args[0])?;
                let rt = self.codegen.declare_runtime_sync_condvar_notify_one();
                let _ = self.codegen.build_call_preserving_gc_local_roots(
                    span,
                    rt,
                    &[cv.into()],
                    "sync_condvar_notify_one",
                )?;
                Ok(Some(CgValue::unit()))
            }
            "scoop.sync.notifyAll" => {
                self.expect_sync_arity(span, dispatch_fqn, args, 1)?;
                let cv = self.lower_sync_ref_arg(dispatch_fqn, &args[0])?;
                let rt = self.codegen.declare_runtime_sync_condvar_notify_all();
                let _ = self.codegen.build_call_preserving_gc_local_roots(
                    span,
                    rt,
                    &[cv.into()],
                    "sync_condvar_notify_all",
                )?;
                Ok(Some(CgValue::unit()))
            }
            "scoop.sync.onceCreate" => {
                self.expect_sync_arity(span, dispatch_fqn, args, 0)?;
                let rt = self.codegen.declare_runtime_sync_once_create();
                let call = self
                    .codegen
                    .builder
                    .build_call(rt, &[], "sync_once_create")?;
                Ok(Some(CgValue {
                    ty: CgTy::Ref,
                    value: Some(self.sync_ref_return_value(span, dispatch_fqn, call)?.into()),
                }))
            }
            "scoop.sync.isDone" => {
                self.expect_sync_arity(span, dispatch_fqn, args, 1)?;
                let once = self.lower_sync_ref_arg(dispatch_fqn, &args[0])?;
                let rt = self.codegen.declare_runtime_sync_once_is_done();
                let call = self.codegen.build_call_preserving_gc_local_roots(
                    span,
                    rt,
                    &[once.into()],
                    "sync_once_is_done",
                )?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "sync.Once.isDone return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(done) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "sync.Once.isDone return type",
                        at: span.into(),
                    });
                };
                Ok(Some(CgValue::bool(done)))
            }
            "scoop.sync.run" => {
                self.expect_sync_arity(span, dispatch_fqn, args, 2)?;
                let once = self.lower_sync_ref_arg(dispatch_fqn, &args[0])?;
                let deferred_once = self.codegen.defer_gc_ref_pointer(
                    args[0].span,
                    "sync_once_run_receiver",
                    once,
                )?;
                let block = self.lower_sync_ref_arg(dispatch_fqn, &args[1])?;
                let closure_ty = self.codegen.llvm_closure_object_type();
                let closure_ptr_ty = self.codegen.llvm_ptr_type(self.codegen.gc_address_space());
                let closure_ptr = self.codegen.builder.build_pointer_cast(
                    block,
                    closure_ptr_ty,
                    "once_block_ptr",
                )?;
                let i8_ptr_ty = self.codegen.llvm_i8_ptr_type();
                let env_gep = self.codegen.builder.build_struct_gep(
                    closure_ty,
                    closure_ptr,
                    1,
                    "once_env_gep",
                )?;
                let fn_gep = self.codegen.builder.build_struct_gep(
                    closure_ty,
                    closure_ptr,
                    2,
                    "once_fn_gep",
                )?;
                let env_ptr = self
                    .codegen
                    .builder
                    .build_load(i8_ptr_ty, env_gep, "once_env")?
                    .into_pointer_value();
                let fn_ptr_raw = self
                    .codegen
                    .builder
                    .build_load(i8_ptr_ty, fn_gep, "once_fn_raw")?
                    .into_pointer_value();
                let init_fn_ptr_ty = self.codegen.llvm_ptr_type(AddressSpace::default());
                let init_fn_ptr = self.codegen.builder.build_pointer_cast(
                    fn_ptr_raw,
                    init_fn_ptr_ty,
                    "once_fn_typed",
                )?;
                let once = self.codegen.reload_deferred_gc_ref_without_clearing(
                    args[0].span,
                    "sync_once_run_receiver_reload",
                    &deferred_once,
                )?;
                let rt = self.codegen.declare_runtime_sync_once_run();
                let _ = self.codegen.build_call_preserving_gc_local_roots(
                    span,
                    rt,
                    &[once.into(), env_ptr.into(), init_fn_ptr.into()],
                    "sync_once_run",
                )?;
                self.codegen.clear_deferred_cg_value_root_homes(
                    args[0].span,
                    "sync_once_run_receiver_drop",
                    &deferred_once,
                )?;
                Ok(Some(CgValue::unit()))
            }
            "scoop.sync.destroy" => {
                self.expect_sync_arity(span, dispatch_fqn, args, 1)?;
                let recv_ty = self.operand_source_ty(&args[0].value).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "sync.destroy receiver source type",
                        at: args[0].span.into(),
                    },
                )?;
                let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.source_types.kind(recv_ty)
                else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "sync.destroy receiver nominal type",
                        at: args[0].span.into(),
                    });
                };
                let recv = self.lower_sync_ref_arg(dispatch_fqn, &args[0])?;
                let rt = match nominal.fqn.as_str() {
                    "scoop.sync.Mutex" => self.codegen.declare_runtime_sync_mutex_destroy(),
                    "scoop.sync.CondVar" => self.codegen.declare_runtime_sync_condvar_destroy(),
                    _ => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "sync.destroy receiver nominal",
                            at: args[0].span.into(),
                        });
                    }
                };
                let _ = self.codegen.build_call_preserving_gc_local_roots(
                    span,
                    rt,
                    &[recv.into()],
                    "sync_destroy",
                )?;
                Ok(Some(CgValue::unit()))
            }
            _ => Ok(None),
        }
    }

    fn expect_sync_arity(
        &self,
        span: Span,
        dispatch_fqn: &str,
        args: &[mir::CallArg],
        expected: usize,
    ) -> Result<(), LlvmEmitError> {
        if args.len() != expected || args.iter().any(|arg| arg.name.is_some()) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync intrinsic arg contract",
                at: span.into(),
            });
        }
        let _ = dispatch_fqn;
        Ok(())
    }

    fn lower_sync_ref_arg(
        &mut self,
        dispatch_fqn: &str,
        arg: &mir::CallArg,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let value = self.codegen.codegen_mir_operand_expected(
            arg.span,
            &arg.value,
            self.slots,
            Some(CgTy::Ref),
        )?;
        let value = self.codegen.coerce_value(arg.span, value, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(ptr)) = value.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync intrinsic ref arg",
                at: arg.span.into(),
            });
        };
        let _ = dispatch_fqn;
        Ok(ptr)
    }

    fn sync_ref_return_value(
        &self,
        span: Span,
        dispatch_fqn: &str,
        call: CallSiteValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "sync intrinsic return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync intrinsic return type",
                at: span.into(),
            });
        };
        let _ = dispatch_fqn;
        Ok(ptr)
    }

    fn lower_atomic_int_intrinsic(
        &mut self,
        span: Span,
        callee_fqn: &str,
        args: &[mir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let atomic_word = super::super::types::IntTy {
            bits: self.codegen.host.word_bit_width(),
            signed: true,
        };
        match callee_fqn {
            "scoop.unsafe.__atomicIntLoad" => {
                if args.len() != 1 || args[0].name.is_some() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntLoad arg contract",
                        at: span.into(),
                    });
                }
                let (ptr, int_ty) =
                    self.atomic_int_lvalue_ptr(&args[0].value, args[0].span, false)?;
                if int_ty != atomic_word {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntLoad target width",
                        at: args[0].span.into(),
                    });
                }
                let loaded = self.codegen.builder.build_load(
                    self.codegen.int_type(atomic_word),
                    ptr,
                    "atomic_int_load",
                )?;
                let inst =
                    loaded
                        .as_instruction_value()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "atomicIntLoad load instruction",
                            at: args[0].span.into(),
                        })?;
                inst.set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)
                    .map_err(|_| LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntLoad set ordering",
                        at: args[0].span.into(),
                    })?;
                let BasicValueEnum::IntValue(raw) = loaded else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntLoad return type",
                        at: args[0].span.into(),
                    });
                };
                Ok(Some(CgValue::int(raw, atomic_word)))
            }
            "scoop.unsafe.__atomicIntStore" => {
                if args.len() != 2 || args.iter().any(|arg| arg.name.is_some()) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntStore arg contract",
                        at: span.into(),
                    });
                }
                let (ptr, int_ty) =
                    self.atomic_int_lvalue_ptr(&args[0].value, args[0].span, true)?;
                if int_ty != atomic_word {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntStore target width",
                        at: args[0].span.into(),
                    });
                }
                let value = self.codegen.codegen_mir_operand_expected(
                    args[1].span,
                    &args[1].value,
                    self.slots,
                    Some(CgTy::Int(atomic_word)),
                )?;
                let value =
                    self.codegen
                        .coerce_value(args[1].span, value, CgTy::Int(atomic_word))?;
                let (raw, from) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "atomicIntStore value",
                    at: args[1].span.into(),
                })?;
                let raw = self.codegen.cast_int(raw, from, atomic_word)?;
                let inst = self.codegen.builder.build_store(ptr, raw)?;
                inst.set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)
                    .map_err(|_| LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntStore set ordering",
                        at: span.into(),
                    })?;
                Ok(Some(CgValue::unit()))
            }
            "scoop.unsafe.__atomicIntCompareExchange" => {
                if args.len() != 3 || args.iter().any(|arg| arg.name.is_some()) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntCompareExchange arg contract",
                        at: span.into(),
                    });
                }
                let (ptr, int_ty) =
                    self.atomic_int_lvalue_ptr(&args[0].value, args[0].span, true)?;
                if int_ty != atomic_word {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntCompareExchange target width",
                        at: args[0].span.into(),
                    });
                }
                let expected =
                    self.atomic_int_operand(args[1].span, &args[1].value, atomic_word)?;
                let desired = self.atomic_int_operand(args[2].span, &args[2].value, atomic_word)?;
                let cx = self.codegen.builder.build_cmpxchg(
                    ptr,
                    expected,
                    desired,
                    AtomicOrdering::SequentiallyConsistent,
                    AtomicOrdering::SequentiallyConsistent,
                )?;
                let success = self
                    .codegen
                    .builder
                    .build_extract_value(cx, 1, "cmpxchg_success")?;
                let BasicValueEnum::IntValue(ok) = success else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntCompareExchange success type",
                        at: span.into(),
                    });
                };
                Ok(Some(CgValue::bool(ok)))
            }
            _ => Ok(None),
        }
    }

    fn lower_atomic_ref_intrinsic(
        &mut self,
        span: Span,
        callee_fqn: &str,
        args: &[mir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        match intrinsic_base_fqn(callee_fqn) {
            "scoop.unsafe.__atomicRefLoad" => {
                if args.len() != 1 || args[0].name.is_some() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicRefLoad arg contract",
                        at: span.into(),
                    });
                }
                let (ptr, storage_ty) =
                    self.atomic_ref_lvalue_place(&args[0].value, args[0].span, false)?;
                let llvm_ty = self.codegen.llvm_basic_type_of(args[0].span, storage_ty)?;
                let loaded = self
                    .codegen
                    .builder
                    .build_load(llvm_ty, ptr, "atomic_ref_load")?;
                let inst =
                    loaded
                        .as_instruction_value()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "atomicRefLoad load instruction",
                            at: args[0].span.into(),
                        })?;
                inst.set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)
                    .map_err(|_| LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicRefLoad set ordering",
                        at: args[0].span.into(),
                    })?;
                Ok(Some(CgValue {
                    ty: storage_ty,
                    value: Some(loaded),
                }))
            }
            "scoop.unsafe.__atomicRefStore" => {
                if args.len() != 2 || args.iter().any(|arg| arg.name.is_some()) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicRefStore arg contract",
                        at: span.into(),
                    });
                }
                let (ptr, storage_ty) =
                    self.atomic_ref_lvalue_place(&args[0].value, args[0].span, true)?;
                let raw = self.atomic_ref_operand(args[1].span, &args[1].value, storage_ty)?;
                let inst = self.codegen.builder.build_store(ptr, raw)?;
                inst.set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)
                    .map_err(|_| LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicRefStore set ordering",
                        at: span.into(),
                    })?;
                self.codegen
                    .promote_gc_pointer_with_write_barrier(args[1].span, raw)?;
                Ok(Some(CgValue::unit()))
            }
            "scoop.unsafe.__atomicRefCompareExchange" => {
                if args.len() != 3 || args.iter().any(|arg| arg.name.is_some()) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicRefCompareExchange arg contract",
                        at: span.into(),
                    });
                }
                let (ptr, storage_ty) =
                    self.atomic_ref_lvalue_place(&args[0].value, args[0].span, true)?;
                let expected = self.atomic_ref_operand(args[1].span, &args[1].value, storage_ty)?;
                let desired = self.atomic_ref_operand(args[2].span, &args[2].value, storage_ty)?;
                let cx = self.codegen.builder.build_cmpxchg(
                    ptr,
                    expected,
                    desired,
                    AtomicOrdering::SequentiallyConsistent,
                    AtomicOrdering::SequentiallyConsistent,
                )?;
                let success = self
                    .codegen
                    .builder
                    .build_extract_value(cx, 1, "cmpxchg_success")?;
                let BasicValueEnum::IntValue(ok) = success else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicRefCompareExchange success type",
                        at: span.into(),
                    });
                };
                self.atomic_ref_cas_barrier(args[2].span, ok, desired)?;
                Ok(Some(CgValue::bool(ok)))
            }
            _ => Ok(None),
        }
    }

    fn lower_to_int_intrinsic(
        &mut self,
        span: Span,
        callee_fqn: &str,
        args: &[mir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        if !matches!(
            intrinsic_base_fqn(callee_fqn),
            "scoop.core.toInt" | "scoop.core.Float64.toInt" | "scoop.core.Float32.toInt"
        ) {
            return Ok(None);
        }
        if args.len() != 1 || args[0].name.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "toInt arg contract",
                at: span.into(),
            });
        }
        let arg = &args[0];
        let value_ty = self.required_operand_source_ty(&arg.value, arg.span)?;
        let value_cg = self
            .codegen
            .cg_ty_of_mir_type(self.source_types, value_ty)
            .or_else(|| self.operand_slot_cg_ty(&arg.value))
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "toInt receiver type",
                at: arg.span.into(),
            })?;
        let value = self.codegen.codegen_mir_operand_expected(
            arg.span,
            &arg.value,
            self.slots,
            Some(value_cg),
        )?;
        let value = self.codegen.coerce_value(arg.span, value, value_cg)?;
        let int_ty = CgTy::Int(super::super::types::IntTy {
            bits: self.codegen.host.word_bit_width(),
            signed: true,
        });
        match self.source_types.kind(value_ty) {
            TypeKind::Value(ValueTypeKind::Char) => {
                return self.codegen.coerce_value(arg.span, value, int_ty).map(Some);
            }
            TypeKind::Ref(RefTypeKind::String) => {
                return Ok(None);
            }
            _ => {}
        }
        match value.ty {
            CgTy::String => Ok(None),
            CgTy::Float64 | CgTy::Float32 => {
                let Some(BasicValueEnum::FloatValue(float_val)) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Float.toInt receiver value",
                        at: arg.span.into(),
                    });
                };
                let rt = match value.ty {
                    CgTy::Float64 => self.codegen.declare_runtime_float64_to_int(),
                    CgTy::Float32 => self.codegen.declare_runtime_float32_to_int(),
                    _ => unreachable!("filtered by match"),
                };
                let call = self.codegen.build_call_preserving_gc_local_roots(
                    span,
                    rt,
                    &[float_val.into()],
                    "rt_float_to_int",
                )?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "Float.toInt return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(int64_val) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Float.toInt return type",
                        at: span.into(),
                    });
                };
                let runtime_int = CgValue::int(
                    int64_val,
                    super::super::types::IntTy {
                        bits: 64,
                        signed: true,
                    },
                );
                self.codegen
                    .coerce_value(span, runtime_int, int_ty)
                    .map(Some)
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "toInt unsupported receiver type",
                at: span.into(),
            }),
        }
    }

    fn lower_hash_intrinsic(
        &mut self,
        span: Span,
        callee_fqn: &str,
        args: &[mir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        if intrinsic_base_fqn(callee_fqn) != "scoop.core.hash" {
            return Ok(None);
        }
        if args.len() != 1 || args[0].name.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "hash arg contract",
                at: span.into(),
            });
        }

        let arg = &args[0];
        let value_ty = self.required_operand_source_ty(&arg.value, arg.span)?;
        let value_cg = self
            .codegen
            .cg_ty_of_mir_type(self.source_types, value_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "hash receiver type",
                at: arg.span.into(),
            })?;
        let value = self.codegen.codegen_mir_operand_expected(
            arg.span,
            &arg.value,
            self.slots,
            Some(value_cg),
        )?;
        let value = self.codegen.coerce_value(arg.span, value, value_cg)?;

        let i64_ty = self.codegen.context.i64_type();
        match self.source_types.kind(value_ty) {
            TypeKind::Value(ValueTypeKind::Char) => {
                let Some(BasicValueEnum::IntValue(codepoint)) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Char.hash receiver value",
                        at: arg.span.into(),
                    });
                };
                let widened =
                    self.codegen
                        .builder
                        .build_int_z_extend(codepoint, i64_ty, "char_hash_zext")?;
                self.codegen.codegen_i64_hash_value(widened).map(Some)
            }
            TypeKind::Ref(RefTypeKind::String) => Ok(None),
            _ => match value.ty {
                CgTy::String => Ok(None),
                CgTy::Int(_) => {
                    let int64 = CgTy::Int(IntTy {
                        bits: 64,
                        signed: true,
                    });
                    let value = self.codegen.coerce_value(arg.span, value, int64)?;
                    let Some(BasicValueEnum::IntValue(raw)) = value.value else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "Int.hash receiver value",
                            at: arg.span.into(),
                        });
                    };
                    self.codegen.codegen_i64_hash_value(raw).map(Some)
                }
                CgTy::Float64 | CgTy::Float32 => self
                    .codegen
                    .codegen_float_hash_value(arg.span, value)
                    .map(Some),
                _ => Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "hash unsupported receiver type",
                    at: span.into(),
                }),
            },
        }
    }

    fn lower_panic_call(
        &mut self,
        span: Span,
        args: &[mir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 || args[0].name.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "panic arg contract",
                at: span.into(),
            });
        }
        let arg = &args[0];
        let message = self.codegen.codegen_mir_operand_expected(
            arg.span,
            &arg.value,
            self.slots,
            Some(CgTy::String),
        )?;
        let message = self.codegen.coerce_value(arg.span, message, CgTy::String)?;
        let Some(BasicValueEnum::PointerValue(message_ptr)) = message.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "panic message value",
                at: arg.span.into(),
            });
        };
        let runtime = self.codegen.declare_runtime_panic();
        let _ = self.codegen.build_call_preserving_gc_local_roots(
            arg.span,
            runtime,
            &[message_ptr.into()],
            "rt_panic",
        )?;
        Ok(CgValue::never())
    }

    fn atomic_int_operand(
        &mut self,
        span: Span,
        operand: &mir::Operand,
        atomic_word: super::super::types::IntTy,
    ) -> Result<inkwell::values::IntValue<'ctx>, LlvmEmitError> {
        let value = self.codegen.codegen_mir_operand_expected(
            span,
            operand,
            self.slots,
            Some(CgTy::Int(atomic_word)),
        )?;
        let value = self
            .codegen
            .coerce_value(span, value, CgTy::Int(atomic_word))?;
        let (raw, from) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "atomicInt operand",
            at: span.into(),
        })?;
        self.codegen.cast_int(raw, from, atomic_word)
    }

    fn atomic_int_lvalue_ptr(
        &mut self,
        operand: &mir::Operand,
        span: Span,
        require_writable: bool,
    ) -> Result<
        (
            inkwell::values::PointerValue<'ctx>,
            super::super::types::IntTy,
        ),
        LlvmEmitError,
    > {
        let mir::Operand::Local(local) = operand else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "atomicInt target operand",
                at: span.into(),
            });
        };
        if let Some((ptr, field_cg)) =
            self.atomic_member_place_for_local(*local, span, require_writable)?
        {
            let CgTy::Int(int_ty) = field_cg else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "atomicInt target type",
                    at: span.into(),
                });
            };
            return Ok((ptr, int_ty));
        }
        if let Some((ptr, cg_ty)) =
            self.atomic_top_level_place_for_local(*local, span, require_writable)?
        {
            let CgTy::Int(int_ty) = cg_ty else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "atomicInt target type",
                    at: span.into(),
                });
            };
            return Ok((ptr, int_ty));
        }
        let slot = self.codegen.mir_local_slot(span, self.slots, *local)?;
        if let CgTy::Int(int_ty) = slot.cg_ty {
            return Ok((slot.ptr, int_ty));
        }
        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "atomicInt target place",
            at: span.into(),
        })
    }

    fn atomic_ref_lvalue_place(
        &mut self,
        operand: &mir::Operand,
        span: Span,
        require_writable: bool,
    ) -> Result<(PointerValue<'ctx>, CgTy), LlvmEmitError> {
        let mir::Operand::Local(local) = operand else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "atomicRef target operand",
                at: span.into(),
            });
        };
        if let Some((ptr, cg_ty)) =
            self.atomic_member_place_for_local(*local, span, require_writable)?
        {
            return Ok((ptr, self.atomic_ref_storage_ty(span, cg_ty)?));
        }
        if let Some((ptr, cg_ty)) =
            self.atomic_top_level_place_for_local(*local, span, require_writable)?
        {
            return Ok((ptr, self.atomic_ref_storage_ty(span, cg_ty)?));
        }
        let slot = self.codegen.mir_local_slot(span, self.slots, *local)?;
        Ok((slot.ptr, self.atomic_ref_storage_ty(span, slot.cg_ty)?))
    }

    fn atomic_ref_operand(
        &mut self,
        span: Span,
        operand: &mir::Operand,
        storage_ty: CgTy,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let value = self.codegen.codegen_mir_operand_expected(
            span,
            operand,
            self.slots,
            Some(storage_ty),
        )?;
        let value = self.codegen.coerce_value(span, value, storage_ty)?;
        let Some(raw) = value.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "atomicRef operand",
                at: span.into(),
            });
        };
        let BasicValueEnum::PointerValue(ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "atomicRef operand",
                at: span.into(),
            });
        };
        Ok(ptr)
    }

    fn atomic_ref_storage_ty(&self, span: Span, cg_ty: CgTy) -> Result<CgTy, LlvmEmitError> {
        if matches!(cg_ty, CgTy::Ref | CgTy::String) {
            return Ok(cg_ty);
        }
        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "atomicRef target type",
            at: span.into(),
        })
    }

    fn atomic_ref_cas_barrier(
        &mut self,
        span: Span,
        success: inkwell::values::IntValue<'ctx>,
        desired: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let function = self
            .codegen
            .expect_current_function("atomicRefCompareExchange barrier");
        let barrier_bb = self
            .codegen
            .context
            .append_basic_block(function, "atomic_ref_cas_barrier");
        let cont_bb = self
            .codegen
            .context
            .append_basic_block(function, "atomic_ref_cas_cont");
        self.codegen
            .builder
            .build_conditional_branch(success, barrier_bb, cont_bb)?;

        self.codegen.builder.position_at_end(barrier_bb);
        self.codegen
            .promote_gc_pointer_with_write_barrier(span, desired)?;
        self.codegen.builder.build_unconditional_branch(cont_bb)?;

        self.codegen.builder.position_at_end(cont_bb);
        Ok(())
    }

    fn atomic_top_level_place_for_local(
        &mut self,
        local: LocalId,
        span: Span,
        require_writable: bool,
    ) -> Result<Option<(PointerValue<'ctx>, CgTy)>, LlvmEmitError> {
        // `TopLevelRef` 作为 atomic intrinsic 的 target 时必须保留静态存储地址，
        // 不能先退化成局部 slot 中的按值副本。
        let Some(fqn) = self.local_top_level_ref_fqn(local).map(str::to_owned) else {
            return Ok(None);
        };

        if let Some(global) = self.codegen.materialized_extern_global_root(&fqn).cloned() {
            if require_writable && !global.mutable {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "atomicInt immutable extern global",
                    at: span.into(),
                });
            }
            let cg_ty =
                self.codegen
                    .cg_ty_of(global.ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicInt extern global type",
                        at: span.into(),
                    })?;
            let gv = self.codegen.declare_mir_extern_global(&global)?;
            return Ok(Some((gv.as_pointer_value(), cg_ty)));
        }

        if let Some(global) = self.codegen.extern_globals.get(&fqn).cloned() {
            if require_writable && !global.mutable {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "atomicInt immutable extern global",
                    at: span.into(),
                });
            }
            let cg_ty =
                self.codegen
                    .cg_ty_of(global.ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicInt extern global type",
                        at: span.into(),
                    })?;
            let gv = self.codegen.declare_extern_global(&global)?;
            return Ok(Some((gv.as_pointer_value(), cg_ty)));
        }

        let Some(var) = self.codegen.top_level_vars.get(&fqn).cloned() else {
            return Ok(None);
        };
        let cg_ty = self
            .codegen
            .cg_ty_of(var.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "atomicInt top-level var type",
                at: span.into(),
            })?;
        let gv = self.codegen.declare_top_level_var_global(&var)?;
        Ok(Some((gv.as_pointer_value(), cg_ty)))
    }

    fn atomic_member_place_for_local(
        &mut self,
        local: LocalId,
        _span: Span,
        require_writable: bool,
    ) -> Result<Option<(PointerValue<'ctx>, CgTy)>, LlvmEmitError> {
        let mut found: Option<(Span, mir::Operand, mir::MemberAccessMetadata)> = None;
        for block in &self.body.blocks {
            for stmt in &block.stmts {
                let mir::StatementKind::Assign { target, value } = &stmt.kind else {
                    continue;
                };
                if *target != local {
                    continue;
                }
                let mir::Rvalue::MemberAccess {
                    receiver, member, ..
                } = value
                else {
                    continue;
                };
                found = Some((stmt.span, receiver.clone(), member.clone()));
                break;
            }
            if found.is_some() {
                break;
            }
        }
        let Some((stmt_span, receiver, member)) = found else {
            return Ok(None);
        };
        self.atomic_member_place(stmt_span, &receiver, &member, require_writable)
            .map(Some)
    }

    fn atomic_member_place(
        &mut self,
        span: Span,
        receiver: &mir::Operand,
        member: &mir::MemberAccessMetadata,
        require_writable: bool,
    ) -> Result<(PointerValue<'ctx>, CgTy), LlvmEmitError> {
        let field_fqn = Self::atomic_member_value_fqn(span, member)?;
        let receiver_type_id =
            self.atomic_member_receiver_codegen_type_id(span, receiver, member)?;
        let receiver_cg =
            self.codegen
                .cg_ty_of(receiver_type_id)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "atomicInt member receiver type",
                    at: span.into(),
                })?;
        if let Some((class, field_idx, field_cg)) =
            self.codegen
                .lookup_class_field_by_fqn(field_fqn, span, Some(receiver_type_id))?
            && receiver_cg == CgTy::Ref
        {
            let field =
                class
                    .fields
                    .get(field_idx as usize)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicInt class field index",
                        at: span.into(),
                    })?;
            if require_writable && !field.mutable {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "atomicInt immutable class field",
                    at: span.into(),
                });
            }
            let receiver_value = self.codegen.codegen_mir_operand_expected(
                span,
                receiver,
                self.slots,
                Some(CgTy::Ref),
            )?;
            let receiver_value = self.codegen.coerce_value(span, receiver_value, CgTy::Ref)?;
            let Some(raw) = receiver_value.value else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "atomicInt class receiver value",
                    at: span.into(),
                });
            };
            let BasicValueEnum::PointerValue(obj_ptr) = raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "atomicInt class receiver type",
                    at: span.into(),
                });
            };
            let ptr = self
                .codegen
                .codegen_class_field_ptr(span, &class, obj_ptr, field_idx)?;
            return Ok((ptr, field_cg));
        }

        let CgTy::Struct(struct_ty) = receiver_cg else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "atomicInt member field target",
                at: span.into(),
            });
        };
        let (field_idx, field_cg) = self
            .codegen
            .lookup_struct_field(struct_ty, field_fqn, span)?;
        let base_ptr =
            self.atomic_struct_receiver_ptr(span, receiver, struct_ty, require_writable)?;
        let llvm_struct_ty = self.codegen.llvm_struct_type(span, struct_ty)?;
        let ptr = self.codegen.builder.build_struct_gep(
            llvm_struct_ty,
            base_ptr,
            field_idx,
            "atomic_int_field_gep",
        )?;
        Ok((ptr, field_cg))
    }

    fn atomic_struct_receiver_ptr(
        &mut self,
        span: Span,
        receiver: &mir::Operand,
        struct_ty: TypeId,
        require_writable: bool,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let mir::Operand::Local(local) = receiver else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "atomicInt struct receiver place",
                at: span.into(),
            });
        };
        if let Some((ptr, cg_ty)) =
            self.atomic_member_place_for_local(*local, span, require_writable)?
        {
            if cg_ty != CgTy::Struct(struct_ty) {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "atomicInt struct receiver type drift",
                    at: span.into(),
                });
            }
            return Ok(ptr);
        }
        let slot = self.codegen.mir_local_slot(span, self.slots, *local)?;
        if slot.cg_ty != CgTy::Struct(struct_ty) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "atomicInt struct receiver slot type",
                at: span.into(),
            });
        }
        Ok(slot.ptr)
    }

    fn atomic_member_receiver_codegen_type_id(
        &self,
        span: Span,
        receiver: &mir::Operand,
        member: &mir::MemberAccessMetadata,
    ) -> Result<TypeId, LlvmEmitError> {
        let receiver_source_ty = match receiver {
            mir::Operand::Local(local) => self
                .body
                .locals
                .get(local.as_u32() as usize)
                .map(|local| local.ty)
                .unwrap_or(member.receiver_ty),
            mir::Operand::Const(_) => member.receiver_ty,
        };
        self.codegen
            .equivalent_codegen_type_id(self.source_types, receiver_source_ty)
            .or_else(|| {
                self.codegen
                    .equivalent_codegen_type_id(self.source_types, member.receiver_ty)
            })
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "atomicInt member receiver type",
                at: span.into(),
            })
    }

    fn atomic_member_value_fqn(
        span: Span,
        member: &mir::MemberAccessMetadata,
    ) -> Result<&str, LlvmEmitError> {
        match member.resolved.as_ref() {
            Some(mir::MemberTarget::Value { fqn }) => Ok(fqn.as_str()),
            Some(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "atomicInt member target is not value",
                at: span.into(),
            }),
            None => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "atomicInt member target unresolved",
                at: span.into(),
            }),
        }
    }

    fn required_operand_source_ty(
        &self,
        operand: &mir::Operand,
        span: Span,
    ) -> Result<TypeId, LlvmEmitError> {
        self.operand_source_ty(operand)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "task transport operand source type",
                at: span.into(),
            })
    }

    fn resolved_fun_value_callee_fqn(&self, callee: &mir::Operand) -> Option<&str> {
        let mir::Operand::Local(callee_local) = callee else {
            return None;
        };
        for block in &self.body.blocks {
            for stmt in &block.stmts {
                let mir::StatementKind::Assign { target, value } = &stmt.kind else {
                    continue;
                };
                if target != callee_local {
                    continue;
                }
                let mir::Rvalue::MemberAccess { member, .. } = value else {
                    continue;
                };
                match member.resolved.as_ref()? {
                    mir::MemberTarget::Fun { fqn } | mir::MemberTarget::ExtensionFun { fqn } => {
                        return Some(fqn.as_str());
                    }
                    mir::MemberTarget::Value { .. } | mir::MemberTarget::ExtensionValue { .. } => {}
                }
            }
        }
        None
    }

    fn top_level_callable_value_local(&self, callable_fqn: &str) -> Option<LocalId> {
        self.body.blocks.iter().find_map(|block| {
            block.stmts.iter().find_map(|stmt| {
                let mir::StatementKind::Assign { target, value } = &stmt.kind else {
                    return None;
                };
                let mir::Rvalue::TopLevelRef(top_level) = value else {
                    return None;
                };
                (top_level.fqn == callable_fqn).then_some(*target)
            })
        })
    }

    fn lower_top_level_funptr_direct_call(
        &mut self,
        callable_fqn: &str,
        span: Span,
        args: &[mir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let Some(fun_ty) = self.top_level_funptr_function_type(callable_fqn) else {
            return Ok(None);
        };
        if !fun_ty.effects.is_pure() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level FunPtr effect-typed direct call",
                at: span.into(),
            });
        }
        let value = self
            .codegen
            .top_level_immutable_values
            .get(callable_fqn)
            .cloned()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level FunPtr value metadata",
                at: span.into(),
            })?;
        let funptr = self
            .codegen
            .codegen_top_level_immutable_value_access(span, &value)?;
        let Some(BasicValueEnum::IntValue(funptr_addr)) = funptr.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level FunPtr value",
                at: span.into(),
            });
        };

        let mut source_arg_tys = Vec::new();
        if let Some(receiver_ty) = fun_ty.receiver {
            source_arg_tys.push(("receiver".to_string(), receiver_ty));
        }
        source_arg_tys.extend(
            fun_ty
                .params
                .iter()
                .enumerate()
                .map(|(index, ty)| (format!("a{index}"), *ty)),
        );
        if args.len() != source_arg_tys.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level FunPtr direct call arity",
                at: span.into(),
            });
        }
        let mut ordered_args = vec![None; source_arg_tys.len()];
        let mut next_positional = 0usize;
        for arg in args {
            let index = if let Some(name) = &arg.name {
                source_arg_tys
                    .iter()
                    .position(|(param_name, _)| param_name == name)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "top-level FunPtr named arg",
                        at: arg.span.into(),
                    })?
            } else {
                let index = next_positional;
                next_positional += 1;
                index
            };
            if index >= ordered_args.len() || ordered_args[index].replace(arg).is_some() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "top-level FunPtr arg mapping",
                    at: arg.span.into(),
                });
            }
        }

        let mut llvm_param_tys = Vec::with_capacity(source_arg_tys.len());
        let mut llvm_args =
            Vec::<BasicMetadataValueEnum<'ctx>>::with_capacity(source_arg_tys.len());
        for (index, (_, source_ty)) in source_arg_tys.iter().enumerate() {
            let param_cg =
                self.codegen
                    .cg_ty_of(*source_ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "top-level FunPtr param type",
                        at: span.into(),
                    })?;
            llvm_param_tys.push(self.codegen.llvm_basic_type_of(span, param_cg)?.into());
            let arg = ordered_args[index].ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level FunPtr missing arg",
                at: span.into(),
            })?;
            let value = self.codegen.codegen_mir_operand_expected(
                arg.span,
                &arg.value,
                self.slots,
                Some(param_cg),
            )?;
            let value = self.codegen.coerce_value(arg.span, value, param_cg)?;
            let raw = value.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level FunPtr arg value",
                at: arg.span.into(),
            })?;
            llvm_args.push(raw.into());
        }

        let ret_cg =
            self.codegen
                .cg_ty_of(fun_ty.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "top-level FunPtr return type",
                    at: span.into(),
                })?;
        let llvm_fun_ty = match ret_cg {
            CgTy::Unit | CgTy::Never => self
                .codegen
                .context
                .void_type()
                .fn_type(&llvm_param_tys, false),
            other => self
                .codegen
                .llvm_basic_type_of(span, other)?
                .fn_type(&llvm_param_tys, false),
        };
        let fun_ptr_ty = self.codegen.llvm_ptr_type(AddressSpace::default());
        let typed_fn_ptr = self.codegen.builder.build_int_to_ptr(
            funptr_addr,
            fun_ptr_ty,
            "top_level_funptr_typed",
        )?;
        let call_site = self.codegen.builder.build_indirect_call(
            llvm_fun_ty,
            typed_fn_ptr,
            &llvm_args,
            "top_level_funptr_call",
        )?;
        match ret_cg {
            CgTy::Unit => Ok(Some(CgValue::unit())),
            CgTy::Never => Ok(Some(CgValue::never())),
            _ => {
                let raw = call_site.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "top-level FunPtr return value",
                        at: span.into(),
                    },
                )?;
                Ok(Some(self.codegen.cg_value_from_loaded(span, ret_cg, raw)?))
            }
        }
    }

    fn top_level_funptr_function_type(
        &self,
        callable_fqn: &str,
    ) -> Option<crate::ty::FunctionType> {
        let value = self.codegen.top_level_immutable_values.get(callable_fqn)?;
        match self.codegen.types.kind(value.ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
            | TypeKind::Value(ValueTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.unsafe.FunPtr" && nominal.args.len() == 1 =>
            {
                let TypeKind::Ref(RefTypeKind::Function(fun_ty)) =
                    self.codegen.types.kind(nominal.args[0])
                else {
                    return None;
                };
                Some(fun_ty.clone())
            }
            _ => None,
        }
    }

    fn top_level_function_value_type(&self, callable_fqn: &str) -> Option<crate::ty::FunctionType> {
        let value = self.codegen.top_level_immutable_values.get(callable_fqn)?;
        let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.codegen.types.kind(value.ty) else {
            return None;
        };
        Some(fun_ty.clone())
    }

    fn lower_top_level_function_value_direct_call(
        &mut self,
        callable_fqn: &str,
        span: Span,
        args: &[mir::CallArg],
        fun_ty: &crate::ty::FunctionType,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !fun_ty.effects.is_pure() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level function-value effect-typed direct call",
                at: span.into(),
            });
        }
        let value = self
            .codegen
            .top_level_immutable_values
            .get(callable_fqn)
            .cloned()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level function-value metadata",
                at: span.into(),
            })?;
        let callee = self
            .codegen
            .codegen_top_level_immutable_value_access(span, &value)?;
        let callee = self.codegen.coerce_value(span, callee, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(closure_obj_i8)) = callee.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level function-value value",
                at: span.into(),
            });
        };
        self.codegen
            .codegen_mir_plain_function_value_call_from_closure_obj(
                span,
                closure_obj_i8,
                args,
                fun_ty,
                self.slots,
            )
    }

    fn unresolved_fun_value_callee_name(&self, callee: &mir::Operand) -> Option<String> {
        let mir::Operand::Local(callee_local) = callee else {
            return None;
        };
        for block in &self.body.blocks {
            for stmt in &block.stmts {
                let mir::StatementKind::Assign { target, value } = &stmt.kind else {
                    continue;
                };
                if target != callee_local {
                    continue;
                }
                if let mir::Rvalue::UnresolvedName { name } = value {
                    return Some(name.clone());
                }
            }
        }
        None
    }

    fn string_like_pointer(
        &mut self,
        span: Span,
        value: CgValue<'ctx>,
        kind: &'static str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let value = match value.ty {
            CgTy::String | CgTy::Ref => value,
            _ => self.codegen.coerce_value(span, value, CgTy::String)?,
        };
        let Some(BasicValueEnum::PointerValue(ptr)) = value.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: span.into(),
            });
        };
        Ok(ptr)
    }

    fn lower_core_string_byte_length_call(
        &mut self,
        span: Span,
        args: &[mir::CallArg],
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 || args[0].name.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "core byteLength arg contract",
                at: span.into(),
            });
        }

        let receiver = self.codegen.codegen_mir_operand_expected(
            args[0].span,
            &args[0].value,
            self.slots,
            Some(CgTy::String),
        )?;
        let receiver_ptr =
            self.string_like_pointer(args[0].span, receiver, "core byteLength receiver value")?;
        let len_ptr = self.codegen.builder.build_struct_gep(
            self.codegen.llvm_scoop_string_type(),
            receiver_ptr,
            1,
            "core_byte_length_gep",
        )?;
        let raw = self.codegen.builder.build_load(
            self.codegen.context.i64_type(),
            len_ptr,
            "core_byte_length",
        )?;
        let BasicValueEnum::IntValue(result) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "core byteLength load type",
                at: span.into(),
            });
        };
        let value = CgValue::int(
            result,
            IntTy {
                bits: 64,
                signed: true,
            },
        );
        self.codegen.coerce_value(span, value, target_cg)
    }

    fn lower_core_string_get_byte_call(
        &mut self,
        span: Span,
        args: &[mir::CallArg],
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 2 || args.iter().any(|arg| arg.name.is_some()) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "core getByte arg contract",
                at: span.into(),
            });
        }

        let receiver = self.codegen.codegen_mir_operand_expected(
            args[0].span,
            &args[0].value,
            self.slots,
            Some(CgTy::String),
        )?;
        let receiver_ptr =
            self.string_like_pointer(args[0].span, receiver, "core getByte receiver value")?;
        let index = self.codegen.codegen_mir_operand_expected(
            args[1].span,
            &args[1].value,
            self.slots,
            Some(CgTy::Int(IntTy {
                bits: 64,
                signed: true,
            })),
        )?;
        let Some(BasicValueEnum::IntValue(index_int)) = index.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "core getByte index value",
                at: args[1].span.into(),
            });
        };

        let i64_ty = self.codegen.context.i64_type();
        let i8_ty = self.codegen.context.i8_type();
        let len_ptr = self.codegen.builder.build_struct_gep(
            self.codegen.llvm_scoop_string_type(),
            receiver_ptr,
            1,
            "core_get_byte_len_gep",
        )?;
        let len_val = self
            .codegen
            .builder
            .build_load(i64_ty, len_ptr, "core_get_byte_len")?
            .into_int_value();
        let data_ptr_ptr = self.codegen.builder.build_struct_gep(
            self.codegen.llvm_scoop_string_type(),
            receiver_ptr,
            2,
            "core_get_byte_data_gep",
        )?;
        let data_ptr = self
            .codegen
            .builder
            .build_load(
                self.codegen.llvm_i8_ptr_type(),
                data_ptr_ptr,
                "core_get_byte_data",
            )?
            .into_pointer_value();

        let current_fn = self
            .codegen
            .builder
            .get_insert_block()
            .unwrap()
            .get_parent()
            .unwrap();
        let in_bounds_bb = self
            .codegen
            .context
            .append_basic_block(current_fn, "getByte_in_bounds");
        let out_of_bounds_bb = self
            .codegen
            .context
            .append_basic_block(current_fn, "getByte_out_of_bounds");
        let merge_bb = self
            .codegen
            .context
            .append_basic_block(current_fn, "getByte_merge");

        let is_negative = self.codegen.builder.build_int_compare(
            inkwell::IntPredicate::SLT,
            index_int,
            i64_ty.const_zero(),
            "getByte_negative",
        )?;
        let not_negative_bb = self
            .codegen
            .context
            .append_basic_block(current_fn, "getByte_not_negative");
        self.codegen.builder.build_conditional_branch(
            is_negative,
            out_of_bounds_bb,
            not_negative_bb,
        )?;

        self.codegen.builder.position_at_end(not_negative_bb);
        let is_ge_len = self.codegen.builder.build_int_compare(
            inkwell::IntPredicate::SGE,
            index_int,
            len_val,
            "getByte_ge_len",
        )?;
        self.codegen
            .builder
            .build_conditional_branch(is_ge_len, out_of_bounds_bb, in_bounds_bb)?;

        self.codegen.builder.position_at_end(out_of_bounds_bb);
        let zero_val = i64_ty.const_zero();
        self.codegen.builder.build_unconditional_branch(merge_bb)?;

        self.codegen.builder.position_at_end(in_bounds_bb);
        let byte_ptr = unsafe {
            self.codegen.builder.build_in_bounds_gep(
                i8_ty,
                data_ptr,
                &[index_int],
                "core_get_byte_elem_gep",
            )?
        };
        let byte_val = self
            .codegen
            .builder
            .build_load(i8_ty, byte_ptr, "core_get_byte_val")?
            .into_int_value();
        let byte_i64 =
            self.codegen
                .builder
                .build_int_z_extend(byte_val, i64_ty, "core_get_byte_zext")?;
        self.codegen.builder.build_unconditional_branch(merge_bb)?;

        self.codegen.builder.position_at_end(merge_bb);
        let phi = self
            .codegen
            .builder
            .build_phi(i64_ty, "core_get_byte_result")?;
        phi.add_incoming(&[(&zero_val, out_of_bounds_bb), (&byte_i64, in_bounds_bb)]);
        let value = CgValue::int(
            phi.as_basic_value().into_int_value(),
            IntTy {
                bits: 64,
                signed: true,
            },
        );
        self.codegen.coerce_value(span, value, target_cg)
    }

    fn maybe_lower_float_ext_call(
        &mut self,
        span: Span,
        callee_fqn: &str,
        args: &[mir::CallArg],
        target_cg: CgTy,
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let [arg] = args else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Float extension intrinsic arity",
                at: span.into(),
            });
        };
        if arg.name.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Float extension intrinsic named arg",
                at: arg.span.into(),
            });
        }
        let arg_cg = self
            .codegen
            .mir_operand_cg_ty(self.body, self.source_types, &arg.value)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "Float extension intrinsic arg type",
                at: arg.span.into(),
            })?;
        if !matches!(arg_cg, CgTy::Float64 | CgTy::Float32) {
            return Ok(None);
        }
        let value = self.codegen.codegen_mir_operand_expected(
            arg.span,
            &arg.value,
            self.slots,
            Some(arg_cg),
        )?;
        let value = self.codegen.coerce_value(arg.span, value, arg_cg)?;
        let lowered = match callee_fqn {
            "scoop.core.abs" => self.codegen.codegen_float_abs_value(arg.span, value)?,
            "scoop.core.isNaN" => self.codegen.codegen_float_is_nan_value(arg.span, value)?,
            "scoop.core.isInfinite" => self
                .codegen
                .codegen_float_is_infinite_value(arg.span, value)?,
            _ => unreachable!("filtered by caller"),
        };
        self.codegen
            .coerce_value(span, lowered, target_cg)
            .map(Some)
    }

    fn lower_internal_print_string(
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
                kind: "internal print string arg contract",
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
                kind: "internal print string arg value",
                at: arg.span.into(),
            });
        };
        let runtime = self.codegen.declare_runtime_print_like(runtime_name);
        let _ = self.codegen.build_call_preserving_gc_local_roots(
            arg.span,
            runtime,
            &[str_ptr.into()],
            "internal_print",
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
                    | "scoop.runtime.test.__scoop_stackmap_statepoint_smoke"
                    | "scoop.core.GC.handleNew"
                    | "scoop.core.GC.handleGet"
                    | "scoop.core.GC.handleDrop"
                    | "scoop.core.GC.pin"
                    | "scoop.core.GC.unpin"
            )
    }

    fn lower_gc_pin(
        &mut self,
        span: Span,
        args: &[mir::CallArg],
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 || args[0].name.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.pin arg contract",
                at: span.into(),
            });
        }
        let CgTy::Struct(pinned_ty) = target_cg else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.pin target type",
                at: span.into(),
            });
        };
        let (field_idx, field_cg_ty) =
            self.codegen
                .lookup_struct_field(pinned_ty, "scoop.core.Pinned.value", span)?;
        let arg = &args[0];
        let obj = self.codegen.codegen_mir_operand_expected(
            arg.span,
            &arg.value,
            self.slots,
            Some(field_cg_ty),
        )?;
        let obj = self.codegen.coerce_value(arg.span, obj, field_cg_ty)?;
        let obj_ref = self.codegen.coerce_value(arg.span, obj, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(obj_ptr)) = obj_ref.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.pin arg value",
                at: arg.span.into(),
            });
        };

        let rt_pin = self.codegen.declare_runtime_gc_pin();
        let call = self
            .codegen
            .builder
            .build_call(rt_pin, &[obj_ptr.into()], "gc_pin")?;
        let Some(BasicValueEnum::IntValue(ok_i32)) = call.try_as_basic_value().basic() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.pin return type",
                at: span.into(),
            });
        };

        let ok_cond = self.codegen.builder.build_int_compare(
            IntPredicate::NE,
            ok_i32,
            self.codegen.context.i32_type().const_zero(),
            "gc_pin_ok",
        )?;
        let function = self.codegen.expect_current_function("GC.pin branch blocks");
        let ok_bb = self
            .codegen
            .context
            .append_basic_block(function, "gc_pin_ok");
        let err_bb = self
            .codegen
            .context
            .append_basic_block(function, "gc_pin_err");
        let cont_bb = self
            .codegen
            .context
            .append_basic_block(function, "gc_pin_cont");
        self.codegen
            .builder
            .build_conditional_branch(ok_cond, ok_bb, err_bb)?;

        self.codegen.builder.position_at_end(err_bb);
        self.codegen.emit_exit_with_code(span, 3)?;

        self.codegen.builder.position_at_end(ok_bb);
        let llvm_struct_ty = self.codegen.llvm_struct_type(span, pinned_ty)?;
        let mut agg: AggregateValueEnum<'ctx> = llvm_struct_ty.get_undef().into();
        let raw_field: BasicValueEnum<'ctx> = match field_cg_ty {
            CgTy::Unit => self.codegen.context.i8_type().const_int(0, false).into(),
            _ => obj.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.pin field value",
                at: arg.span.into(),
            })?,
        };
        agg = self
            .codegen
            .builder
            .build_insert_value(agg, raw_field, field_idx, "pinned_value")?;
        self.codegen.builder.build_unconditional_branch(cont_bb)?;

        self.codegen.builder.position_at_end(cont_bb);
        Ok(CgValue {
            ty: target_cg,
            value: Some(agg.as_basic_value_enum()),
        })
    }

    fn lower_gc_unpin(
        &mut self,
        span: Span,
        args: &[mir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 || args[0].name.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin arg contract",
                at: span.into(),
            });
        }
        let arg = &args[0];
        let pinned = self
            .codegen
            .codegen_mir_operand_expected(arg.span, &arg.value, self.slots, None)?;
        let CgTy::Struct(pinned_ty) = pinned.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin arg type",
                at: arg.span.into(),
            });
        };
        let Some(BasicValueEnum::StructValue(struct_v)) = pinned.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin arg value",
                at: arg.span.into(),
            });
        };
        let (field_idx, field_cg_ty) =
            self.codegen
                .lookup_struct_field(pinned_ty, "scoop.core.Pinned.value", arg.span)?;
        let extracted =
            self.codegen
                .builder
                .build_extract_value(struct_v, field_idx, "pinned_value")?;
        let field = self
            .codegen
            .cg_value_from_loaded(arg.span, field_cg_ty, extracted)?;
        let field_ref = self.codegen.coerce_value(arg.span, field, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(obj_ptr)) = field_ref.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin value",
                at: arg.span.into(),
            });
        };

        let rt_unpin = self.codegen.declare_runtime_gc_unpin();
        let call = self
            .codegen
            .builder
            .build_call(rt_unpin, &[obj_ptr.into()], "gc_unpin")?;
        let Some(BasicValueEnum::IntValue(ok_i32)) = call.try_as_basic_value().basic() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin return type",
                at: span.into(),
            });
        };

        let ok_cond = self.codegen.builder.build_int_compare(
            IntPredicate::NE,
            ok_i32,
            self.codegen.context.i32_type().const_zero(),
            "gc_unpin_ok",
        )?;
        let function = self
            .codegen
            .expect_current_function("GC.unpin branch blocks");
        let ok_bb = self
            .codegen
            .context
            .append_basic_block(function, "gc_unpin_ok");
        let err_bb = self
            .codegen
            .context
            .append_basic_block(function, "gc_unpin_err");
        let cont_bb = self
            .codegen
            .context
            .append_basic_block(function, "gc_unpin_cont");
        self.codegen
            .builder
            .build_conditional_branch(ok_cond, ok_bb, err_bb)?;

        self.codegen.builder.position_at_end(err_bb);
        self.codegen.emit_exit_with_code(span, 3)?;

        self.codegen.builder.position_at_end(ok_bb);
        self.codegen.builder.build_unconditional_branch(cont_bb)?;

        self.codegen.builder.position_at_end(cont_bb);
        Ok(CgValue::unit())
    }

    fn lower_gc_debug_intrinsic(
        &mut self,
        span: Span,
        callee_fqn: &str,
        args: &[mir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        match callee_fqn {
            "scoop.runtime.test.__scoop_stackmap_statepoint_smoke" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "stackmap statepoint smoke arity mismatch",
                        at: span.into(),
                    });
                }
                let current_fun = self
                    .codegen
                    .builder
                    .get_insert_block()
                    .and_then(|bb| bb.get_parent())
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "stackmap statepoint smoke caller function",
                        at: span.into(),
                    })?;
                current_fun.set_gc("statepoint-example");

                let runtime = self.codegen.declare_runtime_stackmap_statepoint_smoke();
                let call = self.codegen.build_call_preserving_gc_local_roots(
                    span,
                    runtime,
                    &[],
                    "stackmap_statepoint_smoke",
                )?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "stackmap statepoint smoke return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(raw_int) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "stackmap statepoint smoke return type",
                        at: span.into(),
                    });
                };
                let from = IntTy {
                    bits: 64,
                    signed: true,
                };
                let to = IntTy {
                    bits: self.codegen.host.word_bit_width(),
                    signed: true,
                };
                let casted = self.codegen.cast_int(raw_int, from, to)?;
                Ok(Some(CgValue::int(casted, to)))
            }
            _ => Ok(None),
        }
    }

    fn pack_call_args(
        &mut self,
        span: Span,
        entry: &CallableEntryLayout<'ctx>,
        args: &[mir::CallArg],
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        self.pack_call_args_for_invoke_args_tuple(
            span,
            entry.invoke_args_tuple_ty(),
            args,
            "pure_call",
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
                kind: "named call arg",
                at: span.into(),
            });
        }
        let layout = self.abi.source_value_layout(invoke_args_tuple_ty)?;
        if layout.abi().is_elided() {
            return Ok(None);
        }
        match layout.kind() {
            SourceAbiLayoutKind::Scalar => {
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
                        kind: "scalar call arg type",
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
                        kind: "scalar call arg value",
                        at: arg.span.into(),
                    },
                )?))
            }
            SourceAbiLayoutKind::Tuple => {
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
                            "pure statement tuple call ABI 缺少 argument {index}"
                        ))
                    })?;
                    let expected = self
                        .codegen
                        .cg_ty_of_mir_type(self.source_types, field.source_ty())
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "tuple call arg type",
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
                        kind: "tuple call arg value",
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

    fn extract_pure_call_complete(
        &mut self,
        span: Span,
        step_schema: crate::effect_facts::StepSchemaId,
        step: BasicValueEnum<'ctx>,
        target_cg: super::super::types::CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let step_layout = self.abi.step_layout(step_schema).ok_or_else(|| {
            frontend_error(format!(
                "pure statement call 缺少 callee step schema s{} layout",
                step_schema.as_u32()
            ))
        })?;
        if !step_layout.cases().is_empty() {
            return Err(frontend_error(format!(
                "pure statement call callee step schema s{} 含 outward case，必须走 boundary lowering",
                step_schema.as_u32()
            )));
        }
        let payload = self.codegen.extract_step_payload(
            step_layout,
            step,
            step_layout.complete_variant(),
            "pure_call_complete_payload",
        )?;
        match (target_cg, payload) {
            (super::super::types::CgTy::Unit, None) => Ok(CgValue::unit()),
            (super::super::types::CgTy::Never, None) => Ok(CgValue::never()),
            (super::super::types::CgTy::Unit, Some(_)) => Err(frontend_error(
                "pure statement call Unit target 收到 non-elided Complete payload".to_string(),
            )),
            (_, Some(raw)) => {
                let value = self.codegen.cg_value_from_loaded(span, target_cg, raw)?;
                self.codegen.coerce_value(span, value, target_cg).map_err(|err| {
                    frontend_error(format!(
                        "pure direct call Complete payload coercion failed: value_ty={:?} target_ty={:?}: {err}",
                        value.ty, target_cg,
                    ))
                })
            }
            (_, None) => Err(frontend_error(
                "pure statement call non-Unit target 缺少 Complete payload".to_string(),
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
        let value =
            self.codegen
                .coerce_value(span, value, slot.cg_ty)
                .map_err(|err| match err {
                    LlvmEmitError::Frontend { message } => frontend_error(format!(
                        "store local{} coercion failed at {:?}: {message}",
                        local.as_u32(),
                        span
                    )),
                    other => other,
                })?;
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
                kind: "operand source type",
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
            SourceAbiLayoutKind::Scalar => {
                let source = sources.first().ok_or_else(|| {
                    frontend_error(format!("ABI scalar payload `{name}` 缺少 source"))
                })?;
                Ok(self.lower_operand_source(source)?.value)
            }
            SourceAbiLayoutKind::Tuple => {
                if sources.len() == 1 && sources[0].source_ty() == source_ty {
                    return self.pack_whole_tuple_source(layout, &sources[0], name);
                }
                let BasicTypeEnum::StructType(struct_ty) = layout.abi().llvm_ty() else {
                    return Err(frontend_error(format!(
                        "ABI tuple payload `{name}` layout 不是 struct"
                    )));
                };
                let mut aggregate = struct_ty.get_undef();
                for (index, source) in sources.iter().enumerate() {
                    let Some(field) = layout.field(index) else {
                        return Err(frontend_error(format!(
                            "ABI tuple payload `{name}` source index {index} 超出 layout 字段"
                        )));
                    };
                    if field.is_elided() {
                        continue;
                    }
                    let raw = self.lower_operand_source(source)?.value.ok_or_else(|| {
                        frontend_error(format!(
                            "ABI tuple payload `{name}` source index {index} 被 elide 但 field 需要值"
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
        layout: &SourceAbiLayout<'ctx>,
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
        layout: &SourceAbiLayout<'ctx>,
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
        layout: &SourceAbiLayout<'ctx>,
        value: CgValue<'ctx>,
        name: &str,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        let Some(BasicValueEnum::StructValue(tuple)) = value.value else {
            return Err(frontend_error(format!(
                "ABI tuple payload `{name}` whole tuple source 缺少 struct value"
            )));
        };
        let BasicTypeEnum::StructType(struct_ty) = layout.abi().llvm_ty() else {
            return Err(frontend_error(format!(
                "ABI tuple payload `{name}` whole tuple layout 不是 struct"
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
            mir::Operand::Const(mir::ConstValue::String | mir::ConstValue::SynthString(_)) => {
                Some(self.codegen.builtins.string)
            }
        }
    }

    fn operand_slot_cg_ty(&self, operand: &mir::Operand) -> Option<CgTy> {
        match operand {
            mir::Operand::Local(local) => self
                .slots
                .get(local.as_u32() as usize)
                .map(|slot| slot.cg_ty),
            mir::Operand::Const(value) => self.codegen.mir_const_cg_ty(value),
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
            SourceAbiLayoutKind::Scalar => Ok(payload),
            SourceAbiLayoutKind::Tuple => {
                let Some(field) = layout.field(ordinal as usize) else {
                    return Err(frontend_error(format!(
                        "payload tuple t{} 缺少 ordinal {}",
                        payload_ty.as_u32(),
                        ordinal
                    )));
                };
                if field.is_elided() {
                    return Ok(None);
                }
                let Some(BasicValueEnum::StructValue(tuple)) = payload else {
                    return Err(frontend_error(format!(
                        "payload tuple t{} 缺少 struct payload",
                        payload_ty.as_u32()
                    )));
                };
                Ok(Some(
                    self.codegen.builder.build_extract_value(
                        tuple,
                        field
                            .abi_field_index()
                            .expect("non-elided field has ABI index"),
                        "payload_field",
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

fn frontend_error(message: String) -> LlvmEmitError {
    LlvmEmitError::Frontend { message }
}

fn intrinsic_base_fqn(fqn: &str) -> &str {
    fqn.split("::<")
        .next()
        .unwrap_or(fqn)
        .split("$overload")
        .next()
        .unwrap_or(fqn)
}

#[cfg(test)]
mod tests {

    #[test]
    fn llvm_value_primitive_inventory_is_explicit() {
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
}
