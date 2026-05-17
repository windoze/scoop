//! Summary-driven MIR inlining passes.
//!
//! This pass is deliberately conservative: callee eligibility still comes from pass-visible
//! monomorphic callable roots, and it only inlines small, non-recursive, body-known straight-line
//! direct calls. Canonical `MaterializedMirPassView` now also publishes request-root reachable
//! ordinary non-generic bodies up front, so caller-side rewrites can update them under the same
//! stable `InstanceKey -> family -> body` query surface.

use std::collections::{HashMap, HashSet};

use super::{
    Body, CallArg, CallKind, FunDecl, InstanceKey, InstanceSummary, LocalDecl, LocalId,
    MaterializedMir, Operand, ParamUseSummary, ResultProvenance, ResultProvenanceSource, Rvalue,
    SiteId, Statement, StatementKind, StructLitField, TerminatorKind, UnwindAction,
    summarize_pass_rewritten_fun,
};

const INLINE_SIZE_THRESHOLD: u32 = 16;
const INLINE_MAX_ITERATIONS: usize = 4;

#[derive(Debug, Clone)]
struct InlineFunction {
    key: InstanceKey,
    fun: FunDecl,
    summary: InstanceSummary,
}

#[derive(Debug, Clone)]
struct InlineCallable {
    fun: FunDecl,
    summary: InstanceSummary,
}

#[derive(Debug)]
struct InlineSnapshot {
    functions: Vec<InlineFunction>,
    caller_candidates: Vec<FunDecl>,
    inline_targets_by_fqn: HashMap<String, InlineCallable>,
    callables_by_fqn: HashMap<String, FunDecl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CallableProvenance {
    DirectFunction(String),
    KnownClosure(String),
}

/// Per-basic-block callable value provenance used only by this pass.
///
/// The map is intentionally local to a block: it keeps the first implementation conservative and
/// avoids pretending that we have a full dataflow solution across branches.
#[derive(Debug, Default)]
struct BlockCallableProvenance {
    locals: HashMap<LocalId, CallableProvenance>,
}

/// 在当前 materialized MIR pass artifacts 上运行保守 summary-driven inlining。
pub(crate) fn run_summary_driven_inlining(materialized: &mut MaterializedMir) {
    for _ in 0..INLINE_MAX_ITERATIONS {
        let snapshot = InlineSnapshot::from_materialized(materialized);
        let mut rewrites = Vec::new();

        for function in &snapshot.functions {
            if let Some(rewritten) = rewrite_fun_once(function, &snapshot) {
                let summary = summarize_pass_rewritten_fun(
                    &rewritten,
                    &materialized.types,
                    Some(&function.summary),
                );
                rewrites.push((function.key.clone(), rewritten, summary));
            }
        }

        let caller_rewrites = snapshot
            .caller_candidates
            .iter()
            .filter_map(|fun| rewrite_callable_body_once(fun, &snapshot))
            .filter(pass_publishable_caller_body)
            .collect::<Vec<_>>();

        if rewrites.is_empty() && caller_rewrites.is_empty() {
            break;
        }

        let pass_artifacts = materialized.pass_artifacts_mut();
        for (key, fun, summary) in rewrites {
            pass_artifacts.replace_callable_body(fun);
            pass_artifacts.set_instance_summary(key, summary);
        }
        for fun in caller_rewrites {
            pass_artifacts.replace_callable_body(fun);
        }
    }
}

impl InlineSnapshot {
    fn from_materialized(materialized: &MaterializedMir) -> Self {
        let pass_view = materialized.pass_view();
        let functions = pass_view
            .instances()
            .filter_map(|family| {
                let fun = family.root_body()?.clone();
                Some(InlineFunction {
                    key: family.key().clone(),
                    fun,
                    summary: family.summary().clone(),
                })
            })
            .collect::<Vec<_>>();
        let caller_candidates = materialized
            .caller_side_pass_candidate_bodies()
            .iter()
            .filter_map(|raw_fun| {
                if pass_view.owner_of_callable(&raw_fun.fqn).is_some() {
                    return None;
                }
                if pass_view.callable_body_is_overridden(&raw_fun.fqn) {
                    return pass_view.callable(&raw_fun.fqn).cloned();
                }
                Some(raw_fun.clone())
            })
            .collect::<Vec<_>>();
        let mut inline_targets_by_fqn = functions
            .iter()
            .map(|function| {
                (
                    function.fun.fqn.clone(),
                    InlineCallable {
                        fun: function.fun.clone(),
                        summary: function.summary.clone(),
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        let mut callables_by_fqn = HashMap::new();
        for function in &functions {
            callables_by_fqn.insert(function.fun.fqn.clone(), function.fun.clone());
        }
        for fun in &caller_candidates {
            inline_targets_by_fqn
                .entry(fun.fqn.clone())
                .or_insert_with(|| InlineCallable {
                    fun: fun.clone(),
                    summary: summarize_pass_rewritten_fun(fun, &materialized.types, None),
                });
            callables_by_fqn
                .entry(fun.fqn.clone())
                .or_insert_with(|| fun.clone());
        }
        Self {
            functions,
            caller_candidates,
            inline_targets_by_fqn,
            callables_by_fqn,
        }
    }

    fn get(&self, fqn: &str) -> Option<&InlineCallable> {
        self.inline_targets_by_fqn.get(fqn)
    }

    fn callable(&self, fqn: &str) -> Option<&FunDecl> {
        self.callables_by_fqn.get(fqn)
    }

    /// Recognize the current MIR shape used for a top-level function value: a non-capturing
    /// closure wrapper that only forwards its call arguments to a direct function target.
    fn forwarding_closure_direct_target(&self, fn_ptr: &str) -> Option<String> {
        let closure = self.callable(fn_ptr)?;
        let body = closure.body.as_ref()?;
        if body.validate_cfg().is_err() || body.blocks.len() != 1 || body.start.as_u32() != 0 {
            return None;
        }
        let block = &body.blocks[0];
        if block.is_cleanup || !matches!(&block.terminator.unwind, UnwindAction::NoUnwind) {
            return None;
        }
        let TerminatorKind::Return { value } = &block.terminator.kind else {
            return None;
        };

        let mut direct_call = None;
        for stmt in &block.stmts {
            match &stmt.kind {
                StatementKind::Nop => {}
                StatementKind::Assign {
                    value: Rvalue::TopLevelRef(_),
                    ..
                } => {}
                StatementKind::Assign {
                    target,
                    value:
                        Rvalue::Call {
                            kind: CallKind::Direct { callee_fqn },
                            args,
                            ..
                        },
                } => {
                    if direct_call.is_some() {
                        return None;
                    }
                    direct_call = Some((*target, callee_fqn, args));
                }
                StatementKind::Assign { .. }
                | StatementKind::StoreMember { .. }
                | StatementKind::StoreTopLevelVar { .. }
                | StatementKind::Todo(_) => return None,
            }
        }
        let (direct_target, direct_callee_fqn, direct_args) = direct_call?;
        if let Some(Operand::Local(return_local)) = value
            && *return_local != direct_target
        {
            return None;
        }
        if matches!(value, Some(Operand::Const(_))) {
            return None;
        }

        let closure_args = closure.params.iter().skip(1).collect::<Vec<_>>();
        if direct_args.len() != closure_args.len() {
            return None;
        }
        for (arg, param) in direct_args.iter().zip(closure_args) {
            if arg.name.is_some()
                || !matches!(arg.value, Operand::Local(local) if local == param.local)
            {
                return None;
            }
        }
        Some(direct_callee_fqn.clone())
    }
}

impl BlockCallableProvenance {
    fn observe_statement(&mut self, stmt: &Statement, snapshot: &InlineSnapshot) {
        let StatementKind::Assign { target, value } = &stmt.kind else {
            return;
        };
        match self.provenance_for_rvalue(value, snapshot) {
            Some(provenance) => {
                self.locals.insert(*target, provenance);
            }
            None => {
                self.locals.remove(target);
            }
        }
    }

    fn provenance_of_operand(&self, operand: &Operand) -> Option<&CallableProvenance> {
        let Operand::Local(local) = operand else {
            return None;
        };
        self.locals.get(local)
    }

    fn provenance_for_rvalue(
        &self,
        value: &Rvalue,
        snapshot: &InlineSnapshot,
    ) -> Option<CallableProvenance> {
        match value {
            Rvalue::Use(operand) => self.provenance_of_operand(operand).cloned(),
            Rvalue::Transport { value, .. } => self.provenance_of_operand(value).cloned(),
            Rvalue::TopLevelRef(top) => Some(CallableProvenance::DirectFunction(top.fqn.clone())),
            Rvalue::MemberAccess { member, .. } => match member.resolved.as_ref() {
                Some(super::MemberTarget::Fun { fqn })
                | Some(super::MemberTarget::ExtensionFun { fqn }) => {
                    Some(CallableProvenance::DirectFunction(fqn.clone()))
                }
                Some(super::MemberTarget::Value { .. })
                | Some(super::MemberTarget::ExtensionValue { .. })
                | None => None,
            },
            Rvalue::MakeClosure { fn_ptr, .. } => {
                Some(CallableProvenance::KnownClosure(fn_ptr.clone()))
            }
            Rvalue::Call {
                kind: CallKind::Direct { callee_fqn },
                args,
                ..
            } => {
                let callee = snapshot.get(callee_fqn)?;
                provenance_from_result(
                    &callee.summary.result_provenance,
                    &callee.fun.params,
                    args,
                    self,
                )
            }
            Rvalue::UnresolvedName { .. }
            | Rvalue::TypeCheck { .. }
            | Rvalue::Cast { .. }
            | Rvalue::SizeOf { .. }
            | Rvalue::KindOf { .. }
            | Rvalue::AlignOf { .. }
            | Rvalue::DescOf { .. }
            | Rvalue::TypeMetadataLiteral(_)
            | Rvalue::EnumVariant { .. }
            | Rvalue::ClassCtor { .. }
            | Rvalue::Call { .. }
            | Rvalue::MakeTuple { .. }
            | Rvalue::StructLit { .. }
            | Rvalue::InterpolatedString { .. }
            | Rvalue::TupleGet { .. }
            | Rvalue::PatternMatch { .. }
            | Rvalue::PatternExtract { .. }
            | Rvalue::PerformResult { .. }
            | Rvalue::Todo(_) => None,
        }
    }
}

fn provenance_from_result(
    result: &ResultProvenance,
    params: &[super::Param],
    args: &[CallArg],
    caller_provenance: &BlockCallableProvenance,
) -> Option<CallableProvenance> {
    match result {
        ResultProvenance::DirectFunction(fqn) => {
            Some(CallableProvenance::DirectFunction(fqn.clone()))
        }
        ResultProvenance::KnownClosure(fn_ptr) => {
            Some(CallableProvenance::KnownClosure(fn_ptr.clone()))
        }
        ResultProvenance::Param(index) => {
            provenance_from_param_result(*index, params, args, caller_provenance)
        }
        ResultProvenance::Join(sources) if sources.len() == 1 => {
            provenance_from_result_source(&sources[0], params, args, caller_provenance)
        }
        ResultProvenance::Unit
        | ResultProvenance::TopLevelValue(_)
        | ResultProvenance::PerformResult(_)
        | ResultProvenance::Join(_)
        | ResultProvenance::Unknown => None,
    }
}

fn provenance_from_result_source(
    source: &ResultProvenanceSource,
    params: &[super::Param],
    args: &[CallArg],
    caller_provenance: &BlockCallableProvenance,
) -> Option<CallableProvenance> {
    match source {
        ResultProvenanceSource::DirectFunction(fqn) => {
            Some(CallableProvenance::DirectFunction(fqn.clone()))
        }
        ResultProvenanceSource::KnownClosure(fn_ptr) => {
            Some(CallableProvenance::KnownClosure(fn_ptr.clone()))
        }
        ResultProvenanceSource::Param(index) => {
            provenance_from_param_result(*index, params, args, caller_provenance)
        }
        ResultProvenanceSource::TopLevelValue(_) | ResultProvenanceSource::PerformResult(_) => None,
    }
}

fn provenance_from_param_result(
    index: usize,
    params: &[super::Param],
    args: &[CallArg],
    caller_provenance: &BlockCallableProvenance,
) -> Option<CallableProvenance> {
    let bound_args = bind_args_to_params(params, args)?;
    let operand = bound_args.get(index)?;
    caller_provenance.provenance_of_operand(operand).cloned()
}

fn rewrite_fun_once(function: &InlineFunction, snapshot: &InlineSnapshot) -> Option<FunDecl> {
    rewrite_callable_body_once(&function.fun, snapshot)
}

fn rewrite_callable_body_once(fun: &FunDecl, snapshot: &InlineSnapshot) -> Option<FunDecl> {
    let mut rewritten = fun.clone();
    let body = rewritten.body.as_mut()?;
    let mut changed = false;

    for block_index in 0..body.blocks.len() {
        let old_stmts = std::mem::take(&mut body.blocks[block_index].stmts);
        let mut new_stmts = Vec::with_capacity(old_stmts.len());
        let mut block_provenance = BlockCallableProvenance::default();

        for stmt in old_stmts {
            if let Some(expanded) =
                try_expand_direct_call(body, &fun.fqn, &stmt, snapshot, &block_provenance)
            {
                for expanded_stmt in expanded {
                    block_provenance.observe_statement(&expanded_stmt, snapshot);
                    new_stmts.push(expanded_stmt);
                }
                changed = true;
            } else {
                block_provenance.observe_statement(&stmt, snapshot);
                new_stmts.push(stmt);
            }
        }

        body.blocks[block_index].stmts = new_stmts;
    }

    if changed {
        remove_dead_inline_artifacts(body);
    }
    changed.then_some(rewritten)
}

fn try_expand_direct_call(
    caller_body: &mut Body,
    caller_fqn: &str,
    stmt: &Statement,
    snapshot: &InlineSnapshot,
    caller_provenance: &BlockCallableProvenance,
) -> Option<Vec<Statement>> {
    let StatementKind::Assign { target, value } = &stmt.kind else {
        return None;
    };
    let Rvalue::Call { kind, args, .. } = value else {
        return None;
    };
    let CallKind::Direct { callee_fqn } = kind else {
        return None;
    };
    if callee_fqn == caller_fqn {
        return None;
    }

    let callee = snapshot.get(callee_fqn)?;
    if !callee_has_inlineable_summary(callee) {
        return None;
    }
    let param_operands = bind_args_to_params(&callee.fun.params, args)?;
    let direct_call_param_provenance = direct_call_param_provenance(
        &callee.summary,
        &callee.fun.params,
        &param_operands,
        caller_provenance,
        snapshot,
    )?;
    if !callee
        .fun
        .body
        .as_ref()
        .is_some_and(|body| body_is_inlineable(body, &direct_call_param_provenance))
    {
        return None;
    }

    expand_straight_line_call(
        caller_body,
        *target,
        stmt.span,
        &callee.fun,
        &param_operands,
        &direct_call_param_provenance,
    )
}

fn callee_has_inlineable_summary(callee: &InlineCallable) -> bool {
    callee.summary.body_known
        && !callee.summary.recursive_scc
        && callee.summary.size_cost <= INLINE_SIZE_THRESHOLD
}

fn direct_call_param_provenance(
    summary: &InstanceSummary,
    params: &[super::Param],
    param_operands: &[Operand],
    caller_provenance: &BlockCallableProvenance,
    snapshot: &InlineSnapshot,
) -> Option<HashMap<LocalId, CallableProvenance>> {
    if params.len() != param_operands.len() {
        return None;
    }
    let mut out = HashMap::new();
    for ((index, param), operand) in params.iter().enumerate().zip(param_operands) {
        if summary.param_use_summaries.get(index) != Some(&ParamUseSummary::DirectCallOnly) {
            continue;
        }
        let provenance = normalize_callable_provenance(
            caller_provenance.provenance_of_operand(operand)?,
            snapshot,
        );
        out.insert(param.local, provenance);
    }
    Some(out)
}

fn normalize_callable_provenance(
    provenance: &CallableProvenance,
    snapshot: &InlineSnapshot,
) -> CallableProvenance {
    match provenance {
        CallableProvenance::KnownClosure(fn_ptr) => snapshot
            .forwarding_closure_direct_target(fn_ptr)
            .map(CallableProvenance::DirectFunction)
            .unwrap_or_else(|| provenance.clone()),
        CallableProvenance::DirectFunction(_) => provenance.clone(),
    }
}

fn body_is_inlineable(
    body: &Body,
    direct_call_param_provenance: &HashMap<LocalId, CallableProvenance>,
) -> bool {
    if body.validate_cfg().is_err() || body.blocks.len() != 1 || body.start.as_u32() != 0 {
        return false;
    }
    let block = &body.blocks[0];
    if block.is_cleanup || !matches!(&block.terminator.unwind, UnwindAction::NoUnwind) {
        return false;
    }
    if !matches!(
        &block.terminator.kind,
        TerminatorKind::Return { value: Some(_) }
    ) {
        return false;
    }
    block
        .stmts
        .iter()
        .all(|stmt| statement_is_inlineable(&stmt.kind, direct_call_param_provenance))
}

pub(super) fn body_is_inlineable_without_callable_provenance(body: &Body) -> bool {
    body_is_inlineable(body, &HashMap::new())
}

pub(super) fn pass_publishable_caller_body(fun: &FunDecl) -> bool {
    if fun.name == "main" {
        // Entry `main` is still lowered through the dedicated HIR `codegen_main_exit_code` path.
        // Publishing a MIR override here would make reachability observe a body that production
        // entry lowering does not consume yet.
        return false;
    }
    let Some(body) = &fun.body else {
        return false;
    };
    if body.validate_cfg().is_err() {
        return false;
    }
    body.blocks.iter().all(|block| {
        !block.is_cleanup
            && matches!(&block.terminator.unwind, UnwindAction::NoUnwind)
            && terminator_is_pass_publishable(&block.terminator.kind)
            && block
                .stmts
                .iter()
                .all(|stmt| statement_is_pass_publishable(&stmt.kind))
    })
}

fn terminator_is_pass_publishable(kind: &TerminatorKind) -> bool {
    matches!(
        kind,
        TerminatorKind::Return { .. }
            | TerminatorKind::Goto { .. }
            | TerminatorKind::CondBr { .. }
            | TerminatorKind::Unreachable
    )
}

fn statement_is_pass_publishable(kind: &StatementKind) -> bool {
    match kind {
        StatementKind::Nop => true,
        StatementKind::Assign { value, .. } => rvalue_is_pass_publishable(value),
        StatementKind::StoreMember { .. } | StatementKind::StoreTopLevelVar { .. } => false,
        StatementKind::Todo(_) => false,
    }
}

fn rvalue_is_pass_publishable(value: &Rvalue) -> bool {
    match value {
        Rvalue::Use(_)
        | Rvalue::Transport { .. }
        | Rvalue::TopLevelRef(_)
        | Rvalue::SizeOf { .. }
        | Rvalue::KindOf { .. }
        | Rvalue::AlignOf { .. }
        | Rvalue::DescOf { .. }
        | Rvalue::TypeMetadataLiteral(_) => true,
        Rvalue::Call { kind, .. } => {
            matches!(kind, CallKind::Direct { .. } | CallKind::Closure { .. })
        }
        Rvalue::EnumVariant { .. } | Rvalue::ClassCtor { .. } => true,
        Rvalue::MakeTuple { .. }
        | Rvalue::StructLit { .. }
        | Rvalue::InterpolatedString { .. }
        | Rvalue::TupleGet { .. }
        | Rvalue::MakeClosure { .. } => true,
        Rvalue::UnresolvedName { .. }
        | Rvalue::TypeCheck { .. }
        | Rvalue::Cast { .. }
        | Rvalue::MemberAccess { .. }
        | Rvalue::PatternMatch { .. }
        | Rvalue::PatternExtract { .. }
        | Rvalue::PerformResult { .. }
        | Rvalue::Todo(_) => false,
    }
}

fn statement_is_inlineable(
    kind: &StatementKind,
    direct_call_param_provenance: &HashMap<LocalId, CallableProvenance>,
) -> bool {
    match kind {
        StatementKind::Nop => true,
        StatementKind::Assign { value, .. } => {
            rvalue_is_inlineable(value, direct_call_param_provenance)
        }
        StatementKind::StoreMember { .. } | StatementKind::StoreTopLevelVar { .. } => false,
        StatementKind::Todo(_) => false,
    }
}

fn rvalue_is_inlineable(
    value: &Rvalue,
    direct_call_param_provenance: &HashMap<LocalId, CallableProvenance>,
) -> bool {
    match value {
        Rvalue::Use(_)
        | Rvalue::Transport { .. }
        | Rvalue::TopLevelRef(_)
        | Rvalue::SizeOf { .. }
        | Rvalue::KindOf { .. }
        | Rvalue::AlignOf { .. }
        | Rvalue::DescOf { .. }
        | Rvalue::TypeMetadataLiteral(_) => true,
        Rvalue::Call { kind, .. } => call_kind_is_inlineable(kind, direct_call_param_provenance),
        Rvalue::EnumVariant { .. } | Rvalue::ClassCtor { .. } => true,
        Rvalue::UnresolvedName { .. }
        | Rvalue::TypeCheck { .. }
        | Rvalue::Cast { .. }
        | Rvalue::MemberAccess { .. }
        | Rvalue::MakeTuple { .. }
        | Rvalue::StructLit { .. }
        | Rvalue::InterpolatedString { .. }
        | Rvalue::TupleGet { .. }
        | Rvalue::PatternMatch { .. }
        | Rvalue::PatternExtract { .. }
        | Rvalue::MakeClosure { .. }
        | Rvalue::PerformResult { .. }
        | Rvalue::Todo(_) => false,
    }
}

fn call_kind_is_inlineable(
    kind: &CallKind,
    direct_call_param_provenance: &HashMap<LocalId, CallableProvenance>,
) -> bool {
    match kind {
        CallKind::Direct { .. } => true,
        CallKind::FunValue { callee } | CallKind::Closure { callee, .. } => {
            operand_direct_call_provenance(callee, direct_call_param_provenance).is_some()
        }
        CallKind::FunPtr { .. } => false,
        CallKind::Virtual { .. } | CallKind::Interface { .. } | CallKind::Resume { .. } => false,
    }
}

fn remove_dead_inline_artifacts(body: &mut Body) {
    // Provenance-driven rewrites can make the temporary function-value materialization dead.
    // Removing only TopLevelRef/MakeClosure assignments keeps this local cleanup narrow.
    loop {
        let used = collect_used_locals(body);
        let mut removed_any = false;
        for block in &mut body.blocks {
            let old_stmts = std::mem::take(&mut block.stmts);
            block.stmts = old_stmts
                .into_iter()
                .filter(|stmt| {
                    if dead_removable_assignment(stmt, &used) {
                        removed_any = true;
                        false
                    } else {
                        true
                    }
                })
                .collect();
        }
        if !removed_any {
            break;
        }
    }
}

fn dead_removable_assignment(stmt: &Statement, used: &HashSet<LocalId>) -> bool {
    let StatementKind::Assign { target, value } = &stmt.kind else {
        return false;
    };
    !used.contains(target) && rvalue_is_dead_removable(value)
}

fn rvalue_is_dead_removable(value: &Rvalue) -> bool {
    matches!(value, Rvalue::TopLevelRef(_) | Rvalue::MakeClosure { .. })
}

fn collect_used_locals(body: &Body) -> HashSet<LocalId> {
    let mut out = HashSet::new();
    for block in &body.blocks {
        for stmt in &block.stmts {
            collect_statement_uses(stmt, &mut out);
        }
        collect_terminator_uses(&block.terminator.kind, &mut out);
    }
    out
}

fn collect_statement_uses(stmt: &Statement, out: &mut HashSet<LocalId>) {
    match &stmt.kind {
        StatementKind::Nop | StatementKind::Todo(_) => {}
        StatementKind::Assign { value, .. } => collect_rvalue_uses(value, out),
        StatementKind::StoreMember {
            receiver,
            value,
            continuation_route,
            ..
        } => {
            collect_operand_use(receiver, out);
            collect_operand_use(value, out);
            if let super::StoredContinuationRoutePublication::Unique(route) = continuation_route {
                out.insert(route.source_local);
            }
        }
        StatementKind::StoreTopLevelVar { value, .. } => collect_operand_use(value, out),
    }
}

fn collect_rvalue_uses(value: &Rvalue, out: &mut HashSet<LocalId>) {
    match value {
        Rvalue::Use(operand)
        | Rvalue::Transport { value: operand, .. }
        | Rvalue::TypeCheck { value: operand, .. }
        | Rvalue::Cast { value: operand, .. }
        | Rvalue::TupleGet { tuple: operand, .. }
        | Rvalue::PatternMatch {
            subject: operand, ..
        }
        | Rvalue::PatternExtract {
            subject: operand, ..
        } => collect_operand_use(operand, out),
        Rvalue::MemberAccess { receiver, .. } => collect_operand_use(receiver, out),
        Rvalue::Call { kind, args, .. } => {
            collect_call_kind_uses(kind, out);
            for arg in args {
                collect_operand_use(&arg.value, out);
            }
        }
        Rvalue::EnumVariant { args, .. } => {
            for arg in args {
                collect_operand_use(&arg.value, out);
            }
        }
        Rvalue::ClassCtor { args, .. } => {
            for arg in args {
                collect_operand_use(&arg.value, out);
            }
        }
        Rvalue::MakeTuple { elements, .. } => {
            for element in elements {
                collect_operand_use(element, out);
            }
        }
        Rvalue::StructLit { fields, .. } => {
            for field in fields {
                collect_operand_use(&field.value, out);
            }
        }
        Rvalue::InterpolatedString { parts, .. } => {
            for part in parts {
                if let super::InterpolatedStringPart::Expr { value, .. } = part {
                    collect_operand_use(value, out);
                }
            }
        }
        Rvalue::MakeClosure { env, .. } => collect_operand_use(env, out),
        Rvalue::TopLevelRef(_)
        | Rvalue::UnresolvedName { .. }
        | Rvalue::SizeOf { .. }
        | Rvalue::KindOf { .. }
        | Rvalue::AlignOf { .. }
        | Rvalue::DescOf { .. }
        | Rvalue::TypeMetadataLiteral(_)
        | Rvalue::PerformResult { .. }
        | Rvalue::Todo(_) => {}
    }
}

fn collect_call_kind_uses(kind: &CallKind, out: &mut HashSet<LocalId>) {
    match kind {
        CallKind::Direct { .. } => {}
        CallKind::Closure { callee, .. }
        | CallKind::FunValue { callee }
        | CallKind::FunPtr { callee } => {
            collect_operand_use(callee, out);
        }
        CallKind::Virtual { receiver, .. } | CallKind::Interface { receiver, .. } => {
            collect_operand_use(receiver, out);
        }
        CallKind::Resume { continuation, .. } => collect_operand_use(continuation, out),
    }
}

fn collect_terminator_uses(kind: &TerminatorKind, out: &mut HashSet<LocalId>) {
    match kind {
        TerminatorKind::Return { value } => {
            if let Some(value) = value {
                collect_operand_use(value, out);
            }
        }
        TerminatorKind::CondBr { cond, .. } => collect_operand_use(cond, out),
        TerminatorKind::Perform { args, .. } => {
            for arg in args {
                collect_operand_use(&arg.value, out);
            }
        }
        TerminatorKind::Goto { .. }
        | TerminatorKind::ResumeUnwind
        | TerminatorKind::Handle { .. }
        | TerminatorKind::Unreachable
        | TerminatorKind::Todo(_) => {}
    }
}

fn collect_operand_use(operand: &Operand, out: &mut HashSet<LocalId>) {
    if let Operand::Local(local) = operand {
        out.insert(*local);
    }
}

fn expand_straight_line_call(
    caller_body: &mut Body,
    caller_target: LocalId,
    call_span: crate::span::Span,
    callee: &FunDecl,
    param_operands: &[Operand],
    direct_call_param_provenance: &HashMap<LocalId, CallableProvenance>,
) -> Option<Vec<Statement>> {
    let callee_body = callee.body.as_ref()?;
    let block = &callee_body.blocks[0];
    let mut next_site_id = caller_body.next_unused_site_id().as_u32();
    let mut local_operands = callee
        .params
        .iter()
        .map(|param| param.local)
        .zip(param_operands.iter().cloned())
        .collect::<HashMap<_, _>>();
    let mut local_map = HashMap::new();
    let mut out = Vec::with_capacity(block.stmts.len() + 1);

    for callee_stmt in &block.stmts {
        match &callee_stmt.kind {
            StatementKind::Nop => out.push(callee_stmt.clone()),
            StatementKind::Assign { target, value } => {
                if local_operands.contains_key(target) {
                    return None;
                }
                let mapped_target = mapped_callee_local(
                    caller_body,
                    callee_body,
                    *target,
                    &mut local_map,
                    callee_stmt.span,
                )?;
                let mapped_value = remap_rvalue(
                    value,
                    &mut next_site_id,
                    &local_operands,
                    &local_map,
                    direct_call_param_provenance,
                )?;
                out.push(Statement {
                    span: callee_stmt.span,
                    kind: StatementKind::Assign {
                        target: mapped_target,
                        value: mapped_value,
                    },
                });
            }
            StatementKind::StoreMember { .. } | StatementKind::StoreTopLevelVar { .. } => {
                return None;
            }
            StatementKind::Todo(_) => return None,
        }
    }

    let TerminatorKind::Return {
        value: Some(return_operand),
    } = &block.terminator.kind
    else {
        return None;
    };
    let return_operand = remap_operand(return_operand, &local_operands, &local_map)?;
    out.push(Statement {
        span: call_span,
        kind: StatementKind::Assign {
            target: caller_target,
            value: Rvalue::Use(return_operand),
        },
    });

    local_operands.clear();
    Some(out)
}

pub(super) fn expand_straight_line_call_without_callable_provenance(
    caller_body: &mut Body,
    caller_target: LocalId,
    call_span: crate::span::Span,
    callee: &FunDecl,
    param_operands: &[Operand],
) -> Option<Vec<Statement>> {
    expand_straight_line_call(
        caller_body,
        caller_target,
        call_span,
        callee,
        param_operands,
        &HashMap::new(),
    )
}

fn bind_args_to_params(params: &[super::Param], args: &[CallArg]) -> Option<Vec<Operand>> {
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

fn mapped_callee_local(
    caller_body: &mut Body,
    callee_body: &Body,
    callee_local: LocalId,
    local_map: &mut HashMap<LocalId, LocalId>,
    span: crate::span::Span,
) -> Option<LocalId> {
    if let Some(mapped) = local_map.get(&callee_local).copied() {
        return Some(mapped);
    }
    let decl = callee_body
        .locals
        .get(callee_local.as_u32() as usize)?
        .clone();
    let mapped = caller_body.push_local(inline_local_decl(decl, span));
    local_map.insert(callee_local, mapped);
    Some(mapped)
}

fn inline_local_decl(mut decl: LocalDecl, span: crate::span::Span) -> LocalDecl {
    decl.span = span;
    decl.name = decl.name.map(|name| format!("inline.{name}"));
    decl
}

fn remap_rvalue(
    value: &Rvalue,
    next_site_id: &mut u32,
    local_operands: &HashMap<LocalId, Operand>,
    local_map: &HashMap<LocalId, LocalId>,
    direct_call_param_provenance: &HashMap<LocalId, CallableProvenance>,
) -> Option<Rvalue> {
    match value {
        Rvalue::Use(operand) => Some(Rvalue::Use(remap_operand(
            operand,
            local_operands,
            local_map,
        )?)),
        Rvalue::Transport { value, transport } => Some(Rvalue::Transport {
            value: remap_operand(value, local_operands, local_map)?,
            transport: transport.clone(),
        }),
        Rvalue::TopLevelRef(top) => Some(Rvalue::TopLevelRef(top.clone())),
        Rvalue::Call {
            kind,
            args,
            transport,
            ..
        } => Some(Rvalue::Call {
            site_id: fresh_cloned_site_id(next_site_id),
            kind: remap_call_kind(
                kind,
                local_operands,
                local_map,
                direct_call_param_provenance,
            )?,
            args: remap_call_args(args, local_operands, local_map)?,
            transport: transport.clone(),
        }),
        Rvalue::EnumVariant {
            enum_ty,
            variant_name,
            args,
            payload,
        } => Some(Rvalue::EnumVariant {
            enum_ty: *enum_ty,
            variant_name: variant_name.clone(),
            args: remap_call_args(args, local_operands, local_map)?,
            payload: payload.clone(),
        }),
        Rvalue::ClassCtor {
            class_fqn,
            ctor,
            args,
            hidden_effects,
            ..
        } => Some(Rvalue::ClassCtor {
            site_id: fresh_cloned_site_id(next_site_id),
            class_fqn: class_fqn.clone(),
            ctor: ctor.clone(),
            args: remap_call_args(args, local_operands, local_map)?,
            hidden_effects: hidden_effects.clone(),
        }),
        Rvalue::SizeOf { value_ty } => Some(Rvalue::SizeOf {
            value_ty: *value_ty,
        }),
        Rvalue::KindOf { value_ty } => Some(Rvalue::KindOf {
            value_ty: *value_ty,
        }),
        Rvalue::AlignOf { value_ty } => Some(Rvalue::AlignOf {
            value_ty: *value_ty,
        }),
        Rvalue::DescOf { value_ty } => Some(Rvalue::DescOf {
            value_ty: *value_ty,
        }),
        Rvalue::TypeMetadataLiteral(metadata) => {
            Some(Rvalue::TypeMetadataLiteral(metadata.clone()))
        }
        Rvalue::StructLit { fields, transport } => Some(Rvalue::StructLit {
            fields: remap_struct_lit_fields(fields, local_operands, local_map)?,
            transport: transport.clone(),
        }),
        Rvalue::MakeTuple {
            elements,
            transport,
        } => Some(Rvalue::MakeTuple {
            elements: elements
                .iter()
                .map(|element| remap_operand(element, local_operands, local_map))
                .collect::<Option<Vec<_>>>()?,
            transport: transport.clone(),
        }),
        Rvalue::UnresolvedName { .. }
        | Rvalue::TypeCheck { .. }
        | Rvalue::Cast { .. }
        | Rvalue::MemberAccess { .. }
        | Rvalue::InterpolatedString { .. }
        | Rvalue::TupleGet { .. }
        | Rvalue::PatternMatch { .. }
        | Rvalue::PatternExtract { .. }
        | Rvalue::MakeClosure { .. }
        | Rvalue::PerformResult { .. }
        | Rvalue::Todo(_) => None,
    }
}

fn remap_struct_lit_fields(
    fields: &[StructLitField],
    local_operands: &HashMap<LocalId, Operand>,
    local_map: &HashMap<LocalId, LocalId>,
) -> Option<Vec<StructLitField>> {
    fields
        .iter()
        .map(|field| {
            Some(StructLitField {
                span: field.span,
                name: field.name.clone(),
                value: remap_operand(&field.value, local_operands, local_map)?,
            })
        })
        .collect()
}

fn fresh_cloned_site_id(next_site_id: &mut u32) -> SiteId {
    let site_id = SiteId::from_raw(*next_site_id);
    *next_site_id = next_site_id
        .checked_add(1)
        .expect("too many cloned MIR site ids in inline pass");
    site_id
}

fn remap_call_args(
    args: &[CallArg],
    local_operands: &HashMap<LocalId, Operand>,
    local_map: &HashMap<LocalId, LocalId>,
) -> Option<Vec<CallArg>> {
    args.iter()
        .map(|arg| {
            Some(CallArg {
                span: arg.span,
                name: arg.name.clone(),
                value: remap_operand(&arg.value, local_operands, local_map)?,
            })
        })
        .collect()
}

fn remap_call_kind(
    kind: &CallKind,
    local_operands: &HashMap<LocalId, Operand>,
    local_map: &HashMap<LocalId, LocalId>,
    direct_call_param_provenance: &HashMap<LocalId, CallableProvenance>,
) -> Option<CallKind> {
    match kind {
        CallKind::Direct { callee_fqn } => Some(CallKind::Direct {
            callee_fqn: callee_fqn.clone(),
        }),
        CallKind::FunValue { callee } => remap_known_callable_call_kind(
            callee,
            local_operands,
            local_map,
            direct_call_param_provenance,
        ),
        CallKind::FunPtr { callee } => Some(CallKind::FunPtr {
            callee: remap_operand(callee, local_operands, local_map)?,
        }),
        CallKind::Closure { callee, fn_ptr } => {
            if let Some(kind) = remap_known_callable_call_kind(
                callee,
                local_operands,
                local_map,
                direct_call_param_provenance,
            ) {
                return Some(kind);
            }
            Some(CallKind::Closure {
                callee: remap_operand(callee, local_operands, local_map)?,
                fn_ptr: fn_ptr.clone(),
            })
        }
        CallKind::Virtual { .. } | CallKind::Interface { .. } | CallKind::Resume { .. } => None,
    }
}

fn remap_known_callable_call_kind(
    callee: &Operand,
    local_operands: &HashMap<LocalId, Operand>,
    local_map: &HashMap<LocalId, LocalId>,
    direct_call_param_provenance: &HashMap<LocalId, CallableProvenance>,
) -> Option<CallKind> {
    let provenance = operand_direct_call_provenance(callee, direct_call_param_provenance)?;
    match provenance {
        CallableProvenance::DirectFunction(callee_fqn) => Some(CallKind::Direct {
            callee_fqn: callee_fqn.clone(),
        }),
        CallableProvenance::KnownClosure(fn_ptr) => Some(CallKind::Closure {
            callee: remap_operand(callee, local_operands, local_map)?,
            fn_ptr: fn_ptr.clone(),
        }),
    }
}

fn operand_direct_call_provenance<'a>(
    operand: &Operand,
    direct_call_param_provenance: &'a HashMap<LocalId, CallableProvenance>,
) -> Option<&'a CallableProvenance> {
    let Operand::Local(local) = operand else {
        return None;
    };
    direct_call_param_provenance.get(local)
}

fn remap_operand(
    operand: &Operand,
    local_operands: &HashMap<LocalId, Operand>,
    local_map: &HashMap<LocalId, LocalId>,
) -> Option<Operand> {
    match operand {
        Operand::Const(value) => Some(Operand::Const(value.clone())),
        Operand::Local(local) => local_operands
            .get(local)
            .cloned()
            .or_else(|| local_map.get(local).copied().map(Operand::Local)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::{
        BasicBlock, BasicBlockId, CallTransportMetadata, Item, LocalSourceKind, MirTransportKind,
        PerformMetadata, Terminator, materialize_for_dump,
    };
    use crate::session::Session;
    use crate::source::SourceFile;
    use crate::ty::TypeStore;

    #[test]
    fn small_direct_call_inlining_rewrites_pass_body_without_mutating_raw_mir() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_inline_direct_call.scoop",
            r#"
package fixtures.mirinline

fun <T> id(x: T): T {
    return x
}

fun <T> wrap(x: T): T {
    return id<T>(x)
}

fun main(): Int {
    return wrap<Int>(1)
}
"#,
        );

        let materialized = materialize_for_dump(&sess, &source).unwrap();
        let wrap_fqn = "fixtures.mirinline.wrap::<Int>";
        let id_fqn = "fixtures.mirinline.id::<Int>";

        let raw_wrap = raw_fun(&materialized, wrap_fqn);
        assert!(
            fun_contains_direct_call(raw_wrap, id_fqn),
            "raw materialized MIR 应继续保留原始 direct call，证明 pass rewrite 没有回写 raw MIR"
        );

        let pass_view = materialized.pass_view();
        let owner = pass_view
            .owner_of_callable(wrap_fqn)
            .expect("wrap instance 应归属一个 pass family")
            .clone();
        let pass_family = pass_view
            .instance(&owner)
            .expect("wrap instance family 应可查询");
        assert!(
            pass_family.summary_is_overridden(),
            "inlining 后应显式覆盖 wrap 的 pass summary"
        );
        let pass_wrap = pass_family.root_body().expect("wrap pass body 应继续存在");
        assert!(
            !fun_contains_direct_call(pass_wrap, id_fqn),
            "pass-rewritten wrap body 不应继续调用被内联的 id"
        );
    }

    #[test]
    fn small_direct_call_inlining_is_not_name_based() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_inline_non_name_based.scoop",
            r#"
package fixtures.mirinline

fun <T> project(value: T): T {
    return value
}

fun <T> shell(value: T): T {
    return project<T>(value)
}

fun main(): Int {
    return shell<Int>(1)
}
"#,
        );

        let materialized = materialize_for_dump(&sess, &source).unwrap();
        let shell_fqn = "fixtures.mirinline.shell::<Int>";
        let project_fqn = "fixtures.mirinline.project::<Int>";
        let pass_shell = materialized
            .pass_view()
            .callable(shell_fqn)
            .expect("shell pass body 应存在");

        assert!(
            !fun_contains_direct_call(pass_shell, project_fqn),
            "内联应由 summary 与 MIR 结构触发，而不是依赖 id/wrap 这类函数名"
        );
    }

    #[test]
    fn caller_side_inlining_keeps_non_generic_pass_roots_visible() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_inline_non_generic_caller.scoop",
            r#"
package fixtures.mirinline

fun <T> id(x: T): T {
    return x
}

fun <T> wrap(x: T): T {
    return id<T>(x)
}

fun caller(x: Int): Int {
    return wrap<Int>(x)
}

fun stable(x: Int): Int {
    return x + 1
}

fun main(): Int {
    return caller(1) + stable(2)
}
"#,
        );

        let materialized = materialize_for_dump(&sess, &source).unwrap();
        let caller_fqn = "fixtures.mirinline.caller";
        let stable_fqn = "fixtures.mirinline.stable";
        let wrap_fqn = "fixtures.mirinline.wrap::<Int>";
        let id_fqn = "fixtures.mirinline.id::<Int>";
        let pass_view = materialized.pass_view();

        assert!(
            pass_view.owner_of_callable(caller_fqn).is_some(),
            "canonical pass view 应为 ordinary non-generic caller 发布稳定 owner"
        );
        let pass_caller = pass_view
            .callable(caller_fqn)
            .expect("caller-side inlining 应把真实 non-generic caller body 写入 pass artifacts");
        assert!(
            !fun_contains_direct_call(pass_caller, wrap_fqn),
            "caller pass body 不应继续保留被内联的 wrap 调用"
        );
        assert!(
            !fun_contains_direct_call(pass_caller, id_fqn),
            "迭代 inlining 后 caller pass body 不应继续保留 wrap 内部的 id 调用"
        );
        assert!(
            pass_view.callable(stable_fqn).is_some(),
            "ordinary non-generic body 应在 canonical pass view 上正式发布"
        );
        assert!(
            !pass_view.callable_body_is_overridden(stable_fqn),
            "未改写的 non-generic body 不应被标记为 pass override"
        );
    }

    #[test]
    fn direct_call_only_param_with_direct_function_provenance_flattens_wrapper() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_inline_direct_call_only_direct_function.scoop",
            r#"
package fixtures.mirinline

fun <T> id(x: T): T {
    return x
}

fun <T> apply(f: (T) -> T / Pure!, x: T): T {
    return f(x)
}

fun caller(x: Int): Int {
    return apply<Int>(id<Int>, x)
}

fun main(): Int {
    return caller(1)
}
"#,
        );

        let materialized = materialize_for_dump(&sess, &source).unwrap();
        let apply_fqn = "fixtures.mirinline.apply::<Int>";
        let id_fqn = "fixtures.mirinline.id::<Int>";
        let caller_fqn = "fixtures.mirinline.caller";

        let pass_view = materialized.pass_view();
        let apply_owner = pass_view
            .owner_of_callable(apply_fqn)
            .expect("apply instance 应归属 pass family");
        let apply_summary = materialized
            .summaries
            .get(apply_owner)
            .expect("apply instance summary 应存在");
        assert_eq!(
            apply_summary.param_use_summaries.first(),
            Some(&ParamUseSummary::DirectCallOnly),
            "高阶 wrapper 参数必须先由 summary 标记为 DirectCallOnly"
        );

        let raw_caller = materialized
            .caller_side_pass_candidate_bodies()
            .iter()
            .find(|fun| fun.fqn == caller_fqn)
            .expect("caller 应进入 caller-side pass 候选");
        assert!(
            fun_contains_direct_call(raw_caller, apply_fqn),
            "raw caller MIR 应先保留 wrapper direct call"
        );

        let pass_caller = pass_view
            .callable(caller_fqn)
            .expect("known direct-function provenance 应让 caller body 被 pass 发布");
        assert!(
            !fun_contains_direct_call(pass_caller, apply_fqn),
            "caller pass body 不应继续调用高阶 wrapper"
        );
        assert!(
            !fun_contains_direct_call(pass_caller, id_fqn),
            "DirectCallOnly + provenance 摊平后应继续走普通 small direct-call inlining"
        );
        assert!(
            !fun_contains_fun_value_call(pass_caller),
            "pass body 中不应残留 wrapper 内部的函数值参数调用"
        );
    }

    #[test]
    fn direct_call_only_param_with_known_closure_provenance_rewrites_to_closure_call() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_inline_direct_call_only_known_closure.scoop",
            r#"
package fixtures.mirinline

fun <T> apply(f: (T) -> T / Pure!, x: T): T {
    return f(x)
}

fun caller(x: Int): Int {
    return apply<Int>({ y -> y + 1 }, x)
}
"#,
        );

        let materialized = materialize_for_dump(&sess, &source).unwrap();
        let caller = materialized
            .caller_side_pass_candidate_bodies()
            .iter()
            .find(|fun| fun.fqn == "fixtures.mirinline.caller")
            .expect("caller 应进入 caller-side pass 候选");
        let snapshot = InlineSnapshot::from_materialized(&materialized);
        let rewritten = rewrite_callable_body_once(caller, &snapshot)
            .expect("known closure provenance 应可改写 caller");

        assert!(
            fun_contains_closure_call(&rewritten),
            "known closure provenance 应把 wrapper 内部 FunValue call 收缩为结构化 ClosureCall"
        );
        assert!(
            !fun_contains_fun_value_call(&rewritten),
            "改写后的 wrapper body 不应继续保留模糊 FunValue call"
        );
    }

    #[test]
    fn non_generic_direct_call_only_wrapper_with_known_closure_provenance_is_published() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_inline_non_generic_known_closure_publish.scoop",
            r#"
package fixtures.mirinline

fun apply(f: (Int) -> Int / Pure!, x: Int): Int {
    return f(x)
}

fun caller(x: Int): Int {
    val delta = 1
    return apply({ y -> y + delta }, x)
}

fun main(): Int {
    return caller(1)
}
"#,
        );

        let materialized = materialize_for_dump(&sess, &source).unwrap();
        let caller_fqn = "fixtures.mirinline.caller";
        let apply_fqn = "fixtures.mirinline.apply";
        let raw_caller = materialized
            .caller_side_pass_candidate_bodies()
            .iter()
            .find(|fun| fun.fqn == caller_fqn)
            .expect("caller 应进入 caller-side pass 候选");
        assert!(
            fun_contains_direct_call(raw_caller, apply_fqn),
            "raw caller MIR 应先保留 direct call 到 non-generic wrapper"
        );

        let pass_caller = materialized
            .pass_view()
            .callable(caller_fqn)
            .expect("known closure provenance 应发布 caller 的 pass-visible MIR body");
        assert!(
            fun_contains_closure_call(pass_caller),
            "pass-visible caller body 应把 wrapper 内部 FunValue call 收缩为结构化 ClosureCall"
        );
        assert!(
            !fun_contains_fun_value_call(pass_caller),
            "pass-visible caller body 不应继续保留模糊 FunValue call"
        );
    }

    #[test]
    fn mir_site_id_inline_clones_allocate_fresh_call_site_ids() {
        let mut types = TypeStore::default();
        let builtins = types.intern_builtins();

        let mut caller_body = Body::new_empty();
        let caller_existing = caller_body.push_local(LocalDecl {
            span: crate::span::Span::new(0, 0),
            name: Some("seed".to_string()),
            ty: builtins.unit,
            source: LocalSourceKind::SourceLocal,
        });
        let caller_target = caller_body.push_local(LocalDecl {
            span: crate::span::Span::new(0, 0),
            name: Some("result".to_string()),
            ty: builtins.unit,
            source: LocalSourceKind::SourceLocal,
        });
        let caller_entry = caller_body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: vec![Statement {
                span: crate::span::Span::new(0, 0),
                kind: StatementKind::Assign {
                    target: caller_existing,
                    value: Rvalue::Call {
                        site_id: SiteId::from_raw(0),
                        kind: CallKind::Direct {
                            callee_fqn: "fixtures.inline.seed".to_string(),
                        },
                        args: Vec::new(),
                        transport: call_transport(builtins.unit),
                    },
                },
            }],
            terminator: Terminator {
                span: crate::span::Span::new(0, 0),
                kind: TerminatorKind::Return { value: None },
                unwind: UnwindAction::NoUnwind,
            },
        });
        caller_body.start = caller_entry;

        let mut callee_body = Body::new_empty();
        let callee_tmp = callee_body.push_local(LocalDecl {
            span: crate::span::Span::new(0, 0),
            name: Some("tmp0".to_string()),
            ty: builtins.unit,
            source: LocalSourceKind::CompilerTemporary,
        });
        let callee_entry = callee_body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: vec![Statement {
                span: crate::span::Span::new(0, 0),
                kind: StatementKind::Assign {
                    target: callee_tmp,
                    value: Rvalue::Call {
                        site_id: SiteId::from_raw(0),
                        kind: CallKind::Direct {
                            callee_fqn: "fixtures.inline.inner".to_string(),
                        },
                        args: Vec::new(),
                        transport: call_transport(builtins.unit),
                    },
                },
            }],
            terminator: Terminator {
                span: crate::span::Span::new(0, 0),
                kind: TerminatorKind::Return {
                    value: Some(Operand::Local(callee_tmp)),
                },
                unwind: UnwindAction::NoUnwind,
            },
        });
        callee_body.start = callee_entry;

        let callee = FunDecl {
            span: crate::span::Span::new(0, 0),
            fqn: "fixtures.inline.wrapper".to_string(),
            name: "wrapper".to_string(),
            ty: builtins.unit,
            params: Vec::new(),
            return_ty: builtins.unit,
            body: Some(callee_body),
        };

        let expanded = expand_straight_line_call_without_callable_provenance(
            &mut caller_body,
            caller_target,
            crate::span::Span::new(0, 0),
            &callee,
            &[],
        )
        .expect("straight-line callee 应可被展开");
        let cloned_site = expanded
            .iter()
            .find_map(|stmt| match &stmt.kind {
                StatementKind::Assign {
                    value: Rvalue::Call { site_id, .. },
                    ..
                } => Some(*site_id),
                _ => None,
            })
            .expect("展开结果里应包含克隆后的 direct call");
        assert_eq!(cloned_site, SiteId::from_raw(1));
    }

    #[test]
    fn mir_site_id_effect_sensitive_bodies_are_not_inlineable() {
        let mut types = TypeStore::default();
        let builtins = types.intern_builtins();
        let mut body = Body::new_empty();
        let result_local = body.push_local(LocalDecl {
            span: crate::span::Span::new(0, 0),
            name: Some("tmp0".to_string()),
            ty: builtins.unit,
            source: LocalSourceKind::CompilerTemporary,
        });

        let entry = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: vec![Statement {
                span: crate::span::Span::new(0, 0),
                kind: StatementKind::Assign {
                    target: result_local,
                    value: Rvalue::PerformResult {
                        op_fqn: "scoop.core.Raise.raise".to_string(),
                        effect_ty: builtins.unit,
                    },
                },
            }],
            terminator: Terminator {
                span: crate::span::Span::new(0, 0),
                kind: TerminatorKind::Perform {
                    site_id: SiteId::from_raw(0),
                    op_fqn: "scoop.core.Raise.raise".to_string(),
                    metadata: PerformMetadata {
                        effect_ty: builtins.unit,
                        op_type_args: Vec::new(),
                        result_ty: builtins.unit,
                        payload_tuple_ty: None,
                        payload_component_tys: Vec::new(),
                        payload_transport: Vec::new(),
                        arg_mapping: Vec::new(),
                    },
                    args: Vec::new(),
                    resume_target: BasicBlockId(1),
                },
                unwind: UnwindAction::Propagate,
            },
        });
        let _resume = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: crate::span::Span::new(0, 0),
                kind: TerminatorKind::Return { value: None },
                unwind: UnwindAction::NoUnwind,
            },
        });
        body.start = entry;

        assert!(
            !body_is_inlineable_without_callable_provenance(&body),
            "带 Perform/cleanup contract 的 body 不应进入 inline clone 路径"
        );
    }

    fn raw_fun<'a>(materialized: &'a MaterializedMir, fqn: &str) -> &'a FunDecl {
        materialized
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == fqn => Some(fun),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing raw MIR fun `{fqn}`"))
    }

    fn fun_contains_direct_call(fun: &FunDecl, expected: &str) -> bool {
        let Some(body) = &fun.body else {
            return false;
        };
        body.blocks.iter().any(|block| {
            block.stmts.iter().any(|stmt| {
                let StatementKind::Assign {
                    value:
                        Rvalue::Call {
                            kind: CallKind::Direct { callee_fqn },
                            ..
                        },
                    ..
                } = &stmt.kind
                else {
                    return false;
                };
                callee_fqn == expected
            })
        })
    }

    fn fun_contains_fun_value_call(fun: &FunDecl) -> bool {
        let Some(body) = &fun.body else {
            return false;
        };
        body.blocks.iter().any(|block| {
            block.stmts.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign {
                        value: Rvalue::Call {
                            kind: CallKind::FunValue { .. },
                            ..
                        },
                        ..
                    }
                )
            })
        })
    }

    fn fun_contains_closure_call(fun: &FunDecl) -> bool {
        let Some(body) = &fun.body else {
            return false;
        };
        body.blocks.iter().any(|block| {
            block.stmts.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign {
                        value: Rvalue::Call {
                            kind: CallKind::Closure { .. },
                            ..
                        },
                        ..
                    }
                )
            })
        })
    }

    fn call_transport(result_ty: crate::ty::TypeId) -> CallTransportMetadata {
        CallTransportMetadata::plain_no_outward(result_ty, MirTransportKind::Unknown)
    }
}
