//! Materialized MIR 的 per-instance summary side tables。
//!
//! 这一层的目标是把“按单态实例组织的最小优化事实”稳定挂到 MIR 输出上，
//! 避免后续 devirtualization / inlining / escape analysis 继续在 backend 现场重建同类查询。

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::{
    Body, CallKind, File, FunDecl, InstanceKey, Item, Operand, Param, Rvalue, StatementKind,
    TerminatorKind,
};

/// 对单态实例暴露的最小 summary。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceSummary {
    pub body_known: bool,
    pub size_cost: u32,
    pub recursive_scc: bool,
    pub may_outward_effect: bool,
    pub may_allocate_closure: bool,
    pub param_use_summaries: Vec<ParamUseSummary>,
    pub result_provenance: ResultProvenance,
}

/// 参数使用摘要的最小四态分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamUseSummary {
    Unused,
    ValueOnly,
    DirectCallOnly,
    Escapes,
}

/// 返回值 provenance 的原子来源。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResultProvenanceSource {
    Param(usize),
    DirectFunction(String),
    KnownClosure(String),
    TopLevelValue(String),
    PerformResult(String),
}

/// 一个实例返回值的最小 provenance。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultProvenance {
    Unit,
    Param(usize),
    DirectFunction(String),
    KnownClosure(String),
    TopLevelValue(String),
    PerformResult(String),
    Join(Vec<ResultProvenanceSource>),
    Unknown,
}

/// `MaterializedMir` 上挂载的 per-instance summary side tables。
#[derive(Debug, Clone, Default)]
pub struct MaterializedMirSummaries {
    instance_summaries: HashMap<InstanceKey, InstanceSummary>,
}

impl MaterializedMirSummaries {
    pub fn get(&self, key: &InstanceKey) -> Option<&InstanceSummary> {
        self.instance_summaries.get(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&InstanceKey, &InstanceSummary)> {
        self.instance_summaries.iter()
    }

    pub fn len(&self) -> usize {
        self.instance_summaries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.instance_summaries.is_empty()
    }
}

/// 一个有 body 的 root instance 在 materialized file 中对应的根函数身份。
#[derive(Debug, Clone)]
pub(crate) struct InstanceRootSummaryInput {
    pub(crate) instance: InstanceKey,
    pub(crate) root_fqn: String,
}

/// declaration-only instance 的保守 summary 输入。
#[derive(Debug, Clone)]
pub(crate) struct DeclOnlySummaryInput {
    pub(crate) instance: InstanceKey,
    pub(crate) root_fqn: String,
    pub(crate) declared_fun_ty: TypeId,
    pub(crate) declared_return_ty: TypeId,
    pub(crate) param_count: usize,
}

#[derive(Debug, Clone)]
struct PendingSummary {
    body_known: bool,
    size_cost: u32,
    recursive_scc: bool,
    may_outward_effect: bool,
    may_allocate_closure: bool,
    param_use_summaries: Vec<ParamUseSummary>,
    result_provenance: ResultProvenance,
    direct_callees: BTreeSet<String>,
    base_outward_effect: bool,
    declared_effectful: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LocalProvenance {
    Empty,
    Known(BTreeSet<ResultProvenanceSource>),
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ReturnProvenanceState {
    Unseen,
    Unit,
    Known(BTreeSet<ResultProvenanceSource>),
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperandUsage {
    Value,
    DirectCallee,
    Escape,
}

#[derive(Debug)]
struct BodySummary {
    size_cost: u32,
    may_allocate_closure: bool,
    base_outward_effect: bool,
    param_use_summaries: Vec<ParamUseSummary>,
    result_provenance: ResultProvenance,
    direct_callees: BTreeSet<String>,
}

/// 根据 materialized MIR 输出与 declaration-only 输入，一次性建立 per-instance summary。
pub(crate) fn build_materialized_summary_table(
    file: &File,
    types: &TypeStore,
    root_instances: &[InstanceRootSummaryInput],
    decl_only_inputs: &[DeclOnlySummaryInput],
) -> MaterializedMirSummaries {
    let functions = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fun(fun) => Some(fun.clone()),
            Item::Todo { .. } => None,
        })
        .collect::<Vec<_>>();

    let mut pending = functions
        .iter()
        .map(|fun| {
            let summary = analyze_materialized_fun(fun, types);
            (fun.fqn.clone(), summary)
        })
        .collect::<HashMap<_, _>>();

    for decl in decl_only_inputs {
        pending.entry(decl.root_fqn.clone()).or_insert_with(|| {
            let result_provenance = if is_unit_ty(types, decl.declared_return_ty) {
                ResultProvenance::Unit
            } else {
                ResultProvenance::Unknown
            };
            PendingSummary {
                body_known: false,
                size_cost: 0,
                recursive_scc: false,
                may_outward_effect: function_ty_declared_effectful(types, decl.declared_fun_ty),
                may_allocate_closure: false,
                param_use_summaries: vec![ParamUseSummary::Escapes; decl.param_count],
                result_provenance,
                direct_callees: BTreeSet::new(),
                base_outward_effect: false,
                declared_effectful: function_ty_declared_effectful(types, decl.declared_fun_ty),
            }
        });
    }

    let recursive_nodes = compute_recursive_nodes(&pending);
    for (fqn, summary) in &mut pending {
        summary.recursive_scc = recursive_nodes.contains(fqn);
    }

    let may_outward_effect = solve_may_outward_effects(&pending);
    for (fqn, summary) in &mut pending {
        summary.may_outward_effect = *may_outward_effect
            .get(fqn)
            .expect("every pending summary should have an outward-effect entry");
    }

    let mut instance_summaries = HashMap::new();
    for root in root_instances {
        if let Some(summary) = pending.get(&root.root_fqn) {
            instance_summaries.insert(root.instance.clone(), summary_to_instance(summary));
        }
    }
    for decl in decl_only_inputs {
        if let Some(summary) = pending.get(&decl.root_fqn) {
            instance_summaries.insert(decl.instance.clone(), summary_to_instance(summary));
        }
    }

    MaterializedMirSummaries { instance_summaries }
}

fn summary_to_instance(summary: &PendingSummary) -> InstanceSummary {
    InstanceSummary {
        body_known: summary.body_known,
        size_cost: summary.size_cost,
        recursive_scc: summary.recursive_scc,
        may_outward_effect: summary.may_outward_effect,
        may_allocate_closure: summary.may_allocate_closure,
        param_use_summaries: summary.param_use_summaries.clone(),
        result_provenance: summary.result_provenance.clone(),
    }
}

fn analyze_materialized_fun(fun: &FunDecl, types: &TypeStore) -> PendingSummary {
    let declared_effectful = function_ty_declared_effectful(types, fun.ty);
    let Some(body) = &fun.body else {
        let result_provenance = if is_unit_ty(types, fun.return_ty) {
            ResultProvenance::Unit
        } else {
            ResultProvenance::Unknown
        };
        return PendingSummary {
            body_known: false,
            size_cost: 0,
            recursive_scc: false,
            may_outward_effect: declared_effectful,
            may_allocate_closure: false,
            param_use_summaries: vec![ParamUseSummary::Escapes; fun.params.len()],
            result_provenance,
            direct_callees: BTreeSet::new(),
            base_outward_effect: false,
            declared_effectful,
        };
    };

    let body_summary = analyze_body(body, &fun.params, types);
    PendingSummary {
        body_known: true,
        size_cost: body_summary.size_cost,
        recursive_scc: false,
        may_outward_effect: body_summary.base_outward_effect,
        may_allocate_closure: body_summary.may_allocate_closure,
        param_use_summaries: body_summary.param_use_summaries,
        result_provenance: body_summary.result_provenance,
        direct_callees: body_summary.direct_callees,
        base_outward_effect: body_summary.base_outward_effect,
        declared_effectful,
    }
}

fn analyze_body(body: &Body, params: &[Param], types: &TypeStore) -> BodySummary {
    let mut entry_states = vec![None; body.blocks.len()];
    let mut start_state = vec![LocalProvenance::Empty; body.locals.len()];
    for (index, param) in params.iter().enumerate() {
        let local = param.local.as_u32() as usize;
        start_state[local] = known_source(ResultProvenanceSource::Param(index));
    }

    let start = body.start.as_u32() as usize;
    entry_states[start] = Some(start_state);

    let mut worklist = VecDeque::from([body.start]);
    let mut param_use_summaries = vec![ParamUseSummary::Unused; params.len()];
    let mut return_provenance = ReturnProvenanceState::Unseen;
    let mut direct_callees = BTreeSet::new();
    let mut may_allocate_closure = false;
    let mut base_outward_effect = false;

    while let Some(bb) = worklist.pop_front() {
        let bb_index = bb.as_u32() as usize;
        let Some(mut state) = entry_states[bb_index].clone() else {
            continue;
        };
        let block = &body.blocks[bb_index];

        for stmt in &block.stmts {
            match &stmt.kind {
                StatementKind::Nop => {}
                StatementKind::Todo(_) => base_outward_effect = true,
                StatementKind::Assign { target, value } => {
                    observe_rvalue(
                        value,
                        &state,
                        &mut param_use_summaries,
                        &mut direct_callees,
                        &mut may_allocate_closure,
                        &mut base_outward_effect,
                    );
                    let target_index = target.as_u32() as usize;
                    let target_ty = body.locals[target_index].ty;
                    state[target_index] = rvalue_provenance(value, target_ty, &state, types);
                }
            }
        }

        observe_terminator(
            &block.terminator.kind,
            &state,
            &mut param_use_summaries,
            &mut return_provenance,
            &mut base_outward_effect,
        );

        block.terminator.for_each_successor(|succ| {
            let succ_index = succ.as_u32() as usize;
            let changed = match &mut entry_states[succ_index] {
                Some(existing) => join_state(existing, &state),
                None => {
                    entry_states[succ_index] = Some(state.clone());
                    true
                }
            };
            if changed {
                worklist.push_back(succ);
            }
        });
    }

    BodySummary {
        size_cost: estimate_body_size(body),
        may_allocate_closure,
        base_outward_effect,
        param_use_summaries,
        result_provenance: finalize_return_provenance(return_provenance),
        direct_callees,
    }
}

fn observe_rvalue(
    value: &Rvalue,
    state: &[LocalProvenance],
    param_use_summaries: &mut [ParamUseSummary],
    direct_callees: &mut BTreeSet<String>,
    may_allocate_closure: &mut bool,
    base_outward_effect: &mut bool,
) {
    match value {
        Rvalue::Use(operand) => {
            observe_operand(operand, OperandUsage::Value, state, param_use_summaries)
        }
        Rvalue::TopLevelRef(_) | Rvalue::UnresolvedName { .. } | Rvalue::PerformResult { .. } => {}
        Rvalue::Unary { operand, .. }
        | Rvalue::TypeCheck { value: operand, .. }
        | Rvalue::Cast { value: operand, .. }
        | Rvalue::TupleGet { tuple: operand, .. }
        | Rvalue::CaptureBoxGet {
            box_operand: operand,
        }
        | Rvalue::PatternMatch {
            subject: operand, ..
        }
        | Rvalue::PatternExtract {
            subject: operand, ..
        } => {
            observe_operand(operand, OperandUsage::Value, state, param_use_summaries);
        }
        Rvalue::Binary { lhs, rhs, .. } => {
            observe_operand(lhs, OperandUsage::Value, state, param_use_summaries);
            observe_operand(rhs, OperandUsage::Value, state, param_use_summaries);
        }
        Rvalue::MemberAccess { receiver, .. } => {
            observe_operand(receiver, OperandUsage::Value, state, param_use_summaries);
        }
        Rvalue::Call { kind, args } => {
            match kind {
                CallKind::Direct { callee_fqn } => {
                    direct_callees.insert(callee_fqn.clone());
                }
                CallKind::Closure { callee, fn_ptr } => {
                    direct_callees.insert(fn_ptr.clone());
                    observe_operand(
                        callee,
                        OperandUsage::DirectCallee,
                        state,
                        param_use_summaries,
                    );
                }
                CallKind::FunValue { callee } => {
                    observe_operand(
                        callee,
                        OperandUsage::DirectCallee,
                        state,
                        param_use_summaries,
                    );
                    *base_outward_effect = true;
                }
                CallKind::Virtual { receiver, .. } | CallKind::Interface { receiver, .. } => {
                    observe_operand(receiver, OperandUsage::Escape, state, param_use_summaries);
                    *base_outward_effect = true;
                }
                CallKind::Resume { continuation, .. } => {
                    observe_operand(
                        continuation,
                        OperandUsage::Escape,
                        state,
                        param_use_summaries,
                    );
                    *base_outward_effect = true;
                }
            }
            for arg in args {
                observe_operand(&arg.value, OperandUsage::Escape, state, param_use_summaries);
            }
        }
        Rvalue::MakeTuple { elements } => {
            for element in elements {
                observe_operand(element, OperandUsage::Escape, state, param_use_summaries);
            }
        }
        Rvalue::CaptureBoxNew { value } => {
            *may_allocate_closure = true;
            observe_operand(value, OperandUsage::Escape, state, param_use_summaries);
        }
        Rvalue::CaptureBoxSet { box_operand, value } => {
            observe_operand(box_operand, OperandUsage::Value, state, param_use_summaries);
            observe_operand(value, OperandUsage::Escape, state, param_use_summaries);
        }
        Rvalue::MakeClosure { env, .. } => {
            *may_allocate_closure = true;
            observe_operand(env, OperandUsage::Escape, state, param_use_summaries);
        }
        Rvalue::Todo(_) => *base_outward_effect = true,
    }
}

fn observe_terminator(
    terminator: &TerminatorKind,
    state: &[LocalProvenance],
    param_use_summaries: &mut [ParamUseSummary],
    return_provenance: &mut ReturnProvenanceState,
    base_outward_effect: &mut bool,
) {
    match terminator {
        TerminatorKind::Return { value } => match value {
            Some(operand) => {
                observe_operand(operand, OperandUsage::Escape, state, param_use_summaries);
                join_return_provenance(return_provenance, operand_provenance(operand, state));
            }
            None => join_return_provenance_unit(return_provenance),
        },
        TerminatorKind::ResumeUnwind
        | TerminatorKind::Goto { .. }
        | TerminatorKind::Unreachable => {}
        TerminatorKind::CondBr { cond, .. } => {
            observe_operand(cond, OperandUsage::Value, state, param_use_summaries);
        }
        TerminatorKind::Perform { args, .. } => {
            for arg in args {
                observe_operand(&arg.value, OperandUsage::Escape, state, param_use_summaries);
            }
            *base_outward_effect = true;
        }
        TerminatorKind::Handle { .. } | TerminatorKind::Todo(_) => *base_outward_effect = true,
    }
}

fn observe_operand(
    operand: &Operand,
    usage: OperandUsage,
    state: &[LocalProvenance],
    param_use_summaries: &mut [ParamUseSummary],
) {
    let provenance = operand_provenance(operand, state);
    let LocalProvenance::Known(sources) = provenance else {
        return;
    };

    let exact_param = (sources.len() == 1)
        .then(|| sources.iter().next())
        .flatten()
        .and_then(|source| match source {
            ResultProvenanceSource::Param(index) => Some(*index),
            ResultProvenanceSource::DirectFunction(_)
            | ResultProvenanceSource::KnownClosure(_)
            | ResultProvenanceSource::TopLevelValue(_)
            | ResultProvenanceSource::PerformResult(_) => None,
        });

    for source in sources {
        let ResultProvenanceSource::Param(index) = source else {
            continue;
        };
        let next = match usage {
            OperandUsage::Value => ParamUseSummary::ValueOnly,
            OperandUsage::DirectCallee => {
                if exact_param == Some(index) {
                    ParamUseSummary::DirectCallOnly
                } else {
                    ParamUseSummary::Escapes
                }
            }
            OperandUsage::Escape => ParamUseSummary::Escapes,
        };
        let slot = &mut param_use_summaries[index];
        *slot = join_param_use(*slot, next);
    }
}

fn operand_provenance(operand: &Operand, state: &[LocalProvenance]) -> LocalProvenance {
    match operand {
        Operand::Local(local) => state[local.as_u32() as usize].clone(),
        Operand::Const(_) => LocalProvenance::Empty,
    }
}

fn rvalue_provenance(
    value: &Rvalue,
    target_ty: TypeId,
    state: &[LocalProvenance],
    types: &TypeStore,
) -> LocalProvenance {
    match value {
        Rvalue::Use(operand) => operand_provenance(operand, state),
        Rvalue::TopLevelRef(top) => {
            if is_function_ty(types, target_ty) {
                known_source(ResultProvenanceSource::DirectFunction(top.fqn.clone()))
            } else {
                known_source(ResultProvenanceSource::TopLevelValue(top.fqn.clone()))
            }
        }
        Rvalue::MemberAccess { member, .. } => match member.resolved.as_ref() {
            Some(super::MemberTarget::Fun { fqn })
            | Some(super::MemberTarget::ExtensionFun { fqn }) => {
                known_source(ResultProvenanceSource::DirectFunction(fqn.clone()))
            }
            Some(super::MemberTarget::Value { fqn })
            | Some(super::MemberTarget::ExtensionValue { fqn }) => {
                known_source(ResultProvenanceSource::TopLevelValue(fqn.clone()))
            }
            None => LocalProvenance::Unknown,
        },
        Rvalue::MakeClosure { fn_ptr, .. } => {
            known_source(ResultProvenanceSource::KnownClosure(fn_ptr.clone()))
        }
        Rvalue::PerformResult { op_fqn, .. } => {
            known_source(ResultProvenanceSource::PerformResult(op_fqn.clone()))
        }
        Rvalue::UnresolvedName { .. } | Rvalue::Todo(_) => LocalProvenance::Unknown,
        Rvalue::Unary { .. }
        | Rvalue::Binary { .. }
        | Rvalue::TypeCheck { .. }
        | Rvalue::Cast { .. }
        | Rvalue::Call { .. }
        | Rvalue::MakeTuple { .. }
        | Rvalue::TupleGet { .. }
        | Rvalue::CaptureBoxNew { .. }
        | Rvalue::CaptureBoxGet { .. }
        | Rvalue::CaptureBoxSet { .. }
        | Rvalue::PatternMatch { .. }
        | Rvalue::PatternExtract { .. } => {
            let _ = (target_ty, types);
            LocalProvenance::Empty
        }
    }
}

fn join_state(existing: &mut [LocalProvenance], incoming: &[LocalProvenance]) -> bool {
    let mut changed = false;
    for (slot, next) in existing.iter_mut().zip(incoming.iter()) {
        let joined = join_local_provenance(slot.clone(), next.clone());
        if joined != *slot {
            *slot = joined;
            changed = true;
        }
    }
    changed
}

fn join_local_provenance(left: LocalProvenance, right: LocalProvenance) -> LocalProvenance {
    match (left, right) {
        (LocalProvenance::Unknown, _) | (_, LocalProvenance::Unknown) => LocalProvenance::Unknown,
        (LocalProvenance::Empty, LocalProvenance::Empty) => LocalProvenance::Empty,
        (LocalProvenance::Empty, LocalProvenance::Known(_))
        | (LocalProvenance::Known(_), LocalProvenance::Empty) => LocalProvenance::Unknown,
        (LocalProvenance::Known(mut left), LocalProvenance::Known(right)) => {
            left.extend(right);
            LocalProvenance::Known(left)
        }
    }
}

fn known_source(source: ResultProvenanceSource) -> LocalProvenance {
    let mut sources = BTreeSet::new();
    sources.insert(source);
    LocalProvenance::Known(sources)
}

fn join_return_provenance(state: &mut ReturnProvenanceState, next: LocalProvenance) {
    let next = match next {
        LocalProvenance::Empty => ReturnProvenanceState::Unknown,
        LocalProvenance::Known(sources) => ReturnProvenanceState::Known(sources),
        LocalProvenance::Unknown => ReturnProvenanceState::Unknown,
    };
    *state = join_return_state(state.clone(), next);
}

fn join_return_provenance_unit(state: &mut ReturnProvenanceState) {
    *state = join_return_state(state.clone(), ReturnProvenanceState::Unit);
}

fn join_return_state(
    left: ReturnProvenanceState,
    right: ReturnProvenanceState,
) -> ReturnProvenanceState {
    match (left, right) {
        (ReturnProvenanceState::Unseen, other) | (other, ReturnProvenanceState::Unseen) => other,
        (ReturnProvenanceState::Unknown, _) | (_, ReturnProvenanceState::Unknown) => {
            ReturnProvenanceState::Unknown
        }
        (ReturnProvenanceState::Unit, ReturnProvenanceState::Unit) => ReturnProvenanceState::Unit,
        (ReturnProvenanceState::Unit, ReturnProvenanceState::Known(_))
        | (ReturnProvenanceState::Known(_), ReturnProvenanceState::Unit) => {
            ReturnProvenanceState::Unknown
        }
        (ReturnProvenanceState::Known(mut left), ReturnProvenanceState::Known(right)) => {
            left.extend(right);
            ReturnProvenanceState::Known(left)
        }
    }
}

fn finalize_return_provenance(state: ReturnProvenanceState) -> ResultProvenance {
    match state {
        ReturnProvenanceState::Unseen | ReturnProvenanceState::Unknown => ResultProvenance::Unknown,
        ReturnProvenanceState::Unit => ResultProvenance::Unit,
        ReturnProvenanceState::Known(sources) => {
            if sources.len() == 1 {
                let source = sources
                    .iter()
                    .next()
                    .cloned()
                    .expect("single source expected");
                match source {
                    ResultProvenanceSource::Param(index) => ResultProvenance::Param(index),
                    ResultProvenanceSource::DirectFunction(fqn) => {
                        ResultProvenance::DirectFunction(fqn)
                    }
                    ResultProvenanceSource::KnownClosure(fn_ptr) => {
                        ResultProvenance::KnownClosure(fn_ptr)
                    }
                    ResultProvenanceSource::TopLevelValue(fqn) => {
                        ResultProvenance::TopLevelValue(fqn)
                    }
                    ResultProvenanceSource::PerformResult(op_fqn) => {
                        ResultProvenance::PerformResult(op_fqn)
                    }
                }
            } else {
                ResultProvenance::Join(sources.into_iter().collect())
            }
        }
    }
}

fn join_param_use(left: ParamUseSummary, right: ParamUseSummary) -> ParamUseSummary {
    match (left, right) {
        (ParamUseSummary::Escapes, _) | (_, ParamUseSummary::Escapes) => ParamUseSummary::Escapes,
        (ParamUseSummary::Unused, other) | (other, ParamUseSummary::Unused) => other,
        (ParamUseSummary::ValueOnly, ParamUseSummary::ValueOnly) => ParamUseSummary::ValueOnly,
        (ParamUseSummary::DirectCallOnly, ParamUseSummary::DirectCallOnly) => {
            ParamUseSummary::DirectCallOnly
        }
        (ParamUseSummary::ValueOnly, ParamUseSummary::DirectCallOnly)
        | (ParamUseSummary::DirectCallOnly, ParamUseSummary::ValueOnly) => ParamUseSummary::Escapes,
    }
}

fn estimate_body_size(body: &Body) -> u32 {
    let mut cost = body.blocks.len() as u32;
    for block in &body.blocks {
        for stmt in &block.stmts {
            cost += statement_cost(&stmt.kind);
        }
        cost += terminator_cost(&block.terminator.kind);
    }
    cost
}

fn statement_cost(kind: &StatementKind) -> u32 {
    match kind {
        StatementKind::Nop => 1,
        StatementKind::Todo(_) => 3,
        StatementKind::Assign { value, .. } => 1 + rvalue_cost(value),
    }
}

fn rvalue_cost(value: &Rvalue) -> u32 {
    match value {
        Rvalue::Use(_)
        | Rvalue::TopLevelRef(_)
        | Rvalue::UnresolvedName { .. }
        | Rvalue::PerformResult { .. } => 1,
        Rvalue::Unary { .. }
        | Rvalue::TypeCheck { .. }
        | Rvalue::Cast { .. }
        | Rvalue::TupleGet { .. }
        | Rvalue::CaptureBoxGet { .. }
        | Rvalue::PatternMatch { .. }
        | Rvalue::PatternExtract { .. } => 2,
        Rvalue::Binary { .. } | Rvalue::MemberAccess { .. } => 3,
        Rvalue::Call { args, .. } => 4 + args.len() as u32,
        Rvalue::MakeTuple { elements } => 2 + elements.len() as u32,
        Rvalue::CaptureBoxNew { .. } | Rvalue::CaptureBoxSet { .. } => 4,
        Rvalue::MakeClosure { .. } => 5,
        Rvalue::Todo(_) => 3,
    }
}

fn terminator_cost(kind: &TerminatorKind) -> u32 {
    match kind {
        TerminatorKind::Return { .. }
        | TerminatorKind::ResumeUnwind
        | TerminatorKind::Goto { .. }
        | TerminatorKind::Unreachable => 1,
        TerminatorKind::CondBr { .. } => 2,
        TerminatorKind::Perform { args, .. } => 4 + args.len() as u32,
        TerminatorKind::Handle { arms, .. } => 5 + arms.len() as u32,
        TerminatorKind::Todo(_) => 3,
    }
}

fn compute_recursive_nodes(pending: &HashMap<String, PendingSummary>) -> HashSet<String> {
    struct Tarjan<'a> {
        graph: &'a HashMap<String, PendingSummary>,
        next_index: usize,
        indices: HashMap<String, usize>,
        lowlinks: HashMap<String, usize>,
        stack: Vec<String>,
        on_stack: HashSet<String>,
        recursive: HashSet<String>,
    }

    impl<'a> Tarjan<'a> {
        fn strongconnect(&mut self, node: String) {
            let index = self.next_index;
            self.next_index += 1;
            self.indices.insert(node.clone(), index);
            self.lowlinks.insert(node.clone(), index);
            self.stack.push(node.clone());
            self.on_stack.insert(node.clone());

            if let Some(summary) = self.graph.get(&node) {
                for succ in &summary.direct_callees {
                    if !self.graph.contains_key(succ) {
                        continue;
                    }
                    if !self.indices.contains_key(succ) {
                        self.strongconnect(succ.clone());
                        let lowlink = self.lowlinks[&node].min(self.lowlinks[succ]);
                        self.lowlinks.insert(node.clone(), lowlink);
                    } else if self.on_stack.contains(succ) {
                        let lowlink = self.lowlinks[&node].min(self.indices[succ]);
                        self.lowlinks.insert(node.clone(), lowlink);
                    }
                }
            }

            if self.lowlinks[&node] != self.indices[&node] {
                return;
            }

            let mut component = Vec::new();
            while let Some(top) = self.stack.pop() {
                self.on_stack.remove(&top);
                let is_done = top == node;
                component.push(top);
                if is_done {
                    break;
                }
            }

            let self_recursive = component.len() == 1
                && self
                    .graph
                    .get(&component[0])
                    .is_some_and(|summary| summary.direct_callees.contains(&component[0]));
            if component.len() > 1 || self_recursive {
                self.recursive.extend(component);
            }
        }
    }

    let mut tarjan = Tarjan {
        graph: pending,
        next_index: 0,
        indices: HashMap::new(),
        lowlinks: HashMap::new(),
        stack: Vec::new(),
        on_stack: HashSet::new(),
        recursive: HashSet::new(),
    };

    for node in pending.keys() {
        if !tarjan.indices.contains_key(node) {
            tarjan.strongconnect(node.clone());
        }
    }

    tarjan.recursive
}

fn solve_may_outward_effects(pending: &HashMap<String, PendingSummary>) -> HashMap<String, bool> {
    let mut effects = pending
        .iter()
        .map(|(fqn, summary)| {
            (
                fqn.clone(),
                if summary.body_known {
                    summary.base_outward_effect
                } else {
                    summary.declared_effectful
                },
            )
        })
        .collect::<HashMap<_, _>>();

    loop {
        let mut changed = false;
        for (fqn, summary) in pending {
            if !summary.body_known || effects.get(fqn).copied().unwrap_or(false) {
                continue;
            }
            let next = summary.base_outward_effect
                || summary
                    .direct_callees
                    .iter()
                    .any(|callee| effects.get(callee).copied().unwrap_or(true));
            if next {
                effects.insert(fqn.clone(), true);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    effects
}

fn function_ty_declared_effectful(types: &TypeStore, fun_ty: TypeId) -> bool {
    let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = types.kind(fun_ty) else {
        return true;
    };
    !fun_ty.effects.is_pure() || !fun_ty.effects_closed
}

fn is_function_ty(types: &TypeStore, ty: TypeId) -> bool {
    matches!(types.kind(ty), TypeKind::Ref(RefTypeKind::Function(_)))
}

fn is_unit_ty(types: &TypeStore, ty: TypeId) -> bool {
    matches!(types.kind(ty), TypeKind::Value(ValueTypeKind::Unit))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use crate::source::SourceFile;
    use crate::ty::TypeStore;

    fn summary_for<'a>(
        materialized: &'a super::super::MaterializedMir,
        template_fqn: &str,
        type_args: &[&str],
    ) -> &'a InstanceSummary {
        let expected = type_args
            .iter()
            .map(|arg| (*arg).to_string())
            .collect::<Vec<_>>();
        let key = materialized
            .instance_keys
            .iter()
            .find(|key| {
                key.template.fqn == template_fqn
                    && key
                        .type_args
                        .iter()
                        .map(|&ty| materialized.types.display(ty).to_string())
                        .collect::<Vec<_>>()
                        == expected
            })
            .unwrap_or_else(|| {
                panic!(
                    "missing instance for {template_fqn}::<{}>",
                    type_args.join(", ")
                )
            });
        materialized
            .summaries
            .get(key)
            .expect("summary should exist for every instance key")
    }

    #[test]
    fn summaries_are_keyed_by_instance_identity() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_summary_identity.scoop",
            r#"
package fixtures.mirsummary

fun <T> id(x: T): T {
    return x
}

fun entry(): Unit {
    id(1)
    id(true)
}
"#,
        );

        let materialized = super::super::materialize_for_dump(&sess, &source).unwrap();
        let summaries = materialized
            .summaries
            .iter()
            .filter(|(key, _)| key.template.fqn == "fixtures.mirsummary.id")
            .collect::<Vec<_>>();
        assert_eq!(
            summaries.len(),
            2,
            "不同 type args 的实例应各自拥有 summary"
        );

        let int_summary = summary_for(&materialized, "fixtures.mirsummary.id", &["Int"]);
        let bool_summary = summary_for(&materialized, "fixtures.mirsummary.id", &["Bool"]);
        assert!(int_summary.body_known);
        assert!(bool_summary.body_known);
        assert_eq!(int_summary.result_provenance, ResultProvenance::Param(0));
        assert_eq!(bool_summary.result_provenance, ResultProvenance::Param(0));
        assert_eq!(
            int_summary.param_use_summaries,
            vec![ParamUseSummary::Escapes]
        );
        assert_eq!(
            bool_summary.param_use_summaries,
            vec![ParamUseSummary::Escapes]
        );
    }

    #[test]
    fn summary_marks_function_param_direct_call_only() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_summary_direct_call_only.scoop",
            r#"
package fixtures.mirsummary

fun <T> callOnce(f: () -> T / Pure!): T {
    return f()
}

fun entry(): Int {
    return callOnce<Int>({ 1 })
}
"#,
        );

        let materialized = super::super::materialize_for_dump(&sess, &source).unwrap();
        let summary = summary_for(&materialized, "fixtures.mirsummary.callOnce", &["Int"]);
        assert_eq!(
            summary.param_use_summaries,
            vec![ParamUseSummary::DirectCallOnly]
        );
        assert_eq!(summary.result_provenance, ResultProvenance::Unknown);
    }

    #[test]
    fn summary_marks_returned_function_param_as_escape() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_summary_return_escape.scoop",
            r#"
package fixtures.mirsummary

fun <T> keep(f: () -> T / Pure!): () -> T / Pure! {
    return f
}

fun entry(): Int {
    val f = keep<Int>({ 1 })
    return f()
}
"#,
        );

        let materialized = super::super::materialize_for_dump(&sess, &source).unwrap();
        let summary = summary_for(&materialized, "fixtures.mirsummary.keep", &["Int"]);
        assert_eq!(summary.param_use_summaries, vec![ParamUseSummary::Escapes]);
        assert_eq!(summary.result_provenance, ResultProvenance::Param(0));
    }

    #[test]
    fn summary_tracks_known_closure_result_and_allocation() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_summary_known_closure.scoop",
            r#"
package fixtures.mirsummary

fun <T> makeThunk(x: T): () -> T / Pure! {
    return { x }
}

fun entry(): Int {
    val f = makeThunk(1)
    return f()
}
"#,
        );

        let materialized = super::super::materialize_for_dump(&sess, &source).unwrap();
        let summary = summary_for(&materialized, "fixtures.mirsummary.makeThunk", &["Int"]);
        assert!(summary.may_allocate_closure);
        assert!(matches!(
            summary.result_provenance,
            ResultProvenance::KnownClosure(_)
        ));
        assert_eq!(summary.param_use_summaries, vec![ParamUseSummary::Escapes]);
    }

    #[test]
    fn declaration_only_instances_stay_body_unknown_and_conservative() {
        let mut types = TypeStore::default();
        let builtins = types.intern_builtins();
        let fun_ty = types.ty_function(
            None,
            vec![builtins.int],
            builtins.int,
            crate::ty::EffectRow::pure(),
            true,
        );
        let return_ty = builtins.int;
        let template = InstanceKey {
            template: super::super::TemplateKey {
                fqn: "fixtures.mirsummary.externId".to_string(),
                source_path: "<mem>/decl_only.scoop".into(),
                decl_span: crate::span::Span::new(0, 0),
            },
            type_args: vec![builtins.int],
            eff_args: Vec::new(),
        };

        let summaries = build_materialized_summary_table(
            &File { items: Vec::new() },
            &types,
            &[],
            &[DeclOnlySummaryInput {
                instance: template.clone(),
                root_fqn: "fixtures.mirsummary.externId::<Int>".to_string(),
                declared_fun_ty: fun_ty,
                declared_return_ty: return_ty,
                param_count: 1,
            }],
        );

        let summary = summaries
            .get(&template)
            .expect("decl-only summary should exist");
        assert!(!summary.body_known);
        assert_eq!(summary.size_cost, 0);
        assert_eq!(summary.param_use_summaries, vec![ParamUseSummary::Escapes]);
        assert_eq!(summary.result_provenance, ResultProvenance::Unknown);
    }
}
