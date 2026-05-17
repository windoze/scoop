//! MIR-level closure / continuation escape analysis.
//!
//! This pass is intentionally conservative. It only publishes non-escaping facts when a closure
//! value is created in MIR and all observed uses are local `Closure` calls, or when a continuation
//! local is only used as the receiver of `Continuation.resume(...)`. Any value returned, passed to
//! another callable, stored into an aggregate/capture box, or seen through unmodelled MIR becomes
//! escaping or unknown. Later simplification and effect planning can consume these facts without
//! asking LLVM codegen to rediscover them from backend state.

use std::collections::{BTreeMap, HashMap};

use crate::ty::{RefTypeKind, TypeKind, TypeStore};

use super::{
    Body, CallKind, FunDecl, LocalId, MaterializedMir, Operand, Rvalue, StatementKind,
    TerminatorKind,
};

/// Conservative escape state for a MIR-created closure or continuation local.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeStatus {
    /// All modelled uses stay local to the callable body.
    NonEscaping,
    /// The value is observed crossing the callable boundary or being stored into another object.
    Escapes,
    /// The current MIR shape is not precise enough to prove either local-only use or escape.
    Unknown,
}

impl EscapeStatus {
    pub fn is_non_escaping(self) -> bool {
        matches!(self, EscapeStatus::NonEscaping)
    }
}

/// Escape fact for one MIR `MakeClosure` result local.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureEscapeFact {
    pub local: LocalId,
    pub local_name: Option<String>,
    pub fn_ptr: String,
    pub status: EscapeStatus,
    pub direct_call_count: usize,
}

/// Escape fact for one continuation local.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuationEscapeFact {
    pub local: LocalId,
    pub local_name: Option<String>,
    pub status: EscapeStatus,
    pub resume_call_count: usize,
    pub resume_call_spans: Vec<crate::span::Span>,
}

/// Escape facts for one pass-visible callable body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallableEscapeFacts {
    closures_by_local: BTreeMap<LocalId, ClosureEscapeFact>,
    continuations_by_local: BTreeMap<LocalId, ContinuationEscapeFact>,
}

impl CallableEscapeFacts {
    pub fn is_empty(&self) -> bool {
        self.closures_by_local.is_empty() && self.continuations_by_local.is_empty()
    }

    pub fn closure(&self, local: LocalId) -> Option<&ClosureEscapeFact> {
        self.closures_by_local.get(&local)
    }

    pub fn closures(&self) -> impl Iterator<Item = &ClosureEscapeFact> {
        self.closures_by_local.values()
    }

    pub fn continuation(&self, local: LocalId) -> Option<&ContinuationEscapeFact> {
        self.continuations_by_local.get(&local)
    }

    pub fn continuations(&self) -> impl Iterator<Item = &ContinuationEscapeFact> {
        self.continuations_by_local.values()
    }
}

/// Escape facts published by the current materialized MIR pass artifacts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MaterializedEscapeFacts {
    by_callable_fqn: BTreeMap<String, CallableEscapeFacts>,
}

impl MaterializedEscapeFacts {
    pub fn is_empty(&self) -> bool {
        self.by_callable_fqn.is_empty()
    }

    pub fn callable(&self, fqn: &str) -> Option<&CallableEscapeFacts> {
        self.by_callable_fqn.get(fqn)
    }

    pub fn callables(&self) -> impl Iterator<Item = (&str, &CallableEscapeFacts)> {
        self.by_callable_fqn
            .iter()
            .map(|(fqn, facts)| (fqn.as_str(), facts))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EscapeOrigin {
    Closure(LocalId),
    Continuation(LocalId),
}

enum OperandUse<'a> {
    LocalClosureCall { fn_ptr: &'a str },
    LocalContinuationResume { span: crate::span::Span },
    Escaping,
}

/// Run escape analysis over the current pass-visible MIR bodies and publish the resulting side
/// table into `MaterializedMirPassArtifacts`.
pub(crate) fn run_escape_analysis(materialized: &mut MaterializedMir) {
    let callables = pass_visible_callables(materialized);
    let mut facts = MaterializedEscapeFacts::default();
    for fun in callables {
        let callable_facts = analyze_callable_escape_facts(&fun, &materialized.types);
        if !callable_facts.is_empty() {
            facts
                .by_callable_fqn
                .insert(fun.fqn.clone(), callable_facts);
        }
    }
    materialized.pass_artifacts_mut().set_escape_facts(facts);
}

fn pass_visible_callables(materialized: &MaterializedMir) -> Vec<FunDecl> {
    let mut by_fqn = BTreeMap::new();
    let pass_view = materialized.pass_view();

    for family in pass_view.instances() {
        for fun in family.callable_bodies() {
            by_fqn.entry(fun.fqn.clone()).or_insert_with(|| fun.clone());
        }
    }

    for raw_fun in materialized.caller_side_pass_candidate_bodies() {
        if pass_view.owner_of_callable(&raw_fun.fqn).is_some() {
            continue;
        }
        let fun = if pass_view.callable_body_is_overridden(&raw_fun.fqn) {
            pass_view
                .callable(&raw_fun.fqn)
                .cloned()
                .unwrap_or_else(|| raw_fun.clone())
        } else {
            raw_fun.clone()
        };
        by_fqn.entry(fun.fqn.clone()).or_insert(fun);
    }

    by_fqn.into_values().collect()
}

fn analyze_callable_escape_facts(fun: &FunDecl, types: &TypeStore) -> CallableEscapeFacts {
    let Some(body) = fun.body.as_ref() else {
        return CallableEscapeFacts::default();
    };

    let mut facts = collect_initial_facts(body, types);
    if facts.is_empty() {
        return facts;
    }

    let aliases = collect_escape_origin_aliases(body, &facts);
    let mut saw_unknown_mir = false;
    for block in &body.blocks {
        for stmt in &block.stmts {
            analyze_statement_uses(stmt, &aliases, &mut facts, &mut saw_unknown_mir);
        }
        analyze_terminator_uses(
            &block.terminator.kind,
            &aliases,
            &mut facts,
            &mut saw_unknown_mir,
        );
    }

    if saw_unknown_mir {
        mark_unescaped_facts_unknown(&mut facts);
    }
    facts
}

fn collect_initial_facts(body: &Body, types: &TypeStore) -> CallableEscapeFacts {
    let mut facts = CallableEscapeFacts::default();

    for (idx, local) in body.locals.iter().enumerate() {
        let local_id = LocalId::from_raw(idx as u32);
        if is_continuation_type(types, local.ty) {
            facts.continuations_by_local.insert(
                local_id,
                ContinuationEscapeFact {
                    local: local_id,
                    local_name: local.name.clone(),
                    status: EscapeStatus::NonEscaping,
                    resume_call_count: 0,
                    resume_call_spans: Vec::new(),
                },
            );
        }
    }

    for block in &body.blocks {
        for stmt in &block.stmts {
            if let StatementKind::Assign {
                target,
                value: Rvalue::MakeClosure { fn_ptr, .. },
            } = &stmt.kind
            {
                facts.closures_by_local.insert(
                    *target,
                    ClosureEscapeFact {
                        local: *target,
                        local_name: local_name(body, *target),
                        fn_ptr: fn_ptr.clone(),
                        status: EscapeStatus::NonEscaping,
                        direct_call_count: 0,
                    },
                );
            }
            collect_resume_continuation_candidate_from_statement(body, stmt, &mut facts);
        }
    }

    facts
}

fn collect_resume_continuation_candidate_from_statement(
    body: &Body,
    stmt: &super::Statement,
    facts: &mut CallableEscapeFacts,
) {
    let StatementKind::Assign {
        value: Rvalue::Call { kind, .. },
        ..
    } = &stmt.kind
    else {
        return;
    };
    collect_resume_continuation_candidate_from_call_kind(body, kind, facts);
}

fn collect_resume_continuation_candidate_from_call_kind(
    body: &Body,
    kind: &CallKind,
    facts: &mut CallableEscapeFacts,
) {
    let CallKind::Resume { continuation, .. } = kind else {
        return;
    };
    let Operand::Local(local) = continuation else {
        return;
    };
    facts
        .continuations_by_local
        .entry(*local)
        .or_insert_with(|| ContinuationEscapeFact {
            local: *local,
            local_name: local_name(body, *local),
            status: EscapeStatus::NonEscaping,
            resume_call_count: 0,
            resume_call_spans: Vec::new(),
        });
}

fn collect_escape_origin_aliases(
    body: &Body,
    facts: &CallableEscapeFacts,
) -> HashMap<LocalId, EscapeOrigin> {
    let mut aliases = HashMap::new();
    aliases.extend(
        facts
            .closures_by_local
            .keys()
            .copied()
            .map(|local| (local, EscapeOrigin::Closure(local))),
    );
    aliases.extend(
        facts
            .continuations_by_local
            .keys()
            .copied()
            .map(|local| (local, EscapeOrigin::Continuation(local))),
    );

    let mut changed = true;
    while changed {
        changed = false;
        for block in &body.blocks {
            for stmt in &block.stmts {
                let StatementKind::Assign {
                    target,
                    value: Rvalue::Use(Operand::Local(source)),
                } = &stmt.kind
                else {
                    continue;
                };
                let Some(origin) = aliases.get(source).copied() else {
                    continue;
                };
                if aliases.get(target).copied() != Some(origin) {
                    aliases.insert(*target, origin);
                    changed = true;
                }
            }
        }
    }

    aliases
}

fn analyze_statement_uses(
    stmt: &super::Statement,
    aliases: &HashMap<LocalId, EscapeOrigin>,
    facts: &mut CallableEscapeFacts,
    saw_unknown_mir: &mut bool,
) {
    match &stmt.kind {
        StatementKind::Assign { value, .. } => {
            analyze_rvalue_uses(value, stmt.span, aliases, facts, saw_unknown_mir);
        }
        StatementKind::StoreMember {
            receiver,
            value,
            continuation_route,
            ..
        } => {
            mark_operand_use(receiver, OperandUse::Escaping, aliases, facts);
            mark_operand_use(value, OperandUse::Escaping, aliases, facts);
            if let super::StoredContinuationRoutePublication::Unique(route) = continuation_route {
                mark_operand_use(
                    &Operand::Local(route.source_local),
                    OperandUse::Escaping,
                    aliases,
                    facts,
                );
            }
        }
        StatementKind::StoreTopLevelVar { value, .. } => {
            mark_operand_use(value, OperandUse::Escaping, aliases, facts);
        }
        StatementKind::Nop => {}
        StatementKind::Todo(_) => {
            *saw_unknown_mir = true;
        }
    }
}

fn analyze_rvalue_uses(
    value: &Rvalue,
    span: crate::span::Span,
    aliases: &HashMap<LocalId, EscapeOrigin>,
    facts: &mut CallableEscapeFacts,
    saw_unknown_mir: &mut bool,
) {
    match value {
        Rvalue::Use(Operand::Local(_)) | Rvalue::Use(Operand::Const(_)) => {}
        Rvalue::Transport { value, .. } => {
            mark_operand_use(value, OperandUse::Escaping, aliases, facts)
        }
        Rvalue::TypeCheck { value: operand, .. }
        | Rvalue::Cast { value: operand, .. }
        | Rvalue::TupleGet { tuple: operand, .. }
        | Rvalue::CaptureBoxNew { value: operand, .. }
        | Rvalue::CaptureBoxGet {
            box_operand: operand,
            ..
        }
        | Rvalue::PatternMatch {
            subject: operand, ..
        }
        | Rvalue::PatternExtract {
            subject: operand, ..
        } => mark_operand_use(operand, OperandUse::Escaping, aliases, facts),
        Rvalue::MemberAccess { receiver, .. } => {
            mark_operand_use(receiver, OperandUse::Escaping, aliases, facts);
        }
        Rvalue::Call { kind, args, .. } => {
            analyze_call_kind_uses(kind, span, aliases, facts);
            for arg in args {
                mark_operand_use(&arg.value, OperandUse::Escaping, aliases, facts);
            }
        }
        Rvalue::EnumVariant { args, .. } => {
            for arg in args {
                mark_operand_use(&arg.value, OperandUse::Escaping, aliases, facts);
            }
        }
        Rvalue::ClassCtor { args, .. } => {
            for arg in args {
                mark_operand_use(&arg.value, OperandUse::Escaping, aliases, facts);
            }
        }
        Rvalue::MakeTuple { elements, .. } => {
            for element in elements {
                mark_operand_use(element, OperandUse::Escaping, aliases, facts);
            }
        }
        Rvalue::StructLit { fields, .. } => {
            for field in fields {
                mark_operand_use(&field.value, OperandUse::Escaping, aliases, facts);
            }
        }
        Rvalue::InterpolatedString { parts, .. } => {
            for part in parts {
                if let super::InterpolatedStringPart::Expr { value, .. } = part {
                    mark_operand_use(value, OperandUse::Escaping, aliases, facts);
                }
            }
        }
        Rvalue::CaptureBoxSet {
            box_operand, value, ..
        } => {
            mark_operand_use(box_operand, OperandUse::Escaping, aliases, facts);
            mark_operand_use(value, OperandUse::Escaping, aliases, facts);
        }
        Rvalue::MakeClosure { env, .. } => {
            mark_operand_use(env, OperandUse::Escaping, aliases, facts);
        }
        Rvalue::TopLevelRef(_)
        | Rvalue::UnresolvedName { .. }
        | Rvalue::SizeOf { .. }
        | Rvalue::KindOf { .. }
        | Rvalue::AlignOf { .. }
        | Rvalue::DescOf { .. }
        | Rvalue::TypeMetadataLiteral(_)
        | Rvalue::PerformResult { .. } => {}
        Rvalue::Todo(reason) => {
            if !is_structural_handle_result_todo(reason) {
                *saw_unknown_mir = true;
            }
        }
    }
}

fn analyze_call_kind_uses(
    kind: &CallKind,
    span: crate::span::Span,
    aliases: &HashMap<LocalId, EscapeOrigin>,
    facts: &mut CallableEscapeFacts,
) {
    match kind {
        CallKind::Direct { .. } => {}
        CallKind::Closure { callee, fn_ptr } => mark_operand_use(
            callee,
            OperandUse::LocalClosureCall { fn_ptr },
            aliases,
            facts,
        ),
        CallKind::FunValue { callee } => {
            let resolved_fn_ptr = local_closure_fn_ptr_for_operand(callee, aliases, facts);
            if let Some(fn_ptr) = resolved_fn_ptr.as_deref() {
                mark_operand_use(
                    callee,
                    OperandUse::LocalClosureCall { fn_ptr },
                    aliases,
                    facts,
                );
            } else {
                mark_operand_use(callee, OperandUse::Escaping, aliases, facts);
            }
        }
        CallKind::FunPtr { callee } => {
            mark_operand_use(callee, OperandUse::Escaping, aliases, facts);
        }
        CallKind::Virtual { receiver, .. } | CallKind::Interface { receiver, .. } => {
            mark_operand_use(receiver, OperandUse::Escaping, aliases, facts);
        }
        CallKind::Resume { continuation, .. } => mark_operand_use(
            continuation,
            OperandUse::LocalContinuationResume { span },
            aliases,
            facts,
        ),
    }
}

fn analyze_terminator_uses(
    kind: &TerminatorKind,
    aliases: &HashMap<LocalId, EscapeOrigin>,
    facts: &mut CallableEscapeFacts,
    saw_unknown_mir: &mut bool,
) {
    match kind {
        TerminatorKind::Return { value } => {
            if let Some(value) = value {
                mark_operand_use(value, OperandUse::Escaping, aliases, facts);
            }
        }
        TerminatorKind::CondBr { cond, .. } => {
            mark_operand_use(cond, OperandUse::Escaping, aliases, facts);
        }
        TerminatorKind::Perform { args, .. } => {
            for arg in args {
                mark_operand_use(&arg.value, OperandUse::Escaping, aliases, facts);
            }
        }
        // `Handle` is a structural boundary whose body/arms/finally are already exposed as
        // successor blocks. The synthetic handle-exit `Todo` terminators likewise do not carry a
        // value use; treating them as unknown would hide facts discovered inside those blocks.
        TerminatorKind::Handle { .. } => {}
        TerminatorKind::Todo(reason) => {
            if !is_structural_handle_exit_todo(reason) {
                *saw_unknown_mir = true;
            }
        }
        TerminatorKind::Goto { .. }
        | TerminatorKind::ResumeUnwind
        | TerminatorKind::Unreachable => {}
    }
}

fn is_structural_handle_exit_todo(reason: &str) -> bool {
    matches!(
        reason,
        "handle body exit pending" | "handle arm exit pending" | "handle finally exit pending"
    )
}

fn is_structural_handle_result_todo(reason: &str) -> bool {
    matches!(reason, "handle result pending")
}

fn mark_operand_use(
    operand: &Operand,
    use_kind: OperandUse<'_>,
    aliases: &HashMap<LocalId, EscapeOrigin>,
    facts: &mut CallableEscapeFacts,
) {
    let Operand::Local(local) = operand else {
        return;
    };
    let Some(origin) = aliases.get(local).copied() else {
        return;
    };

    match (origin, use_kind) {
        (EscapeOrigin::Closure(origin), OperandUse::LocalClosureCall { fn_ptr }) => {
            let Some(fact) = facts.closures_by_local.get_mut(&origin) else {
                return;
            };
            if fact.fn_ptr == fn_ptr {
                fact.direct_call_count += 1;
            } else {
                fact.status = EscapeStatus::Escapes;
            }
        }
        (EscapeOrigin::Continuation(origin), OperandUse::LocalContinuationResume { span }) => {
            if let Some(fact) = facts.continuations_by_local.get_mut(&origin) {
                fact.resume_call_count += 1;
                if !fact.resume_call_spans.contains(&span) {
                    fact.resume_call_spans.push(span);
                }
            }
        }
        (EscapeOrigin::Closure(origin), _)
        | (EscapeOrigin::Continuation(origin), OperandUse::LocalClosureCall { .. }) => {
            mark_origin_escaped(origin, facts);
        }
        (EscapeOrigin::Continuation(origin), OperandUse::Escaping) => {
            if let Some(fact) = facts.continuations_by_local.get_mut(&origin) {
                fact.status = EscapeStatus::Escapes;
            }
        }
    }
}

/// 当 `CallKind::FunValue` 的 callee 实际上能 alias 回本函数内的某个 `MakeClosure`
/// 结果时，把它当成本地 closure 调用：返回该 closure 的 fn_ptr 字符串，否则 `None`。
fn local_closure_fn_ptr_for_operand(
    operand: &Operand,
    aliases: &HashMap<LocalId, EscapeOrigin>,
    facts: &CallableEscapeFacts,
) -> Option<String> {
    let Operand::Local(local) = operand else {
        return None;
    };
    let origin = aliases.get(local).copied()?;
    let EscapeOrigin::Closure(origin_local) = origin else {
        return None;
    };
    let fact = facts.closures_by_local.get(&origin_local)?;
    Some(fact.fn_ptr.clone())
}

fn mark_origin_escaped(origin: LocalId, facts: &mut CallableEscapeFacts) {
    if let Some(fact) = facts.closures_by_local.get_mut(&origin) {
        fact.status = EscapeStatus::Escapes;
    }
    if let Some(fact) = facts.continuations_by_local.get_mut(&origin) {
        fact.status = EscapeStatus::Escapes;
    }
}

fn mark_unescaped_facts_unknown(facts: &mut CallableEscapeFacts) {
    for fact in facts.closures_by_local.values_mut() {
        if fact.status == EscapeStatus::NonEscaping {
            fact.status = EscapeStatus::Unknown;
        }
    }
    for fact in facts.continuations_by_local.values_mut() {
        if fact.status == EscapeStatus::NonEscaping {
            fact.status = EscapeStatus::Unknown;
        }
    }
}

fn is_continuation_type(types: &TypeStore, ty: crate::ty::TypeId) -> bool {
    matches!(
        types.kind(ty),
        TypeKind::Ref(RefTypeKind::Nominal(nominal)) if nominal.fqn == "scoop.core.Continuation"
    )
}

fn local_name(body: &Body, local: LocalId) -> Option<String> {
    body.locals
        .get(local.as_u32() as usize)
        .and_then(|decl| decl.name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::{
        BasicBlock, CallArg, CallTransportMetadata, ClosureEnvTransportMetadata, ConstValue,
        LocalDecl, LocalSourceKind, MirTransportKind, ResumeMetadata, SiteId, Statement,
        Terminator, UnwindAction,
    };
    use crate::opt::OptLevel;
    use crate::session::Session;
    use crate::source::SourceFile;
    use crate::span::Span;
    use crate::ty::{EffectRow, NominalType, TypeStore};

    const SPAN: Span = Span { start: 0, end: 0 };

    #[test]
    fn closure_called_only_locally_is_non_escaping() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let mut body = Body::new_empty();
        let closure = body.push_local(local("f", builtins.any));
        let result = body.push_local(local("result", builtins.int));
        body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: vec![
                Statement {
                    span: SPAN,
                    kind: StatementKind::Assign {
                        target: closure,
                        value: Rvalue::MakeClosure {
                            env: Operand::Const(ConstValue::Unit),
                            fn_ptr: "fixtures.escape.lambda".to_string(),
                            env_contract: ClosureEnvTransportMetadata::empty(builtins.unit),
                        },
                    },
                },
                Statement {
                    span: SPAN,
                    kind: StatementKind::Assign {
                        target: result,
                        value: Rvalue::Call {
                            site_id: SiteId::from_raw(0),
                            kind: CallKind::Closure {
                                callee: Operand::Local(closure),
                                fn_ptr: "fixtures.escape.lambda".to_string(),
                            },
                            args: Vec::new(),
                            transport: call_transport(builtins.int),
                        },
                    },
                },
            ],
            terminator: return_unit(),
        });

        let facts = analyze_body(body, &types);
        let closure_fact = facts.closure(closure).expect("closure fact");
        assert_eq!(closure_fact.status, EscapeStatus::NonEscaping);
        assert_eq!(closure_fact.direct_call_count, 1);
    }

    #[test]
    fn closure_passed_as_argument_is_escaping() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let mut body = Body::new_empty();
        let closure = body.push_local(local("f", builtins.any));
        let result = body.push_local(local("result", builtins.int));
        body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: vec![
                Statement {
                    span: SPAN,
                    kind: StatementKind::Assign {
                        target: closure,
                        value: Rvalue::MakeClosure {
                            env: Operand::Const(ConstValue::Unit),
                            fn_ptr: "fixtures.escape.lambda".to_string(),
                            env_contract: ClosureEnvTransportMetadata::empty(builtins.unit),
                        },
                    },
                },
                Statement {
                    span: SPAN,
                    kind: StatementKind::Assign {
                        target: result,
                        value: Rvalue::Call {
                            site_id: SiteId::from_raw(0),
                            kind: CallKind::Direct {
                                callee_fqn: "fixtures.escape.consume".to_string(),
                            },
                            args: vec![CallArg {
                                span: SPAN,
                                name: None,
                                value: Operand::Local(closure),
                            }],
                            transport: call_transport(builtins.int),
                        },
                    },
                },
            ],
            terminator: return_unit(),
        });

        let facts = analyze_body(body, &types);
        assert_eq!(
            facts.closure(closure).expect("closure fact").status,
            EscapeStatus::Escapes
        );
    }

    #[test]
    fn continuation_used_only_as_resume_receiver_is_non_escaping() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let continuation_ty = continuation_ty(&mut types);
        let mut body = Body::new_empty();
        let continuation = body.push_local(local("k", continuation_ty));
        let result = body.push_local(local("result", builtins.unit));
        body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: vec![Statement {
                span: SPAN,
                kind: StatementKind::Assign {
                    target: result,
                    value: Rvalue::Call {
                        site_id: SiteId::from_raw(0),
                        kind: CallKind::Resume {
                            continuation: Operand::Local(continuation),
                            resume: ResumeMetadata {
                                continuation_ty,
                                resume_ty: builtins.unit,
                                answer_ty: builtins.unit,
                                return_ty: builtins.unit,
                                out_effects: crate::ty::EffectRow::pure(),
                                runtime_error_effect_ty: None,
                                suspends_outward: false,
                            },
                        },
                        args: Vec::new(),
                        transport: call_transport(builtins.unit),
                    },
                },
            }],
            terminator: return_unit(),
        });

        let facts = analyze_body(body, &types);
        let continuation_fact = facts.continuation(continuation).expect("continuation fact");
        assert_eq!(continuation_fact.status, EscapeStatus::NonEscaping);
        assert_eq!(continuation_fact.resume_call_count, 1);
        assert_eq!(continuation_fact.resume_call_spans, vec![SPAN]);
    }

    #[test]
    fn continuation_returned_from_callable_is_escaping() {
        let mut types = TypeStore::new();
        let continuation_ty = continuation_ty(&mut types);
        let mut body = Body::new_empty();
        let continuation = body.push_local(local("k", continuation_ty));
        body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: SPAN,
                kind: TerminatorKind::Return {
                    value: Some(Operand::Local(continuation)),
                },
                unwind: UnwindAction::NoUnwind,
            },
        });

        let facts = analyze_body(body, &types);
        assert_eq!(
            facts
                .continuation(continuation)
                .expect("continuation fact")
                .status,
            EscapeStatus::Escapes
        );
    }

    #[test]
    fn production_pass_view_publishes_escape_facts_only_when_opt_level_enables_them() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_escape_pass_view.scoop",
            r#"
package fixtures.mirescape

fun main(): Int {
    val f: (Int) -> Int = { x -> x + 1 }
    return f(41)
}
"#,
        );

        let o0 = super::super::materialize::materialize_for_dump_with_opt_level(
            &sess,
            &source,
            OptLevel::O0,
        )
        .unwrap();
        assert!(
            o0.pass_view().escape_facts().is_empty(),
            "O0 must not publish MIR escape facts"
        );

        let o2 = super::super::materialize::materialize_for_dump_with_opt_level(
            &sess,
            &source,
            OptLevel::O2,
        )
        .unwrap();
        let main_facts = o2
            .pass_view()
            .escape_facts()
            .callable("fixtures.mirescape.main")
            .expect("main should have escape facts at O2");
        assert!(
            main_facts
                .closures()
                .any(|fact| fact.status.is_non_escaping() && fact.direct_call_count == 1),
            "direct local closure call should be published as non-escaping"
        );
    }

    fn analyze_body(body: Body, types: &TypeStore) -> CallableEscapeFacts {
        let dummy_ty = body
            .locals
            .first()
            .map(|local| local.ty)
            .expect("test bodies should declare at least one local");
        analyze_callable_escape_facts(
            &FunDecl {
                span: SPAN,
                fqn: "fixtures.escape.fun".to_string(),
                name: "fun".to_string(),
                ty: dummy_ty,
                params: Vec::new(),
                return_ty: dummy_ty,
                body: Some(body),
            },
            types,
        )
    }

    fn local(name: &str, ty: crate::ty::TypeId) -> LocalDecl {
        LocalDecl {
            span: SPAN,
            name: Some(name.to_string()),
            ty,
            source: LocalSourceKind::SourceLocal,
        }
    }

    fn return_unit() -> Terminator {
        Terminator {
            span: SPAN,
            kind: TerminatorKind::Return {
                value: Some(Operand::Const(ConstValue::Unit)),
            },
            unwind: UnwindAction::NoUnwind,
        }
    }

    fn call_transport(result_ty: crate::ty::TypeId) -> CallTransportMetadata {
        CallTransportMetadata::plain_no_outward(result_ty, MirTransportKind::Unknown)
    }

    fn continuation_ty(types: &mut TypeStore) -> crate::ty::TypeId {
        let builtins = types.intern_builtins();
        types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "scoop.core.Continuation".to_string(),
            args: vec![builtins.int, builtins.unit],
            eff: Some(EffectRow::pure()),
        })))
    }
}
