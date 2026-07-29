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
}

/// 对整个 Module 执行 effect lowering。
pub fn lower_effects(module: &mut Module, interner: &Interner) {
    // 直接使用传入的 interner（不可变）。合成 Step 类型的 FQN 使用已 intern 的字符串
    // （函数自身 FQN + effect op FQN），不创建新字符串。
    // 这确保 Symbol 在后续 verify/dump 中可被原 interner 解析。
    // 阶段 0：效应行为分类（定点迭代，确定哪些函数是 EffectStep 及其 outward ops）。
    let classes = classify_module(module);
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
                let effect_abi =
                    lower_body(body, &mut module.types, interner, &fqn, &outward, &params);
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
fn classify_module(module: &mut Module) -> HashMap<String, FnEffectClass> {
    // 收集每个函数的 fqn + body 引用信息。
    struct FnFacts {
        fqn: String,
        /// (block_idx, op_fqn, payload_ty) — 词法 Perform 站点。
        performs: Vec<(usize, String, TypeId)>,
        has_resume: bool,
        /// (block_idx, callee_fqn) — Direct 调用点。
        calls: Vec<(usize, String)>,
        /// handle 区域：(区域 block 集合, 区域覆盖的 op 集合)。
        handle_regions: Vec<(HashSet<usize>, HashSet<String>)>,
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
            handle_regions.push((region, ops));
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
        for (i, block) in body.blocks.iter().enumerate() {
            for stmt in &block.stmts {
                if let StatementKind::Assign { value, .. } = &stmt.kind {
                    if let Rvalue::Call {
                        kind: CallKind::Direct { callee_fqn, .. },
                        ..
                    } = value
                    {
                        calls.push((i, callee_fqn.clone()));
                    }
                }
            }
        }
        facts.push(FnFacts {
            fqn,
            performs,
            has_resume: body_has_resume(body),
            calls,
            handle_regions,
        });
    }
    // 初始化分类表。
    let mut classes: HashMap<String, FnEffectClass> = HashMap::new();
    for f in &facts {
        classes.insert(f.fqn.clone(), FnEffectClass::default());
    }
    // 基例：未被本地捕获的 Perform 的 op 逸出。
    for f in &facts {
        let mut outward: Vec<(String, TypeId)> = Vec::new();
        for (bi, op, payload_ty) in &f.performs {
            let covered = f
                .handle_regions
                .iter()
                .any(|(region, ops)| region.contains(bi) && ops.contains(op));
            if !covered && !outward.iter().any(|(o, _)| o == op) {
                outward.push((op.clone(), *payload_ty));
            }
        }
        let cls = classes.get_mut(&f.fqn).unwrap();
        cls.outward_ops = outward;
        cls.is_effect_step = !cls.outward_ops.is_empty() || f.has_resume;
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
                        .any(|(region, ops)| region.contains(bi) && ops.contains(&op));
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
        // 调用点的最内层 handle（区域最小者）。
        let enclosing: Option<&HandleInfo> = handle_regions
            .iter()
            .filter(|(region, _)| region.contains(&bi))
            .min_by_key(|(region, _)| region.len())
            .map(|(_, h)| *h);

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
            // 1. handle arm 覆盖？（arm op 经规范化后与变体 op 比较）。
            let arm_route = enclosing.and_then(|h| {
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
        let panic_block_id =
            BasicBlockId(body.blocks.len() as u32 + 1 + (num_routes * 2) as u32);

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
    body_target: BasicBlockId,
    arm_dispatch: HashMap<String, ArmRoute>,
    exit_target: BasicBlockId,
    /// handle 结果 local（escape 边界克隆的 Complete payload 来源）。
    result_local: LocalId,
}

/// 对单个函数体执行 effect lowering。
///
/// `outward_ops`：阶段 0 分类给出的逸出 effect 操作（含传播而来、本地无 Perform 的）。
/// Handle 终结符在此不消除——保留到阶段 B（adapt_calls），因为调用适配需要
/// handle 区域信息把被传播过来的 Step 路由到对应 arm。
fn lower_body(
    body: &mut Body,
    store: &mut TypeStore,
    interner: &Interner,
    fqn: &str,
    outward_ops: &[(String, TypeId)],
    params: &[(LocalId, TypeId)],
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
        rewrite_captured_performs(body, &plain_routing);
    }
    // 阶段 2：是否变换为 EffectStep。有 escape 捕获时必须变换（continuation
    // 需要本函数的 frame + step 函数指针）。
    let is_effect_step =
        !outward_ops.is_empty() || body_has_resume(body) || !escape_routing.is_empty();
    if is_effect_step {
        lower_to_effect_step(body, store, interner, fqn, outward_ops, params, escape_routing)
    } else {
        None
    }
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
    for block in &body.blocks {
        if let TerminatorKind::Handle {
            arms,
            body_target,
            arm_targets,
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
                body_target: *body_target,
                arm_dispatch,
                exit_target: *exit_target,
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

fn rewrite_captured_performs(body: &mut Body, routing: &HashMap<usize, PerformRoute>) {
    for (&block_idx, route) in routing {
        let perform_args: Vec<Operand> = match &body.blocks[block_idx].terminator.kind {
            TerminatorKind::Perform { args, .. } => args.iter().map(|a| a.value.clone()).collect(),
            _ => continue,
        };
        let span = body.blocks[block_idx].terminator.span;
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
            kind: TerminatorKind::Goto {
                target: route.arm_target,
            },
        };
    }
}

fn rewrite_handles(body: &mut Body) {
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
fn lower_to_effect_step(
    body: &mut Body,
    store: &mut TypeStore,
    interner: &Interner,
    fqn: &str,
    outward_ops: &[(String, TypeId)],
    params: &[(LocalId, TypeId)],
    escape_routing: HashMap<usize, PerformRoute>,
) -> Option<crate::mir::EffectStepAbi> {
    let live_in = analyze::compute_live_in(body);
    let mut perform_sites = collect_perform_sites(body, &live_in);

    // 收集所有需要保存的 live locals（跨所有 Perform 站点的并集）。
    let mut all_live_locals: Vec<LocalId> = Vec::new();
    let mut seen: HashSet<LocalId> = HashSet::new();
    for site in &perform_sites {
        for &lid in &site.live_out_at_resume {
            if seen.insert(lid) {
                all_live_locals.push(lid);
            }
        }
    }

    // 构造 Frame tuple 类型：[state: Int, param_1: P1, ..., live_local_1: T1, ...]。
    // 参数槽必须包含全部参数：resume 时 step 函数只拿到 (frame, word)，
    // 参数值只能经 frame 传入（wrapper 在初始调用时写入，step 入口恢复）。
    let mut frame_field_tys: Vec<TypeId> = vec![store.int()];
    for (_, ty) in params {
        frame_field_tys.push(*ty);
    }
    for &lid in &all_live_locals {
        let ty = body
            .locals
            .get(lid.0 as usize)
            .map(|d| d.ty)
            .unwrap_or_else(|| store.any());
        frame_field_tys.push(ty);
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
    rewrite_perform_sites(
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

    // escape 捕获的边界后缀克隆：把 resume_target 起、到 handle 出口为止的
    // 后缀克隆一份，指向出口的边改指到新块 `Return(result_local)`（随后由
    // wrap_returns_as_complete 包装为 Step::Complete）。resume 路径只走克隆，
    // 不会顺着出口继续执行调用方的剩余代码（初始路径走 arm → 出口）。
    clone_escape_suffixes(body, &mut perform_sites, &escape_routing);

    // Resume 调用保持 `CallKind::Resume` 原样流向 LIR/codegen：
    // resumed 标志检查 + step_fn 间接调用由 codegen 基于 canonical continuation
    // 布局（scoop2_lir::effect）统一 lowering，MIR 不做字段级重写。

    // 添加 state dispatch 入口。
    let (state_local, resume_points) = if !perform_sites.is_empty() {
        add_state_dispatch(
            body,
            &perform_sites,
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
    // 不进入跳过集合。
    let step_val_locals: HashSet<LocalId> = perform_sites
        .iter()
        .filter(|s| !escape_routing.contains_key(&s.block_idx))
        .map(|s| s.resume_local)
        .collect();
    wrap_returns_as_complete(
        body,
        store,
        step_ty,
        step_fqn_sym,
        complete_sym,
        &step_val_locals,
    );

    Some(crate::mir::EffectStepAbi {
        frame_ty,
        step_ty,
        step_variants,
        frame_local,
        state_local,
        resume_points,
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

/// 重写 Perform 站点：保存 live locals 到 frame + 返回 Step case。
///
/// `outward_ops`：规范化 op 列表（下标 + 1 = 变体 tag）。站点构造的 Step 变体
/// 符号取 op 的规范 FQN Symbol（与 step_variants 的 name_sym 一致）。
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
) {
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
            let esc = route.escape.as_ref().expect("escape 路由必有 EscapeCapture");
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

        // 3. 如果 payload 是 tuple，需要先构造 tuple。
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

        // 4. resume_local = Step EnumVariant。
        // resume_local 在非 resuming 路径被改作 Step 值载体（Return 直接返回它），
        // 其声明类型（op 返回类型）同步改为 step_ty，否则 codegen 会把 enum 值
        // 存进按原类型分配的 alloca（例如 ret i64 对 Step struct 返回类型）。
        if let Some(decl) = body.locals.get_mut(site.resume_local.0 as usize) {
            decl.ty = step_ty;
        }
        block.stmts.push(Statement {
            span,
            kind: StatementKind::Assign {
                target: site.resume_local,
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

        // 5. Perform → Return(Step case)。
        block.terminator = Terminator {
            span,
            kind: TerminatorKind::Return {
                value: Some(Operand::Local(site.resume_local)),
            },
        };
    }
    let _ = frame_ty;
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
fn clone_escape_suffixes(
    body: &mut Body,
    sites: &mut [PerformSite],
    escape_routing: &HashMap<usize, PerformRoute>,
) {
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
        site.resume_target =
            BasicBlockId(id_map[&(site.resume_target.0 as usize)] as u32);
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
fn remap_terminator_targets(kind: &mut TerminatorKind, map: &mut impl FnMut(BasicBlockId) -> BasicBlockId) {
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
fn add_state_dispatch(
    body: &mut Body,
    sites: &[PerformSite],
    frame_local: LocalId,
    live_to_slot: &HashMap<LocalId, u128>,
    escape_routing: &HashMap<usize, PerformRoute>,
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

    // 收集 resume points（仅 escape 捕获站点——只有它们产生可被 resume 的
    // continuation）。block 用偏移后的 resume_target；resume_local 的声明类型
    // 在 escape 路径未被改写（非 escape 站点会被改成 step_ty，不记录）。
    let resume_points: Vec<crate::mir::ResumePoint> = shifted_resume_entries
        .iter()
        .zip(sites.iter())
        .filter_map(|((state, target), site)| {
            if !escape_routing.contains_key(&site.block_idx) {
                return None;
            }
            let resume_ty = body
                .locals
                .get(site.resume_local.0 as usize)
                .map(|d| d.ty)
                .unwrap_or_else(|| store.any());
            Some(crate::mir::ResumePoint {
                state: *state,
                block: *target,
                resume_local: site.resume_local,
                resume_ty,
            })
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
