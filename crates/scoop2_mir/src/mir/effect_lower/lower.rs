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

/// 对整个 Module 执行 effect lowering。
pub fn lower_effects(module: &mut Module, interner: &Interner) {
    // 直接使用传入的 interner（不可变）。合成 Step 类型的 FQN 使用已 intern 的字符串
    // （函数自身 FQN + effect op FQN），不创建新字符串。
    // 这确保 Symbol 在后续 verify/dump 中可被原 interner 解析。
    // 阶段 A：Handle dispatch 消除 + EffectStep body 变换。
    let mut changed = true;
    while changed {
        changed = false;
        let need_process: Vec<usize> = module
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| {
                let has_effects = match item {
                    Item::Fun(fd) => fd
                        .body
                        .as_ref()
                        .map_or(false, analyze::has_effect_structures),
                    Item::Initializer(ir) => analyze::has_effect_structures(&ir.body),
                    _ => false,
                };
                if has_effects { Some(i) } else { None }
            })
            .collect();
        if need_process.is_empty() {
            break;
        }
        for idx in need_process {
            let fqn = match &module.items[idx] {
                Item::Fun(fd) => fd.fqn.clone(),
                Item::Initializer(ir) => ir.fqn.clone(),
                _ => continue,
            };
            let body = match &mut module.items[idx] {
                Item::Fun(fd) => fd.body.as_mut(),
                Item::Initializer(ir) => Some(&mut ir.body),
                _ => None,
            };
            if let Some(body) = body {
                if analyze::has_effect_structures(body) {
                    let effect_abi = lower_body(body, &mut module.types, interner, &fqn);
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
                    changed = true;
                }
            }
        }
    }
    // 阶段 B：调用适配——处理对 EffectStep 函数的调用。
    adapt_calls(module, interner);
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

/// 调用适配：当 Plain 函数调用 EffectStep 函数时，处理返回的 Step 值。
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
    if effect_step_info.is_empty() {
        return;
    }
    let effect_step_fqns: std::collections::HashSet<String> =
        effect_step_info.keys().cloned().collect();
    // 对每个函数体中的调用点做适配。
    for item in &mut module.items {
        if let Item::Fun(fd) = item {
            if let Some(body) = &mut fd.body {
                adapt_calls_in_body(
                    body,
                    &effect_step_fqns,
                    &effect_step_info,
                    &mut module.types,
                    interner,
                    &fd.fqn,
                );
            }
        }
        if let Item::Initializer(ir) = item {
            adapt_calls_in_body(
                &mut ir.body,
                &effect_step_fqns,
                &effect_step_info,
                &mut module.types,
                interner,
                &ir.fqn,
            );
        }
    }
}

/// 在单个 body 中适配对 EffectStep 函数的调用。
///
/// 当函数 A 调用 EffectStep 函数 B 时，B 返回 Step 值而非原始返回类型。
/// 调用适配在调用语句后插入检查逻辑：
/// 1. 分配 Step local 接收 B 的返回值
/// 2. 用 PatternMatch 检查 Step 是否为 "Complete" 变体
/// 3. Complete → PatternExtract 提取结果，赋值到原始 target local
/// 4. 非 Complete → 把 Step 值赋值到 target（由外层 handle 或传播处理）
fn adapt_calls_in_body(
    body: &mut Body,
    effect_step_fqns: &std::collections::HashSet<String>,
    effect_step_info: &std::collections::HashMap<String, crate::mir::EffectStepAbi>,
    store: &mut TypeStore,
    interner: &Interner,
    _caller_fqn: &str,
) {
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
        // 获取 Complete 变体的符号信息。
        let complete_variant = callee_abi
            .step_variants
            .iter()
            .find(|v| v.is_complete)
            .cloned();
        let step_fqn_sym = {
            let step_fqn_str = format!("{}$step", callee_fqn);
            interner
                .get(&step_fqn_str)
                .or_else(|| interner.get(&callee_fqn))
        }
        .unwrap_or_else(|| {
            // 使用 callee FQN 的 interner 符号作为合成 Step enum 的 FQN。
            interner.get(&callee_fqn).unwrap_or_default()
        });
        let complete_sym = complete_variant
            .as_ref()
            .map(|v| v.name_sym)
            .unwrap_or_default();

        // 分配 Step local（接收 EffectStep 函数的返回值）。
        let step_local = LocalId(body.locals.len() as u32);
        body.locals.push(LocalDecl {
            span,
            name: None,
            ty: step_ty,
            source: crate::mir::LocalSource::Temp,
            mutable: false,
        });
        // 分配 Bool local（PatternMatch 结果）。
        let bool_local = LocalId(body.locals.len() as u32);
        body.locals.push(LocalDecl {
            span,
            name: None,
            ty: store.bool(),
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

        // 创建 Complete 分支块和 Propagate 分支块。
        let complete_block_id = BasicBlockId(body.blocks.len() as u32);
        let propagate_block_id = BasicBlockId(body.blocks.len() as u32 + 1);

        // 重写当前块：before_stmts + step_local = call(...) + bool_local = PatternMatch + CondBr
        let mut new_stmts = before_stmts;
        // step_local = call(...)
        new_stmts.push(Statement {
            span,
            kind: StatementKind::Assign {
                target: step_local,
                value: orig_value,
            },
        });
        // bool_local = PatternMatch(step_local, Variant(Complete))
        new_stmts.push(Statement {
            span,
            kind: StatementKind::Assign {
                target: bool_local,
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
        body.blocks[bi].terminator = Terminator {
            span: orig_span_term,
            kind: TerminatorKind::CondBr {
                cond: Operand::Local(bool_local),
                then_target: complete_block_id,
                else_target: propagate_block_id,
            },
        };

        // Complete 分支块：提取结果 + 后续语句 + 原终结符。
        let mut complete_stmts = vec![
            // extract_local = PatternExtract(step_local)
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
            // target_local = extract_local
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

        // Propagate 分支块：Return(step_local)——向上传播 Step。
        body.blocks.push(crate::mir::BasicBlock {
            stmts: vec![],
            terminator: Terminator {
                span,
                kind: TerminatorKind::Return {
                    value: Some(Operand::Local(step_local)),
                },
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
}

struct HandleInfo {
    body_target: BasicBlockId,
    arm_dispatch: HashMap<String, ArmRoute>,
    exit_target: BasicBlockId,
}

/// 对单个函数体执行 effect lowering。
fn lower_body(
    body: &mut Body,
    store: &mut TypeStore,
    interner: &Interner,
    fqn: &str,
) -> Option<crate::mir::EffectStepAbi> {
    // 阶段 1：Handle dispatch 消除。
    let handles = collect_handle_info(body);
    if !handles.is_empty() {
        let routing = build_perform_routing(body, &handles);
        rewrite_captured_performs(body, &routing);
        rewrite_handles(body);
    }
    // 阶段 2：检查是否还有未捕获的 Perform 或 Resume。
    let has_uncaptured = body
        .blocks
        .iter()
        .any(|b| matches!(&b.terminator.kind, TerminatorKind::Perform { .. }));
    let has_resume = body_has_resume(body);
    if has_uncaptured || has_resume {
        lower_to_effect_step(body, store, interner, fqn)
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
                    },
                );
            }
            handles.push(HandleInfo {
                body_target: *body_target,
                arm_dispatch,
                exit_target: *exit_target,
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

/// 对含未捕获 Perform / Resume 的 body 执行 EffectStep 变换。
/// 返回 EffectStepAbi 信息（含 frame 类型、outward cases、frame/state local IDs）。
fn lower_to_effect_step(
    body: &mut Body,
    store: &mut TypeStore,
    interner: &Interner,
    fqn: &str,
) -> Option<crate::mir::EffectStepAbi> {
    let live_out = analyze::compute_live_out(body);
    let perform_sites = collect_perform_sites(body, &live_out);

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

    // 构造 Frame tuple 类型：[state: Int, live_local_1: T1, live_local_2: T2, ...]
    let mut frame_field_tys: Vec<TypeId> = vec![store.int()];
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

    // 构建 live_local → frame slot index 映射。
    let live_to_slot: HashMap<LocalId, u128> = all_live_locals
        .iter()
        .enumerate()
        .map(|(i, &lid)| (lid, (i + 1) as u128))
        .collect();

    // 初始化 frame。
    initialize_frame(body, frame_local, frame_ty, &all_live_locals, store);

    // 重写 Perform 站点。
    rewrite_perform_sites(
        body,
        &perform_sites,
        &live_to_slot,
        frame_local,
        frame_ty,
        store,
        interner,
        fqn,
    );

    // Resume 调用保持 `CallKind::Resume` 原样流向 LIR/codegen：
    // resumed 标志检查 + step_fn 间接调用由 codegen 基于 canonical continuation
    // 布局（scoop2_lir::effect）统一 lowering，MIR 不做字段级重写。

    // 添加 state dispatch 入口。
    let state_local = if !perform_sites.is_empty() {
        add_state_dispatch(body, &perform_sites, frame_local, store)
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
        sl
    };

    // 收集 outward cases（所有未捕获 Perform 的 op_fqn）。
    let outward_cases: Vec<String> = perform_sites.iter().map(|s| s.op_fqn.clone()).collect();

    // 构造 Step 合成 enum 类型。
    // Step enum 的 FQN = "<fqn>$step"。
    // 变体：Complete（原始返回类型）+ 每个 outward case 的 effect 操作变体。
    let step_fqn_str = format!("{}$step", fqn);
    let step_fqn_sym = interner
        .get(&step_fqn_str)
        .or_else(|| interner.get(fqn))
        .unwrap_or_default();
    let step_ty = store.value_nominal(scoop2_hir::ty::NominalType {
        fqn: step_fqn_sym,
        args: vec![],
        eff: None,
    });

    // 构造 Step 变体信息。
    let return_ty = body
        .locals
        .first()
        .map(|_| store.any()) // 简化：Complete 变体的 payload 用 Any
        .unwrap_or_else(|| store.any());
    let mut step_variants: Vec<crate::mir::StepVariant> = Vec::new();
    // Complete 变体。
    let complete_sym = interner.get("Complete").unwrap_or_default();
    step_variants.push(crate::mir::StepVariant {
        name: "Complete".to_string(),
        name_sym: complete_sym,
        payload_ty: return_ty,
        is_complete: true,
    });
    // 各 effect 操作变体。
    for case in &outward_cases {
        let variant_name = case.replace('.', "_");
        let variant_sym = interner
            .get(&variant_name)
            .or_else(|| interner.get(case))
            .unwrap_or_default();
        step_variants.push(crate::mir::StepVariant {
            name: variant_name,
            name_sym: variant_sym,
            payload_ty: store.any(),
            is_complete: false,
        });
    }

    Some(crate::mir::EffectStepAbi {
        frame_ty,
        step_ty,
        step_variants,
        frame_local,
        state_local,
    })
}

/// 收集所有 Perform 站点。
fn collect_perform_sites(body: &Body, live_out: &[HashSet<LocalId>]) -> Vec<PerformSite> {
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
            let live_set = if resume_idx < live_out.len() {
                live_out[resume_idx].clone()
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

/// 在入口块开头初始化 frame tuple（state=0, 所有 live locals 为默认值）。
fn initialize_frame(
    body: &mut Body,
    frame_local: LocalId,
    frame_ty: TypeId,
    all_live_locals: &[LocalId],
    store: &mut TypeStore,
) {
    let start_idx = body.start.0 as usize;
    let span = body.blocks[start_idx]
        .stmts
        .first()
        .map(|s| s.span)
        .unwrap_or_default();
    // 构造 frame tuple 的元素：state=0（Int）+ 所有 live locals 的当前值。
    let mut elements: Vec<Operand> = vec![Operand::Const(crate::mir::ConstValue::Int(0, None))];
    for &lid in all_live_locals {
        elements.push(Operand::Local(lid));
    }
    let init_stmt = Statement {
        span,
        kind: StatementKind::Assign {
            target: frame_local,
            value: Rvalue::MakeTuple {
                elements,
                transport: AggregateTransportMetadata {
                    aggregate_ty: frame_ty,
                    kind: AggregateTransportKind::Tuple,
                    fields: Vec::new(),
                },
            },
        },
    };
    body.blocks[start_idx].stmts.insert(0, init_stmt);
    let _ = store;
}

/// 重写 Perform 站点：保存 live locals 到 frame + 返回 Step case。
fn rewrite_perform_sites(
    body: &mut Body,
    sites: &[PerformSite],
    live_to_slot: &HashMap<LocalId, u128>,
    frame_local: LocalId,
    frame_ty: TypeId,
    store: &mut TypeStore,
    interner: &Interner,
    fqn: &str,
) {
    for (state_num, site) in sites.iter().enumerate() {
        let state_num = (state_num + 1) as u128; // 1-based
        let span = body.blocks[site.block_idx].terminator.span;
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
        // 预计算 payload。
        let (payload, payload_ty) = if site.args.len() == 1 {
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
        // 预计算 Step 类型信息。
        // 合成 Step enum 使用函数自身 FQN（已 intern）作为 nominal FQN。
        // variant 使用 op_fqn（已 intern 的 effect 操作名）。
        // 这确保 Symbol 在后续 verify/dump 中可被原 interner 解析。
        let step_fqn_sym = interner.get(fqn).unwrap_or_default();
        let variant_name_sym = interner.get(&site.op_fqn).unwrap_or_default();
        let step_ty = store.value_nominal(scoop2_hir::ty::NominalType {
            fqn: step_fqn_sym,
            args: vec![],
            eff: None,
        });

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
    store: &mut TypeStore,
) -> LocalId {
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
        // 恢复 live locals。
        for (slot_idx, &live_local) in site.live_out_at_resume.iter().enumerate() {
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
                        index: (slot_idx + 1) as u128,
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
    state_local
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
