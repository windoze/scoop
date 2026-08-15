//! 类型表示（[`TypeId`] / [`TypeStore`] / [`TypeKind`] / [`EffectRow`] …）。
//!
//! 本模块是 scoop2 语义阶段共用的类型存储与核心代数，**只依赖 `scoop2_base`**
//!（不依赖 resolve/typecheck 的任何类型）。它对应 spec 的类型宇宙（P2 §1–§3、
//! §5.4、§6、§9；effect 行 P4 §4）。
//!
//! 设计要点：
//!
//! - **hash-consing**：[`TypeStore::intern`] 对结构同构的 [`TypeKind`] 返回同一
//!   [`TypeId`]，所以类型相等性比较退化为整数比较。
//! - **引用 vs 值的结构性区分**：[`TypeKind::Ref`] / [`TypeKind::Value`] 两个分支
//!   把 nominal 类型按声明类别（class/interface/object → 引用；struct/enum → 值）
//!   分到不同分支。这使得「某类型是否引用类型」可**无需查表**地由
//!   [`TypeStore::is_reference`] / [`TypeStore::is_value`] 回答——这对 spec 的
//!   「变体子类型仅对引用类型生效」(P2 §9.1) 至关重要。`Nothing` 既非引用也非值，
//!   单独列为 [`TypeKind::Nothing`]。
//! - **effect 行是集合**：[`EffectRow`] 是规范化（排序去重）的具体 effect 类型 id
//!   集合，外加至多一个多态行变量（[`EffectTail`]，对应声明级 `<eff E>`）；
//!   `Pure` = 空集且无 tail；`+` 为并（幂等/交换/结合）；`⊆` 为子集（双指针 +
//!   tail 包含规则）。generic effect 不变（P4 §4）。闭合行标记 `/ R!` 不挂在行上，
//!   而挂在 [`FunctionType::closed`] 上（闭合性是函数标注的属性，P4 §4.3）。
//! - **类型参数按全局唯一 id 识别**：[`TypeParamId`] 在声明点由
//!   [`TypeStore::mint_param`] 分配一次，不可伪造、不可重复（同名 `T` 在不同声明
//!   中是不同的参数，P3 §17）。元数据（名字/位置/变型/约束/种类）存于
//!   [`TypeParamDecl`] 侧表，不参与身份。结构替换 [`Subst`]（普通参数 → `TypeId`）
//!   与 [`EffSubst`]（effect 行参数 → `EffectRow`，值域不同故分表）均按 id 键，
//!   [`TypeStore::apply_subst_full`] 遍历所有类型/effect 位置。
//!
//! `MonoTypeId` / `as_mono`（codegen 边界的「无 Param 泄漏」不变量）在本阶段**不建
//! 模**——它服务于 codegen，而当前交付物止于 typecheck；现在实现会是死代码。
//! 待新后端出现时再补。

use std::collections::HashMap;

use scoop2_base::{FileId, Span, Symbol};

// ---------------------------------------------------------------------------
// 身份与核心结构体
// ---------------------------------------------------------------------------

/// 已 intern 的类型句柄：[`TypeStore`] 内 `Vec` 的下标。
///
/// 只在产出它的 [`TypeStore`] 内有意义；比较与哈希是 O(1) 整数操作。
#[derive(
    Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct TypeId(pub u32);

impl std::fmt::Debug for TypeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ty#{}", self.0)
    }
}

/// 全局唯一的类型参数身份：声明时由 [`TypeStore::mint_param`] 分配一次，
/// 不可伪造、不可重复（同名 `T` 在不同声明中是不同的参数，P3 §17）。
///
/// 用作 [`TypeKind::Param`] 的载体和 [`Subst`] / [`EffSubst`] 的替换键。
/// 名字/位置/约束等元数据存于 [`TypeParamDecl`] 侧表（通过
/// [`TypeStore::param_decl`] 查询），**不参与身份判定**。
#[derive(
    Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct TypeParamId(pub u32);

impl std::fmt::Debug for TypeParamId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tp#{}", self.0)
    }
}

/// 类型参数的种类：普通类型参数 `<T>` vs effect 行参数 `<eff E>`。
///
/// 两者共享 [`TypeParamId`] 身份体系，但替换值域不同——普通参数替换为一个
/// [`TypeId`]（见 [`Subst`]），effect 行参数替换为一个完整 [`EffectRow`]（见
/// [`EffSubst`]），对应其出现在 [`EffectRow::tail`] 上。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub enum TypeParamKind {
    /// 普通类型参数 `<T: bound>`。
    Type,
    /// effect 行参数 `<eff E = Pure>`（每声明至多一个，必须为最后一项）。
    Effect,
}

/// 声明一个类型参数时的完整元数据，存于 [`TypeStore::param_decls`] 侧表。
///
/// 身份由 [`TypeParamDecl::id`]（[`TypeParamId`]）唯一确定；其余字段仅供诊断、
/// 渲染与（将来的）协变/逆变子类型判定使用。
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TypeParamDecl {
    pub id: TypeParamId,
    /// 参数名，仅诊断/显示（不参与身份）。
    pub name: Symbol,
    pub span: Span,
    pub file: FileId,
    /// 变型（`in`/`out`），从 AST 带下；当前仅存储，子类型派发尚未启用。
    pub variance: Option<Variance>,
    /// 声明 bound（降级后的类型；无 bound 为 `None`）。
    pub bound: Option<TypeId>,
    /// 普通类型参数 vs effect 行参数。
    pub kind: TypeParamKind,
}

/// 类型参数的变型（`in` = 逆变 / `out` = 协变；spec §5.1 `typeParam`）。
///
/// 与 AST 的 [`crate::syntax::ast::Variance`] 同构；独立定义以免 `ty` 模块
/// 反向依赖 `scoop2_syntax::ast`。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub enum Variance {
    In,
    Out,
}

/// effect 行的尾部：至多一个多态行变量（`<eff E>`）。
///
/// - [`EffectTail::Empty`]：闭合的具体行（如 `Pure`、`IO + State`）；
/// - [`EffectTail::Var(id)`]：尾部是抽象行变量 `E`，代表「E 可能含的任意 effect 集」。
///
/// 每个声明至多一个 `<eff E>`（由 parser 强制其为 `TypeParamList` 最后一项），
/// 故 tail 永远是 `Option`，而非列表。
#[derive(Clone, PartialEq, Eq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub enum EffectTail {
    /// 闭合行：可观测 effect 恰为 `terms`。
    Empty,
    /// 行变量 `E`：`terms ∪ E`（E 为任意 effect 集）。
    Var(TypeParamId),
}

impl Default for EffectTail {
    fn default() -> Self {
        EffectTail::Empty
    }
}

/// Effect 行：规范化（按 [`TypeId`] 排序去重）的具体 effect 集合，外加至多一个
/// 多态行变量 [`EffectTail`]。
///
/// - `Pure` ≡ `terms` 为空且 `tail` 为 [`EffectTail::Empty`]；
/// - `+`（并）幂等 / 交换 / 结合，由 [`EffectRow::union`] 维持规范形式；
/// - `⊆`（子集）由 [`EffectRow::is_subset_of`] 判定，含 tail 的行变量包含规则；
/// - generic effect 不变：`Emit<Any>` 与 `Emit<String>` 互不蕴含（P4 §4）。
///
/// `<eff E>`（声明级 effect 行参数）体现为 `tail = Var(E 的 TypeParamId)`，而**不再**
/// 伪装成 `terms` 里的一个 `Param` term——它代表「一整组抽象 effect」，语义与单个
/// 具体 effect 不同。
///
/// 闭合标记 `/ R!` 不在此处，而在 [`FunctionType::closed`]。
#[derive(Clone, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub struct EffectRow {
    pub terms: Vec<TypeId>,
    pub tail: EffectTail,
}

impl EffectRow {
    /// 构造一个闭合行并规范化 `terms`（tail 默认 [`EffectTail::Empty`]）。
    pub fn from_terms(mut terms: Vec<TypeId>) -> Self {
        terms.sort_unstable();
        terms.dedup();
        EffectRow {
            terms,
            tail: EffectTail::Empty,
        }
    }

    /// 构造一个带行变量的行（`terms ∪ E`），并规范化 `terms`。
    pub fn from_terms_with_tail(mut terms: Vec<TypeId>, tail: EffectTail) -> Self {
        terms.sort_unstable();
        terms.dedup();
        EffectRow { terms, tail }
    }

    /// 把一个闭合行的 tail 替换为 `tail`（不改 terms）。
    pub fn with_tail(mut self, tail: EffectTail) -> Self {
        self.tail = tail;
        self
    }

    /// `Pure`（空行，无 tail）。
    pub fn pure() -> Self {
        EffectRow {
            terms: Vec::new(),
            tail: EffectTail::Empty,
        }
    }

    /// 单元素闭合行。
    pub fn single(term: TypeId) -> Self {
        EffectRow {
            terms: vec![term],
            tail: EffectTail::Empty,
        }
    }

    /// 是否为 `Pure`（无具体 effect 且无行变量）。
    pub fn is_pure(&self) -> bool {
        self.terms.is_empty() && matches!(self.tail, EffectTail::Empty)
    }

    /// `self + other`（并），结果规范化。
    ///
    /// tail 合并策略：
    /// - 两边都 [`EffectTail::Empty`] → 结果 tail 为 `Empty`；
    /// - 一边 `Empty`、另一边 `Var(v)` → 结果 tail 为 `Var(v)`（吸收）；
    /// - 两边 `Var(v)` 且 **同一 id** → 结果 tail 为 `Var(v)`；
    /// - 两边 `Var` 但 id **不同** → 行变量不可合并，结果退化为闭合行（tail=`Empty`）。
    ///   这是保守的：`E + F`（两个独立行变量）的确切并集无法用单一 tail 表达，
    ///   保守退化为「丢弃行变量」，对应 typecheck 的宽松判定（不会误判不合法程序为合法）。
    pub fn union(&self, other: &EffectRow) -> EffectRow {
        let mut terms = self.terms.clone();
        terms.extend_from_slice(&other.terms);
        let tail = match (&self.tail, &other.tail) {
            (EffectTail::Empty, EffectTail::Empty) => EffectTail::Empty,
            (EffectTail::Empty, v @ EffectTail::Var(_)) => v.clone(),
            (v @ EffectTail::Var(_), EffectTail::Empty) => v.clone(),
            (EffectTail::Var(a), EffectTail::Var(b)) if a == b => EffectTail::Var(*a),
            // 两个不同行变量并集无法用单 tail 表达 → 保守闭合。
            (EffectTail::Var(_), EffectTail::Var(_)) => EffectTail::Empty,
        };
        EffectRow::from_terms_with_tail(terms, tail)
    }

    /// `self ⊆ other`？
    ///
    /// - `terms` 部分用双指针判定（两边均规范有序）；
    /// - tail 规则（行包含）：
    ///   - other.tail = `Empty` ⇒ 仅当 self.tail 也为 `Empty` 且 terms 子集成立；
    ///   - other.tail = `Var(v)` ⇒ self.tail 可为 `Empty` 或同一 `Var(v)`，
    ///     且 terms 子集成立（Var 可吸收任意具体 effect）。
    ///
    /// 满足：`Pure ⊆ R`、`R ⊆ R + S`、`R ⊆ R`、`R ⊆ R + E`。
    pub fn is_subset_of(&self, other: &EffectRow) -> bool {
        // terms 子集判定（双指针）。
        let mut i = 0usize;
        let mut j = 0usize;
        while i < self.terms.len() && j < other.terms.len() {
            match self.terms[i].cmp(&other.terms[j]) {
                std::cmp::Ordering::Less => return false,
                std::cmp::Ordering::Equal => {
                    i += 1;
                    j += 1;
                }
                std::cmp::Ordering::Greater => j += 1,
            }
        }
        if i != self.terms.len() {
            // self 含有 other 没有的具体 term。
            return false;
        }
        // terms 子集成立；判定 tail。
        match (&self.tail, &other.tail) {
            (EffectTail::Empty, _) => true,
            (EffectTail::Var(a), EffectTail::Var(b)) => a == b,
            // self 有行变量但 other 没有 → 不可包含（other 闭合无法吸收任意 effect）。
            (EffectTail::Var(_), EffectTail::Empty) => false,
        }
    }

    pub fn equals(&self, other: &EffectRow) -> bool {
        self.terms == other.terms && self.tail == other.tail
    }

    /// 是否含某 effect 项（`terms` 规范有序，用二分）。
    pub fn contains(&self, term: TypeId) -> bool {
        self.terms.binary_search(&term).is_ok()
    }

    /// 差集：`self − other`（从 self.terms 中移除 other.terms 中存在的项）。
    ///
    /// **不动 tail**：差集只对具体 effect 有意义；行变量保持原样。
    /// （`Handle` 语义：handled 是具体 effect，body 的行变量不被 handle 消除。）
    /// self.terms 已排序去重，过滤后仍有序，无需再规范化。
    pub fn difference(&self, other: &EffectRow) -> EffectRow {
        let result: Vec<TypeId> = self
            .terms
            .iter()
            .filter(|t| !other.contains(**t))
            .copied()
            .collect();
        EffectRow {
            terms: result,
            tail: self.tail.clone(),
        }
    }
}

impl std::fmt::Debug for EffectRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("EffectRow");
        d.field("terms", &self.terms.iter().map(|t| t.0).collect::<Vec<_>>());
        if let EffectTail::Var(id) = &self.tail {
            d.field("tail", id);
        }
        d.finish()
    }
}

/// nominal 类型的一次出现（class/interface/struct/enum/object）。
///
/// `fqn` 是全限定点分名（interned），用作 nominal 身份键；具体声明信息（成员、
/// 超类型、kind）由 resolve/typecheck 按 fqn 在 Index/TypeEnv 中查得。`eff` 是
/// use-site effect-row 实参（仅对带 `<eff E>` 的 effect 类型有意义）。
#[derive(Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct NominalType {
    pub fqn: Symbol,
    pub args: Vec<TypeId>,
    pub eff: Option<EffectRow>,
}

/// 函数类型 `(P0, .., Pn) -> R / Row [!]`（spec P2 §11、P3 §6.1）。
///
/// 子类型规则（在 typecheck 的 assignability 中实现）：参数逆变 + 返回协变 +
/// effect 行放宽（`R1 ⊆ R2 ⇒ (A)->B/R1 <: (A)->B/R2`）。
#[derive(Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct FunctionType {
    /// 接收者函数类型 `T.(A) -> R` 的 `T`；普通函数为 `None`。
    pub receiver: Option<TypeId>,
    pub params: Vec<TypeId>,
    pub return_ty: TypeId,
    pub effects: EffectRow,
    /// `/ R!` 闭合行：保证可观测效果不超过 `effects`（P4 §4.3）。
    pub closed: bool,
}

/// 并类型（分支 LUB 细化用），规范形式（见 [`UnionType::from_variants`]）。
///
/// 主要用于引用类型的 LUB；值类型若无公共上界需显式装箱，不进并类型。
#[derive(Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct UnionType {
    pub variants: Vec<TypeId>,
}

impl UnionType {
    /// 由一组成员构造规范并类型：展平嵌套并、丢弃 `Nothing`、去重、排序。
    ///
    /// 注意：`Any` 吸收（任一成员为 `Any` 则整体退化为 `Any`）是 lattice 使用点的
    /// 判断（branch_merge，typecheck 阶段），这里只负责规范化结构。
    pub fn from_variants(store: &TypeStore, variants: Vec<TypeId>) -> UnionType {
        let mut flat: Vec<TypeId> = Vec::new();
        for v in variants {
            match store.kind(v) {
                TypeKind::Ref(RefTypeKind::Union(inner)) => flat.extend_from_slice(&inner.variants),
                TypeKind::Nothing => {}
                _ => flat.push(v),
            }
        }
        flat.sort_unstable();
        flat.dedup();
        UnionType { variants: flat }
    }
}

// ---------------------------------------------------------------------------
// TypeKind
// ---------------------------------------------------------------------------

/// 一切类型的顶层分类。
#[derive(Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TypeKind {
    /// 引用类型（GC 托管、按引用传递）。
    Ref(RefTypeKind),
    /// 值类型（内联、拷贝语义）。
    Value(ValueTypeKind),
    /// `Nothing`（bottom）：既非引用也非值，是一切类型的子类型（P2 §2.2）。
    Nothing,
    /// 类型参数出现（只带 [`TypeParamId`] 身份；元数据见 [`TypeParamDecl`] 侧表）。
    Param(TypeParamId),
    /// 星投影 `Type<*>`（仅 use-site 类型实参位置，P2 §9.2）。
    StarProjection,
}

/// 引用类型分支。
#[derive(Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum RefTypeKind {
    /// class / interface / object / 数组 等 nominal 引用类型（含 `Any`、`String`：
    /// 它们是 sysroot 声明的普通 class/interface，FQN 固定 `scoop.core.Any`/`.String`）。
    Nominal(NominalType),
    /// 函数类型（函数值是引用）。
    Function(FunctionType),
    /// 并类型（引用 LUB 细化）。
    Union(UnionType),
}

/// 值类型分支。
#[derive(Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ValueTypeKind {
    /// `()`（0 元组），唯一值 `()`。
    Unit,
    // 注：`Nothing` 在 [`TypeKind::Nothing`]，不在值类型分支。
    /// `Bool`。
    Bool,
    /// `Char`。
    Char,
    /// `Float64`（双精度）。
    Float64,
    /// `Float32`（单精度）。
    Float32,
    /// `Int`（字长有符号，64-bit 目标上为 64 位，P2 §3.3）。
    Int,
    /// `UInt`（字长无符号）。
    UInt,
    /// `Int8`/`Int16`/`Int32`/`Int64`，位宽 8/16/32/64。
    IntN(u16),
    /// `UInt8`/`UInt16`/`UInt32`/`UInt64`。
    UIntN(u16),
    /// 元组 `(T0, .., Tn)`（结构类型）。
    Tuple(Vec<TypeId>),
    /// struct / enum 等 nominal 值类型（含 `Option<T>`：它是普通 enum，
    /// FQN 固定为 `scoop.core.Option`）。
    Nominal(NominalType),
}

impl std::fmt::Debug for TypeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypeKind::Ref(r) => write!(f, "Ref({r:?})"),
            TypeKind::Value(v) => write!(f, "Value({v:?})"),
            TypeKind::Nothing => write!(f, "Nothing"),
            TypeKind::Param(id) => write!(f, "Param({id:?})"),
            TypeKind::StarProjection => write!(f, "StarProjection"),
        }
    }
}

impl std::fmt::Debug for RefTypeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RefTypeKind::Nominal(n) => write!(f, "Nominal({n:?})"),
            RefTypeKind::Function(ft) => write!(f, "Function({ft:?})"),
            RefTypeKind::Union(u) => write!(f, "Union({u:?})"),
        }
    }
}

impl std::fmt::Debug for ValueTypeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueTypeKind::Unit => write!(f, "Unit"),
            ValueTypeKind::Bool => write!(f, "Bool"),
            ValueTypeKind::Char => write!(f, "Char"),
            ValueTypeKind::Float64 => write!(f, "Float64"),
            ValueTypeKind::Float32 => write!(f, "Float32"),
            ValueTypeKind::Int => write!(f, "Int"),
            ValueTypeKind::UInt => write!(f, "UInt"),
            ValueTypeKind::IntN(bits) => write!(f, "IntN({bits})"),
            ValueTypeKind::UIntN(bits) => write!(f, "UIntN({bits})"),
            ValueTypeKind::Tuple(elems) => {
                f.debug_set().entries(elems.iter().map(|e| e.0)).finish()
            }
            ValueTypeKind::Nominal(n) => write!(f, "Nominal({n:?})"),
        }
    }
}

impl std::fmt::Debug for NominalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "nom#{}", self.fqn.as_u32())
    }
}

impl std::fmt::Debug for FunctionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fn")
            .field("params", &self.params.len())
            .field("ret", &self.return_ty)
            .field("closed", &self.closed)
            .finish()
    }
}

impl std::fmt::Debug for UnionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.variants.iter().map(|v| v.0))
            .finish()
    }
}

// ---------------------------------------------------------------------------
// TypeStore：hash-consing 类型存储
// ---------------------------------------------------------------------------

/// 类型存储：拥有全部 [`TypeKind`]，按结构 hash-cons。
///
/// 同时维护类型参数的元数据侧表 [`TypeStore::param_decls`]：每个 [`TypeParamId`]
/// 对应一份 [`TypeParamDecl`]（名字/位置/变型/约束/种类）。参数身份在声明点由
/// [`TypeStore::mint_param`] 分配，全局唯一、不可伪造。
#[derive(Debug, Default, Clone)]
pub struct TypeStore {
    kinds: Vec<TypeKind>,
    dedup: HashMap<TypeKind, u32>,
    /// 类型参数元数据侧表（id → 声明信息）。
    param_decls: HashMap<TypeParamId, TypeParamDecl>,
    /// 下一个待分配的 [`TypeParamId`]。
    next_param_id: u32,
    /// `Option` 的固定 FQN symbol（`scoop.core.Option`）。由 typecheck 启动时
    /// 经 interner resolve 一次后注入。`option()` 便利构造器用它产出 value nominal。
    /// Option 是普通 enum，无任何后门——这只是缓存其固定 FQN。
    option_fqn: Symbol,
    /// `Any` 的固定 FQN symbol（`scoop.core.Any`）。同理缓存。
    any_fqn: Symbol,
    /// `String` 的固定 FQN symbol（`scoop.core.String`）。
    string_fqn: Symbol,
}

// ---------------------------------------------------------------------------
// serde：StoreRepr 镜像（dedup 不序列化——由 kinds 顺序重建；param_decls
// 按 TypeParamId 排序，保证字节确定，PLAN.md C7）。
// ---------------------------------------------------------------------------

/// [`TypeStore`] 的可序列化镜像。
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct StoreRepr {
    kinds: Vec<TypeKind>,
    param_decls: Vec<(TypeParamId, TypeParamDecl)>,
    next_param_id: u32,
    option_fqn: Symbol,
    any_fqn: Symbol,
    string_fqn: Symbol,
}

impl From<&TypeStore> for StoreRepr {
    fn from(s: &TypeStore) -> Self {
        let mut decls: Vec<(TypeParamId, TypeParamDecl)> =
            s.param_decls.iter().map(|(&k, v)| (k, v.clone())).collect();
        decls.sort_by_key(|(id, _)| *id);
        Self {
            kinds: s.kinds.clone(),
            param_decls: decls,
            next_param_id: s.next_param_id,
            option_fqn: s.option_fqn,
            any_fqn: s.any_fqn,
            string_fqn: s.string_fqn,
        }
    }
}

impl From<StoreRepr> for TypeStore {
    fn from(r: StoreRepr) -> Self {
        let mut dedup = HashMap::with_capacity(r.kinds.len());
        for (i, kind) in r.kinds.iter().enumerate() {
            dedup.insert(kind.clone(), i as u32);
        }
        Self {
            kinds: r.kinds,
            dedup,
            param_decls: r.param_decls.into_iter().collect(),
            next_param_id: r.next_param_id,
            option_fqn: r.option_fqn,
            any_fqn: r.any_fqn,
            string_fqn: r.string_fqn,
        }
    }
}

impl serde::Serialize for TypeStore {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        StoreRepr::from(self).serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for TypeStore {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(TypeStore::from(StoreRepr::deserialize(deserializer)?))
    }
}

impl TypeStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注入 `Option` 的固定 FQN（`scoop.core.Option` 的 interned symbol）。
    /// typecheck 启动时调用一次。之后 [`TypeStore::option`] 产出正确的 value nominal。
    pub fn set_option_fqn(&mut self, fqn: Symbol) {
        self.option_fqn = fqn;
    }

    /// 取已注入的 `Option` FQN（用于通用 FQN 判定）。未注入时返回 default Symbol。
    pub fn option_fqn(&self) -> Symbol {
        self.option_fqn
    }

    /// 注入 `Any` 的固定 FQN（`scoop.core.Any`）。
    pub fn set_any_fqn(&mut self, fqn: Symbol) {
        self.any_fqn = fqn;
    }

    /// 取已注入的 `Any` FQN。
    pub fn any_fqn(&self) -> Symbol {
        self.any_fqn
    }

    /// 注入 `String` 的固定 FQN（`scoop.core.String`）。
    pub fn set_string_fqn(&mut self, fqn: Symbol) {
        self.string_fqn = fqn;
    }

    /// 取已注入的 `String` FQN。
    pub fn string_fqn(&self) -> Symbol {
        self.string_fqn
    }

    pub fn len(&self) -> usize {
        self.kinds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }

    /// Intern 一个类型；结构同构者返回同一 [`TypeId`]。
    pub fn intern(&mut self, kind: TypeKind) -> TypeId {
        if let Some(&idx) = self.dedup.get(&kind) {
            return TypeId(idx);
        }
        let idx = self.kinds.len() as u32;
        self.kinds.push(kind.clone());
        self.dedup.insert(kind, idx);
        TypeId(idx)
    }

    /// 只读查找已 intern 的同构类型（不创建）。只读上下文（树构造等）取
    /// `any()` 等便利类型的途径。
    pub fn find_interned(&self, kind: &TypeKind) -> Option<TypeId> {
        self.dedup.get(kind).map(|&idx| TypeId(idx))
    }

    /// 取一个 id 的种类。`id` 必须由本 store 的 [`intern`][Self::intern] 产出。
    pub fn kind(&self, id: TypeId) -> &TypeKind {
        &self.kinds[id.0 as usize]
    }

    /// `ty` 是否为 FQN == `fqn` 的 nominal 类型（ref 或 value）。
    /// 通用 FQN 判定——不针对任何特定类型开后门。
    pub fn is_nominal_with_fqn(&self, ty: TypeId, fqn: Symbol) -> bool {
        match self.kind(ty) {
            TypeKind::Ref(RefTypeKind::Nominal(n)) | TypeKind::Value(ValueTypeKind::Nominal(n)) => {
                n.fqn == fqn
            }
            _ => false,
        }
    }

    /// 若 `ty` 是 FQN == `fqn` 的 nominal 且有类型实参，返回其 args 切片；否则 None。
    pub fn nominal_args_of_fqn(&self, ty: TypeId, fqn: Symbol) -> Option<&[TypeId]> {
        match self.kind(ty) {
            TypeKind::Ref(RefTypeKind::Nominal(n)) | TypeKind::Value(ValueTypeKind::Nominal(n))
                if n.fqn == fqn =>
            {
                Some(&n.args)
            }
            _ => None,
        }
    }

    /// 把 `other` 中的全部类型重新 intern 进本 store，返回 `other` TypeId → 本 store
    /// TypeId 的重映射表。用于合并多个 per-function store 到一个规范 store。
    ///
    /// 结构同构的类型经 hash-cons 复用现有 TypeId；新类型追加到末尾。
    /// 类型参数元数据侧表（[`TypeParamDecl`]）一并迁移——`TypeParamId` 跨 store
    /// **保持不变**（id 是全局语义身份，不由 store 索引决定）。
    pub fn extend_from(&mut self, other: &TypeStore) -> std::collections::HashMap<TypeId, TypeId> {
        let mut remap = std::collections::HashMap::new();
        for (i, kind) in other.kinds.iter().enumerate() {
            let old = TypeId(i as u32);
            let new = self.intern(kind.clone());
            remap.insert(old, new);
        }
        // 迁移参数元数据侧表（id 不重映射）。
        for (id, decl) in &other.param_decls {
            self.param_decls.entry(*id).or_insert_with(|| decl.clone());
        }
        if other.next_param_id > self.next_param_id {
            self.next_param_id = other.next_param_id;
        }
        remap
    }

    /// 把一个 TypeId 按重映射表翻译（缺省返回原值）。
    pub fn remap_id(remap: &std::collections::HashMap<TypeId, TypeId>, id: TypeId) -> TypeId {
        *remap.get(&id).unwrap_or(&id)
    }

    // ----- 类别查询（结构性，无需查 nominal 声明）-----

    /// 是否引用类型（[`TypeKind::Ref`]）。
    pub fn is_reference(&self, id: TypeId) -> bool {
        matches!(self.kind(id), TypeKind::Ref(_))
    }

    /// 是否值类型（[`TypeKind::Value`]）。
    pub fn is_value(&self, id: TypeId) -> bool {
        matches!(self.kind(id), TypeKind::Value(_))
    }

    /// 是否 `Nothing`（bottom）。
    pub fn is_nothing(&self, id: TypeId) -> bool {
        matches!(self.kind(id), TypeKind::Nothing)
    }

    /// 是否 `Unit`。
    pub fn is_unit(&self, id: TypeId) -> bool {
        matches!(self.kind(id), TypeKind::Value(ValueTypeKind::Unit))
    }

    // ----- 内建/常用构造器（均走 intern，hash-cons）-----

    pub fn unit(&mut self) -> TypeId {
        self.intern(TypeKind::Value(ValueTypeKind::Unit))
    }
    pub fn nothing(&mut self) -> TypeId {
        self.intern(TypeKind::Nothing)
    }
    pub fn bool(&mut self) -> TypeId {
        self.intern(TypeKind::Value(ValueTypeKind::Bool))
    }
    pub fn char(&mut self) -> TypeId {
        self.intern(TypeKind::Value(ValueTypeKind::Char))
    }
    pub fn float64(&mut self) -> TypeId {
        self.intern(TypeKind::Value(ValueTypeKind::Float64))
    }
    pub fn float32(&mut self) -> TypeId {
        self.intern(TypeKind::Value(ValueTypeKind::Float32))
    }
    pub fn int(&mut self) -> TypeId {
        self.intern(TypeKind::Value(ValueTypeKind::Int))
    }
    pub fn uint(&mut self) -> TypeId {
        self.intern(TypeKind::Value(ValueTypeKind::UInt))
    }
    /// `IntN(bits)`：bits ∈ {8,16,32,64}（由调用方保证）。
    pub fn int_n(&mut self, bits: u16) -> TypeId {
        self.intern(TypeKind::Value(ValueTypeKind::IntN(bits)))
    }
    /// `UIntN(bits)`：bits ∈ {8,16,32,64}。
    pub fn uint_n(&mut self, bits: u16) -> TypeId {
        self.intern(TypeKind::Value(ValueTypeKind::UIntN(bits)))
    }
    /// `String` 便利构造器：产出 `Ref(Nominal{scoop.core.String})`。
    /// String 是 sysroot 的 class（引用类型）。需先 [`set_string_fqn`] 注入 FQN。
    pub fn string(&mut self) -> TypeId {
        let n = NominalType {
            fqn: self.string_fqn,
            args: vec![],
            eff: None,
        };
        self.intern(TypeKind::Ref(RefTypeKind::Nominal(n)))
    }
    /// `Any` 便利构造器：产出 `Ref(Nominal{scoop.core.Any})`。
    /// Any 是 sysroot 的空 interface（所有类型的根）。需先 [`set_any_fqn`] 注入 FQN。
    pub fn any(&mut self) -> TypeId {
        let n = NominalType {
            fqn: self.any_fqn,
            args: vec![],
            eff: None,
        };
        self.intern(TypeKind::Ref(RefTypeKind::Nominal(n)))
    }
    /// `Option<T>` 便利构造器：产出 `Value(Nominal{fqn: scoop.core.Option, args:[T]})`。
    /// Option 是普通 enum（值类型），无后门——这只是省去手写 nominal 的便捷函数。
    /// 需先 [`set_option_fqn`] 注入 FQN；未注入时 fqn 为 default（仅供测试结构比对）。
    pub fn option(&mut self, inner: TypeId) -> TypeId {
        let n = NominalType {
            fqn: self.option_fqn,
            args: vec![inner],
            eff: None,
        };
        self.intern(TypeKind::Value(ValueTypeKind::Nominal(n)))
    }
    pub fn tuple(&mut self, elems: Vec<TypeId>) -> TypeId {
        self.intern(TypeKind::Value(ValueTypeKind::Tuple(elems)))
    }
    pub fn function(&mut self, ft: FunctionType) -> TypeId {
        self.intern(TypeKind::Ref(RefTypeKind::Function(ft)))
    }
    /// nominal 引用类型（class/interface/object/数组）。
    pub fn ref_nominal(&mut self, n: NominalType) -> TypeId {
        self.intern(TypeKind::Ref(RefTypeKind::Nominal(n)))
    }
    /// nominal 值类型（struct/enum）。
    pub fn value_nominal(&mut self, n: NominalType) -> TypeId {
        self.intern(TypeKind::Value(ValueTypeKind::Nominal(n)))
    }
    pub fn param(&mut self, id: TypeParamId) -> TypeId {
        self.intern(TypeKind::Param(id))
    }
    pub fn star(&mut self) -> TypeId {
        self.intern(TypeKind::StarProjection)
    }

    // ----- 类型参数身份与元数据 -----

    /// 为一个类型参数分配新的全局唯一 [`TypeParamId`]，并登记其元数据到侧表。
    /// 返回该 id；后续可用 [`TypeStore::param`] 把它 intern 成 `TypeKind::Param`。
    ///
    /// 这是参数身份的**唯一 minting 点**：调用方在 AST→HIR 降级时为每个声明位
    /// 调用一次，保证同名 `T` 在不同声明中得到不同 id。
    pub fn mint_param(&mut self, decl: TypeParamDecl) -> TypeParamId {
        let id = decl.id;
        self.param_decls.insert(id, decl);
        if id.0 >= self.next_param_id {
            self.next_param_id = id.0 + 1;
        }
        id
    }

    /// 分配一个**新的** [`TypeParamId`]（自动递增），并登记元数据。用于不便预计算
    /// id 的场景（如合成参数）。返回新分配的 id。
    pub fn fresh_param(
        &mut self,
        decl_init: impl FnOnce(TypeParamId) -> TypeParamDecl,
    ) -> TypeParamId {
        let id = TypeParamId(self.next_param_id);
        self.next_param_id += 1;
        let decl = decl_init(id);
        self.param_decls.insert(id, decl);
        id
    }

    /// 取某 [`TypeParamId`] 的声明元数据。id 必须由 [`TypeStore::mint_param`] /
    /// [`TypeStore::fresh_param`] 登记过。
    pub fn param_decl(&self, id: TypeParamId) -> &TypeParamDecl {
        &self.param_decls[&id]
    }

    /// 取某 [`TypeParamId`] 的声明元数据（可能缺失时返回 `None`）。
    pub fn param_decl_opt(&self, id: TypeParamId) -> Option<&TypeParamDecl> {
        self.param_decls.get(&id)
    }

    /// 按参数名查类型参数 id（名字在单处声明语境内唯一；跨语境同名时取
    /// id 最小者——声明序稳定）。下游（树路径 FunDecl.type_params 填充）用。
    pub fn find_param_by_name(&self, name: scoop2_base::Symbol) -> Option<TypeParamId> {
        self.param_decls
            .iter()
            .filter(|(_, d)| d.name == name)
            .map(|(id, _)| *id)
            .min()
    }

    // ----- 结构替换 -----

    /// 把 [`Subst`]（普通类型参数）+ [`EffSubst`]（effect 行参数）应用到 `ty`，
    /// 返回新 intern 的 [`TypeId`]。
    ///
    /// 遍历函数类型 / nominal / Option / Tuple / effect 行中的所有类型与 effect
    /// 位置；未出现在替换表里的参数保持原样。`eff_subst` 为 `None` 时不对
    /// [`EffectRow::tail`] 的行变量做替换（保持原样）。
    pub fn apply_subst_full(
        &mut self,
        ty: TypeId,
        subst: &Subst,
        eff_subst: Option<&EffSubst>,
    ) -> TypeId {
        let kind = self.kind(ty).clone();
        match kind {
            TypeKind::Param(id) => subst.get(id).unwrap_or(ty),
            TypeKind::Ref(RefTypeKind::Function(f)) => {
                let f = self.apply_subst_function(f, subst, eff_subst);
                self.intern(TypeKind::Ref(RefTypeKind::Function(f)))
            }
            TypeKind::Ref(RefTypeKind::Nominal(n)) => {
                let n = self.apply_subst_nominal(n, subst, eff_subst);
                self.intern(TypeKind::Ref(RefTypeKind::Nominal(n)))
            }
            TypeKind::Ref(RefTypeKind::Union(u)) => {
                let variants = u
                    .variants
                    .into_iter()
                    .map(|v| self.apply_subst_full(v, subst, eff_subst))
                    .collect::<Vec<_>>();
                self.intern(TypeKind::Ref(RefTypeKind::Union(UnionType { variants })))
            }
            TypeKind::Value(ValueTypeKind::Tuple(elems)) => {
                let elems = elems
                    .into_iter()
                    .map(|e| self.apply_subst_full(e, subst, eff_subst))
                    .collect::<Vec<_>>();
                self.intern(TypeKind::Value(ValueTypeKind::Tuple(elems)))
            }
            TypeKind::Value(ValueTypeKind::Nominal(n)) => {
                let n = self.apply_subst_nominal(n, subst, eff_subst);
                self.intern(TypeKind::Value(ValueTypeKind::Nominal(n)))
            }
            // 标量 / Unit / Nothing / Star：内部无参数，原样返回。
            TypeKind::Value(ValueTypeKind::Unit)
            | TypeKind::Value(ValueTypeKind::Bool)
            | TypeKind::Value(ValueTypeKind::Char)
            | TypeKind::Value(ValueTypeKind::Float64)
            | TypeKind::Value(ValueTypeKind::Float32)
            | TypeKind::Value(ValueTypeKind::Int)
            | TypeKind::Value(ValueTypeKind::UInt)
            | TypeKind::Value(ValueTypeKind::IntN(_))
            | TypeKind::Value(ValueTypeKind::UIntN(_))
            | TypeKind::Nothing
            | TypeKind::StarProjection => ty,
        }
    }

    /// 便捷重载：只应用普通类型参数替换（无 effect 行变量替换）。
    /// 等价于 `apply_subst_full(ty, subst, None)`。
    pub fn apply_subst(&mut self, ty: TypeId, subst: &Subst) -> TypeId {
        self.apply_subst_full(ty, subst, None)
    }

    fn apply_subst_nominal(
        &mut self,
        mut n: NominalType,
        subst: &Subst,
        eff_subst: Option<&EffSubst>,
    ) -> NominalType {
        n.args = n
            .args
            .iter()
            .map(|&a| self.apply_subst_full(a, subst, eff_subst))
            .collect();
        if let Some(row) = n.eff.take() {
            n.eff = Some(self.apply_subst_row_full(row, subst, eff_subst));
        }
        n
    }

    fn apply_subst_function(
        &mut self,
        mut f: FunctionType,
        subst: &Subst,
        eff_subst: Option<&EffSubst>,
    ) -> FunctionType {
        f.receiver = f
            .receiver
            .map(|r| self.apply_subst_full(r, subst, eff_subst));
        f.params = f
            .params
            .iter()
            .map(|&p| self.apply_subst_full(p, subst, eff_subst))
            .collect();
        f.return_ty = self.apply_subst_full(f.return_ty, subst, eff_subst);
        f.effects = self.apply_subst_row_full(f.effects, subst, eff_subst);
        f
    }

    /// 对一个 effect row 应用类型参数替换（pub 版本，供 MIR 单态化调用）。
    ///
    /// 同时处理：
    /// - `terms` 中的参数（经 [`Subst`] 替换为具体类型）；
    /// - `tail` 的行变量（经 [`EffSubst`] 替换为具体 effect 行；展开后并回 terms，
    ///   tail 变 [`EffectTail::Empty`]）。
    pub fn apply_subst_row_full(
        &mut self,
        row: EffectRow,
        subst: &Subst,
        eff_subst: Option<&EffSubst>,
    ) -> EffectRow {
        // terms：逐项应用普通替换。
        let mut terms: Vec<TypeId> = row
            .terms
            .iter()
            .map(|&t| self.apply_subst_full(t, subst, eff_subst))
            .collect();
        // tail：行变量替换。
        let tail = match row.tail {
            EffectTail::Empty => EffectTail::Empty,
            EffectTail::Var(id) => {
                if let Some(es) = eff_subst
                    && let Some(replacement) = es.get(id)
                {
                    // E → 具体 effect 行：把 replacement 的 terms 并回，tail 取其 tail。
                    terms.extend_from_slice(&replacement.terms);
                    replacement.tail.clone()
                } else {
                    // 未提供替换：行变量保持原样。
                    EffectTail::Var(id)
                }
            }
        };
        EffectRow::from_terms_with_tail(terms, tail)
    }

    /// 便捷重载：只应用普通类型参数替换到 effect row（不替换行变量 tail）。
    /// 等价于 `apply_subst_row_full(row, subst, None)`。
    pub fn apply_subst_row(&mut self, row: EffectRow, subst: &Subst) -> EffectRow {
        self.apply_subst_row_full(row, subst, None)
    }
}

// ---------------------------------------------------------------------------
// Subst / EffSubst：类型参数替换表（按 TypeParamId 键，值域不同）
// ---------------------------------------------------------------------------

/// 普通类型参数替换表：[`TypeParamId`]（`kind = Type`）→ [`TypeId`]。
///
/// 键用 [`TypeParamId`] 身份（**不再用名字**），保证两个不同声明里同名的 `T`
/// 互不混淆。effect 行参数（`kind = Effect`）的替换见 [`EffSubst`]——两者值域
/// 不同（一个 TypeId，一个 EffectRow），故分表存储。
#[derive(Clone, Default)]
pub struct Subst {
    entries: HashMap<TypeParamId, TypeId>,
}

impl Subst {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, id: TypeParamId, ty: TypeId) {
        self.entries.insert(id, ty);
    }

    pub fn get(&self, id: TypeParamId) -> Option<TypeId> {
        self.entries.get(&id).copied()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

/// effect 行参数替换表：[`TypeParamId`]（`kind = Effect`）→ [`EffectRow`]。
///
/// effect 行参数（`<eff E>`）的替换值是一**整组 effect**（一个完整
/// [`EffectRow`]），而非单个 [`TypeId`]，故与 [`Subst`] 分表。
/// 在 [`TypeStore::apply_subst_row`] 中用于展开 [`EffectRow::tail`] 的行变量。
#[derive(Clone, Default)]
pub struct EffSubst {
    entries: HashMap<TypeParamId, EffectRow>,
}

impl EffSubst {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, id: TypeParamId, row: EffectRow) {
        self.entries.insert(id, row);
    }

    pub fn get(&self, id: TypeParamId) -> Option<&EffectRow> {
        self.entries.get(&id)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// 类型渲染：TypeId → 可读字符串（诊断 / dump-hir 共用）
// ---------------------------------------------------------------------------

/// 把一个 [`TypeId`] 渲染为 spec 风格的可读类型文本。
///
/// 这是全前端唯一的类型渲染器（诊断与 `dump-hir` 共用），取代此前散落在
/// `typecheck/{mod,extern_fn,release_hook}` 里的 4 份重复 `fmt_type*`。
///
/// 渲染约定（与 spec 表面语法一致）：
/// - 标量：`Unit` / `Bool` / `Char` / `Int` / `UInt` / `Float64` / `Float32` /
///   `Int8`..`Int64` / `UInt8`..`UInt64`；
/// - `Option<T>` → `T?`（嵌套不拍平，`Int??` 合法）；
/// - 元组 `(A, B)`（单元素带尾逗号 `(A,)`，空元组 `()`）；
/// - nominal：全限定名 + `<args>` + 可选 `/ eff`（`List<Int>`、`Effect<Raise<Err>>`）；
/// - 函数 `(A, B) -> C / R`（闭合行加 `!`，接收者 `T.(A) -> C`）；
/// - `Any` / `String` / `Nothing` / `*`（星投影）/ 类型参数名。
///
/// `interner` 用于把 [`Symbol`] 解析为字符串；`full_fqn=true` 时 nominal 用全限定名，
/// `false` 时用末段短名（诊断 `found` 字段用短名更易读）。
pub fn render_type(
    store: &TypeStore,
    interner: &scoop2_base::Interner,
    id: TypeId,
    full_fqn: bool,
) -> String {
    // Any（scoop.core.Any）是根类型关键字，始终渲染为短名 `Any`（与 Nothing 一致），
    // 而非全限定 `scoop.core.Any`。仅在 any_fqn 已注入时生效。
    let any_fqn = store.any_fqn();
    if any_fqn != Symbol::default() && store.is_nominal_with_fqn(id, any_fqn) {
        return "Any".to_string();
    }
    render_kind(store, interner, store.kind(id), full_fqn)
}

/// nominal 名渲染辅助：按 `full_fqn` 取全限定或末段短名。
fn nominal_name(interner: &scoop2_base::Interner, fqn: Symbol, full_fqn: bool) -> String {
    let text = interner.resolve(fqn);
    if full_fqn {
        text.to_string()
    } else {
        text.rsplit('.').next().unwrap_or(text).to_string()
    }
}

/// nominal 实参与可选 effect 行渲染：`<A, B>` / `<A, B> / Eff`。
fn nominal_tail(
    store: &TypeStore,
    interner: &scoop2_base::Interner,
    n: &NominalType,
    full_fqn: bool,
) -> String {
    let mut out = String::new();
    if !n.args.is_empty() {
        out.push('<');
        let args: Vec<String> = n
            .args
            .iter()
            .map(|&a| render_type(store, interner, a, full_fqn))
            .collect();
        out.push_str(&args.join(", "));
        out.push('>');
    }
    if let Some(eff) = &n.eff {
        out.push_str(" / ");
        out.push_str(&render_effect_row(store, interner, eff, full_fqn));
    }
    out
}

/// effect 行渲染：`Pure`（空）/ `A` / `A + B` / `A + E`（行变量）。
fn render_effect_row(
    store: &TypeStore,
    interner: &scoop2_base::Interner,
    row: &EffectRow,
    full_fqn: bool,
) -> String {
    let mut items: Vec<String> = row
        .terms
        .iter()
        .map(|&t| render_type(store, interner, t, full_fqn))
        .collect();
    // 行变量渲染为其参数名（从侧表查）。
    if let EffectTail::Var(id) = &row.tail {
        let name = store
            .param_decl_opt(*id)
            .map(|d| interner.resolve(d.name).to_string())
            .unwrap_or_else(|| format!("{id:?}"));
        items.push(name);
    }
    if items.is_empty() {
        "Pure".to_string()
    } else {
        items.join(" + ")
    }
}

/// [`TypeKind`] 级渲染（递归）。
fn render_kind(
    store: &TypeStore,
    interner: &scoop2_base::Interner,
    kind: &TypeKind,
    full_fqn: bool,
) -> String {
    match kind {
        TypeKind::Nothing => "Nothing".into(),
        TypeKind::StarProjection => "*".into(),
        TypeKind::Param(id) => store
            .param_decl_opt(*id)
            .map(|d| interner.resolve(d.name).to_string())
            .unwrap_or_else(|| format!("{id:?}"))
            .into(),
        TypeKind::Ref(r) => render_ref(store, interner, r, full_fqn),
        TypeKind::Value(v) => render_value(store, interner, v, full_fqn),
    }
}

fn render_ref(
    store: &TypeStore,
    interner: &scoop2_base::Interner,
    r: &RefTypeKind,
    full_fqn: bool,
) -> String {
    match r {
        RefTypeKind::Nominal(n) => {
            let mut s = nominal_name(interner, n.fqn, full_fqn);
            s.push_str(&nominal_tail(store, interner, n, full_fqn));
            s
        }
        RefTypeKind::Function(ft) => {
            let mut s = String::new();
            if let Some(recv) = ft.receiver {
                s.push_str(&render_type(store, interner, recv, full_fqn));
                s.push_str(".(");
            } else {
                s.push('(');
            }
            let params: Vec<String> = ft
                .params
                .iter()
                .map(|&p| render_type(store, interner, p, full_fqn))
                .collect();
            s.push_str(&params.join(", "));
            s.push_str(") -> ");
            s.push_str(&render_type(store, interner, ft.return_ty, full_fqn));
            s.push_str(" / ");
            s.push_str(&render_effect_row(store, interner, &ft.effects, full_fqn));
            if ft.closed {
                s.push('!');
            }
            s
        }
        RefTypeKind::Union(u) => {
            // 并类型用 `|` 连接（与 spec 的 union 表面语法一致）。
            let items: Vec<String> = u
                .variants
                .iter()
                .map(|&v| render_type(store, interner, v, full_fqn))
                .collect();
            items.join(" | ")
        }
    }
}

fn render_value(
    store: &TypeStore,
    interner: &scoop2_base::Interner,
    v: &ValueTypeKind,
    full_fqn: bool,
) -> String {
    match v {
        ValueTypeKind::Unit => "()".into(),
        ValueTypeKind::Bool => "Bool".into(),
        ValueTypeKind::Char => "Char".into(),
        ValueTypeKind::Float64 => "Float64".into(),
        ValueTypeKind::Float32 => "Float32".into(),
        ValueTypeKind::Int => "Int".into(),
        ValueTypeKind::UInt => "UInt".into(),
        ValueTypeKind::IntN(bits) => format!("Int{bits}"),
        ValueTypeKind::UIntN(bits) => format!("UInt{bits}"),
        ValueTypeKind::Tuple(els) => {
            if els.is_empty() {
                "()".into()
            } else if els.len() == 1 {
                // 单元组带尾逗号。
                format!("({},)", render_type(store, interner, els[0], full_fqn))
            } else {
                let items: Vec<String> = els
                    .iter()
                    .map(|&e| render_type(store, interner, e, full_fqn))
                    .collect();
                format!("({})", items.join(", "))
            }
        }
        ValueTypeKind::Nominal(n) => {
            // Option<T> 渲染为 `T?`（可读性；嵌套不拍平 `Int??`）。
            // Option 的唯一"特殊之处"是固定 FQN scoop.core.Option——按 FQN 判定。
            if interner.get("scoop.core.Option") == Some(n.fqn) && n.args.len() == 1 {
                return format!("{}?", render_type(store, interner, n.args[0], full_fqn));
            }
            let mut s = nominal_name(interner, n.fqn, full_fqn);
            s.push_str(&nominal_tail(store, interner, n, full_fqn));
            s
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fqn(interner: &mut scoop2_base::Interner, name: &str) -> Symbol {
        interner.intern(name)
    }

    /// 注册一个类型参数到 store（自动分配新 id），返回该 id。
    fn reg_param(
        store: &mut TypeStore,
        interner: &mut scoop2_base::Interner,
        name: &str,
    ) -> TypeParamId {
        store.fresh_param(|id| TypeParamDecl {
            id,
            name: interner.intern(name),
            file: FileId(0),
            span: Span::new(0, 1),
            variance: None,
            bound: None,
            kind: TypeParamKind::Type,
        })
    }

    /// 注册一个 effect 行参数到 store（自动分配新 id），返回该 id。
    fn reg_eff_param(
        store: &mut TypeStore,
        interner: &mut scoop2_base::Interner,
        name: &str,
    ) -> TypeParamId {
        store.fresh_param(|id| TypeParamDecl {
            id,
            name: interner.intern(name),
            file: FileId(0),
            span: Span::new(0, 1),
            variance: None,
            bound: None,
            kind: TypeParamKind::Effect,
        })
    }

    // ----- hash-consing -----

    #[test]
    fn intern_dedups_structurally_equal_kinds() {
        let mut store = TypeStore::new();
        let a = store.int();
        let b = store.int();
        let inner = store.int();
        let c = store.option(inner);
        let d = store.option(inner);
        assert_eq!(a, b, "same kind → same id");
        assert_eq!(c, d, "structurally equal Option<Int> → same id");
        assert_ne!(a, c);
        assert_eq!(store.len(), 2, "two unique kinds interned");
    }

    #[test]
    fn distinct_builtin_scalars_are_distinct() {
        let mut s = TypeStore::new();
        // String/Any 现为 ref nominal：需注入各自 FQN 才能使二者彼此不同（默认 Symbol(0) 相同）。
        s.set_string_fqn(Symbol::from_u32(997));
        s.set_any_fqn(Symbol::from_u32(998));
        let ids = [
            s.unit(),
            s.nothing(),
            s.bool(),
            s.char(),
            s.int(),
            s.uint(),
            s.int_n(8),
            s.int_n(16),
            s.uint_n(64),
            s.float32(),
            s.float64(),
            s.string(),
            s.any(),
        ];
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert_ne!(ids[i], ids[j], "builtins at {i}/{j} must differ");
            }
        }
    }

    // ----- 类别查询 -----

    #[test]
    fn category_queries_are_structural() {
        let mut s = TypeStore::new();
        let mut interner = scoop2_base::Interner::new();
        let ref_n = s.ref_nominal(NominalType {
            fqn: fqn(&mut interner, "a.C"),
            args: vec![],
            eff: None,
        });
        let val_n = s.value_nominal(NominalType {
            fqn: fqn(&mut interner, "a.S"),
            args: vec![],
            eff: None,
        });
        assert!(s.is_reference(ref_n));
        assert!(!s.is_value(ref_n));
        assert!(s.is_value(val_n));
        assert!(!s.is_reference(val_n));
        let string_ty = s.string();
        let int_ty = s.int();
        let nothing_ty = s.nothing();
        let unit_ty = s.unit();
        assert!(s.is_reference(string_ty));
        assert!(s.is_value(int_ty));
        // Nothing 既非引用也非值。
        assert!(!s.is_reference(nothing_ty));
        assert!(!s.is_value(nothing_ty));
        assert!(s.is_nothing(nothing_ty));
        assert!(s.is_unit(unit_ty));
    }

    // ----- EffectRow 代数 -----

    #[test]
    fn effect_row_canonicalizes() {
        let mut s = TypeStore::new();
        let io = s.string(); // 任意 id 用作 effect term 占位
        let rse = s.int();
        let row = EffectRow::from_terms(vec![rse, io, rse, io]);
        assert_eq!(row.terms.len(), 2, "dedup");
        assert_eq!(
            row.terms,
            {
                let mut v = vec![io, rse];
                v.sort();
                v
            },
            "sorted"
        );
    }

    #[test]
    fn effect_row_pure_and_union() {
        let mut s = TypeStore::new();
        let a = s.string();
        let b = s.int();
        let pure = EffectRow::pure();
        assert!(pure.is_pure());
        let ra = EffectRow::single(a);
        let rb = EffectRow::single(b);
        let rab = ra.union(&rb);
        assert_eq!(rab.terms.len(), 2);
        // 幂等
        let rab2 = rab.union(&ra);
        assert_eq!(rab2.terms.len(), 2, "union idempotent");
        // Pure 是单位元
        assert!(ra.union(&pure).equals(&ra));
    }

    #[test]
    fn effect_row_subset() {
        let mut s = TypeStore::new();
        let a = s.string();
        let b = s.int();
        let c = s.bool();
        let abc = EffectRow::from_terms(vec![a, b, c]);
        let ab = EffectRow::from_terms(vec![a, b]);
        let cd = EffectRow::from_terms(vec![c, s.char()]);
        assert!(EffectRow::pure().is_subset_of(&abc), "Pure ⊆ anything");
        assert!(ab.is_subset_of(&abc), "subset");
        assert!(!abc.is_subset_of(&ab), "not subset");
        assert!(abc.is_subset_of(&abc), "reflexive");
        assert!(!cd.is_subset_of(&ab), "disjoint term not subset");
    }

    // ----- 结构替换 -----

    #[test]
    fn subst_into_function_and_option_and_tuple() {
        let mut s = TypeStore::new();
        let mut interner = scoop2_base::Interner::new();
        let t = reg_param(&mut s, &mut interner, "T");
        let t_id = s.param(t);
        let ret = t_id;
        let ft = s.function(FunctionType {
            receiver: None,
            params: vec![t_id],
            return_ty: ret,
            effects: EffectRow::pure(),
            closed: false,
        });
        let mut subst = Subst::new();
        subst.insert(t, s.int());
        let applied = s.apply_subst(ft, &subst);
        // 应用后函数体内不再含 T。
        let TypeKind::Ref(RefTypeKind::Function(f)) = s.kind(applied).clone() else {
            panic!("expected function");
        };
        assert_eq!(f.params[0], s.int());
        assert_eq!(f.return_ty, s.int());
    }

    #[test]
    fn subst_leaves_non_param_types_unchanged() {
        let mut s = TypeStore::new();
        let mut interner = scoop2_base::Interner::new();
        let t = reg_param(&mut s, &mut interner, "T");
        let subst = Subst::new(); // 空
        let opt_inner = s.int();
        let opt = s.option(opt_inner);
        assert_eq!(s.apply_subst(opt, &subst), opt);
        let tup_i = s.int();
        let tup_b = s.bool();
        let tup = s.tuple(vec![tup_i, tup_b]);
        assert_eq!(s.apply_subst(tup, &subst), tup);
        // 参数不在替换表里 → 原样返回。
        let p = s.param(t);
        assert_eq!(s.apply_subst(p, &subst), p);
    }

    #[test]
    fn subst_into_nominal_args_and_eff() {
        let mut s = TypeStore::new();
        let mut interner = scoop2_base::Interner::new();
        let t = reg_param(&mut s, &mut interner, "T");
        let t_id = s.param(t);
        let list = s.ref_nominal(NominalType {
            fqn: interner.intern("List"),
            args: vec![t_id],
            eff: None,
        });
        let mut subst = Subst::new();
        subst.insert(t, s.int());
        let applied = s.apply_subst(list, &subst);
        let TypeKind::Ref(RefTypeKind::Nominal(n)) = s.kind(applied).clone() else {
            panic!("expected nominal ref");
        };
        assert_eq!(n.args, vec![s.int()]);
    }

    #[test]
    fn subst_distinguishes_params_by_decl_site() {
        let mut s = TypeStore::new();
        let mut interner = scoop2_base::Interner::new();
        // 两个同名 T，独立注册 → 不同 id。
        let t1 = reg_param(&mut s, &mut interner, "T");
        let t2 = reg_param(&mut s, &mut interner, "T");
        let id1 = s.param(t1);
        let id2 = s.param(t2);
        assert_ne!(id1, id2, "same name, different decl → distinct id");
    }

    #[test]
    fn subst_id_keys_do_not_collide_across_decls() {
        // 两个不同声明里同名 T：各自的 Subst 只命中自己的 id。
        let mut s = TypeStore::new();
        let mut interner = scoop2_base::Interner::new();
        let t1 = reg_param(&mut s, &mut interner, "T");
        let t2 = reg_param(&mut s, &mut interner, "T");
        // 声明 1 的 Subst 把 t1 映射到 Int。
        let mut subst1 = Subst::new();
        subst1.insert(t1, s.int());
        // t2 不在 subst1 里 → 原样返回（不会因为同名而被误替换）。
        let p2 = s.param(t2);
        assert_eq!(s.apply_subst(p2, &subst1), p2);
    }

    #[test]
    fn effect_tail_substitution_via_eff_subst() {
        // <eff E> 体现为 tail = Var(E)；EffSubst 把 E 展开成具体 effect 行。
        let mut s = TypeStore::new();
        let mut interner = scoop2_base::Interner::new();
        let e = reg_eff_param(&mut s, &mut interner, "E");
        let io = s.string();
        let state = s.int();
        // 行 = {IO} + E
        let row = EffectRow::from_terms_with_tail(vec![io], EffectTail::Var(e));
        let mut es = EffSubst::new();
        es.insert(e, EffectRow::single(state)); // E → {State}
        let applied = s.apply_subst_row_full(row, &Subst::new(), Some(&es));
        // 展开后 = {IO, State}，tail 变 Empty。
        assert_eq!(applied.terms.len(), 2);
        assert!(applied.contains(io));
        assert!(applied.contains(state));
        assert!(matches!(applied.tail, EffectTail::Empty));
    }

    #[test]
    fn effect_row_is_pure_requires_no_tail() {
        let a = TypeStore::new().string();
        // 有 tail 的行不是 Pure，即便 terms 为空。
        let row_with_tail =
            EffectRow::from_terms_with_tail(vec![], EffectTail::Var(TypeParamId(7)));
        assert!(!row_with_tail.is_pure());
        // 真正的 Pure。
        assert!(EffectRow::pure().is_pure());
        let _ = a;
    }

    #[test]
    fn effect_row_subset_with_tail() {
        let mut s = TypeStore::new();
        let mut interner = scoop2_base::Interner::new();
        let e = reg_eff_param(&mut s, &mut interner, "E");
        let io = s.string();
        // {IO} + E 包含 {IO}（tail 可吸收任意）。
        let big = EffectRow::from_terms_with_tail(vec![io], EffectTail::Var(e));
        let small = EffectRow::single(io);
        assert!(small.is_subset_of(&big));
        // {IO} + E 不被 {IO} 包含（闭合行无法吸收任意）。
        assert!(!big.is_subset_of(&small));
    }

    // ----- UnionType 规范化 -----

    #[test]
    fn union_flattens_drops_nothing_dedup_sorts() {
        let mut s = TypeStore::new();
        // String/Any 现为 ref nominal：注入不同 FQN 以保证二者互异（默认 Symbol(0) 相同）。
        s.set_string_fqn(Symbol::from_u32(997));
        s.set_any_fqn(Symbol::from_u32(998));
        let a = s.string();
        let b = s.any();
        let nothing = s.nothing();
        let inner_union = UnionType::from_variants(&s, vec![a, b]);
        let inner = s.intern(TypeKind::Ref(RefTypeKind::Union(inner_union)));
        let u = UnionType::from_variants(&s, vec![a, nothing, inner, a]);
        assert!(u.variants.iter().all(|&v| v != nothing), "Nothing dropped");
        assert_eq!(u.variants.len(), 2, "deduped {{a,b}}");
        assert_eq!(u.variants, {
            let mut v = vec![a, b];
            v.sort();
            v
        });
    }

    // ----- render_type -----

    #[test]
    fn render_scalars_and_option_tuple() {
        let mut s = TypeStore::new();
        let mut interner = scoop2_base::Interner::new();
        // Option 现为 value nominal：option_fqn 必须注入（与 render 的 FQN 判定一致），
        // render 才能识别为 `T?`。
        s.set_option_fqn(interner.intern("scoop.core.Option"));
        // Any 现为 ref nominal{scoop.core.Any}：any_fqn 必须注入，render 才能识别为 `Any`。
        s.set_any_fqn(interner.intern("scoop.core.Any"));
        // String 现为 ref nominal{scoop.core.String}：string_fqn 必须注入，render 才能识别为 FQN。
        s.set_string_fqn(interner.intern("scoop.core.String"));
        let unit = s.unit();
        let bool_ = s.bool();
        let int = s.int();
        let int8 = s.int_n(8);
        let uint64 = s.uint_n(64);
        let f64 = s.float64();
        let str_ = s.string();
        let any = s.any();
        let nothing = s.nothing();
        assert_eq!(render_type(&s, &interner, unit, true), "()");
        assert_eq!(render_type(&s, &interner, bool_, true), "Bool");
        assert_eq!(render_type(&s, &interner, int, true), "Int");
        assert_eq!(render_type(&s, &interner, int8, true), "Int8");
        assert_eq!(render_type(&s, &interner, uint64, true), "UInt64");
        assert_eq!(render_type(&s, &interner, f64, true), "Float64");
        assert_eq!(render_type(&s, &interner, str_, true), "scoop.core.String");
        assert_eq!(render_type(&s, &interner, any, true), "Any");
        assert_eq!(render_type(&s, &interner, nothing, true), "Nothing");
        // Option<Int> → Int?
        let opt = s.option(int);
        assert_eq!(render_type(&s, &interner, opt, true), "Int?");
        // 嵌套 Option<Option<Int>> → Int??（不拍平）
        let opt2 = s.option(opt);
        assert_eq!(render_type(&s, &interner, opt2, true), "Int??");
        // 元组 (Int, Bool) / 单元组 (Int,)
        let b2 = s.bool();
        let tup = s.tuple(vec![int, b2]);
        assert_eq!(render_type(&s, &interner, tup, true), "(Int, Bool)");
        let i3 = s.int();
        let tup1 = s.tuple(vec![i3]);
        assert_eq!(render_type(&s, &interner, tup1, true), "(Int,)");
    }

    #[test]
    fn render_nominal_and_function_and_param() {
        let mut s = TypeStore::new();
        let mut interner = scoop2_base::Interner::new();
        // String 现为 ref nominal{scoop.core.String}：string_fqn 必须注入，render 才能识别为 FQN。
        s.set_string_fqn(interner.intern("scoop.core.String"));
        let int = s.int();
        let bool_ = s.bool();
        let str_ = s.string();
        let unit = s.unit();
        // nominal ref List<Int>
        let list = s.ref_nominal(NominalType {
            fqn: fqn(&mut interner, "a.b.List"),
            args: vec![int],
            eff: None,
        });
        assert_eq!(render_type(&s, &interner, list, true), "a.b.List<Int>");
        assert_eq!(render_type(&s, &interner, list, false), "List<Int>");
        // nominal with effect arg: Effect<Raise<Err>>
        let err = s.ref_nominal(NominalType {
            fqn: fqn(&mut interner, "a.Err"),
            args: vec![],
            eff: None,
        });
        let raise = s.ref_nominal(NominalType {
            fqn: fqn(&mut interner, "a.Raise"),
            args: vec![err],
            eff: None,
        });
        let effect = s.ref_nominal(NominalType {
            fqn: fqn(&mut interner, "a.Effect"),
            args: vec![raise],
            eff: None,
        });
        assert_eq!(
            render_type(&s, &interner, effect, false),
            "Effect<Raise<Err>>"
        );
        // nominal with use-site eff row: Effect<Raise<Err>> / Async
        let async_eff = s.ref_nominal(NominalType {
            fqn: fqn(&mut interner, "a.Async"),
            args: vec![],
            eff: None,
        });
        let raise2 = s.ref_nominal(NominalType {
            fqn: fqn(&mut interner, "a.Raise"),
            args: vec![err],
            eff: None,
        });
        let effect_eff = s.ref_nominal(NominalType {
            fqn: fqn(&mut interner, "a.Effect"),
            args: vec![raise2],
            eff: Some(EffectRow::single(async_eff)),
        });
        assert_eq!(
            render_type(&s, &interner, effect_eff, false),
            "Effect<Raise<Err>> / Async"
        );
        // function (Int, Bool) -> String / Pure
        let ft = s.function(FunctionType {
            receiver: None,
            params: vec![int, bool_],
            return_ty: str_,
            effects: EffectRow::pure(),
            closed: false,
        });
        assert_eq!(
            render_type(&s, &interner, ft, true),
            "(Int, Bool) -> scoop.core.String / Pure"
        );
        // closed function (Int) -> Unit / Raise<Err>!
        let i4 = s.int();
        let raise3 = s.ref_nominal(NominalType {
            fqn: fqn(&mut interner, "a.Raise"),
            args: vec![err],
            eff: None,
        });
        let ft_closed = s.function(FunctionType {
            receiver: None,
            params: vec![i4],
            return_ty: unit,
            effects: EffectRow::single(raise3),
            closed: true,
        });
        assert_eq!(
            render_type(&s, &interner, ft_closed, false),
            "(Int) -> () / Raise<Err>!"
        );
        // receiver function String.(Int) -> Bool / Pure
        let i5 = s.int();
        let str3 = s.string();
        let b5 = s.bool();
        let ft_recv = s.function(FunctionType {
            receiver: Some(str3),
            params: vec![i5],
            return_ty: b5,
            effects: EffectRow::pure(),
            closed: false,
        });
        assert_eq!(
            render_type(&s, &interner, ft_recv, true),
            "scoop.core.String.(Int) -> Bool / Pure"
        );
        // type param + star projection
        let t_id = reg_param(&mut s, &mut interner, "T");
        let p = s.param(t_id);
        assert_eq!(render_type(&s, &interner, p, true), "T");
        let star = s.star();
        assert_eq!(render_type(&s, &interner, star, true), "*");
    }
}
