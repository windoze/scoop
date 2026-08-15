//! MIR 声明段（M4：LIR 输入切换）。
//!
//! LIR 的 7 子系统原先直接查 `TypedHir` 的声明侧表（成员序/enum variant/
//! class 集合等）。本结构把这些表**定稿进 MIR archive**——LIR 只消费 MIR
//! 产出（C1：LIR 不读 HIR）。语义与 `TypedHir` 同名方法逐一镜像。

use scoop2_base::{Interner, Symbol};
use scoop2_hir::hir::TypedHir;
use scoop2_hir::ty::TypeId;

/// 声明侧定稿数据（随 MIR archive 序列化；HashMap 一律排序 Vec——C7）。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MirDecls {
    /// `ordered_members`：FQN → 声明序成员（名字, 类型）。
    pub members: Vec<(Symbol, Vec<(Symbol, TypeId)>)>,
    /// `ordered_class_fields`：class FQN → 声明序字段（名字, 类型）。
    pub class_fields: Vec<(Symbol, Vec<(Symbol, TypeId)>)>,
    /// enum FQN → variant 名（声明序）。
    pub enum_variants: Vec<(Symbol, Vec<Symbol>)>,
    /// 有成员表的 FQN 集合（`members.contains_key` 等价）。
    pub member_fqns: Vec<Symbol>,
    pub class_fqns: Vec<Symbol>,
    pub interface_fqns: Vec<Symbol>,
    /// 类型 FQN → 直接超类型（声明序）。
    pub supertypes: Vec<(Symbol, Vec<Symbol>)>,
    /// 类型 FQN → 直接子类型（派生，已排序）。
    pub direct_subtypes: Vec<(Symbol, Vec<Symbol>)>,
}

fn sorted_entries<T: Clone>(m: &std::collections::HashMap<Symbol, T>) -> Vec<(Symbol, T)> {
    let mut v: Vec<(Symbol, T)> = m.iter().map(|(k, val)| (*k, val.clone())).collect();
    v.sort_unstable_by_key(|(k, _)| *k);
    v
}

/// 内层 HashMap（成员名 → 类型）→ 声明序 Vec（`ordered_members` 的排序规则：
/// member_order 声明序，缺失时按 Symbol 升序——与 TypedHir 同名方法一致）。
fn members_to_ordered(
    hir: &TypedHir,
    fqn: &Symbol,
    inner: &std::collections::HashMap<Symbol, TypeId>,
) -> Vec<(Symbol, TypeId)> {
    if let Some(order) = hir.member_order.get(fqn) {
        order
            .iter()
            .filter_map(|n| inner.get(n).map(|&ty| (*n, ty)))
            .collect()
    } else {
        let mut v: Vec<(Symbol, TypeId)> =
            inner.iter().map(|(n, &ty)| (*n, ty)).collect();
        v.sort_unstable_by_key(|(n, _)| *n);
        v
    }
}

fn sorted_set(s: &std::collections::HashSet<Symbol>) -> Vec<Symbol> {
    let mut v: Vec<Symbol> = s.iter().copied().collect();
    v.sort_unstable();
    v
}

impl MirDecls {
    /// 从 TypedHir 定稿（mir-build 时一次；此后 LIR 不再回 HIR）。
    pub fn from_hir(hir: &TypedHir) -> Self {
        // enum_variants 已是声明序 Vec（TypedHir 同）。
        let enum_variants: Vec<(Symbol, Vec<Symbol>)> = sorted_entries(&hir.enum_variants);
        Self {
            members: {
                let keys: Vec<Symbol> = hir.members.keys().copied().collect();
                let mut out: Vec<(Symbol, Vec<(Symbol, TypeId)>)> = keys
                    .iter()
                    .map(|k| (*k, members_to_ordered(hir, k, &hir.members[k])))
                    .collect();
                out.sort_unstable_by_key(|(k, _)| *k);
                out
            },
            class_fields: {
                let keys: Vec<Symbol> = hir.members.keys().copied().collect();
                let mut out: Vec<(Symbol, Vec<(Symbol, TypeId)>)> = keys
                    .into_iter()
                    .map(|k| (k, hir.ordered_class_fields(k)))
                    .collect();
                out.sort_unstable_by_key(|(k, _)| *k);
                out
            },
            enum_variants,
            member_fqns: hir.members.keys().copied().collect(),
            class_fqns: sorted_set(&hir.class_fqns),
            interface_fqns: sorted_set(&hir.interface_fqns),
            supertypes: sorted_entries(&hir.supertypes),
            direct_subtypes: sorted_entries(&hir.direct_subtypes),
        }
    }

    /// `TypedHir::ordered_members` 镜像。
    pub fn ordered_members(&self, fqn: &Symbol) -> Vec<(Symbol, TypeId)> {
        self.members
            .iter()
            .find(|(k, _)| k == fqn)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    }

    /// `TypedHir::ordered_class_fields` 镜像。
    pub fn ordered_class_fields(&self, fqn: Symbol) -> Vec<(Symbol, TypeId)> {
        self.class_fields
            .iter()
            .find(|(k, _)| *k == fqn)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    }

    /// enum 的 variant 名（声明序）。
    pub fn variants_of(&self, fqn: &Symbol) -> Option<&[Symbol]> {
        self.enum_variants
            .iter()
            .find(|(k, _)| k == fqn)
            .map(|(_, v)| v.as_slice())
    }

    pub fn is_member_fqn(&self, fqn: &Symbol) -> bool {
        self.member_fqns.contains(fqn)
    }

    pub fn is_class(&self, fqn: &Symbol) -> bool {
        self.class_fqns.contains(fqn)
    }

    pub fn is_interface(&self, fqn: &Symbol) -> bool {
        self.interface_fqns.contains(fqn)
    }

    pub fn is_enum(&self, fqn: &Symbol) -> bool {
        self.enum_variants.iter().any(|(k, _)| k == fqn)
    }

    pub fn supertypes_of(&self, fqn: &Symbol) -> &[Symbol] {
        self.supertypes
            .iter()
            .find(|(k, _)| k == fqn)
            .map(|(_, v)| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn direct_subtypes_of(&self, fqn: &Symbol) -> &[Symbol] {
        self.direct_subtypes
            .iter()
            .find(|(k, _)| k == fqn)
            .map(|(_, v)| v.as_slice())
            .unwrap_or(&[])
    }

    /// 符号文本解析（interner 由 archive 顶层携带，此处按引用传入）。
    pub fn text<'a>(&self, interner: &'a Interner, sym: Symbol) -> &'a str {
        interner.resolve(sym)
    }
}
