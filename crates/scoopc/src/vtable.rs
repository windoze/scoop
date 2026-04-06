//! class vtable slot 布局（TODO T1507c2 / T1508b）。
//!
//! 该模块负责在“编译单元（sysroot + 当前 cone）”的 AST 视图上，构建每个 class 的 vtable slot 列表：
//! - slot key（v0）：`name + params_len + has_receiver`；
//! - slot layout：继承链继承 slots，`override` 复用父类 slot；
//! - slot payload：记录每个 slot 在该 class 上最终指向的实现成员（`impl_member_fqn`）。
//!
//! 说明：
//! - 该布局既用于 `scoop dump-rtti` 的可观测导出，也用于 LLVM 后端生成实际 vtable 常量并执行虚调用；
//! - 这里刻意只做“布局与映射”，不做更强的 override 合法性校验（由 typecheck/inheritance 覆盖）。

use std::collections::{HashMap, HashSet};

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::resolve::{Index, ModifierSet};
use crate::source::SourceFile;

/// 一个 class vtable 的单个 slot（codegen-friendly 视图）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassVtableSlot {
    pub slot: u32,
    pub name: String,
    /// 参数个数（不含 receiver；与 `ast::FunDecl.params.len()` 对齐）。
    pub params_len: u32,
    pub has_receiver: bool,
    /// 该 slot 在当前 class 上最终指向的实现成员 FQN（可能来自父类或 override 覆盖）。
    pub impl_member_fqn: String,
}

/// `class_fqn -> vtable slots` 索引。
pub type ClassVtableIndex = HashMap<String, Vec<ClassVtableSlot>>;

#[derive(Debug, Error, Diagnostic)]
pub enum VtableLayoutError {
    #[error("class 继承链存在循环：{fqn}")]
    #[diagnostic(code(scoop::vtable::inheritance_cycle))]
    InheritanceCycle { fqn: String },
}

pub fn collect_class_vtables(
    compilation_unit: &[(&SourceFile, &ast::File)],
    index: &Index,
) -> Result<ClassVtableIndex, VtableLayoutError> {
    let mut classes: HashMap<String, ClassDeclInfo> = HashMap::new();

    for (source, file) in compilation_unit {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            match item {
                ast::Item::Type(ty) => {
                    collect_classes_in_type_decl(source, file, ty, &pkg_prefix, index, &mut classes)
                }
                ast::Item::Object(obj) => collect_classes_in_object_decl(
                    source,
                    file,
                    obj,
                    &pkg_prefix,
                    index,
                    &mut classes,
                ),
                ast::Item::Fun(_)
                | ast::Item::Val(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::TypeAlias(_)
                | ast::Item::ComptimeIf(_) => {}
            }
        }
    }

    build_class_vtables(&classes)
}

#[derive(Debug, Clone)]
struct ClassDeclInfo {
    fqn: String,
    super_class_fqn: Option<String>,
    methods: Vec<ClassMethodInfo>,
}

#[derive(Debug, Clone)]
struct ClassMethodInfo {
    name: String,
    params_len: u32,
    has_receiver: bool,
    modifiers: ModifierSet,
}

fn build_class_vtables(
    classes: &HashMap<String, ClassDeclInfo>,
) -> Result<ClassVtableIndex, VtableLayoutError> {
    let mut memo: ClassVtableIndex = HashMap::new();

    let mut class_fqns: Vec<&str> = classes.keys().map(|k| k.as_str()).collect();
    class_fqns.sort();

    for fqn in class_fqns {
        let mut visiting: HashSet<String> = HashSet::new();
        let slots = compute_class_vtable_slots(fqn, classes, &mut visiting, &mut memo)?;
        memo.insert(fqn.to_string(), slots);
    }

    Ok(memo)
}

fn compute_class_vtable_slots(
    class_fqn: &str,
    classes: &HashMap<String, ClassDeclInfo>,
    visiting: &mut HashSet<String>,
    memo: &mut ClassVtableIndex,
) -> Result<Vec<ClassVtableSlot>, VtableLayoutError> {
    if let Some(found) = memo.get(class_fqn) {
        return Ok(found.clone());
    }

    if !visiting.insert(class_fqn.to_string()) {
        return Err(VtableLayoutError::InheritanceCycle {
            fqn: class_fqn.to_string(),
        });
    }

    let Some(info) = classes.get(class_fqn) else {
        let _ = visiting.remove(class_fqn);
        return Ok(Vec::new());
    };

    let mut slots = if let Some(super_fqn) = info.super_class_fqn.as_deref() {
        if classes.contains_key(super_fqn) {
            compute_class_vtable_slots(super_fqn, classes, visiting, memo)?
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    for m in &info.methods {
        if !(m.modifiers.open || m.modifiers.abstract_ || m.modifiers.override_) {
            continue;
        }

        let key_matches = |s: &ClassVtableSlot| {
            s.name == m.name && s.params_len == m.params_len && s.has_receiver == m.has_receiver
        };

        let member_fqn = format!("{}.{}", info.fqn, m.name);

        if m.modifiers.override_ {
            if let Some(existing) = slots.iter_mut().find(|s| key_matches(s)) {
                existing.impl_member_fqn = member_fqn;
                continue;
            }
        }

        let slot = slots.len() as u32;
        slots.push(ClassVtableSlot {
            slot,
            name: m.name.clone(),
            params_len: m.params_len,
            has_receiver: m.has_receiver,
            impl_member_fqn: member_fqn,
        });
    }

    let _ = visiting.remove(class_fqn);
    Ok(slots)
}

fn collect_classes_in_type_decl(
    source: &SourceFile,
    file: &ast::File,
    decl: &ast::TypeDecl,
    owner_prefix: &str,
    index: &Index,
    out: &mut HashMap<String, ClassDeclInfo>,
) {
    let name = decl.name.text(source).to_string();
    let type_fqn = join_prefix(owner_prefix, &name);

    if matches!(decl.kind, ast::TypeKind::Class) {
        let super_class_fqn = decl
            .supertypes
            .iter()
            .filter(|st| st.ctor_args_span.is_some())
            .find_map(|st| index.type_ref_to_fqn_in_file(source, file, &st.ty));

        let mut methods: Vec<ClassMethodInfo> = Vec::new();
        if let Some(body) = &decl.body {
            for member in &body.members {
                let ast::TypeMember::Fun(fun) = member else {
                    continue;
                };

                let modifiers = ModifierSet::from_modifiers(&fun.modifiers);
                methods.push(ClassMethodInfo {
                    name: fun.name.text(source).to_string(),
                    params_len: fun.params.len() as u32,
                    has_receiver: fun.receiver.is_some(),
                    modifiers,
                });
            }
        }

        out.insert(
            type_fqn.clone(),
            ClassDeclInfo {
                fqn: type_fqn.clone(),
                super_class_fqn,
                methods,
            },
        );
    }

    // nested types
    let Some(body) = &decl.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_classes_in_type_decl(source, file, nested, &type_fqn, index, out);
            }
            ast::TypeMember::Object(obj) => {
                collect_classes_in_object_decl(source, file, obj, &type_fqn, index, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

fn collect_classes_in_object_decl(
    source: &SourceFile,
    file: &ast::File,
    obj: &ast::ObjectDecl,
    owner_prefix: &str,
    index: &Index,
    out: &mut HashMap<String, ClassDeclInfo>,
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
                collect_classes_in_type_decl(source, file, nested, &obj_fqn, index, out);
            }
            ast::TypeMember::Object(nested_obj) => {
                collect_classes_in_object_decl(source, file, nested_obj, &obj_fqn, index, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

fn package_prefix(source: &SourceFile, pkg: Option<&ast::PackageDecl>) -> String {
    let Some(p) = pkg else {
        return String::new();
    };

    let mut out = String::new();
    for (idx, seg) in p.path.iter().enumerate() {
        if idx != 0 {
            out.push('.');
        }
        out.push_str(seg.text(source));
    }
    out
}

fn join_prefix(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}

fn object_decl_name(source: &SourceFile, obj: &ast::ObjectDecl) -> Option<String> {
    if let Some(name) = obj.name.as_ref() {
        return Some(name.text(source).to_string());
    }
    // anonymous object：仅对 companion object 使用（`companion object { ... }`）。
    if matches!(obj.kind, ast::ObjectKind::Companion) {
        return Some("Companion".to_string());
    }
    None
}
