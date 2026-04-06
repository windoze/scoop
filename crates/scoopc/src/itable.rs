//! interface dispatch table（itable）布局（T1507c3 / T1508c）。
//!
//! 目标：
//! - 为 interface 分配稳定 `interface_id`（hash64(FQN)）并生成 method slot 表（声明顺序）。
//! - 为每个 class 生成 itable entries：`interface_id -> slot -> impl_member_fqn`。
//!
//! 说明（v0 简化）：
//! - slot key 仍以“最小形状信息”为主：`name + params_len + has_receiver`；
//! - 若 interface method 有 body（默认方法），且 class 未实现，则 itable slot 指向 interface 自身的默认实现；
//! - 若 interface method 无 body（抽象方法）且无法解析实现，则返回错误（应在 typecheck 阶段先被门禁）。

use std::collections::{HashMap, HashSet};

use miette::Diagnostic;
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::ast;
use crate::resolve::Index;
use crate::source::SourceFile;
use crate::vtable::ClassVtableIndex;

/// interface method 的“最小形状 key”（用于 slot/实现匹配）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MethodShapeKey {
    name: String,
    params_len: u32,
    has_receiver: bool,
}

/// interface method slot 信息（v0：按声明顺序分配 slot）。
#[derive(Debug, Clone)]
pub struct InterfaceMethodSlot {
    pub slot: u32,
    pub name: String,
    pub params_len: u32,
    pub has_receiver: bool,
    /// interface 默认方法（有 body）为 true；抽象方法为 false。
    pub has_body: bool,
}

/// interface 元数据（codegen-friendly）。
#[derive(Debug, Clone)]
pub struct InterfaceInfo {
    pub fqn: String,
    pub interface_id: u64,
    pub super_interfaces: Vec<String>,
    pub method_slots: Vec<InterfaceMethodSlot>,
}

/// interface FQN -> interface info。
pub type InterfaceIndex = HashMap<String, InterfaceInfo>;

/// class itable entry：一个 interface 的 slot → impl 映射。
#[derive(Debug, Clone)]
pub struct ClassItableEntry {
    pub interface_fqn: String,
    pub interface_id: u64,
    pub method_impl_fqns: Vec<String>,
}

/// class FQN -> itable entries（按 interface_id 稳定排序）。
pub type ClassItableIndex = HashMap<String, Vec<ClassItableEntry>>;

#[derive(Debug, Error, Diagnostic)]
pub enum ItableLayoutError {
    #[error("interface 继承链存在循环：{fqn}")]
    #[diagnostic(code(scoop::itable::inheritance_cycle))]
    InheritanceCycle { fqn: String },

    #[error("interface method slot 形状不唯一：{interface_fqn}.{member}")]
    #[diagnostic(code(scoop::itable::ambiguous_interface_method_slot))]
    AmbiguousInterfaceMethodSlot {
        interface_fqn: String,
        member: String,
    },
}

#[derive(Debug, Clone)]
struct ClassDeclInfo {
    fqn: String,
    super_class_fqn: Option<String>,
    direct_interfaces: Vec<String>,
    methods: Vec<ClassMethodInfo>,
}

#[derive(Debug, Clone)]
struct ClassMethodInfo {
    name: String,
    params_len: u32,
    has_receiver: bool,
}

pub fn collect_interfaces_and_class_itables(
    compilation_unit: &[(&SourceFile, &ast::File)],
    index: &Index,
    class_vtables: &ClassVtableIndex,
) -> Result<(InterfaceIndex, ClassItableIndex), ItableLayoutError> {
    let mut interfaces: InterfaceIndex = HashMap::new();
    let mut classes: HashMap<String, ClassDeclInfo> = HashMap::new();

    for (source, file) in compilation_unit {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            match item {
                ast::Item::Type(ty) => {
                    collect_interfaces_in_type_decl(
                        source,
                        file,
                        &pkg_prefix,
                        ty,
                        index,
                        &mut interfaces,
                    );
                    collect_classes_in_type_decl(
                        source,
                        file,
                        &pkg_prefix,
                        ty,
                        index,
                        &mut classes,
                    );
                }
                ast::Item::Object(obj) => {
                    collect_interfaces_in_object_decl(
                        source,
                        file,
                        &pkg_prefix,
                        obj,
                        index,
                        &mut interfaces,
                    );
                    collect_classes_in_object_decl(
                        source,
                        file,
                        &pkg_prefix,
                        obj,
                        index,
                        &mut classes,
                    );
                }
                ast::Item::Fun(_)
                | ast::Item::Val(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::TypeAlias(_)
                | ast::Item::ComptimeIf(_) => {}
            }
        }
    }

    let class_itables = build_class_itables(&classes, &interfaces, class_vtables)?;
    Ok((interfaces, class_itables))
}

fn build_class_itables(
    classes: &HashMap<String, ClassDeclInfo>,
    interfaces: &InterfaceIndex,
    class_vtables: &ClassVtableIndex,
) -> Result<ClassItableIndex, ItableLayoutError> {
    let mut memo_iface_sets: HashMap<String, HashSet<String>> = HashMap::new();
    let mut visiting_classes: HashSet<String> = HashSet::new();

    let mut class_fqns: Vec<&str> = classes.keys().map(|k| k.as_str()).collect();
    class_fqns.sort();

    let mut out: ClassItableIndex = HashMap::new();
    for class_fqn in class_fqns {
        let ifaces = compute_class_interface_closure(
            class_fqn,
            classes,
            interfaces,
            &mut visiting_classes,
            &mut memo_iface_sets,
        )?;

        // vtable impl map：shape -> 真实实现成员（已考虑 override 覆盖）。
        let mut vtable_impls: HashMap<MethodShapeKey, String> = HashMap::new();
        if let Some(vslots) = class_vtables.get(class_fqn) {
            for s in vslots {
                vtable_impls.insert(
                    MethodShapeKey {
                        name: s.name.clone(),
                        params_len: s.params_len,
                        has_receiver: s.has_receiver,
                    },
                    s.impl_member_fqn.clone(),
                );
            }
        }

        let mut entries: Vec<ClassItableEntry> = Vec::new();
        for iface_fqn in ifaces {
            let (interface_id, method_slots) = match interfaces.get(&iface_fqn) {
                Some(info) => (info.interface_id, info.method_slots.clone()),
                None => (stable_hash64(&iface_fqn), Vec::new()),
            };

            // slot -> impl_member_fqn：保持与 slot index 对齐。
            let mut impls: Vec<String> = vec![String::new(); method_slots.len()];

            // v0：slot key（name+params_len+has_receiver）在单个 interface 内必须是唯一的，
            // 否则 lowering/codegen 无法在调用点稳定选中正确 slot。
            let mut seen_shapes: HashSet<MethodShapeKey> = HashSet::new();

            for slot in &method_slots {
                let key = MethodShapeKey {
                    name: slot.name.clone(),
                    params_len: slot.params_len,
                    has_receiver: slot.has_receiver,
                };

                if !seen_shapes.insert(key.clone()) {
                    return Err(ItableLayoutError::AmbiguousInterfaceMethodSlot {
                        interface_fqn: iface_fqn.clone(),
                        member: slot.name.clone(),
                    });
                }

                let impl_member_fqn = if let Some(found) = vtable_impls.get(&key).cloned() {
                    found
                } else if let Some((in_fqn, member_fqn)) =
                    resolve_method_in_class_hierarchy(class_fqn, &key, classes, &mut HashSet::new())
                {
                    let _ = in_fqn;
                    member_fqn
                } else if slot.has_body {
                    // 默认方法：允许回退到 interface 自身的实现。
                    format!("{iface_fqn}.{}", slot.name)
                } else {
                    // 抽象方法：原则上应由 typecheck（`missing_interface_member`）提前门禁。
                    // 为避免“typecheck 未覆盖的 inherited interface”导致后端直接失败，这里用空占位，
                    // 由 codegen 将该 slot 填充为 NULL（若运行期触发调用，则属于未定义行为的防线兜底）。
                    String::new()
                };

                let idx = slot.slot as usize;
                if idx >= impls.len() {
                    // slot 不是连续 0..N 的情况在 v0 不支持（slot 分配即声明顺序）。
                    continue;
                }
                impls[idx] = impl_member_fqn;
            }

            entries.push(ClassItableEntry {
                interface_fqn: iface_fqn,
                interface_id,
                method_impl_fqns: impls,
            });
        }

        entries.sort_by(|a, b| a.interface_id.cmp(&b.interface_id));
        out.insert(class_fqn.to_string(), entries);
    }

    Ok(out)
}

fn compute_class_interface_closure(
    class_fqn: &str,
    classes: &HashMap<String, ClassDeclInfo>,
    interfaces: &InterfaceIndex,
    visiting: &mut HashSet<String>,
    memo: &mut HashMap<String, HashSet<String>>,
) -> Result<HashSet<String>, ItableLayoutError> {
    if let Some(found) = memo.get(class_fqn) {
        return Ok(found.clone());
    }

    if !visiting.insert(class_fqn.to_string()) {
        return Err(ItableLayoutError::InheritanceCycle {
            fqn: class_fqn.to_string(),
        });
    }

    let mut out: HashSet<String> = HashSet::new();
    if let Some(info) = classes.get(class_fqn) {
        for iface in &info.direct_interfaces {
            collect_interface_and_supers(iface, interfaces, &mut out, &mut HashSet::new());
        }

        if let Some(super_fqn) = info.super_class_fqn.as_deref() {
            if classes.contains_key(super_fqn) {
                let super_ifaces = compute_class_interface_closure(
                    super_fqn, classes, interfaces, visiting, memo,
                )?;
                out.extend(super_ifaces);
            }
        }
    }

    let _ = visiting.remove(class_fqn);
    memo.insert(class_fqn.to_string(), out.clone());
    Ok(out)
}

fn collect_interface_and_supers(
    iface_fqn: &str,
    interfaces: &InterfaceIndex,
    out: &mut HashSet<String>,
    visiting: &mut HashSet<String>,
) {
    if !visiting.insert(iface_fqn.to_string()) {
        return;
    }

    if !out.insert(iface_fqn.to_string()) {
        let _ = visiting.remove(iface_fqn);
        return;
    }

    if let Some(iface) = interfaces.get(iface_fqn) {
        for sup in &iface.super_interfaces {
            collect_interface_and_supers(sup, interfaces, out, visiting);
        }
    }

    let _ = visiting.remove(iface_fqn);
}

fn resolve_method_in_class_hierarchy(
    class_fqn: &str,
    key: &MethodShapeKey,
    classes: &HashMap<String, ClassDeclInfo>,
    visiting: &mut HashSet<String>,
) -> Option<(String, String)> {
    if !visiting.insert(class_fqn.to_string()) {
        return None;
    }

    let info = classes.get(class_fqn)?;
    for m in &info.methods {
        if m.name == key.name
            && m.params_len == key.params_len
            && m.has_receiver == key.has_receiver
        {
            let member = format!("{}.{}", info.fqn, m.name);
            let _ = visiting.remove(class_fqn);
            return Some((info.fqn.clone(), member));
        }
    }

    if let Some(super_fqn) = info.super_class_fqn.as_deref() {
        if classes.contains_key(super_fqn) {
            let resolved = resolve_method_in_class_hierarchy(super_fqn, key, classes, visiting);
            let _ = visiting.remove(class_fqn);
            return resolved;
        }
    }

    let _ = visiting.remove(class_fqn);
    None
}

fn collect_interfaces_in_type_decl(
    source: &SourceFile,
    file: &ast::File,
    owner_prefix: &str,
    decl: &ast::TypeDecl,
    index: &Index,
    out: &mut InterfaceIndex,
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
                    has_body: matches!(fun.body, ast::FunBody::Block(_)),
                });
                slot = slot.saturating_add(1);
            }
        }

        out.insert(
            type_fqn.clone(),
            InterfaceInfo {
                fqn: type_fqn.clone(),
                interface_id: stable_hash64(&type_fqn),
                super_interfaces,
                method_slots,
            },
        );
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
    out: &mut InterfaceIndex,
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

fn collect_classes_in_type_decl(
    source: &SourceFile,
    file: &ast::File,
    owner_prefix: &str,
    decl: &ast::TypeDecl,
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

        let direct_interfaces = decl
            .supertypes
            .iter()
            .filter(|st| st.ctor_args_span.is_none())
            .filter_map(|st| index.type_ref_to_fqn_in_file(source, file, &st.ty))
            .collect::<Vec<_>>();

        let mut methods: Vec<ClassMethodInfo> = Vec::new();
        if let Some(body) = &decl.body {
            for member in &body.members {
                let ast::TypeMember::Fun(fun) = member else {
                    continue;
                };

                methods.push(ClassMethodInfo {
                    name: fun.name.text(source).to_string(),
                    params_len: fun.params.len() as u32,
                    has_receiver: fun.receiver.is_some(),
                });
            }
        }

        out.insert(
            type_fqn.clone(),
            ClassDeclInfo {
                fqn: type_fqn.clone(),
                super_class_fqn,
                direct_interfaces,
                methods,
            },
        );
    }

    let Some(body) = &decl.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_classes_in_type_decl(source, file, &type_fqn, nested, index, out);
            }
            ast::TypeMember::Object(obj) => {
                collect_classes_in_object_decl(source, file, &type_fqn, obj, index, out);
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
    owner_prefix: &str,
    obj: &ast::ObjectDecl,
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
                collect_classes_in_type_decl(source, file, &obj_fqn, nested, index, out);
            }
            ast::TypeMember::Object(nested) => {
                collect_classes_in_object_decl(source, file, &obj_fqn, nested, index, out);
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

fn stable_hash64(text: &str) -> u64 {
    let digest = Sha256::digest(text.as_bytes());
    let bytes: [u8; 8] = digest[0..8].try_into().expect("sha256 output is 32 bytes");
    u64::from_le_bytes(bytes)
}
