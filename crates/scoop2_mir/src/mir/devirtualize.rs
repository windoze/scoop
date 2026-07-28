//! 去虚化 pass：final/单候选接收者的 Virtual/Interface 调用改写为 Direct。
//!
//! 在 materialize 的 rewrite 阶段之后运行。遍历 body 中每个 `CallKind::Virtual`/
//! `CallKind::Interface`，检查 `dispatch.receiver_ty` 对应的类型是否可去虚化。
//!
//! 去虚化条件（满足任一即可）：
//! - **final 类型**：值类型、Nothing、具体 class（非 open/abstract）→ 方法不可被 override。
//! - **单候选（CHA）**：即使 receiver 类型是 open/abstract/interface，若在当前编译单元中
//!   该类型只有一个具体子类/实现，且该子类没有 override 此方法，可退化为直接调用。
//!
//! final 判定（`is_final_type`）：
//! - 所有值类型（`TypeKind::Value(_)`）→ final；
//! - `Nothing`（bottom）→ final；
//! - 具体 class ref（非 open/abstract、非 interface）→ final。
//!
//! 单候选判定（`exact_receiver_fqn` + `descendants_and_self`）：
//! - `exact_receiver_fqn`：receiver 的 nominal ref FQN 不在 `direct_subtypes` 的 key 集合中
//!   （即无已知子类）→ 该类型是精确的，可去虚化。
//! - Interface 单候选：遍历实现该 interface 的所有具体 class，若恰好 1 个 → 去虚化。

use std::collections::{HashMap, HashSet, VecDeque};

use scoop2_base::{Interner, Symbol};
use scoop2_hir::ty::{RefTypeKind, TypeKind, TypeStore};

use crate::mir::{Body, CallKind, Module, Rvalue, StatementKind};

/// 去虚化上下文：携带 class 可继承性 + 子类层次信息。
pub struct DevirtContext<'a> {
    pub interner: &'a Interner,
    /// 所有 `open`/`abstract` class 的 FQN 集合（补集 = 具体 class）。
    pub extensible_class_fqns: &'a HashSet<Symbol>,
    /// 所有 interface 的 FQN 集合。
    pub interface_fqns: &'a HashSet<Symbol>,
    /// 超类型 → 直接子类型 FQN 列表（反转 supertypes）。
    /// key 集合 = 所有"有子类"的类型；receiver FQN 不在 key 中 = exact（无子类）。
    pub direct_subtypes: &'a HashMap<Symbol, Vec<Symbol>>,
}

impl<'a> DevirtContext<'a> {
    /// 判断一个 FQN 文本对应的 class 是否可继承（`open`/`abstract`）。
    fn is_extensible_class(&self, fqn_text: &str) -> bool {
        self.interner
            .get(fqn_text)
            .is_some_and(|sym| self.extensible_class_fqns.contains(&sym))
    }

    /// 判断一个 FQN 文本是否对应 interface 类型。
    fn is_interface(&self, fqn_text: &str) -> bool {
        self.interner
            .get(fqn_text)
            .is_some_and(|sym| self.interface_fqns.contains(&sym))
    }

    /// 判断一个 FQN 是否有已知子类（在 direct_subtypes 的 key 中）。
    fn has_known_subtypes(&self, fqn_text: &str) -> bool {
        self.interner
            .get(fqn_text)
            .is_some_and(|sym| self.direct_subtypes.contains_key(&sym))
    }

    /// 获取一个类型的所有直接子类型 FQN。
    fn direct_children(&self, fqn_sym: Symbol) -> &[Symbol] {
        self.direct_subtypes
            .get(&fqn_sym)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// BFS 收集 root 及所有后代（含 root 自身）。
    fn descendants_and_self(&self, root_fqn: Symbol) -> Vec<Symbol> {
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        let mut queue: VecDeque<Symbol> = VecDeque::new();
        queue.push_back(root_fqn);
        seen.insert(root_fqn);
        while let Some(fqn) = queue.pop_front() {
            result.push(fqn);
            for &child in self.direct_children(fqn) {
                if seen.insert(child) {
                    queue.push_back(child);
                }
            }
        }
        result
    }
}

/// 解析 receiver 类型到一个精确的 class FQN（无已知子类 → exact）。
///
/// 返回 `Some(fqn_sym)` 当且仅当 receiver 的静态类型是一个 nominal ref/value class，
/// 且该 class 不在 `direct_subtypes` 的 key 集合中（即无已知子类 → 运行期只可能是此类型）。
/// interface ref → None（interface 本身不精确）；有子类的 class → None。
fn exact_receiver_fqn(
    store: &TypeStore,
    ctx: &DevirtContext,
    ty: scoop2_hir::ty::TypeId,
) -> Option<Symbol> {
    match store.kind(ty) {
        TypeKind::Ref(RefTypeKind::Nominal(n)) => {
            let fqn_text = ctx.interner.resolve(n.fqn);
            // interface 不精确（有多个实现候选）。
            if ctx.is_interface(fqn_text) {
                return None;
            }
            // 有已知子类 → 不精确。
            if ctx.has_known_subtypes(fqn_text) {
                return None;
            }
            Some(n.fqn)
        }
        TypeKind::Value(scoop2_hir::ty::ValueTypeKind::Nominal(n)) => {
            // 值类型 nominal（struct/enum）不可继承 → 总是精确。
            Some(n.fqn)
        }
        _ => None,
    }
}

/// 判断一个类型是否为 final（不可有子类 → 虚方法可安全退化为直接调用）。
fn is_final_type(
    store: &TypeStore,
    ctx: &DevirtContext,
    ty: scoop2_hir::ty::TypeId,
) -> bool {
    match store.kind(ty) {
        TypeKind::Value(_) | TypeKind::Nothing => true,
        TypeKind::Ref(RefTypeKind::Nominal(n)) => {
            let fqn_text = ctx.interner.resolve(n.fqn);
            !ctx.is_extensible_class(fqn_text) && !ctx.is_interface(fqn_text)
        }
        _ => false,
    }
}

/// 对单个 body 执行去虚化。
pub fn devirtualize_body(store: &TypeStore, ctx: &DevirtContext, body: &mut Body) {
    for block in &mut body.blocks {
        for stmt in &mut block.stmts {
            if let StatementKind::Assign { value, .. } = &mut stmt.kind {
                devirtualize_rvalue(store, ctx, value);
            }
        }
    }
}

fn devirtualize_rvalue(store: &TypeStore, ctx: &DevirtContext, rv: &mut Rvalue) {
    match rv {
        Rvalue::Call { kind, .. } => {
            devirtualize_call_kind(store, ctx, kind);
        }
        _ => {}
    }
}

fn devirtualize_call_kind(store: &TypeStore, ctx: &DevirtContext, kind: &mut CallKind) {
    // Virtual（class vtable）和 Interface（itable）分发都可在满足条件时退化为 Direct。
    let (is_interface_dispatch, receiver_ty, owner_fqn, member_name) = match &kind {
        CallKind::Virtual { dispatch, .. } => (false, dispatch.receiver_ty, dispatch.owner_fqn.clone(), dispatch.member_name.clone()),
        CallKind::Interface { dispatch, .. } => (true, dispatch.receiver_ty, dispatch.owner_fqn.clone(), dispatch.member_name.clone()),
        _ => return,
    };

    // 提取 dispatch 的共享数据（在 clone 后操作，避免借用冲突）。
    let dispatch_data: Option<(crate::mir::transport::DispatchMetadata, Option<crate::mir::StableTemplateKey>)> = match &kind {
        CallKind::Virtual { dispatch, .. } | CallKind::Interface { dispatch, .. } => {
            Some((dispatch.clone(), dispatch.stable_template_key.clone()))
        }
        _ => None,
    };
    let Some((dispatch, stk)) = dispatch_data else { return };

    // 条件 1：final 类型 → 直接去虚化（用 member_fqn 或 owner.member）。
    let callee_fqn = if is_final_type(store, ctx, receiver_ty) {
        if dispatch.member_fqn.is_empty() {
            format!("{}.{}", owner_fqn, member_name)
        } else {
            dispatch.member_fqn.clone()
        }
    }
    // 条件 2：单候选 CHA（exact receiver）—— receiver 无已知子类。
    else if let Some(exact_fqn) = exact_receiver_fqn(store, ctx, receiver_ty) {
        let target_fqn_text = ctx.interner.resolve(exact_fqn);
        format!("{}.{}", target_fqn_text, member_name)
    }
    // 条件 3：Interface 单候选——遍历所有实现该 interface 的具体 class。
    else if is_interface_dispatch {
        if let Some(single_impl) = single_interface_impl_fqn(store, ctx, &owner_fqn) {
            let target_fqn_text = ctx.interner.resolve(single_impl);
            format!("{}.{}", target_fqn_text, member_name)
        } else {
            return; // 无法去虚化
        }
    } else {
        return; // 无法去虚化
    };

    let stable_instance_key = stk.as_ref().map(|stk| {
        crate::mir::stable_id::make_stable_instance_key(
            crate::mir::stable_id::StableHashScope::Dump,
            stk.clone(),
            store,
            ctx.interner,
            &dispatch.generic_type_args,
            &dispatch.generic_eff_args,
        )
    });
    *kind = CallKind::Direct {
        callee_fqn,
        type_args: dispatch.generic_type_args.clone(),
        is_intrinsic: false,
        stable_template_key: stk,
        stable_instance_key,
        generic_type_args: dispatch.generic_type_args.clone(),
        generic_eff_args: dispatch.generic_eff_args.clone(),
    };
}

/// 查找实现某 interface 的唯一具体 class（单候选）。
///
/// 遍历 `direct_subtypes` 找到所有继承该 interface 的 class，
/// 过滤掉有子类的（只保留 leaf class），若恰好 1 个 → 返回其 FQN。
fn single_interface_impl_fqn(
    _store: &TypeStore,
    ctx: &DevirtContext,
    interface_fqn_text: &str,
) -> Option<Symbol> {
    let iface_sym = ctx.interner.get(interface_fqn_text)?;
    // 收集所有继承该 interface 的 class（含传递子类）。
    let all_descendants = ctx.descendants_and_self(iface_sym);
    // 过滤到 leaf（无子类）的具体 class。
    let mut candidates: Vec<Symbol> = Vec::new();
    for &impl_fqn in &all_descendants {
        if impl_fqn == iface_sym {
            continue; // 跳过 interface 自身
        }
        let impl_text = ctx.interner.resolve(impl_fqn);
        // 跳过可继承的（open/abstract）——它们可能有子类 override。
        if ctx.is_extensible_class(impl_text) {
            continue;
        }
        candidates.push(impl_fqn);
    }
    // 恰好 1 个候选 → 单候选去虚化。
    if candidates.len() == 1 {
        Some(candidates[0])
    } else {
        None
    }
}

/// 对整个 Module 执行去虚化 pass。
pub fn devirtualize_module(module: &mut Module, ctx: &DevirtContext) {
    let store = &module.types;
    for item in &mut module.items {
        if let crate::mir::Item::Fun(fd) = item {
            if let Some(body) = &mut fd.body {
                devirtualize_body(store, ctx, body);
            }
        }
        if let crate::mir::Item::Initializer(ir) = item {
            devirtualize_body(store, ctx, &mut ir.body);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoop2_base::Interner;
    use scoop2_hir::ty::TypeStore;

    /// 空集合常量（供测试构造 DevirtContext 引用）。
    static EMPTY_EXTENSIBLE: std::sync::LazyLock<HashSet<scoop2_base::Symbol>> =
        std::sync::LazyLock::new(HashSet::new);
    static EMPTY_INTERFACE: std::sync::LazyLock<HashSet<scoop2_base::Symbol>> =
        std::sync::LazyLock::new(HashSet::new);
    static EMPTY_SUBTYPES: std::sync::LazyLock<HashMap<scoop2_base::Symbol, Vec<scoop2_base::Symbol>>> =
        std::sync::LazyLock::new(HashMap::new);

    /// 构造测试用 DevirtContext（空集合：无 interface、无可继承 class、无子类）。
    fn empty_ctx(interner: &Interner) -> DevirtContext<'_> {
        DevirtContext {
            interner,
            extensible_class_fqns: &EMPTY_EXTENSIBLE,
            interface_fqns: &EMPTY_INTERFACE,
            direct_subtypes: &EMPTY_SUBTYPES,
        }
    }

    #[test]
    fn value_types_are_final() {
        let interner = Interner::new();
        let ctx = empty_ctx(&interner);
        let mut store = TypeStore::new();
        let int = store.int();
        assert!(is_final_type(&store, &ctx, int));
        let bool_ty = store.bool();
        assert!(is_final_type(&store, &ctx, bool_ty));
        let unit = store.unit();
        assert!(is_final_type(&store, &ctx, unit));
        let opt = store.option(int);
        assert!(is_final_type(&store, &ctx, opt));
        let tup = store.tuple(vec![int, bool_ty]);
        assert!(is_final_type(&store, &ctx, tup));
    }

    #[test]
    fn ref_types_are_not_final() {
        let interner = Interner::new();
        let ctx = empty_ctx(&interner);
        let mut store = TypeStore::new();
        let str_ty = store.string();
        assert!(!is_final_type(&store, &ctx, str_ty));
        let any_ty = store.any();
        assert!(!is_final_type(&store, &ctx, any_ty));
    }

    #[test]
    fn nothing_is_final() {
        let interner = Interner::new();
        let ctx = empty_ctx(&interner);
        let mut store = TypeStore::new();
        let nothing = store.nothing();
        assert!(is_final_type(&store, &ctx, nothing));
    }

    #[test]
    fn concrete_class_ref_is_final() {
        // 具体 class（不在 extensible_class_fqns 中）→ final。
        let mut interner = Interner::new();
        let concrete_fqn = interner.intern("pkg.Concrete");
        let other_fqn = interner.intern("pkg.Other");
        let extensible: HashSet<_> = std::iter::once(concrete_fqn).collect();
        let interface: HashSet<_> = HashSet::new();
        let ctx = DevirtContext {
            interner: &interner,
            extensible_class_fqns: &extensible,
            interface_fqns: &interface,
            direct_subtypes: &EMPTY_SUBTYPES,
        };
        let mut store = TypeStore::new();
        // 构造一个具体 class nominal ref（不在 extensible 集合中）。
        let concrete_ref = store.ref_nominal(scoop2_hir::ty::NominalType {
            fqn: other_fqn,
            args: vec![],
            eff: None,
        });
        assert!(is_final_type(&store, &ctx, concrete_ref));
    }

    #[test]
    fn extensible_class_ref_is_not_final() {
        // open/abstract class（在 extensible_class_fqns 中）→ 非 final（可被继承/override）。
        let mut interner = Interner::new();
        let open_fqn = interner.intern("pkg.Open");
        let extensible: HashSet<_> = std::iter::once(open_fqn).collect();
        let interface: HashSet<_> = HashSet::new();
        let ctx = DevirtContext {
            interner: &interner,
            extensible_class_fqns: &extensible,
            interface_fqns: &interface,
            direct_subtypes: &EMPTY_SUBTYPES,
        };
        let mut store = TypeStore::new();
        let open_ref = store.ref_nominal(scoop2_hir::ty::NominalType {
            fqn: open_fqn,
            args: vec![],
            eff: None,
        });
        assert!(!is_final_type(&store, &ctx, open_ref));
    }

    #[test]
    fn interface_ref_is_not_final() {
        // interface 类型 → 非 final（itable 分发有多个实现候选）。
        let mut interner = Interner::new();
        let iface_fqn = interner.intern("pkg.IFace");
        let extensible: HashSet<_> = HashSet::new();
        let interface: HashSet<_> = std::iter::once(iface_fqn).collect();
        let ctx = DevirtContext {
            interner: &interner,
            extensible_class_fqns: &extensible,
            interface_fqns: &interface,
            direct_subtypes: &EMPTY_SUBTYPES,
        };
        let mut store = TypeStore::new();
        let iface_ref = store.ref_nominal(scoop2_hir::ty::NominalType {
            fqn: iface_fqn,
            args: vec![],
            eff: None,
        });
        assert!(!is_final_type(&store, &ctx, iface_ref));
    }

    #[test]
    fn exact_receiver_no_subtypes() {
        // receiver 是一个无子类的 class ref → exact_receiver_fqn 返回 Some。
        let mut interner = Interner::new();
        let leaf_fqn = interner.intern("pkg.Leaf");
        let subtypes: HashMap<_, Vec<_>> = HashMap::new(); // 无子类
        let ctx = DevirtContext {
            interner: &interner,
            extensible_class_fqns: &EMPTY_EXTENSIBLE,
            interface_fqns: &EMPTY_INTERFACE,
            direct_subtypes: &subtypes,
        };
        let mut store = TypeStore::new();
        let leaf_ref = store.ref_nominal(scoop2_hir::ty::NominalType {
            fqn: leaf_fqn,
            args: vec![],
            eff: None,
        });
        assert_eq!(exact_receiver_fqn(&store, &ctx, leaf_ref), Some(leaf_fqn));
    }

    #[test]
    fn exact_receiver_with_subtypes_is_none() {
        // receiver 是一个有子类的 class ref → exact_receiver_fqn 返回 None。
        let mut interner = Interner::new();
        let base_fqn = interner.intern("pkg.Base");
        let child_fqn = interner.intern("pkg.Child");
        let mut subtypes: HashMap<_, Vec<_>> = HashMap::new();
        subtypes.insert(base_fqn, vec![child_fqn]); // Base 有子类 Child
        let ctx = DevirtContext {
            interner: &interner,
            extensible_class_fqns: &EMPTY_EXTENSIBLE,
            interface_fqns: &EMPTY_INTERFACE,
            direct_subtypes: &subtypes,
        };
        let mut store = TypeStore::new();
        let base_ref = store.ref_nominal(scoop2_hir::ty::NominalType {
            fqn: base_fqn,
            args: vec![],
            eff: None,
        });
        assert_eq!(exact_receiver_fqn(&store, &ctx, base_ref), None);
    }

    #[test]
    fn descendants_and_self_traverses() {
        // Base → [Child1, Child2]，Child1 → [GrandChild]
        let mut interner = Interner::new();
        let base = interner.intern("pkg.Base");
        let child1 = interner.intern("pkg.Child1");
        let child2 = interner.intern("pkg.Child2");
        let grandchild = interner.intern("pkg.GrandChild");
        let mut subtypes: HashMap<_, Vec<_>> = HashMap::new();
        subtypes.insert(base, vec![child1, child2]);
        subtypes.insert(child1, vec![grandchild]);
        let ctx = DevirtContext {
            interner: &interner,
            extensible_class_fqns: &EMPTY_EXTENSIBLE,
            interface_fqns: &EMPTY_INTERFACE,
            direct_subtypes: &subtypes,
        };
        let desc = ctx.descendants_and_self(base);
        assert_eq!(desc.len(), 4); // Base + Child1 + Child2 + GrandChild
        assert!(desc.contains(&base));
        assert!(desc.contains(&child1));
        assert!(desc.contains(&child2));
        assert!(desc.contains(&grandchild));
    }
}
