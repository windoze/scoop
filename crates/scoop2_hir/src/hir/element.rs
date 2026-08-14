//! 声明层 element 体系（PLAN.md M2-1，增量第一步）。
//!
//! 把 [`TypedHir`] 的 16 张散表收拢为**单一 element 列表**：每个声明（函数 /
//! 方法 / 构造器 / variant / 顶层值 / 类型）一个 [`Element`]，携带声明位
//! （span/file）、种类化载荷、重载消歧 key。M2-5 MIR 翻转后它成为声明的唯一
//! 表示；当前与旧字段并存（增量迁移）。
//!
//! 确定性（C7）：`elements` 按 `(fqn Symbol, overload_key)` 排序；`by_fqn` 索引
//! 由列表重建（不序列化 HashMap 序）。
//!
//! 身份（C3）：本步仍以 `Symbol`（FQN 文本 interner 句柄）+ `overload_key` 为
//! 身份；`StableDefKey` 句柄化（cone 归属 + 定型 id）在 archive v1 落地时切换。

use std::collections::HashMap;

use scoop2_base::{FileId, Span, Symbol};

use super::{TypedHir, TypedSignature};
use crate::stable_id::overload_disambiguation_key;
use crate::ty::TypeId;

/// element 在列表中的下标。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub struct ElementId(pub u32);

/// 一个声明的 element。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Element {
    /// 归属 FQN（方法/字段 = owner 类型 FQN；其余 = 自身 FQN）。
    pub fqn: Symbol,
    /// simple name（方法名 / variant 名 / 字段名；顶层 = 末段名）。
    pub name: Symbol,
    pub kind: ElementKind,
    pub decl_span: Span,
    pub decl_file: FileId,
    /// 重载消歧 key（`[p1,...]->ret/E(row)`；非重载种类为空串）。
    pub overload_key: String,
}

/// element 种类与载荷。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ElementKind {
    /// 顶层函数。
    Fun { sig: TypedSignature },
    /// 成员方法（`fqn` = owner 类型 FQN）。
    Method { sig: TypedSignature },
    /// 构造器（primary / secondary；`fqn` = 类型 FQN）。
    Ctor { sig: TypedSignature },
    /// enum variant（`fqn` = enum FQN）。
    EnumVariant,
    /// 顶层 `val` / `var`。
    Global { ty: TypeId },
    /// 字段 / 属性（`fqn` = owner 类型 FQN）。
    Field { ty: TypeId },
    /// 类型声明（class/interface/struct/enum/effect/object）。
    Type { category: TypeCategory },
}

/// 类型声明类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TypeCategory {
    Class,
    Interface,
    Struct,
    Enum,
    Effect,
    Object,
}

/// 声明层 element 表。
#[derive(Debug, Clone, Default)]
pub struct ElementTable {
    /// 按 `(fqn, name, overload_key)` 排序的 element 列表。
    pub elements: Vec<Element>,
    /// FQN → element 下标列表（派生索引：不序列化，反序列化后重建；同 FQN 即重载集）。
    pub by_fqn: HashMap<Symbol, Vec<ElementId>>,
}

impl serde::Serialize for ElementTable {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.elements.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for ElementTable {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let elements: Vec<Element> = Vec::deserialize(deserializer)?;
        let mut table = ElementTable {
            elements,
            by_fqn: HashMap::new(),
        };
        table.rebuild_index();
        Ok(table)
    }
}

impl ElementTable {
    /// 重建 `by_fqn` 索引（构造 / 反序列化后调用）。
    pub fn rebuild_index(&mut self) {
        self.by_fqn = HashMap::with_capacity(self.elements.len());
        for (i, e) in self.elements.iter().enumerate() {
            self.by_fqn
                .entry(e.fqn)
                .or_default()
                .push(ElementId(i as u32));
        }
    }

    /// 按 FQN 查重载集（声明序 = 列表序）。
    pub fn overloads(&self, fqn: Symbol) -> &[ElementId] {
        self.by_fqn.get(&fqn).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// 按 (owner, name) 查首个方法 / 字段 element。
    pub fn member(&self, owner: Symbol, name: Symbol) -> Option<&Element> {
        self.overloads(owner)
            .iter()
            .map(|&id| &self.elements[id.0 as usize])
            .find(|e| e.name == name)
    }
}

/// 从 TypedHir 的散表装配 element 表（确定性排序）。
pub fn assemble(hir: &TypedHir) -> ElementTable {
    fn push_sig(
        hir: &TypedHir,
        elements: &mut Vec<Element>,
        fqn: Symbol,
        name: Symbol,
        make_kind: fn(TypedSignature) -> ElementKind,
        sig: &TypedSignature,
    ) {
        let overload_key = overload_disambiguation_key(sig, &hir.store, &hir.interner);
        elements.push(Element {
            fqn,
            name,
            kind: make_kind(sig.clone()),
            decl_span: sig.decl_span,
            decl_file: sig.decl_file,
            overload_key,
        });
    }

    let mut elements: Vec<Element> = Vec::new();

    // 顶层函数（key = fqn）。
    for (&fqn, sigs) in &hir.top_level_funs {
        let name = simple_name_of(hir, fqn);
        for sig in sigs {
            push_sig(
                hir,
                &mut elements,
                fqn,
                name,
                |s| ElementKind::Fun { sig: s },
                sig,
            );
        }
    }
    // 成员方法（外层 key = owner）。
    for (&owner, methods) in &hir.member_funs {
        for (&m, sigs) in methods {
            for sig in sigs {
                push_sig(
                    hir,
                    &mut elements,
                    owner,
                    m,
                    |s| ElementKind::Method { sig: s },
                    sig,
                );
            }
        }
    }
    // 构造器。
    for (&type_fqn, sigs) in &hir.ctor_signatures {
        let name = simple_name_of(hir, type_fqn);
        for sig in sigs {
            push_sig(
                hir,
                &mut elements,
                type_fqn,
                name,
                |s| ElementKind::Ctor { sig: s },
                sig,
            );
        }
    }
    // enum variant（无签名；overload_key 为空）。
    for (&enum_fqn, variants) in &hir.enum_variants {
        for &v in variants {
            elements.push(Element {
                fqn: enum_fqn,
                name: v,
                kind: ElementKind::EnumVariant,
                decl_span: Span::default(),
                decl_file: FileId(0),
                overload_key: String::new(),
            });
        }
    }
    // 顶层 val/var。
    for (&fqn, &ty) in &hir.top_level_vals {
        elements.push(Element {
            fqn,
            name: simple_name_of(hir, fqn),
            kind: ElementKind::Global { ty },
            decl_span: Span::default(),
            decl_file: FileId(0),
            overload_key: String::new(),
        });
    }
    // 字段（owner → 字段名 → 类型）。
    for (&owner, fields) in &hir.members {
        for (&fname, &ty) in fields {
            elements.push(Element {
                fqn: owner,
                name: fname,
                kind: ElementKind::Field { ty },
                decl_span: Span::default(),
                decl_file: FileId(0),
                overload_key: String::new(),
            });
        }
    }
    // 类型声明：class/interface 集合 + enum（enum_variants keys）。
    let mut type_kinds: Vec<(Symbol, TypeCategory)> = Vec::new();
    for &fqn in &hir.class_fqns {
        type_kinds.push((fqn, TypeCategory::Class));
    }
    for &fqn in &hir.interface_fqns {
        type_kinds.push((fqn, TypeCategory::Interface));
    }
    for &fqn in hir.enum_variants.keys() {
        type_kinds.push((fqn, TypeCategory::Enum));
    }
    type_kinds.sort_by_key(|(fqn, _)| *fqn);
    type_kinds.dedup_by(|a, b| a.0 == b.0);
    for (fqn, category) in type_kinds {
        elements.push(Element {
            fqn,
            name: simple_name_of(hir, fqn),
            kind: ElementKind::Type { category },
            decl_span: Span::default(),
            decl_file: FileId(0),
            overload_key: String::new(),
        });
    }

    // 确定性排序：(fqn, name, overload_key)。
    elements
        .sort_by(|a, b| (a.fqn, a.name, &a.overload_key).cmp(&(b.fqn, b.name, &b.overload_key)));
    let mut table = ElementTable {
        elements,
        by_fqn: HashMap::new(),
    };
    table.rebuild_index();
    table
}

fn simple_name_of(hir: &TypedHir, fqn: Symbol) -> Symbol {
    let text = hir.interner.resolve(fqn);
    match text.rfind('.') {
        Some(dot) => hir.interner.get(&text[dot + 1..]).unwrap_or(fqn),
        None => fqn,
    }
}
