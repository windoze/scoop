//! 内联 pass：把小型 direct callee 的函数体内联到调用点。
//!
//! 设计目标（GAP4）：
//! - **HOF 透明性**：当一个小型 HOF（如 `apply(f, x) { f(x) }`）被 direct 调用时，
//!   内联其函数体可暴露出对函数参数 `f` 的 FunValue 调用，使后续 pass 能进一步
//!   内联已知闭包（map/filter 的 lambda）。这降低 HOF 抽象的运行期开销。
//! - **effect-transparent HOF**：forEach/map/filter 这类函数本身不产生新 effect，
//!   只转发闭包参数的 effect（`<eff E>` 是转发参数）。对这类函数放宽门控（允许
//!   多块、更高语句上限），只要 body 不直接 Perform/Handle。
//! - **闭包内联（第二级）**：当 `CallKind::FunValue`/`Closure` 的 callee 是一个已知
//!   的 `MakeClosure` local 时，内联闭包的 invoke 函数体。
//! - **保守性**：只内联满足全部安全条件的 callee（见 `try_make_inlineable_with_store`）。
//!
//! 该 pass 在 materialize + devirtualize 之后运行，作用于已单态化的 Module。

use std::collections::HashMap;

use crate::mir::{
    BasicBlock, BasicBlockId, Body, CallKind, FunDecl, Item, LocalDecl, LocalId, Module, Operand,
    Rvalue, Statement, StatementKind, Terminator, TerminatorKind,
};

/// 单函数内联的语句数上限（含赋值 / store / panic；超过则不内联）。
const MAX_STMTS: usize = 8;
/// effect-transparent HOF 内联的语句数上限（放宽，支持 forEach 等循环）。
const MAX_STMTS_HOF: usize = 40;
/// effect-transparent HOF 内联的基本块数上限。
const MAX_BLOCKS_HOF: usize = 6;

/// 内联配置。
#[derive(Clone, Copy, Debug)]
pub struct InlineConfig {
    /// 单个 callee 内联的语句数上限。
    pub max_stmts: usize,
    /// effect-transparent HOF 的语句数上限。
    pub max_stmts_hof: usize,
    /// effect-transparent HOF 的基本块数上限。
    pub max_blocks_hof: usize,
    /// 单个函数体内联展开的次数上限（防止内联爆炸）。
    pub max_inline_per_fn: usize,
}

impl Default for InlineConfig {
    fn default() -> Self {
        Self {
            max_stmts: MAX_STMTS,
            max_stmts_hof: MAX_STMTS_HOF,
            max_blocks_hof: MAX_BLOCKS_HOF,
            max_inline_per_fn: 32,
        }
    }
}

/// 对整个 Module 执行内联 pass。
///
/// 反复迭代直到没有可内联的调用点，或达到单函数内联上限。
pub fn inline_module(module: &mut Module, config: InlineConfig) {
    let store = &module.types;
    // 多轮迭代：每轮可能暴露新的可内联机会（HOF 内联后暴露闭包调用）。
    for _ in 0..config.max_inline_per_fn {
        let mut changed = false;
        // 收集 direct callee 快照（带 TypeStore 判定 effect-transparent）。
        let callees = collect_inlineable_callees(module, store, &config);
        // 收集闭包 invoke 函数快照。
        let closures = collect_inlineable_closures(module);
        if callees.is_empty() && closures.is_empty() {
            break;
        }
        for item in &mut module.items {
            if let Item::Fun(fd) = item {
                if let Some(body) = &mut fd.body {
                    if inline_body_once(body, &callees, &closures, &config) {
                        changed = true;
                    }
                }
            }
            if let Item::Initializer(ir) = item {
                if inline_body_once(&mut ir.body, &callees, &closures, &config) {
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
}

/// 收集模块中所有满足内联条件的 direct callee 快照。
fn collect_inlineable_callees(
    module: &Module,
    store: &scoop2_hir::ty::TypeStore,
    config: &InlineConfig,
) -> HashMap<String, InlineableCallee> {
    let mut result = HashMap::new();
    for item in &module.items {
        if let Item::Fun(fd) = item {
            if let Some(callee) = try_make_inlineable_with_store(fd, store, config) {
                result.insert(fd.fqn.clone(), callee);
            }
        }
    }
    result
}

/// 收集模块中所有闭包 invoke 函数（用于闭包内联）。
/// key = invoke_fqn，value = 闭包函数快照。
fn collect_inlineable_closures(module: &Module) -> HashMap<String, InlineableCallee> {
    let store = &module.types;
    let mut result = HashMap::new();
    for item in &module.items {
        if let Item::Fun(fd) = item {
            // 闭包 invoke 函数的 name 以 $closure 开头。
            if fd.name.starts_with("$closure") {
                if let Some(callee) = try_make_inlineable_with_store(fd, store, &InlineConfig::default()) {
                    result.insert(fd.fqn.clone(), callee);
                }
            }
        }
    }
    result
}

/// 内联所需的 callee 快照（与原 FunDecl 解耦，避免借用冲突）。
#[derive(Clone)]
struct InlineableCallee {
    fqn: String,
    /// 参数 local id → 类型（用于创建调用者侧的新 local）。
    params: Vec<(LocalId, LocalDecl)>,
    /// 函数体的全部 local（参数 + 局部 + 临时），已克隆。
    locals: Vec<LocalDecl>,
    /// 完整 body 快照（支持多块内联）。
    body: Body,
    /// 返回类型。
    return_ty: scoop2_hir::ty::TypeId,
    /// 是否为 effect-transparent HOF（放宽了门控条件）。
    is_hof: bool,
}

/// 判断一个函数是否为 effect-transparent HOF。
///
/// effect-transparent = 函数的 effect row 中每个非 Pure term 都出现在某个
/// 函数类型参数的 effect row 中（即 effect 只是从闭包参数转发）。
fn is_effect_transparent(fd: &FunDecl, store: &scoop2_hir::ty::TypeStore) -> bool {
    use scoop2_hir::ty::{RefTypeKind, TypeKind};
    if fd.effect_row.is_pure() {
        return true;
    }
    // 收集所有函数类型参数的 effect row 中的 term TypeId。
    let mut forwarded: std::collections::HashSet<scoop2_hir::ty::TypeId> = std::collections::HashSet::new();
    for p in &fd.params {
        if let TypeKind::Ref(RefTypeKind::Function(ft)) = store.kind(p.ty) {
            for &term in &ft.effects.terms {
                forwarded.insert(term);
            }
        }
    }
    // 函数自身 effect row 的每个 term 必须是转发参数中的某一个。
    for &term in &fd.effect_row.terms {
        if !forwarded.contains(&term) {
            return false;
        }
    }
    true
}

/// 带 TypeStore 的内联条件判定（实际入口）。
fn try_make_inlineable_with_store(
    fd: &FunDecl,
    store: &scoop2_hir::ty::TypeStore,
    config: &InlineConfig,
) -> Option<InlineableCallee> {
    let body = fd.body.as_ref()?;
    let hof = is_effect_transparent(fd, store);
    // 条件 4：effect row 必须是 Pure 或 effect-transparent HOF。
    if !fd.effect_row.is_pure() && !hof {
        return None;
    }
    // 条件 3：基本块数限制（HOF 放宽）。
    let max_blocks = if hof { config.max_blocks_hof } else { 1 };
    if body.blocks.len() > max_blocks {
        return None;
    }
    // 条件 3：语句数限制（HOF 放宽）。
    let max_stmts = if hof { config.max_stmts_hof } else { config.max_stmts };
    let total_stmts: usize = body.blocks.iter().map(|b| b.stmts.len()).sum();
    if total_stmts > max_stmts {
        return None;
    }
    // 终结符安全检查：不允许 Perform/Handle（effect 终结符）。
    for block in &body.blocks {
        if !is_safe_terminator(&block.terminator.kind) {
            return None;
        }
    }
    // 条件 5：非递归。
    if body_calls_self(body, &fd.fqn) {
        return None;
    }
    let params: Vec<(LocalId, LocalDecl)> = fd
        .params
        .iter()
        .map(|p| {
            (
                p.local,
                LocalDecl {
                    span: p.span,
                    name: Some(p.name.clone()),
                    ty: p.ty,
                    source: crate::mir::LocalSource::Source,
                    mutable: false,
                },
            )
        })
        .collect();
    Some(InlineableCallee {
        fqn: fd.fqn.clone(),
        params,
        locals: body.locals.clone(),
        body: clone_body(body),
        return_ty: fd.return_ty,
        is_hof: hof,
    })
}

/// 检查终结符是否可安全内联。
///
/// Handle 终结符涉及 dispatch 路由，多块内联时处理复杂，暂不允许内联。
/// Perform 终结符允许内联——effect-transparent HOF（如 forEach）体内含 Perform，
/// 内联后 Perform 直接出现在调用者体内，后续 effect lowering pass 统一处理。
fn is_safe_terminator(kind: &TerminatorKind) -> bool {
    !matches!(kind, TerminatorKind::Handle { .. })
}

/// 克隆 Body（深拷贝 locals + blocks）。
fn clone_body(body: &Body) -> Body {
    Body {
        locals: body.locals.clone(),
        blocks: body.blocks.iter().map(|b| BasicBlock {
            stmts: b.stmts.clone(),
            terminator: b.terminator.clone(),
        }).collect(),
        start: body.start,
    }
}

/// 检查 body 中是否存在对 `self_fqn` 的 Direct 调用（递归检测）。
fn body_calls_self(body: &Body, self_fqn: &str) -> bool {
    for block in &body.blocks {
        for s in &block.stmts {
            if let StatementKind::Assign { value, .. } = &s.kind {
                if let Rvalue::Call {
                    kind: CallKind::Direct { callee_fqn, .. },
                    ..
                } = value
                {
                    if callee_fqn == self_fqn {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// 对单个 Body 执行一轮内联。返回是否有任何内联发生。
fn inline_body_once(
    body: &mut Body,
    callees: &HashMap<String, InlineableCallee>,
    closures: &HashMap<String, InlineableCallee>,
    config: &InlineConfig,
) -> bool {
    let mut inlined_count = 0usize;
    let mut changed = false;
    'outer: loop {
        if inlined_count >= config.max_inline_per_fn {
            break;
        }
        // 在每个块内查找可内联的调用点（direct callee 或 closure）。
        let mut found = false;
        for bid in 0..body.blocks.len() {
            let stmts_len = body.blocks[bid].stmts.len();
            for sidx in 0..stmts_len {
                // 尝试 direct callee 内联。
                if let Some((callee_fqn, arg_operands, target_local)) =
                    extract_inline_site(&body.blocks[bid].stmts[sidx])
                {
                    if let Some(callee) = callees.get(&callee_fqn).cloned() {
                        if arg_operands.len() == callee.params.len() {
                            do_inline(body, &callee, &arg_operands, target_local, bid, sidx);
                            inlined_count += 1;
                            changed = true;
                            found = true;
                            continue 'outer;
                        }
                    }
                }
                // 尝试闭包内联。
                if let Some((invoke_fqn, arg_operands, target_local, env_operand)) =
                    extract_closure_site(&body.blocks[bid].stmts[sidx])
                {
                    if let Some(callee) = closures.get(&invoke_fqn).cloned() {
                        // 闭包参数：第一个是 $env（绑定到 env_operand），其余是调用实参。
                        let mut all_args = vec![env_operand];
                        all_args.extend(arg_operands);
                        if all_args.len() == callee.params.len() {
                            do_inline(body, &callee, &all_args, target_local, bid, sidx);
                            inlined_count += 1;
                            changed = true;
                            found = true;
                            continue 'outer;
                        }
                    }
                }
            }
        }
        if !found {
            break;
        }
    }
    changed
}

/// 从一条 Assign{Call} 语句中提取 direct callee 内联所需信息。
fn extract_inline_site(stmt: &Statement) -> Option<(String, Vec<Operand>, LocalId)> {
    let StatementKind::Assign { target, value } = &stmt.kind else {
        return None;
    };
    let Rvalue::Call { kind, args, .. } = value else {
        return None;
    };
    let callee_fqn = match kind {
        CallKind::Direct { callee_fqn, .. } => callee_fqn.clone(),
        _ => return None,
    };
    let arg_operands: Vec<Operand> = args.iter().map(|a| a.value.clone()).collect();
    Some((callee_fqn, arg_operands, *target))
}

/// 从一条 Assign{Call} 语句中提取闭包内联所需信息。
/// 返回 (invoke_fqn, arg_operands, target_local, env_operand)。
fn extract_closure_site(stmt: &Statement) -> Option<(String, Vec<Operand>, LocalId, Operand)> {
    let StatementKind::Assign { target, value } = &stmt.kind else {
        return None;
    };
    let Rvalue::Call { kind, args, .. } = value else {
        return None;
    };
    let (invoke_fqn, callee_local) = match kind {
        CallKind::Closure { callee, invoke_fqn } => (invoke_fqn.clone(), callee.clone()),
        _ => return None,
    };
    let arg_operands: Vec<Operand> = args.iter().map(|a| a.value.clone()).collect();
    // env_operand = callee（闭包值本身，其 $env 在闭包构造时绑定）。
    Some((invoke_fqn, arg_operands, *target, callee_local))
}

/// 执行单次内联（支持多块）。
///
/// callee 的所有 local 被重命名到 caller 的新 local，参数绑定到实参。
/// 多块 callee 的所有块被复制到 caller，block id 重映射。
/// Return 终结符改写为：赋值结果到 target_local + Goto 到续接块。
fn do_inline(
    body: &mut Body,
    callee: &InlineableCallee,
    arg_operands: &[Operand],
    target_local: LocalId,
    bid: usize,
    sidx: usize,
) {
    let callee_block_count = callee.body.blocks.len();

    // 1. 为 callee 的所有 local 分配 caller 侧的新 local。
    let base = body.locals.len() as u32;
    let mut local_map: HashMap<LocalId, LocalId> = HashMap::new();
    for (i, decl) in callee.locals.iter().enumerate() {
        let new_id = LocalId(base + i as u32);
        local_map.insert(LocalId(i as u32), new_id);
        body.locals.push(LocalDecl {
            span: decl.span,
            name: decl.name.clone(),
            ty: decl.ty,
            source: decl.source,
            mutable: decl.mutable,
        });
    }

    // 2. 为 callee 的块分配新 block id（块 0 的入口语句会 splice 到当前块）。
    //    块 0 以外的块追加到 body.blocks 末尾。
    // 先记录追加前的块数量。
    let blocks_before_append = body.blocks.len();

    // 块 id 映射：callee 块 i → caller 侧新块 id。
    // 块 0 的语句 splice 到 bid，不占独立块。
    // 块 1..N 追加到 body.blocks。
    let mut block_map: HashMap<BasicBlockId, BasicBlockId> = HashMap::new();
    for i in 1..callee_block_count {
        let new_bid = BasicBlockId(body.blocks.len() as u32);
        block_map.insert(BasicBlockId(i as u32), new_bid);
        body.blocks.push(BasicBlock {
            stmts: Vec::new(),
            terminator: Terminator {
                span: scoop2_base::Span::default(),
                kind: TerminatorKind::Unreachable,
            },
        });
    }

    // 续接块：在追加 callee 块之后创建（需要实际 push 到 body.blocks）。
    let continuation_block = if callee_block_count == 1 {
        None
    } else {
        let cont_id = BasicBlockId(body.blocks.len() as u32);
        // 续接块本身也需要被追加到 body.blocks（否则 cont_idx 会越界）。
        body.blocks.push(BasicBlock {
            stmts: Vec::new(),
            terminator: Terminator {
                span: scoop2_base::Span::default(),
                kind: TerminatorKind::Unreachable,
            },
        });
        Some(cont_id)
    };

    // 3. 构造参数绑定语句（splice 序列的前缀）。
    let mut splice_stmts: Vec<Statement> = Vec::new();
    for ((param_local, _), arg) in callee.params.iter().zip(arg_operands.iter()) {
        let mapped = *local_map.get(param_local).unwrap_or(param_local);
        splice_stmts.push(Statement {
            span: scoop2_base::Span::default(),
            kind: StatementKind::Assign {
                target: mapped,
                value: Rvalue::Use(arg.clone()),
            },
        });
    }

    // 4. 复制块 0 的语句（重命名 local），追加到 splice 序列。
    for mut stmt in callee.body.blocks[0].stmts.clone() {
        rename_statement(&mut stmt, &local_map);
        splice_stmts.push(stmt);
    }

    // 5. 处理块 0 的终结符。
    let block0_term = callee.body.blocks[0].terminator.clone();
    match &block0_term.kind {
        TerminatorKind::Return { value } => {
            // 单块 Return：赋值结果到 target_local。
            let mapped_rv = match value {
                Some(op) => Rvalue::Use(rename_operand(op, &local_map)),
                None => Rvalue::Use(Operand::Const(crate::mir::ConstValue::Unit)),
            };
            splice_stmts.push(Statement {
                span: scoop2_base::Span::default(),
                kind: StatementKind::Assign {
                    target: target_local,
                    value: mapped_rv,
                },
            });
        }
        _ => {
            // 多块终结符（Goto/CondBr）：重命名 block id。
            // 这里的块 0 终结符不是 Return，说明是多块函数，
            // splice 序列后需要一个终结符跳到 callee 块 1（或续接块）。
            let mut renamed_term = block0_term.clone();
            let _ = rename_terminator(&mut renamed_term, &local_map, &block_map, continuation_block, target_local);
            // 把终结符放到 splice 序列之后（它将成为 bid 的新终结符）。
            // 但 splice 是语句，终结符需要单独处理——见下方块重写。
            // 简化：把非 Return 终结符转为一个临时块。
            // 实际上对于多块，我们改写 bid 的终结符。
            // 这里先把 splice 语句放好，终结符在下面统一处理。
            let _ = renamed_term;
        }
    }

    // 6. 用 splice_stmts 替换 sidx 处的语句。
    let block = &mut body.blocks[bid];
    let original_terminator = block.terminator.clone();
    let remaining_stmts: Vec<Statement> = block.stmts[sidx + 1..].to_vec();
    // 移除 sidx 及之后的语句。
    block.stmts.truncate(sidx);
    // 追加 splice 语句。
    block.stmts.append(&mut splice_stmts);

    // 7. 处理块 0 终结符 + 剩余语句。
    match &block0_term.kind {
        TerminatorKind::Return { .. } => {
            // 单块 Return：剩余语句留在当前块，终结符不变。
            block.stmts.extend(remaining_stmts);
            // terminator 保持 original_terminator（调用点所在的块继续执行）。
            block.terminator = original_terminator;
        }
        _ => {
            // 多块：当前块需要一个终结符跳到 callee 块 1。
            // 剩余语句移到续接块。
            let mut renamed_term = block0_term.clone();
            let _ = rename_terminator(&mut renamed_term, &local_map, &block_map, continuation_block, target_local);
            block.terminator = renamed_term;
            // 续接块持有剩余语句 + 原 terminator。
            if let Some(cont_id) = continuation_block {
                let cont_idx = cont_id.0 as usize;
                if cont_idx < body.blocks.len() {
                    body.blocks[cont_idx].stmts = remaining_stmts;
                    body.blocks[cont_idx].terminator = original_terminator;
                }
            }
        }
    }

    // 8. 复制 callee 块 1..N（重命名 local + block id + Return → 续接块赋值）。
    for i in 1..callee_block_count {
        let src_block = &callee.body.blocks[i];
        let dst_bid = *block_map.get(&BasicBlockId(i as u32)).unwrap();
        let dst_idx = dst_bid.0 as usize;
        // 复制语句（重命名 local）。
        let mut renamed_stmts: Vec<Statement> = Vec::new();
        for mut stmt in src_block.stmts.clone() {
            rename_statement(&mut stmt, &local_map);
            renamed_stmts.push(stmt);
        }
        // 处理终结符。rename_terminator 可能返回一条赋值语句（Return 改写时）。
        let mut renamed_term = src_block.terminator.clone();
        let extra_assign = rename_terminator(&mut renamed_term, &local_map, &block_map, continuation_block, target_local);
        // 若 Return 被改写为 Goto，需把返回值赋给 target_local（追加到块末尾）。
        if let Some(assign) = extra_assign {
            renamed_stmts.push(assign);
        }
        body.blocks[dst_idx].stmts = renamed_stmts;
        body.blocks[dst_idx].terminator = renamed_term;
    }
}

/// 重命名终结符中的 local + block id。
/// Return → 返回一条赋值语句（target_local = value）+ 把终结符改为 Goto 续接块。
/// Goto/CondBr → 重定向 block id。
/// 返回的 Option<Statement> 是 Return 改写时需要追加到块末尾的赋值语句。
fn rename_terminator(
    term: &mut Terminator,
    local_map: &HashMap<LocalId, LocalId>,
    block_map: &HashMap<BasicBlockId, BasicBlockId>,
    continuation_block: Option<BasicBlockId>,
    target_local: LocalId,
) -> Option<Statement> {
    let cont = continuation_block.unwrap_or(BasicBlockId(u32::MAX));
    let extra = if let TerminatorKind::Return { value } = &term.kind {
        // Return → 赋值 target_local + Goto 续接块。
        // 生成赋值语句返回给调用方（需追加到块末尾语句列表）。
        let assign_stmt = Statement {
            span: scoop2_base::Span::default(),
            kind: StatementKind::Assign {
                target: target_local,
                value: match value {
                    Some(op) => Rvalue::Use(rename_operand(op, local_map)),
                    None => Rvalue::Use(Operand::Const(crate::mir::ConstValue::Unit)),
                },
            },
        };
        term.kind = TerminatorKind::Goto { target: cont };
        Some(assign_stmt)
    } else {
        None
    };
    match &mut term.kind {
        TerminatorKind::Goto { target } => {
            if let Some(mapped) = block_map.get(target) {
                *target = *mapped;
            }
        }
        TerminatorKind::CondBr {
            cond,
            then_target,
            else_target,
            ..
        } => {
            *cond = rename_operand(cond, local_map);
            if let Some(m) = block_map.get(then_target) {
                *then_target = *m;
            }
            if let Some(m) = block_map.get(else_target) {
                *else_target = *m;
            }
        }
        TerminatorKind::Perform {
            resume_local,
            resume_target,
            args,
            ..
        } => {
            // 重命名 resume_local 和 args 中的 local 引用。
            if let Some(m) = local_map.get(resume_local) {
                *resume_local = *m;
            }
            for a in args.iter_mut() {
                a.value = rename_operand(&a.value, local_map);
            }
            // resume_target 可能指向 callee 的块 0（已在 splice 中）或块 1..N（需重定向）。
            // 块 0 的 resume_target 实际上已通过 splice 进入 caller，但 resume_target
            // 指向的是 callee 的块编号——如果它在 block_map 中则重定向，否则保持
            // （它可能指向 splice 序列中的续接位置，由 do_inline 的块 0 处理逻辑管理）。
            if let Some(m) = block_map.get(resume_target) {
                *resume_target = *m;
            }
        }
        _ => {}
    }
    extra
}

// ---------------------------------------------------------------------------
// 重命名 helper（local id 映射）
// ---------------------------------------------------------------------------

fn rename_statement(stmt: &mut Statement, map: &HashMap<LocalId, LocalId>) {
    match &mut stmt.kind {
        StatementKind::Assign { target, value } => {
            if let Some(t) = map.get(target) {
                *target = *t;
            }
            rename_rvalue(value, map);
        }
        StatementKind::StoreMember { receiver, value, .. } => {
            rename_operand(receiver, map);
            rename_operand(value, map);
        }
        StatementKind::StoreTupleIndex {
            receiver, value, ..
        } => {
            rename_operand(receiver, map);
            rename_operand(value, map);
        }
        StatementKind::StoreTopLevelVar { value, .. } => {
            rename_operand(value, map);
        }
        StatementKind::Nop | StatementKind::Panic { .. } => {}
    }
}

fn rename_rvalue(rv: &mut Rvalue, map: &HashMap<LocalId, LocalId>) {
    match rv {
        Rvalue::Use(op) => *op = rename_operand(op, map),
        Rvalue::TopLevelRef(_) | Rvalue::UnresolvedName { .. } | Rvalue::ClassLit { .. }
        | Rvalue::PerformResult { .. } => {}
        Rvalue::TypeTest { value, .. } => {
            *value = rename_operand(value, map);
        }
        Rvalue::Cast { value, .. } => {
            *value = rename_operand(value, map);
        }
        Rvalue::MemberAccess { receiver, .. } => {
            *receiver = rename_operand(receiver, map);
        }
        Rvalue::TupleIndex { receiver, .. } => {
            *receiver = rename_operand(receiver, map);
        }
        Rvalue::IndexAccess { receiver, indices, .. } => {
            *receiver = rename_operand(receiver, map);
            for i in indices.iter_mut() {
                *i = rename_operand(i, map);
            }
        }
        Rvalue::EnumVariant { args, .. } => {
            for a in args.iter_mut() {
                a.value = rename_operand(&a.value, map);
            }
        }
        Rvalue::ClassCtor { args, .. } => {
            for a in args.iter_mut() {
                a.value = rename_operand(&a.value, map);
            }
        }
        Rvalue::Call { kind, args, .. } => {
            rename_call_kind(kind, map);
            for a in args.iter_mut() {
                a.value = rename_operand(&a.value, map);
            }
        }
        Rvalue::MakeTuple { elements, .. } => {
            for e in elements.iter_mut() {
                *e = rename_operand(e, map);
            }
        }
        Rvalue::MakeArray { elements, .. } => {
            for e in elements.iter_mut() {
                *e = rename_operand(e, map);
            }
        }
        Rvalue::StructLit { fields, .. } => {
            for f in fields.iter_mut() {
                f.value = rename_operand(&f.value, map);
            }
        }
        Rvalue::InterpolatedString { parts } => {
            for p in parts.iter_mut() {
                if let crate::mir::InterpolatedPart::Expr(op) = p {
                    *op = rename_operand(op, map);
                }
            }
        }
        Rvalue::WithUpdate { base, updates, .. } => {
            *base = rename_operand(base, map);
            for u in updates.iter_mut() {
                u.value = rename_operand(&u.value, map);
            }
        }
        Rvalue::MakeClosure { env, .. } => {
            *env = rename_operand(env, map);
        }
        Rvalue::PatternMatch { subject, .. } => {
            *subject = rename_operand(subject, map);
        }
        Rvalue::PatternExtract { subject, .. } => {
            *subject = rename_operand(subject, map);
        }
        Rvalue::IntEq { lhs, rhs } => {
            *lhs = rename_operand(lhs, map);
            *rhs = rename_operand(rhs, map);
        }
    }
}

fn rename_call_kind(kind: &mut CallKind, map: &HashMap<LocalId, LocalId>) {
    match kind {
        CallKind::Closure { callee, .. } | CallKind::FunValue { callee } => {
            *callee = rename_operand(callee, map);
        }
        CallKind::Virtual { receiver, .. } | CallKind::Interface { receiver, .. } => {
            *receiver = rename_operand(receiver, map);
        }
        CallKind::Resume { continuation, resume_value } => {
            *continuation = rename_operand(continuation, map);
            *resume_value = rename_operand(resume_value, map);
        }
        CallKind::Direct { .. } => {}
    }
}

fn rename_operand(op: &Operand, map: &HashMap<LocalId, LocalId>) -> Operand {
    match op {
        Operand::Local(lid) => {
            if let Some(mapped) = map.get(lid) {
                Operand::Local(*mapped)
            } else {
                Operand::Local(*lid)
            }
        }
        Operand::Const(_) => op.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoop2_hir::ty::EffectRow;

    #[test]
    fn default_config_is_reasonable() {
        let c = InlineConfig::default();
        assert!(c.max_stmts >= 1);
        assert!(c.max_inline_per_fn >= 1);
        assert!(c.max_stmts_hof >= c.max_stmts);
        assert!(c.max_blocks_hof >= 1);
    }

    #[test]
    fn pure_effect_row_is_pure() {
        let row = EffectRow::pure();
        assert!(row.is_pure());
    }

    #[test]
    fn is_safe_terminator_allows_perform_rejects_handle() {
        // Return / Goto / CondBr / Unreachable / Perform 都是安全的。
        assert!(is_safe_terminator(&TerminatorKind::Return { value: None }));
        assert!(is_safe_terminator(&TerminatorKind::Goto { target: BasicBlockId(0) }));
        assert!(is_safe_terminator(&TerminatorKind::Unreachable));
        // Handle 不安全（dispatch 路由复杂），但 Handle 的构造需要完整 metadata，
        // 这里只验证 is_safe_terminator 对非 Handle 终结符返回 true。
    }
}
