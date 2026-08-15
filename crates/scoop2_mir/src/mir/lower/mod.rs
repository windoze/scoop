//! MIR lowering 基建（M2-5 翻转后）。
//!
//! AST lowering 路径已删除（双路径 oracle 325/325 字节一致后退役）——
//! 唯一入口是 [`crate::mir::lower_tree::lower_module_from_trees`]（HIR 树 +
//! item 骨架驱动）。本模块保留 [`FnLowering`] 构建器（树路径复用的机器）
//! 与模块合并的 remap 助手、[`LowerResult`]。

pub mod builder;

pub use builder::FnLowering;

use crate::diagnostics::MirLowerError;
use crate::mir::{Item, Module};

/// lowering 结果：Module + 错误列表。
pub struct LowerResult {
    pub module: Module,
    pub errors: Vec<MirLowerError>,
}

/// 用 remap 表重写 FunDecl 中的所有 TypeId。
pub(crate) fn remap_fun_decl(
    remap: &std::collections::HashMap<scoop2_hir::ty::TypeId, scoop2_hir::ty::TypeId>,
    mut fd: crate::mir::FunDecl,
) -> crate::mir::FunDecl {
    use scoop2_hir::ty::{TypeId, TypeStore};
    fd.ty = TypeStore::remap_id(remap, fd.ty);
    fd.return_ty = TypeStore::remap_id(remap, fd.return_ty);
    for p in &mut fd.params {
        p.ty = TypeStore::remap_id(remap, p.ty);
    }
    if let Some(body) = fd.body.take() {
        fd.body = Some(remap_body(remap, body));
    }
    fd
}

/// 用 remap 表重写任意 Item 中的 TypeId（Fun / Initializer）。
pub(crate) fn remap_item(
    remap: &std::collections::HashMap<scoop2_hir::ty::TypeId, scoop2_hir::ty::TypeId>,
    item: Item,
) -> Item {
    use scoop2_hir::ty::TypeStore;
    match item {
        Item::Fun(fd) => Item::Fun(remap_fun_decl(remap, fd)),
        Item::Initializer(mut ir) => {
            ir.ty = TypeStore::remap_id(remap, ir.ty);
            ir.body = remap_body(remap, ir.body);
            Item::Initializer(ir)
        }
        Item::ExternGlobal(mut g) => {
            g.ty = TypeStore::remap_id(remap, g.ty);
            Item::ExternGlobal(g)
        }
        other => other,
    }
}

/// 用 remap 表重写 Body 中所有 local 的 TypeId（statements 中的 TypeId 也重写）。
fn remap_body(
    remap: &std::collections::HashMap<scoop2_hir::ty::TypeId, scoop2_hir::ty::TypeId>,
    mut body: crate::mir::Body,
) -> crate::mir::Body {
    use scoop2_hir::ty::{TypeId, TypeStore};
    for decl in &mut body.locals {
        decl.ty = TypeStore::remap_id(remap, decl.ty);
    }
    for block in &mut body.blocks {
        for stmt in &mut block.stmts {
            remap_statement(remap, stmt);
        }
        remap_terminator(remap, &mut block.terminator);
    }
    body
}

fn remap_statement(
    remap: &std::collections::HashMap<scoop2_hir::ty::TypeId, scoop2_hir::ty::TypeId>,
    stmt: &mut crate::mir::Statement,
) {
    use crate::mir::StatementKind;
    use scoop2_hir::ty::TypeStore;
    match &mut stmt.kind {
        StatementKind::Assign { value, .. } => remap_rvalue(remap, value),
        StatementKind::StoreMember { value_ty, .. } => {
            *value_ty = TypeStore::remap_id(remap, *value_ty);
        }
        StatementKind::StoreTupleIndex { value_ty, .. }
        | StatementKind::StoreTopLevelVar { value_ty, .. } => {
            *value_ty = TypeStore::remap_id(remap, *value_ty);
        }
        _ => {}
    }
}

fn remap_rvalue(
    remap: &std::collections::HashMap<scoop2_hir::ty::TypeId, scoop2_hir::ty::TypeId>,
    rv: &mut crate::mir::Rvalue,
) {
    use crate::mir::Rvalue;
    use scoop2_hir::ty::TypeStore;
    match rv {
        Rvalue::Use(_)
        | Rvalue::UnresolvedName { .. }
        | Rvalue::InterpolatedString { .. }
        | Rvalue::ClassLit { .. }
        | Rvalue::PatternMatch { .. }
        | Rvalue::PatternExtract { .. }
        | Rvalue::MakeContinuation { .. }
        | Rvalue::MakeChainLink { .. }
        | Rvalue::IntEq { .. } => {}
        Rvalue::TopLevelRef(tl) => {
            for t in &mut tl.generic_type_args {
                *t = TypeStore::remap_id(remap, *t);
            }
        }
        Rvalue::TypeTest { metadata, .. } => {
            metadata.descriptor.ty = TypeStore::remap_id(remap, metadata.descriptor.ty);
            metadata.target_ty = TypeStore::remap_id(remap, metadata.target_ty);
            metadata.source_ty = TypeStore::remap_id(remap, metadata.source_ty);
        }
        Rvalue::Cast { metadata, .. } => {
            metadata.test.source_ty = TypeStore::remap_id(remap, metadata.test.source_ty);
            metadata.test.target_ty = TypeStore::remap_id(remap, metadata.test.target_ty);
        }
        Rvalue::MemberAccess { member, .. } => {
            member.receiver_ty = TypeStore::remap_id(remap, member.receiver_ty);
        }
        Rvalue::TupleIndex { element_ty, .. } => {
            *element_ty = TypeStore::remap_id(remap, *element_ty);
        }
        Rvalue::IndexAccess {
            element_ty,
            receiver_ty,
            ..
        } => {
            *element_ty = TypeStore::remap_id(remap, *element_ty);
            *receiver_ty = TypeStore::remap_id(remap, *receiver_ty);
        }
        Rvalue::EnumVariant {
            enum_ty, payload, ..
        } => {
            *enum_ty = TypeStore::remap_id(remap, *enum_ty);
            payload.aggregate_ty = TypeStore::remap_id(remap, payload.aggregate_ty);
        }
        Rvalue::ClassCtor { .. } => {}
        Rvalue::Call {
            kind, transport, ..
        } => {
            remap_call_kind(remap, kind);
            transport.result.source_ty = TypeStore::remap_id(remap, transport.result.source_ty);
        }
        Rvalue::MakeTuple { transport, .. } | Rvalue::StructLit { transport, .. } => {
            transport.aggregate_ty = TypeStore::remap_id(remap, transport.aggregate_ty);
        }
        Rvalue::MakeArray { result_ty, .. } | Rvalue::WithUpdate { result_ty, .. } => {
            *result_ty = TypeStore::remap_id(remap, *result_ty);
        }
        Rvalue::MakeClosure { env_contract, .. } => {
            env_contract.env_ty = TypeStore::remap_id(remap, env_contract.env_ty);
        }
        Rvalue::PerformResult { result_ty, .. } => {
            *result_ty = TypeStore::remap_id(remap, *result_ty);
        }
        Rvalue::TakeChainLink { result_ty } | Rvalue::ResumeChainLink { result_ty, .. } => {
            *result_ty = TypeStore::remap_id(remap, *result_ty);
        }
    }
}

fn remap_call_kind(
    remap: &std::collections::HashMap<scoop2_hir::ty::TypeId, scoop2_hir::ty::TypeId>,
    kind: &mut crate::mir::CallKind,
) {
    use crate::mir::CallKind;
    use scoop2_hir::ty::TypeStore;
    match kind {
        CallKind::Direct {
            generic_type_args, ..
        } => {
            for t in generic_type_args {
                *t = TypeStore::remap_id(remap, *t);
            }
        }
        CallKind::Virtual { dispatch, .. } | CallKind::Interface { dispatch, .. } => {
            dispatch.receiver_ty = TypeStore::remap_id(remap, dispatch.receiver_ty);
            for t in &mut dispatch.generic_type_args {
                *t = TypeStore::remap_id(remap, *t);
            }
        }
        CallKind::Closure { .. } | CallKind::FunValue { .. } | CallKind::Resume { .. } => {}
    }
}

fn remap_terminator(
    remap: &std::collections::HashMap<scoop2_hir::ty::TypeId, scoop2_hir::ty::TypeId>,
    term: &mut crate::mir::Terminator,
) {
    use scoop2_hir::ty::TypeStore;
    match &mut term.kind {
        crate::mir::TerminatorKind::Perform { metadata, args, .. } => {
            metadata.effect_ty = TypeStore::remap_id(remap, metadata.effect_ty);
            metadata.result_ty = TypeStore::remap_id(remap, metadata.result_ty);
            for a in args.iter_mut() {
                a.value_ty = TypeStore::remap_id(remap, a.value_ty);
            }
        }
        crate::mir::TerminatorKind::Handle { metadata, arms, .. } => {
            metadata.result_ty = TypeStore::remap_id(remap, metadata.result_ty);
            metadata.body_result_ty = TypeStore::remap_id(remap, metadata.body_result_ty);
            for arm in arms.iter_mut() {
                arm.handled_effect_ty = TypeStore::remap_id(remap, arm.handled_effect_ty);
                arm.body_ty = TypeStore::remap_id(remap, arm.body_ty);
            }
        }
        _ => {}
    }
}

