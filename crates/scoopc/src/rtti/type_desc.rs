//! 运行期 type descriptor（`ScoopTypeDescriptor`）的可观测导出（TODO T1507b）。
//!
//! 用途：
//! - `scoop dump-rtti`：调试 GC heap trace（trace bitmap/trace_fn）以及后续 RTTI/type test（`is/as/as?`）。
//! - 当前阶段只输出“编译器侧可静态确定”的 descriptor 信息，不从二进制中反射读取。
//!
//! 约束（v0）：
//! - 仅覆盖：class / closure env / string / box /（可选）runtime builtin array descriptors；
//! - trace bitmap 的计算为 early-stage 版本：以字段顺序 + host pointer layout 推导 slot 索引；
//! - 该输出用于调试与回归，不承诺与未来跨平台 target layout 完全一致（T0803 再对齐）。

use std::collections::{HashMap, HashSet};

use miette::Diagnostic;
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::ast;
use crate::hir;
use crate::parser::ParseError;
use crate::resolve::{Index, ResolveError};
use crate::session::Session;
use crate::source::SourceFile;
use crate::ty::layout::{NicheDomain, NicheStorage, TargetLayout, TypeLayout};
use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

#[derive(Debug, Clone, Serialize)]
pub struct TargetLayoutInfo {
    pub pointer_size: u64,
    pub pointer_align: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TypeDescDump {
    pub target: TargetLayoutInfo,
    /// 按 `name` 排序，保证输出稳定。
    pub types: Vec<TypeDesc>,
    /// interface 元数据（TODO T1507c1）：稳定 interface_id 与 method slots。
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub interfaces: Vec<InterfaceDesc>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TypeDescKind {
    Builtin,
    RuntimeBuiltin,
    Class,
    ClosureObject,
    ClosureEnv,
}

#[derive(Debug, Clone, Serialize)]
pub struct TypeDesc {
    /// descriptor 的 canonical name（用于计算 stable `type_id`）。
    pub name: String,
    pub kind: TypeDescKind,
    pub type_id: u64,

    /// 仅对 class：直接父类（superclass）的 canonical name。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,

    /// 仅对 class：从根到当前类型（含自身）的继承链（best-effort）。
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub parent_chain: Vec<String>,

    /// trace bitmap 的起始偏移（单位：字节）。
    pub trace_start_offset_bytes: u64,
    /// trace bitmap（以 `u64` words 表示；bit i 表示第 i 个 word slot 为 GC pointer）。
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub trace_bitmap_u64: Vec<u64>,
    /// 若该类型用 `trace_fn` 扫描，则这里记录一个可读标识（通常为 runtime C 函数名）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_fn: Option<String>,
}

/// interface 的稳定可观测导出（TODO T1507c1）。
///
/// 注意：interface 本身不是 heap object，因此这里并不复用 `TypeDesc`（`ScoopTypeDescriptor`）。
#[derive(Debug, Clone, Serialize)]
pub struct InterfaceDesc {
    /// interface 的 canonical name（FQN）。
    pub name: String,
    /// 全局稳定 interface id（v0：hash64(name)）。
    pub interface_id: u64,
    /// 直接 super interfaces（best-effort：仅当 `TypeRef::Path` 可解析）。
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub super_interfaces: Vec<String>,
    /// method slots（v0：按声明顺序分配 slot index）。
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub method_slots: Vec<InterfaceMethodSlot>,
}

/// interface method 的 slot 信息（TODO T1507c1）。
#[derive(Debug, Clone, Serialize)]
pub struct InterfaceMethodSlot {
    pub slot: u32,
    pub name: String,
    pub params_len: u32,
    pub has_receiver: bool,
}

#[derive(Debug, Error, Diagnostic)]
pub enum TypeDescError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    HirLower(#[from] hir::HirLowerError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Resolve(#[from] ResolveError),

    #[error("class 继承链存在环：{fqn}")]
    #[diagnostic(code(scoop::rtti::type_desc_inheritance_cycle))]
    InheritanceCycle { fqn: String },
}

/// 从单个输入文件生成 type descriptor dump（供 `scoop dump-rtti` 使用）。
pub fn dump_file_type_desc(
    session: &Session,
    source: &SourceFile,
) -> Result<TypeDescDump, TypeDescError> {
    let lowered = hir::lower_for_dump(session, source)?;
    let target = TargetLayout::host();

    let mut out: Vec<TypeDesc> = Vec::new();
    out.extend(builtin_type_descs(target));

    // class type descriptors（按 class FQN 稳定排序）。
    let mut class_fqns: Vec<String> = lowered.class_inits.keys().cloned().collect();
    class_fqns.sort();
    for fqn in class_fqns {
        if let Some(desc) = class_type_desc(target, &lowered.types, &lowered.class_inits, &fqn)? {
            out.push(desc);
        }
    }

    // closure env descriptors：只为“有 captures 的 closure”生成（无 captures 时 env 不会被分配）。
    let symbol_tys = collect_symbol_types(&lowered.file);
    let fallback_any = builtin_any_type_id(&lowered.types);
    let fallback_unit = builtin_unit_type_id(&lowered.types);
    let mut closures: Vec<hir::ClosureExpr> = Vec::new();
    collect_closures_in_file(&lowered.file, &mut closures);
    for c in closures {
        if c.captures.is_empty() {
            continue;
        }

        let mut capture_tys: Vec<TypeId> = Vec::with_capacity(c.captures.len());
        for cap in &c.captures {
            if let Some(ty) = symbol_tys.get(&cap.id).copied() {
                capture_tys.push(ty);
            } else {
                // best-effort：缺失类型信息时，按 `Any`（ref）对待以避免静默漏报（更利于调试）。
                //
                // 注意：这里不能 `intern` 新类型（会破坏 dump 输出稳定性），因此仅复用已存在的 builtin `Any`。
                if let Some(any) = fallback_any {
                    capture_tys.push(any);
                } else if let Some(unit) = fallback_unit {
                    capture_tys.push(unit);
                } else {
                    // 兜底：理论上不会发生（lowering 必然 intern builtins）。
                    continue;
                }
            }
        }

        let (trace_start, bitmap) =
            trace_bitmap_for_payload_fields(target, &lowered.types, &capture_tys);
        out.push(TypeDesc {
            name: format!("scoop.lambda_env${}", c.id.as_u32()),
            kind: TypeDescKind::ClosureEnv,
            type_id: stable_hash64(&format!("scoop.lambda_env${}", c.id.as_u32())),
            parent: None,
            parent_chain: Vec::new(),
            trace_start_offset_bytes: trace_start,
            trace_bitmap_u64: bitmap,
            trace_fn: None,
        });
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    let interfaces = collect_interface_descs(session, source)?;
    Ok(TypeDescDump {
        target: TargetLayoutInfo {
            pointer_size: target.pointer_size,
            pointer_align: target.pointer_align,
        },
        types: out,
        interfaces,
    })
}

fn collect_interface_descs(
    session: &Session,
    source: &SourceFile,
) -> Result<Vec<InterfaceDesc>, TypeDescError> {
    // 说明：
    // - 这里刻意不复用 `hir::lower_for_dump` 内部构建的 index/AST，避免把内部实现细节泄漏到 API。
    // - interface slot table 的提取只依赖声明头与成员列表，不依赖 body 的 resolver 注入信息。
    let ast = session.parse(source)?;

    let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        pairs.push((&f.source, &f.ast));
    }
    pairs.push((source, &ast));

    let index = Index::build(&pairs)?;

    let mut out: Vec<InterfaceDesc> = Vec::new();
    for (src, file) in &pairs {
        let pkg_prefix = package_prefix(src, file.package.as_ref());
        for item in &file.items {
            match item {
                ast::Item::Type(ty) => {
                    collect_interfaces_in_type_decl(src, file, &pkg_prefix, ty, &index, &mut out);
                }
                ast::Item::Object(obj) => {
                    collect_interfaces_in_object_decl(
                        src,
                        file,
                        &pkg_prefix,
                        obj,
                        &index,
                        &mut out,
                    );
                }
                ast::Item::Fun(_)
                | ast::Item::Val(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::TypeAlias(_) => {}
            }
        }
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn collect_interfaces_in_type_decl(
    source: &SourceFile,
    file: &ast::File,
    owner_prefix: &str,
    decl: &ast::TypeDecl,
    index: &Index,
    out: &mut Vec<InterfaceDesc>,
) {
    let name = decl.name.text(source).to_string();
    let type_fqn = join_prefix(owner_prefix, &name);

    if matches!(decl.kind, ast::TypeKind::Interface) {
        let super_interfaces = decl
            .supertypes
            .iter()
            .filter(|st| st.ctor_args_span.is_none())
            .filter_map(|st| index.type_ref_to_fqn_in_file(source, file, &st.ty))
            .collect::<Vec<_>>();

        let mut method_slots: Vec<InterfaceMethodSlot> = Vec::new();
        if let Some(body) = &decl.body {
            let mut slot = 0u32;
            for member in &body.members {
                let ast::TypeMember::Fun(fun) = member else {
                    continue;
                };
                method_slots.push(InterfaceMethodSlot {
                    slot,
                    name: fun.name.text(source).to_string(),
                    params_len: fun.params.len() as u32,
                    has_receiver: fun.receiver.is_some(),
                });
                slot = slot.saturating_add(1);
            }
        }

        out.push(InterfaceDesc {
            name: type_fqn.clone(),
            interface_id: stable_hash64(&type_fqn),
            super_interfaces,
            method_slots,
        });
    }

    let Some(body) = &decl.body else {
        return;
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_interfaces_in_type_decl(source, file, &type_fqn, nested, index, out);
            }
            ast::TypeMember::Object(obj) => {
                collect_interfaces_in_object_decl(source, file, &type_fqn, obj, index, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

fn collect_interfaces_in_object_decl(
    source: &SourceFile,
    file: &ast::File,
    owner_prefix: &str,
    obj: &ast::ObjectDecl,
    index: &Index,
    out: &mut Vec<InterfaceDesc>,
) {
    let Some(name) = object_decl_name(source, obj) else {
        return;
    };
    let obj_fqn = join_prefix(owner_prefix, &name);

    let Some(body) = &obj.body else {
        return;
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_interfaces_in_type_decl(source, file, &obj_fqn, nested, index, out);
            }
            ast::TypeMember::Object(nested) => {
                collect_interfaces_in_object_decl(source, file, &obj_fqn, nested, index, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

fn object_decl_name(source: &SourceFile, obj: &ast::ObjectDecl) -> Option<String> {
    match obj.name.as_ref() {
        Some(name) => Some(name.text(source).to_string()),
        None => match obj.kind {
            ast::ObjectKind::Companion => Some("Companion".to_string()),
            ast::ObjectKind::Object => None,
        },
    }
}

fn join_prefix(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}

fn builtin_any_type_id(types: &TypeStore) -> Option<TypeId> {
    types
        .iter_ids()
        .find(|id| matches!(types.kind(*id), TypeKind::Ref(RefTypeKind::Any)))
}

fn builtin_unit_type_id(types: &TypeStore) -> Option<TypeId> {
    types
        .iter_ids()
        .find(|id| matches!(types.kind(*id), TypeKind::Value(ValueTypeKind::Unit)))
}

fn package_prefix(source: &SourceFile, pkg: Option<&ast::PackageDecl>) -> String {
    let Some(pkg) = pkg else {
        return String::new();
    };
    pkg.path
        .iter()
        .map(|id| source.slice(id.span))
        .collect::<Vec<_>>()
        .join(".")
}

fn builtin_type_descs(target: TargetLayout) -> Vec<TypeDesc> {
    let header_size = gc_object_header_size(target);

    // 说明：builtin/runtime 类型的字段布局在 early stage 固定（见 `crates/scoopc/src/llvm/codegen.rs`）。
    let mut out = Vec::new();

    // `scoop.core.String`：无 GC pointer 字段（trace bitmap 为空）。
    out.push(TypeDesc {
        name: "scoop.core.String".to_string(),
        kind: TypeDescKind::Builtin,
        type_id: stable_hash64("scoop.core.String"),
        parent: None,
        parent_chain: Vec::new(),
        trace_start_offset_bytes: header_size,
        trace_bitmap_u64: Vec::new(),
        trace_fn: None,
    });

    // `scoop.runtime.BoxedUnit`：仅对象头。
    out.push(TypeDesc {
        name: "scoop.runtime.BoxedUnit".to_string(),
        kind: TypeDescKind::Builtin,
        type_id: stable_hash64("scoop.runtime.BoxedUnit"),
        parent: None,
        parent_chain: Vec::new(),
        trace_start_offset_bytes: 0,
        trace_bitmap_u64: Vec::new(),
        trace_fn: None,
    });

    // `scoop.runtime.BoxedInt{bits}_{i|u}`：对象头 + 整数 payload，无引用字段。
    let word_bits = target.pointer_size.saturating_mul(8).clamp(8, 1024);
    for (signed, suffix) in [(true, "i"), (false, "u")] {
        let name = format!("scoop.runtime.BoxedInt{word_bits}_{suffix}");
        out.push(TypeDesc {
            name: name.clone(),
            kind: TypeDescKind::Builtin,
            type_id: stable_hash64(&name),
            parent: None,
            parent_chain: Vec::new(),
            trace_start_offset_bytes: header_size,
            trace_bitmap_u64: Vec::new(),
            trace_fn: None,
        });

        let _ = signed; // 保留 signed 变量，便于未来扩展其它位宽 box 时复用。
    }

    // `scoop.runtime.ScoopClosure`：payload `{ env_ptr(gc), fn_ptr(native) }`。
    // trace bitmap：slot0=env_ptr。
    out.push(TypeDesc {
        name: "scoop.runtime.ScoopClosure".to_string(),
        kind: TypeDescKind::ClosureObject,
        type_id: stable_hash64("scoop.runtime.ScoopClosure"),
        parent: None,
        parent_chain: Vec::new(),
        trace_start_offset_bytes: header_size,
        trace_bitmap_u64: vec![1u64],
        trace_fn: None,
    });

    // runtime builtin array descriptors（type_id=0；由 runtime C 定义并写入对象头）。
    out.push(TypeDesc {
        name: "runtime.builtin.SCOOP_ARRAY_WORD_TYPE_DESC".to_string(),
        kind: TypeDescKind::RuntimeBuiltin,
        type_id: 0,
        parent: None,
        parent_chain: Vec::new(),
        trace_start_offset_bytes: 0,
        trace_bitmap_u64: Vec::new(),
        trace_fn: None,
    });
    out.push(TypeDesc {
        name: "runtime.builtin.SCOOP_ARRAY_REF_TYPE_DESC".to_string(),
        kind: TypeDescKind::RuntimeBuiltin,
        type_id: 0,
        parent: None,
        parent_chain: Vec::new(),
        trace_start_offset_bytes: 0,
        trace_bitmap_u64: Vec::new(),
        trace_fn: Some("scoop_array_trace_ref_elems".to_string()),
    });
    out.push(TypeDesc {
        name: "runtime.builtin.SCOOP_ARRAY_BUILDER_WORD_TYPE_DESC".to_string(),
        kind: TypeDescKind::RuntimeBuiltin,
        type_id: 0,
        parent: None,
        parent_chain: Vec::new(),
        trace_start_offset_bytes: 0,
        trace_bitmap_u64: Vec::new(),
        trace_fn: None,
    });
    out.push(TypeDesc {
        name: "runtime.builtin.SCOOP_ARRAY_BUILDER_REF_TYPE_DESC".to_string(),
        kind: TypeDescKind::RuntimeBuiltin,
        type_id: 0,
        parent: None,
        parent_chain: Vec::new(),
        trace_start_offset_bytes: 0,
        trace_bitmap_u64: Vec::new(),
        trace_fn: Some("scoop_array_builder_trace_ref_elems".to_string()),
    });

    out
}

fn class_type_desc(
    target: TargetLayout,
    types: &TypeStore,
    class_inits: &hir::ClassInitIndex,
    class_fqn: &str,
) -> Result<Option<TypeDesc>, TypeDescError> {
    let Some(base) = class_inits.get(class_fqn) else {
        return Ok(None);
    };

    let mut visiting: HashSet<String> = HashSet::new();
    let (fields, chain) = flatten_class_fields(class_fqn, class_inits, &mut visiting)?;
    let parent = base.super_class_fqn.clone();

    let payload_tys: Vec<TypeId> = fields.iter().map(|f| f.ty).collect();
    let (trace_start, bitmap) = trace_bitmap_for_payload_fields(target, types, &payload_tys);

    Ok(Some(TypeDesc {
        name: base.fqn.clone(),
        kind: TypeDescKind::Class,
        type_id: stable_hash64(&base.fqn),
        parent,
        parent_chain: chain,
        trace_start_offset_bytes: trace_start,
        trace_bitmap_u64: bitmap,
        trace_fn: None,
    }))
}

fn flatten_class_fields(
    class_fqn: &str,
    class_inits: &hir::ClassInitIndex,
    visiting: &mut HashSet<String>,
) -> Result<(Vec<hir::ClassField>, Vec<String>), TypeDescError> {
    if !visiting.insert(class_fqn.to_string()) {
        return Err(TypeDescError::InheritanceCycle {
            fqn: class_fqn.to_string(),
        });
    }

    let Some(base) = class_inits.get(class_fqn) else {
        // best-effort：外部 class 不在当前 compilation unit 的 class_inits 中时，返回空字段与只含自身的链。
        let _ = visiting.remove(class_fqn);
        return Ok((Vec::new(), vec![class_fqn.to_string()]));
    };

    let mut fields: Vec<hir::ClassField> = Vec::new();
    let mut chain: Vec<String> = Vec::new();

    if let Some(super_fqn) = base.super_class_fqn.as_deref() {
        if class_inits.contains_key(super_fqn) {
            let (super_fields, mut super_chain) =
                flatten_class_fields(super_fqn, class_inits, visiting)?;
            fields.extend(super_fields);
            chain.append(&mut super_chain);
        } else {
            // best-effort：父类不在当前文件索引里，仅保留名字用于调试输出。
            chain.push(super_fqn.to_string());
        }
    }

    fields.extend(base.fields.clone());
    chain.push(base.fqn.clone());

    let _ = visiting.remove(class_fqn);
    Ok((fields, chain))
}

fn collect_symbol_types(file: &hir::File) -> HashMap<hir::SymbolId, TypeId> {
    let mut out: HashMap<hir::SymbolId, TypeId> = HashMap::new();

    for item in &file.items {
        match item {
            hir::Item::Fun(fun) => {
                for p in &fun.params {
                    out.insert(p.id, p.ty);
                }
                if let Some(body) = fun.body.as_ref() {
                    collect_symbol_types_in_block(body, &mut out);
                }
            }
            hir::Item::Val(v) => {
                if let Some(id) = v.id {
                    out.insert(id, v.ty);
                }
                if let Some(init) = v.init.as_ref() {
                    collect_symbol_types_in_expr(init, &mut out);
                }
            }
            hir::Item::Todo { .. } => {}
        }
    }

    out
}

fn collect_symbol_types_in_block(block: &hir::Block, out: &mut HashMap<hir::SymbolId, TypeId>) {
    for stmt in &block.stmts {
        collect_symbol_types_in_stmt(stmt, out);
    }
}

fn collect_symbol_types_in_stmt(stmt: &hir::Stmt, out: &mut HashMap<hir::SymbolId, TypeId>) {
    match &stmt.kind {
        hir::StmtKind::Empty => {}
        hir::StmtKind::Expr(e) => collect_symbol_types_in_expr(e, out),
        hir::StmtKind::Val(v) => {
            if let Some(id) = v.id {
                out.insert(id, v.ty);
            }
            if let Some(init) = v.init.as_ref() {
                collect_symbol_types_in_expr(init, out);
            }
        }
        hir::StmtKind::Assign { lhs, rhs, .. } => {
            collect_symbol_types_in_expr(lhs, out);
            collect_symbol_types_in_expr(rhs, out);
        }
        hir::StmtKind::While { cond, body } => {
            collect_symbol_types_in_expr(cond, out);
            collect_symbol_types_in_block(body, out);
        }
        hir::StmtKind::Return { value } => {
            if let Some(v) = value.as_ref() {
                collect_symbol_types_in_expr(v, out);
            }
        }
        hir::StmtKind::Break { .. } | hir::StmtKind::Continue { .. } | hir::StmtKind::Todo(_) => {}
    }
}

fn collect_symbol_types_in_expr(expr: &hir::Expr, out: &mut HashMap<hir::SymbolId, TypeId>) {
    match &expr.kind {
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::VarRef(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::Todo(_) => {}
        hir::ExprKind::StructLit { fields, .. } => {
            for f in fields {
                collect_symbol_types_in_expr(&f.value, out);
            }
        }
        hir::ExprKind::TupleLit { elements } => {
            for e in elements {
                collect_symbol_types_in_expr(e, out);
            }
        }
        hir::ExprKind::InterpolatedString { parts, .. } => {
            for p in parts {
                if let hir::InterpolatedStringPart::Expr { expr } = p {
                    collect_symbol_types_in_expr(expr, out);
                }
            }
        }
        hir::ExprKind::Unary { expr, .. } => collect_symbol_types_in_expr(expr, out),
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            collect_symbol_types_in_expr(lhs, out);
            collect_symbol_types_in_expr(rhs, out);
        }
        hir::ExprKind::Block(b) => collect_symbol_types_in_block(b, out),
        hir::ExprKind::Call { callee, args } => {
            collect_symbol_types_in_expr(callee, out);
            for arg in args {
                match arg {
                    hir::CallArg::Positional(e) => collect_symbol_types_in_expr(e, out),
                    hir::CallArg::Named { value, .. } => collect_symbol_types_in_expr(value, out),
                }
            }
        }
        hir::ExprKind::Closure(c) => {
            // closure params 也会占用 SymbolId；记录下来便于后续 capture type best-effort。
            for p in &c.params {
                out.insert(p.id, p.ty);
            }
            collect_symbol_types_in_expr(&c.body, out);
        }
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_symbol_types_in_expr(cond, out);
            collect_symbol_types_in_expr(then_branch, out);
            if let Some(e) = else_branch.as_ref() {
                collect_symbol_types_in_expr(e, out);
            }
        }
        hir::ExprKind::When { subject, arms } => {
            collect_symbol_types_in_expr(subject, out);
            for arm in arms {
                if let Some(guard) = arm.guard.as_ref() {
                    collect_symbol_types_in_expr(guard, out);
                }
                collect_symbol_types_in_expr(&arm.body, out);
            }
        }
        hir::ExprKind::MemberAccess { receiver, .. } => collect_symbol_types_in_expr(receiver, out),
        hir::ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    hir::CallArg::Positional(e) => collect_symbol_types_in_expr(e, out),
                    hir::CallArg::Named { value, .. } => collect_symbol_types_in_expr(value, out),
                }
            }
        }
        hir::ExprKind::Handle(h) => {
            collect_symbol_types_in_block(&h.body, out);
            for arm in &h.arms {
                collect_symbol_types_in_expr(&arm.body, out);
            }
            if let Some(finally) = h.finally.as_ref() {
                collect_symbol_types_in_block(finally, out);
            }
        }
    }
}

fn collect_closures_in_file(file: &hir::File, out: &mut Vec<hir::ClosureExpr>) {
    for item in &file.items {
        match item {
            hir::Item::Fun(fun) => {
                if let Some(body) = fun.body.as_ref() {
                    collect_closures_in_block(body, out);
                }
            }
            hir::Item::Val(v) => {
                if let Some(init) = v.init.as_ref() {
                    collect_closures_in_expr(init, out);
                }
            }
            hir::Item::Todo { .. } => {}
        }
    }
}

fn collect_closures_in_block(block: &hir::Block, out: &mut Vec<hir::ClosureExpr>) {
    for stmt in &block.stmts {
        collect_closures_in_stmt(stmt, out);
    }
}

fn collect_closures_in_stmt(stmt: &hir::Stmt, out: &mut Vec<hir::ClosureExpr>) {
    match &stmt.kind {
        hir::StmtKind::Empty => {}
        hir::StmtKind::Expr(e) => collect_closures_in_expr(e, out),
        hir::StmtKind::Val(v) => {
            if let Some(init) = v.init.as_ref() {
                collect_closures_in_expr(init, out);
            }
        }
        hir::StmtKind::Assign { lhs, rhs, .. } => {
            collect_closures_in_expr(lhs, out);
            collect_closures_in_expr(rhs, out);
        }
        hir::StmtKind::While { cond, body } => {
            collect_closures_in_expr(cond, out);
            collect_closures_in_block(body, out);
        }
        hir::StmtKind::Return { value } => {
            if let Some(v) = value.as_ref() {
                collect_closures_in_expr(v, out);
            }
        }
        hir::StmtKind::Break { .. } | hir::StmtKind::Continue { .. } | hir::StmtKind::Todo(_) => {}
    }
}

fn collect_closures_in_expr(expr: &hir::Expr, out: &mut Vec<hir::ClosureExpr>) {
    match &expr.kind {
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::VarRef(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::Todo(_) => {}
        hir::ExprKind::StructLit { fields, .. } => {
            for f in fields {
                collect_closures_in_expr(&f.value, out);
            }
        }
        hir::ExprKind::TupleLit { elements } => {
            for e in elements {
                collect_closures_in_expr(e, out);
            }
        }
        hir::ExprKind::InterpolatedString { parts, .. } => {
            for p in parts {
                if let hir::InterpolatedStringPart::Expr { expr } = p {
                    collect_closures_in_expr(expr, out);
                }
            }
        }
        hir::ExprKind::Unary { expr, .. } => collect_closures_in_expr(expr, out),
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            collect_closures_in_expr(lhs, out);
            collect_closures_in_expr(rhs, out);
        }
        hir::ExprKind::Block(b) => collect_closures_in_block(b, out),
        hir::ExprKind::Call { callee, args } => {
            collect_closures_in_expr(callee, out);
            for arg in args {
                match arg {
                    hir::CallArg::Positional(e) => collect_closures_in_expr(e, out),
                    hir::CallArg::Named { value, .. } => collect_closures_in_expr(value, out),
                }
            }
        }
        hir::ExprKind::Closure(c) => {
            out.push(c.clone());
            collect_closures_in_expr(&c.body, out);
        }
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_closures_in_expr(cond, out);
            collect_closures_in_expr(then_branch, out);
            if let Some(e) = else_branch.as_ref() {
                collect_closures_in_expr(e, out);
            }
        }
        hir::ExprKind::When { subject, arms } => {
            collect_closures_in_expr(subject, out);
            for arm in arms {
                if let Some(guard) = arm.guard.as_ref() {
                    collect_closures_in_expr(guard, out);
                }
                collect_closures_in_expr(&arm.body, out);
            }
        }
        hir::ExprKind::MemberAccess { receiver, .. } => collect_closures_in_expr(receiver, out),
        hir::ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    hir::CallArg::Positional(e) => collect_closures_in_expr(e, out),
                    hir::CallArg::Named { value, .. } => collect_closures_in_expr(value, out),
                }
            }
        }
        hir::ExprKind::Handle(h) => {
            collect_closures_in_block(&h.body, out);
            for arm in &h.arms {
                collect_closures_in_expr(&arm.body, out);
            }
            if let Some(finally) = h.finally.as_ref() {
                collect_closures_in_block(finally, out);
            }
        }
    }
}

fn trace_bitmap_for_payload_fields(
    target: TargetLayout,
    types: &TypeStore,
    fields: &[TypeId],
) -> (u64, Vec<u64>) {
    let trace_start = gc_object_header_size(target);
    let ptr_size = target.pointer_size.max(1);

    let mut slots: Vec<u64> = Vec::new();
    let mut off: u64 = 0;

    for &ty in fields {
        let layout = type_layout(types, target, ty);
        off = align_to(off, layout.align);

        if is_gc_pointer_like(types, ty) && off % ptr_size == 0 {
            slots.push(off / ptr_size);
        }

        off = off.saturating_add(layout.size);
    }

    slots.sort();
    slots.dedup();

    let Some(&max_slot) = slots.last() else {
        return (trace_start, Vec::new());
    };
    let len = (max_slot / 64) + 1;
    let mut words = vec![0u64; len as usize];
    for slot in slots {
        let wi = (slot / 64) as usize;
        let bit = (slot % 64) as u32;
        words[wi] |= 1u64 << bit;
    }

    (trace_start, words)
}

fn is_gc_pointer_like(types: &TypeStore, ty: TypeId) -> bool {
    match types.kind(ty) {
        // 绝大多数 ref type 在运行期是 GC-managed pointer；但少量“句柄型 ref”（Task/Executor）在 early stage
        // 会降为 word-sized integer handle（不应计入 GC pointer slots）。
        TypeKind::Ref(RefTypeKind::Nominal(nominal))
            if nominal.fqn == "scoop.core.Task" || nominal.fqn == "scoop.task.Executor" =>
        {
            false
        }
        TypeKind::Ref(_) => true,
        TypeKind::Value(ValueTypeKind::Option(inner)) => is_gc_pointer_like(types, *inner),
        _ => false,
    }
}

fn type_layout(types: &TypeStore, target: TargetLayout, ty: TypeId) -> TypeLayout {
    // v0：这里只需要足够支撑“字段 offset → bitmap slot”的推导，不追求覆盖所有类型语法。
    match types.kind(ty) {
        TypeKind::Ref(_) => pointer_layout(target),
        TypeKind::Param(_) => pointer_layout(target).without_niche(),
        TypeKind::Value(vk) => match vk {
            ValueTypeKind::Unit | ValueTypeKind::Nothing => TypeLayout::new(0, 1),
            ValueTypeKind::Bool => TypeLayout::new(1, 1).with_niche(NicheDomain {
                storage: NicheStorage::U8,
                next: 2,
                end: 256,
            }),
            ValueTypeKind::Int | ValueTypeKind::UInt => {
                TypeLayout::new(target.pointer_size, target.pointer_align)
            }
            ValueTypeKind::IntN(bits) | ValueTypeKind::UIntN(bits) => {
                let size = (u64::from(*bits) + 7) / 8;
                let align = size.clamp(1, target.pointer_align.max(1));
                TypeLayout::new(size, align)
            }
            ValueTypeKind::Tuple(elements) => aggregate_fields_layout(types, target, elements),
            ValueTypeKind::Option(inner) => option_layout(types, target, *inner),
            ValueTypeKind::Nominal(_) => {
                // early stage：对 nominal value types 先按 opaque word 处理（与 LLVM codegen 的 niche/boxing 兜底一致）。
                TypeLayout::new(target.pointer_size, target.pointer_align)
            }
        },
    }
}

fn option_layout(types: &TypeStore, target: TargetLayout, inner: TypeId) -> TypeLayout {
    let inner_layout = type_layout(types, target, inner);

    // niche path：inner 提供可用 niche 值时，Option 与 inner 共享 layout（对 offsets 影响很大）。
    if let Some(mut domain) = inner_layout.niche {
        if domain.take_one().is_some() {
            return TypeLayout::new(inner_layout.size, inner_layout.align).with_niche(domain);
        }
    }

    // tagged union fallback：`tag(i32) + payload(word)`（early stage 与 LLVM codegen 对齐）。
    let tag = TypeLayout::new(4, 4);
    let payload = TypeLayout::new(target.pointer_size, target.pointer_align);
    let payload_off = align_to(tag.size, payload.align);
    let align = tag.align.max(payload.align);
    let size = align_to(payload_off + payload.size, align);
    TypeLayout::new(size, align)
}

fn aggregate_fields_layout(
    types: &TypeStore,
    target: TargetLayout,
    fields: &[TypeId],
) -> TypeLayout {
    let mut size = 0u64;
    let mut align = 1u64;
    for &field in fields {
        let l = type_layout(types, target, field);
        size = align_to(size, l.align);
        size = size.saturating_add(l.size);
        align = align.max(l.align);
    }
    size = align_to(size, align);
    TypeLayout::new(size, align)
}

fn pointer_layout(target: TargetLayout) -> TypeLayout {
    TypeLayout::new(target.pointer_size, target.pointer_align).with_niche(NicheDomain {
        storage: NicheStorage::Pointer,
        next: 0,
        end: target.pointer_align.max(1),
    })
}

fn gc_object_header_size(target: TargetLayout) -> u64 {
    // `typedef struct { void* next; void* type_desc; uint64_t size_bytes; uint32_t flags; uint32_t mark; }`
    let ptr = TypeLayout::new(target.pointer_size, target.pointer_align);
    let u64_l = TypeLayout::new(8, 8.min(target.pointer_align.max(1)));
    let u32_l = TypeLayout::new(4, 4);

    let mut off = 0u64;
    off = align_to(off, ptr.align);
    off += ptr.size;
    off = align_to(off, ptr.align);
    off += ptr.size;
    off = align_to(off, u64_l.align);
    off += u64_l.size;
    off = align_to(off, u32_l.align);
    off += u32_l.size;
    off = align_to(off, u32_l.align);
    off += u32_l.size;

    // struct overall align：取 max align 再向上对齐。
    let align = ptr.align.max(u64_l.align).max(u32_l.align).max(1);
    align_to(off, align)
}

trait WithoutNiche {
    fn without_niche(self) -> Self;
}

impl WithoutNiche for TypeLayout {
    fn without_niche(mut self) -> Self {
        self.niche = None;
        self
    }
}

fn stable_hash64(text: &str) -> u64 {
    let digest = Sha256::digest(text.as_bytes());
    let bytes: [u8; 8] = digest[0..8].try_into().expect("sha256 output is 32 bytes");
    u64::from_le_bytes(bytes)
}

fn align_to(value: u64, align: u64) -> u64 {
    if align <= 1 {
        return value;
    }
    let mask = align - 1;
    (value + mask) & !mask
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn dump_rtti_type_desc_class_parent_chain_and_bitmap() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual(
            "<mem>",
            r#"
package rtti

import scoop.core.*

class Base(val s: String)

class Derived(val ok: Bool, val t: String) : Base("hi")
"#,
        );

        let dump = dump_file_type_desc(&sess, &src).unwrap();
        let mut by_name: BTreeMap<&str, &TypeDesc> = BTreeMap::new();
        for ty in &dump.types {
            by_name.insert(ty.name.as_str(), ty);
        }

        let base = by_name.get("rtti.Base").unwrap();
        assert_eq!(base.parent.as_deref(), None);
        assert_eq!(base.trace_bitmap_u64, vec![1u64]);

        let derived = by_name.get("rtti.Derived").unwrap();
        assert_eq!(derived.parent.as_deref(), Some("rtti.Base"));
        assert_eq!(
            derived.parent_chain,
            vec!["rtti.Base".to_string(), "rtti.Derived".to_string()]
        );
        assert_eq!(derived.trace_bitmap_u64, vec![5u64]);
    }

    #[test]
    fn dump_rtti_type_desc_closure_env_bitmap_marks_ref_captures() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual(
            "<mem>",
            r#"
package a
import scoop.core.*

fun main() {
  val s: String = "hi"
  val b: Bool = true
  val f = { println(s); b }
  // 防止未来优化把 f 彻底消掉（当前阶段不影响 lowering）。
  if (b) { println("ok") } else { println("no") }
}
"#,
        );

        let dump = dump_file_type_desc(&sess, &src).unwrap();
        let env = dump
            .types
            .iter()
            .find(|t| t.kind == TypeDescKind::ClosureEnv)
            .expect("should contain at least one closure env desc");

        assert!(env.name.starts_with("scoop.lambda_env$"));
        assert_eq!(env.trace_bitmap_u64, vec![1u64]);
    }

    #[test]
    fn dump_rtti_interface_id_and_method_slots() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual(
            "<mem>",
            r#"
package rtti

import scoop.core.*

interface IFoo {
  fun ping()
  fun add(x: Int, y: Int): Int
}

interface IBar : IFoo {
  fun pong()
}
"#,
        );

        let dump = dump_file_type_desc(&sess, &src).unwrap();

        let foo = dump
            .interfaces
            .iter()
            .find(|i| i.name == "rtti.IFoo")
            .expect("should contain rtti.IFoo");
        assert_eq!(foo.interface_id, stable_hash64("rtti.IFoo"));
        assert_eq!(foo.super_interfaces, Vec::<String>::new());
        assert_eq!(foo.method_slots.len(), 2);
        assert_eq!(foo.method_slots[0].slot, 0);
        assert_eq!(foo.method_slots[0].name, "ping");
        assert_eq!(foo.method_slots[0].params_len, 0);
        assert_eq!(foo.method_slots[1].slot, 1);
        assert_eq!(foo.method_slots[1].name, "add");
        assert_eq!(foo.method_slots[1].params_len, 2);

        let bar = dump
            .interfaces
            .iter()
            .find(|i| i.name == "rtti.IBar")
            .expect("should contain rtti.IBar");
        assert_eq!(bar.interface_id, stable_hash64("rtti.IBar"));
        assert_eq!(bar.super_interfaces, vec!["rtti.IFoo".to_string()]);
        assert_eq!(bar.method_slots.len(), 1);
        assert_eq!(bar.method_slots[0].slot, 0);
        assert_eq!(bar.method_slots[0].name, "pong");
        assert_eq!(bar.method_slots[0].params_len, 0);
    }
}
