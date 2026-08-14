//! `TypedHir` 的手动 serde：HashMap/HashSet 字段按 key 排序序列化（字节确定，
//! PLAN.md C7——HashMap 迭代序不得进入字节流），反序列化重建哈希表。
//!
//! v0 archive（PLAN.md M1）使用；M2 element 体系落地后本实现随 v0 一并退役。

use std::collections::{HashMap, HashSet};

use scoop2_base::Symbol;

use super::{
    ClassCtorParamInfo, SuperCtorDelegation, TypeConstraintsSnapshot, TypedFile, TypedHir,
    TypedSignature, type_info::TypeInfo,
};
use crate::ty::{StoreRepr, TypeId, TypeStore};

/// 排序条目（K 升序）；序列化确定性用。
fn sorted_entries<K: Ord + Copy, V: Clone>(m: &HashMap<K, V>) -> Vec<(K, V)> {
    let mut v: Vec<(K, V)> = m.iter().map(|(&k, val)| (k, val.clone())).collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

/// 排序成员（HashSet → 升序 Vec）。
fn sorted_set<K: Ord + Copy>(s: &HashSet<K>) -> Vec<K> {
    let mut v: Vec<K> = s.iter().copied().collect();
    v.sort_unstable();
    v
}

/// `TypedHir` 的可序列化镜像：全部表为排序 Vec。
#[derive(serde::Serialize, serde::Deserialize)]
struct TypedHirRepr {
    store: StoreRepr,
    /// interner：AST / TypedHir 中全部 Symbol 的解析依据（typecheck 结束时的
    /// 会话快照，覆盖所有文件 intern 过的文本）。
    interner: scoop2_base::Interner,
    top_level_funs: Vec<(Symbol, Vec<TypedSignature>)>,
    member_funs: Vec<(Symbol, Vec<(Symbol, Vec<TypedSignature>)>)>,
    member_fun_order: Vec<(Symbol, Vec<Symbol>)>,
    members: Vec<(Symbol, Vec<(Symbol, TypeId)>)>,
    member_order: Vec<(Symbol, Vec<Symbol>)>,
    ctor_signatures: Vec<(Symbol, Vec<TypedSignature>)>,
    top_level_vals: Vec<(Symbol, TypeId)>,
    enum_variants: Vec<(Symbol, Vec<Symbol>)>,
    type_constraints: Vec<(Symbol, TypeConstraintsSnapshot)>,
    interface_fqns: Vec<Symbol>,
    class_fqns: Vec<Symbol>,
    extensible_class_fqns: Vec<Symbol>,
    direct_subtypes: Vec<(Symbol, Vec<Symbol>)>,
    supertypes: Vec<(Symbol, Vec<Symbol>)>,
    class_ctor_params: Vec<(Symbol, Vec<ClassCtorParamInfo>)>,
    super_ctor_delegations: Vec<(Symbol, SuperCtorDelegation)>,
    type_infos: Vec<(TypeId, TypeInfo)>,
    files: Vec<TypedFile>,
}

impl From<&TypedHir> for TypedHirRepr {
    fn from(h: &TypedHir) -> Self {
        Self {
            store: StoreRepr::from(&h.store),
            interner: h.interner.clone(),
            top_level_funs: sorted_entries(&h.top_level_funs),
            member_funs: sorted_entries(&h.member_funs)
                .into_iter()
                .map(|(k, inner)| (k, sorted_entries(&inner)))
                .collect(),
            member_fun_order: sorted_entries(&h.member_fun_order),
            members: sorted_entries(&h.members)
                .into_iter()
                .map(|(k, inner)| (k, sorted_entries(&inner)))
                .collect(),
            member_order: sorted_entries(&h.member_order),
            ctor_signatures: sorted_entries(&h.ctor_signatures),
            top_level_vals: sorted_entries(&h.top_level_vals),
            enum_variants: sorted_entries(&h.enum_variants),
            type_constraints: sorted_entries(&h.type_constraints),
            interface_fqns: sorted_set(&h.interface_fqns),
            class_fqns: sorted_set(&h.class_fqns),
            extensible_class_fqns: sorted_set(&h.extensible_class_fqns),
            direct_subtypes: sorted_entries(&h.direct_subtypes),
            supertypes: sorted_entries(&h.supertypes),
            class_ctor_params: sorted_entries(&h.class_ctor_params),
            super_ctor_delegations: sorted_entries(&h.super_ctor_delegations),
            type_infos: sorted_entries(&h.type_infos),
            files: h.files.clone(),
        }
    }
}

impl From<TypedHirRepr> for TypedHir {
    fn from(r: TypedHirRepr) -> Self {
        Self {
            store: TypeStore::from(r.store),
            interner: r.interner,
            top_level_funs: r.top_level_funs.into_iter().collect(),
            member_funs: r
                .member_funs
                .into_iter()
                .map(|(k, inner)| (k, inner.into_iter().collect()))
                .collect(),
            member_fun_order: r.member_fun_order.into_iter().collect(),
            members: r
                .members
                .into_iter()
                .map(|(k, inner)| (k, inner.into_iter().collect()))
                .collect(),
            member_order: r.member_order.into_iter().collect(),
            ctor_signatures: r.ctor_signatures.into_iter().collect(),
            top_level_vals: r.top_level_vals.into_iter().collect(),
            enum_variants: r.enum_variants.into_iter().collect(),
            type_constraints: r.type_constraints.into_iter().collect(),
            interface_fqns: r.interface_fqns.into_iter().collect(),
            class_fqns: r.class_fqns.into_iter().collect(),
            extensible_class_fqns: r.extensible_class_fqns.into_iter().collect(),
            direct_subtypes: r.direct_subtypes.into_iter().collect(),
            supertypes: r.supertypes.into_iter().collect(),
            class_ctor_params: r.class_ctor_params.into_iter().collect(),
            super_ctor_delegations: r.super_ctor_delegations.into_iter().collect(),
            type_infos: r.type_infos.into_iter().collect(),
            files: r.files,
        }
    }
}

impl serde::Serialize for TypedHir {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        TypedHirRepr::from(self).serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for TypedHir {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(TypedHir::from(TypedHirRepr::deserialize(deserializer)?))
    }
}
