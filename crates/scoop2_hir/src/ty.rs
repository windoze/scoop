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
//! - **effect 行是集合**：[`EffectRow`] 是规范化（排序去重）的 effect 类型 id
//!   集合；`Pure` = 空集；`+` 为并（幂等/交换/结合）；`⊆` 为子集（双指针）。
//!   generic effect 不变（P4 §4）。闭合行标记 `/ R!` 不挂在行上，而挂在
//!   [`FunctionType::closed`] 上（闭合性是函数标注的属性，P4 §4.3）。
//! - **类型参数按声明位识别**：[`TypeParamType`] 由 `(file, span)` 全局唯一定位，
//!   因此不同声明里同名的 `T` 是不同的参数（P3 §17）。结构替换 [`Subst`] /
//!   [`TypeStore::apply_subst`] 遍历所有类型/effect 位置。
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
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TypeId(pub u32);

impl std::fmt::Debug for TypeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ty#{}", self.0)
    }
}

/// 一个类型参数的出现身份：`(file, span)` 全局唯一（同名 `T` 在不同声明中不同）。
///
/// `name` 仅供诊断/显示，不参与「这是哪个参数」的判定（同一 span 即同一参数，
/// 名字必然相同）。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeParamType {
    pub name: Symbol,
    pub file: FileId,
    pub span: Span,
}

impl std::fmt::Debug for TypeParamType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "param({}:{:?})", self.file, self.span)
    }
}

/// Effect 行：规范化（按 [`TypeId`] 排序去重）的 effect 类型 id 集合。
///
/// - `Pure` ≡ `terms` 为空；
/// - `+`（并）幂等 / 交换 / 结合，由 [`EffectRow::union`] 维持规范形式；
/// - `⊆`（子集）由 [`EffectRow::is_subset_of`] 用双指针在两个有序集合上判定；
/// - generic effect 不变：`Emit<Any>` 与 `Emit<String>` 互不蕴含（P4 §4）。
///
/// 闭合标记 `/ R!` 不在此处，而在 [`FunctionType::closed`]。
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct EffectRow {
    pub terms: Vec<TypeId>,
}

impl EffectRow {
    /// 构造一个行并规范化（排序去重）。
    pub fn from_terms(mut terms: Vec<TypeId>) -> Self {
        terms.sort_unstable();
        terms.dedup();
        EffectRow { terms }
    }

    /// `Pure`（空行）。
    pub fn pure() -> Self {
        EffectRow { terms: Vec::new() }
    }

    /// 单元素行。
    pub fn single(term: TypeId) -> Self {
        EffectRow { terms: vec![term] }
    }

    pub fn is_pure(&self) -> bool {
        self.terms.is_empty()
    }

    /// `self + other`（并），结果规范化。
    pub fn union(&self, other: &EffectRow) -> EffectRow {
        let mut terms = self.terms.clone();
        terms.extend_from_slice(&other.terms);
        EffectRow::from_terms(terms)
    }

    /// `self ⊆ other`？双指针（两边均规范有序）。
    ///
    /// 满足：`Pure ⊆ R`、`R ⊆ R + S`、`R ⊆ R`。
    pub fn is_subset_of(&self, other: &EffectRow) -> bool {
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
        // self 的剩余项必须为空（否则 self 含有 other 没有的项）。
        i == self.terms.len()
    }

    pub fn equals(&self, other: &EffectRow) -> bool {
        self.terms == other.terms
    }

    /// 是否含某 effect 项（`terms` 规范有序，用二分）。
    pub fn contains(&self, term: TypeId) -> bool {
        self.terms.binary_search(&term).is_ok()
    }

    /// 差集：`self − other`（从 self 中移除 other 中存在的 terms）。
    /// self 已排序去重，过滤后仍有序，无需再规范化。
    pub fn difference(&self, other: &EffectRow) -> EffectRow {
        let result: Vec<TypeId> = self
            .terms
            .iter()
            .filter(|t| !other.contains(**t))
            .copied()
            .collect();
        EffectRow { terms: result }
    }
}

impl std::fmt::Debug for EffectRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.terms.iter().map(|t| t.0))
            .finish()
    }
}

/// nominal 类型的一次出现（class/interface/struct/enum/object）。
///
/// `fqn` 是全限定点分名（interned），用作 nominal 身份键；具体声明信息（成员、
/// 超类型、kind）由 resolve/typecheck 按 fqn 在 Index/TypeEnv 中查得。`eff` 是
/// use-site effect-row 实参（仅对带 `<eff E>` 的 effect 类型有意义）。
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct NominalType {
    pub fqn: Symbol,
    pub args: Vec<TypeId>,
    pub eff: Option<EffectRow>,
}

/// 函数类型 `(P0, .., Pn) -> R / Row [!]`（spec P2 §11、P3 §6.1）。
///
/// 子类型规则（在 typecheck 的 assignability 中实现）：参数逆变 + 返回协变 +
/// effect 行放宽（`R1 ⊆ R2 ⇒ (A)->B/R1 <: (A)->B/R2`）。
#[derive(Clone, PartialEq, Eq, Hash)]
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
#[derive(Clone, PartialEq, Eq, Hash)]
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
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum TypeKind {
    /// 引用类型（GC 托管、按引用传递）。
    Ref(RefTypeKind),
    /// 值类型（内联、拷贝语义）。
    Value(ValueTypeKind),
    /// `Nothing`（bottom）：既非引用也非值，是一切类型的子类型（P2 §2.2）。
    Nothing,
    /// 类型参数出现。
    Param(TypeParamType),
    /// 星投影 `Type<*>`（仅 use-site 类型实参位置，P2 §9.2）。
    StarProjection,
}

/// 引用类型分支。
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum RefTypeKind {
    /// `Any`：所有可传递引用值的根接口（P2 §2.1）。
    Any,
    /// `String`（GC 引用、内容相等，P2 §3.5）。
    String,
    /// class / interface / object / 数组 等 nominal 引用类型。
    Nominal(NominalType),
    /// 函数类型（函数值是引用）。
    Function(FunctionType),
    /// 并类型（引用 LUB 细化）。
    Union(UnionType),
}

/// 值类型分支。
#[derive(Clone, PartialEq, Eq, Hash)]
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
    /// `Option<T>`（`T?` 的脱糖；嵌套不拍平，P2 §6）。
    Option(TypeId),
    /// 元组 `(T0, .., Tn)`（结构类型）。
    Tuple(Vec<TypeId>),
    /// struct / enum 等 nominal 值类型。
    Nominal(NominalType),
}

impl std::fmt::Debug for TypeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypeKind::Ref(r) => write!(f, "Ref({r:?})"),
            TypeKind::Value(v) => write!(f, "Value({v:?})"),
            TypeKind::Nothing => write!(f, "Nothing"),
            TypeKind::Param(p) => write!(f, "Param({p:?})"),
            TypeKind::StarProjection => write!(f, "StarProjection"),
        }
    }
}

impl std::fmt::Debug for RefTypeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RefTypeKind::Any => write!(f, "Any"),
            RefTypeKind::String => write!(f, "String"),
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
            ValueTypeKind::Option(inner) => write!(f, "Option({inner:?})"),
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
#[derive(Debug, Default, Clone)]
pub struct TypeStore {
    kinds: Vec<TypeKind>,
    dedup: HashMap<TypeKind, u32>,
}

impl TypeStore {
    pub fn new() -> Self {
        Self::default()
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

    /// 取一个 id 的种类。`id` 必须由本 store 的 [`intern`][Self::intern] 产出。
    pub fn kind(&self, id: TypeId) -> &TypeKind {
        &self.kinds[id.0 as usize]
    }

    /// 把 `other` 中的全部类型重新 intern 进本 store，返回 `other` TypeId → 本 store
    /// TypeId 的重映射表。用于合并多个 per-function store 到一个规范 store。
    ///
    /// 结构同构的类型经 hash-cons 复用现有 TypeId；新类型追加到末尾。
    pub fn extend_from(&mut self, other: &TypeStore) -> std::collections::HashMap<TypeId, TypeId> {
        let mut remap = std::collections::HashMap::new();
        for (i, kind) in other.kinds.iter().enumerate() {
            let old = TypeId(i as u32);
            let new = self.intern(kind.clone());
            remap.insert(old, new);
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
    pub fn string(&mut self) -> TypeId {
        self.intern(TypeKind::Ref(RefTypeKind::String))
    }
    pub fn any(&mut self) -> TypeId {
        self.intern(TypeKind::Ref(RefTypeKind::Any))
    }
    pub fn option(&mut self, inner: TypeId) -> TypeId {
        self.intern(TypeKind::Value(ValueTypeKind::Option(inner)))
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
    pub fn param(&mut self, p: TypeParamType) -> TypeId {
        self.intern(TypeKind::Param(p))
    }
    pub fn star(&mut self) -> TypeId {
        self.intern(TypeKind::StarProjection)
    }

    // ----- 结构替换 -----

    /// 把 [`Subst`] 应用到 `ty`，返回新 intern 的 [`TypeId`]。
    ///
    /// 遍历函数类型 / nominal / Option / Tuple / effect 行中的所有类型与 effect
    /// 位置；未出现在替换表里的参数保持原样。
    pub fn apply_subst(&mut self, ty: TypeId, subst: &Subst) -> TypeId {
        let kind = self.kind(ty).clone();
        match kind {
            TypeKind::Param(p) => subst.get(&p).unwrap_or(ty),
            TypeKind::Ref(RefTypeKind::Function(f)) => {
                let f = self.apply_subst_function(f, subst);
                self.intern(TypeKind::Ref(RefTypeKind::Function(f)))
            }
            TypeKind::Ref(RefTypeKind::Nominal(n)) => {
                let n = self.apply_subst_nominal(n, subst);
                self.intern(TypeKind::Ref(RefTypeKind::Nominal(n)))
            }
            TypeKind::Ref(RefTypeKind::Union(u)) => {
                let variants = u
                    .variants
                    .into_iter()
                    .map(|v| self.apply_subst(v, subst))
                    .collect::<Vec<_>>();
                self.intern(TypeKind::Ref(RefTypeKind::Union(UnionType { variants })))
            }
            TypeKind::Value(ValueTypeKind::Option(inner)) => {
                let inner = self.apply_subst(inner, subst);
                self.intern(TypeKind::Value(ValueTypeKind::Option(inner)))
            }
            TypeKind::Value(ValueTypeKind::Tuple(elems)) => {
                let elems = elems
                    .into_iter()
                    .map(|e| self.apply_subst(e, subst))
                    .collect::<Vec<_>>();
                self.intern(TypeKind::Value(ValueTypeKind::Tuple(elems)))
            }
            TypeKind::Value(ValueTypeKind::Nominal(n)) => {
                let n = self.apply_subst_nominal(n, subst);
                self.intern(TypeKind::Value(ValueTypeKind::Nominal(n)))
            }
            // 标量 / Unit / Any / String / Nothing / Star：内部无参数，原样返回。
            TypeKind::Ref(RefTypeKind::Any)
            | TypeKind::Ref(RefTypeKind::String)
            | TypeKind::Value(ValueTypeKind::Unit)
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

    fn apply_subst_nominal(&mut self, mut n: NominalType, subst: &Subst) -> NominalType {
        n.args = n.args.iter().map(|&a| self.apply_subst(a, subst)).collect();
        if let Some(row) = n.eff.take() {
            n.eff = Some(self.apply_subst_row(row, subst));
        }
        n
    }

    fn apply_subst_function(&mut self, mut f: FunctionType, subst: &Subst) -> FunctionType {
        f.receiver = f.receiver.map(|r| self.apply_subst(r, subst));
        f.params = f
            .params
            .iter()
            .map(|&p| self.apply_subst(p, subst))
            .collect();
        f.return_ty = self.apply_subst(f.return_ty, subst);
        f.effects = self.apply_subst_row(f.effects, subst);
        f
    }

    /// 对一个 effect row 应用类型参数替换（pub 版本，供 MIR 单态化调用）。
    pub fn apply_subst_row(&mut self, row: EffectRow, subst: &Subst) -> EffectRow {
        self.apply_subst_row_inner(row, subst)
    }

    fn apply_subst_row_inner(&mut self, row: EffectRow, subst: &Subst) -> EffectRow {
        let terms = row
            .terms
            .iter()
            .map(|&t| self.apply_subst(t, subst))
            .collect();
        EffectRow::from_terms(terms)
    }
}

// ---------------------------------------------------------------------------
// Subst：类型参数 → 类型 的替换表
// ---------------------------------------------------------------------------

/// 类型参数替换表（键为参数身份 [`TypeParamType`]）。
#[derive(Clone, Default)]
pub struct Subst {
    entries: HashMap<TypeParamType, TypeId>,
}

impl Subst {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, param: TypeParamType, ty: TypeId) {
        self.entries.insert(param, ty);
    }

    pub fn get(&self, param: &TypeParamType) -> Option<TypeId> {
        self.entries.get(param).copied()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
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

/// effect 行渲染：`Pure`（空）/ `A` / `A + B`。
fn render_effect_row(
    store: &TypeStore,
    interner: &scoop2_base::Interner,
    row: &EffectRow,
    full_fqn: bool,
) -> String {
    if row.terms.is_empty() {
        return "Pure".to_string();
    }
    let items: Vec<String> = row
        .terms
        .iter()
        .map(|&t| render_type(store, interner, t, full_fqn))
        .collect();
    items.join(" + ")
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
        TypeKind::Param(p) => interner.resolve(p.name).into(),
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
        RefTypeKind::Any => "Any".into(),
        RefTypeKind::String => {
            if full_fqn {
                "scoop.core.String".into()
            } else {
                "String".into()
            }
        }
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
        ValueTypeKind::Option(inner) => {
            // 嵌套 Option 不拍平：`Int??` 合法。inner 若是裸标识需加括号？spec 用 `T?`
            // 后缀且对复合类型无歧义，故直接后缀。
            format!("{}?", render_type(store, interner, *inner, full_fqn))
        }
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

    fn param(name: &str, interner: &mut scoop2_base::Interner) -> TypeParamType {
        TypeParamType {
            name: interner.intern(name),
            file: FileId(0),
            span: Span::new(0, 1),
        }
    }

    fn param_at(name: &str, interner: &mut scoop2_base::Interner, off: usize) -> TypeParamType {
        TypeParamType {
            name: interner.intern(name),
            file: FileId(0),
            span: Span::new(off, off + 1),
        }
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
        let t = param("T", &mut interner);
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
        let t = param_at("T", &mut interner, 5);
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
        let t = param("T", &mut interner);
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
        // 两个同名 T，不同 span → 不同参数。
        let t1 = param_at("T", &mut interner, 1);
        let t2 = param_at("T", &mut interner, 9);
        let id1 = s.param(t1);
        let id2 = s.param(t2);
        assert_ne!(id1, id2, "same name, different decl site → distinct");
    }

    // ----- UnionType 规范化 -----

    #[test]
    fn union_flattens_drops_nothing_dedup_sorts() {
        let mut s = TypeStore::new();
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
        let interner = scoop2_base::Interner::new();
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
        let p = s.param(param("T", &mut interner));
        assert_eq!(render_type(&s, &interner, p, true), "T");
        let star = s.star();
        assert_eq!(render_type(&s, &interner, star, true), "*");
    }
}
