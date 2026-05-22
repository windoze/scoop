//! Source-slice lowering helpers shared by LLVM's LIR-owned body emission path.
//!
//! Production body emission reaches these helpers only with a callable/source body
//! already published on the late-lowered LIR contract. This module must not be used
//! as a backend fallback that discovers callable bodies from HIR or a residual pass
//! view.

use std::collections::HashSet;

use inkwell::values::{BasicMetadataValueEnum, BasicValueEnum, FunctionValue, PointerValue};

use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::*;

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

pub(in crate::llvm::codegen) enum PlainDispatchTarget<'h> {
    Virtual {
        slot: u32,
        sig_fun: &'h hir::FunDecl,
    },
    Interface {
        interface_id: u64,
        slot: u32,
        receiver_ty: TypeId,
        sig_fun: &'h hir::FunDecl,
    },
}

impl<'h> PlainDispatchTarget<'h> {
    fn sig_fun(&self) -> &'h hir::FunDecl {
        match self {
            Self::Virtual { sig_fun, .. } | Self::Interface { sig_fun, .. } => sig_fun,
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

fn ensure_raw_mir_call_kind_is_route_safe(
    body_fqn: &str,
    span: crate::span::Span,
    kind: &crate::mir::CallKind,
) -> Result<(), LlvmEmitError> {
    match kind {
        crate::mir::CallKind::Direct { .. }
        | crate::mir::CallKind::Closure { .. }
        | crate::mir::CallKind::FunValue { .. }
        | crate::mir::CallKind::FunPtr { .. } => Ok(()),
        crate::mir::CallKind::Virtual { .. }
        | crate::mir::CallKind::Interface { .. }
        | crate::mir::CallKind::Resume { .. } => Err(raw_mir_route_gate_error(
            body_fqn,
            span,
            "PIPELINE_GAPS §3.6",
            RAW_MIR_CALL_KIND_DETAIL,
        )),
    }
}

fn ensure_raw_mir_rvalue_is_route_safe(
    body_fqn: &str,
    span: crate::span::Span,
    value: &crate::mir::Rvalue,
) -> Result<(), LlvmEmitError> {
    match value {
        crate::mir::Rvalue::Call { kind, .. } => {
            ensure_raw_mir_call_kind_is_route_safe(body_fqn, span, kind)
        }
        crate::mir::Rvalue::PerformResult { .. } => Err(raw_mir_route_gate_error(
            body_fqn,
            span,
            "PIPELINE_GAPS §3.3",
            RAW_MIR_PERFORM_RESULT_DETAIL,
        )),
        _ => Ok(()),
    }
}

fn ensure_raw_mir_terminator_is_route_safe(
    body_fqn: &str,
    terminator: &crate::mir::Terminator,
) -> Result<(), LlvmEmitError> {
    match &terminator.kind {
        crate::mir::TerminatorKind::Perform { .. } => Err(raw_mir_route_gate_error(
            body_fqn,
            terminator.span,
            "PIPELINE_GAPS §3.2",
            RAW_MIR_PERFORM_TERMINATOR_DETAIL,
        )),
        crate::mir::TerminatorKind::Handle { .. } | crate::mir::TerminatorKind::ResumeUnwind => {
            Err(raw_mir_route_gate_error(
                body_fqn,
                terminator.span,
                "PIPELINE_GAPS §3.1",
                RAW_MIR_EFFECT_CONTROL_TERMINATOR_DETAIL,
            ))
        }
        crate::mir::TerminatorKind::Todo(_) => Err(raw_mir_route_gate_error(
            body_fqn,
            terminator.span,
            "PIPELINE_GAPS §2.3",
            RAW_MIR_TODO_TERMINATOR_DETAIL,
        )),
        crate::mir::TerminatorKind::Return { .. }
        | crate::mir::TerminatorKind::Goto { .. }
        | crate::mir::TerminatorKind::CondBr { .. }
        | crate::mir::TerminatorKind::Unreachable => Ok(()),
    }
}

fn ensure_raw_mir_body_route_is_safe(
    body_fqn: &str,
    body: &crate::mir::Body,
) -> Result<(), LlvmEmitError> {
    for block in &body.blocks {
        for stmt in &block.stmts {
            if let crate::mir::StatementKind::Assign { value, .. } = &stmt.kind {
                ensure_raw_mir_rvalue_is_route_safe(body_fqn, stmt.span, value)?;
            }
        }
        ensure_raw_mir_terminator_is_route_safe(body_fqn, &block.terminator)?;
    }
    Ok(())
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

fn mir_direct_call_base_fqn(fqn: &str) -> &str {
    let base = fqn.rsplit_once("::<").map(|(base, _)| base).unwrap_or(fqn);
    base.split_once("$overload")
        .map(|(base, _)| base)
        .unwrap_or(base)
}

fn decompose_target_triple(triple: &str) -> (String, String, String, String) {
    let mut parts = triple.split('-');
    let arch = parts.next().unwrap_or("").to_string();
    let vendor = parts.next().unwrap_or("").to_string();
    let os = parts.next().unwrap_or("").to_string();
    let env = parts.next().unwrap_or("").to_string();
    (arch, vendor, os, env)
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

fn map_mir_call_args_to_mir_params(
    params: &[crate::mir::Param],
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

fn collect_mir_operand_use(operand: &crate::mir::Operand, out: &mut HashSet<crate::mir::LocalId>) {
    if let crate::mir::Operand::Local(local) = operand {
        out.insert(*local);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_backend_gate_gap(
        result: Result<(), LlvmEmitError>,
        expected_gap: &'static str,
        expected_detail: &'static str,
    ) {
        let entry = crate::llvm::codegen_gap_inventory::codegen_gap_entry(expected_gap)
            .expect("expected gap must stay in inventory");
        match result.expect_err("helper should reject invalid raw MIR route") {
            LlvmEmitError::BackendGate(error) => {
                assert_eq!(error.gap_id, expected_gap);
                assert_eq!(error.owner_task, entry.owner_task);
                assert_eq!(error.route, entry.route.as_str());
                assert_eq!(error.detail, expected_detail);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    fn test_span() -> crate::span::Span {
        crate::span::Span::new(0, 1)
    }

    fn source_local(body: &mut crate::mir::Body, ty: TypeId, name: &str) -> crate::mir::LocalId {
        body.push_local(crate::mir::LocalDecl {
            span: test_span(),
            name: Some(name.to_string()),
            ty,
            source: crate::mir::LocalSourceKind::SourceLocal,
        })
    }

    fn single_block_body(
        stmts: Vec<crate::mir::Statement>,
        terminator: crate::mir::Terminator,
    ) -> crate::mir::Body {
        let mut body = crate::mir::Body::new_empty();
        body.push_block(crate::mir::BasicBlock {
            is_cleanup: false,
            stmts,
            terminator,
        });
        body.start = crate::mir::BasicBlockId::from_raw(0);
        body
    }

    fn return_terminator() -> crate::mir::Terminator {
        crate::mir::Terminator {
            span: test_span(),
            kind: crate::mir::TerminatorKind::Return { value: None },
            unwind: crate::mir::UnwindAction::NoUnwind,
        }
    }

    fn body_with_virtual_call(result_ty: TypeId) -> crate::mir::Body {
        let mut body = crate::mir::Body::new_empty();
        let receiver = source_local(&mut body, result_ty, "receiver");
        let target = source_local(&mut body, result_ty, "target");
        let stmt = crate::mir::Statement {
            span: test_span(),
            kind: crate::mir::StatementKind::Assign {
                target,
                value: crate::mir::Rvalue::Call {
                    site_id: crate::mir::SiteId::from_raw(1),
                    kind: crate::mir::CallKind::Virtual {
                        receiver: crate::mir::Operand::Local(receiver),
                        dispatch: crate::mir::DispatchMetadata {
                            owner_fqn: "sample.Box".to_string(),
                            member_name: "value".to_string(),
                            member_fqn: "sample.Box.value".to_string(),
                            member_decl_span: None,
                            receiver_ty: result_ty,
                        },
                    },
                    args: Vec::new(),
                    transport: crate::mir::CallTransportMetadata::plain_no_outward(
                        result_ty,
                        crate::mir::MirTransportKind::Scalar,
                    ),
                },
            },
        };
        body.push_block(crate::mir::BasicBlock {
            is_cleanup: false,
            stmts: vec![stmt],
            terminator: return_terminator(),
        });
        body.start = crate::mir::BasicBlockId::from_raw(0);
        body
    }

    fn body_with_interface_call(result_ty: TypeId) -> crate::mir::Body {
        let mut body = crate::mir::Body::new_empty();
        let receiver = source_local(&mut body, result_ty, "receiver");
        let target = source_local(&mut body, result_ty, "target");
        let stmt = crate::mir::Statement {
            span: test_span(),
            kind: crate::mir::StatementKind::Assign {
                target,
                value: crate::mir::Rvalue::Call {
                    site_id: crate::mir::SiteId::from_raw(2),
                    kind: crate::mir::CallKind::Interface {
                        receiver: crate::mir::Operand::Local(receiver),
                        dispatch: crate::mir::DispatchMetadata {
                            owner_fqn: "sample.IBox".to_string(),
                            member_name: "value".to_string(),
                            member_fqn: "sample.IBox.value".to_string(),
                            member_decl_span: None,
                            receiver_ty: result_ty,
                        },
                    },
                    args: Vec::new(),
                    transport: crate::mir::CallTransportMetadata::plain_no_outward(
                        result_ty,
                        crate::mir::MirTransportKind::Scalar,
                    ),
                },
            },
        };
        body.push_block(crate::mir::BasicBlock {
            is_cleanup: false,
            stmts: vec![stmt],
            terminator: return_terminator(),
        });
        body.start = crate::mir::BasicBlockId::from_raw(0);
        body
    }

    fn body_with_resume_call(value_ty: TypeId) -> crate::mir::Body {
        let mut body = crate::mir::Body::new_empty();
        let continuation = source_local(&mut body, value_ty, "k");
        let target = source_local(&mut body, value_ty, "target");
        let stmt = crate::mir::Statement {
            span: test_span(),
            kind: crate::mir::StatementKind::Assign {
                target,
                value: crate::mir::Rvalue::Call {
                    site_id: crate::mir::SiteId::from_raw(3),
                    kind: crate::mir::CallKind::Resume {
                        continuation: crate::mir::Operand::Local(continuation),
                        resume: crate::mir::ResumeMetadata {
                            continuation_ty: value_ty,
                            resume_ty: value_ty,
                            answer_ty: value_ty,
                            return_ty: value_ty,
                            out_effects: crate::ty::EffectRow::pure(),
                            runtime_error_effect_ty: Some(value_ty),
                            suspends_outward: false,
                        },
                    },
                    args: Vec::new(),
                    transport: crate::mir::CallTransportMetadata::plain_no_outward(
                        value_ty,
                        crate::mir::MirTransportKind::Scalar,
                    ),
                },
            },
        };
        body.push_block(crate::mir::BasicBlock {
            is_cleanup: false,
            stmts: vec![stmt],
            terminator: return_terminator(),
        });
        body.start = crate::mir::BasicBlockId::from_raw(0);
        body
    }

    fn body_with_perform_result(effect_ty: TypeId) -> crate::mir::Body {
        let mut body = crate::mir::Body::new_empty();
        let target = source_local(&mut body, effect_ty, "target");
        let stmt = crate::mir::Statement {
            span: test_span(),
            kind: crate::mir::StatementKind::Assign {
                target,
                value: crate::mir::Rvalue::PerformResult {
                    op_fqn: "sample.Ping.hit".to_string(),
                    effect_ty,
                },
            },
        };
        body.push_block(crate::mir::BasicBlock {
            is_cleanup: false,
            stmts: vec![stmt],
            terminator: return_terminator(),
        });
        body.start = crate::mir::BasicBlockId::from_raw(0);
        body
    }

    fn perform_metadata(effect_ty: TypeId) -> crate::mir::PerformMetadata {
        crate::mir::PerformMetadata {
            effect_ty,
            op_type_args: Vec::new(),
            result_ty: effect_ty,
            payload_tuple_ty: None,
            payload_component_tys: Vec::new(),
            payload_transport: Vec::new(),
            arg_mapping: Vec::new(),
        }
    }

    #[test]
    fn llvm_raw_route_gate_rejects_unsupported_call_kinds_before_body_emission() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();

        for body in [
            body_with_virtual_call(builtins.int),
            body_with_interface_call(builtins.int),
            body_with_resume_call(builtins.int),
        ] {
            let result = ensure_raw_mir_body_route_is_safe("sample.main.$lambda0", &body);
            assert_backend_gate_gap(result, "PIPELINE_GAPS §3.6", RAW_MIR_CALL_KIND_DETAIL);
        }
    }

    #[test]
    fn llvm_raw_route_gate_rejects_perform_result_before_body_emission() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();

        let result = ensure_raw_mir_body_route_is_safe(
            "sample.main.$lambda0",
            &body_with_perform_result(builtins.int),
        );

        assert_backend_gate_gap(result, "PIPELINE_GAPS §3.3", RAW_MIR_PERFORM_RESULT_DETAIL);
    }

    #[test]
    fn raw_mir_effect_control_route_rejects_unsafe_terminators_before_body_emission() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();

        let handle_body = single_block_body(
            Vec::new(),
            crate::mir::Terminator {
                span: test_span(),
                kind: crate::mir::TerminatorKind::Handle {
                    site_id: crate::mir::SiteId::from_raw(4),
                    metadata: crate::mir::HandleMetadata {
                        result_ty: builtins.int,
                        body_result_ty: builtins.int,
                        finally_result_ty: None,
                    },
                    arms: Vec::new(),
                    has_finally: false,
                    body_target: crate::mir::BasicBlockId::from_raw(0),
                    arm_targets: Vec::new(),
                    finally_target: None,
                    exit_target: crate::mir::BasicBlockId::from_raw(0),
                },
                unwind: crate::mir::UnwindAction::NoUnwind,
            },
        );
        assert_backend_gate_gap(
            ensure_raw_mir_body_route_is_safe("sample.main.$lambda0", &handle_body),
            "PIPELINE_GAPS §3.1",
            RAW_MIR_EFFECT_CONTROL_TERMINATOR_DETAIL,
        );

        let resume_unwind_body = single_block_body(
            Vec::new(),
            crate::mir::Terminator {
                span: test_span(),
                kind: crate::mir::TerminatorKind::ResumeUnwind,
                unwind: crate::mir::UnwindAction::NoUnwind,
            },
        );
        assert_backend_gate_gap(
            ensure_raw_mir_body_route_is_safe("sample.main.$lambda0", &resume_unwind_body),
            "PIPELINE_GAPS §3.1",
            RAW_MIR_EFFECT_CONTROL_TERMINATOR_DETAIL,
        );

        let perform_body = single_block_body(
            Vec::new(),
            crate::mir::Terminator {
                span: test_span(),
                kind: crate::mir::TerminatorKind::Perform {
                    site_id: crate::mir::SiteId::from_raw(5),
                    op_fqn: "sample.Ping.hit".to_string(),
                    metadata: perform_metadata(builtins.int),
                    args: Vec::new(),
                    resume_target: crate::mir::BasicBlockId::from_raw(0),
                },
                unwind: crate::mir::UnwindAction::Cleanup {
                    target: crate::mir::BasicBlockId::from_raw(0),
                },
            },
        );
        assert_backend_gate_gap(
            ensure_raw_mir_body_route_is_safe("sample.main.$lambda0", &perform_body),
            "PIPELINE_GAPS §3.2",
            RAW_MIR_PERFORM_TERMINATOR_DETAIL,
        );
    }

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
