//! 稳定身份（定义层）：canonical 类型文本编码 + [`StableDefKey`]（PLAN.md M0-1）。
//!
//! canonical 编码自 `scoop2_mir::stable_id` 上移：`TypeKind` → 跨构建稳定文本，
//! HIR / MIR 两层 key 构造共用同一编码，保证同一类型在两层身份体系中文本一致。
//!
//! 两套身份（PLAN.md C3，同纪律、不同 key 值）：
//!
//! - **定义身份**（本模块，[`StableDefKey`]）：指向「哪个声明」。跨 cone 引用、
//!   序列化、增量复用、lang-items 注册表用这一层。M2 起每个 element 铸造一个。
//! - **实例身份**（`scoop2_mir::transport::StableTemplateKey` / `StableInstanceKey`）：
//!   指向「哪个模板的哪组实参实例」。MIR 单态化产物（M3 起为 archive 的主键）。
//!
//! 重载消歧：函数命名空间是重载集，同名重载的稳定身份由
//! [`overload_disambiguation_key`]（签名 canonical 文本）区分——修复「重载集按
//! 声明序无稳定身份」的缺口（PLAN.md M0-2）。

use scoop2_base::{Interner, StableConeKey, StableHashScope, stable_hash};

use crate::hir::TypedSignature;
use crate::ty::{EffectRow, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

// ---------------------------------------------------------------------------
// canonical encoding（自 scoop2_mir 上移；编码规则不变，保证既有 stable key
// 字节一致）
// ---------------------------------------------------------------------------

/// canonical 编码深度上限（防环）。
///
/// 语言内递归类型只能经 nominal（按 FQN 浅编码）发生，正常类型深度远低于此；
/// 超限时输出 `?depth` 占位（保持与既有实现的字节兼容）。
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
        // String 现为 Ref(Nominal{scoop.core.String})，由 nominal arm 编码为 N(scoop.core.String)。
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
                format!(";eff={}", canonical_effect_row_text(types, interner, row))
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
            let row = canonical_effect_row_closed_text(types, interner, &f.effects, f.closed);
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
        TypeKind::Value(ValueTypeKind::Nominal(n)) if n.fqn == types.option_fqn() => {
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
pub fn canonical_effect_row_text(
    types: &TypeStore,
    interner: &Interner,
    row: &EffectRow,
) -> String {
    canonical_effect_row_closed_text(types, interner, row, false)
}

/// 把 effect row 编码为 canonical 文本（带闭合标记）。
pub fn canonical_effect_row_closed_text(
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
// 定义身份：StableDefKey
// ---------------------------------------------------------------------------

/// 声明所属命名空间（三命名空间模型，resolve/symbol.rs）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DefNamespace {
    /// 类型命名空间（class/interface/struct/enum/effect/object/typealias）。
    Type,
    /// 值命名空间（顶层 val/var）。
    Value,
    /// 函数命名空间（重载集）。
    Fun,
}

impl DefNamespace {
    /// 命名空间标签（进 canonical 文本）。
    pub fn as_str(self) -> &'static str {
        match self {
            DefNamespace::Type => "ty",
            DefNamespace::Value => "val",
            DefNamespace::Fun => "fun",
        }
    }
}

/// 声明种类（StableDefKey 的组成部分；M2 element 体系齐全后逐 kind 对接）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DefKind {
    /// 顶层函数。
    Function,
    /// 成员方法（owner 为类型 FQN）。
    Method,
    /// 构造器（primary / secondary）。
    Constructor,
    /// enum variant（owner 为 enum FQN）。
    EnumVariant,
    /// 字段 / 属性（owner 为类型 FQN）。
    Field,
    /// effect 操作（owner 为 effect FQN）。
    EffectOp,
    /// 顶层 val/var。
    Global,
    /// 类型声明（class/interface/struct/enum/effect）。
    TypeDecl,
    /// object 单例。
    Object,
    /// 扩展函数/属性（owner 为接收者 FQN）。
    Extension,
}

impl DefKind {
    /// 种类标签（进 canonical 文本）。
    pub fn as_str(self) -> &'static str {
        match self {
            DefKind::Function => "fn",
            DefKind::Method => "meth",
            DefKind::Constructor => "ctor",
            DefKind::EnumVariant => "variant",
            DefKind::Field => "field",
            DefKind::EffectOp => "effop",
            DefKind::Global => "global",
            DefKind::TypeDecl => "tydecl",
            DefKind::Object => "object",
            DefKind::Extension => "ext",
        }
    }
}

/// 定义身份：跨 cone / 跨构建 / 序列化 / 增量复用的元素稳定 key。
///
/// canonical 形如 `def(app::fun/pkg.f#fn/V(Int))`：
/// `{cone}::{namespace}/{owner}.{name}#{kind}` + 重载消歧后缀（仅 Fun 命名空间且
/// 非空时）。所有组成部分均为稳定文本；会话内 dense id 不进本 key。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StableDefKey {
    /// 所属 cone 的稳定 key。
    pub cone: StableConeKey,
    /// 命名空间。
    pub namespace: DefNamespace,
    /// owner 路径（成员声明为其类型 FQN 文本；顶层声明为空）。
    pub owner: String,
    /// simple name。
    pub name: String,
    /// 声明种类。
    pub kind: DefKind,
    /// 重载消歧 key（同 FQN 重载的签名 canonical 文本；非重载为空）。
    pub overload_key: String,
}

impl StableDefKey {
    /// canonical 文本（`path_hash` 的输入）。
    pub fn canonical(&self) -> String {
        let owner_prefix = if self.owner.is_empty() {
            String::new()
        } else {
            format!("{}.", self.owner)
        };
        let overload_suffix = if self.overload_key.is_empty() {
            String::new()
        } else {
            format!("/{}", self.overload_key)
        };
        format!(
            "def({}::{}/{}{}{}#{})",
            self.cone,
            self.namespace.as_str(),
            owner_prefix,
            self.name,
            overload_suffix,
            self.kind.as_str()
        )
    }

    /// 紧凑定长身份：scope 前缀 FNV-1a hex16。跨 cone 引用与 map key 用它，
    /// canonical 文本仅调试/诊断。
    pub fn path_hash(&self, scope: StableHashScope) -> String {
        stable_hash(scope, &self.canonical())
    }
}

/// 函数签名的重载消歧 key：`[p1,p2,...]->ret/E(row)`（vararg 参数列表尾缀 `*`）。
///
/// 同 FQN 重载靠它区分身份。与 MIR 层 `build_overload_sig`（仅参数类型）是
/// **不同的 key**：MIR 的 StableTemplateKey 为字节稳定不做变更；本 key 是 HIR
/// 定义身份专用，参数 + 返回 + effect 行 + vararg 全参与，消歧更强。
pub fn overload_disambiguation_key(
    sig: &TypedSignature,
    types: &TypeStore,
    interner: &Interner,
) -> String {
    let mut params: Vec<String> = sig
        .param_types
        .iter()
        .map(|&ty| canonical_type_text(types, interner, ty))
        .collect();
    if sig.has_vararg {
        if let Some(last) = params.last_mut() {
            *last = format!("{last}*");
        }
    }
    let ret = canonical_type_text(types, interner, sig.return_ty);
    let eff = canonical_effect_row_text(types, interner, &sig.effect_row);
    format!("[{}]->{ret}/{eff}", params.join(","))
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
    fn effect_row_pure_encodes_stable() {
        let store = TypeStore::new();
        let interner = Interner::new();
        let row = EffectRow::pure();
        let text = canonical_effect_row_text(&store, &interner, &row);
        assert_eq!(text, "Pure");
    }

    #[test]
    fn def_key_canonical_shape() {
        let key = StableDefKey {
            cone: StableConeKey::from_cone_name("app"),
            namespace: DefNamespace::Fun,
            owner: String::new(),
            name: String::from("f"),
            kind: DefKind::Function,
            overload_key: String::new(),
        };
        assert_eq!(key.canonical(), "def(app::fun/f#fn)");
        assert!(!key.path_hash(StableHashScope::DefPath).is_empty());
    }

    #[test]
    fn def_key_member_includes_owner() {
        let top = StableDefKey {
            cone: StableConeKey::from_cone_name("app"),
            namespace: DefNamespace::Fun,
            owner: String::new(),
            name: String::from("run"),
            kind: DefKind::Function,
            overload_key: String::new(),
        };
        let method = StableDefKey {
            owner: String::from("app.Shape"),
            name: String::from("run"),
            ..top.clone()
        };
        assert_ne!(top.canonical(), method.canonical());
        assert_ne!(
            top.path_hash(StableHashScope::DefPath),
            method.path_hash(StableHashScope::DefPath)
        );
    }

    #[test]
    fn overload_key_distinguishes_reloads() {
        let mut store = TypeStore::new();
        let interner = Interner::new();
        let int = store.int();
        let bool_ty = store.bool();
        let mk = |params: Vec<TypeId>| TypedSignature {
            param_types: params,
            return_ty: int,
            type_param_count: 0,
            param_names: vec![],
            has_defaults: vec![],
            default_exprs: vec![],
            effect_row: EffectRow::pure(),
            has_vararg: false,
            decl_span: scoop2_base::Span::new(0, 1),
            decl_file: scoop2_base::FileId(0),
        };
        let k1 = overload_disambiguation_key(&mk(vec![int]), &store, &interner);
        let k2 = overload_disambiguation_key(&mk(vec![int, bool_ty]), &store, &interner);
        assert_ne!(k1, k2);
        assert_eq!(k1, "[V(Int)]->V(Int)/Pure");
    }
}
