//! Effect 分析：分类函数（Plain vs EffectStep）、检测挂起点、liveness 分析。

use std::collections::HashSet;

use crate::mir::{Body, CallKind, LocalId, Rvalue, StatementKind, TerminatorKind};

/// 函数是否含有任何 effect 结构（Perform/Handle/Resume）。
pub fn has_effect_structures(body: &Body) -> bool {
    for block in &body.blocks {
        // 检查语句中的 Resume 调用。
        for stmt in &block.stmts {
            if let StatementKind::Assign { value, .. } = &stmt.kind {
                if let Rvalue::Call { kind, .. } = value {
                    if matches!(kind, CallKind::Resume { .. }) {
                        return true;
                    }
                }
            }
        }
        // 检查终结符。
        match &block.terminator.kind {
            TerminatorKind::Perform { .. } | TerminatorKind::Handle { .. } => return true,
            _ => {}
        }
    }
    false
}

/// 函数是否含有 Handle 终结符。
pub fn has_handle(body: &Body) -> bool {
    for block in &body.blocks {
        if matches!(block.terminator.kind, TerminatorKind::Handle { .. }) {
            return true;
        }
    }
    false
}

/// 收集函数体中所有 Perform 的 op_fqn（用于确定 Step 的 outward cases）。
pub fn collect_perform_ops(body: &Body) -> Vec<String> {
    let mut ops = Vec::new();
    for block in &body.blocks {
        if let TerminatorKind::Perform { op_fqn, .. } = &block.terminator.kind {
            if !ops.contains(op_fqn) {
                ops.push(op_fqn.clone());
            }
        }
    }
    ops
}

/// 计算 basic block 的 live-out 集合（该块出口处存活的 locals）。
///
/// 标准 backward dataflow liveness analysis：
/// live_out(B) = ∪ live_in(S)  for each successor S
/// live_in(B) = (live_out(B) − def(B)) ∪ use(B)
///
/// 不动点迭代直到收敛。
pub fn compute_live_out(body: &Body) -> Vec<HashSet<LocalId>> {
    let n = body.blocks.len();
    let mut live_out: Vec<HashSet<LocalId>> = vec![HashSet::new(); n];
    // 预计算每个块的 use/def。
    let (uses, defs): (Vec<HashSet<LocalId>>, Vec<HashSet<LocalId>>) = body
        .blocks
        .iter()
        .map(|block| compute_use_def(block))
        .unzip();
    // 不动点迭代。
    let mut changed = true;
    while changed {
        changed = false;
        for i in 0..n {
            let mut new_live_out = HashSet::new();
            for succ in body.blocks[i].successors() {
                let succ_idx = succ.0 as usize;
                if succ_idx < n {
                    let live_in_succ: HashSet<LocalId> = live_out[succ_idx]
                        .difference(&defs[succ_idx])
                        .cloned()
                        .collect::<HashSet<_>>()
                        .union(&uses[succ_idx])
                        .cloned()
                        .collect();
                    new_live_out.extend(live_in_succ);
                }
            }
            if new_live_out != live_out[i] {
                live_out[i] = new_live_out;
                changed = true;
            }
        }
    }
    live_out
}

/// 计算每个块的 live_in 集合（块入口处的活跃 local）。
/// live_in[i] = (live_out[i] - defs[i]) ∪ uses[i]（uses 为块内 def 前的向上暴露使用）。
pub fn compute_live_in(body: &Body) -> Vec<HashSet<LocalId>> {
    let live_out = compute_live_out(body);
    body.blocks
        .iter()
        .enumerate()
        .map(|(i, block)| {
            let (uses, defs) = compute_use_def(block);
            live_out[i]
                .difference(&defs)
                .cloned()
                .collect::<HashSet<_>>()
                .union(&uses)
                .cloned()
                .collect()
        })
        .collect()
}

/// 计算一个 basic block 的 use/def 集合。
fn compute_use_def(block: &crate::mir::BasicBlock) -> (HashSet<LocalId>, HashSet<LocalId>) {    let mut uses = HashSet::new();
    let mut defs = HashSet::new();
    // 语句中的 use/def。
    for stmt in &block.stmts {
        if let StatementKind::Assign { target, value } = &stmt.kind {
            // target 是 def。
            defs.insert(*target);
            // value 中的 local 引用是 use。
            collect_rvalue_uses(value, &mut uses, &defs);
        }
    }
    // 终结符中的 use。
    collect_terminator_uses(&block.terminator.kind, &mut uses, &defs);
    (uses, defs)
}

/// 收集 Rvalue 中使用的 locals（在 def 之前使用的）。
pub(crate) fn collect_rvalue_uses(rv: &Rvalue, uses: &mut HashSet<LocalId>, defs: &HashSet<LocalId>) {
    match rv {
        Rvalue::Use(op) => collect_operand_uses(op, uses, defs),
        Rvalue::Call { kind, args, .. } => {
            collect_call_kind_uses(kind, uses, defs);
            for a in args {
                collect_operand_uses(&a.value, uses, defs);
            }
        }
        Rvalue::MakeTuple { elements, .. } | Rvalue::MakeArray { elements, .. } => {
            for e in elements {
                collect_operand_uses(e, uses, defs);
            }
        }
        Rvalue::MemberAccess { receiver, .. } | Rvalue::TupleIndex { receiver, .. } => {
            collect_operand_uses(receiver, uses, defs);
        }
        Rvalue::IndexAccess {
            receiver, indices, ..
        } => {
            collect_operand_uses(receiver, uses, defs);
            for i in indices {
                collect_operand_uses(i, uses, defs);
            }
        }
        Rvalue::ClassCtor { args, .. } | Rvalue::EnumVariant { args, .. } => {
            for a in args {
                collect_operand_uses(&a.value, uses, defs);
            }
        }
        Rvalue::MakeClosure { env, .. } => {
            collect_operand_uses(env, uses, defs);
        }
        Rvalue::WithUpdate { base, updates, .. } => {
            collect_operand_uses(base, uses, defs);
            for u in updates {
                collect_operand_uses(&u.value, uses, defs);
            }
        }
        Rvalue::StructLit { fields, .. } => {
            for f in fields {
                collect_operand_uses(&f.value, uses, defs);
            }
        }
        Rvalue::Cast { value, .. } | Rvalue::TypeTest { value, .. } => {
            collect_operand_uses(value, uses, defs);
        }
        Rvalue::PatternMatch { subject, .. } | Rvalue::PatternExtract { subject, .. } => {
            collect_operand_uses(subject, uses, defs);
        }
        Rvalue::IntEq { lhs, rhs } => {
            collect_operand_uses(lhs, uses, defs);
            collect_operand_uses(rhs, uses, defs);
        }
        Rvalue::InterpolatedString { parts } => {
            for p in parts {
                if let crate::mir::InterpolatedPart::Expr(op) = p {
                    collect_operand_uses(op, uses, defs);
                }
            }
        }
        _ => {}
    }
}

fn collect_call_kind_uses(kind: &CallKind, uses: &mut HashSet<LocalId>, defs: &HashSet<LocalId>) {
    match kind {
        CallKind::Closure { callee, .. } | CallKind::FunValue { callee } => {
            collect_operand_uses(callee, uses, defs);
        }
        CallKind::Virtual { receiver, .. } | CallKind::Interface { receiver, .. } => {
            collect_operand_uses(receiver, uses, defs);
        }
        CallKind::Resume {
            continuation,
            resume_value,
        } => {
            collect_operand_uses(continuation, uses, defs);
            collect_operand_uses(resume_value, uses, defs);
        }
        CallKind::Direct { .. } => {}
    }
}

pub(crate) fn collect_terminator_uses(
    kind: &TerminatorKind,
    uses: &mut HashSet<LocalId>,
    defs: &HashSet<LocalId>,
) {
    match kind {
        TerminatorKind::Return { value: Some(op) } => {
            collect_operand_uses(op, uses, defs);
        }
        TerminatorKind::CondBr { cond, .. } => {
            collect_operand_uses(cond, uses, defs);
        }
        TerminatorKind::Perform { args, .. } => {
            for a in args {
                collect_operand_uses(&a.value, uses, defs);
            }
        }
        _ => {}
    }
}

fn collect_operand_uses(
    op: &crate::mir::Operand,
    uses: &mut HashSet<LocalId>,
    defs: &HashSet<LocalId>,
) {
    if let crate::mir::Operand::Local(lid) = op {
        if !defs.contains(lid) {
            uses.insert(*lid);
        }
    }
}
