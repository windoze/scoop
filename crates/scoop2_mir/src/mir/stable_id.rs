//! Stable keys：canonical encoder + scope-prefixed hash。
//!
//! 把 `TypeKind` 编码为稳定文本（跨构建确定），用于构造
//! [`StableTemplateKey`] / [`StableInstanceKey`] 的 `canonical` + `hash` 字段。
//!
//! 编码规则（与参考实现 `scoopc_hir/src/stable_id.rs` 对齐）：
//! - 标量值类型：`V(Int)` / `V(Bool)` / ... ;
//! - Option：`V(Option<{inner}>)`;
//! - Tuple：`T({elem0,elem1,...})`;
//! - Nominal：`N(fqn<{args};eff={row}>)` — FQN 用 interner 文本（稳定）;
//! - Function：`F({receiver};[{params}]->{ret}/{effects})`;
//! - TypeParam：`P({name})` — name 用 interner 文本（稳定）;
//! - EffectRow：`E({terms sorted + deduped})`;
//! - StarProjection：`S({read_ty})`;
//! - Union：`U({variants sorted})`;
//!
//! Hash：scope-prefixed FNV-1a，使同文本在不同 scope（dump / abi / rtti）产生不同 hash。

use scoop2_base::Interner;
use scoop2_hir::ty::{EffectRow, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::transport::{StableInstanceKey, StableTemplateKey};

// ---------------------------------------------------------------------------
// canonical encoding
// ---------------------------------------------------------------------------

/// canonical 编码深度上限（防环）。
const MAX_CANONICAL_DEPTH: usize = 64;

/// 把一个 `TypeId` 编码为 canonical 文本。
///
/// 使用 `interner` 把 `Symbol` 解析为 FQN 文本（跨构建稳定）。
pub fn canonical_type_text(types: &TypeStore, interner: &Interner, ty: TypeId) -> String {
    let mut cache: std::collections::HashMap<TypeId, String> = std::collections::HashMap::new();
    encode_type(types, interner, ty, 0, &mut cache)
}

fn encode_type(
    types: &TypeStore,
    interner: &Interner,
    ty: TypeId,
    depth: usize,
    cache: &mut std::collections::HashMap<TypeId, String>,
) -> String {
    if let Some(cached) = cache.get(&ty) {
        return cached.clone();
    }
    if depth > MAX_CANONICAL_DEPTH {
        return "?depth".to_string();
    }
    let encoded = match types.kind(ty) {
        TypeKind::Ref(RefTypeKind::Any) => "R(Any)".to_string(),
        TypeKind::Ref(RefTypeKind::String) => "R(String)".to_string(),
        TypeKind::Ref(RefTypeKind::Nominal(n)) => {
            let fqn_text = interner.resolve(n.fqn).to_string();
            let args: Vec<String> = n
                .args
                .iter()
                .map(|&a| encode_type(types, interner, a, depth + 1, cache))
                .collect();
            let args_str = if args.is_empty() {
                String::new()
            } else {
                format!("<{}>", args.join(","))
            };
            let eff_str = if let Some(row) = &n.eff {
                format!(";eff={}", encode_effect_row(types, interner, row))
            } else {
                String::new()
            };
            format!("N({fqn_text}{args_str}{eff_str})")
        }
        TypeKind::Ref(RefTypeKind::Function(f)) => {
            let receiver = match f.receiver {
                Some(r) => encode_type(types, interner, r, depth + 1, cache),
                None => "-".to_string(),
            };
            let params: Vec<String> = f
                .params
                .iter()
                .map(|&p| encode_type(types, interner, p, depth + 1, cache))
                .collect();
            let return_ty = encode_type(types, interner, f.return_ty, depth + 1, cache);
            let row = encode_effect_row_with_closed(types, interner, &f.effects, f.closed);
            format!("F({receiver};[{}]->{return_ty}/{row})", params.join(","))
        }
        TypeKind::Ref(RefTypeKind::Union(u)) => {
            let mut variants: Vec<String> = u
                .variants
                .iter()
                .map(|&v| encode_type(types, interner, v, depth + 1, cache))
                .collect();
            variants.sort();
            format!("U({})", variants.join(","))
        }
        TypeKind::Value(ValueTypeKind::Unit) => "V(Unit)".to_string(),
        TypeKind::Value(ValueTypeKind::Bool) => "V(Bool)".to_string(),
        TypeKind::Value(ValueTypeKind::Char) => "V(Char)".to_string(),
        TypeKind::Value(ValueTypeKind::Float64) => "V(Float64)".to_string(),
        TypeKind::Value(ValueTypeKind::Float32) => "V(Float32)".to_string(),
        TypeKind::Value(ValueTypeKind::Int) => "V(Int)".to_string(),
        TypeKind::Value(ValueTypeKind::UInt) => "V(UInt)".to_string(),
        TypeKind::Value(ValueTypeKind::IntN(bits)) => format!("V(Int{bits})"),
        TypeKind::Value(ValueTypeKind::UIntN(bits)) => format!("V(UInt{bits})"),
        // Option<T>：保持原编码 V(Option<inner>)（Option 现为 value nominal，走 FQN 判定）。
        TypeKind::Value(ValueTypeKind::Nominal(n))
            if n.fqn == types.option_fqn() =>
        {
            let inner = n.args.first().copied();
            match inner {
                Some(inner) => format!(
                    "V(Option<{}>)",
                    encode_type(types, interner, inner, depth + 1, cache)
                ),
                None => "V(Option<>)".to_string(),
            }
        }
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
            let elems: Vec<String> = elements
                .iter()
                .map(|&e| encode_type(types, interner, e, depth + 1, cache))
                .collect();
            format!("T({})", elems.join(","))
        }
        TypeKind::Value(ValueTypeKind::Nominal(n)) => {
            let fqn_text = interner.resolve(n.fqn).to_string();
            let args: Vec<String> = n
                .args
                .iter()
                .map(|&a| encode_type(types, interner, a, depth + 1, cache))
                .collect();
            let args_str = if args.is_empty() {
                String::new()
            } else {
                format!("<{}>", args.join(","))
            };
            format!("N({fqn_text}{args_str})")
        }
        TypeKind::Nothing => "Nothing".to_string(),
        TypeKind::Param(p) => {
            format!("P({})", interner.resolve(types.param_decl(*p).name))
        }
        TypeKind::StarProjection => "Star".to_string(),
    };
    cache.insert(ty, encoded.clone());
    encoded
}

/// 把 effect row 编码为 canonical 文本（terms 排序去重）。
/// 使用 interner 把每个 effect term 的 TypeId 编码为 canonical 类型文本（跨会话稳定）。
fn encode_effect_row(types: &TypeStore, interner: &Interner, row: &EffectRow) -> String {
    encode_effect_row_with_closed(types, interner, row, false)
}

/// 把 effect row 编码为 canonical 文本（带闭合标记）。
fn encode_effect_row_with_closed(
    types: &TypeStore,
    interner: &Interner,
    row: &EffectRow,
    closed: bool,
) -> String {
    if row.terms.is_empty() {
        return if closed { "Pure!" } else { "Pure" }.to_string();
    }
    let mut cache: std::collections::HashMap<TypeId, String> = std::collections::HashMap::new();
    let mut terms: Vec<String> = row
        .terms
        .iter()
        .map(|&t| encode_type(types, interner, t, 0, &mut cache))
        .collect();
    terms.sort();
    terms.dedup();
    let joined = terms.join(",");
    if closed {
        format!("E({joined})!")
    } else {
        format!("E({joined})")
    }
}

// ---------------------------------------------------------------------------
// scope-prefixed hash
// ---------------------------------------------------------------------------

/// Stable hash scope：使同文本在不同用途产生不同 hash。
#[derive(Clone, Copy, Debug)]
pub enum StableHashScope {
    Dump,
    Abi,
    Rtti,
    Private,
}

impl StableHashScope {
    fn prefix(self) -> &'static str {
        match self {
            StableHashScope::Dump => "dump",
            StableHashScope::Abi => "abi",
            StableHashScope::Rtti => "rtti",
            StableHashScope::Private => "priv",
        }
    }
}

/// scope-prefixed FNV-1a hex hash。
pub fn stable_hash(scope: StableHashScope, text: &str) -> String {
    let prefixed = format!("{}:{}", scope.prefix(), text);
    let mut h: u64 = 0xcbf29ce484222325;
    for b in prefixed.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}

// ---------------------------------------------------------------------------
// StableTemplateKey / StableInstanceKey construction
// ---------------------------------------------------------------------------

/// 构造 StableTemplateKey。
///
/// `fqn` 是函数的全限定名文本（稳定）。
/// `type_params` 是其类型参数名文本序列（稳定）。
/// `overload_sig` 是参数类型的 canonical 编码（用于区分同名重载）。
pub fn make_stable_template_key(
    scope: StableHashScope,
    fqn: &str,
    type_params: &[String],
    overload_sig: &str,
) -> StableTemplateKey {
    let tp_strs: Vec<String> = type_params.iter().map(|tp| format!("P({tp})")).collect();
    let canonical = if tp_strs.is_empty() && overload_sig.is_empty() {
        format!("template({fqn})")
    } else if tp_strs.is_empty() {
        format!("template({fqn};sig={overload_sig})")
    } else if overload_sig.is_empty() {
        format!("template({fqn};[{}])", tp_strs.join(","))
    } else {
        format!("template({fqn};[{}];sig={overload_sig})", tp_strs.join(","))
    };
    let hash = stable_hash(scope, &canonical);
    StableTemplateKey { canonical, hash }
}

/// 构造 StableInstanceKey。
pub fn make_stable_instance_key(
    scope: StableHashScope,
    template: StableTemplateKey,
    types: &TypeStore,
    interner: &Interner,
    type_args: &[TypeId],
    eff_args: &[EffectRow],
) -> StableInstanceKey {
    let canonical_type_args: Vec<String> = type_args
        .iter()
        .map(|&ty| canonical_type_text(types, interner, ty))
        .collect();
    let canonical_effect_args: Vec<String> = eff_args
        .iter()
        .map(|row| encode_effect_row(types, interner, row))
        .collect();
    let instance_canonical = format!(
        "{}/T[{}]/E[{}]",
        template.canonical,
        canonical_type_args.join(","),
        canonical_effect_args.join(",")
    );
    let hash = stable_hash(scope, &instance_canonical);
    StableInstanceKey {
        template,
        canonical_type_args,
        canonical_effect_args,
        hash,
    }
}

/// 从参数类型列表构建 overload signature canonical 文本。
pub fn build_overload_sig(
    types: &TypeStore,
    interner: &Interner,
    param_types: &[TypeId],
) -> String {
    param_types
        .iter()
        .map(|&ty| canonical_type_text(types, interner, ty))
        .collect::<Vec<_>>()
        .join(",")
}

/// 为模块中所有顶层函数（含闭包 invoke 函数）计算 stable template key。
///
/// 遍历 `Module.items` 中的 `Item::Fun`，对每个有 fqn 的函数：
/// 1. 从 type_params 构造类型参数名列表；
/// 2. 从 params 构造 overload signature（canonical param types）；
/// 3. 调用 `make_stable_template_key` 生成 key；
/// 4. 填充 `FunDecl.stable_template_key`。
///
/// 这确保即使函数未被调用（public API），也有 stable key 供分离编译使用。
pub fn compute_public_stable_keys(module: &mut crate::mir::Module, interner: &Interner) {
    let store = &module.types;
    for item in &mut module.items {
        if let crate::mir::Item::Fun(fd) = item {
            if fd.stable_template_key.is_none() {
                // 构造类型参数名列表（从 type_params TypeParamId → 经 store 侧表查 name → 文本）。
                let tp_names: Vec<String> = fd
                    .type_params
                    .iter()
                    .map(|&id| interner.resolve(store.param_decl(id).name).to_string())
                    .collect();
                // 构造 overload signature（canonical param types）。
                let param_types: Vec<TypeId> = fd.params.iter().map(|p| p.ty).collect();
                let overload_sig = build_overload_sig(store, interner, &param_types);
                let stk = make_stable_template_key(
                    StableHashScope::Dump,
                    &fd.fqn,
                    &tp_names,
                    &overload_sig,
                );
                fd.stable_template_key = Some(stk);
            }
            // 为函数体中的 ClassCtor / EnumVariant 计算 stable key。
            if let Some(body) = &mut fd.body {
                compute_rvalue_stable_keys(body, store, interner);
            }
        }
        if let crate::mir::Item::Initializer(ir) = item {
            compute_rvalue_stable_keys(&mut ir.body, store, interner);
        }
    }
}

/// 为 body 中所有 ClassCtor / EnumVariant 的 stable key 字段填充值。
fn compute_rvalue_stable_keys(body: &mut crate::mir::Body, store: &TypeStore, interner: &Interner) {
    for block in &mut body.blocks {
        for stmt in &mut block.stmts {
            if let crate::mir::StatementKind::Assign { value, .. } = &mut stmt.kind {
                fill_rvalue_stable_key(value, store, interner);
            }
        }
    }
}

/// 为单个 Rvalue 的 ClassCtor / EnumVariant stable key 填充（若为 None）。
fn fill_rvalue_stable_key(rv: &mut crate::mir::Rvalue, store: &TypeStore, interner: &Interner) {
    match rv {
        crate::mir::Rvalue::ClassCtor {
            ctor,
            args,
            type_fqn,
            ..
        } => {
            if ctor.stable_template_key.is_none() {
                let fqn_text = interner.resolve(*type_fqn).to_string();
                let param_types: Vec<TypeId> = args.iter().map(|a| a.value_ty).collect();
                let overload_sig = build_overload_sig(store, interner, &param_types);
                ctor.stable_template_key = Some(make_stable_template_key(
                    StableHashScope::Dump,
                    &fqn_text,
                    &[],
                    &overload_sig,
                ));
            }
        }
        crate::mir::Rvalue::EnumVariant {
            enum_fqn,
            variant_name,
            args,
            stable_key,
            ..
        } => {
            if stable_key.is_none() {
                let enum_text = interner.resolve(*enum_fqn);
                let variant_text = interner.resolve(*variant_name);
                let fqn = format!("{}.{}", enum_text, variant_text);
                let param_types: Vec<TypeId> = args.iter().map(|a| a.value_ty).collect();
                let overload_sig = build_overload_sig(store, interner, &param_types);
                *stable_key = Some(make_stable_template_key(
                    StableHashScope::Dump,
                    &fqn,
                    &[],
                    &overload_sig,
                ));
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_int_is_stable() {
        let mut store = TypeStore::new();
        let interner = Interner::new();
        let int = store.int();
        let text = canonical_type_text(&store, &interner, int);
        assert_eq!(text, "V(Int)");
    }

    #[test]
    fn canonical_tuple_preserves_order() {
        let mut store = TypeStore::new();
        let interner = Interner::new();
        let int = store.int();
        let bool_ty = store.bool();
        let tuple = store.tuple(vec![int, bool_ty]);
        let text = canonical_type_text(&store, &interner, tuple);
        assert_eq!(text, "T(V(Int),V(Bool))");
    }

    #[test]
    fn canonical_option_is_nested() {
        let mut store = TypeStore::new();
        let interner = Interner::new();
        let int = store.int();
        let opt = store.option(int);
        let text = canonical_type_text(&store, &interner, opt);
        assert_eq!(text, "V(Option<V(Int)>)");
    }

    #[test]
    fn stable_hash_is_deterministic() {
        let h1 = stable_hash(StableHashScope::Dump, "test");
        let h2 = stable_hash(StableHashScope::Dump, "test");
        assert_eq!(h1, h2);
    }

    #[test]
    fn stable_hash_scope_isolates() {
        let h_dump = stable_hash(StableHashScope::Dump, "test");
        let h_abi = stable_hash(StableHashScope::Abi, "test");
        assert_ne!(h_dump, h_abi);
    }

    #[test]
    fn template_key_includes_fqn() {
        let key = make_stable_template_key(StableHashScope::Dump, "pkg.main", &[], "");
        assert!(key.canonical.contains("pkg.main"));
        assert!(!key.hash.is_empty());
    }

    #[test]
    fn template_key_with_overload_sig() {
        let key_no_sig = make_stable_template_key(StableHashScope::Dump, "pkg.f", &[], "");
        let key_with_sig = make_stable_template_key(StableHashScope::Dump, "pkg.f", &[], "V(Int)");
        assert_ne!(key_no_sig.canonical, key_with_sig.canonical);
        assert_ne!(key_no_sig.hash, key_with_sig.hash);
    }

    #[test]
    fn instance_key_includes_type_args() {
        let mut store = TypeStore::new();
        let interner = Interner::new();
        let int = store.int();
        let template = make_stable_template_key(StableHashScope::Dump, "pkg.id", &[], "");
        let instance = make_stable_instance_key(
            StableHashScope::Dump,
            template,
            &store,
            &interner,
            &[int],
            &[],
        );
        assert_eq!(instance.canonical_type_args, vec!["V(Int)".to_string()]);
        assert!(!instance.hash.is_empty());
    }

    #[test]
    fn effect_row_pure_encodes_stable() {
        let store = TypeStore::new();
        let interner = Interner::new();
        let row = EffectRow::pure();
        let text = encode_effect_row(&store, &interner, &row);
        assert_eq!(text, "Pure");
    }
}
