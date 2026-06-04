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
    AggregateValueEnum, BasicMetadataValueEnum, BasicValue, BasicValueEnum, FunctionValue,
    PointerValue,
};
use inkwell::{AddressSpace, AtomicOrdering, IntPredicate};
use scoopc_lir_facts::LirGlobalRootKind;

use crate::effect_lowered::LirCallArg;
use crate::effect_lowered::ir::{
    LateLoweredOperandSource, LateLoweredOperandValueSource, LateLoweredPlainCallSite,
    LateLoweredProgram, LateLoweredSourceBody,
};
use crate::effect_lowered::mir_source::{self as mir, LocalId};
use crate::llvm::LlvmEmitError;
use crate::span::Span;
use crate::stable_id::canonical_record;
use crate::ty::{MonoTypeId, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::super::mir_body::MirLocalSlot;
use super::super::types::{CgTy, CgValue, IntTy};
use super::super::{CallableCarrierKind, MainCodegen};
use super::stable_naming;
use super::types::{
    CallableEntryLayout, CallableLayout, ProgramAbiQuery, SourceAbiLayout, SourceAbiLayoutKind,
    StepLayout,
};

/// A borrow-scoped facade over effect-neutral LLVM value primitives.
pub(super) struct ValuePrimitives<'p, 'a, 'ctx> {
    codegen: &'p mut MainCodegen<'a, 'ctx>,
    program: &'a LateLoweredProgram,
    plain_call_sites: Option<&'a [LateLoweredPlainCallSite]>,
    source_types: &'a TypeStore,
    body: &'a LateLoweredSourceBody,
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

fn source_carrier_types(types: &TypeStore, carrier_ty: TypeId) -> Option<Vec<TypeId>> {
    match types.kind(carrier_ty) {
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => Some(elements.clone()),
        TypeKind::Value(ValueTypeKind::Unit) => Some(Vec::new()),
        _ => Some(vec![carrier_ty]),
    }
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_lir_pure_effect_step_direct_call(
        &mut self,
        span: Span,
        abi: &ProgramAbiQuery<'ctx>,
        callee_fqn: &str,
        args: &[LirCallArg],
        body: &crate::effect_lowered::LirExecutableBody,
        source_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let layout = abi.callable_layout_by_root_fqn(callee_fqn).map_err(|err| {
            frontend_error(format!(
                "LIR pure statement call 缺少 callee `{callee_fqn}` 的 published LIR callable contract: {err:?}"
            ))
        })?;
        let entry = layout.direct_entry();
        if entry.return_step_schema() != layout.step_schema() {
            return Err(frontend_error(format!(
                "LIR pure statement call `{callee_fqn}` direct entry return schema 漂移：entry=s{} layout=s{}",
                entry.return_step_schema().as_u32(),
                layout.step_schema().as_u32()
            )));
        }
        let payload = self.pack_lir_call_args_for_invoke_args_tuple(
            span,
            abi,
            entry.invoke_args_tuple_ty(),
            args,
            body,
            source_types,
            slots,
            "lir_pure_call",
        )?;
        let callee = self
            .module
            .get_function(entry.symbol_name())
            .ok_or_else(|| {
                frontend_error(format!(
                    "LIR pure statement call `{callee_fqn}` 缺少 direct entry shell `{}`",
                    entry.symbol_name()
                ))
            })?;
        let mut call_args = Vec::<BasicMetadataValueEnum<'ctx>>::new();
        if !entry.args_abi().is_elided() {
            call_args.push(
                payload
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "LIR pure statement call `{callee_fqn}` 需要 non-elided args payload"
                        ))
                    })?
                    .into(),
            );
        }
        let call = self
            .builder
            .build_call(callee, &call_args, "lir_pure_call_step")?;
        let step = call.try_as_basic_value().basic().ok_or_else(|| {
            frontend_error(format!(
                "LIR pure statement call `{callee_fqn}` direct entry 未返回 Step_F"
            ))
        })?;
        self.extract_lir_pure_call_complete(span, abi, layout, step, target_cg)
    }

    #[allow(clippy::too_many_arguments)]
    fn pack_lir_call_args_for_invoke_args_tuple(
        &mut self,
        span: Span,
        abi: &ProgramAbiQuery<'ctx>,
        invoke_args_tuple_ty: TypeId,
        args: &[LirCallArg],
        body: &crate::effect_lowered::LirExecutableBody,
        source_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        name: &str,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        if args.iter().any(|arg| arg.name.is_some()) {
            panic!(
                "pack_lir_call_args_for_invoke_args_tuple: LIR call ABI verifier accepted named argument before canonicalization at {span:?}"
            );
        }
        let layout = abi.source_value_layout(invoke_args_tuple_ty)?;
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
                    .cg_ty_of_mir_type(source_types, layout.source_ty())
                    .unwrap_or_else(|| {
                        panic!(
                            "pack_lir_call_args_for_invoke_args_tuple: scalar call ABI accepted non-codegen arg type at {:?}",
                            arg.span
                        )
                    });
                let value =
                    self.codegen_lir_operand_expected(arg.span, &arg.value, slots, Some(expected))?;
                let value = self.coerce_value(arg.span, value, expected)?;
                Ok(Some(
                    self.expect_cg_value(value, "scalar LIR call arg value"),
                ))
            }
            SourceAbiLayoutKind::Tuple => {
                if args.len() == 1
                    && self.lir_operand_type_id(body, &args[0].value) == Some(layout.source_ty())
                {
                    let expected = self
                        .cg_ty_of_mir_type(source_types, layout.source_ty())
                        .unwrap_or_else(|| {
                            panic!(
                                "pack_lir_call_args_for_invoke_args_tuple: whole tuple arg has no codegen type"
                            )
                        });
                    let value = self.codegen_lir_operand_expected(
                        args[0].span,
                        &args[0].value,
                        slots,
                        Some(expected),
                    )?;
                    let value = self.coerce_value(args[0].span, value, expected)?;
                    return Ok(Some(
                        self.expect_cg_value(value, "whole tuple LIR call arg"),
                    ));
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
                            "LIR pure statement tuple call ABI 缺少 argument {index}"
                        ))
                    })?;
                    let expected = self
                        .cg_ty_of_mir_type(source_types, field.source_ty())
                        .unwrap_or_else(|| {
                            panic!(
                                "pack_lir_call_args_for_invoke_args_tuple: tuple call ABI accepted non-codegen arg type at {:?}",
                                arg.span
                            )
                        });
                    let value = self.codegen_lir_operand_expected(
                        arg.span,
                        &arg.value,
                        slots,
                        Some(expected),
                    )?;
                    let value = self.coerce_value(arg.span, value, expected)?;
                    let raw = self.expect_cg_value(value, "tuple LIR call arg value");
                    aggregate = self
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

    fn extract_lir_pure_call_complete(
        &mut self,
        span: Span,
        abi: &ProgramAbiQuery<'ctx>,
        callable_layout: &CallableLayout<'ctx>,
        step: BasicValueEnum<'ctx>,
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let step_schema = callable_layout.step_schema();
        let step_layout = abi
            .step_layout_for_callable(callable_layout)
            .ok_or_else(|| {
                frontend_error(format!(
                    "LIR pure statement call 缺少 callee step schema s{} layout",
                    step_schema.as_u32()
                ))
            })?;
        if !step_layout.cases().is_empty() {
            return Err(frontend_error(format!(
                "LIR pure statement call callee step schema s{} 含 outward case，必须走 boundary lowering",
                step_schema.as_u32()
            )));
        }
        let payload = self.extract_step_payload(
            step_layout,
            step,
            step_layout.complete_variant(),
            "lir_pure_call_complete_payload",
        )?;
        match (target_cg, payload) {
            (CgTy::Unit, None) => Ok(CgValue::unit()),
            (CgTy::Never, None) => Ok(CgValue::never()),
            (CgTy::Unit, Some(_)) => Err(frontend_error(
                "LIR pure statement call Unit target 收到 non-elided Complete payload".to_string(),
            )),
            (_, Some(raw)) => {
                let value = self.cg_value_from_loaded(span, target_cg, raw)?;
                self.coerce_value(span, value, target_cg).map_err(|err| {
                    frontend_error(format!(
                        "LIR pure direct call Complete payload coercion failed: value_ty={:?} target_ty={:?}: {err}",
                        value.ty, target_cg,
                    ))
                })
            }
            (_, None) => Err(frontend_error(
                "LIR pure statement call non-Unit target 缺少 Complete payload".to_string(),
            )),
        }
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
        program: &'a LateLoweredProgram,
        plain_call_sites: Option<&'a [LateLoweredPlainCallSite]>,
        source_types: &'a TypeStore,
        body: &'a LateLoweredSourceBody,
        slots: &'p [MirLocalSlot<'ctx>],
        abi: &'p ProgramAbiQuery<'ctx>,
    ) -> Self {
        Self {
            codegen,
            program,
            plain_call_sites,
            source_types,
            body,
            slots,
            abi,
        }
    }

    fn known_plain_call_target_fqn(&self, site_id: mir::SiteId) -> Option<&'a str> {
        let facts = self
            .plain_call_sites?
            .iter()
            .find(|site| site.site_id() == site_id)?
            .facts();
        let crate::effect_facts::CallSiteTarget::KnownInstance(instance) = facts.target() else {
            return None;
        };
        self.program
            .callables()
            .iter()
            .find(|callable| callable.instance_key() == instance)
            .map(|callable| callable.root_fqn())
    }

    fn plain_call_param_names(&self, callee_fqn: &str, param_count: usize) -> Vec<String> {
        self.program
            .callable(callee_fqn)
            .and_then(|callable| callable.source_callable())
            .map(|source| {
                source
                    .params
                    .iter()
                    .map(|param| param.name.clone())
                    .collect::<Vec<_>>()
            })
            .filter(|names| names.len() == param_count)
            .unwrap_or_else(|| (0..param_count).map(|idx| format!("arg{idx}")).collect())
    }

    fn lower_published_plain_direct_call(
        &mut self,
        span: Span,
        callee_fqn: &str,
        args: &[mir::CallArg],
        target_cg: super::super::types::CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let layout = self.abi.plain_callable_layout_by_root_fqn(callee_fqn)?;
        let entry = layout.direct_entry();
        let mut param_tys = entry.param_tys().to_vec();
        if args.iter().any(|arg| arg.name.is_some())
            && self
                .program
                .callable(callee_fqn)
                .and_then(|callable| callable.source_callable())
                .is_none()
        {
            return Err(frontend_error(format!(
                "plain direct call `{callee_fqn}` 使用 named args，但缺少 LIR-owned source callable 参数名 contract"
            )));
        }
        let mut param_names = self.plain_call_param_names(callee_fqn, param_tys.len());
        if callee_fqn.contains(".getValue")
            && args.len() == param_tys.len()
            && param_tys.len() >= 3
            && param_names.first().is_some_and(|name| name != "this")
            && let Some(last_ty) = param_tys.pop()
        {
            param_tys.insert(0, last_ty);
            if let Some(last_name) = param_names.pop() {
                param_names.insert(0, last_name);
            }
        }
        let ret_cg = self
            .codegen
            .cg_ty_of_mir_type(self.source_types, entry.return_ty())
            .or_else(|| {
                self.codegen
                    .equivalent_codegen_type_id(self.source_types, entry.return_ty())
                    .and_then(|ty| self.codegen.try_cg_ty_of_type_id(ty))
            })
            .unwrap_or_else(|| {
                panic!(
                    "lower_published_plain_direct_call: LIR plain callable verifier accepted unsupported return type"
                )
            });
        let hidden_sret_result_ty = self.codegen.hidden_sret_result_ty(span, ret_cg)?;
        let evaluated_args = self.codegen.codegen_bound_mir_call_args_from_signature(
            span,
            &param_names,
            &param_tys,
            args,
            self.slots,
            false,
            self.source_types,
        )?;
        let mut llvm_args = Vec::<BasicMetadataValueEnum<'ctx>>::with_capacity(
            evaluated_args.len() + usize::from(hidden_sret_result_ty.is_some()),
        );
        let sret_result_slot = if hidden_sret_result_ty.is_some() {
            let slot =
                self.codegen
                    .create_entry_alloca(span, "lir_plain_direct_call_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        llvm_args.extend(evaluated_args.iter().map(|arg| arg.value));

        let llvm_fun = self
            .codegen
            .module
            .get_function(entry.symbol_name())
            .ok_or_else(|| {
                frontend_error(format!(
                    "plain direct call `{callee_fqn}` 缺少 published plain entry `{}`",
                    entry.symbol_name()
                ))
            })?;
        let call_site_result = self
            .codegen
            .with_conservative_gc_local_root_spills(span, |cg| {
                let call_site =
                    cg.builder
                        .build_call(llvm_fun, &llvm_args, "lir_plain_direct_call")?;
                if let Some(result_ty) = hidden_sret_result_ty {
                    cg.add_sret_attribute_to_call(call_site, 0, result_ty);
                }
                call_site.set_call_convention(cg.llvm_call_convention_for_fqn(callee_fqn));
                Ok(call_site)
            });
        self.codegen
            .release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        if let Some(result_ptr) = sret_result_slot {
            self.codegen.sync_hidden_sret_result_roots(
                span,
                ret_cg,
                result_ptr,
                "lir_plain_direct_call_sret",
            )?;
        }
        let value = match ret_cg {
            super::super::types::CgTy::Unit => CgValue::unit(),
            super::super::types::CgTy::Never => CgValue::never(),
            _ => {
                if let Some(result_ptr) = sret_result_slot {
                    self.codegen.load_hidden_sret_result_from_ptr(
                        span,
                        ret_cg,
                        result_ptr,
                        "lir_plain_direct_call_sret",
                    )?
                } else {
                    let deferred = self.codegen.defer_direct_call_result(
                        span,
                        ret_cg,
                        call_site,
                        "lir_plain_direct_call_result",
                    )?;
                    self.codegen.materialize_deferred_cg_value(
                        span,
                        "lir_plain_direct_call_result_reload",
                        deferred.unwrap_or_else(|| {
                            panic!(
                                "lower_published_plain_direct_call: LIR plain ABI verifier accepted missing deferred return value"
                            )
                        }),
                    )?
                }
            }
        };
        self.codegen.coerce_value(span, value, target_cg)
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
                    && (!self
                        .codegen
                        .lir_global_root_has_kind(fqn, LirGlobalRootKind::TopLevelImmutableVal)
                        || !used_locals.contains(target))
                {
                    return Ok(());
                }
                if let mir::Rvalue::TopLevelRef(mir::TopLevelRef { fqn, .. }) = rvalue
                    && (self
                        .codegen
                        .lir_global_root_has_kind(fqn, LirGlobalRootKind::TopLevelMutableVar)
                        || self.codegen.has_extern_global_contract(fqn))
                    && self.local_is_only_atomic_target(*target)
                {
                    return Ok(());
                }
                if let mir::Rvalue::TopLevelRef(mir::TopLevelRef { fqn, .. }) = rvalue
                    && !self
                        .codegen
                        .lir_global_root_has_kind(fqn, LirGlobalRootKind::ObjectSingleton)
                    && !self
                        .codegen
                        .lir_global_root_has_kind(fqn, LirGlobalRootKind::TopLevelImmutableVal)
                    && !self
                        .codegen
                        .lir_global_root_has_kind(fqn, LirGlobalRootKind::TopLevelMutableVar)
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
                if matches!(rvalue, mir::Rvalue::Todo(reason) if reason == "missing expr")
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
                            "pure assignment local{} store failed for rvalue {:?}: value_ty={:?} target_ty={:?}: {err}",
                            target.as_u32(),
                            rvalue,
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
            panic!(
                "lower_effect_neutral_rvalue: materialized MIR verifier accepted unresolved enum variant call without payload schema at {span:?}"
            );
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
                .unwrap_or_else(|| {
                    panic!(
                        "lower_effect_neutral_rvalue: closure adapter verifier accepted non-codegen closure env type at {span:?}"
                    )
                });
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
                site_id,
                kind,
                args,
                transport,
                ..
            } => self.lower_pure_direct_call(
                span,
                *site_id,
                kind,
                args,
                transport,
                target_cg,
                target_local,
            ),
            mir::Rvalue::MakeClosure {
                env,
                fn_ptr,
                env_contract,
            } => {
                let env_cg = self
                    .codegen
                    .mir_operand_cg_ty(self.body, self.source_types, env)
                    .unwrap_or_else(|| {
                        panic!(
                            "lower_effect_neutral_rvalue: closure verifier accepted non-codegen carrier env type at {span:?}"
                        )
                    });
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
                site_id,
                class_fqn,
                ctor,
                args,
                ..
            } => {
                let class_layout_key =
                    self.class_ctor_layout_key(span, *site_id, class_fqn, target_local)?;
                self.codegen.codegen_mir_class_ctor_call(
                    span,
                    *site_id,
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
                    site_id,
                    kind: mir::CallKind::Direct { .. },
                    args,
                    ..
                } = value
                    && self
                        .codegen
                        .published_lir_source_call_site(*site_id)
                        .and_then(|site| {
                            site.semantic_root_fqn.as_deref().or_else(|| {
                                site.contract
                                    .exact_callee
                                    .as_ref()
                                    .map(|exact| exact.root_fqn.as_str())
                            })
                        })
                        .map(|root| {
                            root.starts_with("scoop.unsafe.__atomicInt")
                                || root.starts_with("scoop.unsafe.__atomicRef")
                        })
                        .unwrap_or(false)
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
        self.codegen
            .lir_global_root_has_kind(fqn, LirGlobalRootKind::ObjectSingleton)
            || self.codegen.lookup_object_property_by_fqn(fqn).is_some()
            || self
                .codegen
                .lir_global_root_has_kind(fqn, LirGlobalRootKind::TopLevelImmutableVal)
            || self
                .codegen
                .lir_global_root_has_kind(fqn, LirGlobalRootKind::TopLevelMutableVar)
            || self.codegen.has_extern_global_contract(fqn)
            || self.static_enum_unit_variant_value(fqn)
    }

    fn static_enum_unit_variant_value(&self, fqn: &str) -> bool {
        let Some((owner_fqn, variant_name)) = fqn.rsplit_once('.') else {
            return false;
        };
        self.codegen
            .expect_active_lir_program("static_enum_unit_variant_value")
            .physical_layout()
            .enums
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
        span: Span,
        site_id: mir::SiteId,
        class_fqn: &str,
        target_local: Option<LocalId>,
    ) -> Result<crate::effect_lowered::source::ClassInstanceKey, LlvmEmitError> {
        let target_ty = if let Ok(site) = self
            .codegen
            .required_lir_class_ctor_call_site(site_id, "effect-lowered class ctor layout")
        {
            if site.class_fqn != class_fqn {
                return Err(frontend_error(format!(
                    "class ctor site{} LIR class `{}` disagrees with MIR class `{class_fqn}`",
                    site_id.as_u32(),
                    site.class_fqn
                )));
            }
            site.result_ty
        } else {
            target_local
                .and_then(|local| self.body.locals.get(local.as_u32() as usize))
                .map(|local| local.ty)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "class ctor `{class_fqn}` at {span:?} target local missing typed nominal result (target_local={target_local:?})"
                    ))
                })?
        };
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.source_types.kind(target_ty) else {
            return Err(frontend_error(format!(
                "class ctor `{class_fqn}` at {span:?} site{} has non-nominal LIR result type t{}",
                site_id.as_u32(),
                target_ty.as_u32()
            )));
        };
        if nominal.fqn != class_fqn {
            return Err(frontend_error(format!(
                "class ctor `{class_fqn}` at {span:?} site{} has mismatched LIR nominal `{}`",
                site_id.as_u32(),
                nominal.fqn
            )));
        }

        let layout = self.abi.class_instance_layout(target_ty)?;
        if layout.base_fqn() != class_fqn {
            return Err(frontend_error(format!(
                "class ctor `{class_fqn}` target type t{} resolved to mismatched class layout `{}`",
                target_ty.as_u32(),
                layout.base_fqn()
            )));
        }
        Ok(layout.class_key().clone())
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
            panic!(
                "maybe_build_effect_typed_closure_target_fn_ptr_for_source_ty: TypeStore equivalence verifier accepted non-codegen effect-typed surface function at {span:?}"
            );
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
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) =
            self.codegen.types.kind(struct_ty.inner())
        else {
            return Ok(());
        };
        let layout_key = self.codegen.nominal_layout_key(nominal);
        let layout = self.codegen.struct_layouts.get(&layout_key).unwrap_or_else(|| {
            panic!(
                "install_effect_typed_closure_target_overrides_for_struct_fields: layout verifier accepted missing struct layout at {span:?}"
            )
        });
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
            mir::CallKind::Direct { callee_fqn, .. } => {
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

    fn source_type_matching_codegen_ty(&self, codegen_ty: MonoTypeId) -> Option<TypeId> {
        let display = self.codegen.types.display(codegen_ty.inner()).to_string();
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
        let raw_closure = self.codegen.expect_cg_pointer(
            closure,
            "store_closure_dynamic_entry struct closure adapter value",
        );
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
            .unwrap_or_else(|| {
                panic!(
                    "define_effect_typed_plain_closure_adapter: closure adapter ABI accepted missing carrier param at {span:?}"
                )
            })
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
            let result_ty = complete_payload_ty.unwrap_or_else(|| {
                panic!(
                    "define_effect_typed_plain_closure_adapter: closure adapter ABI accepted hidden sret without Complete payload type at {span:?}"
                )
            });
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
                let payload = call.try_as_basic_value().basic().unwrap_or_else(|| {
                    panic!(
                        "define_effect_typed_plain_closure_adapter: closure adapter ABI accepted valueless plain return at {span:?}"
                    )
                });
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
                .unwrap_or_else(|| {
                    panic!(
                        "define_effect_typed_effectful_closure_adapter: closure adapter ABI accepted missing carrier param at {span:?}"
                    )
                })
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
            .unwrap_or_else(|| {
                panic!(
                    "define_effect_typed_effectful_closure_adapter: source carrier entry returned no Step value at {span:?}"
                )
            });
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
            .unwrap_or_else(|| {
                panic!(
                    "adapter_explicit_args: closure adapter ABI accepted missing args payload param at {span:?}"
                )
            });
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

    #[allow(clippy::too_many_arguments)]
    fn lower_pure_direct_call(
        &mut self,
        span: Span,
        site_id: mir::SiteId,
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
            mir::CallKind::Direct { callee_fqn, .. } => callee_fqn,
            mir::CallKind::Closure { .. }
            | mir::CallKind::FunValue { .. }
            | mir::CallKind::FunPtr { .. }
            | mir::CallKind::Virtual { .. }
            | mir::CallKind::Interface { .. } => {
                if let mir::CallKind::Interface { receiver, dispatch } = kind
                    && dispatch.owner_fqn == "scoop.core.ToString"
                    && dispatch.member_name == "toString"
                    && let Some(callee_fqn) = self.builtin_to_string_impl_fqn_for_operand(receiver)
                {
                    let mut direct_args = Vec::with_capacity(args.len() + 1);
                    direct_args.push(mir::CallArg {
                        span,
                        name: None,
                        value: receiver.clone(),
                    });
                    direct_args.extend(args.iter().cloned());
                    return self.lower_published_plain_direct_call(
                        span,
                        callee_fqn,
                        &direct_args,
                        target_cg,
                    );
                }
                if let Some(receiver) = match kind {
                    mir::CallKind::Virtual { receiver, .. }
                    | mir::CallKind::Interface { receiver, .. } => Some(receiver),
                    _ => None,
                } && let Some(callee_fqn) = self.known_plain_call_target_fqn(site_id)
                {
                    let mut direct_args = Vec::with_capacity(args.len() + 1);
                    direct_args.push(mir::CallArg {
                        span,
                        name: None,
                        value: receiver.clone(),
                    });
                    direct_args.extend(args.iter().cloned());
                    return self.lower_published_plain_direct_call(
                        span,
                        callee_fqn,
                        &direct_args,
                        target_cg,
                    );
                }
                return self.codegen.codegen_mir_plain_dynamic_call(
                    span,
                    Some(site_id),
                    kind,
                    args,
                    self.body,
                    self.source_types,
                    self.slots,
                );
            }
            mir::CallKind::Resume { .. } => {
                panic!(
                    "lower_pure_direct_call: resume call reached effect-neutral lowering at {span:?}; boundary lowering must route it"
                );
            }
        };
        let source_site = self.codegen.published_lir_source_call_site(site_id);
        let source_site_missing = source_site.is_none();
        if source_site_missing
            && matches!(
                kind,
                mir::CallKind::Direct {
                    stable_template_key: Some(_),
                    ..
                }
            )
        {
            return Err(frontend_error(format!(
                "direct call site{} lacks published LIR source call-site contract",
                site_id.as_u32()
            )));
        }
        let plain_site = if source_site.is_none() {
            self.codegen.published_lir_plain_call_site(site_id)
        } else {
            None
        };
        let published_call_root = source_site
            .and_then(|site| site.contract.exact_callee.as_ref())
            .or_else(|| plain_site.and_then(|site| site.contract.exact_callee.as_ref()))
            .map(|exact| exact.root_fqn.clone());
        let published_semantic_root = source_site
            .and_then(|site| site.semantic_root_fqn.clone())
            .or_else(|| published_call_root.clone());
        let published_named_entry = source_site
            .and_then(|site| site.named_entry_name.clone())
            .or_else(|| {
                published_call_root
                    .as_deref()
                    .and_then(|root| self.codegen.published_named_intrinsic_entry_name(root))
            })
            .or_else(|| {
                self.codegen
                    .published_named_intrinsic_entry_name(callee_fqn)
            });
        let callee_fqn = published_call_root
            .as_deref()
            .unwrap_or(callee_fqn.as_str());
        let source_intrinsic_root = published_semantic_root.as_deref();
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
        if let Some(value) = self.lower_internal_print_string(span, callee_fqn, args)? {
            return Ok(value);
        }
        if let Some(value) = self.lower_gc_debug_intrinsic(span, callee_fqn, args)? {
            return Ok(value);
        }
        if let Some(value) = self.lower_to_int_intrinsic(span, source_intrinsic_root, args)? {
            return Ok(value);
        }
        if let Some(value) = self.lower_hash_intrinsic(span, source_intrinsic_root, args)? {
            return Ok(value);
        }
        if callee_fqn == "scoop.core.ToString.toString"
            && args.first().is_some_and(|arg| {
                self.builtin_to_string_impl_fqn_for_operand(&arg.value)
                    .is_some()
            })
        {
            return self.lower_to_string_intrinsic(span, args, target_cg);
        }
        if callee_fqn == "scoop.core.byteLength" {
            return self.lower_core_string_byte_length_call(span, args, target_cg);
        }
        if callee_fqn == "scoop.core.getByte" {
            return self.lower_core_string_get_byte_call(span, args, target_cg);
        }
        if matches!(
            callee_fqn,
            "scoop.core.abs" | "scoop.core.isNaN" | "scoop.core.isInfinite"
        ) && let Some(value) =
            self.maybe_lower_float_ext_call(span, callee_fqn, args, target_cg)?
        {
            return Ok(value);
        }
        if let Some(value) = self.lower_atomic_int_intrinsic(span, source_intrinsic_root, args)? {
            return Ok(value);
        }
        if let Some(value) = self.lower_atomic_ref_intrinsic(span, source_intrinsic_root, args)? {
            return Ok(value);
        }
        if callee_fqn == "scoop.core.panic" {
            return self.lower_panic_call(span, args);
        }
        let intrinsic_base_fqn = source_intrinsic_root;
        if intrinsic_base_fqn == Some("scoop.unsafe.invoke") {
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
                Some(site_id),
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
                Some(site_id),
                callee_fqn,
                args,
                self.body,
                self.source_types,
                transport,
                self.slots,
            )?;
            return self.codegen.coerce_value(span, value, target_cg);
        }
        let named_intrinsic_entry = published_named_entry;
        if let Some(entry_name) = named_intrinsic_entry
            && let Some(value) = self.codegen.try_codegen_named_intrinsic_mir_direct_call(
                span,
                &entry_name,
                args,
                self.body,
                self.source_types,
                transport.array.as_ref(),
                self.slots,
            )?
        {
            return Ok(value);
        }
        if self
            .abi
            .maybe_plain_callable_layout_by_root_fqn(callee_fqn)?
            .is_some()
        {
            return self.lower_published_plain_direct_call(span, callee_fqn, args, target_cg);
        }
        if let Some(value) = self.lower_top_level_funptr_direct_call(callee_fqn, span, args)? {
            return Ok(value);
        }
        if let Some(fun_ty) = self.top_level_function_value_type(callee_fqn) {
            return self
                .lower_top_level_function_value_direct_call(callee_fqn, span, args, &fun_ty);
        }
        if let Some(callee_local) = self.top_level_callable_value_local(callee_fqn) {
            return self.codegen.codegen_mir_plain_dynamic_call(
                span,
                None,
                &mir::CallKind::FunValue {
                    callee: mir::Operand::Local(callee_local),
                },
                args,
                self.body,
                self.source_types,
                self.slots,
            );
        }
        let layout = self.abi.callable_layout_by_root_fqn(callee_fqn).map_err(|err| {
            frontend_error(format!(
                "pure statement call 缺少 callee `{callee_fqn}` 的 published LIR callable contract: {err:?}"
            ))
        })?;
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
        self.extract_pure_call_complete(span, layout, step, target_cg)
    }

    fn lower_atomic_int_intrinsic(
        &mut self,
        _span: Span,
        base_fqn: Option<&str>,
        args: &[mir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let atomic_word = super::super::types::IntTy {
            bits: self.codegen.host.word_bit_width(),
            signed: true,
        };
        let Some(base_fqn) = base_fqn else {
            return Ok(None);
        };
        match base_fqn {
            "scoop.unsafe.__atomicIntLoad" => {
                if args.len() != 1 || args[0].name.is_some() {
                    self.codegen.panic_verified_intrinsic_contract(
                        "effect_lowered_atomic_int_load",
                        "argument binding drift",
                    );
                }
                let (ptr, int_ty) =
                    self.atomic_int_lvalue_ptr(&args[0].value, args[0].span, false)?;
                if int_ty != atomic_word {
                    self.codegen.panic_verified_intrinsic_contract(
                        "effect_lowered_atomic_int_load",
                        "target width drift",
                    );
                }
                let loaded = self.codegen.builder.build_load(
                    self.codegen.int_type(atomic_word),
                    ptr,
                    "atomic_int_load",
                )?;
                let inst = loaded.as_instruction_value().unwrap_or_else(|| {
                    self.codegen.panic_verified_intrinsic_contract(
                        "effect_lowered_atomic_int_load",
                        "load instruction missing",
                    )
                });
                inst.set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)
                    .unwrap_or_else(|_| {
                        self.codegen.panic_verified_intrinsic_contract(
                            "effect_lowered_atomic_int_load",
                            "failed to set atomic ordering",
                        )
                    });
                let raw = self
                    .codegen
                    .expect_int_value(loaded, "effect_lowered_atomic_int_load return");
                Ok(Some(CgValue::int(raw, atomic_word)))
            }
            "scoop.unsafe.__atomicIntStore" => {
                if args.len() != 2 || args.iter().any(|arg| arg.name.is_some()) {
                    self.codegen.panic_verified_intrinsic_contract(
                        "effect_lowered_atomic_int_store",
                        "argument binding drift",
                    );
                }
                let (ptr, int_ty) =
                    self.atomic_int_lvalue_ptr(&args[0].value, args[0].span, true)?;
                if int_ty != atomic_word {
                    self.codegen.panic_verified_intrinsic_contract(
                        "effect_lowered_atomic_int_store",
                        "target width drift",
                    );
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
                let (raw, from) = self
                    .codegen
                    .expect_cg_int(value, "effect_lowered_atomic_int_store value");
                let raw = self.codegen.cast_int(raw, from, atomic_word)?;
                let inst = self.codegen.builder.build_store(ptr, raw)?;
                inst.set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)
                    .unwrap_or_else(|_| {
                        self.codegen.panic_verified_intrinsic_contract(
                            "effect_lowered_atomic_int_store",
                            "failed to set atomic ordering",
                        )
                    });
                Ok(Some(CgValue::unit()))
            }
            "scoop.unsafe.__atomicIntCompareExchange" => {
                if args.len() != 3 || args.iter().any(|arg| arg.name.is_some()) {
                    self.codegen.panic_verified_intrinsic_contract(
                        "effect_lowered_atomic_int_cmpxchg",
                        "argument binding drift",
                    );
                }
                let (ptr, int_ty) =
                    self.atomic_int_lvalue_ptr(&args[0].value, args[0].span, true)?;
                if int_ty != atomic_word {
                    self.codegen.panic_verified_intrinsic_contract(
                        "effect_lowered_atomic_int_cmpxchg",
                        "target width drift",
                    );
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
                let ok = self
                    .codegen
                    .expect_int_value(success, "effect_lowered_atomic_int_cmpxchg success");
                Ok(Some(CgValue::bool(ok)))
            }
            _ => Ok(None),
        }
    }

    fn lower_atomic_ref_intrinsic(
        &mut self,
        _span: Span,
        base_fqn: Option<&str>,
        args: &[mir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let Some(base_fqn) = base_fqn else {
            return Ok(None);
        };
        match base_fqn {
            "scoop.unsafe.__atomicRefLoad" => {
                if args.len() != 1 || args[0].name.is_some() {
                    self.codegen.panic_verified_intrinsic_contract(
                        "effect_lowered_atomic_ref_load",
                        "argument binding drift",
                    );
                }
                let (ptr, storage_ty) =
                    self.atomic_ref_lvalue_place(&args[0].value, args[0].span, false)?;
                let llvm_ty = self.codegen.llvm_basic_type_of(args[0].span, storage_ty)?;
                let loaded = self
                    .codegen
                    .builder
                    .build_load(llvm_ty, ptr, "atomic_ref_load")?;
                let inst = loaded.as_instruction_value().unwrap_or_else(|| {
                    self.codegen.panic_verified_intrinsic_contract(
                        "effect_lowered_atomic_ref_load",
                        "load instruction missing",
                    )
                });
                inst.set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)
                    .unwrap_or_else(|_| {
                        self.codegen.panic_verified_intrinsic_contract(
                            "effect_lowered_atomic_ref_load",
                            "failed to set atomic ordering",
                        )
                    });
                Ok(Some(CgValue {
                    ty: storage_ty,
                    value: Some(loaded),
                }))
            }
            "scoop.unsafe.__atomicRefStore" => {
                if args.len() != 2 || args.iter().any(|arg| arg.name.is_some()) {
                    self.codegen.panic_verified_intrinsic_contract(
                        "effect_lowered_atomic_ref_store",
                        "argument binding drift",
                    );
                }
                let (ptr, storage_ty) =
                    self.atomic_ref_lvalue_place(&args[0].value, args[0].span, true)?;
                let raw = self.atomic_ref_operand(args[1].span, &args[1].value, storage_ty)?;
                let inst = self.codegen.builder.build_store(ptr, raw)?;
                inst.set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)
                    .unwrap_or_else(|_| {
                        self.codegen.panic_verified_intrinsic_contract(
                            "effect_lowered_atomic_ref_store",
                            "failed to set atomic ordering",
                        )
                    });
                self.codegen
                    .promote_gc_pointer_with_write_barrier(args[1].span, raw)?;
                Ok(Some(CgValue::unit()))
            }
            "scoop.unsafe.__atomicRefCompareExchange" => {
                if args.len() != 3 || args.iter().any(|arg| arg.name.is_some()) {
                    self.codegen.panic_verified_intrinsic_contract(
                        "effect_lowered_atomic_ref_cmpxchg",
                        "argument binding drift",
                    );
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
                let ok = self
                    .codegen
                    .expect_int_value(success, "effect_lowered_atomic_ref_cmpxchg success");
                self.atomic_ref_cas_barrier(args[2].span, ok, desired)?;
                Ok(Some(CgValue::bool(ok)))
            }
            _ => Ok(None),
        }
    }

    fn lower_to_int_intrinsic(
        &mut self,
        span: Span,
        base_fqn: Option<&str>,
        args: &[mir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let Some(base_fqn) = base_fqn else {
            return Ok(None);
        };
        if !matches!(
            base_fqn,
            "scoop.core.toInt" | "scoop.core.Float64.toInt" | "scoop.core.Float32.toInt"
        ) {
            return Ok(None);
        }
        if args.len() != 1 || args[0].name.is_some() {
            self.codegen.panic_verified_intrinsic_contract(
                "effect-lowered toInt intrinsic",
                "argument count or named argument drift",
            );
        }
        let arg = &args[0];
        let value_ty = self.required_operand_source_ty(&arg.value, arg.span)?;
        let value_cg = self
            .codegen
            .cg_ty_of_mir_type(self.source_types, value_ty)
            .or_else(|| self.operand_slot_cg_ty(&arg.value))
            .unwrap_or_else(|| {
                panic!(
                    "lower_to_int_intrinsic: intrinsic verifier accepted non-codegen receiver type at {:?}",
                    arg.span
                )
            });
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
                let float_val = self
                    .codegen
                    .expect_float_value(value.value.unwrap_or_else(|| {
                        panic!(
                            "lower_to_int_intrinsic: Float.toInt receiver did not publish a value at {:?}",
                            arg.span
                        )
                    }), "Float.toInt receiver value");
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
                let raw = self
                    .codegen
                    .expect_basic_value(call, "Float.toInt runtime return value");
                let int64_val = self
                    .codegen
                    .expect_int_value(raw, "Float.toInt runtime return type");
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
            _ => panic!(
                "lower_to_int_intrinsic: intrinsic verifier accepted unsupported toInt receiver type at {span:?}"
            ),
        }
    }

    fn lower_hash_intrinsic(
        &mut self,
        span: Span,
        base_fqn: Option<&str>,
        args: &[mir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let Some(base_fqn) = base_fqn else {
            return Ok(None);
        };
        if base_fqn != "scoop.core.hash" {
            return Ok(None);
        }
        if args.len() != 1 || args[0].name.is_some() {
            self.codegen.panic_verified_intrinsic_contract(
                "effect-lowered hash intrinsic",
                "argument count or named argument drift",
            );
        }

        let arg = &args[0];
        let value_ty = self.required_operand_source_ty(&arg.value, arg.span)?;
        let value_cg = self
            .codegen
            .cg_ty_of_mir_type(self.source_types, value_ty)
            .unwrap_or_else(|| {
                panic!(
                    "lower_hash_intrinsic: intrinsic verifier accepted non-codegen receiver type at {:?}",
                    arg.span
                )
            });
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
                let codepoint = self.codegen.expect_int_value(
                    value.value.unwrap_or_else(|| {
                        panic!(
                            "lower_hash_intrinsic: Char.hash receiver did not publish a value at {:?}",
                            arg.span
                        )
                    }),
                    "Char.hash receiver value",
                );
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
                    let raw = self.codegen.expect_int_value(
                        value.value.unwrap_or_else(|| {
                            panic!(
                                "lower_hash_intrinsic: Int.hash receiver did not publish a value at {:?}",
                                arg.span
                            )
                        }),
                        "Int.hash receiver value",
                    );
                    self.codegen.codegen_i64_hash_value(raw).map(Some)
                }
                CgTy::Float64 | CgTy::Float32 => self
                    .codegen
                    .codegen_float_hash_value(arg.span, value)
                    .map(Some),
                _ => panic!(
                    "lower_hash_intrinsic: intrinsic verifier accepted unsupported hash receiver type at {span:?}"
                ),
            },
        }
    }

    fn lower_to_string_intrinsic(
        &mut self,
        span: Span,
        args: &[mir::CallArg],
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let Some(arg) = args.first() else {
            self.codegen.panic_verified_intrinsic_contract(
                "effect-lowered toString intrinsic",
                "missing receiver",
            );
        };
        let value_ty = self.required_operand_source_ty(&arg.value, arg.span)?;
        let entry_name = match self.source_types.kind(value_ty) {
            TypeKind::Value(ValueTypeKind::Bool) => "bool_to_string",
            TypeKind::Value(ValueTypeKind::Char) => "char_to_string",
            TypeKind::Value(
                ValueTypeKind::Int
                | ValueTypeKind::UInt
                | ValueTypeKind::IntN(_)
                | ValueTypeKind::UIntN(_),
            ) => "int_to_string",
            TypeKind::Value(ValueTypeKind::Float64) => "float64_to_string",
            TypeKind::Value(ValueTypeKind::Float32) => "float32_to_string",
            TypeKind::Ref(RefTypeKind::String) => {
                let value = self.codegen.codegen_mir_operand_expected(
                    arg.span,
                    &arg.value,
                    self.slots,
                    Some(CgTy::String),
                )?;
                return self.codegen.coerce_value(span, value, target_cg);
            }
            _ => return Ok(CgValue::unit()),
        };
        let value = self
            .codegen
            .try_codegen_named_intrinsic_mir_direct_call(
                span,
                entry_name,
                args,
                self.body,
                self.source_types,
                None,
                self.slots,
            )?
            .unwrap_or_else(|| {
                self.codegen.panic_verified_intrinsic_contract(
                    "effect-lowered toString intrinsic",
                    "missing runtime intrinsic entry",
                )
            });
        self.codegen.coerce_value(span, value, target_cg)
    }

    fn lower_panic_call(
        &mut self,
        _span: Span,
        args: &[mir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 || args[0].name.is_some() {
            self.codegen.panic_verified_intrinsic_contract(
                "effect-lowered panic intrinsic",
                "argument count or named argument drift",
            );
        }
        let arg = &args[0];
        let message = self.codegen.codegen_mir_operand_expected(
            arg.span,
            &arg.value,
            self.slots,
            Some(CgTy::String),
        )?;
        let message = self.codegen.coerce_value(arg.span, message, CgTy::String)?;
        let message_ptr = self
            .codegen
            .expect_cg_pointer(message, "effect-lowered panic message value");
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
        let (raw, from) = self
            .codegen
            .expect_cg_int(value, "effect_lowered_atomic_int_operand");
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
            self.codegen.panic_verified_intrinsic_contract(
                "effect_lowered_atomic_int_lvalue_ptr",
                "target operand is not local",
            );
        };
        if let Some((ptr, field_cg)) =
            self.atomic_member_place_for_local(*local, span, require_writable)?
        {
            let CgTy::Int(int_ty) = field_cg else {
                self.codegen.panic_verified_intrinsic_contract(
                    "effect_lowered_atomic_int_lvalue_ptr",
                    "member target is not an integer slot",
                );
            };
            return Ok((ptr, int_ty));
        }
        if let Some((ptr, cg_ty)) =
            self.atomic_top_level_place_for_local(*local, span, require_writable)?
        {
            let CgTy::Int(int_ty) = cg_ty else {
                self.codegen.panic_verified_intrinsic_contract(
                    "effect_lowered_atomic_int_lvalue_ptr",
                    "top-level target is not an integer slot",
                );
            };
            return Ok((ptr, int_ty));
        }
        let slot = self.codegen.mir_local_slot(span, self.slots, *local)?;
        if let CgTy::Int(int_ty) = slot.cg_ty {
            return Ok((slot.ptr, int_ty));
        }
        self.codegen.panic_verified_intrinsic_contract(
            "effect_lowered_atomic_int_lvalue_ptr",
            "local target is not an integer slot",
        )
    }

    fn atomic_ref_lvalue_place(
        &mut self,
        operand: &mir::Operand,
        span: Span,
        require_writable: bool,
    ) -> Result<(PointerValue<'ctx>, CgTy), LlvmEmitError> {
        let mir::Operand::Local(local) = operand else {
            self.codegen.panic_verified_intrinsic_contract(
                "effect_lowered_atomic_ref_lvalue_place",
                "target operand is not local",
            );
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
        Ok(self
            .codegen
            .expect_cg_pointer(value, "effect_lowered_atomic_ref_operand"))
    }

    fn atomic_ref_storage_ty(&self, _span: Span, cg_ty: CgTy) -> Result<CgTy, LlvmEmitError> {
        if matches!(cg_ty, CgTy::Ref | CgTy::String) {
            return Ok(cg_ty);
        }
        self.codegen.panic_verified_intrinsic_contract(
            "effect_lowered_atomic_ref_storage_ty",
            "target is not a ref storage type",
        )
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
        _span: Span,
        require_writable: bool,
    ) -> Result<Option<(PointerValue<'ctx>, CgTy)>, LlvmEmitError> {
        // `TopLevelRef` 作为 atomic intrinsic 的 target 时必须保留静态存储地址，
        // 不能先退化成局部 slot 中的按值副本。
        let Some(fqn) = self.local_top_level_ref_fqn(local).map(str::to_owned) else {
            return Ok(None);
        };

        if self
            .codegen
            .lir_global_root_has_kind(&fqn, LirGlobalRootKind::ExternGlobal)
        {
            let root = self
                .codegen
                .expect_lir_global_root_kind(
                    &fqn,
                    LirGlobalRootKind::ExternGlobal,
                    "effect_lowered_atomic_top_level_place_for_local",
                )
                .clone();
            let extern_global = root.extern_global.as_ref().unwrap_or_else(|| {
                panic!(
                    "effect_lowered_atomic_top_level_place_for_local: extern LIR root is missing contract"
                )
            });
            if require_writable && !extern_global.mutable {
                self.codegen.panic_verified_intrinsic_contract(
                    "effect_lowered_atomic_top_level_place_for_local",
                    "extern global target is not writable",
                );
            }
            let cg_ty = self.codegen.cg_ty_of_type_id(
                self.codegen
                    .lir_global_root_ty(&root, "effect_lowered_atomic extern global"),
                "effect_lowered_atomic extern global",
            );
            let gv = self.codegen.declare_lir_extern_global(&root)?;
            return Ok(Some((gv.as_pointer_value(), cg_ty)));
        }

        if !self
            .codegen
            .lir_global_root_has_kind(&fqn, LirGlobalRootKind::TopLevelMutableVar)
        {
            return Ok(None);
        }
        let root = self
            .codegen
            .expect_lir_global_root_kind(
                &fqn,
                LirGlobalRootKind::TopLevelMutableVar,
                "effect_lowered_atomic_top_level_place_for_local",
            )
            .clone();
        let cg_ty = self.codegen.cg_ty_of_type_id(
            self.codegen
                .lir_global_root_ty(&root, "effect_lowered_atomic top-level var"),
            "effect_lowered_atomic top-level var",
        );
        let gv = self.codegen.declare_lir_top_level_var_global(&root)?;
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
        let field_fqn = self.atomic_member_value_fqn(member);
        let receiver_type_id =
            self.atomic_member_receiver_codegen_type_id(span, receiver, member)?;
        let receiver_cg = self
            .codegen
            .cg_ty_of_type_id(receiver_type_id, "effect_lowered_atomic member receiver");
        if let Some((class, field_idx, field_cg)) =
            self.codegen
                .lookup_class_field_by_fqn(field_fqn, span, Some(receiver_type_id))?
            && receiver_cg == CgTy::Ref
        {
            let field = class.fields.get(field_idx as usize).unwrap_or_else(|| {
                self.codegen.panic_verified_intrinsic_contract(
                    "effect_lowered_atomic_member_place",
                    "class field index drift",
                )
            });
            if require_writable && !field.mutable {
                self.codegen.panic_verified_intrinsic_contract(
                    "effect_lowered_atomic_member_place",
                    "class field target is not writable",
                );
            }
            let receiver_value = self.codegen.codegen_mir_operand_expected(
                span,
                receiver,
                self.slots,
                Some(CgTy::Ref),
            )?;
            let receiver_value = self.codegen.coerce_value(span, receiver_value, CgTy::Ref)?;
            let obj_ptr = self
                .codegen
                .expect_cg_pointer(receiver_value, "effect_lowered_atomic class receiver");
            let ptr = self
                .codegen
                .codegen_class_field_ptr(span, &class, obj_ptr, field_idx)?;
            return Ok((ptr, field_cg));
        }

        let CgTy::Struct(struct_ty) = receiver_cg else {
            self.codegen.panic_verified_intrinsic_contract(
                "effect_lowered_atomic_member_place",
                "member receiver is not class ref or struct",
            );
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
        struct_ty: MonoTypeId,
        require_writable: bool,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let mir::Operand::Local(local) = receiver else {
            self.codegen.panic_verified_intrinsic_contract(
                "effect_lowered_atomic_struct_receiver_ptr",
                "struct receiver operand is not local",
            );
        };
        if let Some((ptr, cg_ty)) =
            self.atomic_member_place_for_local(*local, span, require_writable)?
        {
            if cg_ty != CgTy::Struct(struct_ty) {
                self.codegen.panic_verified_intrinsic_contract(
                    "effect_lowered_atomic_struct_receiver_ptr",
                    "nested struct receiver type drift",
                );
            }
            return Ok(ptr);
        }
        let slot = self.codegen.mir_local_slot(span, self.slots, *local)?;
        if slot.cg_ty != CgTy::Struct(struct_ty) {
            self.codegen.panic_verified_intrinsic_contract(
                "effect_lowered_atomic_struct_receiver_ptr",
                "struct receiver slot type drift",
            );
        }
        Ok(slot.ptr)
    }

    fn atomic_member_receiver_codegen_type_id(
        &self,
        _span: Span,
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
        Ok(self
            .codegen
            .equivalent_codegen_type_id(self.source_types, receiver_source_ty)
            .or_else(|| {
                self.codegen
                    .equivalent_codegen_type_id(self.source_types, member.receiver_ty)
            })
            .unwrap_or_else(|| {
                self.codegen.panic_verified_intrinsic_contract(
                    "effect_lowered_atomic_member_receiver_codegen_type_id",
                    "receiver type has no codegen equivalent",
                )
            }))
    }

    fn atomic_member_value_fqn<'b>(&self, member: &'b mir::MemberAccessMetadata) -> &'b str {
        match member.resolved.as_ref() {
            Some(mir::MemberTarget::Value { fqn }) => fqn.as_str(),
            Some(_) => self.codegen.panic_verified_intrinsic_contract(
                "effect_lowered_atomic_member_value_fqn",
                "member target is not a value",
            ),
            None => self.codegen.panic_verified_intrinsic_contract(
                "effect_lowered_atomic_member_value_fqn",
                "member target is unresolved",
            ),
        }
    }

    fn required_operand_source_ty(
        &self,
        operand: &mir::Operand,
        _span: Span,
    ) -> Result<TypeId, LlvmEmitError> {
        Ok(self.operand_source_ty(operand).unwrap_or_else(|| {
            self.codegen.panic_verified_intrinsic_contract(
                "required_operand_source_ty",
                "task transport operand source type is missing",
            )
        }))
    }

    fn builtin_to_string_impl_fqn_for_operand(
        &self,
        operand: &mir::Operand,
    ) -> Option<&'static str> {
        let ty = self.operand_source_ty(operand)?;
        self.builtin_to_string_impl_fqn_for_ty(ty)
    }

    fn builtin_to_string_impl_fqn_for_ty(&self, ty: TypeId) -> Option<&'static str> {
        match self.source_types.kind(ty) {
            TypeKind::Value(ValueTypeKind::Bool) => Some("scoop.core.Bool.toString"),
            TypeKind::Value(ValueTypeKind::Char) => Some("scoop.core.Char.toString"),
            TypeKind::Value(ValueTypeKind::Float64) => Some("scoop.core.Float64.toString"),
            TypeKind::Value(ValueTypeKind::Float32) => Some("scoop.core.Float32.toString"),
            TypeKind::Value(ValueTypeKind::Int) => Some("scoop.core.Int.toString"),
            TypeKind::Ref(RefTypeKind::String) => Some("scoop.core.String.toString"),
            TypeKind::Ref(RefTypeKind::Nominal(nominal)) if nominal.fqn == "scoop.core.String" => {
                Some("scoop.core.String.toString")
            }
            _ => None,
        }
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
            panic!(
                "lower_top_level_funptr_direct_call: effect-typed top-level FunPtr `{callable_fqn}` reached effect-neutral direct call at {span:?}"
            );
        }
        let value = self
            .codegen
            .top_level_immutable_values
            .get(callable_fqn)
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "lower_top_level_funptr_direct_call: immutable value verifier accepted missing FunPtr metadata for `{callable_fqn}` at {span:?}"
                )
            });
        let funptr = self
            .codegen
            .codegen_top_level_immutable_value_access(span, &value)?;
        let funptr_addr = self
            .codegen
            .expect_cg_int(funptr, "top-level FunPtr value")
            .0;

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
            panic!(
                "lower_top_level_funptr_direct_call: FunPtr call verifier accepted arity drift for `{callable_fqn}` at {span:?}"
            );
        }
        let mut ordered_args = vec![None; source_arg_tys.len()];
        let mut next_positional = 0usize;
        for arg in args {
            let index = if let Some(name) = &arg.name {
                source_arg_tys
                    .iter()
                    .position(|(param_name, _)| param_name == name)
                    .unwrap_or_else(|| {
                        panic!(
                            "lower_top_level_funptr_direct_call: FunPtr call verifier accepted unknown named argument `{name}` at {:?}",
                            arg.span
                        )
                    })
            } else {
                let index = next_positional;
                next_positional += 1;
                index
            };
            if index >= ordered_args.len() || ordered_args[index].replace(arg).is_some() {
                panic!(
                    "lower_top_level_funptr_direct_call: FunPtr call verifier accepted duplicate/out-of-range argument at {:?}",
                    arg.span
                );
            }
        }

        let mut llvm_param_tys = Vec::with_capacity(source_arg_tys.len());
        let mut llvm_args =
            Vec::<BasicMetadataValueEnum<'ctx>>::with_capacity(source_arg_tys.len());
        for (index, (_, source_ty)) in source_arg_tys.iter().enumerate() {
            let param_cg =
                self.codegen
                    .cg_ty_of_mir_type(self.source_types, *source_ty)
                    .unwrap_or_else(|| {
                        panic!(
                            "lower_top_level_funptr_direct_call: FunPtr call verifier accepted non-codegen param type at {span:?}"
                        )
                    });
            llvm_param_tys.push(self.codegen.llvm_basic_type_of(span, param_cg)?.into());
            let arg = ordered_args[index].unwrap_or_else(|| {
                panic!(
                    "lower_top_level_funptr_direct_call: FunPtr call verifier accepted missing argument {index} at {span:?}"
                )
            });
            let value = self.codegen.codegen_mir_operand_expected(
                arg.span,
                &arg.value,
                self.slots,
                Some(param_cg),
            )?;
            let value = self.codegen.coerce_value(arg.span, value, param_cg)?;
            let raw = self
                .codegen
                .expect_cg_value(value, "top-level FunPtr arg value");
            llvm_args.push(raw.into());
        }

        let ret_cg =
            self.codegen
                .cg_ty_of_mir_type(self.source_types, fun_ty.return_ty)
                .unwrap_or_else(|| {
                    panic!(
                        "lower_top_level_funptr_direct_call: FunPtr call verifier accepted non-codegen return type at {span:?}"
                    )
                });
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
                let raw = self
                    .codegen
                    .expect_basic_value(call_site, "top-level FunPtr return value");
                Ok(Some(self.codegen.cg_value_from_loaded(span, ret_cg, raw)?))
            }
        }
    }

    fn top_level_funptr_function_type(
        &self,
        callable_fqn: &str,
    ) -> Option<crate::ty::FunctionType> {
        let root = self.codegen.lir_global_root(callable_fqn)?;
        if root.kind != LirGlobalRootKind::TopLevelImmutableVal {
            return None;
        }
        match self.codegen.types.kind(root.ty?) {
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
        let root = self.codegen.lir_global_root(callable_fqn)?;
        if root.kind != LirGlobalRootKind::TopLevelImmutableVal {
            return None;
        }
        let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.codegen.types.kind(root.ty?) else {
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
            panic!(
                "lower_top_level_function_value_direct_call: effect-typed top-level function value `{callable_fqn}` reached effect-neutral direct call at {span:?}"
            );
        }
        let value = self
            .codegen
            .top_level_immutable_values
            .get(callable_fqn)
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "lower_top_level_function_value_direct_call: immutable value verifier accepted missing function-value metadata for `{callable_fqn}` at {span:?}"
                )
            });
        let callee = self
            .codegen
            .codegen_top_level_immutable_value_access(span, &value)?;
        let callee = self.codegen.coerce_value(span, callee, CgTy::Ref)?;
        let closure_obj_i8 = self
            .codegen
            .expect_cg_pointer(callee, "top-level function-value value");
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
        let ptr = self.codegen.expect_cg_pointer(value, kind);
        Ok(ptr)
    }

    fn lower_core_string_byte_length_call(
        &mut self,
        span: Span,
        args: &[mir::CallArg],
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 || args[0].name.is_some() {
            self.codegen.panic_verified_intrinsic_contract(
                "effect-lowered core.byteLength",
                "argument count or named argument drift",
            );
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
        let result = self
            .codegen
            .expect_int_value(raw, "core byteLength load type");
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
            self.codegen.panic_verified_intrinsic_contract(
                "effect-lowered core.getByte",
                "argument count or named argument drift",
            );
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
        let index_int = self
            .codegen
            .expect_cg_int(index, "core getByte index value")
            .0;

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
            self.codegen.panic_verified_intrinsic_contract(
                "effect-lowered Float extension intrinsic",
                "argument count drift",
            );
        };
        if arg.name.is_some() {
            self.codegen.panic_verified_intrinsic_contract(
                "effect-lowered Float extension intrinsic",
                "named argument drift",
            );
        }
        let arg_cg = self
            .codegen
            .mir_operand_cg_ty(self.body, self.source_types, &arg.value)
            .unwrap_or_else(|| {
                panic!(
                    "maybe_lower_float_ext_call: intrinsic verifier accepted non-codegen Float extension arg type at {:?}",
                    arg.span
                )
            });
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
        _span: Span,
        callee_fqn: &str,
        args: &[mir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let runtime_name = match callee_fqn {
            "scoop.core.__scoop_print_string" => "scoop_print",
            "scoop.core.__scoop_println_string" => "scoop_println",
            _ => return Ok(None),
        };
        if args.len() != 1 || args[0].name.is_some() {
            self.codegen.panic_verified_intrinsic_contract(
                "effect-lowered internal print string",
                "argument count or named argument drift",
            );
        }
        let arg = &args[0];
        let value = self.codegen.codegen_mir_operand_expected(
            arg.span,
            &arg.value,
            self.slots,
            Some(CgTy::String),
        )?;
        let value = self.codegen.coerce_value(arg.span, value, CgTy::String)?;
        let str_ptr = self
            .codegen
            .expect_cg_pointer(value, "internal print string arg value");
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
        self.codegen
            .published_codegen_callable_signature(fqn)
            .is_some()
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
        let arg = self.codegen.expect_mir_positional_intrinsic_arg(
            args,
            1,
            0,
            "GC.pin effect-lowered lowering",
        );
        let CgTy::Struct(pinned_ty) = target_cg else {
            self.codegen.panic_verified_intrinsic_contract(
                "GC.pin effect-lowered lowering",
                "target is not a Pinned struct",
            );
        };
        let (field_idx, field_cg_ty) =
            self.codegen
                .lookup_struct_field(pinned_ty, "scoop.core.Pinned.value", span)?;
        let obj = self.codegen.codegen_mir_operand_expected(
            arg.span,
            &arg.value,
            self.slots,
            Some(field_cg_ty),
        )?;
        let obj = self.codegen.coerce_value(arg.span, obj, field_cg_ty)?;
        let obj_ref = self.codegen.coerce_value(arg.span, obj, CgTy::Ref)?;
        let obj_ptr = self
            .codegen
            .expect_cg_pointer(obj_ref, "GC.pin effect-lowered argument");

        let rt_pin = self.codegen.declare_runtime_gc_pin();
        let call = self
            .codegen
            .builder
            .build_call(rt_pin, &[obj_ptr.into()], "gc_pin")?;
        let raw = self
            .codegen
            .expect_basic_value(call, "GC.pin effect-lowered runtime return");
        let ok_i32 = self
            .codegen
            .expect_int_value(raw, "GC.pin effect-lowered runtime return");

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
            _ => self
                .codegen
                .expect_cg_value(obj, "GC.pin effect-lowered Pinned.value field"),
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
        let arg = self.codegen.expect_mir_positional_intrinsic_arg(
            args,
            1,
            0,
            "GC.unpin effect-lowered lowering",
        );
        let pinned = self
            .codegen
            .codegen_mir_operand_expected(arg.span, &arg.value, self.slots, None)?;
        let CgTy::Struct(pinned_ty) = pinned.ty else {
            self.codegen.panic_verified_intrinsic_contract(
                "GC.unpin effect-lowered lowering",
                "argument is not a Pinned struct",
            );
        };
        let raw = self
            .codegen
            .expect_cg_value(pinned, "GC.unpin effect-lowered argument");
        let struct_v = self
            .codegen
            .expect_struct_value(raw, "GC.unpin effect-lowered argument");
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
        let obj_ptr = self
            .codegen
            .expect_cg_pointer(field_ref, "GC.unpin effect-lowered Pinned.value field");

        let rt_unpin = self.codegen.declare_runtime_gc_unpin();
        let call = self
            .codegen
            .builder
            .build_call(rt_unpin, &[obj_ptr.into()], "gc_unpin")?;
        let raw = self
            .codegen
            .expect_basic_value(call, "GC.unpin effect-lowered runtime return");
        let ok_i32 = self
            .codegen
            .expect_int_value(raw, "GC.unpin effect-lowered runtime return");

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
                    self.codegen.panic_verified_intrinsic_contract(
                        "effect-lowered stackmap statepoint smoke",
                        "argument list drift",
                    );
                }
                let current_fun = self
                    .codegen
                    .expect_current_function("effect-lowered stackmap statepoint smoke");
                current_fun.set_gc("statepoint-example");

                let runtime = self.codegen.declare_runtime_stackmap_statepoint_smoke();
                let call = self.codegen.build_call_preserving_gc_local_roots(
                    span,
                    runtime,
                    &[],
                    "stackmap_statepoint_smoke",
                )?;
                let raw = self
                    .codegen
                    .expect_basic_value(call, "effect-lowered stackmap statepoint smoke return");
                let raw_int = self
                    .codegen
                    .expect_int_value(raw, "effect-lowered stackmap statepoint smoke return");
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
            panic!(
                "pack_call_args_for_invoke_args_tuple: effect call ABI verifier accepted named argument before canonicalization at {span:?}"
            );
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
                    .unwrap_or_else(|| {
                        panic!(
                            "pack_call_args_for_invoke_args_tuple: scalar call ABI accepted non-codegen arg type at {:?}",
                            arg.span
                        )
                    });
                let value = self.codegen.codegen_mir_operand_expected(
                    arg.span,
                    &arg.value,
                    self.slots,
                    Some(expected),
                )?;
                let value = self.codegen.coerce_value(arg.span, value, expected)?;
                Ok(Some(
                    self.codegen.expect_cg_value(value, "scalar call arg value"),
                ))
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
                        .unwrap_or_else(|| {
                            panic!(
                                "pack_call_args_for_invoke_args_tuple: tuple call ABI accepted non-codegen arg type at {:?}",
                                arg.span
                            )
                        });
                    let value = self.codegen.codegen_mir_operand_expected(
                        arg.span,
                        &arg.value,
                        self.slots,
                        Some(expected),
                    )?;
                    let value = self.codegen.coerce_value(arg.span, value, expected)?;
                    let raw = self.codegen.expect_cg_value(value, "tuple call arg value");
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
        callable_layout: &CallableLayout<'ctx>,
        step: BasicValueEnum<'ctx>,
        target_cg: super::super::types::CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let step_schema = callable_layout.step_schema();
        let step_layout = self
            .abi
            .step_layout_for_callable(callable_layout)
            .ok_or_else(|| {
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
            .unwrap_or_else(|| {
                panic!(
                    "lower_operand_source: late-lowered verifier accepted non-codegen operand source type at {span:?}"
                )
            });
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
        let layout = match self.abi.source_value_layout(source_ty) {
            Ok(layout) => layout,
            Err(err) if !sources.is_empty() => {
                return self.pack_sources_from_field_layouts(sources, name).map_err(|fallback_err| {
                    frontend_error(format!(
                        "{err}; fallback ABI packing from operand sources also failed: {fallback_err}"
                    ))
                });
            }
            Err(err) => return Err(err),
        };
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

    fn pack_sources_from_field_layouts(
        &mut self,
        sources: &[LateLoweredOperandSource],
        name: &str,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        let mut values = Vec::new();
        let mut field_tys = Vec::new();
        for source in sources {
            let field_layout = self.abi.source_value_layout(source.source_ty())?;
            if field_layout.abi().is_elided() {
                continue;
            }
            let raw = self.lower_operand_source(source)?.value.ok_or_else(|| {
                frontend_error(format!(
                    "ABI tuple payload `{name}` fallback source was elided but field needs value"
                ))
            })?;
            field_tys.push(raw.get_type());
            values.push(raw);
        }
        if values.is_empty() {
            return Ok(None);
        }
        let struct_ty = self.codegen.context.struct_type(&field_tys, false);
        let mut aggregate = struct_ty.get_undef();
        for (index, value) in values.into_iter().enumerate() {
            aggregate = self
                .codegen
                .builder
                .build_insert_value(
                    aggregate,
                    value,
                    index as u32,
                    &format!("{name}_field{index}"),
                )?
                .into_struct_value();
        }
        Ok(Some(aggregate.into()))
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

#[cfg(all(test, not(feature = "standalone-codegen-crate")))]
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
