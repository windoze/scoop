//! HIR → MIR lowering。
//!
//! 入口 [`lower_module`]：消费 `ast::File` + `scoop2_hir::TypedHir`，产出
//! [`crate::mir::Module`]。每个函数体由 [`FnLowering`] 构建器 lower 为
//! [`crate::mir::Body`]（locals + 基本块图）。
//!
//! 表达式 lowering 是 ANF 风格：每个表达式 lower 为一个临时 local 的赋值
//! （`StatementKind::Assign { target, value: Rvalue }`）。控制流（if/when/while/for/
//! break/continue/try/handle/perform）lower 为基本块图。
//!
//! 覆盖全部 33 `ExprKind` + 8 `StmtKind`，无 fallback：合法构造全部 lower；
//! 非法构造（如 `break` 出循环）报具体拒绝码（`scoop::mir::break_outside_loop` 等），
//! comptime 反射特性（splice field）报 `scoop::mir::splice_field_removed`。

pub mod builder;
pub mod expr;
pub mod stmt;
#[cfg(test)]
mod tests;

pub use builder::FnLowering;

use scoop2_base::diag::DiagnosticSink;
use scoop2_hir::hir::TypedHir;

use crate::diagnostics::MirLowerError;
use crate::mir::{Item, MetadataKind, MetadataRoot, Module};

/// lowering 结果：Module + 错误列表（错误已 push 进 sink）。
pub struct LowerResult {
    pub module: Module,
    pub errors: Vec<MirLowerError>,
}

/// 把一组用户文件 lower 为 MIR Module。
///
/// - `files`：与 typecheck 输入一致顺序的 (FileId, &ast::File)（仅 User 文件）。
/// - `hir`：typed HIR（含 expr_types 与语义事实侧表）。
/// - `diags`：诊断 sink（lowering 错误 push 进此）。
pub fn lower_module<'f>(
    files: impl Iterator<Item = (scoop2_base::FileId, &'f scoop2_syntax::ast::File)>,
    hir: &TypedHir,
    diags: &mut DiagnosticSink,
) -> LowerResult {
    let mut module = Module {
        items: Vec::new(),
        // lowering 过程中新 intern 的类型需要 store；从 hir 克隆（TypeStore 是 Clone 的）。
        types: hir.store.clone(),
    };
    let mut errors: Vec<MirLowerError> = Vec::new();
    for (file_id, file) in files {
        lower_file(file_id, file, hir, &mut module, &mut errors);
    }
    // 把错误 push 进 sink。
    for e in &errors {
        diags.push(e.to_diagnostic());
    }
    LowerResult { module, errors }
}

/// lower 单个文件的顶层 items。
fn lower_file(
    file_id: scoop2_base::FileId,
    file: &scoop2_syntax::ast::File,
    hir: &TypedHir,
    module: &mut Module,
    errors: &mut Vec<MirLowerError>,
) {
    use scoop2_syntax::ast::ItemKind;
    let package_prefix = hir
        .file(file_id)
        .map(|f| f.package_prefix.as_str())
        .unwrap_or("");
    let mut local_items: Vec<Item> = Vec::new();
    for item in &file.items {
        match &item.kind {
            ItemKind::Fun(d) => {
                let base = module.types.clone();
                let (fd, nested, fn_store) =
                    builder::lower_fun_decl(file_id, d, hir, package_prefix, &base, errors);
                if let Some(fd) = fd {
                    // 合并 per-function store 到 module.types，remap TypeId。
                    let remap = module.types.extend_from(&fn_store);
                    local_items.push(Item::Fun(remap_fun_decl(&remap, fd)));
                    // 嵌套闭包函数作为 sibling items（同样 remap）。
                    for nf in nested {
                        local_items.push(Item::Fun(remap_fun_decl(&remap, nf)));
                    }
                }
            }
            ItemKind::Val(d) => {
                let base = module.types.clone();
                let (ir_opt, val_store) =
                    builder::lower_top_level_val(file_id, d, hir, package_prefix, &base, errors);
                if let Some(ir) = ir_opt {
                    let remap = module.types.extend_from(&val_store);
                    local_items.push(remap_item(&remap, ir));
                }
            }
            ItemKind::Type(d) => {
                let fqn = fqn_of(package_prefix, d.name.symbol, hir);
                let owner_sym = hir.interner.get(&fqn).unwrap_or_default();
                let kind = match d.kind {
                    scoop2_syntax::ast::TypeKind::Class => MetadataKind::Class,
                    scoop2_syntax::ast::TypeKind::Interface => MetadataKind::Interface,
                    scoop2_syntax::ast::TypeKind::Struct => MetadataKind::Struct,
                    scoop2_syntax::ast::TypeKind::Enum => MetadataKind::Enum,
                    scoop2_syntax::ast::TypeKind::Effect => MetadataKind::Effect,
                };
                local_items.push(Item::Metadata(MetadataRoot {
                    span: d.name.span,
                    fqn,
                    kind,
                    file: file_id,
                }));
                // 类型体的成员函数也需 lower（method bodies）。
                if let Some(body) = &d.body {
                    let base = module.types.clone();
                    let member_items = builder::lower_type_member_funs_with_stores(
                        file_id,
                        &body.members,
                        Some(d),
                        hir,
                        package_prefix,
                        &base,
                        errors,
                        owner_sym,
                    );
                    for (it, st) in member_items {
                        let remap = module.types.extend_from(&st);
                        local_items.push(remap_item(&remap, it));
                    }
                }
                // class 初始化 callable（`<Class>.$init`）：仅当类有 init 块 / 属性初始化器 /
                // 超类委托时合成。codegen 在 scoop_alloc_typed 之后调用它，执行 Kotlin 顺序
                // 的 super 委托 / 属性参数赋值 / 属性初始化器 / init 块。
                if let Some((init_fd, nested, init_store)) = builder::lower_class_init_callable(
                    file_id,
                    d,
                    hir,
                    package_prefix,
                    &module.types.clone(),
                    errors,
                    owner_sym,
                ) {
                    let remap = module.types.extend_from(&init_store);
                    local_items.push(remap_item(&remap, Item::Fun(init_fd)));
                    for nf in nested {
                        local_items.push(remap_item(&remap, Item::Fun(nf)));
                    }
                }
            }
            ItemKind::Object(d) => {
                if let Some(name) = &d.name {
                    let fqn = fqn_of(package_prefix, name.symbol, hir);
                    local_items.push(Item::Metadata(MetadataRoot {
                        span: name.span,
                        fqn,
                        kind: MetadataKind::Object,
                        file: file_id,
                    }));
                }
                if let Some(body) = &d.body {
                    let base = module.types.clone();
                    let owner_sym = d
                        .name
                        .and_then(|n| {
                            let fqn = fqn_of(package_prefix, n.symbol, hir);
                            hir.interner.get(&fqn)
                        })
                        .unwrap_or_default();
                    let member_items = builder::lower_type_member_funs_with_stores(
                        file_id,
                        &body.members,
                        None,
                        hir,
                        package_prefix,
                        &base,
                        errors,
                        owner_sym,
                    );
                    for (it, st) in member_items {
                        let remap = module.types.extend_from(&st);
                        local_items.push(remap_item(&remap, it));
                    }
                }
            }
            ItemKind::ExtensionProperty(_) | ItemKind::TypeAlias(_) => {
                // 扩展属性 / typealias：MIR 不单独建模（成员访问决议已在 HIR 侧表）。
            }
        }
    }
    module.items.extend(local_items);
}

/// 用 remap 表重写 FunDecl 中的所有 TypeId。
fn remap_fun_decl(
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
fn remap_item(
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

/// 构造 FQN 文本。
pub(crate) fn fqn_of(prefix: &str, simple: scoop2_base::Symbol, hir: &TypedHir) -> String {
    let name = hir.interner.resolve(simple);
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{}.{}", prefix, name)
    }
}
