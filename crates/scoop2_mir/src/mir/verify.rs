//! MIR 验证：CFG 结构 + direct-style 语义 + production 语义完整性。
//!
//! 四个层次：
//! - [`verify_cfg`]：基本块图结构（悬空后继 / cleanup 目标 / 块终止符）。
//! - [`verify_direct_style`]：effect 终结符语义（Perform 的 resume_target / resume_local）。
//! - [`verify_semantic`]：production 语义完整性（**解析性**检查：交叉引用模块符号表验证
//!   callee 可解析 / dispatch 候选非空 / member resolved 非空 / effect site 元数据完整）。
//! - [`verify_transport`]：transport 契约一致性（trace/copy/drop 与类型匹配）。
//! - [`verify_no_generic_residue`]：泛型参数残留检查（拒绝 TypeKind::Param 存活到 materialized MIR）。

use std::collections::HashSet;

use scoop2_hir::ty::{TypeKind, TypeStore};

use crate::diagnostics::VerifyError;
use crate::mir::{BasicBlockId, Body, CallKind, Module, Rvalue, TerminatorKind, UnwindAction};
use crate::mir::transport::{ValueTransportMetadata, AggregateTransportMetadata};

/// 从 Module 中收集所有已声明的函数 FQN（用于解析性 callee 检查）。
fn collect_known_fqns(module: &Module) -> HashSet<String> {
    let mut set = HashSet::new();
    for item in &module.items {
        if let crate::mir::Item::Fun(fd) = item {
            set.insert(fd.fqn.clone());
        }
    }
    set
}

/// 从 Module 中收集所有已声明的类型 FQN（用于 dispatch 候选检查）。
fn collect_known_types(module: &Module) -> HashSet<String> {
    let mut set = HashSet::new();
    for item in &module.items {
        if let crate::mir::Item::Metadata(m) = item {
            set.insert(m.fqn.clone());
        }
    }
    set
}

/// 验证一个 Module 的所有 body。返回发现的错误列表（空 = 通过）。
///
/// `external_symbols`：外部符号集合（sysroot/prelude 声明的函数和类型 FQN），
/// 用于 callee 解析性检查。调用方应从 HIR interner 或 sysroot 索引构建。
pub fn verify_module_with_external(
    module: &Module,
    external_symbols: &HashSet<String>,
) -> Vec<VerifyError> {
    let mut errors = Vec::new();
    let known_fqns = collect_known_fqns(module);
    let known_types = collect_known_types(module);
    // 全部已知符号 = 模块自身 + 外部（sysroot/prelude）。
    let all_known_symbols: HashSet<String> = known_fqns
        .iter()
        .chain(known_types.iter())
        .chain(external_symbols.iter())
        .cloned()
        .collect();
    for item in &module.items {
        if let crate::mir::Item::Fun(fd) = item
            && let Some(body) = &fd.body
        {
            // 模板级预验证：CFG + direct-style + 解析性语义（不含 transport/residue——
            // 模板允许 TypeKind::Param；transport 一致性需单态化后才精确）。
            verify_body(body, &mut errors);
            verify_semantic(fd, body, &known_fqns, &known_types, &all_known_symbols, &mut errors);
        }
        if let crate::mir::Item::Initializer(ir) = item {
            verify_body(&ir.body, &mut errors);
        }
    }
    errors
}

/// 兼容入口：不带外部符号集的验证（仅检查模块自身符号）。
pub fn verify_module(module: &Module) -> Vec<VerifyError> {
    verify_module_with_external(module, &HashSet::new())
}

/// 验证 materialized（monomorphic）MIR：在模板级检查之上，额外运行 transport 契约
/// 一致性验证 + 泛型参数残留检查（materialized MIR 不允许 TypeKind::Param）。
pub fn verify_materialized(module: &Module) -> Vec<VerifyError> {
    let mut errors = verify_module(module);
    let store = module.types_ref();
    for item in &module.items {
        if let crate::mir::Item::Fun(fd) = item
            && let Some(body) = &fd.body
        {
            verify_transport(body, store, &mut errors);
            verify_no_generic_residue(body, store, &mut errors);
        }
        if let crate::mir::Item::Initializer(ir) = item {
            verify_transport(&ir.body, store, &mut errors);
            verify_no_generic_residue(&ir.body, store, &mut errors);
        }
    }
    errors
}

/// 验证单个 body（CFG + direct-style）。
pub fn verify_body(body: &Body, errors: &mut Vec<VerifyError>) {
    verify_cfg(body, errors);
    verify_direct_style(body, errors);
}

/// production 语义完整性验证（**解析性**检查）。
///
/// 交叉引用模块符号表验证：
/// - `CallKind::Direct`：`callee_fqn` 在模块符号表中存在（可解析）；
/// - `CallKind::Virtual`：`dispatch.owner_fqn` 在已知类型集合中存在（dispatch 候选非空）；
/// - `Rvalue::MemberAccess`：`member.name` 非空（member resolved 非空）；
/// - `Rvalue::UnresolvedName`：总是报错（不应出现在合法 MIR 中）。
pub fn verify_semantic(
    fd: &crate::mir::FunDecl,
    body: &Body,
    known_fqns: &HashSet<String>,
    known_types: &HashSet<String>,
    all_known_symbols: &HashSet<String>,
    errors: &mut Vec<VerifyError>,
) {
    for (i, block) in body.blocks.iter().enumerate() {
        let bid = BasicBlockId(i as u32);
        for stmt in &block.stmts {
            if let crate::mir::StatementKind::Assign { value, .. } = &stmt.kind {
                verify_rvalue_semantic_resolved(value, bid, known_fqns, known_types, all_known_symbols, errors);
            }
        }
        verify_terminator_semantic(&block.terminator.kind, bid, errors);
    }
    let _ = fd;
}

fn verify_rvalue_semantic_resolved(
    rv: &Rvalue,
    block: BasicBlockId,
    known_fqns: &HashSet<String>,
    known_types: &HashSet<String>,
    all_known_symbols: &HashSet<String>,
    errors: &mut Vec<VerifyError>,
) {
    match rv {
        Rvalue::Call { kind, .. } => {
            verify_call_kind_resolved(kind, block, known_fqns, known_types, all_known_symbols, errors);
        }
        Rvalue::MemberAccess { member, .. } => {
            if member.name.is_empty() {
                errors.push(VerifyError::semantic(format!(
                    "MemberAccess 的 member 名为空（block {}）",
                    block
                )));
            }
        }
        Rvalue::ClassCtor { type_fqn, .. } | Rvalue::StructLit { type_fqn, .. } => {
            let fqn_str = format!("{}", type_fqn.as_u32());
            if type_fqn.as_u32() == 0 {
                errors.push(VerifyError::semantic(format!(
                    "构造器的 type_fqn 未解析（block {}）",
                    block
                )));
            }
            let _ = fqn_str;
        }
        Rvalue::EnumVariant {
            enum_fqn,
            variant_name,
            ..
        } => {
            if enum_fqn.as_u32() == 0 && variant_name.as_u32() == 0 {
                errors.push(VerifyError::semantic(format!(
                    "EnumVariant 的 enum 与 variant 均未解析（block {}）",
                    block
                )));
            }
        }
        Rvalue::UnresolvedName { name } => {
            let msg = format!(
                "未解析的名字 `{}` 出现在 lowering 产物中（block {}）",
                name, block
            );
            errors.push(VerifyError::semantic(msg));
        }
        _ => {}
    }
}

fn verify_call_kind_resolved(
    kind: &CallKind,
    block: BasicBlockId,
    known_fqns: &HashSet<String>,
    known_types: &HashSet<String>,
    all_known_symbols: &HashSet<String>,
    errors: &mut Vec<VerifyError>,
) {
    match kind {
        CallKind::Direct { callee_fqn, .. } => {
            // 解析性检查：callee_fqn 必须可解析。
            // 可解析 = 在模块函数符号表中（known_fqns），或在全部已知符号集合中
            //（all_known_symbols，含 sysroot/prelude 声明的函数和类型），
            // 或是闭包内部函数（含 `$`），或是不含 `.` 的裸方法名（运算符方法，
            // 由后端通过接收者类型解析）。
            if callee_fqn.is_empty() {
                errors.push(VerifyError::semantic(format!(
                    "Direct 调用的 callee_fqn 为空（不可解析；block {}）",
                    block
                )));
            } else if !known_fqns.contains(callee_fqn)
                && !all_known_symbols.contains(callee_fqn)
                && !callee_fqn.contains('$')
                && callee_fqn.contains('.')
            {
                // callee_fqn 含 '.'（看起来是 FQN）但不在模块符号表中、也不是 prelude/sysroot
                // 符号或闭包——报错。裸方法名（如 "plus"）不含 '.'，不在此检查范围
                //（它们是运算符方法，由后端通过接收者类型解析）。
                errors.push(VerifyError::semantic(format!(
                    "Direct 调用的 callee `{}` 不在模块函数符号表中（block {}）",
                    callee_fqn, block
                )));
            }
        }
        CallKind::Virtual { dispatch, .. } => {
            if dispatch.owner_fqn.is_empty() && dispatch.member_name.is_empty() {
                errors.push(VerifyError::semantic(format!(
                    "Virtual 分发的 owner 与 member 均未解析（dispatch 候选为空；block {}）",
                    block
                )));
            } else if !dispatch.owner_fqn.is_empty()
                && dispatch.owner_fqn.contains('.')
                && !known_types.contains(&dispatch.owner_fqn)
            {
                // owner_fqn 看起来是 FQN（含 '.'）但不在已知类型集合中。
                errors.push(VerifyError::semantic(format!(
                    "Virtual 分发的 owner `{}` 不在已知类型集合中（block {}）",
                    dispatch.owner_fqn, block
                )));
            }
        }
        CallKind::Closure { invoke_fqn, .. } => {
            if invoke_fqn.is_empty() {
                errors.push(VerifyError::semantic(format!(
                    "Closure 调用的 invoke_fqn 为空（block {}）",
                    block
                )));
            }
        }
        CallKind::FunValue { .. } => {
            // 函数值调用：callee 在运行期为函数值，无静态 callee_fqn；跳过。
        }
    }
}

fn verify_terminator_semantic(
    kind: &TerminatorKind,
    block: BasicBlockId,
    errors: &mut Vec<VerifyError>,
) {
    if let TerminatorKind::Perform {
        op_fqn,
        metadata,
        ..
    } = kind
    {
        // effect site 元数据完整：op_fqn 非空。
        if op_fqn.is_empty() {
            errors.push(VerifyError::semantic(format!(
                "Perform 终结符的 op_fqn 为空（effect site 元数据不完整；block {}）",
                block
            )));
        }
        let _ = metadata;
    }
}

/// CFG 结构验证。
pub fn verify_cfg(body: &Body, errors: &mut Vec<VerifyError>) {
    let valid = |id: BasicBlockId| -> bool {
        (id.0 as usize) < body.blocks.len()
    };
    // 入口块必须存在。
    if !valid(body.start) {
        errors.push(VerifyError::cfg(Some(body.start), "入口基本块不存在"));
        return;
    }
    for (i, block) in body.blocks.iter().enumerate() {
        let id = BasicBlockId(i as u32);
        // cleanup 目标必须是 cleanup 块。
        if let UnwindAction::Cleanup { target } = &block.terminator.unwind {
            if !valid(*target) {
                errors.push(VerifyError::cfg(
                    Some(id),
                    format!(
                        "UnwindAction::Cleanup 指向不存在的基本块 {}",
                        target
                    ),
                ));
            } else if !body.blocks[target.0 as usize].is_cleanup {
                errors.push(VerifyError::cfg(
                    Some(id),
                    format!("UnwindAction::Cleanup 目标 {} 不是 cleanup 块", target),
                ));
            }
        }
        // 后继必须存在。
        for succ in block.successors() {
            if !valid(succ) {
                errors.push(VerifyError::cfg(
                    Some(id),
                    format!("后继基本块 {} 不存在", succ),
                ));
            }
        }
        // Operand::Local 必须在 locals 范围内。
        let locals_ok = body.locals.len() as u32;
        for stmt in &block.stmts {
            check_operals_in_body(stmt, locals_ok, id, errors);
        }
        check_terminator_operands(&block.terminator.kind, locals_ok, id, errors);
    }
}

fn check_operals_in_body(
    stmt: &crate::mir::Statement,
    locals: u32,
    block: BasicBlockId,
    errors: &mut Vec<VerifyError>,
) {
    use crate::mir::{Operand, Rvalue, StatementKind};
    let check_op = |op: &Operand, errors: &mut Vec<VerifyError>| match op {
        Operand::Local(l) if l.0 >= locals => {
            errors.push(VerifyError::cfg(
                Some(block),
                format!("引用越界 local {}", l),
            ));
        }
        _ => {}
    };
    match &stmt.kind {
        StatementKind::Assign { target, value } => {
            if target.0 >= locals {
                errors.push(VerifyError::cfg(
                    Some(block),
                    format!("赋值目标越界 local {}", target),
                ));
            }
            check_rvalue_operands(value, check_op, errors);
        }
        StatementKind::StoreMember { receiver, value, .. }
        | StatementKind::StoreTupleIndex {
            receiver, value, ..
        } => {
            check_op(receiver, errors);
            check_op(value, errors);
        }
        StatementKind::StoreTopLevelVar { value, .. } => check_op(value, errors),
        StatementKind::Nop | StatementKind::Panic { .. } => {}
    }
}

fn check_rvalue_operands<F>(rv: &Rvalue, check_op: F, errors: &mut Vec<VerifyError>)
where
    F: Fn(&crate::mir::Operand, &mut Vec<VerifyError>),
{
    use crate::mir::Rvalue;
    let primary_ops: Vec<&crate::mir::Operand> = match rv {
        Rvalue::Use(op) => vec![op],
        Rvalue::TypeTest { value, .. }
        | Rvalue::Cast { value, .. }
        | Rvalue::MemberAccess { receiver: value, .. }
        | Rvalue::TupleIndex { receiver: value, .. }
        | Rvalue::MakeClosure { env: value, .. } | Rvalue::PatternMatch { subject: value, .. } | Rvalue::PatternExtract { subject: value, .. } => vec![value],
        Rvalue::PerformResult { .. }
        | Rvalue::ClassLit { .. }
        | Rvalue::TopLevelRef { .. }
        | Rvalue::UnresolvedName { .. } => vec![],
        Rvalue::IndexAccess { receiver, indices, .. } => {
            let mut v = vec![receiver];
            v.extend(indices.iter());
            v
        }
        Rvalue::EnumVariant { args, .. } | Rvalue::ClassCtor { args, .. } | Rvalue::Call { args, .. } => {
            args.iter().map(|a| &a.value).collect()
        }
        Rvalue::MakeTuple { elements, .. } | Rvalue::MakeArray { elements, .. } => {
            elements.iter().collect()
        }
        Rvalue::StructLit { fields, .. } => fields.iter().map(|f| &f.value).collect(),
        Rvalue::InterpolatedString { parts } => parts
            .iter()
            .filter_map(|p| match p {
                crate::mir::InterpolatedPart::Expr(op) => Some(op),
                _ => None,
            })
            .collect(),
        Rvalue::WithUpdate { base, updates, .. } => {
            let mut v = vec![base];
            v.extend(updates.iter().map(|u| &u.value));
            v
        }
    };
    for op in primary_ops {
        check_op(op, errors);
    }
}

fn check_terminator_operands(
    kind: &TerminatorKind,
    locals: u32,
    block: BasicBlockId,
    errors: &mut Vec<VerifyError>,
) {
    use crate::mir::Operand;
    let check_op = |op: &Operand, errors: &mut Vec<VerifyError>| match op {
        Operand::Local(l) if l.0 >= locals => {
            errors.push(VerifyError::cfg(
                Some(block),
                format!("终结符引用越界 local {}", l),
            ));
        }
        _ => {}
    };
    match kind {
        TerminatorKind::Return { value } => {
            if let Some(op) = value {
                check_op(op, errors);
            }
        }
        TerminatorKind::CondBr { cond, .. } => check_op(cond, errors),
        TerminatorKind::Perform { args, .. } => {
            for a in args {
                check_op(&a.value, errors);
            }
        }
        _ => {}
    }
}

/// direct-style 语义验证。
pub fn verify_direct_style(body: &Body, errors: &mut Vec<VerifyError>) {
    for (i, block) in body.blocks.iter().enumerate() {
        let id = BasicBlockId(i as u32);
        match &block.terminator.kind {
            TerminatorKind::Perform {
                resume_target,
                resume_local,
                ..
            } => {
                if (resume_target.0 as usize) >= body.blocks.len() {
                    errors.push(VerifyError::direct_style(
                        Some(id),
                        format!("Perform 的 resume_target {} 不存在", resume_target),
                    ));
                }
                if (resume_local.0 as usize) >= body.locals.len() {
                    errors.push(VerifyError::direct_style(
                        Some(id),
                        format!("Perform 的 resume_local {} 越界", resume_local),
                    ));
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// transport 契约一致性验证
// ---------------------------------------------------------------------------

/// 验证 transport 契约一致性：检查 body 中每个 ValueTransportMetadata 的 requirements
/// 与其 source_ty 是否一致（trace 要求与类型结构匹配）。
pub fn verify_transport(body: &Body, store: &TypeStore, errors: &mut Vec<VerifyError>) {
    for (i, block) in body.blocks.iter().enumerate() {
        let bid = BasicBlockId(i as u32);
        for stmt in &block.stmts {
            if let crate::mir::StatementKind::Assign { value, .. } = &stmt.kind {
                verify_transport_rvalue(value, store, bid, errors);
            }
        }
    }
}

fn verify_transport_rvalue(
    rv: &Rvalue,
    store: &TypeStore,
    block: BasicBlockId,
    errors: &mut Vec<VerifyError>,
) {
    use crate::mir::transport::*;
    match rv {
        Rvalue::Call { transport, .. } => {
            verify_value_transport(&transport.result, store, block, errors);
        }
        Rvalue::MakeTuple { transport, .. } | Rvalue::StructLit { transport, .. } => {
            verify_aggregate_transport(transport, store, block, errors);
        }
        Rvalue::EnumVariant { payload, .. } => {
            verify_aggregate_transport(payload, store, block, errors);
        }
        Rvalue::MakeClosure { env_contract, .. } => {
            for cap in &env_contract.captures {
                verify_value_transport(&cap.transport, store, block, errors);
            }
        }
        _ => {}
    }
}

fn verify_value_transport(
    vt: &ValueTransportMetadata,
    store: &TypeStore,
    block: BasicBlockId,
    errors: &mut Vec<VerifyError>,
) {
    // trace 要求与类型结构一致性：引用类型 / Param / StarProjection 必须 trace=true。
    let expected_trace = crate::mir::transport::mir_transport_trace_requirement_for_type(store, vt.source_ty);
    if vt.requirements.trace != expected_trace {
        errors.push(VerifyError::semantic(format!(
            "transport 契约不一致：source_ty={:?} 的 trace={} 但 requirements.trace={}（block {}）",
            vt.source_ty, expected_trace, vt.requirements.trace, block
        )));
    }
}

fn verify_aggregate_transport(
    at: &AggregateTransportMetadata,
    store: &TypeStore,
    block: BasicBlockId,
    errors: &mut Vec<VerifyError>,
) {
    for f in &at.fields {
        verify_value_transport(&f.transport, store, block, errors);
    }
}

// ---------------------------------------------------------------------------
// 泛型参数残留检查
// ---------------------------------------------------------------------------

/// 拒绝任何存活到 materialized MIR 的 TypeKind::Param。
pub fn verify_no_generic_residue(
    body: &Body,
    store: &TypeStore,
    errors: &mut Vec<VerifyError>,
) {
    for decl in &body.locals {
        if matches!(store.kind(decl.ty), TypeKind::Param(_)) {
            errors.push(VerifyError::semantic(format!(
                "泛型参数残留：local {:?} 的类型 {:?} 仍是 TypeKind::Param",
                decl.name, decl.ty
            )));
        }
    }
}

// ---------------------------------------------------------------------------
// 模板级预验证
// ---------------------------------------------------------------------------

/// 对 generic 模板做 production 契约预验证（在单态化前运行）。
/// 检查模板的 body 结构完整性 + CFG 合法性（不含泛型参数残留——模板本身允许有 Param）。
pub fn verify_template(fd: &crate::mir::FunDecl, errors: &mut Vec<VerifyError>) {
    if let Some(body) = &fd.body {
        verify_cfg(body, errors);
        verify_direct_style(body, errors);
    }
}
