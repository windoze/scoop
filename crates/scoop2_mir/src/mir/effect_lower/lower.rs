//! Effect lowering 主 pass。
//!
//! 对每个含 effect 结构（Perform/Handle/Resume）的函数体做完整转换，
//! 消除所有 effect 相关的控制流结构（Perform/Handle/Resume 终结符）。
//!
//! ## 转换策略
//!
//! ### 阶段 1：Handle dispatch 消除
//! Handle 终结符 → goto body_target；被捕获的 Perform → 赋值 args 到 binder locals + goto arm。
//!
//! ### 阶段 2：EffectStep 变换（未捕获 Perform + Resume）
//! 函数 `f(args) -> R / Effects` 变换为状态机：
//!
//! 1. **Frame tuple local**：分配一个 tuple local 作为 frame，含 state 字段（Int）
//!    + 所有跨 Perform 存活的 live locals。frame 在函数入口初始化。
//!
//! 2. **Perform 重写**：每个未捕获的 Perform 变为：
//!    - StoreTupleIndex：把 live locals 保存到 frame tuple 的对应索引
//!    - StoreTupleIndex：设置 frame.state = <resume 编号>
//!    - 构造 Step case（EnumVariant）赋值到 resume_local
//!    - Return(resume_local)（函数返回 Step 值）
//!
//! 3. **Resume 重写**：`k.resume(v)` 变为 Direct 调用 `scoop.core.Continuation.resume`，
//!    传递 continuation + resume value 作为参数。
//!
//! 4. **State dispatch 入口**：在函数入口前插入分发块，用 PatternMatch 检查
//!    frame.state 的值，跳转到对应的 resume 续点。

use std::collections::{HashMap, HashSet};

use scoop2_base::Interner;
use scoop2_hir::ty::{TypeId, TypeStore};

use crate::mir::{
    BasicBlockId, Body, CallKind, Item, LocalDecl, LocalId, Module, Operand, Rvalue, Statement,
    StatementKind, Terminator, TerminatorKind,
    transport::{
        AggregateTransportKind, AggregateTransportMetadata, CallTransportMetadata,
        MemberAccessMetadata, MirTransportKind, StoredContinuationRoutePublication,
    },
};

use super::analyze;

/// 一个函数的效应行为分类结果。
#[derive(Clone, Debug, Default)]
struct FnEffectClass {
    /// 是否为 EffectStep 函数（Step ABI：返回 Step tagged union）。
    is_effect_step: bool,
    /// 向外逸出的 effect 操作（op_fqn → payload 类型，按首次出现序）。
    /// 来源：本函数词法上未被本地 handle 捕获的 Perform + 调用的 EffectStep
    /// 函数逸出的、未被本地 handle 覆盖的操作（沿调用图定点传播）。
    outward_ops: Vec<(String, TypeId)>,
    /// 函数值 / 闭包间接调用站点：(block_idx, stmt_idx, 效果行展开的
    /// (规范 op_fqn, payload_ty))。供 collect_call_chain_sites 合成站点级
    /// Step 变体表（callee 静态未知，Step 形状取自函数类型的效果行）。
    fun_sites: Vec<(usize, usize, Vec<(String, TypeId)>)>,
}

/// 对整个 Module 执行 effect lowering。
pub fn lower_effects(module: &mut Module, interner: &Interner) {
    // 直接使用传入的 interner（不可变）。合成 Step 类型的 FQN 使用已 intern 的字符串
    // （函数自身 FQN + effect op FQN），不创建新字符串。
    // 这确保 Symbol 在后续 verify/dump 中可被原 interner 解析。
    // 阶段 0：效应行为分类（定点迭代，确定哪些函数是 EffectStep 及其 outward ops）。
    let classes = classify_module(module, interner);
    // EffectStep 函数 FQN 集合 + 各函数规范化 outward ops（call-chain 站点收集用：
    // 识别 EffectStep → EffectStep 的 Direct 调用边）。
    let effect_step_fqns: HashSet<String> = classes
        .iter()
        .filter(|(_, c)| c.is_effect_step)
        .map(|(f, _)| f.clone())
        .collect();
    let canon_outward: HashMap<String, Vec<(String, TypeId)>> = classes
        .iter()
        .map(|(f, c)| {
            (
                f.clone(),
                c.outward_ops
                    .iter()
                    .map(|(op, ty)| (canonical_op_fqn(module, op), *ty))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    // handle arm op 的规范化映射（短名 → 全限定名），与 adapt_calls 内的一致。
    let mut canon_map: HashMap<String, String> = HashMap::new();
    for item in &module.items {
        let body = match item {
            Item::Fun(fd) => fd.body.as_ref(),
            Item::Initializer(ir) => Some(&ir.body),
            _ => None,
        };
        if let Some(body) = body {
            for h in collect_handle_info(body) {
                for op in h.arm_dispatch.keys() {
                    if !canon_map.contains_key(op) {
                        canon_map.insert(op.clone(), canonical_op_fqn(module, op));
                    }
                }
            }
        }
    }
    // 阶段 A：Handle dispatch 消除 + EffectStep body 变换。
    // 每个 body 只变换一次：lower_body 一次性消除全部 Perform/Handle，
    // 而 Resume 调用按设计原样保留（流向 LIR/codegen），因此变换后的 body
    // 仍会被 has_effect_structures 判定为含 effect 结构——若按"直到无变化"
    // 循环会对同一 body 反复套娃变换（frame 越包越大），永不终止。
    let need_process: Vec<usize> = module
        .items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            let has_effects = match item {
                Item::Fun(fd) => {
                    fd.body
                        .as_ref()
                        .map_or(false, analyze::has_effect_structures)
                        || classes.get(&fd.fqn).map_or(false, |c| c.is_effect_step)
                }
                Item::Initializer(ir) => analyze::has_effect_structures(&ir.body),
                _ => false,
            };
            if has_effects { Some(i) } else { None }
        })
        .collect();
    for idx in need_process {
        let fqn = match &module.items[idx] {
            Item::Fun(fd) => fd.fqn.clone(),
            Item::Initializer(ir) => ir.fqn.clone(),
            _ => continue,
        };
        let outward: Vec<(String, TypeId)> = classes
            .get(&fqn)
            .map(|c| {
                c.outward_ops
                    .iter()
                    .map(|(op, ty)| (canonical_op_fqn(module, op), *ty))
                    .collect()
            })
            .unwrap_or_default();
        let params: Vec<(LocalId, TypeId)> = match &module.items[idx] {
            Item::Fun(fd) => fd.params.iter().map(|p| (p.local, p.ty)).collect(),
            _ => Vec::new(),
        };
        let body = match &mut module.items[idx] {
            Item::Fun(fd) => fd.body.as_mut(),
            Item::Initializer(ir) => Some(&mut ir.body),
            _ => None,
        };
        if let Some(body) = body {
            let is_effect_step = classes.get(&fqn).map_or(false, |c| c.is_effect_step);
            if analyze::has_effect_structures(body) || is_effect_step {
                let fun_sites = classes
                    .get(&fqn)
                    .map(|c| c.fun_sites.clone())
                    .unwrap_or_default();
                let effect_abi = lower_body(
                    body,
                    &mut module.types,
                    interner,
                    &fqn,
                    &outward,
                    &params,
                    &effect_step_fqns,
                    &canon_outward,
                    &canon_map,
                    &fun_sites,
                );
                // 如果 body 变换为 EffectStep，设置 FunDecl 的 effect_abi + 更新 return_ty。
                if let Some(abi) = effect_abi.clone() {
                    if let Item::Fun(fd) = &mut module.items[idx] {
                        fd.effect_abi = Some(abi.clone());
                        // 更新 return_ty 为 Step 类型。
                        let orig_return_ty = fd.return_ty;
                        fd.return_ty = abi.step_ty;
                        // 更新 step_variants 中 Complete 变体的 payload_ty 为原始返回类型。
                        if let Some(ref mut abi_field) = fd.effect_abi {
                            for v in &mut abi_field.step_variants {
                                if v.is_complete {
                                    v.payload_ty = orig_return_ty;
                                }
                            }
                        }
                    }
                    // 向 Module 添加 Step enum 的 MetadataRoot 声明。
                    add_step_enum_metadata(module, &fqn, &effect_abi.unwrap());
                    // 向 Module 添加 Continuation struct 的 MetadataRoot 声明。
                    add_continuation_struct_metadata(module, &fqn);
                }
            }
        }
    }
    // 阶段 B：调用适配——处理对 EffectStep 函数的调用 + 消除剩余 Handle 终结符。
    adapt_calls(module, interner);
}

// =========================================================================
// 阶段 0：效应行为分类
// =========================================================================

/// 全模块效应行为分类：确定每个函数的 is_effect_step + outward_ops。
///
/// 基例：函数体内有未被本地 handle 捕获的 Perform（其 op 逸出）或 Resume 调用。
/// 归纳：函数调用了 EffectStep 函数，且调用点不在能覆盖被调方全部 outward ops 的
/// handle 区域内 → 未覆盖的 op 逸出，该函数也成为 EffectStep。迭代至定点。
fn classify_module(module: &mut Module, interner: &Interner) -> HashMap<String, FnEffectClass> {
    // 收集每个函数的 fqn + body 引用信息。
    struct FnFacts {
        fqn: String,
        /// (block_idx, op_fqn, payload_ty) — 词法 Perform 站点。
        performs: Vec<(usize, String, TypeId)>,
        has_resume: bool,
        /// (block_idx, callee_fqn) — Direct 调用点。
        calls: Vec<(usize, String)>,
        /// (block_idx, stmt_idx, callee 函数值类型) — FunValue/Closure 间接调用点
        /// （callee 静态未知，效果集取自函数值类型的效果行，随后展开）。
        fun_call_tys: Vec<(usize, usize, TypeId)>,
        /// handle 区域：(区域 block 集合, 区域覆盖的 op 集合, 其中 escape arm 覆盖的 op 集合)。
        handle_regions: Vec<(HashSet<usize>, HashSet<String>, HashSet<String>)>,
    }
    let mut facts: Vec<FnFacts> = Vec::new();
    for item in &module.items {
        let (fqn, body) = match item {
            Item::Fun(fd) => (fd.fqn.clone(), fd.body.as_ref()),
            Item::Initializer(ir) => (ir.fqn.clone(), Some(&ir.body)),
            _ => continue,
        };
        let Some(body) = body else { continue };
        let handles = collect_handle_info(body);
        let mut handle_regions = Vec::with_capacity(handles.len());
        for h in &handles {
            let region = find_handle_body_region(body, h);
            let ops: HashSet<String> = h.arm_dispatch.keys().cloned().collect();
            let escape_ops: HashSet<String> = h
                .arm_dispatch
                .iter()
                .filter(|(_, r)| r.continuation_local.is_some())
                .map(|(op, _)| op.clone())
                .collect();
            handle_regions.push((region, ops, escape_ops));
        }
        let mut performs = Vec::new();
        for (i, block) in body.blocks.iter().enumerate() {
            if let TerminatorKind::Perform {
                op_fqn, metadata, ..
            } = &block.terminator.kind
            {
                let payload_ty = metadata
                    .payload_tuple_ty
                    .unwrap_or_else(|| module.types.unit());
                performs.push((i, op_fqn.clone(), payload_ty));
            }
        }
        let mut calls = Vec::new();
        let mut fun_call_tys = Vec::new();
        for (i, block) in body.blocks.iter().enumerate() {
            for (si, stmt) in block.stmts.iter().enumerate() {
                if let StatementKind::Assign { value, .. } = &stmt.kind {
                    match value {
                        Rvalue::Call {
                            kind: CallKind::Direct { callee_fqn, .. },
                            ..
                        } => {
                            calls.push((i, callee_fqn.clone()));
                        }
                        Rvalue::Call {
                            kind: CallKind::FunValue { callee } | CallKind::Closure { callee, .. },
                            ..
                        } => {
                            // callee local 的函数类型效果行决定该站点可能的挂起集。
                            if let Operand::Local(lid) = callee
                                && let Some(decl) = body.locals.get(lid.0 as usize)
                            {
                                fun_call_tys.push((i, si, decl.ty));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        facts.push(FnFacts {
            fqn,
            performs,
            has_resume: body_has_resume(body),
            calls,
            fun_call_tys,
            handle_regions,
        });
    }
    // 全模块词法 Perform 预扫描：规范 op fqn → payload 类型（首次出现胜出）。
    // 函数值效果行的 op payload 类型由此取得——能挂起的 op 必有某个函数词法
    // perform 它；行内无词法 Perform 的 op 无法挂起，展开时跳过。
    let mut lexical_ops: HashMap<String, TypeId> = HashMap::new();
    for f in &facts {
        for (_, op, payload_ty) in &f.performs {
            let canon = canonical_op_fqn(module, op);
            lexical_ops.entry(canon).or_insert(*payload_ty);
        }
    }
    // 展开每个间接调用点的效果行：(block, stmt, [(规范 op, payload_ty)])。
    let expand_row = |fn_ty: TypeId| -> Vec<(String, TypeId)> {
        let scoop2_hir::ty::TypeKind::Ref(scoop2_hir::ty::RefTypeKind::Function(ft)) =
            module.types.kind(fn_ty)
        else {
            return Vec::new();
        };
        let mut ops: Vec<(String, TypeId)> = Vec::new();
        for term in &ft.effects.terms {
            let scoop2_hir::ty::TypeKind::Ref(scoop2_hir::ty::RefTypeKind::Nominal(n)) =
                module.types.kind(*term)
            else {
                continue;
            };
            let effect_fqn = interner.resolve(n.fqn);
            for (op, payload_ty) in &lexical_ops {
                if op.as_str() == effect_fqn || op.starts_with(&format!("{effect_fqn}.")) {
                    if !ops.iter().any(|(o, _)| o == op) {
                        ops.push((op.clone(), *payload_ty));
                    }
                }
            }
        }
        ops
    };
    let fun_sites_by_fn: HashMap<String, Vec<(usize, usize, Vec<(String, TypeId)>)>> = facts
        .iter()
        .map(|f| {
            let sites = f
                .fun_call_tys
                .iter()
                .filter_map(|(bi, si, ty)| {
                    let ops = expand_row(*ty);
                    if ops.is_empty() {
                        None
                    } else {
                        Some((*bi, *si, ops))
                    }
                })
                .collect();
            (f.fqn.clone(), sites)
        })
        .collect();
    // 初始化分类表。
    let mut classes: HashMap<String, FnEffectClass> = HashMap::new();
    for f in &facts {
        classes.insert(f.fqn.clone(), FnEffectClass::default());
    }
    // 间接调用站点落表（供 lower_body 的 call-chain 站点收集使用）。
    for (fqn, sites) in fun_sites_by_fn {
        if let Some(cls) = classes.get_mut(&fqn) {
            cls.fun_sites = sites;
        }
    }
    // 基例：未被本地捕获的 Perform 的 op 逸出。
    for f in &facts {
        let mut outward: Vec<(String, TypeId)> = Vec::new();
        for (bi, op, payload_ty) in &f.performs {
            let covered = f
                .handle_regions
                .iter()
                .any(|(region, ops, _)| region.contains(bi) && ops.contains(op));
            if !covered && !outward.iter().any(|(o, _)| o == op) {
                outward.push((op.clone(), *payload_ty));
            }
        }
        let cls = classes.get_mut(&f.fqn).unwrap();
        cls.outward_ops = outward;
        cls.is_effect_step = !cls.outward_ops.is_empty() || f.has_resume;
    }
    // 基例补充：间接调用（FunValue/Closure）效果行中未被本地 handle 覆盖的
    // op 同样逸出——callee 静态未知，效果集按函数类型的效果行保守取得。
    for f in &facts {
        let Some(cls) = classes.get_mut(&f.fqn) else {
            continue;
        };
        for (bi, _, row_ops) in &cls.fun_sites.clone() {
            for (op, payload_ty) in row_ops {
                let covered = f.handle_regions.iter().any(|(region, ops, _)| {
                    region.contains(bi)
                        && (ops.contains(op)
                            || ops.iter().any(|a| canonical_op_fqn(module, a) == *op))
                });
                if !covered && !cls.outward_ops.iter().any(|(o, _)| o == op) {
                    cls.outward_ops.push((op.clone(), *payload_ty));
                }
            }
        }
        if !cls.is_effect_step && !cls.outward_ops.is_empty() {
            cls.is_effect_step = true;
        }
    }
    // 基例补充：escape arm 捕获的 EffectStep 调用点（case b）——callee 的挂起
    // 被本函数 escape arm（`, k ->`）捕获时，本函数必须变换为 EffectStep
    // （构造 continuation 需要本函数的 frame + step 函数指针），即使该 op
    // 不向外逸出、arm 内也没有 Resume 调用。
    for f in &facts {
        if classes.get(&f.fqn).map_or(false, |c| c.is_effect_step) {
            continue;
        }
        let mut has_escape_call = false;
        for (bi, callee) in &f.calls {
            let callee_cls = match classes.get(callee) {
                Some(c) => c,
                None => continue,
            };
            // callee 是 EffectStep 且其任一 outward op 被本函数某 handle 区域
            // 的 escape arm 覆盖（调用点在该区域内）。
            if !callee_cls.is_effect_step {
                continue;
            }
            for (region, _, escape_ops) in &f.handle_regions {
                if !region.contains(bi) {
                    continue;
                }
                if callee_cls
                    .outward_ops
                    .iter()
                    .any(|(op, _)| escape_ops.contains(op))
                {
                    has_escape_call = true;
                    break;
                }
            }
            if has_escape_call {
                break;
            }
        }
        if has_escape_call {
            classes.get_mut(&f.fqn).unwrap().is_effect_step = true;
        }
    }
    // 基例补充（间接调用版）：FunValue/Closure 站点的效果行 op 被本函数
    // escape arm 覆盖时，本函数同样必须变换为 EffectStep（构造 continuation
    // 需要本函数的 frame + step 函数指针）。
    for f in &facts {
        if classes.get(&f.fqn).map_or(false, |c| c.is_effect_step) {
            continue;
        }
        let fun_sites = &classes.get(&f.fqn).unwrap().fun_sites;
        let mut has_escape_call = false;
        for (bi, _, row_ops) in fun_sites {
            for (region, _, escape_ops) in &f.handle_regions {
                if !region.contains(bi) {
                    continue;
                }
                if row_ops.iter().any(|(op, _)| {
                    escape_ops.contains(op)
                        || escape_ops
                            .iter()
                            .any(|a| canonical_op_fqn(module, a) == *op)
                }) {
                    has_escape_call = true;
                    break;
                }
            }
            if has_escape_call {
                break;
            }
        }
        if has_escape_call {
            classes.get_mut(&f.fqn).unwrap().is_effect_step = true;
        }
    }
    // 归纳：沿调用边传播未覆盖的 outward ops，迭代至定点。
    let mut changed = true;
    while changed {
        changed = false;
        for f in &facts {
            // 收集本轮要加入的 (op, payload_ty)。
            let mut additions: Vec<(String, TypeId)> = Vec::new();
            for (bi, callee) in &f.calls {
                let callee_outward = match classes.get(callee) {
                    Some(c) if c.is_effect_step => c.outward_ops.clone(),
                    _ => continue,
                };
                for (op, payload_ty) in callee_outward {
                    let covered = f
                        .handle_regions
                        .iter()
                        .any(|(region, ops, _)| region.contains(bi) && ops.contains(&op));
                    if !covered {
                        additions.push((op, payload_ty));
                    }
                }
            }
            if additions.is_empty() {
                continue;
            }
            let cls = classes.get_mut(&f.fqn).unwrap();
            for (op, payload_ty) in additions {
                if !cls.outward_ops.iter().any(|(o, _)| o == &op) {
                    cls.outward_ops.push((op, payload_ty));
                    changed = true;
                }
            }
            if !cls.is_effect_step && !cls.outward_ops.is_empty() {
                cls.is_effect_step = true;
                changed = true;
            }
        }
    }
    classes
}

/// 把短 op_fqn（如 "Edge.visit"）解析为模块中规范的全限定名。
/// 依次尝试：精确匹配 → 后缀匹配（".{op}" 结尾）→ 原样返回。
fn canonical_op_fqn(module: &Module, op_fqn: &str) -> String {
    let mut suffix_hit: Option<&str> = None;
    for item in &module.items {
        let fqn = match item {
            Item::Fun(fd) => Some(fd.fqn.as_str()),
            Item::Metadata(m) => Some(m.fqn.as_str()),
            _ => None,
        };
        let Some(fqn) = fqn else { continue };
        if fqn == op_fqn {
            return fqn.to_string();
        }
        if suffix_hit.is_none() && fqn.len() > op_fqn.len() && fqn.ends_with(op_fqn) {
            let sep = fqn.len() - op_fqn.len() - 1;
            if fqn.as_bytes()[sep] == b'.' {
                suffix_hit = Some(fqn);
            }
        }
    }
    suffix_hit.unwrap_or(op_fqn).to_string()
}

/// 向 Module 添加 Step enum 的 MetadataRoot 声明。
fn add_step_enum_metadata(module: &mut Module, fqn: &str, abi: &crate::mir::EffectStepAbi) {
    let step_fqn = format!("{}$step", fqn);
    // 检查是否已存在（避免重复添加）。
    let exists = module.items.iter().any(|item| {
        if let Item::Metadata(m) = item {
            m.fqn == step_fqn
        } else {
            false
        }
    });
    if !exists {
        module.items.push(Item::Metadata(crate::mir::MetadataRoot {
            span: scoop2_base::Span::default(),
            fqn: step_fqn,
            kind: crate::mir::MetadataKind::Enum,
            file: scoop2_base::FileId(0),
        }));
    }
    let _ = abi;
}

/// 向 Module 添加 Continuation struct 的 MetadataRoot 声明。
/// Continuation 对象作为合成 struct，含 resumed: Bool 字段。
fn add_continuation_struct_metadata(module: &mut Module, fqn: &str) {
    let cont_fqn = format!("{}$continuation", fqn);
    let exists = module.items.iter().any(|item| {
        if let Item::Metadata(m) = item {
            m.fqn == cont_fqn
        } else {
            false
        }
    });
    if !exists {
        module.items.push(Item::Metadata(crate::mir::MetadataRoot {
            span: scoop2_base::Span::default(),
            fqn: cont_fqn,
            kind: crate::mir::MetadataKind::Struct,
            file: scoop2_base::FileId(0),
        }));
    }
}

/// 调用适配：当函数调用 EffectStep 函数时，处理返回的 Step 值。
/// 同时在此阶段消除所有剩余的 Handle 终结符（它们保留至此是为了给
/// 调用适配提供 handle 区域/arm 路由信息）。
fn adapt_calls(module: &mut Module, interner: &Interner) {
    // 收集所有 EffectStep 函数的 FQN → EffectStepAbi 映射。
    let effect_step_info: std::collections::HashMap<String, crate::mir::EffectStepAbi> = module
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Fun(fd) = item {
                if let Some(abi) = &fd.effect_abi {
                    return Some((fd.fqn.clone(), abi.clone()));
                }
            }
            None
        })
        .collect();
    let effect_step_fqns: std::collections::HashSet<String> =
        effect_step_info.keys().cloned().collect();
    // 预计算 handle arm op 的规范化映射（短名 → 全限定名），避免在可变遍历中借用 module。
    let mut canon_map: HashMap<String, String> = HashMap::new();
    for item in &module.items {
        let body = match item {
            Item::Fun(fd) => fd.body.as_ref(),
            Item::Initializer(ir) => Some(&ir.body),
            _ => None,
        };
        if let Some(body) = body {
            for h in collect_handle_info(body) {
                for op in h.arm_dispatch.keys() {
                    if !canon_map.contains_key(op) {
                        canon_map.insert(op.clone(), canonical_op_fqn(module, op));
                    }
                }
            }
        }
    }
    // 对每个函数体：先适配调用点，再消除 Handle 终结符。
    for item in &mut module.items {
        let caller_fqn = match item {
            Item::Fun(fd) => fd.fqn.clone(),
            Item::Initializer(ir) => ir.fqn.clone(),
            _ => String::new(),
        };
        let caller_abi = effect_step_info.get(&caller_fqn).cloned();
        let body = match item {
            Item::Fun(fd) => fd.body.as_mut(),
            Item::Initializer(ir) => Some(&mut ir.body),
            _ => None,
        };
        if let Some(body) = body {
            adapt_calls_in_body(
                body,
                &effect_step_fqns,
                &effect_step_info,
                caller_abi.as_ref(),
                &canon_map,
                &mut module.types,
                interner,
                &caller_fqn,
            );
            rewrite_handles(body);
        }
    }
}

/// 在单个 body 中适配对 EffectStep 函数的调用。
///
/// 当函数 A 调用 EffectStep 函数 B 时，B 返回 Step 值而非原始返回类型。
/// 调用适配在调用语句后插入 dispatch 链：
/// 1. `step_local = call(...)`；`cond = PatternMatch(step_local, Complete)`。
/// 2. Complete → PatternExtract 提取结果，赋值到原始 target local，继续原控制流。
/// 3. 非 Complete 且调用点在某个 handle 区域内、区域 arm 覆盖该 op →
///    提取 payload 并解构到 arm binder locals，goto arm。
/// 4. 非 Complete 且 A 自身是 EffectStep 且 outward 含该 op →
///    用 A 自己的 Step 变体重新包装 payload，Return 向上传播。
/// 5. 其余（无 handler 覆盖的未处理 effect）→ Panic("unhandled effect")。
#[allow(clippy::too_many_arguments)]
fn adapt_calls_in_body(
    body: &mut Body,
    effect_step_fqns: &std::collections::HashSet<String>,
    effect_step_info: &std::collections::HashMap<String, crate::mir::EffectStepAbi>,
    caller_abi: Option<&crate::mir::EffectStepAbi>,
    canon_map: &HashMap<String, String>,
    store: &mut TypeStore,
    interner: &Interner,
    caller_fqn: &str,
) {
    // 当前 body 的 handle 区域（内层优先排序在使用处进行）。
    let handles = collect_handle_info(body);
    let mut handle_regions: Vec<(HashSet<usize>, &HandleInfo)> = Vec::new();
    for h in &handles {
        let region = find_handle_body_region(body, h);
        handle_regions.push((region, h));
    }
    // 收集所有需要适配的调用点（block_idx, stmt_idx, callee_fqn, target_local, original_result_ty）。
    // phase A 已适配的 call-chain 站点（caller 自身 effect_abi 里有记录）跳过。
    let mut call_sites: Vec<(usize, usize, String, LocalId, TypeId)> = Vec::new();
    for (bi, block) in body.blocks.iter().enumerate() {
        for (si, stmt) in block.stmts.iter().enumerate() {
            if let StatementKind::Assign { target, value } = &stmt.kind {
                if let Rvalue::Call {
                    kind: CallKind::Direct { callee_fqn, .. },
                    transport,
                    ..
                } = value
                {
                    if effect_step_fqns.contains(callee_fqn) {
                        let already_adapted = caller_abi.is_some_and(|abi| {
                            abi.call_chain_sites
                                .iter()
                                .any(|s| s.block_idx == bi && s.stmt_idx == si)
                                || abi.cloned_call_sites.contains(&(bi, si))
                        });
                        if already_adapted {
                            continue;
                        }
                        let result_ty = transport.result.source_ty;
                        call_sites.push((bi, si, callee_fqn.clone(), *target, result_ty));
                    }
                }
            }
        }
    }
    // 逆序处理（避免索引偏移）。
    for (bi, si, callee_fqn, target_local, result_ty) in call_sites.into_iter().rev() {
        let span = body.blocks[bi].stmts[si].span;
        // 获取 callee 的 Step 变体信息。
        let callee_abi = match effect_step_info.get(&callee_fqn) {
            Some(abi) => abi.clone(),
            None => continue, // callee 不是 EffectStep（不应发生，但防御性跳过）
        };
        let step_ty = callee_abi.step_ty;
        let step_fqn_sym = interner.get(&callee_fqn).unwrap_or_default();
        let complete_sym = callee_abi
            .step_variants
            .iter()
            .find(|v| v.is_complete)
            .map(|v| v.name_sym)
            .unwrap_or_default();
        // 调用点的 handle 覆盖查找：从最内层 handle（区域最小者）向外层
        // 逐层找第一个 arm 覆盖该 op 的 handle——嵌套 handle 时内层未覆盖的
        // op 向外层 handler 传播（如 escape arm body 内调用 perform 外层
        // effect 的函数）。
        let regions_inner_first: Vec<&HandleInfo> = {
            let mut rs: Vec<&(HashSet<usize>, &HandleInfo)> = handle_regions
                .iter()
                .filter(|(region, _)| region.contains(&bi))
                .collect();
            rs.sort_by_key(|(region, _)| region.len());
            rs.into_iter().map(|(_, h)| *h).collect()
        };

        // 分配 Step local（接收 EffectStep 函数的返回值）。
        let step_local = LocalId(body.locals.len() as u32);
        body.locals.push(LocalDecl {
            span,
            name: None,
            ty: step_ty,
            source: crate::mir::LocalSource::Temp,
            mutable: false,
        });
        // 分配提取结果 local。
        let extract_local = LocalId(body.locals.len() as u32);
        body.locals.push(LocalDecl {
            span,
            name: None,
            ty: result_ty,
            source: crate::mir::LocalSource::Temp,
            mutable: false,
        });

        // 保存原块的终结符和后续语句（在修改前保存）。
        let orig_terminator = body.blocks[bi].terminator.clone();
        let orig_span_term = body.blocks[bi].terminator.span;
        let after_stmts: Vec<Statement> = body.blocks[bi].stmts[si + 1..].to_vec();
        let before_stmts: Vec<Statement> = body.blocks[bi].stmts[..si].to_vec();
        let orig_value = match &body.blocks[bi].stmts[si].kind {
            StatementKind::Assign { value, .. } => value.clone(),
            _ => Rvalue::Use(Operand::Const(crate::mir::ConstValue::Unit)),
        };

        // 计划 dispatch 链：每个非 Complete 变体的去向。
        enum VariantRoute {
            /// 路由到 handle arm：binder locals + arm 目标块。
            Arm {
                binder_locals: Vec<LocalId>,
                target: BasicBlockId,
            },
            /// 用调用方自己的 Step 变体重新包装并向上传播。
            Rewrap { variant_sym: scoop2_base::Symbol },
        }
        let mut routes: Vec<(usize, VariantRoute)> = Vec::new(); // (callee variant index, route)
        for (vi, variant) in callee_abi.step_variants.iter().enumerate() {
            if variant.is_complete {
                continue;
            }
            // 1. handle arm 覆盖？（arm op 经规范化后与变体 op 比较；内层→外层）。
            let arm_route = regions_inner_first.iter().find_map(|h| {
                h.arm_dispatch.iter().find(|(op, _)| {
                    canon_map.get(*op).map(|c| c.replace('.', "_")) == Some(variant.name.clone())
                        || op.replace('.', "_") == variant.name
                })
            });
            if let Some((_, arm)) = arm_route {
                routes.push((
                    vi,
                    VariantRoute::Arm {
                        binder_locals: arm.binder_locals.clone(),
                        target: arm.target,
                    },
                ));
                continue;
            }
            // 2. 调用方自身传播？（按变体名匹配调用方 Step 变体）。
            if let Some(cabi) = caller_abi {
                if let Some(own) = cabi
                    .step_variants
                    .iter()
                    .find(|v| !v.is_complete && v.name == variant.name)
                {
                    routes.push((
                        vi,
                        VariantRoute::Rewrap {
                            variant_sym: own.name_sym,
                        },
                    ));
                    continue;
                }
            }
            // 3. 未覆盖：落入 panic 分支（不生成路由）。
        }

        // 创建 Complete 分支块。
        let complete_block_id = BasicBlockId(body.blocks.len() as u32);
        // 为每条路由创建 check 块 + 执行块；最后是 panic 块。
        // 布局（追加顺序）：complete, (check0, act0, check1, act1, ...), panic
        let num_routes = routes.len();
        let check_block_ids: Vec<BasicBlockId> = (0..num_routes)
            .map(|i| BasicBlockId(body.blocks.len() as u32 + 1 + (i * 2) as u32))
            .collect();
        let act_block_ids: Vec<BasicBlockId> = (0..num_routes)
            .map(|i| BasicBlockId(body.blocks.len() as u32 + 2 + (i * 2) as u32))
            .collect();
        let panic_block_id = BasicBlockId(body.blocks.len() as u32 + 1 + (num_routes * 2) as u32);

        // 重写当前块：before_stmts + step_local = call(...) + cond = PatternMatch(Complete) + CondBr
        let bool_ty = store.bool();
        let cond_local = LocalId(body.locals.len() as u32);
        body.locals.push(LocalDecl {
            span,
            name: None,
            ty: bool_ty,
            source: crate::mir::LocalSource::Temp,
            mutable: false,
        });
        let mut new_stmts = before_stmts;
        new_stmts.push(Statement {
            span,
            kind: StatementKind::Assign {
                target: step_local,
                value: orig_value,
            },
        });
        new_stmts.push(Statement {
            span,
            kind: StatementKind::Assign {
                target: cond_local,
                value: Rvalue::PatternMatch {
                    subject: Operand::Local(step_local),
                    pattern: crate::mir::Pattern::Variant {
                        enum_fqn: step_fqn_sym,
                        variant_name: complete_sym,
                        args: vec![],
                    },
                },
            },
        });
        body.blocks[bi].stmts = new_stmts;
        let first_else = check_block_ids.first().copied().unwrap_or(panic_block_id);
        body.blocks[bi].terminator = Terminator {
            span: orig_span_term,
            kind: TerminatorKind::CondBr {
                cond: Operand::Local(cond_local),
                then_target: complete_block_id,
                else_target: first_else,
            },
        };

        // Complete 分支块：提取结果 + 后续语句 + 原终结符。
        let mut complete_stmts = vec![
            Statement {
                span,
                kind: StatementKind::Assign {
                    target: extract_local,
                    value: Rvalue::PatternExtract {
                        subject: Operand::Local(step_local),
                        path: vec![],
                        result_ty,
                    },
                },
            },
            Statement {
                span,
                kind: StatementKind::Assign {
                    target: target_local,
                    value: Rvalue::Use(Operand::Local(extract_local)),
                },
            },
        ];
        complete_stmts.extend(after_stmts);
        body.blocks.push(crate::mir::BasicBlock {
            stmts: complete_stmts,
            terminator: orig_terminator,
        });

        // 每条路由：check 块（PatternMatch 变体）+ act 块（绑定/重包装）。
        for (ri, (vi, route)) in routes.iter().enumerate() {
            let variant = &callee_abi.step_variants[*vi];
            let else_target = check_block_ids
                .get(ri + 1)
                .copied()
                .unwrap_or(panic_block_id);
            let check_cond = LocalId(body.locals.len() as u32);
            body.locals.push(LocalDecl {
                span,
                name: None,
                ty: bool_ty,
                source: crate::mir::LocalSource::Temp,
                mutable: false,
            });
            body.blocks.push(crate::mir::BasicBlock {
                stmts: vec![Statement {
                    span,
                    kind: StatementKind::Assign {
                        target: check_cond,
                        value: Rvalue::PatternMatch {
                            subject: Operand::Local(step_local),
                            pattern: crate::mir::Pattern::Variant {
                                enum_fqn: step_fqn_sym,
                                variant_name: variant.name_sym,
                                args: vec![],
                            },
                        },
                    },
                }],
                terminator: Terminator {
                    span,
                    kind: TerminatorKind::CondBr {
                        cond: Operand::Local(check_cond),
                        then_target: act_block_ids[ri],
                        else_target,
                    },
                },
            });
            // act 块。
            match route {
                VariantRoute::Arm {
                    binder_locals,
                    target,
                } => {
                    let mut stmts = Vec::new();
                    // callee 挂起时会把 chain link 写入 TLS（outward Perform 的
                    // MakeChainLink）。本路径是 abandon 语义（普通 arm 捕获后
                    // 不再 resume），取走并丢弃，维持"op-Step 路由后 TLS 为空"
                    // 的不变式。
                    let chain_sink = LocalId(body.locals.len() as u32);
                    body.locals.push(LocalDecl {
                        span,
                        name: None,
                        ty: store.any(),
                        source: crate::mir::LocalSource::Temp,
                        mutable: false,
                    });
                    stmts.push(Statement {
                        span,
                        kind: StatementKind::Assign {
                            target: chain_sink,
                            value: Rvalue::TakeChainLink {
                                result_ty: store.any(),
                            },
                        },
                    });
                    if !binder_locals.is_empty() {
                        // 提取整个 payload。
                        let payload_local = LocalId(body.locals.len() as u32);
                        body.locals.push(LocalDecl {
                            span,
                            name: None,
                            ty: variant.payload_ty,
                            source: crate::mir::LocalSource::Temp,
                            mutable: false,
                        });
                        stmts.push(Statement {
                            span,
                            kind: StatementKind::Assign {
                                target: payload_local,
                                value: Rvalue::PatternExtract {
                                    subject: Operand::Local(step_local),
                                    path: vec![],
                                    result_ty: variant.payload_ty,
                                },
                            },
                        });
                        // payload 形态与 rewrite_perform_sites 的构造对称：
                        // 多参数 op → tuple，按 TupleIndex(i) 解构到各 binder；
                        // 单参数 op → 裸值，唯一 binder 直接使用。
                        // 组件类型取 payload tuple 的元素类型（op 声明参数类型），
                        // 不依赖 arm binder 的声明类型（可能缺 ascription）。
                        // 仅多 binder（= 多参数 op，payload 为构造的 tuple）时按
                        // TupleIndex 解构；单 binder 直接使用裸 payload（即便其
                        // 类型本身是 tuple——那是 op 的唯一参数值）。
                        let tuple_elems: Option<Vec<TypeId>> = if binder_locals.len() > 1 {
                            match store.kind(variant.payload_ty) {
                                scoop2_hir::ty::TypeKind::Value(
                                    scoop2_hir::ty::ValueTypeKind::Tuple(elems),
                                ) => Some(elems.clone()),
                                _ => None,
                            }
                        } else {
                            None
                        };
                        for (bii, binder) in binder_locals.iter().enumerate() {
                            let value = match &tuple_elems {
                                Some(elems) => Rvalue::TupleIndex {
                                    receiver: Operand::Local(payload_local),
                                    index: bii as u128,
                                    element_ty: elems
                                        .get(bii)
                                        .copied()
                                        .unwrap_or(variant.payload_ty),
                                },
                                None => Rvalue::Use(Operand::Local(payload_local)),
                            };
                            stmts.push(Statement {
                                span,
                                kind: StatementKind::Assign {
                                    target: *binder,
                                    value,
                                },
                            });
                        }
                    }
                    body.blocks.push(crate::mir::BasicBlock {
                        stmts,
                        terminator: Terminator {
                            span,
                            kind: TerminatorKind::Goto { target: *target },
                        },
                    });
                }
                VariantRoute::Rewrap { variant_sym } => {
                    let cabi = caller_abi.expect("Rewrap 路由要求调用方是 EffectStep");
                    let payload_local = LocalId(body.locals.len() as u32);
                    body.locals.push(LocalDecl {
                        span,
                        name: None,
                        ty: variant.payload_ty,
                        source: crate::mir::LocalSource::Temp,
                        mutable: false,
                    });
                    let own_step_local = LocalId(body.locals.len() as u32);
                    body.locals.push(LocalDecl {
                        span,
                        name: None,
                        ty: cabi.step_ty,
                        source: crate::mir::LocalSource::Temp,
                        mutable: false,
                    });
                    let caller_step_sym = interner.get(caller_fqn).unwrap_or_default();
                    body.blocks.push(crate::mir::BasicBlock {
                        stmts: vec![
                            Statement {
                                span,
                                kind: StatementKind::Assign {
                                    target: payload_local,
                                    value: Rvalue::PatternExtract {
                                        subject: Operand::Local(step_local),
                                        path: vec![],
                                        result_ty: variant.payload_ty,
                                    },
                                },
                            },
                            Statement {
                                span,
                                kind: StatementKind::Assign {
                                    target: own_step_local,
                                    value: Rvalue::EnumVariant {
                                        enum_ty: cabi.step_ty,
                                        enum_fqn: caller_step_sym,
                                        variant_name: *variant_sym,
                                        args: vec![crate::mir::CallArg {
                                            name: None,
                                            is_spread: false,
                                            value: Operand::Local(payload_local),
                                            value_ty: variant.payload_ty,
                                        }],
                                        payload: AggregateTransportMetadata {
                                            aggregate_ty: variant.payload_ty,
                                            kind: AggregateTransportKind::EnumPayload,
                                            fields: Vec::new(),
                                        },
                                        stable_key: None,
                                    },
                                },
                            },
                        ],
                        terminator: Terminator {
                            span,
                            kind: TerminatorKind::Return {
                                value: Some(Operand::Local(own_step_local)),
                            },
                        },
                    });
                }
            }
        }

        // panic 块：未处理的 effect（无 handler 覆盖且无法传播）。
        body.blocks.push(crate::mir::BasicBlock {
            stmts: vec![Statement {
                span,
                kind: StatementKind::Panic {
                    message: format!("unhandled effect: no handler for step of {callee_fqn}"),
                },
            }],
            terminator: Terminator {
                span,
                kind: TerminatorKind::Unreachable,
            },
        });
    }
}

// =========================================================================
// Handle arm 路由信息
// =========================================================================

struct ArmRoute {
    target: BasicBlockId,
    binder_locals: Vec<LocalId>,
    /// resuming arm 的 continuation binder（Some = escape continuation arm）。
    continuation_local: Option<LocalId>,
}

struct HandleInfo {
    /// Handle 终结符所在块下标（嵌套关系判定用）。
    handle_block: usize,
    body_target: BasicBlockId,
    arm_dispatch: HashMap<String, ArmRoute>,
    exit_target: BasicBlockId,
    finally_target: Option<BasicBlockId>,
    /// handle 结果 local（escape 边界克隆的 Complete payload 来源）。
    result_local: LocalId,
}

/// 对单个函数体执行 effect lowering。
///
/// `outward_ops`：阶段 0 分类给出的逸出 effect 操作（含传播而来、本地无 Perform 的）。
/// Handle 终结符在此不消除——保留到阶段 B（adapt_calls），因为调用适配需要
/// handle 区域信息把被传播过来的 Step 路由到对应 arm。
///
/// `effect_step_fqns` / `canon_outward` / `canon_map`：call-chain（case b）站点
/// 收集用——识别对 EffectStep 函数的 Direct 调用、callee 的规范 outward ops、
/// arm op 的规范化映射。
#[allow(clippy::too_many_arguments)]
fn lower_body(
    body: &mut Body,
    store: &mut TypeStore,
    interner: &Interner,
    fqn: &str,
    outward_ops: &[(String, TypeId)],
    params: &[(LocalId, TypeId)],
    effect_step_fqns: &HashSet<String>,
    canon_outward: &HashMap<String, Vec<(String, TypeId)>>,
    canon_map: &HashMap<String, String>,
    fun_sites: &[(usize, usize, Vec<(String, TypeId)>)],
) -> Option<crate::mir::EffectStepAbi> {
    // 阶段 1：被本地 handle 捕获的 Perform → 绑定实参 + goto arm。
    // Handle 终结符本身保留（阶段 B 消除）。
    // escape continuation（resuming）arm 捕获的 Perform 不在此重写——它们需要
    // frame + MakeContinuation + 边界克隆，留给阶段 2 的 EffectStep 变换。
    let handles = collect_handle_info(body);
    let mut escape_routing: HashMap<usize, PerformRoute> = HashMap::new();
    if !handles.is_empty() {
        let routing = build_perform_routing(body, &handles);
        let mut plain_routing = HashMap::new();
        for (bi, route) in routing {
            if route.escape.is_some() {
                escape_routing.insert(bi, route);
            } else {
                plain_routing.insert(bi, route);
            }
        }
        rewrite_captured_performs(body, &plain_routing, &handles);
    }
    // case b：escape arm 捕获的 EffectStep 调用点也强制 EffectStep 变换
    // （构造 continuation 需要本函数 frame + step 函数指针）。
    let has_escape_call_site =
        body_has_escape_call_site(body, &handles, effect_step_fqns, canon_outward, canon_map);
    // 间接调用（FunValue/Closure）版本：站点效果行的 op 被 escape arm 覆盖时
    // 同样强制 EffectStep 变换（canon_map 规范化 arm op 后比较）。
    let has_escape_fun_site = !fun_sites.is_empty() && {
        let mut handle_regions: Vec<(HashSet<usize>, &HandleInfo)> = Vec::new();
        for h in &handles {
            let region = find_handle_body_region(body, h);
            handle_regions.push((region, h));
        }
        fun_sites.iter().any(|(bi, _, row_ops)| {
            handle_regions.iter().any(|(region, h)| {
                region.contains(bi)
                    && row_ops.iter().any(|(op, _)| {
                        h.arm_dispatch.iter().any(|(aop, r)| {
                            r.continuation_local.is_some()
                                && (canon_map.get(aop).map(|c| c == op).unwrap_or(false)
                                    || aop == op)
                        })
                    })
            })
        })
    };
    // 阶段 2：是否变换为 EffectStep。有 escape 捕获时必须变换（continuation
    // 需要本函数的 frame + step 函数指针）。
    let is_effect_step = !outward_ops.is_empty()
        || body_has_resume(body)
        || !escape_routing.is_empty()
        || has_escape_call_site
        || has_escape_fun_site;
    if is_effect_step {
        lower_to_effect_step(
            body,
            store,
            interner,
            fqn,
            outward_ops,
            params,
            escape_routing,
            effect_step_fqns,
            canon_outward,
            canon_map,
            fun_sites,
        )
    } else {
        None
    }
}

/// 检测 body 内是否存在被 escape arm 覆盖的 EffectStep → EffectStep 调用点。
fn body_has_escape_call_site(
    body: &Body,
    handles: &[HandleInfo],
    effect_step_fqns: &HashSet<String>,
    canon_outward: &HashMap<String, Vec<(String, TypeId)>>,
    canon_map: &HashMap<String, String>,
) -> bool {
    if handles.is_empty() {
        return false;
    }
    let mut handle_regions: Vec<(HashSet<usize>, &HandleInfo)> = Vec::new();
    for h in handles {
        let region = find_handle_body_region(body, h);
        handle_regions.push((region, h));
    }
    for (bi, block) in body.blocks.iter().enumerate() {
        for stmt in &block.stmts {
            let callee_fqn = match &stmt.kind {
                StatementKind::Assign {
                    value:
                        Rvalue::Call {
                            kind: CallKind::Direct { callee_fqn, .. },
                            ..
                        },
                    ..
                } => callee_fqn,
                _ => continue,
            };
            if !effect_step_fqns.contains(callee_fqn) {
                continue;
            }
            let Some(callee_ops) = canon_outward.get(callee_fqn) else {
                continue;
            };
            // 最内层 handle（区域最小者）。
            let enclosing = handle_regions
                .iter()
                .filter(|(region, _)| region.contains(&bi))
                .min_by_key(|(region, _)| region.len())
                .map(|(_, h)| *h);
            let Some(h) = enclosing else { continue };
            // callee 任一 outward op 被该 handle 的 escape arm 覆盖？
            let hit = callee_ops.iter().any(|(op, _)| {
                let variant_name = op.replace('.', "_");
                h.arm_dispatch.iter().any(|(aop, route)| {
                    route.continuation_local.is_some()
                        && (canon_map.get(aop).map(|c| c.replace('.', "_")).as_deref()
                            == Some(variant_name.as_str())
                            || aop.replace('.', "_") == variant_name)
                })
            });
            if hit {
                return true;
            }
        }
    }
    false
}

fn body_has_resume(body: &Body) -> bool {
    for block in &body.blocks {
        for stmt in &block.stmts {
            if let StatementKind::Assign { value, .. } = &stmt.kind {
                if let Rvalue::Call {
                    kind: CallKind::Resume { .. },
                    ..
                } = value
                {
                    return true;
                }
            }
        }
    }
    false
}

// =========================================================================
// 阶段 1：Handle dispatch 消除
// =========================================================================

fn collect_handle_info(body: &Body) -> Vec<HandleInfo> {
    let mut handles = Vec::new();
    for (block_idx, block) in body.blocks.iter().enumerate() {
        if let TerminatorKind::Handle {
            arms,
            body_target,
            arm_targets,
            finally_target,
            exit_target,
            metadata,
            ..
        } = &block.terminator.kind
        {
            let mut arm_dispatch = HashMap::new();
            for (arm, &target) in arms.iter().zip(arm_targets.iter()) {
                arm_dispatch.insert(
                    arm.op_fqn.clone(),
                    ArmRoute {
                        target,
                        binder_locals: arm.binder_locals.clone(),
                        continuation_local: arm.continuation_local,
                    },
                );
            }
            handles.push(HandleInfo {
                handle_block: block_idx,
                body_target: *body_target,
                arm_dispatch,
                exit_target: *exit_target,
                finally_target: *finally_target,
                result_local: metadata.result_local,
            });
        }
    }
    handles
}

fn build_perform_routing(body: &Body, handles: &[HandleInfo]) -> HashMap<usize, PerformRoute> {
    let mut routing = HashMap::new();
    for handle in handles {
        let body_region = find_handle_body_region(body, handle);
        for block_idx in body_region {
            if let TerminatorKind::Perform { op_fqn, .. } = &body.blocks[block_idx].terminator.kind
            {
                if let Some(arm) = handle.arm_dispatch.get(op_fqn) {
                    routing.insert(
                        block_idx,
                        PerformRoute {
                            arm_target: arm.target,
                            binder_locals: arm.binder_locals.clone(),
                            escape: arm.continuation_local.map(|k| EscapeCapture {
                                continuation_local: k,
                                result_local: handle.result_local,
                                exit_target: handle.exit_target,
                            }),
                        },
                    );
                }
            }
        }
    }
    routing
}

struct PerformRoute {
    arm_target: BasicBlockId,
    binder_locals: Vec<LocalId>,
    /// Some = escape continuation（resuming）arm 捕获：交给 EffectStep 变换
    /// 构造 MakeContinuation + 边界克隆，而非阶段 1 的简单 bind+goto。
    escape: Option<EscapeCapture>,
}

/// escape continuation 捕获点的上下文（构造 continuation 与边界克隆用）。
#[derive(Clone)]
struct EscapeCapture {
    /// arm 的 continuation binder local（k）。
    continuation_local: LocalId,
    /// handle 结果 local（克隆后缀的 Complete payload）。
    result_local: LocalId,
    /// handle 出口块（边界：克隆中指向它的边改为 Return(Complete)）。
    exit_target: BasicBlockId,
}

fn find_handle_body_region(body: &Body, handle: &HandleInfo) -> HashSet<usize> {
    let mut region = HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    let exit_idx = handle.exit_target.0 as usize;
    queue.push_back(handle.body_target.0 as usize);
    while let Some(idx) = queue.pop_front() {
        if idx >= body.blocks.len() || region.contains(&idx) || idx == exit_idx {
            continue;
        }
        region.insert(idx);
        for succ in body.blocks[idx].successors() {
            let succ_idx = succ.0 as usize;
            if !region.contains(&succ_idx) && succ_idx != exit_idx {
                queue.push_back(succ_idx);
            }
        }
    }
    region
}

fn rewrite_captured_performs(
    body: &mut Body,
    routing: &HashMap<usize, PerformRoute>,
    handles: &[HandleInfo],
) {
    if routing.is_empty() {
        return;
    }
    // 各 handle 的 body 区域（与 build_perform_routing 同一算法，识别路由
    // 目标 handle 用）与全作用域（body + arms + finally，逃逸路径 finally
    // 链判定用）。
    let body_regions: Vec<HashSet<usize>> = handles
        .iter()
        .map(|h| find_handle_body_region(body, h))
        .collect();
    let extents: Vec<HashSet<usize>> = handles.iter().map(|h| handle_extent(body, h)).collect();
    let finally_regions: Vec<Option<HashSet<usize>>> = handles
        .iter()
        .map(|h| {
            h.finally_target.map(|fb| {
                let mut region = HashSet::new();
                let mut queue: std::collections::VecDeque<usize> =
                    std::collections::VecDeque::from([fb.0 as usize]);
                let exit_idx = h.exit_target.0 as usize;
                while let Some(b) = queue.pop_front() {
                    if b == exit_idx || b >= body.blocks.len() || !region.insert(b) {
                        continue;
                    }
                    for t in terminator_targets(&body.blocks[b].terminator.kind) {
                        queue.push_back(t.0 as usize);
                    }
                }
                region
            })
        })
        .collect();

    for (&block_idx, route) in routing {
        let (perform_args, op_fqn): (Vec<Operand>, String) =
            match &body.blocks[block_idx].terminator.kind {
                TerminatorKind::Perform { args, op_fqn, .. } => (
                    args.iter().map(|a| a.value.clone()).collect(),
                    op_fqn.clone(),
                ),
                _ => continue,
            };
        let span = body.blocks[block_idx].terminator.span;
        // 逃逸路径 finally 链：arm body（或嵌套 handle 区域）内的 Perform 路由到
        // 外层 handle 的 arm 时，中间带 finally 的 handle 必须先执行 finally 再
        // 继续向外。finally 经克隆接入（原 finally 块服务正常完成路径，出口是
        // exit_target；克隆的出口是下一跳 finally 或路由目标 arm）。
        let routed = handles
            .iter()
            .enumerate()
            .filter(|(i, h)| {
                body_regions[*i].contains(&block_idx) && h.arm_dispatch.contains_key(&op_fqn)
            })
            .map(|(i, _)| i)
            .last();
        let chain: Vec<usize> = match routed {
            Some(h) => {
                let v: Vec<usize> = handles
                    .iter()
                    .enumerate()
                    .filter(|(i, f)| {
                        *i != h
                            && f.finally_target.is_some()
                            && extents[*i].contains(&block_idx)
                            && extents[h].contains(&f.handle_block)
                            && !finally_regions[*i]
                                .as_ref()
                                .is_some_and(|r| r.contains(&block_idx))
                    })
                    .map(|(i, _)| i)
                    .collect();
                // 内层优先：handle 块被更多链内 handle 的作用域包含者更内层。
                let counts: Vec<usize> = v
                    .iter()
                    .map(|&i| {
                        v.iter()
                            .filter(|&&j| j != i && extents[j].contains(&handles[i].handle_block))
                            .count()
                    })
                    .collect();
                let mut order: Vec<usize> = (0..v.len()).collect();
                order.sort_by_key(|&k| std::cmp::Reverse(counts[k]));
                order.into_iter().map(|k| v[k]).collect()
            }
            None => Vec::new(),
        };
        // 外层先克隆（next_hop 逆向链接），链头 = 最内层 finally 克隆入口。
        let mut next_hop = route.arm_target;
        for &fi in chain.iter().rev() {
            next_hop = clone_finally_region(body, &handles[fi], next_hop);
        }
        let block = &mut body.blocks[block_idx];
        for (i, &binder_local) in route.binder_locals.iter().enumerate() {
            if let Some(arg_value) = perform_args.get(i) {
                block.stmts.push(Statement {
                    span,
                    kind: StatementKind::Assign {
                        target: binder_local,
                        value: Rvalue::Use(arg_value.clone()),
                    },
                });
            }
        }
        block.terminator = Terminator {
            span,
            kind: TerminatorKind::Goto { target: next_hop },
        };
    }
}

/// handle 的动态作用域：body + arms + finally（不越过 exit_target）。
/// 逃逸路径 finally 链的嵌套判定用（BFS 会穿过嵌套 Handle 终结符进入其
/// body/arms/finally——`terminator_targets` 展开 Handle 的全部目标）。
fn handle_extent(body: &Body, handle: &HandleInfo) -> HashSet<usize> {
    let mut region = HashSet::new();
    let mut queue: std::collections::VecDeque<usize> =
        std::collections::VecDeque::from([handle.body_target.0 as usize]);
    queue.extend(handle.arm_dispatch.values().map(|r| r.target.0 as usize));
    if let Some(fb) = handle.finally_target {
        queue.push_back(fb.0 as usize);
    }
    let exit_idx = handle.exit_target.0 as usize;
    while let Some(b) = queue.pop_front() {
        if b == exit_idx || b >= body.blocks.len() || !region.insert(b) {
            continue;
        }
        for t in terminator_targets(&body.blocks[b].terminator.kind) {
            queue.push_back(t.0 as usize);
        }
    }
    region
}

/// 克隆 handle 的 finally 区用于逃逸路径：区域内指向 exit_target 的边改接
/// `after`（下一跳 finally 克隆或路由目标 arm），内部边重映射到克隆块。
/// 返回克隆区入口块 id。
fn clone_finally_region(body: &mut Body, handle: &HandleInfo, after: BasicBlockId) -> BasicBlockId {
    let Some(fb) = handle.finally_target else {
        return after;
    };
    let exit_idx = handle.exit_target.0 as usize;
    let mut region: Vec<usize> = Vec::new();
    let mut seen: HashSet<usize> = HashSet::new();
    let mut queue: std::collections::VecDeque<usize> =
        std::collections::VecDeque::from([fb.0 as usize]);
    while let Some(b) = queue.pop_front() {
        if b == exit_idx || b >= body.blocks.len() || !seen.insert(b) {
            continue;
        }
        region.push(b);
        for t in terminator_targets(&body.blocks[b].terminator.kind) {
            queue.push_back(t.0 as usize);
        }
    }
    if region.is_empty() {
        return after;
    }
    let base = body.blocks.len();
    let id_map: HashMap<usize, usize> = region
        .iter()
        .enumerate()
        .map(|(i, &b)| (b, base + i))
        .collect();
    for &b in &region {
        let mut nb = body.blocks[b].clone();
        remap_terminator_targets(&mut nb.terminator.kind, &mut |t| {
            if t.0 as usize == exit_idx {
                after
            } else if let Some(&mapped) = id_map.get(&(t.0 as usize)) {
                BasicBlockId(mapped as u32)
            } else {
                t
            }
        });
        body.blocks.push(nb);
    }
    BasicBlockId(id_map[&(fb.0 as usize)] as u32)
}

fn rewrite_handles(body: &mut Body) {
    // finally 接线：Handle 带 finally_target 时，body/arm 完成路径原先把
    // 控制流直接送到 exit_target，finally 块只有 Handle 终结符引用、永不
    // 执行。这里把 handle 区域内指向 exit_target 的边改指 finally 块
    // （finally 块自身 → exit_target 的边保持不变），使 finally 在 handle
    // 退出时恰好执行一次。escape 后缀克隆在阶段 A 已完成（旧接线），克隆
    // 边界不经 finally——resume 完成路径不重复执行 finally。
    let mut rewrites: Vec<(usize, BasicBlockId, BasicBlockId)> = Vec::new();
    for block in body.blocks.iter() {
        let (body_target, arm_targets, finally_target, exit_target) = match &block.terminator.kind {
            TerminatorKind::Handle {
                body_target,
                arm_targets,
                finally_target: Some(fb),
                exit_target,
                ..
            } => (*body_target, arm_targets.clone(), *fb, *exit_target),
            _ => continue,
        };
        // BFS 收集 handle 区域（body + arms，不越过 exit_target / finally 块）。
        let mut region: HashSet<usize> = HashSet::new();
        let mut queue: std::collections::VecDeque<usize> =
            std::collections::VecDeque::from([body_target.0 as usize]);
        queue.extend(arm_targets.iter().map(|t| t.0 as usize));
        while let Some(b) = queue.pop_front() {
            if b == exit_target.0 as usize
                || b == finally_target.0 as usize
                || b >= body.blocks.len()
                || !region.insert(b)
            {
                continue;
            }
            for t in terminator_targets(&body.blocks[b].terminator.kind) {
                queue.push_back(t.0 as usize);
            }
        }
        // 区域内指向 exit_target 的边 → finally 块。
        for &b in &region {
            if terminator_targets(&body.blocks[b].terminator.kind)
                .iter()
                .any(|t| *t == exit_target)
            {
                rewrites.push((b, exit_target, finally_target));
            }
        }
    }
    for (b, exit_target, finally_target) in rewrites {
        remap_terminator_targets(&mut body.blocks[b].terminator.kind, &mut |t| {
            if t == exit_target { finally_target } else { t }
        });
    }
    for block in &mut body.blocks {
        let body_target = match &block.terminator.kind {
            TerminatorKind::Handle { body_target, .. } => *body_target,
            _ => continue,
        };
        block.terminator = Terminator {
            span: block.terminator.span,
            kind: TerminatorKind::Goto {
                target: body_target,
            },
        };
    }
}

// =========================================================================
// 阶段 2：EffectStep 变换
// =========================================================================

/// Perform 站点信息。
struct PerformSite {
    block_idx: usize,
    op_fqn: String,
    args: Vec<Operand>,
    resume_target: BasicBlockId,
    resume_local: LocalId,
    live_out_at_resume: Vec<LocalId>,
}

/// 对含未捕获 Perform / Resume / 传播 effect 的 body 执行 EffectStep 变换。
/// 返回 EffectStepAbi 信息（含 frame 类型、outward cases、frame/state local IDs）。
///
/// `outward_ops`：已规范化为全限定名的逸出 op 列表（含 payload 类型），
/// 顺序即 Step 变体序号（tag = 下标 + 1；tag 0 = Complete）。
#[allow(clippy::too_many_arguments)]
fn lower_to_effect_step(
    body: &mut Body,
    store: &mut TypeStore,
    interner: &Interner,
    fqn: &str,
    outward_ops: &[(String, TypeId)],
    params: &[(LocalId, TypeId)],
    escape_routing: HashMap<usize, PerformRoute>,
    effect_step_fqns: &HashSet<String>,
    canon_outward: &HashMap<String, Vec<(String, TypeId)>>,
    canon_map: &HashMap<String, String>,
    fun_sites: &[(usize, usize, Vec<(String, TypeId)>)],
) -> Option<crate::mir::EffectStepAbi> {
    // escape continuation 的精确 answer 类型回填：MIR lowering 把 resuming arm
    // 的 k binder 分配为 Any（见 mir/lower/expr.rs lower_handle），`k.resume(v)`
    // 的结果 local 随之退化为未替换的泛型参数（Continuation 的 Answer 类型参
    // 数）或 Any；LIR 把两者都布局为 Any ref，codegen 按 Any 抽取 Complete
    // payload 会把 answer 装箱成 GC 对象，下游按 Int 使用即读成指针位。这里
    // 用 handle 结果 local 的精确类型改写 resume 调用目标 local 的声明类型
    // 和调用的 transport.result.source_ty（LIR 的 result_ty 取自后者；仅当
    // 其仍为 Param/Any；克隆复制语句时 local id 不变，一处改写全覆盖）。
    // 必须在 frame 布局之前做，frame 槽类型取自 local 声明类型。
    // 同时收集全部 escape answer 类型（含 Any）：Step 布局的 Complete
    // payload 槽必须装得下 answer（见 EffectStepAbi::escape_answer_tys）。
    let mut escape_answer_tys: Vec<TypeId> = Vec::new();
    {
        let any_ty = store.any();
        let is_imprecise = |store: &TypeStore, ty: TypeId| {
            matches!(store.kind(ty), scoop2_hir::ty::TypeKind::Param(_))
                || store.is_nominal_with_fqn(ty, store.any_fqn())
        };
        let mut resume_retype: Vec<(usize, usize, usize, TypeId)> = Vec::new();
        for route in escape_routing.values() {
            let Some(esc) = &route.escape else {
                continue;
            };
            let answer_ty = body
                .locals
                .get(esc.result_local.0 as usize)
                .map(|d| d.ty)
                .unwrap_or(any_ty);
            if !escape_answer_tys.contains(&answer_ty) {
                escape_answer_tys.push(answer_ty);
            }
            if is_imprecise(store, answer_ty) {
                continue;
            }
            for (bi, block) in body.blocks.iter().enumerate() {
                for (si, stmt) in block.stmts.iter().enumerate() {
                    if let StatementKind::Assign { target, value } = &stmt.kind
                        && let Rvalue::Call {
                            kind: CallKind::Resume { continuation, .. },
                            ..
                        } = value
                        && matches!(continuation, Operand::Local(c) if *c == esc.continuation_local)
                        && body
                            .locals
                            .get(target.0 as usize)
                            .is_some_and(|d| is_imprecise(store, d.ty))
                    {
                        resume_retype.push((bi, si, target.0 as usize, answer_ty));
                    }
                }
            }
        }
        for (bi, si, tid, ty) in resume_retype {
            body.locals[tid].ty = ty;
            let new_kind = match store.kind(ty) {
                scoop2_hir::ty::TypeKind::Value(_) => {
                    crate::mir::transport::MirTransportKind::Scalar
                }
                scoop2_hir::ty::TypeKind::Ref(_) => {
                    crate::mir::transport::MirTransportKind::Reference
                }
                _ => crate::mir::transport::MirTransportKind::Unknown,
            };
            if let StatementKind::Assign {
                value: Rvalue::Call { transport, .. },
                ..
            } = &mut body.blocks[bi].stmts[si].kind
            {
                transport.result.source_ty = ty;
                transport.result.kind = new_kind;
                let agg_imprecise = transport
                    .aggregate_return
                    .as_ref()
                    .is_some_and(|v| is_imprecise(store, v.source_ty));
                if agg_imprecise {
                    transport.aggregate_return = None;
                }
            }
        }
    }
    let live_in = analyze::compute_live_in(body);
    let mut perform_sites = collect_perform_sites(body, &live_in);

    // case b：收集 EffectStep → EffectStep 调用点（call-chain 站点）。
    // state 编号续在 perform 站点之后（perform 1..=P，call sites P+1..）。
    let mut call_plans = collect_call_chain_sites(
        body,
        &live_in,
        effect_step_fqns,
        canon_outward,
        canon_map,
        outward_ops,
        (perform_sites.len() + 1) as u128,
        store,
        interner,
        fun_sites,
    );

    // 收集所有需要保存的 live locals（跨所有 Perform 站点 + call-chain 站点的并集）。
    let mut all_live_locals: Vec<LocalId> = Vec::new();
    let mut seen: HashSet<LocalId> = HashSet::new();
    for site in &perform_sites {
        for &lid in &site.live_out_at_resume {
            if seen.insert(lid) {
                all_live_locals.push(lid);
            }
        }
    }
    for plan in &call_plans {
        for &lid in &plan.live_set {
            if seen.insert(lid) {
                all_live_locals.push(lid);
            }
        }
    }

    // 构造 Frame tuple 类型：[state: Int, param_1: P1, ..., live_local_1: T1, ...]。
    // 参数槽必须包含全部参数：resume 时 step 函数只拿到 (frame, word)，
    // 参数值只能经 frame 传入（wrapper 在初始调用时写入，step 入口恢复）。
    // 零尺寸槽类型（Unit / Nothing / 全零尺寸 tuple）在 tuple 布局中与下一字段
    // 共享偏移，而保存/恢复按槽写字节会踩掉相邻真实字段——统一替换为 Bool
    // （1 字节哑槽；codegen 对 Unit 本就按 i8 读写，行为一致）。
    let mut frame_field_tys: Vec<TypeId> = vec![store.int()];
    for (_, ty) in params {
        frame_field_tys.push(if is_zero_sized_slot_ty(store, *ty) {
            store.bool()
        } else {
            *ty
        });
    }
    for &lid in &all_live_locals {
        let ty = body
            .locals
            .get(lid.0 as usize)
            .map(|d| d.ty)
            .unwrap_or_else(|| store.any());
        frame_field_tys.push(if is_zero_sized_slot_ty(store, ty) {
            store.bool()
        } else {
            ty
        });
    }
    // 每个 call-chain 站点追加一个 link 槽（Any ref，存 callee 挂起时的 chain
    // link；frame descriptor 自动 trace——collect_gc_word_offsets 识别 Any ref）。
    for (i, plan) in call_plans.iter_mut().enumerate() {
        plan.link_slot = (frame_field_tys.len() + i) as u128;
    }
    for _ in &call_plans {
        frame_field_tys.push(store.any());
    }
    let frame_ty = store.tuple(frame_field_tys);

    // 分配 frame local。
    let frame_local = LocalId(body.locals.len() as u32);
    body.locals.push(LocalDecl {
        span: scoop2_base::Span::default(),
        name: Some("$frame".to_string()),
        ty: frame_ty,
        source: crate::mir::LocalSource::Temp,
        mutable: true,
    });

    // 构建 live_local → frame slot index 映射（slot 0 = state，随后是参数槽）。
    let live_to_slot: HashMap<LocalId, u128> = all_live_locals
        .iter()
        .enumerate()
        .map(|(i, &lid)| (lid, (i + 1 + params.len()) as u128))
        .collect();

    // frame 不再在 body 内初始化：codegen 的 `sym` wrapper 负责堆分配
    // （scoop_alloc_typed）+ 清零 + 写参数槽，body 编译为 `sym$step(frame, word)`。

    // 构造 Step 合成 enum 类型。
    // Step enum 用函数自身 FQN（已 intern）作为 nominal FQN/标识 Symbol，
    // 确保 Symbol 在后续 verify/dump/LIR 中可被原 interner 解析且不与其它
    // 函数的合成类型冲突。
    let step_fqn_sym = interner.get(fqn).unwrap_or_default();
    let step_ty = store.value_nominal(scoop2_hir::ty::NominalType {
        fqn: step_fqn_sym,
        args: vec![],
        eff: None,
    });

    // 构造 Step 变体信息：tag 0 = Complete（payload = 原始返回类型，由调用方回填），
    // tag i+1 = outward_ops[i]（payload = 分类给出的规范 payload 类型，
    // 与本函数 Perform 站点构造的 payload 类型一致，保证跨函数传播时类型统一）。
    // Complete 的 name_sym 用函数自身 FQN Symbol——它是每个函数内唯一的
    // "非 op" 标识，与任何 effect 变体（op FQN Symbol）不会冲突。
    let complete_sym = step_fqn_sym;
    let mut step_variants: Vec<crate::mir::StepVariant> = vec![crate::mir::StepVariant {
        name: "Complete".to_string(),
        name_sym: complete_sym,
        payload_ty: store.any(), // 占位：调用方用原始返回类型回填
        is_complete: true,
    }];
    for (op, payload_ty) in outward_ops {
        let variant_name = op.replace('.', "_");
        let variant_sym = interner.get(op).unwrap_or_default();
        step_variants.push(crate::mir::StepVariant {
            name: variant_name,
            name_sym: variant_sym,
            payload_ty: *payload_ty,
            is_complete: false,
        });
    }

    // 重写 Perform 站点（outward → Return(Step 变体)；escape 捕获 →
    // 保存 frame + MakeContinuation + 绑定实参 + goto arm）。
    // 返回 outward 站点的 Step 载体 locals。
    let perform_carriers = rewrite_perform_sites(
        body,
        &perform_sites,
        &live_to_slot,
        frame_local,
        frame_ty,
        step_ty,
        step_fqn_sym,
        outward_ops,
        store,
        interner,
        &escape_routing,
    );

    // case b：call-chain 站点适配——重写调用块（step_local = call + Goto B_N）、
    // 新建 resume 续点块 R_N（ResumeChainLink）与路由块 B_N（Complete/op 分流），
    // act 块含链式 housekeeping（TakeChainLink → frame link 槽；escape →
    // MakeContinuation；传播 → MakeChainLink + 重包装 Return）。
    // 返回 Rewrap act 块的 Step 载体 locals（供 wrap_returns_as_complete 跳过）。
    let mut chain_step_vals = adapt_call_chain_sites(
        body,
        &mut call_plans,
        frame_local,
        &live_to_slot,
        step_ty,
        step_fqn_sym,
        store,
        interner,
        fqn,
    );

    // escape 捕获的边界后缀克隆：把 resume_target 起、到 handle 出口为止的
    // 后缀克隆一份，指向出口的边改指到新块 `Return(result_local)`（随后由
    // wrap_returns_as_complete 包装为 Step::Complete）。resume 路径只走克隆，
    // 不会顺着出口继续执行调用方的剩余代码（初始路径走 arm → 出口）。
    // call-chain 站点（有 N_cap 的）同样从 R_N 克隆，并把克隆块内的
    // MakeChainLink state 从 N_prop 重写为 N_cap（resume 路径的传播挂起
    // 必须回到克隆续点，边界是 Complete 而非 handle 出口）。
    clone_escape_suffixes(body, &mut perform_sites, &escape_routing, &mut call_plans);

    // Resume 调用保持 `CallKind::Resume` 原样流向 LIR/codegen：
    // resumed 标志检查 + step_fn 间接调用由 codegen 基于 canonical continuation
    // 布局（scoop2_lir::effect）统一 lowering，MIR 不做字段级重写。

    // 添加 state dispatch 入口。
    let (state_local, resume_points) = if !perform_sites.is_empty() || !call_plans.is_empty() {
        add_state_dispatch(
            body,
            &perform_sites,
            &mut call_plans,
            frame_local,
            &live_to_slot,
            &escape_routing,
            store,
        )
    } else {
        // 无 Perform 但有 Resume：分配一个占位 state local。
        let sl = LocalId(body.locals.len() as u32);
        body.locals.push(LocalDecl {
            span: scoop2_base::Span::default(),
            name: Some("$state".to_string()),
            ty: store.int(),
            source: crate::mir::LocalSource::Temp,
            mutable: false,
        });
        (sl, Vec::new())
    };

    // 把所有"原始返回"（返回原始返回类型值的 Return）包装为
    // `Return(Step::Complete(value))`——EffectStep 函数的每个返回路径都必须
    // 产出 Step 值，不只是 Perform 站点产生的挂起返回。
    // escape 站点的 resume_local 不持有 Step 值（它是 resume 投递目标），
    // 不进入跳过集合；跳过集合 = outward Perform 的 carrier + Rewrap act 的
    // own_step locals（它们已经持有 Step 值）。
    let mut step_val_locals: HashSet<LocalId> = perform_carriers.into_iter().collect();
    step_val_locals.extend(chain_step_vals.drain(..));
    wrap_returns_as_complete(
        body,
        store,
        step_ty,
        step_fqn_sym,
        complete_sym,
        &step_val_locals,
    );

    // call-chain 站点信息入 ABI（坐标已被 add_state_dispatch 修正为 shift 后值），
    // 供阶段 B（adapt_calls）跳过已适配的调用点。
    let call_chain_sites: Vec<crate::mir::CallChainSite> = call_plans
        .iter()
        .map(|p| crate::mir::CallChainSite {
            block_idx: p.block_idx,
            stmt_idx: p.stmt_idx,
            callee_fqn: p.callee_fqn.clone(),
            target_local: p.target_local,
            result_ty: p.result_ty,
            link_slot: p.link_slot,
            state_cap: p.state_cap,
            state_prop: p.state_prop,
            step_local: p.step_local,
            resume_block: p.resume_block,
            dispatch_block: p.dispatch_block,
            step_ty: p.step_ty,
            step_fqn_sym: p.step_fqn_sym,
            step_variants: p.variants.clone(),
        })
        .collect();
    // 克隆块内同一调用语句的最终坐标：直接扫描克隆块找 callee 的 Direct
    // 调用（restore 语句可能在克隆后 splice 到块首，stmt 下标只能在 shift +
    // splice 都完成后定位）。
    let mut cloned_call_sites: Vec<(usize, usize)> = Vec::new();
    for p in &call_plans {
        for &bid in &p.cloned_call_blocks {
            let bi = bid.0 as usize;
            let Some(block) = body.blocks.get(bi) else {
                continue;
            };
            for (si, stmt) in block.stmts.iter().enumerate() {
                if let StatementKind::Assign {
                    value:
                        Rvalue::Call {
                            kind: CallKind::Direct { callee_fqn, .. },
                            ..
                        },
                    ..
                } = &stmt.kind
                {
                    if *callee_fqn == p.callee_fqn {
                        cloned_call_sites.push((bi, si));
                    }
                }
            }
        }
    }

    Some(crate::mir::EffectStepAbi {
        frame_ty,
        step_ty,
        step_variants,
        frame_local,
        state_local,
        resume_points,
        call_chain_sites,
        cloned_call_sites,
        escape_answer_tys,
    })
}

/// 把 body 中所有返回原始值的 Return 终结符包装为 Step::Complete 构造返回。
/// `step_val_locals`：已经持有 Step 值的 local（Perform 站点的 resume_local），
/// 返回这些 local 的 Return 跳过包装。
fn wrap_returns_as_complete(
    body: &mut Body,
    store: &mut TypeStore,
    step_ty: TypeId,
    step_fqn_sym: scoop2_base::Symbol,
    complete_sym: scoop2_base::Symbol,
    step_val_locals: &HashSet<LocalId>,
) {
    for bi in 0..body.blocks.len() {
        let (span, value) = match &body.blocks[bi].terminator.kind {
            TerminatorKind::Return { value } => (body.blocks[bi].terminator.span, value.clone()),
            _ => continue,
        };
        // 已持有 Step 值的返回：跳过。
        if let Some(Operand::Local(l)) = &value {
            if step_val_locals.contains(l) {
                continue;
            }
        }
        let (payload, payload_ty) = match &value {
            Some(op) => (op.clone(), operand_type(op, body)),
            None => (Operand::Const(crate::mir::ConstValue::Unit), store.unit()),
        };
        let complete_local = LocalId(body.locals.len() as u32);
        body.locals.push(LocalDecl {
            span,
            name: None,
            ty: step_ty,
            source: crate::mir::LocalSource::Temp,
            mutable: false,
        });
        body.blocks[bi].stmts.push(Statement {
            span,
            kind: StatementKind::Assign {
                target: complete_local,
                value: Rvalue::EnumVariant {
                    enum_ty: step_ty,
                    enum_fqn: step_fqn_sym,
                    variant_name: complete_sym,
                    args: vec![crate::mir::CallArg {
                        name: None,
                        is_spread: false,
                        value: payload,
                        value_ty: payload_ty,
                    }],
                    payload: AggregateTransportMetadata {
                        aggregate_ty: payload_ty,
                        kind: AggregateTransportKind::EnumPayload,
                        fields: Vec::new(),
                    },
                    stable_key: None,
                },
            },
        });
        body.blocks[bi].terminator = Terminator {
            span,
            kind: TerminatorKind::Return {
                value: Some(Operand::Local(complete_local)),
            },
        };
    }
}

/// 收集所有 Perform 站点。
/// 收集所有 Perform 终结符站点。
/// `live_in`：每个块入口处的活跃 local（`analyze::compute_live_in`）。
/// 保存/恢复集取 resume_target 的 **live_in**（不是 live_out——resume_target
/// 块内 def 前使用的 local 也必须恢复，例如 resume 后立即读取前次结果的场景）。
fn collect_perform_sites(body: &Body, live_in: &[HashSet<LocalId>]) -> Vec<PerformSite> {
    let mut sites = Vec::new();
    for (i, block) in body.blocks.iter().enumerate() {
        if let TerminatorKind::Perform {
            op_fqn,
            args,
            resume_local,
            resume_target,
            ..
        } = &block.terminator.kind
        {
            let resume_idx = resume_target.0 as usize;
            let live_set = if resume_idx < live_in.len() {
                live_in[resume_idx].clone()
            } else {
                HashSet::new()
            };
            // 排序 live locals 以确定性顺序。
            let mut live_vec: Vec<LocalId> = live_set.into_iter().collect();
            live_vec.sort_by_key(|l| l.0);
            sites.push(PerformSite {
                block_idx: i,
                op_fqn: op_fqn.clone(),
                args: args.iter().map(|a| a.value.clone()).collect(),
                resume_target: *resume_target,
                resume_local: *resume_local,
                live_out_at_resume: live_vec,
            });
        }
    }
    sites
}

// =========================================================================
// 阶段 2b：call-chain（case b）站点
// =========================================================================

/// call-chain 站点的路由（callee 的每个非 Complete Step 变体一条）。
#[derive(Clone)]
enum CallChainRoute {
    /// 普通 arm（非 escape）：act = TakeChainLink 丢弃（abandon 语义，清 TLS）
    /// + payload 解构到 binder + goto arm。
    Arm {
        binder_locals: Vec<LocalId>,
        target: BasicBlockId,
    },
    /// escape arm：act = housekeeping（保存 live + frame.state = N_cap +
    /// TakeChainLink → frame link 槽）+ MakeContinuation → k binder +
    /// payload 解构 + goto arm。
    EscapeArm {
        binder_locals: Vec<LocalId>,
        continuation_local: LocalId,
        target: BasicBlockId,
    },
    /// 传播：act = housekeeping（保存 live + frame.state = N_prop +
    /// TakeChainLink → frame link 槽 + MakeChainLink → TLS）+
    /// 用调用方自己的 Step 变体重包装 payload + Return。
    Rewrap { variant_sym: scoop2_base::Symbol },
}

/// 一条 EffectStep → EffectStep 调用边的变换计划（phase A 内部）。
struct CallChainSitePlan {
    /// 调用语句坐标（phase A 记录时的原始值；add_state_dispatch 修正为 shift 后）。
    block_idx: usize,
    stmt_idx: usize,
    callee_fqn: String,
    /// 原始调用结果目标 local / 类型（= callee Complete payload 类型）。
    target_local: LocalId,
    result_ty: TypeId,
    /// callee 的 Step 类型（nominal，跨函数同 TypeId）与 FQN Symbol。
    step_ty: TypeId,
    step_fqn_sym: scoop2_base::Symbol,
    /// callee 的 Step 变体表（Complete + outward ops，与 callee 自身 abi 一致）。
    variants: Vec<crate::mir::StepVariant>,
    /// 非 Complete 变体的路由（variant index in `variants` → route）。
    routes: Vec<(usize, CallChainRoute)>,
    /// 跨调用存活的 live locals（保存/恢复集；overapprox：live_in[bi] ∪
    /// 调用点之后的 uses）。
    live_set: Vec<LocalId>,
    /// frame 中的 link 槽下标（frame 构造时分配）。
    link_slot: u128,
    /// escape 捕获态 / 传播态编号。
    state_cap: Option<u128>,
    state_prop: Option<u128>,
    /// enclosing handle 的 escape 上下文（exit_target, result_local），克隆边界用。
    esc: Option<(BasicBlockId, LocalId)>,
    /// adapt 回填：resume 续点块 R_N / 路由块 B_N / step_local。
    resume_block: BasicBlockId,
    dispatch_block: BasicBlockId,
    step_local: LocalId,
    /// clone 回填：N_cap 的 dispatch 目标（克隆 R_N）。
    cloned_resume_block: Option<BasicBlockId>,
    /// clone 回填：包含本站点调用语句的克隆块 id（escape 后缀克隆可能把
    /// 调用块一并复制；add_state_dispatch 时同步 shift，阶段 B 跳过用）。
    cloned_call_blocks: Vec<BasicBlockId>,
}

/// 收集 body 内的 call-chain 站点（Direct 调用 EffectStep callee，且至少一个
/// callee outward op 需要 escape 捕获或向外传播）。
///
/// 全部由普通 arm 覆盖的站点不生成计划（留给阶段 B 旧路径 + TakeChainLink-drop）。
/// `state_base`：state 编号起点（perform 站点占 1..state_base-1）。
#[allow(clippy::too_many_arguments)]
fn collect_call_chain_sites(
    body: &Body,
    live_in: &[HashSet<LocalId>],
    effect_step_fqns: &HashSet<String>,
    canon_outward: &HashMap<String, Vec<(String, TypeId)>>,
    canon_map: &HashMap<String, String>,
    caller_outward: &[(String, TypeId)],
    state_base: u128,
    store: &mut TypeStore,
    interner: &Interner,
    fun_sites: &[(usize, usize, Vec<(String, TypeId)>)],
) -> Vec<CallChainSitePlan> {
    let handles = collect_handle_info(body);
    let mut handle_regions: Vec<(HashSet<usize>, &HandleInfo)> = Vec::new();
    for h in &handles {
        let region = find_handle_body_region(body, h);
        handle_regions.push((region, h));
    }
    // 区域按大小升序（内层 handle 在前）——嵌套 handle 的 arm 覆盖查找从
    // 最内层向外层逐层进行：内层未覆盖的 op 传播到外层 handler。
    let mut regions_inner_first: Vec<&(HashSet<usize>, &HandleInfo)> =
        handle_regions.iter().collect();
    regions_inner_first.sort_by_key(|(region, _)| region.len());
    let mut plans = Vec::new();
    let mut next_state = state_base;
    for (bi, block) in body.blocks.iter().enumerate() {
        for (si, stmt) in block.stmts.iter().enumerate() {
            // (target, callee 显示名, result_ty, 站点 op 集, 站点 Step 标识 Symbol,
            //  是否间接站点)。间接站点（FunValue/Closure）callee 静态未知：
            // op 集取自 classify 展开的函数值效果行，Step 标识用 default Symbol
            // （合成 Step 类型，tag 由 LIR 按站点变体表登记）。
            let (target, callee_fqn, result_ty, callee_ops, site_sym, is_indirect) =
                match &stmt.kind {
                    StatementKind::Assign {
                        target,
                        value:
                            Rvalue::Call {
                                kind: CallKind::Direct { callee_fqn, .. },
                                transport,
                                ..
                            },
                    } if effect_step_fqns.contains(callee_fqn) => {
                        let Some(ops) = canon_outward.get(callee_fqn) else {
                            continue;
                        };
                        (
                            *target,
                            callee_fqn.clone(),
                            transport.result.source_ty,
                            ops.clone(),
                            interner.get(callee_fqn).unwrap_or_default(),
                            false,
                        )
                    }
                    StatementKind::Assign {
                        target,
                        value:
                            Rvalue::Call {
                                kind: CallKind::FunValue { .. } | CallKind::Closure { .. },
                                transport,
                                ..
                            },
                    } => {
                        let Some((_, _, row_ops)) = fun_sites
                            .iter()
                            .find(|(fbi, fsi, _)| *fbi == bi && *fsi == si)
                        else {
                            continue;
                        };
                        // v1 仅支持单 op 效果行：站点 Step 变体按位置对 tag
                        // （Complete=0, op=1），多 op 行的位置序与动态 callee 的
                        // outward 序无法静态保证一致，误配会导致 tag 错位——跳过
                        // （退化为旧的未适配行为，不产生错误结果）。
                        if row_ops.len() != 1 {
                            continue;
                        }
                        (
                            *target,
                            format!("<fun_value@{bi}:{si}>"),
                            transport.result.source_ty,
                            row_ops.clone(),
                            scoop2_base::Symbol::default(),
                            true,
                        )
                    }
                    _ => continue,
                };
            let callee_fqn_sym = site_sym;
            // 构造 callee 的 Step 变体表（与 callee 自身 lower_to_effect_step
            // 的产物一致：Complete name_sym = callee FQN Symbol；op 变体
            // name_sym = 规范 op FQN Symbol）。间接站点的 Complete name_sym =
            // default Symbol（与 site_sym 一致，LIR tag 表按 (step_fqn_sym,
            // name_sym) 登记，Complete 恒 tag 0）。
            let mut variants: Vec<crate::mir::StepVariant> = vec![crate::mir::StepVariant {
                name: "Complete".to_string(),
                name_sym: callee_fqn_sym,
                payload_ty: result_ty,
                is_complete: true,
            }];
            for (op, payload_ty) in &callee_ops {
                variants.push(crate::mir::StepVariant {
                    name: op.replace('.', "_"),
                    name_sym: interner.get(op).unwrap_or_default(),
                    payload_ty: *payload_ty,
                    is_complete: false,
                });
            }
            // 逐变体路由。
            let mut routes: Vec<(usize, CallChainRoute)> = Vec::new();
            let mut needs_cap = false;
            let mut needs_prop = false;
            // escape arm 所属 handle（克隆边界取该 handle 的出口/结果 local）。
            let mut esc_handle: Option<&HandleInfo> = None;
            for (vi, variant) in variants.iter().enumerate().skip(1) {
                // arm 覆盖查找：从最内层 handle 向外层逐层找第一个 arm 覆盖
                // 该 op 的 handle（嵌套 handle：内层未覆盖的 op 向外传播）。
                let arm_route = regions_inner_first
                    .iter()
                    .filter(|(region, _)| region.contains(&bi))
                    .find_map(|(_, h)| {
                        h.arm_dispatch
                            .iter()
                            .find(|(aop, _)| {
                                canon_map.get(*aop).map(|c| c.replace('.', "_"))
                                    == Some(variant.name.clone())
                                    || aop.replace('.', "_") == variant.name
                            })
                            .map(|r| (*h, r))
                    });
                if let Some((h, (_, route))) = arm_route {
                    if let Some(k) = route.continuation_local {
                        needs_cap = true;
                        esc_handle = Some(h);
                        routes.push((
                            vi,
                            CallChainRoute::EscapeArm {
                                binder_locals: route.binder_locals.clone(),
                                continuation_local: k,
                                target: route.target,
                            },
                        ));
                    } else {
                        routes.push((
                            vi,
                            CallChainRoute::Arm {
                                binder_locals: route.binder_locals.clone(),
                                target: route.target,
                            },
                        ));
                    }
                    continue;
                }
                // 传播：op 必须在 caller 的 outward 中（classify 定点保证）。
                // 变体符号 = op 规范 FQN Symbol（与 caller 自身 step_variants 一致）。
                if caller_outward
                    .iter()
                    .any(|(o, _)| o.replace('.', "_") == variant.name)
                {
                    needs_prop = true;
                    routes.push((
                        vi,
                        CallChainRoute::Rewrap {
                            variant_sym: variant.name_sym,
                        },
                    ));
                }
                // 否则：落入 panic 分支（不生成路由）。
            }
            if !needs_cap && !needs_prop {
                continue;
            }
            // live 集：live_in[bi] ∪ 调用点之后的 uses（overapprox 安全）。
            let mut live: HashSet<LocalId> = live_in.get(bi).cloned().unwrap_or_default();
            let empty_defs: HashSet<LocalId> = HashSet::new();
            for s in &block.stmts[si + 1..] {
                if let StatementKind::Assign { value, .. } = &s.kind {
                    analyze::collect_rvalue_uses(value, &mut live, &empty_defs);
                }
            }
            analyze::collect_terminator_uses(&block.terminator.kind, &mut live, &empty_defs);
            let mut live_set: Vec<LocalId> = live.into_iter().collect();
            live_set.sort_by_key(|l| l.0);
            let state_cap = if needs_cap {
                let s = next_state;
                next_state += 1;
                Some(s)
            } else {
                None
            };
            let state_prop = if needs_prop {
                let s = next_state;
                next_state += 1;
                Some(s)
            } else {
                None
            };
            let esc = if needs_cap {
                esc_handle.map(|h| (h.exit_target, h.result_local))
            } else {
                None
            };
            // 站点 Step 类型：Direct 站点 = callee 的 Step nominal（fqn = callee
            // FQN Symbol，与 callee 自身 abi.step_ty 同 TypeId）；间接站点 =
            // 合成 nominal（fqn = default Symbol，args 编码 Complete/op payload
            // 类型）——args 使不同 payload 组合的站点得到不同 TypeId，保证 LIR
            // 按站点变体表登记的布局互不串扰。
            let step_ty = if is_indirect {
                let mut args = vec![result_ty];
                args.extend(variants.iter().skip(1).map(|v| v.payload_ty));
                store.value_nominal(scoop2_hir::ty::NominalType {
                    fqn: scoop2_base::Symbol::default(),
                    args,
                    eff: None,
                })
            } else {
                store.value_nominal(scoop2_hir::ty::NominalType {
                    fqn: callee_fqn_sym,
                    args: vec![],
                    eff: None,
                })
            };
            plans.push(CallChainSitePlan {
                block_idx: bi,
                stmt_idx: si,
                callee_fqn,
                target_local: target,
                result_ty,
                step_ty,
                step_fqn_sym: callee_fqn_sym,
                variants,
                routes,
                live_set,
                link_slot: 0, // frame 构造时分配
                state_cap,
                state_prop,
                esc,
                resume_block: BasicBlockId(0),
                dispatch_block: BasicBlockId(0),
                step_local: LocalId(0),
                cloned_resume_block: None,
                cloned_call_blocks: Vec::new(),
            });
        }
    }
    plans
}

/// call-chain 站点适配（phase A 主体）：对每个计划重写调用块并生成
/// R_N（resume 续点）/ B_N（路由）/ complete / check / act / panic 块。
///
/// 返回 Rewrap act 块的 Step 载体 locals（供 wrap_returns_as_complete 跳过）。
#[allow(clippy::too_many_arguments)]
fn adapt_call_chain_sites(
    body: &mut Body,
    plans: &mut [CallChainSitePlan],
    frame_local: LocalId,
    live_to_slot: &HashMap<LocalId, u128>,
    caller_step_ty: TypeId,
    caller_step_fqn_sym: scoop2_base::Symbol,
    store: &mut TypeStore,
    _interner: &Interner,
    _caller_fqn: &str,
) -> Vec<LocalId> {
    let mut step_val_locals: Vec<LocalId> = Vec::new();
    // 逆序处理（避免同块多调用点的语句索引偏移）。
    let mut order: Vec<usize> = (0..plans.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse((plans[i].block_idx, plans[i].stmt_idx)));
    let bool_ty = store.bool();
    let any_ty = store.any();
    let unit_ty = store.unit();
    let int_ty = store.int();
    for pi in order {
        // 先取出需要的计划字段（避免与 body 的可变借用冲突）。
        let (
            bi,
            si,
            step_ty,
            step_fqn_sym,
            link_slot,
            state_cap,
            state_prop,
            result_ty,
            target_local,
        ) = {
            let p = &plans[pi];
            (
                p.block_idx,
                p.stmt_idx,
                p.step_ty,
                p.step_fqn_sym,
                p.link_slot,
                p.state_cap,
                p.state_prop,
                p.result_ty,
                p.target_local,
            )
        };
        let complete_sym = step_fqn_sym; // Complete 变体 name_sym = callee FQN Symbol
        let span = body.blocks[bi].stmts[si].span;
        // 分配 locals。
        let step_local = alloc_temp_local(body, span, step_ty);
        let chain_tmp = alloc_temp_local(body, span, any_ty);
        let unit_sink = alloc_temp_local(body, span, unit_ty);
        let cond_local = alloc_temp_local(body, span, bool_ty);
        // 保存原块信息。
        let orig_terminator = body.blocks[bi].terminator.clone();
        let orig_span_term = body.blocks[bi].terminator.span;
        let after_stmts: Vec<Statement> = body.blocks[bi].stmts[si + 1..].to_vec();
        let before_stmts: Vec<Statement> = body.blocks[bi].stmts[..si].to_vec();
        let mut orig_value = match &body.blocks[bi].stmts[si].kind {
            StatementKind::Assign { value, .. } => value.clone(),
            _ => Rvalue::Use(Operand::Const(crate::mir::ConstValue::Unit)),
        };
        // 间接调用站点（FunValue/Closure）：调用的静态结果类型改写为本站点的
        // 合成 Step 类型——LIR 的 call result_ty 取自 transport.result.source_ty，
        // codegen 按它构造 fn_ptr 的返回类型；不改写则按原始返回类型（如 i64）
        // 调用返回 Step 聚合（{i8,[N x i8]}）的 wrapper，ABI 不匹配直接踩内存。
        // （Direct 站点不需要：codegen 取 callee 的真实 FunctionValue 声明。）
        if let Rvalue::Call {
            kind: CallKind::FunValue { .. } | CallKind::Closure { .. },
            transport,
            ..
        } = &mut orig_value
        {
            transport.result.source_ty = step_ty;
        }
        // R_N 的 restore 语句（恢复 live locals；step_local 由 ResumeChainLink
        // 写入，不在 live 集中）。
        let mut restore_stmts: Vec<Statement> = Vec::new();
        let live_snapshot = plans[pi].live_set.clone();
        for live_local in &live_snapshot {
            let Some(&slot_idx) = live_to_slot.get(live_local) else {
                continue;
            };
            let live_ty = body
                .locals
                .get(live_local.0 as usize)
                .map(|d| d.ty)
                .unwrap_or(any_ty);
            let temp_local = alloc_temp_local(body, span, live_ty);
            restore_stmts.push(Statement {
                span,
                kind: StatementKind::Assign {
                    target: temp_local,
                    value: Rvalue::TupleIndex {
                        receiver: Operand::Local(frame_local),
                        index: slot_idx,
                        element_ty: live_ty,
                    },
                },
            });
            restore_stmts.push(Statement {
                span,
                kind: StatementKind::Assign {
                    target: *live_local,
                    value: Rvalue::Use(Operand::Local(temp_local)),
                },
            });
        }
        // housekeeping（保存 live + frame.state + TakeChainLink → link 槽），
        // escape 与传播 act 块共用。
        let save_live_stmts = |body: &mut Body| -> Vec<Statement> {
            let mut stmts = Vec::new();
            for live_local in &live_snapshot {
                let Some(&slot_idx) = live_to_slot.get(live_local) else {
                    continue;
                };
                let live_ty = body
                    .locals
                    .get(live_local.0 as usize)
                    .map(|d| d.ty)
                    .unwrap_or(any_ty);
                stmts.push(Statement {
                    span,
                    kind: StatementKind::StoreTupleIndex {
                        receiver: Operand::Local(frame_local),
                        index: slot_idx,
                        value: Operand::Local(*live_local),
                        value_ty: live_ty,
                    },
                });
            }
            stmts
        };
        let housekeeping = |body: &mut Body, state: u128| -> Vec<Statement> {
            let mut stmts = save_live_stmts(body);
            // frame.state = state。
            stmts.push(Statement {
                span,
                kind: StatementKind::StoreTupleIndex {
                    receiver: Operand::Local(frame_local),
                    index: 0,
                    value: Operand::Const(crate::mir::ConstValue::Int(state, None)),
                    value_ty: int_ty,
                },
            });
            // chain_tmp = TakeChainLink（消费 callee link）。
            stmts.push(Statement {
                span,
                kind: StatementKind::Assign {
                    target: chain_tmp,
                    value: Rvalue::TakeChainLink { result_ty: any_ty },
                },
            });
            // frame[link_slot] = chain_tmp。
            stmts.push(Statement {
                span,
                kind: StatementKind::StoreTupleIndex {
                    receiver: Operand::Local(frame_local),
                    index: link_slot,
                    value: Operand::Local(chain_tmp),
                    value_ty: any_ty,
                },
            });
            stmts
        };
        // payload 解构语句（与阶段 B 的多/单 binder 逻辑一致）。
        let variants = plans[pi].variants.clone();
        let routes = plans[pi].routes.clone();
        let bind_payload =
            |body: &mut Body, payload_ty: TypeId, binder_locals: &[LocalId]| -> Vec<Statement> {
                let mut stmts = Vec::new();
                if binder_locals.is_empty() {
                    return stmts;
                }
                let payload_local = {
                    let id = LocalId(body.locals.len() as u32);
                    body.locals.push(LocalDecl {
                        span,
                        name: None,
                        ty: payload_ty,
                        source: crate::mir::LocalSource::Temp,
                        mutable: false,
                    });
                    id
                };
                stmts.push(Statement {
                    span,
                    kind: StatementKind::Assign {
                        target: payload_local,
                        value: Rvalue::PatternExtract {
                            subject: Operand::Local(step_local),
                            path: vec![],
                            result_ty: payload_ty,
                        },
                    },
                });
                let tuple_elems: Option<Vec<TypeId>> = if binder_locals.len() > 1 {
                    match store.kind(payload_ty) {
                        scoop2_hir::ty::TypeKind::Value(scoop2_hir::ty::ValueTypeKind::Tuple(
                            elems,
                        )) => Some(elems.clone()),
                        _ => None,
                    }
                } else {
                    None
                };
                for (bii, binder) in binder_locals.iter().enumerate() {
                    let value = match &tuple_elems {
                        Some(elems) => Rvalue::TupleIndex {
                            receiver: Operand::Local(payload_local),
                            index: bii as u128,
                            element_ty: elems.get(bii).copied().unwrap_or(payload_ty),
                        },
                        None => Rvalue::Use(Operand::Local(payload_local)),
                    };
                    stmts.push(Statement {
                        span,
                        kind: StatementKind::Assign {
                            target: *binder,
                            value,
                        },
                    });
                }
                stmts
            };
        // 块 id 布局（追加顺序）：R_N, B_N, complete, (check_i, act_i)*, panic。
        let base = body.blocks.len();
        let resume_bid = BasicBlockId(base as u32);
        let dispatch_bid = BasicBlockId((base + 1) as u32);
        let complete_bid = BasicBlockId((base + 2) as u32);
        let num_routes = routes.len();
        let check_bids: Vec<BasicBlockId> = (0..num_routes)
            .map(|i| BasicBlockId((base + 3 + i * 2) as u32))
            .collect();
        let act_bids: Vec<BasicBlockId> = (0..num_routes)
            .map(|i| BasicBlockId((base + 4 + i * 2) as u32))
            .collect();
        let panic_bid = BasicBlockId((base + 3 + num_routes * 2) as u32);

        // R_N：restore + step_local = ResumeChainLink + Goto B_N。
        let mut rn_stmts = restore_stmts;
        rn_stmts.push(Statement {
            span,
            kind: StatementKind::Assign {
                target: step_local,
                value: Rvalue::ResumeChainLink {
                    link_slot,
                    result_ty: step_ty,
                },
            },
        });
        body.blocks.push(crate::mir::BasicBlock {
            stmts: rn_stmts,
            terminator: Terminator {
                span,
                kind: TerminatorKind::Goto {
                    target: dispatch_bid,
                },
            },
        });
        // B_N：cond = PatternMatch(step_local, Complete) + CondBr。
        let first_else = check_bids.first().copied().unwrap_or(panic_bid);
        body.blocks.push(crate::mir::BasicBlock {
            stmts: vec![Statement {
                span,
                kind: StatementKind::Assign {
                    target: cond_local,
                    value: Rvalue::PatternMatch {
                        subject: Operand::Local(step_local),
                        pattern: crate::mir::Pattern::Variant {
                            enum_fqn: step_fqn_sym,
                            variant_name: complete_sym,
                            args: vec![],
                        },
                    },
                },
            }],
            terminator: Terminator {
                span,
                kind: TerminatorKind::CondBr {
                    cond: Operand::Local(cond_local),
                    then_target: complete_bid,
                    else_target: first_else,
                },
            },
        });
        // complete 块：清 link 槽（已消费）+ 提取结果 + 后续语句 + 原终结符。
        let mut complete_stmts = vec![
            Statement {
                span,
                kind: StatementKind::StoreTupleIndex {
                    receiver: Operand::Local(frame_local),
                    index: link_slot,
                    value: Operand::Const(crate::mir::ConstValue::Null),
                    value_ty: any_ty,
                },
            },
            Statement {
                span,
                kind: StatementKind::Assign {
                    target: target_local,
                    value: Rvalue::PatternExtract {
                        subject: Operand::Local(step_local),
                        path: vec![],
                        result_ty,
                    },
                },
            },
        ];
        complete_stmts.extend(after_stmts);
        body.blocks.push(crate::mir::BasicBlock {
            stmts: complete_stmts,
            terminator: orig_terminator,
        });
        // 同块内 stmt 下标更大的其他 call-chain 站点：其（已适配的）调用语句
        // 随 after_stmts 迁入了本 complete 块。同步修正其坐标——
        // `call_chain_sites` 记录的坐标供阶段 B（adapt_calls）按坐标跳过
        // 已适配调用，坐标过期会导致同一调用被重复适配（嵌套 dispatch 里
        // 再包一层 dispatch，路由信息丢失 → 误 panic "unhandled effect"）。
        // complete 块在 after_stmts 前固定 prepend 2 条语句（清 link 槽 +
        // 提取结果）。
        for pj in 0..plans.len() {
            if pj != pi && plans[pj].block_idx == bi && plans[pj].stmt_idx > si {
                plans[pj].block_idx = complete_bid.0 as usize;
                plans[pj].stmt_idx = 2 + (plans[pj].stmt_idx - si - 1);
            }
        }
        // check / act 块。
        for (ri, (vi, route)) in routes.iter().enumerate() {
            let variant = &variants[*vi];
            let else_target = check_bids.get(ri + 1).copied().unwrap_or(panic_bid);
            let check_cond = alloc_temp_local(body, span, bool_ty);
            body.blocks.push(crate::mir::BasicBlock {
                stmts: vec![Statement {
                    span,
                    kind: StatementKind::Assign {
                        target: check_cond,
                        value: Rvalue::PatternMatch {
                            subject: Operand::Local(step_local),
                            pattern: crate::mir::Pattern::Variant {
                                enum_fqn: step_fqn_sym,
                                variant_name: variant.name_sym,
                                args: vec![],
                            },
                        },
                    },
                }],
                terminator: Terminator {
                    span,
                    kind: TerminatorKind::CondBr {
                        cond: Operand::Local(check_cond),
                        then_target: act_bids[ri],
                        else_target,
                    },
                },
            });
            match route {
                CallChainRoute::Arm {
                    binder_locals,
                    target,
                } => {
                    // 普通 arm：TakeChainLink 丢弃（abandon 语义，清 TLS）。
                    let mut stmts = vec![Statement {
                        span,
                        kind: StatementKind::Assign {
                            target: chain_tmp,
                            value: Rvalue::TakeChainLink { result_ty: any_ty },
                        },
                    }];
                    stmts.extend(bind_payload(body, variant.payload_ty, binder_locals));
                    body.blocks.push(crate::mir::BasicBlock {
                        stmts,
                        terminator: Terminator {
                            span,
                            kind: TerminatorKind::Goto { target: *target },
                        },
                    });
                }
                CallChainRoute::EscapeArm {
                    binder_locals,
                    continuation_local,
                    target,
                } => {
                    let cap = state_cap.expect("EscapeArm 路由必有 state_cap");
                    let mut stmts = housekeeping(body, cap);
                    // k = MakeContinuation{state: N_cap}。
                    stmts.push(Statement {
                        span,
                        kind: StatementKind::Assign {
                            target: *continuation_local,
                            value: Rvalue::MakeContinuation { state: cap },
                        },
                    });
                    stmts.extend(bind_payload(body, variant.payload_ty, binder_locals));
                    body.blocks.push(crate::mir::BasicBlock {
                        stmts,
                        terminator: Terminator {
                            span,
                            kind: TerminatorKind::Goto { target: *target },
                        },
                    });
                }
                CallChainRoute::Rewrap { variant_sym } => {
                    let prop = state_prop.expect("Rewrap 路由必有 state_prop");
                    let mut stmts = housekeeping(body, prop);
                    // unit_sink = MakeChainLink{state: N_prop}（本层 link 写 TLS）。
                    stmts.push(Statement {
                        span,
                        kind: StatementKind::Assign {
                            target: unit_sink,
                            value: Rvalue::MakeChainLink { state: prop },
                        },
                    });
                    // 重包装：own_step = caller_Step::variant(payload)。
                    let payload_local = alloc_temp_local(body, span, variant.payload_ty);
                    stmts.push(Statement {
                        span,
                        kind: StatementKind::Assign {
                            target: payload_local,
                            value: Rvalue::PatternExtract {
                                subject: Operand::Local(step_local),
                                path: vec![],
                                result_ty: variant.payload_ty,
                            },
                        },
                    });
                    let own_step_local = alloc_temp_local(body, span, caller_step_ty);
                    step_val_locals.push(own_step_local);
                    stmts.push(Statement {
                        span,
                        kind: StatementKind::Assign {
                            target: own_step_local,
                            value: Rvalue::EnumVariant {
                                enum_ty: caller_step_ty,
                                enum_fqn: caller_step_fqn_sym,
                                variant_name: *variant_sym,
                                args: vec![crate::mir::CallArg {
                                    name: None,
                                    is_spread: false,
                                    value: Operand::Local(payload_local),
                                    value_ty: variant.payload_ty,
                                }],
                                payload: AggregateTransportMetadata {
                                    aggregate_ty: variant.payload_ty,
                                    kind: AggregateTransportKind::EnumPayload,
                                    fields: Vec::new(),
                                },
                                stable_key: None,
                            },
                        },
                    });
                    body.blocks.push(crate::mir::BasicBlock {
                        stmts,
                        terminator: Terminator {
                            span,
                            kind: TerminatorKind::Return {
                                value: Some(Operand::Local(own_step_local)),
                            },
                        },
                    });
                }
            }
        }
        // panic 块：未处理的 effect（无 handler 覆盖且无法传播）。
        body.blocks.push(crate::mir::BasicBlock {
            stmts: vec![Statement {
                span,
                kind: StatementKind::Panic {
                    message: format!(
                        "unhandled effect: no handler for step of {}",
                        plans[pi].callee_fqn
                    ),
                },
            }],
            terminator: Terminator {
                span,
                kind: TerminatorKind::Unreachable,
            },
        });
        // 重写调用块：before_stmts + step_local = call + Goto B_N。
        let mut new_stmts = before_stmts;
        new_stmts.push(Statement {
            span,
            kind: StatementKind::Assign {
                target: step_local,
                value: orig_value,
            },
        });
        body.blocks[bi].stmts = new_stmts;
        body.blocks[bi].terminator = Terminator {
            span: orig_span_term,
            kind: TerminatorKind::Goto {
                target: dispatch_bid,
            },
        };
        // 回填计划。
        let p = &mut plans[pi];
        p.step_local = step_local;
        p.resume_block = resume_bid;
        p.dispatch_block = dispatch_bid;
    }
    step_val_locals
}

/// 重写 Perform 站点：保存 live locals 到 frame + 返回 Step case。
///
/// `outward_ops`：规范化 op 列表（下标 + 1 = 变体 tag）。站点构造的 Step 变体
/// 符号取 op 的规范 FQN Symbol（与 step_variants 的 name_sym 一致）。
///
/// 返回 outward 站点的 Step 载体 local 列表（供 wrap_returns_as_complete 跳过）。
/// outward 站点还会写 chain link 到 TLS（`MakeChainLink`）——callee 的挂起经
/// TLS 单槽传给 caller 的 act 块（TakeChainLink），组成 case b 的调用链。
#[allow(clippy::too_many_arguments)]
fn rewrite_perform_sites(
    body: &mut Body,
    sites: &[PerformSite],
    live_to_slot: &HashMap<LocalId, u128>,
    frame_local: LocalId,
    frame_ty: TypeId,
    step_ty: TypeId,
    step_fqn_sym: scoop2_base::Symbol,
    outward_ops: &[(String, TypeId)],
    store: &mut TypeStore,
    interner: &Interner,
    escape_routing: &HashMap<usize, PerformRoute>,
) -> Vec<LocalId> {
    let mut step_carriers: Vec<LocalId> = Vec::new();
    for (state_num, site) in sites.iter().enumerate() {
        let state_num = (state_num + 1) as u128; // 1-based
        let span = body.blocks[site.block_idx].terminator.span;
        let escape_route = escape_routing.get(&site.block_idx);
        // 预计算 live locals 的类型（避免借用冲突）。
        let live_local_tys: Vec<(LocalId, u128, TypeId)> = site
            .live_out_at_resume
            .iter()
            .filter_map(|&live_local| {
                let slot_idx = *live_to_slot.get(&live_local)?;
                let live_ty = body
                    .locals
                    .get(live_local.0 as usize)
                    .map(|d| d.ty)
                    .unwrap_or_else(|| store.any());
                Some((live_local, slot_idx, live_ty))
            })
            .collect();
        // 预计算 payload（escape 站点不需要——实参直接绑到 arm binder）。
        let (payload, payload_ty) = if escape_route.is_some() {
            (Operand::Const(crate::mir::ConstValue::Unit), store.unit())
        } else if site.args.len() == 1 {
            let ty = operand_type(&site.args[0], body);
            (site.args[0].clone(), ty)
        } else if site.args.is_empty() {
            (Operand::Const(crate::mir::ConstValue::Unit), store.unit())
        } else {
            let arg_tys: Vec<TypeId> = site.args.iter().map(|op| operand_type(op, body)).collect();
            let tuple_ty = store.tuple(arg_tys.clone());
            let tuple_local = LocalId(body.locals.len() as u32);
            body.locals.push(LocalDecl {
                span,
                name: None,
                ty: tuple_ty,
                source: crate::mir::LocalSource::Temp,
                mutable: false,
            });
            (Operand::Local(tuple_local), tuple_ty)
        };
        // 该站点的 Step 变体符号：op 的规范 FQN Symbol（与 step_variants 一致）。
        let variant_name_sym = outward_ops
            .iter()
            .find(|(op, _)| op == &site.op_fqn)
            .map(|(op, _)| interner.get(op).unwrap_or_default())
            .unwrap_or_default();

        let block = &mut body.blocks[site.block_idx];

        // 1. 保存 live locals 到 frame tuple。
        for (live_local, slot_idx, live_ty) in &live_local_tys {
            block.stmts.push(Statement {
                span,
                kind: StatementKind::StoreTupleIndex {
                    receiver: Operand::Local(frame_local),
                    index: *slot_idx,
                    value: Operand::Local(*live_local),
                    value_ty: *live_ty,
                },
            });
        }

        // 2. 设置 frame.state = state_num。
        block.stmts.push(Statement {
            span,
            kind: StatementKind::StoreTupleIndex {
                receiver: Operand::Local(frame_local),
                index: 0,
                value: Operand::Const(crate::mir::ConstValue::Int(state_num as u128, None)),
                value_ty: store.int(),
            },
        });

        // escape 捕获：构造 continuation → k，绑定实参 → arm binder，goto arm。
        // resume_local 保持原类型（它是 resume 值投递目标，不是 Step 载体）。
        if let Some(route) = escape_route {
            let esc = route
                .escape
                .as_ref()
                .expect("escape 路由必有 EscapeCapture");
            block.stmts.push(Statement {
                span,
                kind: StatementKind::Assign {
                    target: esc.continuation_local,
                    value: Rvalue::MakeContinuation { state: state_num },
                },
            });
            for (i, binder) in route.binder_locals.iter().enumerate() {
                if let Some(arg) = site.args.get(i) {
                    block.stmts.push(Statement {
                        span,
                        kind: StatementKind::Assign {
                            target: *binder,
                            value: Rvalue::Use(arg.clone()),
                        },
                    });
                }
            }
            block.terminator = Terminator {
                span,
                kind: TerminatorKind::Goto {
                    target: route.arm_target,
                },
            };
            continue;
        }

        // 3. 挂起前写 chain link 到 TLS：callee（本函数）的挂起经 TLS 单槽
        // 传给 caller 的 act 块（TakeChainLink 消费），组成 case b 调用链。
        // link 记录本函数 frame + `sym$step`，外层 resume 时逐层恢复。
        let chain_sink = LocalId(body.locals.len() as u32);
        body.locals.push(LocalDecl {
            span,
            name: None,
            ty: store.unit(),
            source: crate::mir::LocalSource::Temp,
            mutable: false,
        });
        block.stmts.push(Statement {
            span,
            kind: StatementKind::Assign {
                target: chain_sink,
                value: Rvalue::MakeChainLink { state: state_num },
            },
        });

        // 4. 如果 payload 是 tuple，需要先构造 tuple。
        if let Operand::Local(tuple_local) = &payload {
            // 检查这个 local 是否是我们刚刚分配的（payload_ty 是 tuple）。
            // 如果是多参数 Perform，需要先构造 tuple。
            if site.args.len() > 1 {
                block.stmts.push(Statement {
                    span,
                    kind: StatementKind::Assign {
                        target: *tuple_local,
                        value: Rvalue::MakeTuple {
                            elements: site.args.clone(),
                            transport: AggregateTransportMetadata {
                                aggregate_ty: payload_ty,
                                kind: AggregateTransportKind::Tuple,
                                fields: Vec::new(),
                            },
                        },
                    },
                });
            }
        }

        // 5. Step 载体：独立的 carrier local（step_ty）。resume_local 保持
        // op 结果类型不变——它是 chain resume 时 word 投递的目标（见
        // add_state_dispatch 的 resume_points），不能改作 Step 载体。
        let carrier = LocalId(body.locals.len() as u32);
        body.locals.push(LocalDecl {
            span,
            name: None,
            ty: step_ty,
            source: crate::mir::LocalSource::Temp,
            mutable: false,
        });
        step_carriers.push(carrier);
        block.stmts.push(Statement {
            span,
            kind: StatementKind::Assign {
                target: carrier,
                value: Rvalue::EnumVariant {
                    enum_ty: step_ty,
                    enum_fqn: step_fqn_sym,
                    variant_name: variant_name_sym,
                    args: vec![crate::mir::CallArg {
                        name: None,
                        is_spread: false,
                        value: payload,
                        value_ty: payload_ty,
                    }],
                    payload: AggregateTransportMetadata {
                        aggregate_ty: payload_ty,
                        kind: AggregateTransportKind::EnumPayload,
                        fields: Vec::new(),
                    },
                    stable_key: None,
                },
            },
        });

        // 6. Perform → Return(Step case)。
        block.terminator = Terminator {
            span,
            kind: TerminatorKind::Return {
                value: Some(Operand::Local(carrier)),
            },
        };
    }
    let _ = frame_ty;
    step_carriers
}

/// escape continuation 捕获的边界后缀克隆。
///
/// 对每个 escape 站点：从 resume_target 起 BFS（不越过 handle 出口 exit_target），
/// 把可达块整体克隆一份；克隆中指向 exit_target 的边改指到新建的
/// `Return(result_local)` 块（随后由 wrap_returns_as_complete 包装为
/// `Return(Step::Complete(result_local))`）。site.resume_target 更新为克隆块 id，
/// 供 state dispatch 把 resume 路径导向克隆后缀——初始路径（perform → arm →
/// 出口）与 resume 路径（克隆后缀 → Complete 返回）由此分离。
///
/// BFS 还纳入**同 handle 的嵌套 escape 站点的 arm**：resume 路径上再次 perform
/// 时控制流会 `Goto arm`（rewrite_perform_sites 的产物），该 arm 流向同一
/// exit_target——若不克隆 arm，resume 路径会顺着原始出口重放调用方剩余代码。
/// 逸出到**外层** handle 的 perform（exit_target 不同）不克隆其 arm（case b
/// 暂不支持，边保持指向原始 arm）。
///
/// call-chain 站点（有 N_cap 的）同样克隆：BFS 从 R_N 起，边界为 enclosing
/// handle 的 exit_target；克隆块内的 `MakeChainLink{state: N_prop}` 重写为
/// N_cap——resume 路径上的传播挂起必须回到**克隆**续点（边界是 Complete
/// 而非 handle 出口）。N_cap 的 dispatch 目标（克隆 R_N）回填到
/// plan.cloned_resume_block。perform 站点的克隆也应用同一 state 重写
/// （其 BFS 区域可能附带 call-chain 站点的 act 块）。
fn clone_escape_suffixes(
    body: &mut Body,
    sites: &mut [PerformSite],
    escape_routing: &HashMap<usize, PerformRoute>,
    call_plans: &mut [CallChainSitePlan],
) {
    // N_prop → N_cap 重写映射（所有同时具备两个 state 的 call-chain 站点）。
    let prop_to_cap: HashMap<u128, u128> = call_plans
        .iter()
        .filter_map(|p| match (p.state_prop, p.state_cap) {
            (Some(prop), Some(cap)) => Some((prop, cap)),
            _ => None,
        })
        .collect();
    for site in sites.iter_mut() {
        let Some(route) = escape_routing.get(&site.block_idx) else {
            continue;
        };
        let Some(esc) = &route.escape else { continue };
        let exit_idx = esc.exit_target.0 as usize;
        // BFS 收集区域（不越过 exit_target；同 handle 的嵌套 escape 站点的
        // arm 一并纳入）。
        let mut region: Vec<usize> = Vec::new();
        let mut seen: HashSet<usize> = HashSet::new();
        let mut queue: std::collections::VecDeque<usize> =
            std::collections::VecDeque::from([site.resume_target.0 as usize]);
        while let Some(b) = queue.pop_front() {
            if b == exit_idx || b >= body.blocks.len() || !seen.insert(b) {
                continue;
            }
            region.push(b);
            for t in terminator_targets(&body.blocks[b].terminator.kind) {
                queue.push_back(t.0 as usize);
            }
            // 块 b 是同 handle 的嵌套 escape 站点 → 其 arm 也在边界内。
            if let Some(nested) = escape_routing.get(&b) {
                if nested
                    .escape
                    .as_ref()
                    .is_some_and(|e2| e2.exit_target.0 as usize == exit_idx)
                {
                    queue.push_back(nested.arm_target.0 as usize);
                }
            }
        }
        if region.is_empty() {
            continue;
        }
        // 克隆块 id 映射；complete 块在克隆块之后。
        let base = body.blocks.len();
        let id_map: HashMap<usize, usize> = region
            .iter()
            .enumerate()
            .map(|(i, &b)| (b, base + i))
            .collect();
        // 区域内的 call-chain 站点调用块被一并克隆：记录克隆块 id，
        // 阶段 B 需要跳过克隆里的同一调用语句。
        for p in call_plans.iter_mut() {
            if let Some(&cloned) = id_map.get(&p.block_idx) {
                p.cloned_call_blocks.push(BasicBlockId(cloned as u32));
            }
        }
        let complete_bid = base + region.len();
        let span = body.blocks[site.resume_target.0 as usize].terminator.span;
        for &b in &region {
            let mut nb = body.blocks[b].clone();
            remap_terminator_targets(&mut nb.terminator.kind, &mut |t| {
                if t.0 as usize == exit_idx {
                    BasicBlockId(complete_bid as u32)
                } else if let Some(&mapped) = id_map.get(&(t.0 as usize)) {
                    BasicBlockId(mapped as u32)
                } else {
                    t
                }
            });
            rewrite_chain_states_in_block(&mut nb, &prop_to_cap);
            body.blocks.push(nb);
        }
        body.blocks.push(crate::mir::BasicBlock {
            stmts: vec![],
            terminator: Terminator {
                span,
                kind: TerminatorKind::Return {
                    value: Some(Operand::Local(esc.result_local)),
                },
            },
        });
        site.resume_target = BasicBlockId(id_map[&(site.resume_target.0 as usize)] as u32);
    }
    // call-chain 站点的克隆（有 escape 覆盖的站点）。
    for pi in 0..call_plans.len() {
        let (resume_block, esc, state_cap) = {
            let p = &call_plans[pi];
            (p.resume_block, p.esc, p.state_cap)
        };
        let (Some((exit_target, result_local)), Some(_)) = (esc, state_cap) else {
            continue;
        };
        let exit_idx = exit_target.0 as usize;
        // BFS 从 R_N 起（不越过 exit_target；同 handle 的嵌套 escape perform
        // 站点的 arm 一并纳入——resume 路径上 re-perform 会 Goto arm）。
        let mut region: Vec<usize> = Vec::new();
        let mut seen: HashSet<usize> = HashSet::new();
        let mut queue: std::collections::VecDeque<usize> =
            std::collections::VecDeque::from([resume_block.0 as usize]);
        while let Some(b) = queue.pop_front() {
            if b == exit_idx || b >= body.blocks.len() || !seen.insert(b) {
                continue;
            }
            region.push(b);
            for t in terminator_targets(&body.blocks[b].terminator.kind) {
                queue.push_back(t.0 as usize);
            }
            if let Some(nested) = escape_routing.get(&b) {
                if nested
                    .escape
                    .as_ref()
                    .is_some_and(|e2| e2.exit_target.0 as usize == exit_idx)
                {
                    queue.push_back(nested.arm_target.0 as usize);
                }
            }
        }
        if region.is_empty() {
            continue;
        }
        let base = body.blocks.len();
        let id_map: HashMap<usize, usize> = region
            .iter()
            .enumerate()
            .map(|(i, &b)| (b, base + i))
            .collect();
        let complete_bid = base + region.len();
        let span = body.blocks[resume_block.0 as usize].terminator.span;
        // 区域内若包含其他 call-chain 站点的调用块（complete 续行路径上的
        // 后续间接调用），同样记录克隆块 id 供阶段 B 跳过。
        for pj in 0..call_plans.len() {
            if let Some(&cloned) = id_map.get(&call_plans[pj].block_idx) {
                call_plans[pj]
                    .cloned_call_blocks
                    .push(BasicBlockId(cloned as u32));
            }
        }
        for &b in &region {
            let mut nb = body.blocks[b].clone();
            remap_terminator_targets(&mut nb.terminator.kind, &mut |t| {
                if t.0 as usize == exit_idx {
                    BasicBlockId(complete_bid as u32)
                } else if let Some(&mapped) = id_map.get(&(t.0 as usize)) {
                    BasicBlockId(mapped as u32)
                } else {
                    t
                }
            });
            rewrite_chain_states_in_block(&mut nb, &prop_to_cap);
            body.blocks.push(nb);
        }
        body.blocks.push(crate::mir::BasicBlock {
            stmts: vec![],
            terminator: Terminator {
                span,
                kind: TerminatorKind::Return {
                    value: Some(Operand::Local(result_local)),
                },
            },
        });
        call_plans[pi].cloned_resume_block =
            Some(BasicBlockId(id_map[&(resume_block.0 as usize)] as u32));
    }
}

/// 把块内 `MakeChainLink{state}` 的 state 按映射重写（N_prop → N_cap）。
/// 用于克隆块：resume 路径上的传播挂起必须 dispatch 回克隆续点。
fn rewrite_chain_states_in_block(block: &mut crate::mir::BasicBlock, map: &HashMap<u128, u128>) {
    if map.is_empty() {
        return;
    }
    for stmt in &mut block.stmts {
        if let StatementKind::Assign {
            value: Rvalue::MakeChainLink { state },
            ..
        } = &mut stmt.kind
        {
            if let Some(&cap) = map.get(state) {
                *state = cap;
            }
        }
    }
}

/// 收集终结符的所有跳转目标块 id。
fn terminator_targets(kind: &TerminatorKind) -> Vec<BasicBlockId> {
    match kind {
        TerminatorKind::Goto { target } => vec![*target],
        TerminatorKind::CondBr {
            then_target,
            else_target,
            ..
        } => vec![*then_target, *else_target],
        TerminatorKind::Handle {
            body_target,
            arm_targets,
            finally_target,
            exit_target,
            ..
        } => {
            let mut v = vec![*body_target, *exit_target];
            v.extend(arm_targets.iter().copied());
            if let Some(f) = finally_target {
                v.push(*f);
            }
            v
        }
        TerminatorKind::Perform { resume_target, .. } => vec![*resume_target],
        _ => Vec::new(),
    }
}

/// 对终结符的所有跳转目标应用映射函数。
fn remap_terminator_targets(
    kind: &mut TerminatorKind,
    map: &mut impl FnMut(BasicBlockId) -> BasicBlockId,
) {
    match kind {
        TerminatorKind::Goto { target } => *target = map(*target),
        TerminatorKind::CondBr {
            then_target,
            else_target,
            ..
        } => {
            *then_target = map(*then_target);
            *else_target = map(*else_target);
        }
        TerminatorKind::Handle {
            body_target,
            arm_targets,
            finally_target,
            exit_target,
            ..
        } => {
            *body_target = map(*body_target);
            for t in arm_targets.iter_mut() {
                *t = map(*t);
            }
            if let Some(f) = finally_target {
                *f = map(*f);
            }
            *exit_target = map(*exit_target);
        }
        TerminatorKind::Perform { resume_target, .. } => *resume_target = map(*resume_target),
        _ => {}
    }
}

/// 添加 state dispatch 入口块。
///
/// 在 body 的所有块前插入一个 dispatch 块链，检查 frame.state 的值：
/// - state == 0 → goto original_start（初始入口）
/// - state == N → goto resume_target_N（恢复重入）
///
/// 使用 Rvalue::IntEq 生成 Bool 比较结果，通过 CondBr 路由。
///
/// 同时在每个 resume_target 块开头插入从 frame 恢复 live locals 的语句。
/// call-chain 站点的 state 也入 dispatch 链（N_cap → 克隆 R_N，N_prop →
/// 原始 R_N）；其 R_N 的 restore 语句已在 adapt 时生成，此处不重复插入。
/// 返回后 plan 的坐标（block_idx / 块 id）已修正为 shift 后的值。
fn add_state_dispatch(
    body: &mut Body,
    sites: &[PerformSite],
    call_plans: &mut [CallChainSitePlan],
    frame_local: LocalId,
    live_to_slot: &HashMap<LocalId, u128>,
    _escape_routing: &HashMap<usize, PerformRoute>,
    store: &mut TypeStore,
) -> (LocalId, Vec<crate::mir::ResumePoint>) {
    let original_start = body.start;
    // 分配 state local（用于读取 frame.state 字段）。
    let state_local = LocalId(body.locals.len() as u32);
    body.locals.push(LocalDecl {
        span: scoop2_base::Span::default(),
        name: Some("$state".to_string()),
        ty: store.int(),
        source: crate::mir::LocalSource::Temp,
        mutable: false,
    });
    // 分配 Bool local（用于 IntEq 比较结果）。
    let bool_local = LocalId(body.locals.len() as u32);
    body.locals.push(LocalDecl {
        span: scoop2_base::Span::default(),
        name: Some("$dispatch_cond".to_string()),
        ty: store.bool(),
        source: crate::mir::LocalSource::Temp,
        mutable: false,
    });

    // 为每个 resume_target 块插入从 frame 恢复 live locals 的语句。
    // 同时收集每个 site 的 state 编号和 resume_target。
    let mut resume_entries: Vec<(u128, BasicBlockId)> = Vec::new();
    for (i, site) in sites.iter().enumerate() {
        let state_num = (i + 1) as u128;
        let resume_idx = site.resume_target.0 as usize;
        resume_entries.push((state_num, site.resume_target));
        if resume_idx >= body.blocks.len() {
            continue;
        }
        let span = body.blocks[resume_idx]
            .stmts
            .first()
            .map(|s| s.span)
            .unwrap_or_default();
        let mut restore_stmts: Vec<Statement> = Vec::new();
        // 先恢复 state_local（从 frame.state）。
        restore_stmts.push(Statement {
            span,
            kind: StatementKind::Assign {
                target: state_local,
                value: Rvalue::TupleIndex {
                    receiver: Operand::Local(frame_local),
                    index: 0,
                    element_ty: store.int(),
                },
            },
        });
        // 恢复 live locals（slot 下标取 live_to_slot——slot 0 之后是参数槽）。
        // resume_local 跳过：它的值由 codegen 的 resume 值投递（块首、restore
        // 语句之前）写入，restore 会用 frame 里的旧值覆盖投递值。
        for &live_local in &site.live_out_at_resume {
            if live_local == site.resume_local {
                continue;
            }
            let Some(&slot_idx) = live_to_slot.get(&live_local) else {
                continue;
            };
            let live_ty = body
                .locals
                .get(live_local.0 as usize)
                .map(|d| d.ty)
                .unwrap_or_else(|| store.any());
            let temp_local = LocalId(body.locals.len() as u32);
            body.locals.push(LocalDecl {
                span,
                name: None,
                ty: live_ty,
                source: crate::mir::LocalSource::Temp,
                mutable: false,
            });
            restore_stmts.push(Statement {
                span,
                kind: StatementKind::Assign {
                    target: temp_local,
                    value: Rvalue::TupleIndex {
                        receiver: Operand::Local(frame_local),
                        index: slot_idx,
                        element_ty: live_ty,
                    },
                },
            });
            restore_stmts.push(Statement {
                span,
                kind: StatementKind::Assign {
                    target: live_local,
                    value: Rvalue::Use(Operand::Local(temp_local)),
                },
            });
        }
        body.blocks[resume_idx].stmts.splice(0..0, restore_stmts);
    }
    // call-chain 站点的 dispatch entry：N_cap → 克隆 R_N，N_prop → 原始 R_N。
    // （R_N 的 restore 语句已在 adapt 时生成，此处不再插入。）
    for plan in call_plans.iter() {
        if let (Some(cap), Some(cloned)) = (plan.state_cap, plan.cloned_resume_block) {
            resume_entries.push((cap, cloned));
        }
        if let Some(prop) = plan.state_prop {
            resume_entries.push((prop, plan.resume_block));
        }
    }

    // 构造 CondBr dispatch 链。
    // 由于插入新块会改变所有块的索引，我们需要先收集 dispatch 块信息，
    // 然后在 body.blocks 前面插入所有 dispatch 块。
    // dispatch 块结构（从后往前构建）：
    //   dispatch_N: bool = IntEq(state, N); CondBr(bool, resume_N, unreachable)
    //   ...
    //   dispatch_1: bool = IntEq(state, 1); CondBr(bool, resume_1, dispatch_2)
    //   dispatch_0: bool = IntEq(state, 0); CondBr(bool, original_start, dispatch_1)
    //
    // 但由于原始块索引会因插入而偏移，我们使用以下策略：
    // 1. 在 body.blocks 开头插入 N+1 个 dispatch 块
    // 2. 所有原始块的索引偏移 N+1
    // 3. 更新所有块中的 BasicBlockId 引用（Goto/CondBr/Handle 目标）

    let num_dispatch = resume_entries.len() + 1; // +1 for state==0 check
    let offset = num_dispatch as u32;

    // 构造 dispatch 块（从最后一个往前）。
    // 最后一个 dispatch 块的 else 分支 → Unreachable（不应该有未知 state）。
    let mut dispatch_blocks: Vec<crate::mir::BasicBlock> = Vec::with_capacity(num_dispatch);

    // 先偏移所有现有块中的 block id 引用。
    shift_block_ids(body, offset);
    // call-chain 计划里记录的坐标/块 id 同步偏移（phase B 按 shift 后坐标匹配）。
    for plan in call_plans.iter_mut() {
        plan.block_idx += offset as usize;
        plan.resume_block.0 += offset;
        plan.dispatch_block.0 += offset;
        if let Some(c) = &mut plan.cloned_resume_block {
            c.0 += offset;
        }
        for c in &mut plan.cloned_call_blocks {
            c.0 += offset;
        }
    }

    // 现在原始块的索引已偏移。original_start 和 resume_targets 也需要偏移。
    let shifted_original_start = BasicBlockId(original_start.0 + offset);
    let shifted_resume_entries: Vec<(u128, BasicBlockId)> = resume_entries
        .iter()
        .map(|(state, target)| (*state, BasicBlockId(target.0 + offset)))
        .collect();

    // 从后往前构建 CondBr 链。
    // unreachable 块（最后一个 dispatch 的 else 目标）= 第一个原始块之前，
    // 我们在 dispatch 块之后插入一个 unreachable 块。
    let unreachable_bid = BasicBlockId(num_dispatch as u32 + body.blocks.len() as u32);

    // 构建从后往前的 dispatch 块。
    let mut next_else = unreachable_bid;
    // dispatch 块按逆序构建，然后反转。
    let mut chain: Vec<crate::mir::BasicBlock> = Vec::new();

    // 为每个 resume state 创建一个 dispatch 块（从后往前）。
    for &(state_num, resume_bid) in shifted_resume_entries.iter().rev() {
        let dispatch_bid = BasicBlockId(chain.len() as u32); // 临时 ID，后面会调整
        let _ = dispatch_bid;
        chain.push(crate::mir::BasicBlock {
            stmts: vec![
                // 读取 state 从 frame。
                Statement {
                    span: scoop2_base::Span::default(),
                    kind: StatementKind::Assign {
                        target: state_local,
                        value: Rvalue::TupleIndex {
                            receiver: Operand::Local(frame_local),
                            index: 0,
                            element_ty: store.int(),
                        },
                    },
                },
                // bool = IntEq(state, state_num)
                Statement {
                    span: scoop2_base::Span::default(),
                    kind: StatementKind::Assign {
                        target: bool_local,
                        value: Rvalue::IntEq {
                            lhs: Operand::Local(state_local),
                            rhs: Operand::Const(crate::mir::ConstValue::Int(state_num, None)),
                        },
                    },
                },
            ],
            terminator: Terminator {
                span: scoop2_base::Span::default(),
                kind: TerminatorKind::CondBr {
                    cond: Operand::Local(bool_local),
                    then_target: resume_bid,
                    else_target: next_else,
                },
            },
        });
        // 下一个 dispatch 块的 else 目标 = 当前块。
        // chain 中的块索引从 0 开始，但我们需要正向索引。
        next_else = BasicBlockId((chain.len() - 1) as u32);
    }

    // 入口 dispatch 块（state == 0 → original_start, else → 第一个 resume dispatch）。
    // chain 现在是逆序的（最后一个 resume dispatch 在 chain[0]）。
    // 入口 dispatch 的 else 目标 = chain 中第一个 resume dispatch 的正向索引。
    // chain 反转后：chain[0] = 第一个 resume dispatch, chain[1] = 第二个, ...
    // 入口 dispatch 在最前面（block 0），入口 dispatch 的 else = block 1（第一个 resume dispatch）。
    chain.reverse();

    // 此时 chain 的正向索引：
    // chain[0] = dispatch for state 1, chain[1] = dispatch for state 2, ...
    // 但 next_else 是逆序构建的，需要修正。
    // 实际上 chain 反转后，每个块的 else_target 指向 chain 中的前一个块（按反转后顺序），
    // 但我们构建时 else_target 指向的是更后面的 dispatch——这需要修正。

    // 重新正确地构建 dispatch 块（正向）。
    chain.clear();
    let total_dispatch = resume_entries.len() + 1;

    // dispatch_0: state == 0 → original_start, else → dispatch_1
    // dispatch_1: state == 1 → resume_1, else → dispatch_2
    // ...
    // dispatch_N: state == N → resume_N, else → unreachable

    for i in 0..total_dispatch {
        // dispatch_0（入口）：state == 0 → original_start。frame 由 codegen 的
        // step 函数参数提供（堆分配、已初始化），读 frame.state 是良定义的。
        let (check_state, then_target) = if i == 0 {
            (0u128, shifted_original_start)
        } else {
            let entry = &shifted_resume_entries[i - 1];
            (entry.0, entry.1)
        };
        let else_target = if i + 1 < total_dispatch {
            BasicBlockId((i + 1) as u32)
        } else {
            unreachable_bid
        };
        chain.push(crate::mir::BasicBlock {
            stmts: vec![
                Statement {
                    span: scoop2_base::Span::default(),
                    kind: StatementKind::Assign {
                        target: state_local,
                        value: Rvalue::TupleIndex {
                            receiver: Operand::Local(frame_local),
                            index: 0,
                            element_ty: store.int(),
                        },
                    },
                },
                Statement {
                    span: scoop2_base::Span::default(),
                    kind: StatementKind::Assign {
                        target: bool_local,
                        value: Rvalue::IntEq {
                            lhs: Operand::Local(state_local),
                            rhs: Operand::Const(crate::mir::ConstValue::Int(check_state, None)),
                        },
                    },
                },
            ],
            terminator: Terminator {
                span: scoop2_base::Span::default(),
                kind: TerminatorKind::CondBr {
                    cond: Operand::Local(bool_local),
                    then_target,
                    else_target,
                },
            },
        });
    }

    // 插入 dispatch 块到 body.blocks 开头。
    // dispatch 块的索引：0..total_dispatch。
    // 原始块（已偏移）：total_dispatch..total_dispatch+len。
    // unreachable 块：total_dispatch + len。
    let mut new_blocks = chain;
    new_blocks.append(&mut body.blocks);
    // 添加 unreachable 块。
    new_blocks.push(crate::mir::BasicBlock {
        stmts: vec![],
        terminator: Terminator {
            span: scoop2_base::Span::default(),
            kind: TerminatorKind::Unreachable,
        },
    });
    body.blocks = new_blocks;
    // 更新 start 指向 dispatch_0。
    body.start = BasicBlockId(0);

    // 收集 resume points（全部 perform 站点：escape 站点的 continuation 可被
    // 直接 resume；outward 站点经 case b 调用链被 ResumeChainLink 恢复——两者
    // 都需要 codegen 在续点块首把 resume word 投递到 resume_local）。
    // block 用偏移后的 resume_target；resume_local 的声明类型恒为 op 结果类型
    // （outward 站点的 Step 载体已拆分为独立 carrier local，不改写 resume_local）。
    // 注意：zip 只覆盖前 P 个 entry（perform 站点）；call-chain entry 自带
    // word 投递机制（ResumeChainLink 直读 step 函数 word 参数），不进此表。
    let resume_points: Vec<crate::mir::ResumePoint> = shifted_resume_entries
        .iter()
        .zip(sites.iter())
        .map(|((state, target), site)| {
            let resume_ty = body
                .locals
                .get(site.resume_local.0 as usize)
                .map(|d| d.ty)
                .unwrap_or_else(|| store.any());
            crate::mir::ResumePoint {
                state: *state,
                block: *target,
                resume_local: site.resume_local,
                resume_ty,
            }
        })
        .collect();
    (state_local, resume_points)
}

/// 偏移 body 中所有块的 BasicBlockId 引用（Goto/CondBr/Handle/Perform 目标）。
fn shift_block_ids(body: &mut Body, offset: u32) {
    for block in &mut body.blocks {
        match &mut block.terminator.kind {
            TerminatorKind::Goto { target }
            | TerminatorKind::Perform {
                resume_target: target,
                ..
            } => {
                target.0 += offset;
            }
            TerminatorKind::CondBr {
                then_target,
                else_target,
                ..
            } => {
                then_target.0 += offset;
                else_target.0 += offset;
            }
            TerminatorKind::Handle {
                body_target,
                arm_targets,
                finally_target,
                exit_target,
                ..
            } => {
                body_target.0 += offset;
                for t in arm_targets.iter_mut() {
                    t.0 += offset;
                }
                if let Some(f) = finally_target {
                    f.0 += offset;
                }
                exit_target.0 += offset;
            }
            _ => {}
        }
    }
    body.start.0 += offset;
}

// =========================================================================
// 辅助函数
// =========================================================================

/// 分配一个临时 local（Temp 来源、不可变）。
fn alloc_temp_local(body: &mut Body, span: scoop2_base::Span, ty: TypeId) -> LocalId {
    let id = LocalId(body.locals.len() as u32);
    body.locals.push(LocalDecl {
        span,
        name: None,
        ty,
        source: crate::mir::LocalSource::Temp,
        mutable: false,
    });
    id
}

fn operand_type(op: &Operand, body: &Body) -> TypeId {
    match op {
        Operand::Local(lid) => body
            .locals
            .get(lid.0 as usize)
            .map(|d| d.ty)
            .unwrap_or_else(|| scoop2_hir::ty::TypeStore::new().any()),
        Operand::Const(c) => const_type(c),
    }
}

fn const_type(c: &crate::mir::ConstValue) -> TypeId {
    let mut store = TypeStore::new();
    match c {
        crate::mir::ConstValue::Bool(_) => store.bool(),
        crate::mir::ConstValue::Char(_) => store.char(),
        crate::mir::ConstValue::Unit => store.unit(),
        crate::mir::ConstValue::Int(_, _) => store.int(),
        crate::mir::ConstValue::Float(_, _) => store.float64(),
        crate::mir::ConstValue::String(_) => store.string(),
        crate::mir::ConstValue::Null => store.any(),
    }
}

fn split_op_fqn(op_fqn: &str, owner_fqn: &str) -> (String, String) {
    let variant = op_fqn.replace('.', "_");
    (format!("{}$step", owner_fqn), variant)
}

/// 类型在 LIR 布局中是否零尺寸（Unit / Nothing / 全零尺寸元素的 tuple 递归）。
/// frame 槽位判定用：零尺寸槽在 tuple 布局中与下一字段共享偏移，不能作为
/// 独立可写槽位。
fn is_zero_sized_slot_ty(store: &TypeStore, ty: TypeId) -> bool {
    match store.kind(ty) {
        scoop2_hir::ty::TypeKind::Value(scoop2_hir::ty::ValueTypeKind::Unit)
        | scoop2_hir::ty::TypeKind::Nothing => true,
        scoop2_hir::ty::TypeKind::Value(scoop2_hir::ty::ValueTypeKind::Tuple(elems)) => {
            elems.iter().all(|&e| is_zero_sized_slot_ty(store, e))
        }
        _ => false,
    }
}
