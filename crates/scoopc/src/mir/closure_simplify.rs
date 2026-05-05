//! Conservative non-escaping closure simplification.
//!
//! This pass deliberately consumes the escape facts already published in `MaterializedMirPassView`
//! instead of rediscovering escape state while rewriting. The first implementation only eliminates
//! the no-capture shape that the current production MIR bridge can still lower after rewriting:
//! a `MakeClosure` with `Unit` env, proven non-escaping, called exactly once in the same callable
//! body, and backed by a straight-line closure body.

use std::collections::{BTreeMap, HashMap, HashSet};

use super::{
    Body, CallArg, CallKind, CallableEscapeFacts, EscapeStatus, FunDecl, InstanceKey,
    InstanceSummary, Item, LocalId, MaterializedMir, Operand, Rvalue, Statement, StatementKind,
    TerminatorKind, summarize_pass_rewritten_fun,
};

#[derive(Debug, Clone)]
struct SimplifyFunction {
    key: InstanceKey,
    fun: FunDecl,
    summary: InstanceSummary,
}

#[derive(Debug)]
struct ClosureSimplifySnapshot {
    functions: Vec<SimplifyFunction>,
    caller_candidates: Vec<FunDecl>,
    callables_by_fqn: HashMap<String, FunDecl>,
    facts_by_fqn: BTreeMap<String, CallableEscapeFacts>,
}

#[derive(Debug, Clone)]
struct ClosureProvenance {
    origin: LocalId,
    fn_ptr: String,
    env: Operand,
}

#[derive(Debug, Default)]
struct BlockClosureProvenance {
    locals: HashMap<LocalId, ClosureProvenance>,
}

/// Run the minimal non-escaping closure simplification over pass-visible MIR bodies.
///
/// Returns `true` when the pass published at least one rewritten callable body.
pub(crate) fn run_non_escaping_closure_simplification(materialized: &mut MaterializedMir) -> bool {
    if materialized.pass_view().escape_facts().is_empty() {
        return false;
    }

    let snapshot = ClosureSimplifySnapshot::from_materialized(materialized);
    let mut instance_rewrites = Vec::new();
    let mut caller_rewrites = Vec::new();

    for function in &snapshot.functions {
        let Some(rewritten) = rewrite_callable_body_once(&function.fun, &snapshot) else {
            continue;
        };
        if !super::inline::pass_publishable_caller_body(&rewritten) {
            continue;
        }
        let summary =
            summarize_pass_rewritten_fun(&rewritten, &materialized.types, Some(&function.summary));
        instance_rewrites.push((function.key.clone(), rewritten, summary));
    }

    for fun in &snapshot.caller_candidates {
        let Some(rewritten) = rewrite_callable_body_once(fun, &snapshot) else {
            continue;
        };
        if super::inline::pass_publishable_caller_body(&rewritten) {
            caller_rewrites.push(rewritten);
        }
    }

    if instance_rewrites.is_empty() && caller_rewrites.is_empty() {
        return false;
    }

    let pass_artifacts = materialized.pass_artifacts_mut();
    for (key, fun, summary) in instance_rewrites {
        pass_artifacts.replace_callable_body(fun);
        pass_artifacts.set_instance_summary(key, summary);
    }
    for fun in caller_rewrites {
        pass_artifacts.replace_callable_body(fun);
    }
    true
}

impl ClosureSimplifySnapshot {
    fn from_materialized(materialized: &MaterializedMir) -> Self {
        let pass_view = materialized.pass_view();
        let functions = pass_view
            .instances()
            .filter_map(|family| {
                let fun = family.root_body()?.clone();
                Some(SimplifyFunction {
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
        let mut callables_by_fqn = materialized
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fun(fun) => Some((fun.fqn.clone(), fun.clone())),
                Item::Todo { .. } => None,
            })
            .collect::<HashMap<_, _>>();
        for family in pass_view.instances() {
            for fun in family.callable_bodies() {
                callables_by_fqn.insert(fun.fqn.clone(), fun.clone());
            }
        }
        for fun in &caller_candidates {
            callables_by_fqn.insert(fun.fqn.clone(), fun.clone());
        }
        let facts_by_fqn = pass_view
            .escape_facts()
            .callables()
            .map(|(fqn, facts)| (fqn.to_string(), facts.clone()))
            .collect();
        Self {
            functions,
            caller_candidates,
            callables_by_fqn,
            facts_by_fqn,
        }
    }

    fn callable(&self, fqn: &str) -> Option<&FunDecl> {
        self.callables_by_fqn.get(fqn)
    }

    fn facts(&self, fqn: &str) -> Option<&CallableEscapeFacts> {
        self.facts_by_fqn.get(fqn)
    }
}

impl BlockClosureProvenance {
    fn observe_statement(&mut self, stmt: &Statement, facts: &CallableEscapeFacts) {
        let StatementKind::Assign { target, value } = &stmt.kind else {
            return;
        };
        match value {
            Rvalue::MakeClosure { env, fn_ptr }
                if matches!(env, Operand::Const(super::ConstValue::Unit))
                    && proven_non_escaping_single_call_closure(facts, *target, fn_ptr)
                        .is_some() =>
            {
                self.locals.insert(
                    *target,
                    ClosureProvenance {
                        origin: *target,
                        fn_ptr: fn_ptr.clone(),
                        env: env.clone(),
                    },
                );
            }
            Rvalue::Use(operand) => match self.provenance_of_operand(operand).cloned() {
                Some(provenance) => {
                    self.locals.insert(*target, provenance);
                }
                None => {
                    self.locals.remove(target);
                }
            },
            _ => {
                self.locals.remove(target);
            }
        }
    }

    fn provenance_of_operand(&self, operand: &Operand) -> Option<&ClosureProvenance> {
        let Operand::Local(local) = operand else {
            return None;
        };
        self.locals.get(local)
    }
}

fn rewrite_callable_body_once(
    fun: &FunDecl,
    snapshot: &ClosureSimplifySnapshot,
) -> Option<FunDecl> {
    let facts = snapshot.facts(&fun.fqn)?;
    let mut rewritten = fun.clone();
    let body = rewritten.body.as_mut()?;
    let mut changed = false;

    for block_index in 0..body.blocks.len() {
        let old_stmts = std::mem::take(&mut body.blocks[block_index].stmts);
        let mut new_stmts = Vec::with_capacity(old_stmts.len());
        let mut block_provenance = BlockClosureProvenance::default();

        for stmt in old_stmts {
            if let Some(expanded) = try_expand_non_escaping_closure_call(
                body,
                &stmt,
                facts,
                snapshot,
                &block_provenance,
            ) {
                for expanded_stmt in expanded {
                    block_provenance.observe_statement(&expanded_stmt, facts);
                    new_stmts.push(expanded_stmt);
                }
                changed = true;
            } else {
                block_provenance.observe_statement(&stmt, facts);
                new_stmts.push(stmt);
            }
        }

        body.blocks[block_index].stmts = new_stmts;
    }

    if changed {
        remove_dead_closure_artifacts(body);
    }
    changed.then_some(rewritten)
}

fn try_expand_non_escaping_closure_call(
    caller_body: &mut Body,
    stmt: &Statement,
    facts: &CallableEscapeFacts,
    snapshot: &ClosureSimplifySnapshot,
    block_provenance: &BlockClosureProvenance,
) -> Option<Vec<Statement>> {
    let StatementKind::Assign { target, value } = &stmt.kind else {
        return None;
    };
    let Rvalue::Call { kind, args, .. } = value else {
        return None;
    };
    let CallKind::Closure { callee, fn_ptr } = kind else {
        return None;
    };
    let provenance = block_provenance.provenance_of_operand(callee)?;
    if provenance.fn_ptr != *fn_ptr {
        return None;
    }
    proven_non_escaping_single_call_closure(facts, provenance.origin, fn_ptr)?;

    let closure = snapshot.callable(fn_ptr)?;
    let closure_body = closure.body.as_ref()?;
    if !super::inline::body_is_inlineable_without_callable_provenance(closure_body) {
        return None;
    }
    let param_operands = bind_closure_call_args(closure, &provenance.env, args)?;
    super::inline::expand_straight_line_call_without_callable_provenance(
        caller_body,
        *target,
        stmt.span,
        closure,
        &param_operands,
    )
}

fn proven_non_escaping_single_call_closure<'a>(
    facts: &'a CallableEscapeFacts,
    local: LocalId,
    fn_ptr: &str,
) -> Option<&'a super::ClosureEscapeFact> {
    let fact = facts.closure(local)?;
    (fact.status == EscapeStatus::NonEscaping
        && fact.direct_call_count == 1
        && fact.fn_ptr == fn_ptr)
        .then_some(fact)
}

fn bind_closure_call_args(
    closure: &FunDecl,
    env: &Operand,
    args: &[CallArg],
) -> Option<Vec<Operand>> {
    let value_params = closure.params.get(1..)?;
    if args.len() != value_params.len() {
        return None;
    }

    let mut slots = vec![None; value_params.len()];
    let mut next_positional = 0usize;
    for arg in args {
        let index = if let Some(name) = &arg.name {
            value_params.iter().position(|param| &param.name == name)?
        } else {
            while next_positional < slots.len() && slots[next_positional].is_some() {
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

    let mut out = Vec::with_capacity(closure.params.len());
    out.push(env.clone());
    out.extend(slots.into_iter().collect::<Option<Vec<_>>>()?);
    Some(out)
}

fn remove_dead_closure_artifacts(body: &mut Body) {
    loop {
        let used = collect_used_locals(body);
        let mut removed_any = false;
        for block in &mut body.blocks {
            let old_stmts = std::mem::take(&mut block.stmts);
            block.stmts = old_stmts
                .into_iter()
                .filter(|stmt| {
                    if dead_closure_artifact_assignment(stmt, &used) {
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

fn dead_closure_artifact_assignment(stmt: &Statement, used: &HashSet<LocalId>) -> bool {
    let StatementKind::Assign { target, value } = &stmt.kind else {
        return false;
    };
    !used.contains(target) && matches!(value, Rvalue::MakeClosure { .. } | Rvalue::Use(_))
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
    }
}

fn collect_rvalue_uses(value: &Rvalue, out: &mut HashSet<LocalId>) {
    match value {
        Rvalue::Use(operand)
        | Rvalue::Unary { operand, .. }
        | Rvalue::TypeCheck { value: operand, .. }
        | Rvalue::Cast { value: operand, .. }
        | Rvalue::TupleGet { tuple: operand, .. }
        | Rvalue::CaptureBoxNew { value: operand }
        | Rvalue::CaptureBoxGet {
            box_operand: operand,
        }
        | Rvalue::PatternMatch {
            subject: operand, ..
        }
        | Rvalue::PatternExtract {
            subject: operand, ..
        } => collect_operand_use(operand, out),
        Rvalue::Binary { lhs, rhs, .. } => {
            collect_operand_use(lhs, out);
            collect_operand_use(rhs, out);
        }
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
        Rvalue::MakeTuple { elements } => {
            for element in elements {
                collect_operand_use(element, out);
            }
        }
        Rvalue::InterpolatedString { parts, .. } => {
            for part in parts {
                if let super::InterpolatedStringPart::Expr { value, .. } = part {
                    collect_operand_use(value, out);
                }
            }
        }
        Rvalue::CaptureBoxSet { box_operand, value } => {
            collect_operand_use(box_operand, out);
            collect_operand_use(value, out);
        }
        Rvalue::MakeClosure { env, .. } => collect_operand_use(env, out),
        Rvalue::TopLevelRef(_)
        | Rvalue::UnresolvedName { .. }
        | Rvalue::PerformResult { .. }
        | Rvalue::Todo(_) => {}
    }
}

fn collect_call_kind_uses(kind: &CallKind, out: &mut HashSet<LocalId>) {
    match kind {
        CallKind::Direct { .. } => {}
        CallKind::Closure { callee, .. } | CallKind::FunValue { callee } => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::materialize::materialize_for_dump_with_opt_level;
    use crate::opt::OptLevel;
    use crate::session::Session;
    use crate::source::SourceFile;

    #[test]
    fn non_escaping_unit_env_closure_call_is_inlined_from_escape_facts() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_closure_simplify_local.scoop",
            r#"
package fixtures.mirclosuresimplify

fun applyLocal(x: Int): Int {
    val f: (Int) -> Int = { y -> y + 1 }
    return f(x)
}

fun main(): Int {
    return applyLocal(41)
}
"#,
        );

        let materialized =
            materialize_for_dump_with_opt_level(&sess, &source, OptLevel::O2).unwrap();
        let pass_view = materialized.pass_view();
        let rewritten = pass_view
            .callable("fixtures.mirclosuresimplify.applyLocal")
            .expect("non-escaping closure simplification should publish applyLocal");

        assert!(
            pass_view.callable_body_is_overridden("fixtures.mirclosuresimplify.applyLocal"),
            "simplification must publish a pass-rewritten callable body"
        );
        assert!(
            !fun_contains_make_closure(rewritten),
            "rewritten body should remove the local closure allocation"
        );
        assert!(
            !fun_contains_closure_call(rewritten),
            "rewritten body should inline the local closure call"
        );
        assert!(
            pass_view
                .escape_facts()
                .callable("fixtures.mirclosuresimplify.applyLocal")
                .is_none_or(|facts| facts.closures().next().is_none()),
            "escape facts should be refreshed after removing the closure local"
        );
    }

    #[test]
    fn escaping_closure_is_not_simplified() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_closure_simplify_escape.scoop",
            r#"
package fixtures.mirclosuresimplify

fun leak(): (Int) -> Int / Pure! {
    val f: (Int) -> Int = { y -> y + 1 }
    return f
}

fun main(): Int {
    val f: (Int) -> Int / Pure! = leak()
    return 0
}
"#,
        );

        let materialized =
            materialize_for_dump_with_opt_level(&sess, &source, OptLevel::O2).unwrap();
        let pass_view = materialized.pass_view();
        assert!(
            !pass_view.callable_body_is_overridden("fixtures.mirclosuresimplify.leak"),
            "escaping closure producer must not be rewritten"
        );
        let leak_facts = pass_view
            .escape_facts()
            .callable("fixtures.mirclosuresimplify.leak")
            .expect("leak should still publish escaping closure facts");
        assert!(
            leak_facts
                .closures()
                .any(|fact| fact.status == EscapeStatus::Escapes),
            "returned closure should remain classified as escaping"
        );
    }

    #[test]
    fn o0_without_escape_facts_does_not_simplify_closure() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_closure_simplify_o0.scoop",
            r#"
package fixtures.mirclosuresimplify

fun applyLocal(x: Int): Int {
    val f: (Int) -> Int = { y -> y + 1 }
    return f(x)
}

fun main(): Int {
    return applyLocal(41)
}
"#,
        );

        let materialized =
            materialize_for_dump_with_opt_level(&sess, &source, OptLevel::O0).unwrap();
        let pass_view = materialized.pass_view();
        assert!(
            pass_view.escape_facts().is_empty(),
            "O0 must not publish escape facts"
        );
        assert!(
            !pass_view.callable_body_is_overridden("fixtures.mirclosuresimplify.applyLocal"),
            "closure simplification must not run without pass-view escape facts"
        );
    }

    fn fun_contains_make_closure(fun: &FunDecl) -> bool {
        fun.body.as_ref().is_some_and(|body| {
            body.blocks.iter().any(|block| {
                block.stmts.iter().any(|stmt| {
                    matches!(
                        &stmt.kind,
                        StatementKind::Assign {
                            value: Rvalue::MakeClosure { .. },
                            ..
                        }
                    )
                })
            })
        })
    }

    fn fun_contains_closure_call(fun: &FunDecl) -> bool {
        fun.body.as_ref().is_some_and(|body| {
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
        })
    }
}
