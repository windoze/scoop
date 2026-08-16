//! per-cone 符号/类型 id 空间（C2 / M2-6 收官）。
//!
//! 每个 cone archive 携带自己的**符号空间**与**类型空间**（本 cone 的
//! TypedFile 引用到的实体闭包），条目带稳定 key（符号 = 文本；类型 =
//! canonical 文本 hash）——**跨 cone 引用 = (ConeId, 稳定 key)**：两个 cone
//! 的空间里同一稳定 key 即同一实体，装配期按稳定 key 合并去重。
//!
//! 装配（重放）：全部空间的条目按 global id 升序重放进 merged interner /
//! store——hash-cons + 「组合类型只引用更小 id」的不变量保证重放精确复现
//! 写出会话的 id 分配（TypedFile 内嵌 id 因此保持有效；确定性由 C7 的
//! 同输入同序保证，版本头 + 稳定 key 冲突检查守护跨会话误用）。

use std::collections::{BTreeMap, BTreeSet};

use scoop2_base::{Interner, Symbol};
use scoop2_hir::hir::TypedFile;
use scoop2_hir::hir::tree::{TreeExprKind, TreePattern};
use scoop2_hir::stable_id::canonical_type_text;
use scoop2_hir::ty::{EffectRow, TypeId, TypeKind, TypeStore};

/// 符号空间条目：global id（写出会话）+ 稳定 key（文本——即跨 cone 身份）。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ConeSymbolEntry {
    pub global: u32,
    pub text: String,
}

/// 类型空间条目：global id + 稳定 key（canonical 文本 hash）+ 结构（重放用）。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ConeTypeEntry {
    pub global: u32,
    /// canonical 类型文本（稳定 key 的人类可读形态；hash 由装配器重算校验）。
    pub canonical: String,
    pub kind: TypeKind,
}

/// 一个 cone 的 id 空间（符号 + 类型；条目按 global id 升序）。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct ConeArena {
    pub symbols: Vec<ConeSymbolEntry>,
    pub types: Vec<ConeTypeEntry>,
}

/// 收集 TypedFile 引用的符号/类型（含类型结构闭包）。
pub fn collect_typed_file_usage(
    tf: &TypedFile,
    store: &TypeStore,
    interner: &Interner,
) -> (BTreeSet<Symbol>, BTreeSet<TypeId>) {
    let mut syms: BTreeSet<Symbol> = BTreeSet::new();
    let mut tys: BTreeSet<TypeId> = BTreeSet::new();

    // expr_types：全部值。
    for &ty in tf.expr_types.values() {
        tys.insert(ty);
    }

    // facts：各表的符号/类型字段。
    use scoop2_hir::hir::{ResolvedCall, ResolvedMember, ResolvedPlace};
    use scoop2_hir::resolve::output::ResolvedValue;
    for rv in tf.facts.value_refs.values() {
        match rv {
            ResolvedValue::Local { .. } => {}
            ResolvedValue::TopLevelValue { fqn } | ResolvedValue::TopLevelFun { fqn } => {
                syms.insert(*fqn);
            }
        }
    }
    for rc in tf.facts.call_resolutions.values() {
        match rc {
            ResolvedCall::TopLevelFun {
                fqn,
                explicit_type_args,
                inferred_type_args,
                param_types,
                return_ty,
                ..
            } => {
                syms.insert(*fqn);
                for t in explicit_type_args
                    .iter()
                    .chain(inferred_type_args)
                    .chain(param_types)
                {
                    tys.insert(*t);
                }
                tys.insert(*return_ty);
            }
            ResolvedCall::Method {
                receiver_ty,
                owner_fqn,
                method_name,
                explicit_type_args,
                param_types,
                ..
            } => {
                tys.insert(*receiver_ty);
                syms.insert(*owner_fqn);
                syms.insert(*method_name);
                for t in explicit_type_args.iter().chain(param_types) {
                    tys.insert(*t);
                }
            }
            ResolvedCall::Constructor { type_fqn, .. } => {
                syms.insert(*type_fqn);
            }
            ResolvedCall::EnumVariant {
                enum_fqn,
                variant_name,
                ..
            } => {
                syms.insert(*enum_fqn);
                syms.insert(*variant_name);
            }
            ResolvedCall::LocalValue { local_name, .. } => {
                syms.insert(*local_name);
            }
            ResolvedCall::FunValue { .. } | ResolvedCall::EffectOp { .. } => {}
        }
    }
    for rm in tf.facts.member_refs.values() {
        match rm {
            ResolvedMember::Field {
                owner_fqn,
                member_name,
                member_ty,
                ..
            } => {
                syms.insert(*owner_fqn);
                syms.insert(*member_name);
                tys.insert(*member_ty);
            }
            ResolvedMember::Method {
                receiver_ty,
                owner_fqn,
                method_name,
                ..
            } => {
                tys.insert(*receiver_ty);
                syms.insert(*owner_fqn);
                syms.insert(*method_name);
            }
            ResolvedMember::TupleIndex { .. } => {}
        }
    }
    for rp in tf.facts.assign_places.values() {
        match rp {
            ResolvedPlace::Local { name, local_ty } => {
                syms.insert(*name);
                tys.insert(*local_ty);
            }
            ResolvedPlace::TopLevelVar { fqn, ty } => {
                syms.insert(*fqn);
                tys.insert(*ty);
            }
            ResolvedPlace::MemberField {
                receiver_ty,
                owner_fqn,
                member_name,
                ..
            } => {
                tys.insert(*receiver_ty);
                syms.insert(*owner_fqn);
                syms.insert(*member_name);
            }
            ResolvedPlace::Index {
                receiver_ty,
                owner_fqn,
            } => {
                tys.insert(*receiver_ty);
                syms.insert(*owner_fqn);
            }
        }
    }
    for bs in tf.facts.pattern_bindings.values() {
        for b in bs {
            syms.insert(b.name);
            tys.insert(b.ty);
        }
    }
    for row in tf.facts.expr_effect_rows.values() {
        collect_effect_row(row, &mut tys);
    }
    for binders in tf.facts.handle_escape_binders.values() {
        for &(sym, ty) in binders {
            syms.insert(sym);
            tys.insert(ty);
        }
    }
    for &ty in tf.facts.type_ref_resolutions.values() {
        tys.insert(ty);
    }

    // trees：body 节点类型 + 局部/模式/骨架的符号与类型。
    use scoop2_hir::hir::tree::{TreeExprKind, TreePattern, TreeStmt};
    for tree in &tf.trees {
        for local in &tree.body.locals {
            syms.insert(local.name);
            tys.insert(local.ty);
        }
        for e in &tree.body.exprs {
            tys.insert(e.ty);
            collect_expr_kind(&e.kind, &mut syms, &mut tys);
        }
        for s in &tree.body.stmts {
            if let TreeStmt::Destructure { pat, .. } = s {
                collect_pattern(pat, &mut syms, &mut tys);
            }
        }
    }
    for entry in &tf.item_skeleton {
        if let Some(sym) = interner.get(&entry.fqn) {
            syms.insert(sym);
        }
    }

    // 类型闭包：TypeKind 内嵌引用（组合类型只引用更小 id——升序单遍即闭包）。
    close_type_closure(&mut tys, store);
    (syms, tys)
}

fn collect_effect_row(row: &EffectRow, tys: &mut BTreeSet<TypeId>) {
    for &t in &row.terms {
        tys.insert(t);
    }
}

fn collect_expr_kind(kind: &TreeExprKind, syms: &mut BTreeSet<Symbol>, tys: &mut BTreeSet<TypeId>) {
    use scoop2_hir::hir::tree::TreeExprKind;
    match kind {
        TreeExprKind::TopLevelValRef { fqn } => {
            syms.insert(*fqn);
        }
        TreeExprKind::StructLit { fqn, .. } => {
            let _ = fqn; // fqn 是文本——无需符号（arena 记录于事实侧）
        }
        TreeExprKind::Cast { target, .. } | TreeExprKind::TypeCheck { target, .. } => {
            tys.insert(*target);
        }
        _ => {}
    }
}

fn collect_pattern(pat: &TreePattern, syms: &mut BTreeSet<Symbol>, tys: &mut BTreeSet<TypeId>) {
    match pat {
        TreePattern::Binder { local, node_ty } => {
            tys.insert(*node_ty);
            let _ = local;
        }
        TreePattern::Is { ty } => {
            tys.insert(*ty);
        }
        TreePattern::Tuple(els) | TreePattern::Or(els) => {
            for e in els {
                collect_pattern(e, syms, tys);
            }
        }
        TreePattern::Variant { args, .. } => {
            for a in args {
                collect_pattern(a, syms, tys);
            }
        }
        TreePattern::Struct { fields } => {
            for f in fields {
                syms.insert(f.name);
                if let Some(sub) = &f.sub {
                    collect_pattern(sub, syms, tys);
                }
            }
        }
        _ => {}
    }
}

/// 类型闭包：升序遍历（BTreeSet）把 TypeKind 内嵌 TypeId 并入。
fn close_type_closure(tys: &mut BTreeSet<TypeId>, store: &TypeStore) {
    let mut work: Vec<TypeId> = tys.iter().copied().collect();
    while let Some(t) = work.pop() {
        let mut push = |x: TypeId| {
            if !tys.contains(&x) {
                tys.insert(x);
                work.push(x);
            }
        };
        match store.kind(t) {
            TypeKind::Ref(r) => match r {
                scoop2_hir::ty::RefTypeKind::Nominal(n) => {
                    for &a in &n.args {
                        push(a);
                    }
                    if let Some(eff) = &n.eff {
                        let _ = eff;
                    }
                }
                scoop2_hir::ty::RefTypeKind::Function(f) => {
                    for &p in &f.params {
                        push(p);
                    }
                    push(f.return_ty);
                    for &t in &f.effects.terms {
                        push(t);
                    }
                }
                scoop2_hir::ty::RefTypeKind::Union(u) => {
                    for &m in &u.variants {
                        push(m);
                    }
                }
                _ => {}
            },
            TypeKind::Value(v) => match v {
                scoop2_hir::ty::ValueTypeKind::Nominal(n) => {
                    for &a in &n.args {
                        push(a);
                    }
                }
                scoop2_hir::ty::ValueTypeKind::Tuple(els) => {
                    for &e in els {
                        push(e);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

/// 从收集结果构造 cone 的 id 空间。
pub fn build_cone_arena(
    syms: &BTreeSet<Symbol>,
    tys: &BTreeSet<TypeId>,
    store: &TypeStore,
    interner: &Interner,
) -> ConeArena {
    ConeArena {
        symbols: syms
            .iter()
            .map(|&s| ConeSymbolEntry {
                global: s.as_u32(),
                text: interner.resolve(s).to_string(),
            })
            .collect(),
        types: tys
            .iter()
            .map(|&t| ConeTypeEntry {
                global: t.0,
                canonical: canonical_type_text(store, interner, t),
                kind: store.kind(t).clone(),
            })
            .collect(),
    }
}

/// 装配重放：全部空间按 global id 升序重放进 merged interner/store。
///
/// 返回重放统计；冲突（同 global 不同实体）返回 Err（archive 损坏——装配
/// 是全管线唯一可失败解析点）。
pub fn replay_arenas(
    arenas: &[&ConeArena],
    cone_keys: &[String],
    interner: &mut Interner,
    store: &mut TypeStore,
) -> Result<(usize, usize), String> {
    // 符号：global → text（跨 cone 冲突检查）。
    let mut sym_map: BTreeMap<u32, String> = BTreeMap::new();
    for arena in arenas {
        for e in &arena.symbols {
            match sym_map.get(&e.global) {
                Some(prev) if *prev != e.text => {
                    return Err(format!(
                        "符号空间冲突：global {} 在不同 cone 中为 {prev:?} 与 {:?}",
                        e.global, e.text
                    ));
                }
                _ => {
                    sym_map.insert(e.global, e.text.clone());
                }
            }
        }
    }
    let mut interned = 0;
    for (global, text) in &sym_map {
        let got = interner.intern(text);
        if got.as_u32() != *global {
            return Err(format!(
                "符号重放漂移：global {} → intern {}（text={text:?}）——archive 跨会话失效",
                global,
                got.as_u32()
            ));
        }
        interned += 1;
    }

    // 类型：global → (canonical, kind)（跨 cone 稳定 key 一致性检查）。
    let mut ty_map: BTreeMap<u32, (String, TypeKind)> = BTreeMap::new();
    for (arena, key) in arenas.iter().zip(cone_keys) {
        for e in &arena.types {
            match ty_map.get(&e.global) {
                Some((prev_c, _)) if *prev_c != e.canonical => {
                    return Err(format!(
                        "类型空间冲突：global {} 在不同 cone（{key}）中 canonical 不一致",
                        e.global
                    ));
                }
                _ => {
                    ty_map.insert(e.global, (e.canonical.clone(), e.kind.clone()));
                }
            }
        }
    }
    let mut replayed = 0;
    for (global, (_, kind)) in &ty_map {
        // 升序重放：kind 内嵌 id 均小于 global（组合只引用更小 id）。
        store.replay_type(TypeId(*global), kind.clone());
        replayed += 1;
    }
    Ok((interned, replayed))
}

/// 共享段用量：声明表 + 未分区文件的符号/类型（写出时计算——共享段
/// interner/store 切分后，其表引用的实体由此迷你空间承载）。
pub fn collect_shared_usage(
    hir: &scoop2_hir::hir::TypedHir,
    unpartitioned: &[TypedFile],
    covered_syms: &BTreeSet<Symbol>,
    covered_tys: &BTreeSet<TypeId>,
) -> (BTreeSet<Symbol>, BTreeSet<TypeId>) {
    let mut syms: BTreeSet<Symbol> = BTreeSet::new();
    let mut tys: BTreeSet<TypeId> = BTreeSet::new();
    let mut sig = |s: &scoop2_hir::hir::TypedSignature| {
        for &t in &s.param_types {
            tys.insert(t);
        }
        tys.insert(s.return_ty);
    };
    for (_, sigs) in &hir.top_level_funs {
        for s in sigs {
            sig(s);
        }
    }
    for (_, ms) in &hir.member_funs {
        for (_, sigs) in ms {
            for s in sigs {
                sig(s);
            }
        }
    }
    for (_, sigs) in &hir.ctor_signatures {
        for s in sigs {
            sig(s);
        }
    }
    for (&k, &v) in &hir.top_level_vals {
        syms.insert(k);
        tys.insert(v);
    }
    for (k, vs) in &hir.enum_variants {
        syms.insert(*k);
        for &v in vs {
            syms.insert(v);
        }
    }
    for (k, inner) in &hir.members {
        syms.insert(*k);
        for (&n, &t) in inner {
            syms.insert(n);
            tys.insert(t);
        }
    }
    for (k, _) in &hir.class_ctor_params {
        syms.insert(*k);
    }
    for cp in hir.class_ctor_params.values() {
        for p in cp {
            tys.insert(p.ty);
        }
    }
    for (k, sd) in &hir.super_ctor_delegations {
        syms.insert(*k);
        syms.insert(sd.super_fqn);
        for &t in &sd.arg_tys {
            tys.insert(t);
        }
    }
    for (k, tc) in &hir.type_constraints {
        syms.insert(*k);
        for &n in &tc.type_params {
            syms.insert(n);
        }
    }
    for k in hir.interface_fqns.iter().chain(hir.class_fqns.iter()) {
        syms.insert(*k);
    }
    for k in &hir.extensible_class_fqns {
        syms.insert(*k);
    }
    for (k, subs) in &hir.direct_subtypes {
        syms.insert(*k);
        for &v in subs {
            syms.insert(v);
        }
    }
    for (k, supers) in &hir.supertypes {
        syms.insert(*k);
        for &v in supers {
            syms.insert(v);
        }
    }
    for (&t, _) in &hir.type_infos {
        tys.insert(t);
    }
    for e in &hir.elements.elements {
        syms.insert(e.fqn);
        syms.insert(e.name);
    }
    for f in unpartitioned {
        let (fs, ft) = collect_typed_file_usage(f, &hir.store, &hir.interner);
        syms.extend(fs);
        tys.extend(ft);
    }
    close_type_closure(&mut tys, &hir.store);
    // 补集：cone 空间未覆盖的 id（重放连续性——写出会话的 id 空间 0..N 连续，
    // 并集必须覆盖全部，否则装配无法精确复现）。
    for (sym, _) in hir.interner.iter_by_id() {
        if !covered_syms.contains(&sym) {
            syms.insert(sym);
        }
    }
    for id in 0..(hir.store.len() as u32) {
        let t = TypeId(id);
        if !covered_tys.contains(&t) && !tys.contains(&t) {
            tys.insert(t);
        }
    }
    (syms, tys)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym(global: u32, text: &str) -> ConeSymbolEntry {
        ConeSymbolEntry {
            global,
            text: text.to_string(),
        }
    }

    /// 跨 cone 稳定 key 冲突（同 global 不同文本）→ 装配拒绝。
    #[test]
    fn replay_rejects_cross_cone_symbol_conflict() {
        let a = ConeArena {
            symbols: vec![sym(2, "add")],
            types: vec![],
        };
        let b = ConeArena {
            symbols: vec![sym(2, "sub")],
            types: vec![],
        };
        let mut interner = Interner::new();
        let mut store = TypeStore::new();
        let err = replay_arenas(
            &[&a, &b],
            &["cone.a".into(), "cone.b".into()],
            &mut interner,
            &mut store,
        );
        assert!(err.is_err(), "同 global 不同文本应拒绝");
    }

    /// 重放精确复现：条目按 global 升序 intern → id 一致。
    #[test]
    fn replay_reproduces_session_ids() {
        let a = ConeArena {
            symbols: vec![sym(0, "alpha"), sym(2, "gamma")],
            types: vec![],
        };
        let b = ConeArena {
            symbols: vec![sym(1, "beta"), sym(2, "gamma")],
            types: vec![],
        };
        let mut interner = Interner::new();
        let mut store = TypeStore::new();
        replay_arenas(
            &[&a, &b],
            &["cone.a".into(), "cone.b".into()],
            &mut interner,
            &mut store,
        )
        .expect("重放成功");
        assert_eq!(interner.get("alpha").unwrap().as_u32(), 0);
        assert_eq!(interner.get("beta").unwrap().as_u32(), 1);
        assert_eq!(interner.get("gamma").unwrap().as_u32(), 2);
    }
}
