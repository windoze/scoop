//! Summary-driven MIR inlining passes.
//!
//! This pass is deliberately conservative: callee eligibility still comes from pass-visible
//! monomorphic callable roots, and it only inlines small, non-recursive, body-known straight-line
//! direct calls. Caller-side rewrites may also publish request-root reachable non-generic bodies,
//! but only after a pass actually changes them and the rewritten MIR stays inside the currently
//! supported production pass-body subset.

use std::collections::HashMap;

use super::{
    Body, CallArg, CallKind, FunDecl, InstanceKey, InstanceSummary, LocalDecl, LocalId,
    MaterializedMir, Operand, Rvalue, Statement, StatementKind, TerminatorKind, UnwindAction,
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

#[derive(Debug)]
struct InlineSnapshot {
    functions: Vec<InlineFunction>,
    caller_candidates: Vec<FunDecl>,
    by_fqn: HashMap<String, usize>,
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
        let by_fqn = functions
            .iter()
            .enumerate()
            .map(|(idx, function)| (function.fun.fqn.clone(), idx))
            .collect::<HashMap<_, _>>();
        Self {
            functions,
            caller_candidates,
            by_fqn,
        }
    }

    fn get(&self, fqn: &str) -> Option<&InlineFunction> {
        self.by_fqn
            .get(fqn)
            .and_then(|&idx| self.functions.get(idx))
    }
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

        for stmt in old_stmts {
            if let Some(expanded) = try_expand_direct_call(body, &fun.fqn, &stmt, snapshot) {
                new_stmts.extend(expanded);
                changed = true;
            } else {
                new_stmts.push(stmt);
            }
        }

        body.blocks[block_index].stmts = new_stmts;
    }

    changed.then_some(rewritten)
}

fn try_expand_direct_call(
    caller_body: &mut Body,
    caller_fqn: &str,
    stmt: &Statement,
    snapshot: &InlineSnapshot,
) -> Option<Vec<Statement>> {
    let StatementKind::Assign { target, value } = &stmt.kind else {
        return None;
    };
    let Rvalue::Call { kind, args } = value else {
        return None;
    };
    let CallKind::Direct { callee_fqn } = kind else {
        return None;
    };
    if callee_fqn == caller_fqn {
        return None;
    }

    let callee = snapshot.get(callee_fqn)?;
    if !callee_is_inline_candidate(callee) {
        return None;
    }

    expand_straight_line_call(caller_body, *target, stmt.span, &callee.fun, args)
}

fn callee_is_inline_candidate(callee: &InlineFunction) -> bool {
    callee.summary.body_known
        && !callee.summary.recursive_scc
        && callee.summary.size_cost <= INLINE_SIZE_THRESHOLD
        && callee.fun.body.as_ref().is_some_and(body_is_inlineable)
}

fn body_is_inlineable(body: &Body) -> bool {
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
        .all(|stmt| statement_is_inlineable(&stmt.kind))
}

fn pass_publishable_caller_body(fun: &FunDecl) -> bool {
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
        StatementKind::Todo(_) => false,
    }
}

fn rvalue_is_pass_publishable(value: &Rvalue) -> bool {
    match value {
        Rvalue::Use(_) | Rvalue::TopLevelRef(_) | Rvalue::Unary { .. } | Rvalue::Binary { .. } => {
            true
        }
        Rvalue::Call { kind, .. } => matches!(kind, CallKind::Direct { .. }),
        Rvalue::UnresolvedName { .. }
        | Rvalue::TypeCheck { .. }
        | Rvalue::Cast { .. }
        | Rvalue::MemberAccess { .. }
        | Rvalue::MakeTuple { .. }
        | Rvalue::TupleGet { .. }
        | Rvalue::CaptureBoxNew { .. }
        | Rvalue::CaptureBoxGet { .. }
        | Rvalue::CaptureBoxSet { .. }
        | Rvalue::PatternMatch { .. }
        | Rvalue::PatternExtract { .. }
        | Rvalue::MakeClosure { .. }
        | Rvalue::PerformResult { .. }
        | Rvalue::Todo(_) => false,
    }
}

fn statement_is_inlineable(kind: &StatementKind) -> bool {
    match kind {
        StatementKind::Nop => true,
        StatementKind::Assign { value, .. } => rvalue_is_inlineable(value),
        StatementKind::Todo(_) => false,
    }
}

fn rvalue_is_inlineable(value: &Rvalue) -> bool {
    match value {
        Rvalue::Use(_) | Rvalue::TopLevelRef(_) | Rvalue::Unary { .. } | Rvalue::Binary { .. } => {
            true
        }
        Rvalue::Call { kind, .. } => matches!(kind, CallKind::Direct { .. }),
        Rvalue::UnresolvedName { .. }
        | Rvalue::TypeCheck { .. }
        | Rvalue::Cast { .. }
        | Rvalue::MemberAccess { .. }
        | Rvalue::MakeTuple { .. }
        | Rvalue::TupleGet { .. }
        | Rvalue::CaptureBoxNew { .. }
        | Rvalue::CaptureBoxGet { .. }
        | Rvalue::CaptureBoxSet { .. }
        | Rvalue::PatternMatch { .. }
        | Rvalue::PatternExtract { .. }
        | Rvalue::MakeClosure { .. }
        | Rvalue::PerformResult { .. }
        | Rvalue::Todo(_) => false,
    }
}

fn expand_straight_line_call(
    caller_body: &mut Body,
    caller_target: LocalId,
    call_span: crate::span::Span,
    callee: &FunDecl,
    args: &[CallArg],
) -> Option<Vec<Statement>> {
    let callee_body = callee.body.as_ref()?;
    let block = &callee_body.blocks[0];
    let param_operands = bind_args_to_params(&callee.params, args)?;
    let mut local_operands = callee
        .params
        .iter()
        .map(|param| param.local)
        .zip(param_operands)
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
                let mapped_value = remap_rvalue(value, &local_operands, &local_map)?;
                out.push(Statement {
                    span: callee_stmt.span,
                    kind: StatementKind::Assign {
                        target: mapped_target,
                        value: mapped_value,
                    },
                });
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
    local_operands: &HashMap<LocalId, Operand>,
    local_map: &HashMap<LocalId, LocalId>,
) -> Option<Rvalue> {
    match value {
        Rvalue::Use(operand) => Some(Rvalue::Use(remap_operand(
            operand,
            local_operands,
            local_map,
        )?)),
        Rvalue::TopLevelRef(top) => Some(Rvalue::TopLevelRef(top.clone())),
        Rvalue::Unary { op, operand } => Some(Rvalue::Unary {
            op: *op,
            operand: remap_operand(operand, local_operands, local_map)?,
        }),
        Rvalue::Binary { lhs, op, rhs } => Some(Rvalue::Binary {
            lhs: remap_operand(lhs, local_operands, local_map)?,
            op: *op,
            rhs: remap_operand(rhs, local_operands, local_map)?,
        }),
        Rvalue::Call {
            kind: CallKind::Direct { callee_fqn },
            args,
        } => Some(Rvalue::Call {
            kind: CallKind::Direct {
                callee_fqn: callee_fqn.clone(),
            },
            args: args
                .iter()
                .map(|arg| {
                    Some(CallArg {
                        span: arg.span,
                        name: arg.name.clone(),
                        value: remap_operand(&arg.value, local_operands, local_map)?,
                    })
                })
                .collect::<Option<Vec<_>>>()?,
        }),
        Rvalue::Call { .. }
        | Rvalue::UnresolvedName { .. }
        | Rvalue::TypeCheck { .. }
        | Rvalue::Cast { .. }
        | Rvalue::MemberAccess { .. }
        | Rvalue::MakeTuple { .. }
        | Rvalue::TupleGet { .. }
        | Rvalue::CaptureBoxNew { .. }
        | Rvalue::CaptureBoxGet { .. }
        | Rvalue::CaptureBoxSet { .. }
        | Rvalue::PatternMatch { .. }
        | Rvalue::PatternExtract { .. }
        | Rvalue::MakeClosure { .. }
        | Rvalue::PerformResult { .. }
        | Rvalue::Todo(_) => None,
    }
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
    use crate::mir::{Item, materialize_for_dump};
    use crate::session::Session;
    use crate::source::SourceFile;

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
    fn caller_side_inlining_publishes_only_rewritten_non_generic_body() {
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
            pass_view.owner_of_callable(caller_fqn).is_none(),
            "non-generic caller 不应被伪装成某个 materialized instance family"
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
            pass_view.callable(stable_fqn).is_none(),
            "未被 pass 改写的 non-generic body 不应无条件进入 pass view"
        );
        assert!(
            !pass_view.callable_body_is_overridden(stable_fqn),
            "未改写的 non-generic body 不应被标记为 pass override"
        );
    }

    fn raw_fun<'a>(materialized: &'a MaterializedMir, fqn: &str) -> &'a FunDecl {
        materialized
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == fqn => Some(fun),
                Item::Fun(_) | Item::Todo { .. } => None,
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
}
