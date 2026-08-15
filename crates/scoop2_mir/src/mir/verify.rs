//! MIR 验证：CFG 结构 + direct-style 语义 + production 语义完整性。
//!
//! 四个层次：
//! - [`verify_cfg`]：基本块图结构（悬空后继 / 块终止符）。
//! - [`verify_direct_style`]：effect 终结符语义（Perform 的 resume_target / resume_local）。
//! - [`verify_semantic`]：production 语义完整性（**解析性**检查：交叉引用模块符号表验证
//!   callee 可解析 / dispatch 候选非空 / member resolved 非空 / effect site 元数据完整）。
//! - [`verify_transport`]：transport 契约一致性（trace/copy/drop 与类型匹配）。
//! - [`verify_no_generic_residue`]：泛型参数残留检查（拒绝 TypeKind::Param 存活到 materialized MIR）。

use std::collections::HashSet;

use scoop2_hir::ty::{RefTypeKind, TypeId, TypeKind, TypeStore};

use crate::diagnostics::VerifyError;
use crate::mir::transport::{AggregateTransportMetadata, ValueTransportMetadata};
use crate::mir::{BasicBlockId, Body, CallKind, Module, Rvalue, TerminatorKind};

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
    // M5（C 类转 ICE）：verify 是编译器自检——对合法程序失败即 bug。返回值
    // 仍为错误列表（调用方决定通道），但 debug 构建先断言（不静默吞）。
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
            verify_semantic(
                fd,
                body,
                &known_fqns,
                &known_types,
                &all_known_symbols,
                &mut errors,
            );
        }
        if let crate::mir::Item::Initializer(ir) = item {
            verify_body(&ir.body, &mut errors);
        }
    }
{
        // M5 C 类 ICE：verify 对合法程序失败 = 编译器 bug（debug 断言暴露；
        // release 返回错误列表交由调用方走 bug 通道，不进用户诊断）。
        debug_assert!(
            errors.is_empty(),
            "ICE[verify]: MIR 自检失败（编译器 bug）：{:?}",
            errors
        );
        errors
    }
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
{
        // M5 C 类 ICE：verify 对合法程序失败 = 编译器 bug（debug 断言暴露；
        // release 返回错误列表交由调用方走 bug 通道，不进用户诊断）。
        debug_assert!(
            errors.is_empty(),
            "ICE[verify]: MIR 自检失败（编译器 bug）：{:?}",
            errors
        );
        errors
    }
}

/// 验证 materialized MIR（带外部符号集，用于跨模块/外部函数解析性检查）。
pub fn verify_materialized_with_external(
    module: &Module,
    external_symbols: &HashSet<String>,
) -> Vec<VerifyError> {
    let mut errors = verify_module_with_external(module, external_symbols);
    let store = module.types_ref();
    for item in &module.items {
        if let crate::mir::Item::Fun(fd) = item
            && let Some(body) = &fd.body
        {
            verify_transport(body, store, &mut errors);
            verify_no_generic_residue(body, store, &mut errors);
            verify_no_effect_residue(body, &mut errors);
        }
        if let crate::mir::Item::Initializer(ir) = item {
            verify_transport(&ir.body, store, &mut errors);
            verify_no_generic_residue(&ir.body, store, &mut errors);
            verify_no_effect_residue(&ir.body, &mut errors);
        }
    }
{
        // M5 C 类 ICE：verify 对合法程序失败 = 编译器 bug（debug 断言暴露；
        // release 返回错误列表交由调用方走 bug 通道，不进用户诊断）。
        debug_assert!(
            errors.is_empty(),
            "ICE[verify]: MIR 自检失败（编译器 bug）：{:?}",
            errors
        );
        errors
    }
}

/// 验证 effect lowering 后无 Perform/Handle 终结符残留。
/// 被 Handle 捕获的 Perform 和 Handle 终结符应已被 effect_lower pass 消除。
fn verify_no_effect_residue(body: &Body, errors: &mut Vec<VerifyError>) {
    for (i, block) in body.blocks.iter().enumerate() {
        match &block.terminator.kind {
            TerminatorKind::Handle { .. } => {
                errors.push(VerifyError::semantic(format!(
                    "effect lowering 后仍有 Handle 终结符残留（block {}）",
                    BasicBlockId(i as u32)
                )));
            }
            TerminatorKind::Perform { .. } => {
                errors.push(VerifyError::semantic(format!(
                    "effect lowering 后仍有 Perform 终结符残留（block {}）",
                    BasicBlockId(i as u32)
                )));
            }
            _ => {}
        }
    }
    // 检查 Resume 调用残留。
    for (i, block) in body.blocks.iter().enumerate() {
        for stmt in &block.stmts {
            if let crate::mir::StatementKind::Assign { value, .. } = &stmt.kind {
                if let crate::mir::Rvalue::Call {
                    kind: crate::mir::CallKind::Resume { .. },
                    ..
                } = value
                {
                    errors.push(VerifyError::semantic(format!(
                        "effect lowering 后仍有 Resume 调用残留（block {}）",
                        BasicBlockId(i as u32)
                    )));
                }
            }
        }
    }
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
                verify_rvalue_semantic_resolved(
                    value,
                    bid,
                    known_fqns,
                    known_types,
                    all_known_symbols,
                    errors,
                );
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
            verify_call_kind_resolved(
                kind,
                block,
                known_fqns,
                known_types,
                all_known_symbols,
                errors,
            );
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
        CallKind::Virtual { dispatch, .. } | CallKind::Interface { dispatch, .. } => {
            let kind_label = if matches!(kind, CallKind::Interface { .. }) {
                "Interface"
            } else {
                "Virtual"
            };
            if dispatch.owner_fqn.is_empty() && dispatch.member_name.is_empty() {
                errors.push(VerifyError::semantic(format!(
                    "{} 分发的 owner 与 member 均未解析（dispatch 候选为空；block {}）",
                    kind_label, block
                )));
            } else if !dispatch.owner_fqn.is_empty()
                && dispatch.owner_fqn.contains('.')
                && !known_types.contains(&dispatch.owner_fqn)
            {
                // owner_fqn 看起来是 FQN（含 '.'）但不在已知类型集合中。
                errors.push(VerifyError::semantic(format!(
                    "{} 分发的 owner `{}` 不在已知类型集合中（block {}）",
                    kind_label, dispatch.owner_fqn, block
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
        CallKind::Resume { .. } => {
            // continuation resume：continuation 在运行期为 continuation 对象；跳过。
        }
    }
}

fn verify_terminator_semantic(
    kind: &TerminatorKind,
    block: BasicBlockId,
    errors: &mut Vec<VerifyError>,
) {
    if let TerminatorKind::Perform {
        op_fqn, metadata, ..
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
    let valid = |id: BasicBlockId| -> bool { (id.0 as usize) < body.blocks.len() };
    // 入口块必须存在。
    if !valid(body.start) {
        errors.push(VerifyError::cfg(Some(body.start), "入口基本块不存在"));
        return;
    }
    for (i, block) in body.blocks.iter().enumerate() {
        let id = BasicBlockId(i as u32);
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
        StatementKind::StoreMember {
            receiver, value, ..
        }
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
        | Rvalue::MemberAccess {
            receiver: value, ..
        }
        | Rvalue::TupleIndex {
            receiver: value, ..
        }
        | Rvalue::MakeClosure { env: value, .. }
        | Rvalue::PatternMatch { subject: value, .. }
        | Rvalue::PatternExtract { subject: value, .. } => vec![value],
        Rvalue::IntEq { lhs, rhs } => vec![lhs, rhs],
        Rvalue::PerformResult { .. }
        | Rvalue::ClassLit { .. }
        | Rvalue::MakeContinuation { .. }
        | Rvalue::MakeChainLink { .. }
        | Rvalue::TakeChainLink { .. }
        | Rvalue::ResumeChainLink { .. }
        | Rvalue::TopLevelRef { .. }
        | Rvalue::UnresolvedName { .. } => vec![],
        Rvalue::IndexAccess {
            receiver, indices, ..
        } => {
            let mut v = vec![receiver];
            v.extend(indices.iter());
            v
        }
        Rvalue::EnumVariant { args, .. }
        | Rvalue::ClassCtor { args, .. }
        | Rvalue::Call { args, .. } => args.iter().map(|a| &a.value).collect(),
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
    let expected_trace =
        crate::mir::transport::mir_transport_trace_requirement_for_type(store, vt.source_ty);
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
///
/// 检查 locals、statements 中的 TypeId、terminators 中的 TypeId。
pub fn verify_no_generic_residue(body: &Body, store: &TypeStore, errors: &mut Vec<VerifyError>) {
    // locals
    for decl in &body.locals {
        check_type_for_param(store, decl.ty, &format!("local {:?}", decl.name), errors);
    }
    // statements + rvalues
    for (bi, block) in body.blocks.iter().enumerate() {
        let bid = BasicBlockId(bi as u32);
        for stmt in &block.stmts {
            verify_no_residue_statement(stmt, store, bid, errors);
        }
        verify_no_residue_terminator(&block.terminator, store, bid, errors);
    }
}

fn check_type_for_param(store: &TypeStore, ty: TypeId, ctx: &str, errors: &mut Vec<VerifyError>) {
    // 递归检查：不仅看顶层 TypeKind::Param，也看 Nominal/Function/Option/Tuple/Union 内嵌的 Param。
    use scoop2_hir::ty::ValueTypeKind;
    match store.kind(ty) {
        TypeKind::Param(_) => {
            errors.push(VerifyError::semantic(format!(
                "泛型参数残留：{ctx} 的类型 {:?} 仍是 TypeKind::Param",
                ty
            )));
        }
        TypeKind::Ref(RefTypeKind::Nominal(n)) => {
            for &arg in &n.args {
                check_type_for_param(store, arg, ctx, errors);
            }
            if let Some(eff) = &n.eff {
                check_effect_row_for_param(store, eff, ctx, errors);
            }
        }
        TypeKind::Ref(RefTypeKind::Function(f)) => {
            if let Some(r) = f.receiver {
                check_type_for_param(store, r, ctx, errors);
            }
            for &p in &f.params {
                check_type_for_param(store, p, ctx, errors);
            }
            check_type_for_param(store, f.return_ty, ctx, errors);
            check_effect_row_for_param(store, &f.effects, ctx, errors);
        }
        TypeKind::Ref(RefTypeKind::Union(u)) => {
            for &v in &u.variants {
                check_type_for_param(store, v, ctx, errors);
            }
        }
        TypeKind::Value(ValueTypeKind::Tuple(elems)) => {
            for &e in elems {
                check_type_for_param(store, e, ctx, errors);
            }
        }
        TypeKind::Value(ValueTypeKind::Nominal(n)) => {
            for &arg in &n.args {
                check_type_for_param(store, arg, ctx, errors);
            }
            if let Some(eff) = &n.eff {
                check_effect_row_for_param(store, eff, ctx, errors);
            }
        }
        _ => {}
    }
}

/// 检查 EffectRow 中是否含 TypeKind::Param term。
fn check_effect_row_for_param(
    store: &TypeStore,
    row: &scoop2_hir::ty::EffectRow,
    ctx: &str,
    errors: &mut Vec<VerifyError>,
) {
    for &term in &row.terms {
        check_type_for_param(store, term, ctx, errors);
    }
}

/// 检查 Vec<TypeId> 中每个元素。
fn check_type_ids_for_param(
    store: &TypeStore,
    tys: &[TypeId],
    ctx: &str,
    errors: &mut Vec<VerifyError>,
) {
    for &ty in tys {
        check_type_for_param(store, ty, ctx, errors);
    }
}

/// 检查 Option<TypeId>。
fn check_optional_type_for_param(
    store: &TypeStore,
    ty: Option<TypeId>,
    ctx: &str,
    errors: &mut Vec<VerifyError>,
) {
    if let Some(ty) = ty {
        check_type_for_param(store, ty, ctx, errors);
    }
}

fn verify_no_residue_statement(
    stmt: &crate::mir::Statement,
    store: &TypeStore,
    block: BasicBlockId,
    errors: &mut Vec<VerifyError>,
) {
    match &stmt.kind {
        crate::mir::StatementKind::Assign { value, .. } => {
            verify_no_residue_rvalue(value, store, block, errors);
        }
        crate::mir::StatementKind::StoreMember {
            member, value_ty, ..
        } => {
            check_type_for_param(
                store,
                member.receiver_ty,
                &format!("StoreMember.receiver_ty (block {block})"),
                errors,
            );
            check_effect_row_for_param(
                store,
                &member.hidden_effects,
                &format!("StoreMember.hidden_effects (block {block})"),
                errors,
            );
            check_type_for_param(
                store,
                *value_ty,
                &format!("StoreMember value_ty (block {block})"),
                errors,
            );
        }
        crate::mir::StatementKind::StoreTupleIndex { value_ty, .. }
        | crate::mir::StatementKind::StoreTopLevelVar { value_ty, .. } => {
            check_type_for_param(
                store,
                *value_ty,
                &format!("StoreXxx value_ty (block {block})"),
                errors,
            );
        }
        _ => {}
    }
}

fn verify_no_residue_rvalue(
    rv: &Rvalue,
    store: &TypeStore,
    block: BasicBlockId,
    errors: &mut Vec<VerifyError>,
) {
    use crate::mir::transport::*;
    match rv {
        Rvalue::Call {
            kind,
            args,
            transport,
            ..
        } => {
            // 检查 CallKind 中的类型参数残留。
            match kind {
                crate::mir::CallKind::Direct {
                    type_args,
                    generic_type_args,
                    generic_eff_args,
                    ..
                } => {
                    check_type_ids_for_param(
                        store,
                        type_args,
                        &format!("Direct.type_args (block {block})"),
                        errors,
                    );
                    check_type_ids_for_param(
                        store,
                        generic_type_args,
                        &format!("Direct.generic_type_args (block {block})"),
                        errors,
                    );
                    for r in generic_eff_args {
                        check_effect_row_for_param(
                            store,
                            r,
                            &format!("Direct.generic_eff_args (block {block})"),
                            errors,
                        );
                    }
                }
                crate::mir::CallKind::Virtual { dispatch, .. }
                | crate::mir::CallKind::Interface { dispatch, .. } => {
                    check_type_for_param(
                        store,
                        dispatch.receiver_ty,
                        &format!("dispatch.receiver_ty (block {block})"),
                        errors,
                    );
                    check_type_ids_for_param(
                        store,
                        &dispatch.generic_type_args,
                        &format!("dispatch.generic_type_args (block {block})"),
                        errors,
                    );
                    for r in &dispatch.generic_eff_args {
                        check_effect_row_for_param(
                            store,
                            r,
                            &format!("dispatch.generic_eff_args (block {block})"),
                            errors,
                        );
                    }
                }
                _ => {}
            }
            // 检查 args 的 value_ty。
            for a in args {
                check_type_for_param(
                    store,
                    a.value_ty,
                    &format!("CallArg.value_ty (block {block})"),
                    errors,
                );
            }
            // 检查 transport。
            check_type_for_param(
                store,
                transport.result.source_ty,
                &format!("Call.transport.result (block {block})"),
                errors,
            );
            if let Some(ar) = &transport.aggregate_return {
                check_type_for_param(
                    store,
                    ar.source_ty,
                    &format!("Call.transport.aggregate_return (block {block})"),
                    errors,
                );
            }
            if let Some(arr) = &transport.array {
                check_type_for_param(
                    store,
                    arr.array_ty,
                    &format!("Call.transport.array.array_ty (block {block})"),
                    errors,
                );
                check_type_for_param(
                    store,
                    arr.element_ty,
                    &format!("Call.transport.array.element_ty (block {block})"),
                    errors,
                );
            }
            if let Some(gc) = &transport.gc {
                check_type_for_param(
                    store,
                    gc.subject_ty,
                    &format!("Call.transport.gc.subject_ty (block {block})"),
                    errors,
                );
            }
        }
        Rvalue::TopLevelRef(tl) => {
            check_type_ids_for_param(
                store,
                &tl.generic_type_args,
                &format!("TopLevelRef.generic_type_args (block {block})"),
                errors,
            );
            check_effect_row_for_param(
                store,
                &tl.hidden_effects,
                &format!("TopLevelRef.hidden_effects (block {block})"),
                errors,
            );
            for r in &tl.generic_eff_args {
                check_effect_row_for_param(
                    store,
                    r,
                    &format!("TopLevelRef.generic_eff_args (block {block})"),
                    errors,
                );
            }
        }
        Rvalue::MemberAccess { member, .. } => {
            check_type_for_param(
                store,
                member.receiver_ty,
                &format!("MemberAccess.receiver_ty (block {block})"),
                errors,
            );
            check_effect_row_for_param(
                store,
                &member.hidden_effects,
                &format!("MemberAccess.hidden_effects (block {block})"),
                errors,
            );
        }
        Rvalue::TupleIndex { element_ty, .. } | Rvalue::IndexAccess { element_ty, .. } => {
            check_type_for_param(
                store,
                *element_ty,
                &format!("element_ty (block {block})"),
                errors,
            );
        }
        Rvalue::EnumVariant {
            enum_ty,
            payload,
            args,
            ..
        } => {
            check_type_for_param(
                store,
                *enum_ty,
                &format!("EnumVariant.enum_ty (block {block})"),
                errors,
            );
            check_type_for_param(
                store,
                payload.aggregate_ty,
                &format!("EnumVariant.payload.aggregate_ty (block {block})"),
                errors,
            );
            for a in args {
                check_type_for_param(
                    store,
                    a.value_ty,
                    &format!("EnumVariant.arg.value_ty (block {block})"),
                    errors,
                );
            }
        }
        Rvalue::ClassCtor {
            hidden_effects,
            args,
            ..
        } => {
            for a in args {
                check_type_for_param(
                    store,
                    a.value_ty,
                    &format!("ClassCtor.arg.value_ty (block {block})"),
                    errors,
                );
            }
            check_effect_row_for_param(
                store,
                hidden_effects,
                &format!("ClassCtor.hidden_effects (block {block})"),
                errors,
            );
        }
        Rvalue::MakeTuple { transport, .. } | Rvalue::StructLit { transport, .. } => {
            check_type_for_param(
                store,
                transport.aggregate_ty,
                &format!("aggregate_ty (block {block})"),
                errors,
            );
        }
        Rvalue::StructLit { fields, .. } => {
            for f in fields {
                check_type_for_param(
                    store,
                    f.value_ty,
                    &format!("StructLitField.value_ty (block {block})"),
                    errors,
                );
            }
        }
        Rvalue::MakeArray { result_ty, .. } | Rvalue::WithUpdate { result_ty, .. } => {
            check_type_for_param(
                store,
                *result_ty,
                &format!("result_ty (block {block})"),
                errors,
            );
        }
        Rvalue::WithUpdate { updates, .. } => {
            for u in updates {
                check_type_for_param(
                    store,
                    u.value_ty,
                    &format!("WithUpdateField.value_ty (block {block})"),
                    errors,
                );
            }
        }
        Rvalue::MakeClosure { env_contract, .. } => {
            check_type_for_param(
                store,
                env_contract.env_ty,
                &format!("MakeClosure.env_ty (block {block})"),
                errors,
            );
        }
        Rvalue::PerformResult { result_ty, .. } => {
            check_type_for_param(
                store,
                *result_ty,
                &format!("PerformResult.result_ty (block {block})"),
                errors,
            );
        }
        Rvalue::PatternExtract { result_ty, .. } => {
            check_type_for_param(
                store,
                *result_ty,
                &format!("PatternExtract.result_ty (block {block})"),
                errors,
            );
        }
        Rvalue::IntEq { .. } => {}
        Rvalue::PatternMatch { pattern, .. } => {
            verify_no_residue_pattern(pattern, store, block, errors);
        }
        Rvalue::TypeTest { metadata, .. } => {
            verify_no_residue_type_test(metadata, store, block, errors);
        }
        Rvalue::Cast { metadata, .. } => {
            verify_no_residue_type_test(&metadata.test, store, block, errors);
            use crate::mir::transport::RuntimeCastResult as R;
            match &metadata.result {
                R::Target { ty } => check_type_for_param(
                    store,
                    *ty,
                    &format!("Cast.result.Target (block {block})"),
                    errors,
                ),
                R::Option { option_ty, some_ty } => {
                    check_type_for_param(
                        store,
                        *option_ty,
                        &format!("Cast.result.Option.option_ty (block {block})"),
                        errors,
                    );
                    check_type_for_param(
                        store,
                        *some_ty,
                        &format!("Cast.result.Option.some_ty (block {block})"),
                        errors,
                    );
                }
            }
        }
        _ => {}
    }
}

fn verify_no_residue_type_test(
    m: &crate::mir::transport::RuntimeTypeTestMetadata,
    store: &TypeStore,
    block: BasicBlockId,
    errors: &mut Vec<VerifyError>,
) {
    use crate::mir::transport::RuntimeTypeParameterizedMatch as P;
    check_type_for_param(
        store,
        m.source_ty,
        &format!("TypeTest.source_ty (block {block})"),
        errors,
    );
    check_type_for_param(
        store,
        m.target_ty,
        &format!("TypeTest.target_ty (block {block})"),
        errors,
    );
    check_type_for_param(
        store,
        m.descriptor.ty,
        &format!("TypeTest.descriptor.ty (block {block})"),
        errors,
    );
    match &m.parameterized {
        P::None => {}
        P::Nominal {
            type_args,
            effect_arg,
        } => {
            check_type_ids_for_param(
                store,
                type_args,
                &format!("TypeTest.parameterized.Nominal.type_args (block {block})"),
                errors,
            );
            if let Some(ea) = effect_arg {
                check_effect_row_for_param(
                    store,
                    ea,
                    &format!("TypeTest.parameterized.Nominal.effect_arg (block {block})"),
                    errors,
                );
            }
        }
        P::Function {
            receiver,
            params,
            return_ty,
            effects,
            ..
        } => {
            check_optional_type_for_param(
                store,
                *receiver,
                &format!("TypeTest.parameterized.Function.receiver (block {block})"),
                errors,
            );
            check_type_ids_for_param(
                store,
                params,
                &format!("TypeTest.parameterized.Function.params (block {block})"),
                errors,
            );
            check_type_for_param(
                store,
                *return_ty,
                &format!("TypeTest.parameterized.Function.return_ty (block {block})"),
                errors,
            );
            check_effect_row_for_param(
                store,
                effects,
                &format!("TypeTest.parameterized.Function.effects (block {block})"),
                errors,
            );
        }
        P::Option { payload_ty } => {
            check_type_for_param(
                store,
                *payload_ty,
                &format!("TypeTest.parameterized.Option.payload_ty (block {block})"),
                errors,
            );
        }
        P::Tuple { element_tys } => {
            check_type_ids_for_param(
                store,
                element_tys,
                &format!("TypeTest.parameterized.Tuple.element_tys (block {block})"),
                errors,
            );
        }
        P::Union { variants } => {
            check_type_ids_for_param(
                store,
                variants,
                &format!("TypeTest.parameterized.Union.variants (block {block})"),
                errors,
            );
        }
        P::StarProjection { read_ty } => {
            check_type_for_param(
                store,
                *read_ty,
                &format!("TypeTest.parameterized.StarProjection.read_ty (block {block})"),
                errors,
            );
        }
    }
}

fn verify_no_residue_pattern(
    pat: &crate::mir::Pattern,
    store: &TypeStore,
    block: BasicBlockId,
    errors: &mut Vec<VerifyError>,
) {
    use crate::mir::Pattern;
    match pat {
        Pattern::Wildcard
        | Pattern::IntLit(_)
        | Pattern::CharLit(_)
        | Pattern::StringLit(_)
        | Pattern::BoolLit(_) => {}
        Pattern::Bind { ty, .. } => {
            check_type_for_param(
                store,
                *ty,
                &format!("Pattern.Bind.ty (block {block})"),
                errors,
            );
        }
        Pattern::Is { ty, .. } => {
            check_type_for_param(
                store,
                *ty,
                &format!("Pattern.Is.ty (block {block})"),
                errors,
            );
        }
        Pattern::Tuple { elements } => {
            for e in elements {
                verify_no_residue_pattern(e, store, block, errors);
            }
        }
        Pattern::Struct { fields, .. } => {
            for f in fields {
                verify_no_residue_pattern(&f.pattern, store, block, errors);
            }
        }
        Pattern::Variant { args, .. } => {
            for a in args {
                verify_no_residue_pattern(a, store, block, errors);
            }
        }
        Pattern::Or { patterns } => {
            for p in patterns {
                verify_no_residue_pattern(p, store, block, errors);
            }
        }
    }
}

fn verify_no_residue_terminator(
    term: &crate::mir::Terminator,
    store: &TypeStore,
    block: BasicBlockId,
    errors: &mut Vec<VerifyError>,
) {
    match &term.kind {
        TerminatorKind::Perform { metadata, args, .. } => {
            check_type_for_param(
                store,
                metadata.effect_ty,
                &format!("Perform.effect_ty (block {block})"),
                errors,
            );
            check_type_for_param(
                store,
                metadata.result_ty,
                &format!("Perform.result_ty (block {block})"),
                errors,
            );
            check_type_ids_for_param(
                store,
                &metadata.op_type_args,
                &format!("Perform.op_type_args (block {block})"),
                errors,
            );
            check_optional_type_for_param(
                store,
                metadata.payload_tuple_ty,
                &format!("Perform.payload_tuple_ty (block {block})"),
                errors,
            );
            check_type_ids_for_param(
                store,
                &metadata.payload_component_tys,
                &format!("Perform.payload_component_tys (block {block})"),
                errors,
            );
            for a in args {
                check_type_for_param(
                    store,
                    a.value_ty,
                    &format!("Perform.arg.value_ty (block {block})"),
                    errors,
                );
            }
        }
        TerminatorKind::Handle { metadata, arms, .. } => {
            check_type_for_param(
                store,
                metadata.result_ty,
                &format!("Handle.result_ty (block {block})"),
                errors,
            );
            check_type_for_param(
                store,
                metadata.body_result_ty,
                &format!("Handle.body_result_ty (block {block})"),
                errors,
            );
            check_optional_type_for_param(
                store,
                metadata.finally_result_ty,
                &format!("Handle.finally_result_ty (block {block})"),
                errors,
            );
            for arm in arms {
                check_type_for_param(
                    store,
                    arm.handled_effect_ty,
                    &format!("HandleArm.handled_effect_ty (block {block})"),
                    errors,
                );
                check_type_for_param(
                    store,
                    arm.body_ty,
                    &format!("HandleArm.body_ty (block {block})"),
                    errors,
                );
                check_type_ids_for_param(
                    store,
                    &arm.op_type_args,
                    &format!("HandleArm.op_type_args (block {block})"),
                    errors,
                );
                check_optional_type_for_param(
                    store,
                    arm.payload_tuple_ty,
                    &format!("HandleArm.payload_tuple_ty (block {block})"),
                    errors,
                );
                check_type_ids_for_param(
                    store,
                    &arm.payload_component_tys,
                    &format!("HandleArm.payload_component_tys (block {block})"),
                    errors,
                );
            }
        }
        _ => {}
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
