//! Source-slice lowering helpers shared by LLVM's LIR-owned body emission path.
//!
//! Production body emission reaches these helpers only with a callable/source body
//! already published on the late-lowered LIR contract. This module must not be used
//! as a backend fallback that discovers callable bodies from HIR or a residual pass
//! view.

use std::collections::HashSet;

use inkwell::values::{BasicMetadataValueEnum, BasicValueEnum, FunctionValue, PointerValue};

use crate::effect_lowered::ir::{
    LateLoweredCompletionPayloadSource, LateLoweredOperandValueSource, LateLoweredStateTerminator,
    StateId,
};
use crate::effect_lowered::{
    LirCallArg, LirCallKind, LirExecutableBody, LirLocalDecl, LirMemberAccessMetadata,
    LirMemberTarget, LirOperand, LirRvalue, LirStatementKind, LirTopLevelRefTarget,
};
use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::effect_lowered::ProgramAbiQuery;
use super::*;

pub(in crate::llvm::codegen) mod immutability;

#[derive(Clone, Copy)]
pub(super) struct MirLocalSlot<'ctx> {
    pub(super) cg_ty: CgTy,
    pub(super) ptr: PointerValue<'ctx>,
}

#[derive(Clone, Copy)]
pub(in crate::llvm::codegen) struct MirBodyCodegenCtx<'m, 'ctx> {
    body: &'m crate::mir::Body,
    mir_types: &'m TypeStore,
    slots: &'m [MirLocalSlot<'ctx>],
}

#[derive(Clone, Copy)]
pub(in crate::llvm::codegen) struct LirBodyCodegenCtx<'m, 'ctx> {
    body: &'m LirExecutableBody,
    source_types: &'m TypeStore,
    slots: &'m [MirLocalSlot<'ctx>],
}

pub(in crate::llvm::codegen) enum PlainDispatchTarget {
    Virtual {
        slot: u32,
        signature: CodegenCallableSignature,
    },
    Interface {
        interface_fqn: String,
        interface_id: u64,
        slot: u32,
        receiver_ty: TypeId,
        signature: CodegenCallableSignature,
    },
}

impl PlainDispatchTarget {
    fn signature(&self) -> &CodegenCallableSignature {
        match self {
            Self::Virtual { signature, .. } | Self::Interface { signature, .. } => signature,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Virtual { .. } => "plain virtual",
            Self::Interface { .. } => "plain interface",
        }
    }
}

fn frontend_error(message: String) -> LlvmEmitError {
    LlvmEmitError::Frontend { message }
}

fn bind_mir_call_args_to_params(
    params: &[crate::mir::Param],
    args: &[crate::mir::CallArg],
) -> Option<Vec<crate::mir::Operand>> {
    if args.len() != params.len() {
        return None;
    }

    let mut slots = vec![None; params.len()];
    let mut next_positional = 0usize;
    for arg in args {
        let index = if let Some(name) = &arg.name {
            params.iter().position(|param| &param.name == name)?
        } else {
            while next_positional < params.len() && slots[next_positional].is_some() {
                next_positional += 1;
            }
            let index = next_positional;
            next_positional += 1;
            index
        };
        if index >= slots.len() || slots[index].is_some() {
            return None;
        }
        slots[index] = Some(arg.value.clone());
    }

    slots.into_iter().collect()
}

const RAW_MIR_EFFECT_CONTROL_TERMINATOR_DETAIL: &str = "raw MIR effect/control terminator must be rejected or rerouted before plain/materialized MIR body emission";
const RAW_MIR_PERFORM_TERMINATOR_DETAIL: &str = "raw MIR Perform terminator must route through the published late-lowered boundary before body emission; plain/materialized MIR codegen must not guess cleanup or resume contracts";
const RAW_MIR_PERFORM_RESULT_DETAIL: &str = "raw MIR PerformResult must be eliminated before body emission; plain/materialized MIR codegen must not synthesize a default value";
const RAW_MIR_CALL_KIND_DETAIL: &str = "raw MIR dynamic dispatch/resume call kind requires an upstream published handoff contract; plain/materialized MIR codegen only accepts raw-safe direct/closure/fun-value calls";
const RAW_MIR_TODO_TERMINATOR_DETAIL: &str = "pass MIR Todo terminator must remain an upstream impossible-state guard and may not enter plain/materialized MIR body emission";

fn raw_mir_route_gate_error(
    body_fqn: &str,
    span: crate::span::Span,
    gap_id: &'static str,
    detail: &'static str,
) -> LlvmEmitError {
    let entry = crate::llvm::codegen_gap_inventory::codegen_gap_entry(gap_id)
        .expect("raw MIR route gate gap id must be in inventory");
    LlvmEmitError::BackendGate(Box::new(crate::llvm::BackendGateError {
        body_fqn: body_fqn.to_string(),
        source_span: span,
        gap_id: entry.gap_id,
        owner_task: entry.owner_task,
        suggested_owner: entry.suggested_owner,
        route: entry.route.as_str(),
        detail,
        at: span.into(),
    }))
}

fn mir_empty_return_contract_is_lowerable(
    _span: crate::span::Span,
    declared_return_cg: CgTy,
) -> Result<(), LlvmEmitError> {
    if matches!(declared_return_cg, CgTy::Unit) {
        return Ok(());
    }
    std::panic::panic_any("MIR verifier must reject non-Unit empty returns before LLVM codegen")
}

#[derive(Clone, Copy)]
pub(in crate::llvm::codegen) struct MirMemberPlace<'ctx> {
    ptr: PointerValue<'ctx>,
    field_cg: CgTy,
    writable: bool,
    packed_alignment: Option<u32>,
}

mod aggregates;
mod args;
mod call;
mod callable_lookup;
mod cast;
mod const_pat;
mod dispatch;
mod member;
mod operand;
mod string;
mod terminator;
mod transport;
mod types;
mod value_args;

fn mir_member_value_fqn_for_codegen(
    _span: crate::span::Span,
    member: &crate::mir::MemberAccessMetadata,
) -> Result<&str, LlvmEmitError> {
    match member.resolved.as_ref() {
        Some(crate::mir::MemberTarget::Value { fqn }) => Ok(fqn.as_str()),
        Some(_) => {
            panic!("mir_member_value_fqn_for_codegen: verifier accepted non-value member target")
        }
        None => {
            panic!("mir_member_value_fqn_for_codegen: verifier accepted unresolved member target")
        }
    }
}

fn lir_member_value_key_for_codegen(
    _span: crate::span::Span,
    member: &LirMemberAccessMetadata,
) -> Result<&str, LlvmEmitError> {
    match &member.resolved {
        LirMemberTarget::Value { member } | LirMemberTarget::ExtensionValue { member } => {
            Ok(member.as_str())
        }
        LirMemberTarget::Fun { .. } | LirMemberTarget::ExtensionFun { .. } => {
            panic!(
                "lir_member_value_key_for_codegen: LIR verifier accepted non-value member target"
            )
        }
    }
}

fn mir_store_member_continuation_route_is_lowerable(
    span: crate::span::Span,
    body: &crate::mir::Body,
    continuation_route: &crate::mir::StoredContinuationRoutePublication,
) -> Result<(), LlvmEmitError> {
    match continuation_route {
        crate::mir::StoredContinuationRoutePublication::Ambiguous => {
            panic!(
                "mir_store_member_continuation_route_is_lowerable: materialized MIR verifier accepted ambiguous member continuation route at {span:?}"
            );
        }
        crate::mir::StoredContinuationRoutePublication::None => Ok(()),
        crate::mir::StoredContinuationRoutePublication::Unique(route) => {
            let Some(local) = body.locals.get(route.source_local.as_u32() as usize) else {
                panic!(
                    "mir_store_member_continuation_route_is_lowerable: materialized MIR verifier accepted missing continuation route source local at {span:?}"
                );
            };
            if local.ty != route.source_ty {
                panic!(
                    "mir_store_member_continuation_route_is_lowerable: materialized MIR verifier accepted continuation route source type drift at {span:?}"
                );
            }
            Ok(())
        }
    }
}

fn lir_store_member_continuation_route_is_lowerable(
    span: crate::span::Span,
    body: &LirExecutableBody,
    continuation_route: &crate::mir::StoredContinuationRoutePublication,
) -> Result<(), LlvmEmitError> {
    match continuation_route {
        crate::mir::StoredContinuationRoutePublication::Ambiguous => {
            panic!(
                "lir_store_member_continuation_route_is_lowerable: LIR verifier accepted ambiguous member continuation route at {span:?}"
            );
        }
        crate::mir::StoredContinuationRoutePublication::None => Ok(()),
        crate::mir::StoredContinuationRoutePublication::Unique(route) => {
            let Some(local) = body.locals().get(route.source_local.as_u32() as usize) else {
                panic!(
                    "lir_store_member_continuation_route_is_lowerable: LIR verifier accepted missing continuation route source local at {span:?}"
                );
            };
            if local.ty() != route.source_ty {
                panic!(
                    "lir_store_member_continuation_route_is_lowerable: LIR verifier accepted continuation route source type drift at {span:?}"
                );
            }
            Ok(())
        }
    }
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

fn map_lir_call_args_to_param_names(
    param_names: &[String],
    args: &[LirCallArg],
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

pub(super) fn collect_mir_local_uses(body: &crate::mir::Body) -> HashSet<crate::mir::LocalId> {
    let mut out = HashSet::new();
    for block in &body.blocks {
        for stmt in &block.stmts {
            match &stmt.kind {
                crate::mir::StatementKind::Assign { value, .. } => {
                    collect_mir_rvalue_uses(value, &mut out);
                }
                crate::mir::StatementKind::StoreMember {
                    receiver, value, ..
                } => {
                    collect_mir_operand_use(receiver, &mut out);
                    collect_mir_operand_use(value, &mut out);
                }
                crate::mir::StatementKind::StoreTopLevelVar { value, .. } => {
                    collect_mir_operand_use(value, &mut out);
                }
                crate::mir::StatementKind::Nop | crate::mir::StatementKind::Todo(_) => {}
            }
        }
        collect_mir_terminator_uses(&block.terminator.kind, &mut out);
    }
    out
}

pub(super) fn collect_lir_local_uses(
    body: &LirExecutableBody,
    source_types: &TypeStore,
) -> HashSet<crate::effect_lowered::mir_source::LocalId> {
    let mut out = HashSet::new();
    for state in body.states().states() {
        for stmt in state.body().statements() {
            match &stmt.kind {
                LirStatementKind::Assign { value, .. } => {
                    collect_lir_rvalue_uses(value, source_types, &mut out);
                }
                LirStatementKind::StoreMember {
                    receiver, value, ..
                } => {
                    collect_lir_operand_use(receiver, &mut out);
                    collect_lir_operand_use(value, &mut out);
                }
                LirStatementKind::StoreGlobal { value, .. } => {
                    collect_lir_operand_use(value, &mut out);
                }
                LirStatementKind::Nop => {}
            }
        }
        collect_lir_terminator_uses(state.body().terminator(), &mut out);
    }
    out
}

fn collect_mir_operand_use(operand: &crate::mir::Operand, out: &mut HashSet<crate::mir::LocalId>) {
    if let crate::mir::Operand::Local(local) = operand {
        out.insert(*local);
    }
}

fn collect_lir_operand_use(
    operand: &LirOperand,
    out: &mut HashSet<crate::effect_lowered::mir_source::LocalId>,
) {
    if let LirOperand::Local(local) = operand {
        out.insert(*local);
    }
}

fn collect_lir_call_kind_uses(
    kind: &LirCallKind,
    out: &mut HashSet<crate::effect_lowered::mir_source::LocalId>,
) {
    match kind {
        LirCallKind::Direct { .. } => {}
        LirCallKind::Closure { callee, .. }
        | LirCallKind::FunValue { callee }
        | LirCallKind::FunPtr { callee } => collect_lir_operand_use(callee, out),
        LirCallKind::Virtual { receiver, .. } | LirCallKind::Interface { receiver, .. } => {
            collect_lir_operand_use(receiver, out);
        }
        LirCallKind::Resume { continuation, .. } => {
            collect_lir_operand_use(continuation, out);
        }
    }
}

fn collect_lir_rvalue_uses(
    value: &LirRvalue,
    source_types: &TypeStore,
    out: &mut HashSet<crate::effect_lowered::mir_source::LocalId>,
) {
    match value {
        LirRvalue::Use(operand)
        | LirRvalue::Transport { value: operand, .. }
        | LirRvalue::TypeCheck { value: operand, .. }
        | LirRvalue::Cast { value: operand, .. }
        | LirRvalue::TupleGet { tuple: operand, .. }
        | LirRvalue::PatternMatch {
            subject: operand, ..
        }
        | LirRvalue::PatternExtract {
            subject: operand, ..
        } => collect_lir_operand_use(operand, out),
        LirRvalue::MemberAccess {
            receiver, member, ..
        } => {
            if lir_member_access_uses_receiver(source_types, member) {
                collect_lir_operand_use(receiver, out);
            }
        }
        LirRvalue::Call { kind, args, .. } => {
            collect_lir_call_kind_uses(kind, out);
            for arg in args {
                collect_lir_operand_use(&arg.value, out);
            }
        }
        LirRvalue::EnumVariant { args, .. } | LirRvalue::ClassCtor { args, .. } => {
            for arg in args {
                collect_lir_operand_use(&arg.value, out);
            }
        }
        LirRvalue::MakeTuple { elements, .. } => {
            for element in elements {
                collect_lir_operand_use(element, out);
            }
        }
        LirRvalue::StructLit { fields, .. } => {
            for field in fields {
                collect_lir_operand_use(&field.value, out);
            }
        }
        LirRvalue::InterpolatedString { parts, .. } => {
            for part in parts {
                if let crate::effect_lowered::LirInterpolatedStringPartKind::Expr {
                    value, ..
                } = &part.kind
                {
                    collect_lir_operand_use(value, out);
                }
            }
        }
        LirRvalue::MakeClosure { env, .. } => collect_lir_operand_use(env, out),
        LirRvalue::TopLevelRef(_)
        | LirRvalue::SizeOf { .. }
        | LirRvalue::KindOf { .. }
        | LirRvalue::AlignOf { .. }
        | LirRvalue::DescOf { .. }
        | LirRvalue::TypeMetadataLiteral(_)
        | LirRvalue::PerformResult { .. } => {}
    }
}

fn lir_member_access_uses_receiver(
    source_types: &TypeStore,
    member: &LirMemberAccessMetadata,
) -> bool {
    !matches!(
        source_types.kind(member.receiver_ty),
        TypeKind::Ref(RefTypeKind::Any)
    )
}

fn collect_lir_terminator_uses(
    terminator: &LateLoweredStateTerminator,
    out: &mut HashSet<crate::effect_lowered::mir_source::LocalId>,
) {
    match terminator {
        LateLoweredStateTerminator::Return { payload_source, .. } => {
            if let Some(source) = payload_source.operand_source()
                && let LateLoweredOperandValueSource::Local(local) = source.value()
            {
                out.insert(*local);
            }
        }
        LateLoweredStateTerminator::Branch { cond_local, .. } => {
            out.insert(*cond_local);
        }
        LateLoweredStateTerminator::Suspend { .. }
        | LateLoweredStateTerminator::Goto { .. }
        | LateLoweredStateTerminator::HandleDispatch { .. }
        | LateLoweredStateTerminator::LocalRuntimeError { .. }
        | LateLoweredStateTerminator::ResumeUnwind
        | LateLoweredStateTerminator::Unreachable
        | LateLoweredStateTerminator::Abandon => {}
    }
}

fn collect_mir_call_kind_uses(kind: &crate::mir::CallKind, out: &mut HashSet<crate::mir::LocalId>) {
    match kind {
        crate::mir::CallKind::Direct { .. } => {}
        crate::mir::CallKind::Closure { callee, .. }
        | crate::mir::CallKind::FunValue { callee }
        | crate::mir::CallKind::FunPtr { callee } => collect_mir_operand_use(callee, out),
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
        | crate::mir::Rvalue::Transport { value: operand, .. }
        | crate::mir::Rvalue::TypeCheck { value: operand, .. }
        | crate::mir::Rvalue::Cast { value: operand, .. }
        | crate::mir::Rvalue::MemberAccess {
            receiver: operand, ..
        }
        | crate::mir::Rvalue::TupleGet { tuple: operand, .. }
        | crate::mir::Rvalue::PatternMatch {
            subject: operand, ..
        }
        | crate::mir::Rvalue::PatternExtract {
            subject: operand, ..
        } => collect_mir_operand_use(operand, out),
        crate::mir::Rvalue::Call { kind, args, .. } => {
            collect_mir_call_kind_uses(kind, out);
            for arg in args {
                collect_mir_operand_use(&arg.value, out);
            }
        }
        crate::mir::Rvalue::EnumVariant { args, .. } => {
            for arg in args {
                collect_mir_operand_use(&arg.value, out);
            }
        }
        crate::mir::Rvalue::ClassCtor { args, .. } => {
            for arg in args {
                collect_mir_operand_use(&arg.value, out);
            }
        }
        crate::mir::Rvalue::MakeTuple { elements, .. } => {
            for element in elements {
                collect_mir_operand_use(element, out);
            }
        }
        crate::mir::Rvalue::StructLit { fields, .. } => {
            for field in fields {
                collect_mir_operand_use(&field.value, out);
            }
        }
        crate::mir::Rvalue::InterpolatedString { parts, .. } => {
            for part in parts {
                if let crate::mir::InterpolatedStringPart::Expr { value, .. } = part {
                    collect_mir_operand_use(value, out);
                }
            }
        }
        crate::mir::Rvalue::MakeClosure { env, .. } => collect_mir_operand_use(env, out),
        crate::mir::Rvalue::TopLevelRef(_)
        | crate::mir::Rvalue::UnresolvedName { .. }
        | crate::mir::Rvalue::SizeOf { .. }
        | crate::mir::Rvalue::KindOf { .. }
        | crate::mir::Rvalue::AlignOf { .. }
        | crate::mir::Rvalue::DescOf { .. }
        | crate::mir::Rvalue::TypeMetadataLiteral(_)
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

#[cfg(all(test, not(feature = "standalone-codegen-crate")))]
mod tests {
    use super::*;

    #[test]
    fn mir_member_access_codegen_rejects_unresolved_metadata() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let member = crate::mir::MemberAccessMetadata {
            name: "count".to_string(),
            receiver_ty: builtins.int,
            resolved: None,
            hidden_effects: crate::ty::EffectRow::pure(),
        };

        let panic = std::panic::catch_unwind(|| {
            let _ = mir_member_value_fqn_for_codegen(crate::span::Span::new(0, 1), &member);
        })
        .expect_err("unresolved member metadata should be an internal verifier invariant");
        assert_eq!(
            panic.downcast_ref::<&str>().copied(),
            Some("mir_member_value_fqn_for_codegen: verifier accepted unresolved member target")
        );
    }

    #[test]
    fn mir_store_member_codegen_rejects_ambiguous_continuation_route() {
        let body = crate::mir::Body::new_empty();
        let panic = std::panic::catch_unwind(|| {
            let _ = mir_store_member_continuation_route_is_lowerable(
                crate::span::Span::new(0, 1),
                &body,
                &crate::mir::StoredContinuationRoutePublication::Ambiguous,
            );
        })
        .expect_err("ambiguous continuation route should be an internal verifier invariant");
        assert!(
            panic
                .downcast_ref::<String>()
                .is_some_and(|message| message.contains("ambiguous member continuation route"))
        );
    }

    #[test]
    fn mir_store_member_codegen_validates_unique_continuation_route_source() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let mut body = crate::mir::Body::new_empty();
        let source_local = body.push_local(crate::mir::LocalDecl {
            span: crate::span::Span::new(0, 1),
            name: Some("k".to_string()),
            ty: builtins.unit,
            source: crate::mir::LocalSourceKind::SourceLocal,
        });

        let ok = mir_store_member_continuation_route_is_lowerable(
            crate::span::Span::new(0, 1),
            &body,
            &crate::mir::StoredContinuationRoutePublication::Unique(
                crate::mir::StoredContinuationValueRoute {
                    source_local,
                    source_ty: builtins.unit,
                    path: Vec::new(),
                },
            ),
        );
        assert!(ok.is_ok());

        let panic = std::panic::catch_unwind(|| {
            let _ = mir_store_member_continuation_route_is_lowerable(
                crate::span::Span::new(0, 1),
                &body,
                &crate::mir::StoredContinuationRoutePublication::Unique(
                    crate::mir::StoredContinuationValueRoute {
                        source_local,
                        source_ty: builtins.int,
                        path: Vec::new(),
                    },
                ),
            );
        })
        .expect_err("continuation source type drift should be an internal verifier invariant");
        assert!(
            panic
                .downcast_ref::<String>()
                .is_some_and(|message| message.contains("source type drift"))
        );
    }

    #[test]
    fn mir_no_return_none_raw_codegen_rejects_non_unit_empty_return() {
        let panic = std::panic::catch_unwind(|| {
            let _ = mir_empty_return_contract_is_lowerable(
                crate::span::Span::new(0, 1),
                CgTy::Int(IntTy {
                    bits: 64,
                    signed: true,
                }),
            );
        })
        .expect_err("non-Unit empty MIR return should be an internal verifier invariant");

        assert_eq!(
            panic.downcast_ref::<&str>().copied(),
            Some("MIR verifier must reject non-Unit empty returns before LLVM codegen")
        );
    }

    #[test]
    fn mir_no_return_none_raw_codegen_allows_unit_empty_return() {
        assert!(
            mir_empty_return_contract_is_lowerable(crate::span::Span::new(0, 1), CgTy::Unit,)
                .is_ok()
        );
    }
}
