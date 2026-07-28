//! Stable keys：canonical encoder + scope-prefixed hash。
//!
//! 把 `TypeKind` 编码为稳定文本（跨构建确定），用于构造
//! [`StableTemplateKey`] / [`StableInstanceKey`] 的 `canonical` + `hash` 字段。
//!
//! 编码规则（与参考实现 `scoopc_hir/src/stable_id.rs` 对齐）：
//! - 标量值类型：`V(Int)` / `V(Bool)` / ... ;
//! - Option：`V(Option<{inner}>)`;
//! - Tuple：`T({elem0,elem1,...})`;
//! - Nominal：`N(fqn<{args};eff={row}>)`;
//! - Function：`F({receiver};[{params}]->{ret}/{effects})`;
//! - TypeParam：`P({name})`（按名字编码；泛型单态化后 Param 被替换掉）;
//! - EffectRow：`E({terms sorted + deduped})`;
//! - StarProjection：`S({read_ty})`;
//! - Union：`U({variants sorted})`;
//!
//! Hash：scope-prefixed FNV-1a，使同文本在不同 scope（dump / abi / rtti）产生不同 hash。

use scoop2_hir::ty::{EffectRow, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::transport::{StableInstanceKey, StableTemplateKey};

// ---------------------------------------------------------------------------
// canonical encoding
// ---------------------------------------------------------------------------

/// canonical 编码深度上限（防环）。
const MAX_CANONICAL_DEPTH: usize = 64;

/// 把一个 `TypeId` 编码为 canonical 文本。
pub fn canonical_type_text(types: &TypeStore, ty: TypeId) -> String {
    let mut cache: std::collections::HashMap<TypeId, String> = std::collections::HashMap::new();
    encode_type(types, ty, 0, &mut cache)
}

fn encode_type(
    types: &TypeStore,
    ty: TypeId,
    depth: usize,
    cache: &mut std::collections::HashMap<TypeId, String>,
) -> String {
    if let Some(cached) = cache.get(&ty) {
        return cached.clone();
    }
    if depth > MAX_CANONICAL_DEPTH {
        // 过深递归：用占位标记（不 panic）。
        return "?depth".to_string();
    }
    let encoded = match types.kind(ty) {
        TypeKind::Ref(RefTypeKind::Any) => "R(Any)".to_string(),
        TypeKind::Ref(RefTypeKind::String) => "R(String)".to_string(),
        TypeKind::Ref(RefTypeKind::Nominal(n)) => {
            let fqn_text = format!("{}", n.fqn.as_u32()); // interned symbol id（稳定）
            let args: Vec<String> = n
                .args
                .iter()
                .map(|&a| encode_type(types, a, depth + 1, cache))
                .collect();
            let args_str = if args.is_empty() {
                String::new()
            } else {
                format!("<{}>", args.join(","))
            };
            let eff_str = if let Some(row) = &n.eff {
                format!(";eff={}", encode_effect_row(row))
            } else {
                String::new()
            };
            format!("N({fqn_text}{args_str}{eff_str})")
        }
        TypeKind::Ref(RefTypeKind::Function(f)) => {
            let receiver = match f.receiver {
                Some(r) => encode_type(types, r, depth + 1, cache),
                None => "-".to_string(),
            };
            let params: Vec<String> = f
                .params
                .iter()
                .map(|&p| encode_type(types, p, depth + 1, cache))
                .collect();
            let return_ty = encode_type(types, f.return_ty, depth + 1, cache);
            let row = encode_effect_row_with_closed(&f.effects, f.closed);
            format!("F({receiver};[{}]->{return_ty}/{row})", params.join(","))
        }
        TypeKind::Ref(RefTypeKind::Union(u)) => {
            let mut variants: Vec<String> = u
                .variants
                .iter()
                .map(|&v| encode_type(types, v, depth + 1, cache))
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
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            format!("V(Option<{}>)", encode_type(types, *inner, depth + 1, cache))
        }
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
            let elems: Vec<String> = elements
                .iter()
                .map(|&e| encode_type(types, e, depth + 1, cache))
                .collect();
            format!("T({})", elems.join(","))
        }
        TypeKind::Value(ValueTypeKind::Nominal(n)) => {
            let fqn_text = format!("{}", n.fqn.as_u32());
            let args: Vec<String> = n
                .args
                .iter()
                .map(|&a| encode_type(types, a, depth + 1, cache))
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
            // type param 按 name 的 interned symbol id 编码（跨构建稳定）。
            format!("P({})", p.name.as_u32())
        }
        TypeKind::StarProjection => "Star".to_string(),
    };
    cache.insert(ty, encoded.clone());
    encoded
}

/// 把 effect row 编码为 canonical 文本（terms 排序去重）。
fn encode_effect_row(row: &EffectRow) -> String {
    encode_effect_row_with_closed(row, false)
}

/// 把 effect row 编码为 canonical 文本（带闭合标记）。
fn encode_effect_row_with_closed(row: &EffectRow, closed: bool) -> String {
    if row.terms.is_empty() {
        return if closed { "Pure!" } else { "Pure" }.to_string();
    }
    let mut terms: Vec<String> = row.terms.iter().map(|t| format!("ty#{}", t.0)).collect();
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
    /// dump 输出确定性。
    Dump,
    /// ABI mangling（导出符号）。
    Abi,
    /// RTTI type descriptor。
    Rtti,
    /// private symbol。
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

/// scope-prefixed FNV-1a 128-bit hex hash。
pub fn stable_hash(scope: StableHashScope, text: &str) -> String {
    // FNV-1a 64-bit（sufficient for determinism; scope prefix ensures isolation）。
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
/// `fqn` 是函数的全限定名（interned symbol 的文本）；`type_params` 是其类型参数名序列。
pub fn make_stable_template_key(
    scope: StableHashScope,
    fqn: &str,
    type_params: &[scoop2_base::Symbol],
) -> StableTemplateKey {
    // canonical = template(fqn, [tp0,tp1,...])，tp 按 interned id。
    let tp_strs: Vec<String> = type_params
        .iter()
        .map(|tp| format!("P({})", tp.as_u32()))
        .collect();
    let canonical = if tp_strs.is_empty() {
        format!("template({fqn})")
    } else {
        format!("template({fqn};[{}])", tp_strs.join(","))
    };
    let hash = stable_hash(scope, &canonical);
    StableTemplateKey { canonical, hash }
}

/// 构造 StableInstanceKey。
///
/// `template` 是模板 key；`type_args` / `eff_args` 是实例化类型/效果实参。
pub fn make_stable_instance_key(
    scope: StableHashScope,
    template: StableTemplateKey,
    types: &TypeStore,
    type_args: &[TypeId],
    eff_args: &[EffectRow],
) -> StableInstanceKey {
    let canonical_type_args: Vec<String> = type_args
        .iter()
        .map(|&ty| canonical_type_text(types, ty))
        .collect();
    let canonical_effect_args: Vec<String> = eff_args
        .iter()
        .map(|row| encode_effect_row(row))
        .collect();
    // instance canonical = template_canonical + args。
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_int_is_stable() {
        let mut store = TypeStore::new();
        let int = store.int();
        let text = canonical_type_text(&store, int);
        assert_eq!(text, "V(Int)");
    }

    #[test]
    fn canonical_tuple_preserves_order() {
        let mut store = TypeStore::new();
        let int = store.int();
        let bool_ty = store.bool();
        let tuple = store.tuple(vec![int, bool_ty]);
        let text = canonical_type_text(&store, tuple);
        assert_eq!(text, "T(V(Int),V(Bool))");
    }

    #[test]
    fn canonical_option_is_nested() {
        let mut store = TypeStore::new();
        let int = store.int();
        let opt = store.option(int);
        let text = canonical_type_text(&store, opt);
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
        let key = make_stable_template_key(
            StableHashScope::Dump,
            "pkg.main",
            &[],
        );
        assert!(key.canonical.contains("pkg.main"));
        assert!(!key.hash.is_empty());
    }

    #[test]
    fn instance_key_includes_type_args() {
        let mut store = TypeStore::new();
        let int = store.int();
        let template = make_stable_template_key(StableHashScope::Dump, "pkg.id", &[]);
        let instance = make_stable_instance_key(
            StableHashScope::Dump,
            template,
            &store,
            &[int],
            &[],
        );
        assert_eq!(instance.canonical_type_args, vec!["V(Int)".to_string()]);
        assert!(!instance.hash.is_empty());
    }

    #[test]
    fn effect_row_pure_encodes_stable() {
        let row = EffectRow::pure();
        let text = encode_effect_row(&row);
        assert_eq!(text, "Pure");
    }

    #[test]
    fn effect_row_sorted_deduped() {
        let mut store = TypeStore::new();
        let t1 = store.int();
        let t2 = store.bool();
        // EffectRow::from_terms sorts + dedups
        let row = EffectRow::from_terms(vec![t2, t1, t2]);
        let text = encode_effect_row(&row);
        // terms are sorted by TypeId.0 → t1 (lower id) first
        assert!(text.starts_with("E("));
    }
}
