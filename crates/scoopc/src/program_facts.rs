//! backend-agnostic 的编译单元共享事实。
//!
//! 当前先收口 effect/state-machine planning、higher-order suspendability summary 与后续
//! MIR/shared analysis 都会复用的一组稳定 side tables，避免继续在 LLVM codegen 内重复
//! 现场拼装同类 `HashMap` / `HashSet`。

use std::collections::{HashMap, HashSet};

use crate::hir;
use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

/// 由 HIR lowering 产出的稳定程序事实集合。
///
/// 这组 side tables 不依赖 LLVM builder / module / runtime ABI，可由任意 backend 或纯分析
/// 路径复用。当前先覆盖 effect/state-machine planning 与 suspendability summary 直接使用的
/// 事实，后续再按 `T5000c+` 继续扩展到更广的 mid-end 消费面。
#[derive(Debug, Clone, Default)]
pub(crate) struct ProgramFacts {
    pub(crate) ctor_call_targets: hir::CtorCallSiteIndex,
    pub(crate) continuation_resume_call_sites: hir::ContinuationResumeCallSiteIndex,
    pub(crate) non_pure_continuation_resume_call_sites: hir::NonPureContinuationResumeCallSiteIndex,
    pub(crate) top_level_value_tys: HashMap<String, TypeId>,
    pub(crate) fun_return_tys: HashMap<String, TypeId>,
    pub(crate) object_property_tys: HashMap<String, TypeId>,
    pub(crate) struct_field_tys: HashMap<String, HashMap<String, TypeId>>,
    pub(crate) class_field_tys: HashMap<String, HashMap<String, TypeId>>,
    pub(crate) class_super_keys: HashMap<String, String>,
    pub(crate) object_value_fqns: HashSet<String>,
    pub(crate) object_property_fqns: HashSet<String>,
    pub(crate) top_level_immutable_value_fqns: HashSet<String>,
}

impl ProgramFacts {
    /// 从 lowering 后的 HIR 一次性构造共享事实。
    ///
    /// 这里显式依赖 `LoweredHir` 的 side tables，而不是从 backend emitter 现场回捞数据，
    /// 以便后续让 LLVM codegen、effect summary 与 MIR pass 复用同一份事实层。
    #[allow(dead_code)]
    pub(crate) fn from_lowered(lowered: &hir::LoweredHir) -> Self {
        let top_level_value_tys = lowered
            .top_level_vars
            .iter()
            .map(|(fqn, var)| (fqn.clone(), var.ty))
            .chain(
                lowered
                    .top_level_consts
                    .iter()
                    .map(|(fqn, value)| (fqn.clone(), value.ty)),
            )
            .chain(
                lowered
                    .top_level_immutable_values
                    .iter()
                    .map(|(fqn, value)| (fqn.clone(), value.ty)),
            )
            .collect();
        let fun_return_tys = lowered
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                hir::Item::Fun(fun) => Some((fun.fqn.clone(), fun.return_ty)),
                _ => None,
            })
            .chain(
                lowered
                    .member_funs
                    .iter()
                    .map(|fun| (fun.fqn.clone(), fun.return_ty)),
            )
            .collect();
        let object_property_tys = lowered
            .object_inits
            .iter()
            .flat_map(|(owner_fqn, object_init)| {
                object_init
                    .properties
                    .iter()
                    .map(move |(name, property)| (format!("{owner_fqn}.{name}"), property.ty))
            })
            .collect();
        let struct_field_tys = lowered
            .struct_layouts
            .iter()
            .map(|(layout_key, layout)| {
                let fields = layout
                    .fields
                    .iter()
                    .filter_map(|field| field.ty.map(|ty| (field.fqn.clone(), ty)))
                    .collect::<HashMap<_, _>>();
                (layout_key.clone(), fields)
            })
            .collect();
        let class_field_tys = lowered
            .class_inits
            .iter()
            .map(|(layout_key, class)| {
                let fields = class
                    .fields
                    .iter()
                    .map(|field| (field.fqn.clone(), field.ty))
                    .collect::<HashMap<_, _>>();
                (layout_key.clone(), fields)
            })
            .collect();
        let class_super_keys = lowered
            .class_inits
            .iter()
            .filter_map(|(layout_key, class)| {
                class
                    .super_class_fqn
                    .clone()
                    .map(|super_key| (layout_key.clone(), super_key))
            })
            .collect();
        let object_value_fqns = lowered.object_inits.keys().cloned().collect();
        let object_property_fqns = lowered
            .object_inits
            .iter()
            .flat_map(|(owner_fqn, object_init)| {
                object_init
                    .properties
                    .keys()
                    .map(|name| format!("{owner_fqn}.{name}"))
                    .collect::<Vec<_>>()
            })
            .collect();
        let top_level_immutable_value_fqns =
            lowered.top_level_immutable_values.keys().cloned().collect();

        Self {
            ctor_call_targets: lowered.ctor_call_sites.clone(),
            continuation_resume_call_sites: lowered.continuation_resume_call_sites.clone(),
            non_pure_continuation_resume_call_sites: lowered
                .non_pure_continuation_resume_call_sites
                .clone(),
            top_level_value_tys,
            fun_return_tys,
            object_property_tys,
            struct_field_tys,
            class_field_tys,
            class_super_keys,
            object_value_fqns,
            object_property_fqns,
            top_level_immutable_value_fqns,
        }
    }

    /// 查询 top-level `var` / `const` / `immutable val` 的 concrete type。
    pub(crate) fn top_level_value_ty(&self, fqn: &str) -> Option<TypeId> {
        self.top_level_value_tys.get(fqn).copied()
    }

    /// 查询 object property 的 concrete type。
    pub(crate) fn object_property_ty(&self, fqn: &str) -> Option<TypeId> {
        self.object_property_tys.get(fqn).copied()
    }

    /// 查询已知函数或方法的声明返回类型。
    pub(crate) fn fun_return_ty(&self, fqn: &str) -> Option<TypeId> {
        self.fun_return_tys.get(fqn).copied()
    }

    /// 在 exact nominal receiver 已知时，解析其字段的 concrete type。
    pub(crate) fn resolve_nominal_field_ty(
        &self,
        types: &TypeStore,
        receiver_ty: TypeId,
        field_fqn: &str,
    ) -> Option<TypeId> {
        self.resolve_struct_field_ty(types, receiver_ty, field_fqn)
            .or_else(|| self.resolve_class_field_ty(types, receiver_ty, field_fqn))
    }

    fn resolve_struct_field_ty(
        &self,
        types: &TypeStore,
        receiver_ty: TypeId,
        field_fqn: &str,
    ) -> Option<TypeId> {
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = types.kind(receiver_ty) else {
            return None;
        };
        let layout_key = hir::mangle_nominal_fqn(&nominal.fqn, &nominal.args, types);
        self.struct_field_tys
            .get(&layout_key)
            .and_then(|fields| fields.get(field_fqn).copied())
            .or_else(|| {
                (layout_key != nominal.fqn)
                    .then(|| {
                        self.struct_field_tys
                            .get(&nominal.fqn)
                            .and_then(|fields| fields.get(field_fqn).copied())
                    })
                    .flatten()
            })
    }

    fn resolve_class_field_ty(
        &self,
        types: &TypeStore,
        receiver_ty: TypeId,
        field_fqn: &str,
    ) -> Option<TypeId> {
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = types.kind(receiver_ty) else {
            return None;
        };
        let layout_key = hir::mangle_nominal_fqn(&nominal.fqn, &nominal.args, types);
        self.lookup_class_field_ty_by_key(&layout_key, field_fqn)
            .or_else(|| {
                (layout_key != nominal.fqn)
                    .then(|| self.lookup_class_field_ty_by_key(&nominal.fqn, field_fqn))
                    .flatten()
            })
    }

    fn lookup_class_field_ty_by_key(&self, class_key: &str, field_fqn: &str) -> Option<TypeId> {
        if let Some(ty) = self
            .class_field_tys
            .get(class_key)
            .and_then(|fields| fields.get(field_fqn).copied())
        {
            return Some(ty);
        }
        self.class_super_keys
            .get(class_key)
            .and_then(|super_key| self.lookup_class_field_ty_by_key(super_key, field_fqn))
    }
}
