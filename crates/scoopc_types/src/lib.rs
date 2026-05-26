//! Shared type-system foundations for the compiler pipeline.
//!
//! This crate owns the compiler-wide type universe: `TypeId`, `TypeStore`,
//! `TypeKind`, `EffectRow`, builtin type IDs, and backend-neutral layout data.
//! Stage and fact crates should depend on this base crate rather than on the
//! `scoopc` facade.
//!
//! 编译器内部类型表示（early stage）。
//!
//! 目标（T0401）：
//! - 在编译器内部引入稳定的 `TypeId`/`TypeKind` 结构，作为 typecheck 的基础设施
//! - 显式区分引用类型（GC-managed）与值类型（copy 语义）
//! - 支持最小 builtin：`Any`/`String`/`Nothing`/`Unit`/`Bool`/`Option<T>` 与整数族 `Int/UInt/IntN/UIntN`
//! - （T0435）支持函数类型：`(A, B) -> C / R` 与 receiver function type `T.(...) -> ... / R`
//!
//! 当前阶段只提供数据结构与格式化输出；类型推断/求解、subtyping 等语义在后续任务实现。

#![forbid(unsafe_code)]

pub mod layout;

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Schema version carried by persisted compiler products.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct WireSchemaVersion {
    pub major: u16,
    pub minor: u16,
}

impl WireSchemaVersion {
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }
}

/// TypeStore/fact/LIR binary wire schema.
///
/// Version history:
/// - 1.0：初始版本
/// - 1.1：`ConeArtifactManifest` 新增 `cone_kind` 字段（P10-T04-b）；同时让旧 artifact
///   通过 `ensure_compatible` 被显式拒绝。
/// - 1.2：P10 final cleanup 后，LIR type-context facts 记录 portable `TypeStore`
///   wire format 已实现，不再携带 P7/P8 的 deferred 决策。
pub const WIRE_SCHEMA_VERSION: WireSchemaVersion = WireSchemaVersion::new(1, 2);

pub mod serde_static_str {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &&'static str, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(value)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<&'static str, D::Error>
    where
        D: Deserializer<'de>,
    {
        let owned = String::deserialize(deserializer)?;
        Ok(Box::leak(owned.into_boxed_str()))
    }
}

/// `TypeStore` 内部类型表的索引。
///
/// 说明：
/// - 目前用 `u32` 足够覆盖编译期需要的类型数量
/// - 后续若引入跨 session 的类型缓存或增量编译，可再调整表示
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct TypeId(u32);

impl TypeId {
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

/// 编译器内部的类型种类。
///
/// 这里把“引用类型 vs 值类型”作为第一层分类，便于后续：
/// - 决定布局与 ABI（value types 可内联，ref types 走对象头/指针）
/// - 决定 GC 扫描策略（ref types 需要追踪；value types 递归含 ref 字段时另行处理）
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TypeKind {
    Ref(RefTypeKind),
    Value(ValueTypeKind),
    /// `*` star projection 的内部表示。
    ///
    /// 说明：
    /// - 仅应通过类型实参位置引入（例如 `Array<*>`）；
    /// - `read_ty` 表示运行时可读视图（当前为 `Any?` 一类 boxed ref view）；
    /// - 它不是普通的 ref/value 类型：typecheck 需要保留“只读/禁写”语义。
    StarProjection(StarProjectionType),
    /// 类型参数（generic type parameter）。
    ///
    /// 说明：
    /// - 该节点用于在 typecheck 阶段表示 `T`/`U` 这类“尚未实例化”的抽象类型；
    /// - 与 Rust 类似，类型参数可能被实例化为值类型或引用类型，因此在当前阶段我们把它视为
    ///   “kind 未知”的类型：既不是 ref 也不是 value；
    /// - 需要具体 kind 的语义（例如 `Any` 顶类型、装箱、布局）应当在后续通过约束/实例化后处理。
    Param(TypeParamType),
}

impl TypeKind {
    pub fn is_ref(&self) -> bool {
        matches!(self, TypeKind::Ref(_))
    }

    pub fn is_value(&self) -> bool {
        matches!(self, TypeKind::Value(_))
    }
}

/// 引用类型（GC-managed）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum RefTypeKind {
    /// 顶层类型：所有引用类型的 supertype。
    ///
    /// 说明：值类型装箱到 `Any` 属于后续任务（PLAN §4.3）。
    Any,

    /// `String`：内建字符串类型（引用类型，GC-managed）。
    ///
    /// 说明：该类型在源级可由 sysroot 声明，但其布局/语义由编译器与运行时固定。
    String,

    /// 名义引用类型（class/interface/effect 等）。
    Nominal(NominalType),

    /// 函数类型（spec §7.5）。
    ///
    /// 说明：
    /// - 函数值在运行期以闭包/对象的形式存在，因此在内部类型表示中视为引用类型；
    /// - receiver function type（`T.(...) -> ...`）被建模为“带 receiver 的函数类型”，其子类型规则
    ///   与参数一致（逆变）。
    Function(FunctionType),

    /// 受限 union 类型：`A | B | ...`。
    ///
    /// 说明：
    /// - 该类型目前主要用于“分支结果类型合并”（if/when 的 LUB）在缺少合适公共超类型时的保守精化；
    /// - 运行时表示与更强的静态语义（例如对 union 的成员访问/智能转换）会在后续阶段逐步补齐；
    /// - 当前实现保证：
    ///   - 展平嵌套 union（`(A | B) | C` → `A | B | C`）
    ///   - 去重与稳定排序（用于稳定诊断与 fixtures 断言）
    ///   - `Nothing` 被消去（`Nothing | T` → `T`）
    ///   - `Any` 吸收其它项（`Any | T` → `Any`）
    Union(UnionType),
}

/// union 类型的规范化表示。
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct UnionType {
    /// 已规范化：排序 + 去重 + 无嵌套 union + 不包含 `Nothing`。
    pub variants: Vec<TypeId>,
}

/// 名义类型（nominal type）的最小表示。
///
/// 说明：
/// - 早期阶段（T0403）仅需要 “FQN + type args” 来完成 TypeRef lowering 与 arity 检查；
/// - 更丰富的信息（字段/方法、布局、vtable 等）会在后续阶段逐步接入。
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct NominalType {
    pub fqn: String,
    pub args: Vec<TypeId>,
    /// use-site effect row 实参（`Type<eff Row>`）。
    ///
    /// 说明：
    /// - 仅当该 nominal type 在声明处包含 `eff` row 参数时为 `Some`；
    /// - 当前阶段我们把它视为 nominal type identity 的一部分（与 type args 类似）；
    /// - 更复杂的 row 变量/约束与子类型关系留给后续任务（T0515+）。
    pub eff: Option<EffectRow>,
}

pub fn is_builtin_scalar_nominal_value_type(types: &TypeStore, ty: TypeId) -> bool {
    let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = types.kind(ty) else {
        return false;
    };
    nominal.args.is_empty() && is_builtin_scalar_nominal_value_fqn(&nominal.fqn)
}

fn is_builtin_scalar_nominal_value_fqn(fqn: &str) -> bool {
    matches!(
        fqn,
        "scoop.core.Bool"
            | "scoop.core.Char"
            | "scoop.core.Float64"
            | "scoop.core.Double"
            | "scoop.core.Float32"
            | "scoop.core.Int"
            | "scoop.core.UInt"
            | "scoop.core.UIntPtr"
            | "scoop.core.Short"
            | "scoop.core.Long"
            | "scoop.core.Byte"
            | "scoop.core.UShort"
            | "scoop.core.ULong"
    ) || fqn
        .strip_prefix("scoop.core.Int")
        .and_then(|suffix| (!suffix.is_empty()).then_some(suffix))
        .and_then(|suffix| suffix.parse::<u16>().ok())
        .is_some()
        || fqn
            .strip_prefix("scoop.core.UInt")
            .and_then(|suffix| (!suffix.is_empty()).then_some(suffix))
            .and_then(|suffix| suffix.parse::<u16>().ok())
            .is_some()
}

/// ABI convention accepted by `@Extern` metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ExternAbi {
    #[default]
    C,
    Scoop,
}

impl ExternAbi {
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "c" => Some(Self::C),
            "scoop" => Some(Self::Scoop),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::C => "c",
            Self::Scoop => "scoop",
        }
    }
}

/// `*` star projection 的最小内部表示。
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct StarProjectionType {
    /// 运行时可读视图；当前等价于 boxed `Any?`。
    pub read_ty: TypeId,
}

/// 类型参数类型（`T`）。
///
/// 注意：同名的 `T` 在不同声明里应当视为不同的类型参数，因此这里用
/// `(decl_file, decl_span)` 来唯一标识其来源（用于 Hash/Eq 与 interning）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TypeParamType {
    pub name: String,
    pub decl_file: PathBuf,
    pub decl_span: scoopc_span::Span,
}

/// Internal pseudo declaration file for effect-row type parameters.
///
/// The marker is type-system data rather than HIR-owned data because typecheck,
/// HIR lowering, and MIR materialization all need to recognize the same
/// synthetic `TypeKind::Param` identity.
pub const EFFECT_ROW_PARAM_DECL_FILE: &str = "<hir-effect-row-param>";

/// effect row（spec §5.8）的内部表示。
///
/// 当前阶段（T0435）先把 row expression 限制为“显式项的并集”（集合语义）：
/// - `Pure` 由 `terms.is_empty()` 表示
/// - `A + B + A` 会被 canonicalize 为去重后的集合
///
/// 注意：更完整的 effect polymorphism（row 变量、推断、约束求解）留给后续任务（PLAN §6）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct EffectRow {
    /// 规范化后的集合（排序 + 去重）。
    pub terms: Vec<TypeId>,
}

impl EffectRow {
    pub fn pure() -> Self {
        Self { terms: Vec::new() }
    }

    pub fn new(mut terms: Vec<TypeId>) -> Self {
        terms.sort();
        terms.dedup();
        Self { terms }
    }

    pub fn is_pure(&self) -> bool {
        self.terms.is_empty()
    }

    /// `self ⊆ other`：是否“需要不多于 other 的效果”。
    pub fn is_subset_of(&self, other: &EffectRow) -> bool {
        if self.terms.is_empty() {
            return true;
        }
        if other.terms.is_empty() {
            return false;
        }

        // `terms` 已排序；用双指针做线性子集判断。
        let mut i = 0;
        let mut j = 0;
        while i < self.terms.len() && j < other.terms.len() {
            let a = self.terms[i];
            let b = other.terms[j];
            if a == b {
                i += 1;
                j += 1;
                continue;
            }
            if a > b {
                j += 1;
                continue;
            }
            // a < b：说明 other 中缺少 a
            return false;
        }
        i == self.terms.len()
    }
}

/// 函数类型（spec §7.5）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct FunctionType {
    pub receiver: Option<TypeId>,
    pub params: Vec<TypeId>,
    pub return_ty: TypeId,
    pub effects: EffectRow,
    /// effect row 是否为闭合（`/ R!`，spec §5.8.4）。
    ///
    /// 说明：
    /// - `closed=true` 表示该函数类型的 effects 在语义上不允许“额外未声明的 effects”逃逸；
    /// - 当前阶段该标记主要用于 program boundary（`Pure!`）与 `Any` 擦除门禁（T0632）。
    pub effects_closed: bool,
}

/// 值类型（copy 语义）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ValueTypeKind {
    /// `Unit`：0 元 tuple 的语义等价物（spec §2.3.3）。
    Unit,
    /// `Nothing`：bottom / uninhabited（例如 `Raise.raise` 的返回类型）。
    Nothing,

    /// `Bool`：内建布尔类型（值类型）。
    ///
    /// 说明：该类型在源级可由 sysroot 声明，但其布局/语义由编译器与运行时固定。
    Bool,
    /// `Char`：内建 Unicode scalar value（值类型）。
    ///
    /// 说明：当前阶段把它建模为独立标量，而不是“某种整数别名”，避免把算术规则与字符语义混在一起。
    Char,
    /// `Float64`：内建双精度浮点类型（IEEE 754 binary64）。
    Float64,
    /// `Float32`：内建单精度浮点类型（IEEE 754 binary32）。
    Float32,

    /// word-sized 整数（随 target 指针宽度变化，spec §2.3.4）。
    Int,
    /// word-sized 无符号整数。
    UInt,
    /// 固定位宽有符号整数，例如 `Int32`。
    IntN(u16),
    /// 固定位宽无符号整数，例如 `UInt64`。
    UIntN(u16),

    /// `Option<T>`：nullable sugar `T?` 的 desugar 目标（spec §2.4）。
    Option(TypeId),

    /// Tuple 类型（为后续 tuple/Unit 表达式类型检查做准备）。
    Tuple(Vec<TypeId>),

    /// 名义值类型（struct/enum 等）。
    Nominal(NominalType),
}

/// 已单态化的 `TypeId`：保证整棵类型树（含 nominal args、function
/// receiver/params/return/effects、union variants、tuple elements、option
/// inner、star projection inner、nominal use-site eff row）不含 `TypeKind::Param`。
///
/// **唯一构造路径**：`TypeStore::as_mono(t: TypeId) -> Result<MonoTypeId, ParamLeak>`。
/// 故意不实现 `From<TypeId>` / `Into<TypeId>` / `unsafe`/`unchecked` 等绕过构造，
/// 以便 codegen 的 "non-codegen type 不可能进入 codegen" 不变量可以由 Rust 类型
/// 系统在 `MonoTypeId` 的传递路径上静态维持。
///
/// `inner()` accessor 仅用于 hash-cons 比较与诊断输出；任何把 `TypeId` 重新喂回
/// 到需要 `MonoTypeId` 的位置的调用都必须再走一次 `as_mono`。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct MonoTypeId(TypeId);

impl MonoTypeId {
    /// 取出底层的 `TypeId`。仅用于 hash-cons 比较 / 诊断输出。
    pub fn inner(self) -> TypeId {
        self.0
    }
}

/// `as_mono` 拒绝时返回的诊断信息。
///
/// - `offending`：第一个被发现的 `TypeKind::Param` 节点的 `TypeId`；
/// - `leak_path`：从 `as_mono` 的输入 `TypeId` 走到 `offending` 经过的嵌套位置序列
///   （顶到底）。顶层 `Param` 时为空。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ParamLeak {
    pub offending: TypeId,
    pub leak_path: Vec<TypeKindLabel>,
}

/// 描述类型树中“嵌套位置”的标签，用于在 `ParamLeak.leak_path` 中复述
/// 从输入 `TypeId` 走到 `Param` 的路径。
///
/// 共 10 个位置，覆盖当前 `TypeKind` 中所有可嵌套 `TypeId` 的语义槽。
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TypeKindLabel {
    /// 进入 `NominalType.args[index]`（含 `Ref::Nominal` 与 `Value::Nominal`）。
    NominalArg { fqn: String, index: usize },
    /// 进入 `NominalType.eff.terms[index]`（use-site effect row 实参）。
    NominalEffect { fqn: String, index: usize },
    /// 进入 `RefTypeKind::Union.variants[index]`。
    UnionVariant { index: usize },
    /// 进入 `RefTypeKind::Function.receiver`。
    FunctionReceiver,
    /// 进入 `RefTypeKind::Function.params[index]`。
    FunctionParam { index: usize },
    /// 进入 `RefTypeKind::Function.return_ty`。
    FunctionReturn,
    /// 进入 `RefTypeKind::Function.effects.terms[index]`。
    FunctionEffect { index: usize },
    /// 进入 `ValueTypeKind::Tuple.elements[index]`。
    TupleElement { index: usize },
    /// 进入 `ValueTypeKind::Option(inner)`。
    OptionInner,
    /// 进入 `TypeKind::StarProjection(inner.read_ty)`。
    StarProjectionInner,
}

/// 与 `TypeKind` 同形的并行视图：所有可嵌套 `TypeId` 的位置都暴露为 `MonoTypeId`，
/// 调用方拿到 view 时即可静态确定 children 已是单态化类型。
///
/// 由 `TypeStore::kind_mono` 按需构造；返回的 `MonoNominal::fqn` 借自 `TypeStore`。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MonoTypeKind<'a> {
    Ref(MonoRefKind<'a>),
    Value(MonoValueKind<'a>),
    StarProjection(MonoStarProjection),
}

/// `RefTypeKind` 的 `MonoTypeId` 视图。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MonoRefKind<'a> {
    Any,
    String,
    Nominal(MonoNominal<'a>),
    Function(MonoFunction),
    Union(MonoUnion),
}

/// `ValueTypeKind` 的 `MonoTypeId` 视图。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MonoValueKind<'a> {
    Unit,
    Nothing,
    Bool,
    Char,
    Float64,
    Float32,
    Int,
    UInt,
    IntN(u16),
    UIntN(u16),
    Option(MonoTypeId),
    Tuple(Vec<MonoTypeId>),
    Nominal(MonoNominal<'a>),
}

/// `NominalType` 的 `MonoTypeId` 视图（`fqn` 借自 `TypeStore`）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MonoNominal<'a> {
    pub fqn: &'a str,
    pub args: Vec<MonoTypeId>,
    pub eff: Option<MonoEffectRow>,
}

/// `FunctionType` 的 `MonoTypeId` 视图。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MonoFunction {
    pub receiver: Option<MonoTypeId>,
    pub params: Vec<MonoTypeId>,
    pub return_ty: MonoTypeId,
    pub effects: MonoEffectRow,
    pub effects_closed: bool,
}

/// `UnionType` 的 `MonoTypeId` 视图。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MonoUnion {
    pub variants: Vec<MonoTypeId>,
}

/// `EffectRow` 的 `MonoTypeId` 视图。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MonoEffectRow {
    pub terms: Vec<MonoTypeId>,
}

/// `StarProjectionType` 的 `MonoTypeId` 视图。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MonoStarProjection {
    pub read_ty: MonoTypeId,
}

/// 类型表：负责分配 `TypeId` 并存储 `TypeKind`。
///
/// 当前阶段采用“push-only arena + 简单去重（hash-cons）”：
/// - 对同构 `TypeKind` 复用同一个 `TypeId`，让早期 typecheck 可以直接用 `TypeId` 做相等比较；
/// - 更复杂的跨 session/增量 interning 可在后续需要时再演进。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TypeStore {
    kinds: Vec<TypeKind>,
    index: HashMap<TypeKind, TypeId>,
}

impl Serialize for TypeStore {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.kinds.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TypeStore {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let kinds = Vec::<TypeKind>::deserialize(deserializer)?;
        let mut store = TypeStore::new();
        for kind in kinds {
            let expected =
                TypeId(u32::try_from(store.kinds.len()).map_err(serde::de::Error::custom)?);
            let actual = store.intern(kind);
            if actual != expected {
                return Err(serde::de::Error::custom(
                    "portable TypeStore wire format contains duplicate type kind",
                ));
            }
        }
        store
            .validate_references()
            .map_err(serde::de::Error::custom)?;
        Ok(store)
    }
}

impl TypeStore {
    pub fn new() -> Self {
        Self {
            kinds: Vec::new(),
            index: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.kinds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }

    pub fn iter_ids(&self) -> impl Iterator<Item = TypeId> + '_ {
        (0..self.kinds.len()).map(|idx| TypeId(idx as u32))
    }

    pub fn kind(&self, id: TypeId) -> &TypeKind {
        &self.kinds[id.0 as usize]
    }

    pub fn validate_references(&self) -> Result<(), String> {
        for id in self.iter_ids() {
            self.validate_type_id(id, self.kind(id))?;
        }
        Ok(())
    }

    fn validate_type_id(&self, owner: TypeId, kind: &TypeKind) -> Result<(), String> {
        let check = |slot: &'static str, child: TypeId| {
            if (child.0 as usize) < self.kinds.len() {
                Ok(())
            } else {
                Err(format!(
                    "type t{} references out-of-range {slot} t{} in portable TypeStore",
                    owner.as_u32(),
                    child.as_u32()
                ))
            }
        };
        match kind {
            TypeKind::Ref(RefTypeKind::Any | RefTypeKind::String)
            | TypeKind::Value(
                ValueTypeKind::Unit
                | ValueTypeKind::Nothing
                | ValueTypeKind::Bool
                | ValueTypeKind::Char
                | ValueTypeKind::Float64
                | ValueTypeKind::Float32
                | ValueTypeKind::Int
                | ValueTypeKind::UInt
                | ValueTypeKind::IntN(_)
                | ValueTypeKind::UIntN(_),
            )
            | TypeKind::Param(_) => Ok(()),
            TypeKind::Ref(RefTypeKind::Nominal(n)) | TypeKind::Value(ValueTypeKind::Nominal(n)) => {
                for &arg in &n.args {
                    check("nominal arg", arg)?;
                }
                if let Some(eff) = &n.eff {
                    for &term in &eff.terms {
                        check("nominal effect term", term)?;
                    }
                }
                Ok(())
            }
            TypeKind::Ref(RefTypeKind::Function(f)) => {
                if let Some(receiver) = f.receiver {
                    check("function receiver", receiver)?;
                }
                for &param in &f.params {
                    check("function param", param)?;
                }
                check("function return", f.return_ty)?;
                for &term in &f.effects.terms {
                    check("function effect term", term)?;
                }
                Ok(())
            }
            TypeKind::Ref(RefTypeKind::Union(u)) => {
                for &variant in &u.variants {
                    check("union variant", variant)?;
                }
                Ok(())
            }
            TypeKind::Value(ValueTypeKind::Option(inner)) => check("option inner", *inner),
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                for &element in elements {
                    check("tuple element", element)?;
                }
                Ok(())
            }
            TypeKind::StarProjection(star) => check("star projection read type", star.read_ty),
        }
    }

    pub fn intern(&mut self, kind: TypeKind) -> TypeId {
        if let Some(id) = self.index.get(&kind).copied() {
            return id;
        }

        let id = TypeId(u32::try_from(self.kinds.len()).expect("too many types"));
        self.kinds.push(kind.clone());
        self.index.insert(kind, id);
        id
    }

    pub fn display<'a>(&'a self, id: TypeId) -> TypeDisplay<'a> {
        TypeDisplay { store: self, id }
    }

    /// T0130: 在 TypeStore 中查找匹配指定 FQN（无 type args）的 Nominal 引用类型。
    pub fn find_nominal_ref_by_fqn(&self, fqn: &str) -> Option<TypeId> {
        for id in self.iter_ids() {
            if let TypeKind::Ref(RefTypeKind::Nominal(n)) = self.kind(id)
                && n.fqn == fqn
                && n.args.is_empty()
            {
                return Some(id);
            }
        }
        None
    }

    pub fn is_ref(&self, id: TypeId) -> bool {
        self.kind(id).is_ref()
    }

    pub fn is_value(&self, id: TypeId) -> bool {
        self.kind(id).is_value()
    }

    /// 构造并返回一组常用 builtin 类型的 `TypeId`。
    pub fn intern_builtins(&mut self) -> BuiltinTypes {
        BuiltinTypes {
            any: self.intern(TypeKind::Ref(RefTypeKind::Any)),
            string: self.intern(TypeKind::Ref(RefTypeKind::String)),
            unit: self.intern(TypeKind::Value(ValueTypeKind::Unit)),
            nothing: self.intern(TypeKind::Value(ValueTypeKind::Nothing)),
            bool_: self.intern(TypeKind::Value(ValueTypeKind::Bool)),
            int: self.intern(TypeKind::Value(ValueTypeKind::Int)),
            uint: self.intern(TypeKind::Value(ValueTypeKind::UInt)),
            // Keep previously existing builtin `TypeId`s stable for HIR/test fixtures.
            char_: self.intern(TypeKind::Value(ValueTypeKind::Char)),
            float64: self.intern(TypeKind::Value(ValueTypeKind::Float64)),
            float32: self.intern(TypeKind::Value(ValueTypeKind::Float32)),
        }
    }

    /// 以只读方式回查当前 `TypeStore` 中已存在的 builtin 类型集合。
    ///
    /// 当前编译主线在进入 MIR/effect 阶段前就已经完成 builtin interning，因此这里不再要求
    /// 可变借用；若某个 builtin 尚未存在，则返回 `None` 让调用方显式处理，而不是隐式重新分配。
    pub fn builtins(&self) -> Option<BuiltinTypes> {
        Some(BuiltinTypes {
            any: self.find_builtin_ref(RefTypeKind::Any)?,
            string: self.find_builtin_ref(RefTypeKind::String)?,
            unit: self.find_builtin_value(ValueTypeKind::Unit)?,
            nothing: self.find_builtin_value(ValueTypeKind::Nothing)?,
            bool_: self.find_builtin_value(ValueTypeKind::Bool)?,
            char_: self.find_builtin_value(ValueTypeKind::Char)?,
            float64: self.find_builtin_value(ValueTypeKind::Float64)?,
            float32: self.find_builtin_value(ValueTypeKind::Float32)?,
            int: self.find_builtin_value(ValueTypeKind::Int)?,
            uint: self.find_builtin_value(ValueTypeKind::UInt)?,
        })
    }

    fn find_builtin_ref(&self, needle: RefTypeKind) -> Option<TypeId> {
        self.iter_ids()
            .find(|id| self.kind(*id) == &TypeKind::Ref(needle.clone()))
    }

    fn find_builtin_value(&self, needle: ValueTypeKind) -> Option<TypeId> {
        self.iter_ids()
            .find(|id| self.kind(*id) == &TypeKind::Value(needle.clone()))
    }

    pub fn ty_int_n(&mut self, bits: u16) -> TypeId {
        self.intern(TypeKind::Value(ValueTypeKind::IntN(bits)))
    }

    pub fn ty_uint_n(&mut self, bits: u16) -> TypeId {
        self.intern(TypeKind::Value(ValueTypeKind::UIntN(bits)))
    }

    pub fn ty_option(&mut self, inner: TypeId) -> TypeId {
        self.intern(TypeKind::Value(ValueTypeKind::Option(inner)))
    }

    pub fn ty_tuple(&mut self, elements: Vec<TypeId>) -> TypeId {
        self.intern(TypeKind::Value(ValueTypeKind::Tuple(elements)))
    }

    pub fn ty_function(
        &mut self,
        receiver: Option<TypeId>,
        params: Vec<TypeId>,
        return_ty: TypeId,
        effects: EffectRow,
        effects_closed: bool,
    ) -> TypeId {
        self.intern(TypeKind::Ref(RefTypeKind::Function(FunctionType {
            receiver,
            params,
            return_ty,
            effects,
            effects_closed,
        })))
    }

    /// 构造一个 star projection 类型（例如 `Array<*>` 中的 `*`）。
    pub fn ty_star_projection(&mut self, read_ty: TypeId) -> TypeId {
        self.intern(TypeKind::StarProjection(StarProjectionType { read_ty }))
    }

    /// 构造一个 union 类型（`A | B | ...`），并做最小规范化。
    pub fn ty_union(&mut self, variants: Vec<TypeId>) -> TypeId {
        let mut flat: Vec<TypeId> = Vec::with_capacity(variants.len());

        for v in variants {
            match self.kind(v) {
                // 展平嵌套 union。
                TypeKind::Ref(RefTypeKind::Union(u)) => flat.extend(u.variants.iter().copied()),
                // `Nothing | T` → `T`（bottom 不贡献到 union）。
                TypeKind::Value(ValueTypeKind::Nothing) => {}
                _ => flat.push(v),
            }
        }

        // `Any` 吸收其它项。
        if let Some(any_id) = flat
            .iter()
            .copied()
            .find(|id| matches!(self.kind(*id), TypeKind::Ref(RefTypeKind::Any)))
        {
            return any_id;
        }

        // 规范化：稳定排序 + 去重（用于稳定诊断与 fixtures 断言）。
        //
        // 注意：不能直接按 `TypeId` 排序，因为 `TypeId` 的分配顺序会受 intern 顺序影响；
        // 这里按格式化后的文本排序，使得输出对使用方更稳定（同时保留 `TypeId` 作为 tie-break）。
        flat.sort_by(|a, b| {
            let sa = self.display(*a).to_string();
            let sb = self.display(*b).to_string();
            sa.cmp(&sb).then_with(|| a.cmp(b))
        });
        flat.dedup();

        if flat.len() == 1 {
            return flat[0];
        }

        self.intern(TypeKind::Ref(RefTypeKind::Union(UnionType {
            variants: flat,
        })))
    }

    /// 构造一个类型参数 `TypeId`（例如 `T`）。
    pub fn ty_param(&mut self, param: TypeParamType) -> TypeId {
        self.intern(TypeKind::Param(param))
    }

    /// T0130: 将一个 TypeId 从另一个 TypeStore 复制到当前 TypeStore 中。
    ///
    /// 用途：monomorph key 中的 TypeId 来自 typecheck 阶段的 TypeStore，
    /// 但 HIR lowering 使用独立的 TypeStore。此方法递归拷贝类型结构，
    /// 保证 TypeId 在当前 store 中有效。
    pub fn re_intern_from(&mut self, other: &TypeStore, id: TypeId) -> TypeId {
        let kind = other.kind(id);
        match kind {
            TypeKind::Value(v) => match v {
                ValueTypeKind::Unit
                | ValueTypeKind::Nothing
                | ValueTypeKind::Bool
                | ValueTypeKind::Char
                | ValueTypeKind::Float64
                | ValueTypeKind::Float32
                | ValueTypeKind::Int
                | ValueTypeKind::UInt
                | ValueTypeKind::IntN(_)
                | ValueTypeKind::UIntN(_) => self.intern(kind.clone()),
                ValueTypeKind::Option(inner) => {
                    let new_inner = self.re_intern_from(other, *inner);
                    self.ty_option(new_inner)
                }
                ValueTypeKind::Tuple(elems) => {
                    let new_elems: Vec<TypeId> = elems
                        .iter()
                        .map(|&e| self.re_intern_from(other, e))
                        .collect();
                    self.ty_tuple(new_elems)
                }
                ValueTypeKind::Nominal(n) => {
                    let new_args: Vec<TypeId> = n
                        .args
                        .iter()
                        .map(|&a| self.re_intern_from(other, a))
                        .collect();
                    let new_eff = n
                        .eff
                        .as_ref()
                        .map(|e| self.re_intern_effect_row_from(other, e));
                    self.intern(TypeKind::Value(ValueTypeKind::Nominal(NominalType {
                        fqn: n.fqn.clone(),
                        args: new_args,
                        eff: new_eff,
                    })))
                }
            },
            TypeKind::Ref(r) => match r {
                RefTypeKind::Any | RefTypeKind::String => self.intern(kind.clone()),
                RefTypeKind::Nominal(n) => {
                    let new_args: Vec<TypeId> = n
                        .args
                        .iter()
                        .map(|&a| self.re_intern_from(other, a))
                        .collect();
                    let new_eff = n
                        .eff
                        .as_ref()
                        .map(|e| self.re_intern_effect_row_from(other, e));
                    self.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
                        fqn: n.fqn.clone(),
                        args: new_args,
                        eff: new_eff,
                    })))
                }
                RefTypeKind::Function(f) => {
                    let new_receiver = f.receiver.map(|r| self.re_intern_from(other, r));
                    let new_params: Vec<TypeId> = f
                        .params
                        .iter()
                        .map(|&p| self.re_intern_from(other, p))
                        .collect();
                    let new_return = self.re_intern_from(other, f.return_ty);
                    let new_effects = self.re_intern_effect_row_from(other, &f.effects);
                    self.ty_function(
                        new_receiver,
                        new_params,
                        new_return,
                        new_effects,
                        f.effects_closed,
                    )
                }
                RefTypeKind::Union(u) => {
                    let new_variants: Vec<TypeId> = u
                        .variants
                        .iter()
                        .map(|&v| self.re_intern_from(other, v))
                        .collect();
                    self.ty_union(new_variants)
                }
            },
            TypeKind::StarProjection(star) => {
                let new_read_ty = self.re_intern_from(other, star.read_ty);
                self.ty_star_projection(new_read_ty)
            }
            TypeKind::Param(p) => self.intern(TypeKind::Param(p.clone())),
        }
    }

    fn re_intern_effect_row_from(&mut self, other: &TypeStore, row: &EffectRow) -> EffectRow {
        let new_terms: Vec<TypeId> = row
            .terms
            .iter()
            .map(|&t| self.re_intern_from(other, t))
            .collect();
        EffectRow::new(new_terms)
    }
}

/// `MonoTypeId` 与 `MonoTypeKind` 的核心 API。
///
/// 设计原则：
/// - `as_mono` 是把 `TypeId` 升级为 `MonoTypeId` 的**唯一入口**，做整棵类型树的
///   深度 `Param`-free 校验；
/// - 一旦持有 `MonoTypeId`，所有后续位置（args、receiver/params/return/effects、
///   union variants、tuple elements、option inner、star projection inner、nominal
///   use-site eff row）都已被 `as_mono` 校验过，因此 `kind_mono` 返回的视图直接把
///   children 暴露为 `MonoTypeId`，无需调用方重复校验。
impl TypeStore {
    /// 把 `TypeId` 升级为 `MonoTypeId`，校验整棵类型树不含 `TypeKind::Param`。
    ///
    /// 使用迭代 worklist + visited 集合：
    /// - 避免递归类型导致栈溢出；
    /// - `visited: HashSet<TypeId>` 防止 hash-cons 复用导致的重复访问与潜在环路；
    /// - 子节点按 REVERSE 顺序入栈，使 LIFO 弹出顺序与左到右深度优先一致，
    ///   令多次调用同一 leak 输入产生相同 `leak_path`。
    pub fn as_mono(&self, id: TypeId) -> Result<MonoTypeId, ParamLeak> {
        use std::collections::HashSet;

        let mut worklist: Vec<(TypeId, Vec<TypeKindLabel>)> = vec![(id, Vec::new())];
        let mut visited: HashSet<TypeId> = HashSet::new();

        while let Some((curr, path)) = worklist.pop() {
            if !visited.insert(curr) {
                continue;
            }
            match self.kind(curr) {
                TypeKind::Param(_) => {
                    return Err(ParamLeak {
                        offending: curr,
                        leak_path: path,
                    });
                }
                TypeKind::Ref(r) => match r {
                    RefTypeKind::Any | RefTypeKind::String => {}
                    RefTypeKind::Nominal(n) => push_nominal(n, &path, &mut worklist),
                    RefTypeKind::Function(f) => push_function(f, &path, &mut worklist),
                    RefTypeKind::Union(u) => push_union(u, &path, &mut worklist),
                },
                TypeKind::Value(v) => match v {
                    ValueTypeKind::Unit
                    | ValueTypeKind::Nothing
                    | ValueTypeKind::Bool
                    | ValueTypeKind::Char
                    | ValueTypeKind::Float64
                    | ValueTypeKind::Float32
                    | ValueTypeKind::Int
                    | ValueTypeKind::UInt
                    | ValueTypeKind::IntN(_)
                    | ValueTypeKind::UIntN(_) => {}
                    ValueTypeKind::Option(inner) => {
                        let mut p = path.clone();
                        p.push(TypeKindLabel::OptionInner);
                        worklist.push((*inner, p));
                    }
                    ValueTypeKind::Tuple(elems) => push_tuple(elems, &path, &mut worklist),
                    ValueTypeKind::Nominal(n) => push_nominal(n, &path, &mut worklist),
                },
                TypeKind::StarProjection(star) => {
                    let mut p = path.clone();
                    p.push(TypeKindLabel::StarProjectionInner);
                    worklist.push((star.read_ty, p));
                }
            }
        }

        Ok(MonoTypeId(id))
    }

    /// 在 `MonoTypeId` 上拿到 `TypeKind` 的并行视图：所有 children 已是 `MonoTypeId`。
    ///
    /// 若 `MonoTypeId` 通过合法构造路径（`as_mono`）取得，本方法不会触发 `Param`
    /// 分支；否则触发 `unreachable!`（`MonoTypeId` 的不变量被绕过）。
    pub fn kind_mono(&self, id: MonoTypeId) -> MonoTypeKind<'_> {
        match self.kind(id.inner()) {
            TypeKind::Ref(r) => MonoTypeKind::Ref(self.mono_ref_kind(r)),
            TypeKind::Value(v) => MonoTypeKind::Value(self.mono_value_kind(v)),
            TypeKind::StarProjection(star) => MonoTypeKind::StarProjection(MonoStarProjection {
                read_ty: MonoTypeId(star.read_ty),
            }),
            TypeKind::Param(_) => {
                unreachable!("MonoTypeId invariant violated: Param leaked through as_mono")
            }
        }
    }

    fn mono_ref_kind<'a>(&self, r: &'a RefTypeKind) -> MonoRefKind<'a> {
        match r {
            RefTypeKind::Any => MonoRefKind::Any,
            RefTypeKind::String => MonoRefKind::String,
            RefTypeKind::Nominal(n) => MonoRefKind::Nominal(self.mono_nominal(n)),
            RefTypeKind::Function(f) => MonoRefKind::Function(self.mono_function(f)),
            RefTypeKind::Union(u) => MonoRefKind::Union(self.mono_union(u)),
        }
    }

    fn mono_value_kind<'a>(&self, v: &'a ValueTypeKind) -> MonoValueKind<'a> {
        match v {
            ValueTypeKind::Unit => MonoValueKind::Unit,
            ValueTypeKind::Nothing => MonoValueKind::Nothing,
            ValueTypeKind::Bool => MonoValueKind::Bool,
            ValueTypeKind::Char => MonoValueKind::Char,
            ValueTypeKind::Float64 => MonoValueKind::Float64,
            ValueTypeKind::Float32 => MonoValueKind::Float32,
            ValueTypeKind::Int => MonoValueKind::Int,
            ValueTypeKind::UInt => MonoValueKind::UInt,
            ValueTypeKind::IntN(b) => MonoValueKind::IntN(*b),
            ValueTypeKind::UIntN(b) => MonoValueKind::UIntN(*b),
            ValueTypeKind::Option(inner) => MonoValueKind::Option(MonoTypeId(*inner)),
            ValueTypeKind::Tuple(elems) => {
                MonoValueKind::Tuple(elems.iter().copied().map(MonoTypeId).collect())
            }
            ValueTypeKind::Nominal(n) => MonoValueKind::Nominal(self.mono_nominal(n)),
        }
    }

    fn mono_nominal<'a>(&self, n: &'a NominalType) -> MonoNominal<'a> {
        MonoNominal {
            fqn: &n.fqn,
            args: n.args.iter().copied().map(MonoTypeId).collect(),
            eff: n.eff.as_ref().map(mono_effect_row),
        }
    }

    fn mono_function(&self, f: &FunctionType) -> MonoFunction {
        MonoFunction {
            receiver: f.receiver.map(MonoTypeId),
            params: f.params.iter().copied().map(MonoTypeId).collect(),
            return_ty: MonoTypeId(f.return_ty),
            effects: mono_effect_row(&f.effects),
            effects_closed: f.effects_closed,
        }
    }

    fn mono_union(&self, u: &UnionType) -> MonoUnion {
        MonoUnion {
            variants: u.variants.iter().copied().map(MonoTypeId).collect(),
        }
    }
}

fn mono_effect_row(r: &EffectRow) -> MonoEffectRow {
    MonoEffectRow {
        terms: r.terms.iter().copied().map(MonoTypeId).collect(),
    }
}

fn push_nominal(
    n: &NominalType,
    path: &[TypeKindLabel],
    worklist: &mut Vec<(TypeId, Vec<TypeKindLabel>)>,
) {
    // 子节点按 REVERSE 顺序入栈，使 LIFO 弹出顺序为左到右深度优先：
    // 先把后面的 effect terms 入栈，再把前面的 args 入栈。
    if let Some(eff) = &n.eff {
        for (idx, &term) in eff.terms.iter().enumerate().rev() {
            let mut p = path.to_vec();
            p.push(TypeKindLabel::NominalEffect {
                fqn: n.fqn.clone(),
                index: idx,
            });
            worklist.push((term, p));
        }
    }
    for (idx, &arg) in n.args.iter().enumerate().rev() {
        let mut p = path.to_vec();
        p.push(TypeKindLabel::NominalArg {
            fqn: n.fqn.clone(),
            index: idx,
        });
        worklist.push((arg, p));
    }
}

fn push_function(
    f: &FunctionType,
    path: &[TypeKindLabel],
    worklist: &mut Vec<(TypeId, Vec<TypeKindLabel>)>,
) {
    for (idx, &term) in f.effects.terms.iter().enumerate().rev() {
        let mut p = path.to_vec();
        p.push(TypeKindLabel::FunctionEffect { index: idx });
        worklist.push((term, p));
    }
    {
        let mut p = path.to_vec();
        p.push(TypeKindLabel::FunctionReturn);
        worklist.push((f.return_ty, p));
    }
    for (idx, &param) in f.params.iter().enumerate().rev() {
        let mut p = path.to_vec();
        p.push(TypeKindLabel::FunctionParam { index: idx });
        worklist.push((param, p));
    }
    if let Some(rec) = f.receiver {
        let mut p = path.to_vec();
        p.push(TypeKindLabel::FunctionReceiver);
        worklist.push((rec, p));
    }
}

fn push_union(
    u: &UnionType,
    path: &[TypeKindLabel],
    worklist: &mut Vec<(TypeId, Vec<TypeKindLabel>)>,
) {
    for (idx, &v) in u.variants.iter().enumerate().rev() {
        let mut p = path.to_vec();
        p.push(TypeKindLabel::UnionVariant { index: idx });
        worklist.push((v, p));
    }
}

fn push_tuple(
    elems: &[TypeId],
    path: &[TypeKindLabel],
    worklist: &mut Vec<(TypeId, Vec<TypeKindLabel>)>,
) {
    for (idx, &e) in elems.iter().enumerate().rev() {
        let mut p = path.to_vec();
        p.push(TypeKindLabel::TupleElement { index: idx });
        worklist.push((e, p));
    }
}

/// `TypeStore` 中 builtin 类型的 ID 集合。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BuiltinTypes {
    pub any: TypeId,
    pub string: TypeId,
    pub unit: TypeId,
    pub nothing: TypeId,
    pub bool_: TypeId,
    pub char_: TypeId,
    pub float64: TypeId,
    pub float32: TypeId,
    pub int: TypeId,
    pub uint: TypeId,
}

/// `TypeId` 的可格式化视图（需要 `TypeStore` 才能递归打印）。
pub struct TypeDisplay<'a> {
    store: &'a TypeStore,
    id: TypeId,
}

impl fmt::Display for TypeDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        format_type(self.store, self.id, f, 0)
    }
}

fn format_type(
    store: &TypeStore,
    id: TypeId,
    f: &mut fmt::Formatter<'_>,
    depth: usize,
) -> fmt::Result {
    // 防御性：后续引入递归类型（例如自引用 struct）时避免栈爆。
    if depth > 64 {
        return write!(f, "<type-recursion>");
    }

    match store.kind(id) {
        TypeKind::Ref(RefTypeKind::Any) => write!(f, "Any"),
        TypeKind::Ref(RefTypeKind::String) => write!(f, "String"),
        TypeKind::Ref(RefTypeKind::Nominal(n)) => format_nominal(store, n, f, depth),
        TypeKind::Ref(RefTypeKind::Function(fun)) => format_function_type(store, fun, f, depth),
        TypeKind::Ref(RefTypeKind::Union(u)) => format_union_type(store, u, f, depth),
        TypeKind::StarProjection(_) => write!(f, "*"),
        TypeKind::Value(ValueTypeKind::Unit) => write!(f, "Unit"),
        TypeKind::Value(ValueTypeKind::Nothing) => write!(f, "Nothing"),
        TypeKind::Value(ValueTypeKind::Bool) => write!(f, "Bool"),
        TypeKind::Value(ValueTypeKind::Char) => write!(f, "Char"),
        TypeKind::Value(ValueTypeKind::Float64) => write!(f, "Float64"),
        TypeKind::Value(ValueTypeKind::Float32) => write!(f, "Float32"),
        TypeKind::Value(ValueTypeKind::Int) => write!(f, "Int"),
        TypeKind::Value(ValueTypeKind::UInt) => write!(f, "UInt"),
        TypeKind::Value(ValueTypeKind::IntN(bits)) => write!(f, "Int{bits}"),
        TypeKind::Value(ValueTypeKind::UIntN(bits)) => write!(f, "UInt{bits}"),
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            write!(f, "Option<")?;
            format_type(store, *inner, f, depth + 1)?;
            write!(f, ">")
        }
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
            write!(f, "(")?;
            for (idx, element) in elements.iter().copied().enumerate() {
                if idx != 0 {
                    write!(f, ", ")?;
                }
                format_type(store, element, f, depth + 1)?;
            }
            if elements.len() == 1 {
                // 单元素 tuple 需要 trailing comma 以避免与括号表达式混淆。
                write!(f, ",")?;
            }
            write!(f, ")")
        }
        TypeKind::Value(ValueTypeKind::Nominal(n)) => format_nominal(store, n, f, depth),
        TypeKind::Param(p) => write!(f, "{}", p.name),
    }
}

fn format_union_type(
    store: &TypeStore,
    union: &UnionType,
    f: &mut fmt::Formatter<'_>,
    depth: usize,
) -> fmt::Result {
    for (idx, v) in union.variants.iter().copied().enumerate() {
        if idx != 0 {
            write!(f, " | ")?;
        }
        format_type(store, v, f, depth + 1)?;
    }
    Ok(())
}

fn format_function_type(
    store: &TypeStore,
    fun: &FunctionType,
    f: &mut fmt::Formatter<'_>,
    depth: usize,
) -> fmt::Result {
    if let Some(receiver) = fun.receiver {
        format_type(store, receiver, f, depth + 1)?;
        write!(f, ".")?;
    }

    write!(f, "(")?;
    for (idx, param) in fun.params.iter().copied().enumerate() {
        if idx != 0 {
            write!(f, ", ")?;
        }
        format_type(store, param, f, depth + 1)?;
    }
    write!(f, ") -> ")?;
    format_type(store, fun.return_ty, f, depth + 1)?;

    // 当前阶段统一显示 effect row（即使是 Pure），避免在诊断中丢失信息。
    write!(f, " / ")?;
    format_effect_row(store, &fun.effects, f, depth + 1)?;
    if fun.effects_closed {
        write!(f, "!")?;
    }
    Ok(())
}

fn format_effect_row(
    store: &TypeStore,
    row: &EffectRow,
    f: &mut fmt::Formatter<'_>,
    depth: usize,
) -> fmt::Result {
    if row.is_pure() {
        return write!(f, "Pure");
    }

    if row.terms.len() == 1 {
        return format_type(store, row.terms[0], f, depth + 1);
    }

    write!(f, "(")?;
    for (idx, term) in row.terms.iter().copied().enumerate() {
        if idx != 0 {
            write!(f, " + ")?;
        }
        format_type(store, term, f, depth + 1)?;
    }
    write!(f, ")")
}

fn format_nominal(
    store: &TypeStore,
    nominal: &NominalType,
    f: &mut fmt::Formatter<'_>,
    depth: usize,
) -> fmt::Result {
    write!(f, "{}", nominal.fqn)?;
    if !nominal.args.is_empty() || nominal.eff.is_some() {
        write!(f, "<")?;
        for (idx, arg) in nominal.args.iter().copied().enumerate() {
            if idx != 0 {
                write!(f, ", ")?;
            }
            format_type(store, arg, f, depth + 1)?;
        }
        if let Some(eff) = &nominal.eff {
            if !nominal.args.is_empty() {
                write!(f, ", ")?;
            }
            write!(f, "eff ")?;
            format_effect_row(store, eff, f, depth + 1)?;
        }
        write!(f, ">")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_display_formats_builtins_and_composites() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();

        assert_eq!(tys.display(builtins.any).to_string(), "Any");
        assert_eq!(tys.display(builtins.string).to_string(), "String");
        assert_eq!(tys.display(builtins.unit).to_string(), "Unit");
        assert_eq!(tys.display(builtins.nothing).to_string(), "Nothing");
        assert_eq!(tys.display(builtins.bool_).to_string(), "Bool");
        assert_eq!(tys.display(builtins.char_).to_string(), "Char");
        assert_eq!(tys.display(builtins.float64).to_string(), "Float64");
        assert_eq!(tys.display(builtins.float32).to_string(), "Float32");
        assert_eq!(tys.display(builtins.int).to_string(), "Int");
        assert_eq!(tys.display(builtins.uint).to_string(), "UInt");

        let int32 = tys.ty_int_n(32);
        let uint64 = tys.ty_uint_n(64);
        assert_eq!(tys.display(int32).to_string(), "Int32");
        assert_eq!(tys.display(uint64).to_string(), "UInt64");

        let opt_int32 = tys.ty_option(int32);
        assert_eq!(tys.display(opt_int32).to_string(), "Option<Int32>");

        let tuple = tys.ty_tuple(vec![builtins.int, builtins.uint]);
        assert_eq!(tys.display(tuple).to_string(), "(Int, UInt)");
    }

    #[test]
    fn type_display_formats_function_types_with_effects() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();

        let pure = tys.ty_function(
            None,
            vec![builtins.any],
            builtins.any,
            EffectRow::pure(),
            false,
        );
        assert_eq!(tys.display(pure).to_string(), "(Any) -> Any / Pure");

        let raise_any = tys.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "scoop.core.Raise".to_string(),
            args: vec![builtins.any],
            eff: None,
        })));
        let effectful = tys.ty_function(
            Some(builtins.string),
            vec![builtins.any],
            builtins.any,
            EffectRow::new(vec![raise_any]),
            false,
        );
        assert_eq!(
            tys.display(effectful).to_string(),
            "String.(Any) -> Any / scoop.core.Raise<Any>"
        );
    }

    #[test]
    fn portable_type_store_round_trip_preserves_generic_effect_and_projection_types() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let generic_t = types.ty_param(TypeParamType {
            name: "T".to_string(),
            decl_file: std::path::PathBuf::from("generic.scoop"),
            decl_span: scoopc_span::Span::new(10, 11),
        });
        let effect = types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "app.Log".to_string(),
            args: vec![generic_t],
            eff: None,
        })));
        let tuple = types.ty_tuple(vec![generic_t, builtins.string]);
        let star = types.ty_star_projection(builtins.any);
        let union = types.ty_union(vec![star, builtins.string]);
        let option_string = types.ty_option(builtins.string);
        let callable = types.ty_function(
            Some(generic_t),
            vec![builtins.int, tuple, union],
            option_string,
            EffectRow::new(vec![effect]),
            true,
        );
        let generic_box = types.intern(TypeKind::Value(ValueTypeKind::Nominal(NominalType {
            fqn: "app.Box".to_string(),
            args: vec![generic_t, callable],
            eff: Some(EffectRow::new(vec![effect])),
        })));

        let bytes = bincode::serialize(&types).expect("serialize portable TypeStore");
        let decoded: TypeStore =
            bincode::deserialize(&bytes).expect("deserialize portable TypeStore");

        assert_eq!(decoded, types);
        assert_eq!(
            decoded.display(callable).to_string(),
            types.display(callable).to_string()
        );
        assert_eq!(
            decoded.display(generic_box).to_string(),
            types.display(generic_box).to_string()
        );
        decoded
            .validate_references()
            .expect("decoded TypeStore references are valid");
    }

    #[test]
    fn effect_row_canonicalizes_and_checks_subset() {
        let mut tys = TypeStore::new();

        let a = tys.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "fixtures.EffectA".to_string(),
            args: Vec::new(),
            eff: None,
        })));
        let b = tys.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "fixtures.EffectB".to_string(),
            args: Vec::new(),
            eff: None,
        })));

        let pure = EffectRow::pure();
        let row_a = EffectRow::new(vec![a]);
        let row_ab = EffectRow::new(vec![a, b]);
        let row_ba_dup = EffectRow::new(vec![b, a, b]);

        // `+` 的集合语义：去重 + 顺序归一化。
        assert_eq!(row_ab, row_ba_dup);

        // containment / subeffecting：`R1 ⊆ R2`。
        assert!(pure.is_subset_of(&row_a));
        assert!(row_a.is_subset_of(&row_ab));
        assert!(!row_ab.is_subset_of(&row_a));
        assert!(!row_a.is_subset_of(&pure));
    }

    #[test]
    fn type_display_formats_nominal_with_effect_row_arg() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();

        let async_eff = tys.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "fixtures.Async".to_string(),
            args: Vec::new(),
            eff: None,
        })));
        let disposable_async = tys.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "fixtures.Disposable".to_string(),
            args: Vec::new(),
            eff: Some(EffectRow::new(vec![async_eff])),
        })));
        assert_eq!(
            tys.display(disposable_async).to_string(),
            "fixtures.Disposable<eff fixtures.Async>"
        );

        let disposable_pure = tys.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "fixtures.Disposable".to_string(),
            args: vec![builtins.any],
            eff: Some(EffectRow::pure()),
        })));
        assert_eq!(
            tys.display(disposable_pure).to_string(),
            "fixtures.Disposable<Any, eff Pure>"
        );
    }

    #[test]
    fn type_display_formats_union_types_and_canonicalizes() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();

        let u = tys.ty_union(vec![builtins.int, builtins.string]);
        assert_eq!(tys.display(u).to_string(), "Int | String");

        // `Nothing` 被消去，`Any` 吸收其它项，嵌套 union 被展平且去重。
        let nested = tys.ty_union(vec![builtins.nothing, u, builtins.string]);
        assert_eq!(tys.display(nested).to_string(), "Int | String");

        let any_absorb = tys.ty_union(vec![builtins.any, builtins.int]);
        assert_eq!(tys.display(any_absorb).to_string(), "Any");
    }

    #[test]
    fn type_kind_knows_ref_vs_value() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();
        let opt_int = tys.ty_option(builtins.int);

        assert!(tys.is_ref(builtins.any));
        assert!(!tys.is_value(builtins.any));

        assert!(tys.is_ref(builtins.string));
        assert!(!tys.is_value(builtins.string));

        assert!(tys.is_value(builtins.bool_));
        assert!(!tys.is_ref(builtins.bool_));

        assert!(tys.is_value(builtins.int));
        assert!(!tys.is_ref(builtins.int));

        assert!(tys.is_value(opt_int));
    }

    // ---- MonoTypeId / as_mono / kind_mono baseline ----

    fn make_param(tys: &mut TypeStore, name: &str) -> TypeId {
        tys.ty_param(TypeParamType {
            name: name.to_string(),
            decl_file: PathBuf::from("/test/decl.scoop"),
            decl_span: scoopc_span::Span::synthetic_prelude(),
        })
    }

    fn make_nominal_ref(tys: &mut TypeStore, fqn: &str, args: Vec<TypeId>) -> TypeId {
        tys.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: fqn.to_string(),
            args,
            eff: None,
        })))
    }

    fn make_nominal_ref_with_eff(
        tys: &mut TypeStore,
        fqn: &str,
        args: Vec<TypeId>,
        eff: EffectRow,
    ) -> TypeId {
        tys.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: fqn.to_string(),
            args,
            eff: Some(eff),
        })))
    }

    #[test]
    fn as_mono_accepts_scalars_and_builtin_refs() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();

        for id in [
            builtins.int,
            builtins.uint,
            builtins.bool_,
            builtins.char_,
            builtins.unit,
            builtins.nothing,
            builtins.float64,
            builtins.float32,
            builtins.any,
            builtins.string,
        ] {
            let mono = tys.as_mono(id).expect("scalar/builtin must be mono");
            assert_eq!(mono.inner(), id);
        }

        let int_n = tys.ty_int_n(7);
        let uint_n = tys.ty_uint_n(13);
        assert_eq!(tys.as_mono(int_n).unwrap().inner(), int_n);
        assert_eq!(tys.as_mono(uint_n).unwrap().inner(), uint_n);
    }

    #[test]
    fn as_mono_rejects_top_level_param_with_empty_path() {
        let mut tys = TypeStore::new();
        let t = make_param(&mut tys, "T");

        let err = tys.as_mono(t).unwrap_err();
        assert_eq!(err.offending, t);
        assert!(
            err.leak_path.is_empty(),
            "top-level Param should have empty leak_path, got {:?}",
            err.leak_path
        );
    }

    #[test]
    fn as_mono_rejects_nested_nominal_arg_param() {
        let mut tys = TypeStore::new();
        let t = make_param(&mut tys, "T");
        let box_t = make_nominal_ref(&mut tys, "scoop.test.Box", vec![t]);

        let err = tys.as_mono(box_t).unwrap_err();
        assert_eq!(err.offending, t);
        assert_eq!(
            err.leak_path,
            vec![TypeKindLabel::NominalArg {
                fqn: "scoop.test.Box".to_string(),
                index: 0,
            }]
        );
    }

    #[test]
    fn as_mono_accepts_nested_nominal_arg_concrete() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();
        let box_int = make_nominal_ref(&mut tys, "scoop.test.Box", vec![builtins.int]);

        let mono = tys.as_mono(box_int).expect("Box<Int> should be mono");
        assert_eq!(mono.inner(), box_int);
    }

    #[test]
    fn as_mono_rejects_nominal_eff_row_param() {
        let mut tys = TypeStore::new();
        let t = make_param(&mut tys, "T");
        let foo_with_param_eff = make_nominal_ref_with_eff(
            &mut tys,
            "scoop.test.Foo",
            Vec::new(),
            EffectRow::new(vec![t]),
        );

        let err = tys.as_mono(foo_with_param_eff).unwrap_err();
        assert_eq!(err.offending, t);
        assert_eq!(
            err.leak_path,
            vec![TypeKindLabel::NominalEffect {
                fqn: "scoop.test.Foo".to_string(),
                index: 0,
            }]
        );
    }

    #[test]
    fn as_mono_rejects_tuple_element_param() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();
        let t = make_param(&mut tys, "T");
        let tup = tys.ty_tuple(vec![builtins.int, t]);

        let err = tys.as_mono(tup).unwrap_err();
        assert_eq!(err.offending, t);
        assert_eq!(
            err.leak_path,
            vec![TypeKindLabel::TupleElement { index: 1 }]
        );
    }

    #[test]
    fn as_mono_accepts_tuple_with_concrete_elements() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();
        let tup = tys.ty_tuple(vec![builtins.int, builtins.string]);

        let mono = tys.as_mono(tup).expect("(Int, String) should be mono");
        assert_eq!(mono.inner(), tup);
    }

    #[test]
    fn as_mono_rejects_option_inner_param() {
        let mut tys = TypeStore::new();
        let t = make_param(&mut tys, "T");
        let opt_t = tys.ty_option(t);

        let err = tys.as_mono(opt_t).unwrap_err();
        assert_eq!(err.offending, t);
        assert_eq!(err.leak_path, vec![TypeKindLabel::OptionInner]);
    }

    #[test]
    fn as_mono_accepts_option_with_concrete_inner() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();
        let opt_bool = tys.ty_option(builtins.bool_);

        let mono = tys.as_mono(opt_bool).expect("Option<Bool> should be mono");
        assert_eq!(mono.inner(), opt_bool);
    }

    #[test]
    fn as_mono_rejects_function_param_position() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();
        let t = make_param(&mut tys, "T");
        let fun = tys.ty_function(None, vec![t], builtins.int, EffectRow::pure(), false);

        let err = tys.as_mono(fun).unwrap_err();
        assert_eq!(err.offending, t);
        assert_eq!(
            err.leak_path,
            vec![TypeKindLabel::FunctionParam { index: 0 }]
        );
    }

    #[test]
    fn as_mono_rejects_function_return_position() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();
        let t = make_param(&mut tys, "T");
        let fun = tys.ty_function(None, vec![builtins.int], t, EffectRow::pure(), false);

        let err = tys.as_mono(fun).unwrap_err();
        assert_eq!(err.offending, t);
        assert_eq!(err.leak_path, vec![TypeKindLabel::FunctionReturn]);
    }

    #[test]
    fn as_mono_rejects_function_receiver_position() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();
        let t = make_param(&mut tys, "T");
        let fun = tys.ty_function(
            Some(t),
            vec![builtins.int],
            builtins.int,
            EffectRow::pure(),
            false,
        );

        let err = tys.as_mono(fun).unwrap_err();
        assert_eq!(err.offending, t);
        assert_eq!(err.leak_path, vec![TypeKindLabel::FunctionReceiver]);
    }

    #[test]
    fn as_mono_rejects_function_effect_row_param() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();
        let t = make_param(&mut tys, "T");
        let fun = tys.ty_function(
            None,
            vec![builtins.int],
            builtins.int,
            EffectRow::new(vec![t]),
            false,
        );

        let err = tys.as_mono(fun).unwrap_err();
        assert_eq!(err.offending, t);
        assert_eq!(
            err.leak_path,
            vec![TypeKindLabel::FunctionEffect { index: 0 }]
        );
    }

    #[test]
    fn as_mono_rejects_union_variant_param() {
        let mut tys = TypeStore::new();
        let a = make_nominal_ref(&mut tys, "scoop.test.A", Vec::new());
        let b = make_nominal_ref(&mut tys, "scoop.test.B", Vec::new());
        let t = make_param(&mut tys, "T");

        // 直接通过 intern 构造 union 以保留指定的 variant 顺序（绕过 `ty_union` 的排序）。
        let u = tys.intern(TypeKind::Ref(RefTypeKind::Union(UnionType {
            variants: vec![a, b, t],
        })));

        let err = tys.as_mono(u).unwrap_err();
        assert_eq!(err.offending, t);
        assert_eq!(
            err.leak_path,
            vec![TypeKindLabel::UnionVariant { index: 2 }]
        );
    }

    #[test]
    fn as_mono_rejects_star_projection_inner_param() {
        let mut tys = TypeStore::new();
        let t = make_param(&mut tys, "T");
        let star = tys.ty_star_projection(t);

        let err = tys.as_mono(star).unwrap_err();
        assert_eq!(err.offending, t);
        assert_eq!(err.leak_path, vec![TypeKindLabel::StarProjectionInner]);
    }

    #[test]
    fn as_mono_handles_deeply_nested_nominal_without_overflow() {
        // Box<Box<Box<Int>>> — 多次嵌套同一 nominal，验证 visited 去重不会丢解。
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();
        let inner = make_nominal_ref(&mut tys, "scoop.test.Box", vec![builtins.int]);
        let mid = make_nominal_ref(&mut tys, "scoop.test.Box", vec![inner]);
        let outer = make_nominal_ref(&mut tys, "scoop.test.Box", vec![mid]);

        let mono = tys
            .as_mono(outer)
            .expect("nested concrete Box must be mono");
        assert_eq!(mono.inner(), outer);
    }

    #[test]
    fn kind_mono_children_align_with_underlying_typekind() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();

        // Tuple
        let tup = tys.ty_tuple(vec![builtins.int, builtins.string]);
        let mono_tup = tys.as_mono(tup).unwrap();
        match tys.kind_mono(mono_tup) {
            MonoTypeKind::Value(MonoValueKind::Tuple(elems)) => {
                assert_eq!(elems.len(), 2);
                assert_eq!(elems[0].inner(), builtins.int);
                assert_eq!(elems[1].inner(), builtins.string);
            }
            other => panic!("expected MonoValueKind::Tuple, got {other:?}"),
        }

        // Option
        let opt = tys.ty_option(builtins.bool_);
        let mono_opt = tys.as_mono(opt).unwrap();
        match tys.kind_mono(mono_opt) {
            MonoTypeKind::Value(MonoValueKind::Option(inner)) => {
                assert_eq!(inner.inner(), builtins.bool_);
            }
            other => panic!("expected MonoValueKind::Option, got {other:?}"),
        }

        // Nominal Ref with args
        let box_int = make_nominal_ref(&mut tys, "scoop.test.Box", vec![builtins.int]);
        let mono_box = tys.as_mono(box_int).unwrap();
        match tys.kind_mono(mono_box) {
            MonoTypeKind::Ref(MonoRefKind::Nominal(n)) => {
                assert_eq!(n.fqn, "scoop.test.Box");
                assert_eq!(n.args.len(), 1);
                assert_eq!(n.args[0].inner(), builtins.int);
                assert!(n.eff.is_none());
            }
            other => panic!("expected MonoRefKind::Nominal, got {other:?}"),
        }

        // Function with receiver / effects
        let raise_any = make_nominal_ref(&mut tys, "scoop.core.Raise", vec![builtins.any]);
        let fun = tys.ty_function(
            Some(builtins.string),
            vec![builtins.int, builtins.bool_],
            builtins.unit,
            EffectRow::new(vec![raise_any]),
            true,
        );
        let mono_fun = tys.as_mono(fun).unwrap();
        match tys.kind_mono(mono_fun) {
            MonoTypeKind::Ref(MonoRefKind::Function(f)) => {
                assert_eq!(f.receiver.map(MonoTypeId::inner), Some(builtins.string));
                assert_eq!(
                    f.params.iter().map(|m| m.inner()).collect::<Vec<_>>(),
                    vec![builtins.int, builtins.bool_]
                );
                assert_eq!(f.return_ty.inner(), builtins.unit);
                assert_eq!(f.effects.terms.len(), 1);
                assert_eq!(f.effects.terms[0].inner(), raise_any);
                assert!(f.effects_closed);
            }
            other => panic!("expected MonoRefKind::Function, got {other:?}"),
        }

        // Star projection
        let star = tys.ty_star_projection(builtins.any);
        let mono_star = tys.as_mono(star).unwrap();
        match tys.kind_mono(mono_star) {
            MonoTypeKind::StarProjection(s) => {
                assert_eq!(s.read_ty.inner(), builtins.any);
            }
            other => panic!("expected MonoTypeKind::StarProjection, got {other:?}"),
        }
    }

    #[test]
    fn kind_mono_aligns_for_union_value_nominal_and_use_site_eff_row() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();

        // Union: 跳过 ty_union 规范化，直接 intern 以保留指定 variant 顺序。
        let u = tys.intern(TypeKind::Ref(RefTypeKind::Union(UnionType {
            variants: vec![builtins.int, builtins.string, builtins.bool_],
        })));
        let mono_u = tys.as_mono(u).unwrap();
        match tys.kind_mono(mono_u) {
            MonoTypeKind::Ref(MonoRefKind::Union(MonoUnion { variants })) => {
                assert_eq!(
                    variants.iter().map(|m| m.inner()).collect::<Vec<_>>(),
                    vec![builtins.int, builtins.string, builtins.bool_]
                );
            }
            other => panic!("expected MonoRefKind::Union, got {other:?}"),
        }

        // Value::Nominal（与 Ref::Nominal 走同一个 mono_nominal helper，但视图位置不同）。
        let value_nominal = tys.intern(TypeKind::Value(ValueTypeKind::Nominal(NominalType {
            fqn: "scoop.test.ValueStruct".to_string(),
            args: vec![builtins.int],
            eff: None,
        })));
        let mono_value_nominal = tys.as_mono(value_nominal).unwrap();
        match tys.kind_mono(mono_value_nominal) {
            MonoTypeKind::Value(MonoValueKind::Nominal(n)) => {
                assert_eq!(n.fqn, "scoop.test.ValueStruct");
                assert_eq!(n.args.len(), 1);
                assert_eq!(n.args[0].inner(), builtins.int);
                assert!(n.eff.is_none());
            }
            other => panic!("expected MonoValueKind::Nominal, got {other:?}"),
        }

        // Use-site effect row（`Foo<eff Async>`）：mono_nominal.eff 把 EffectRow.terms 一一映射。
        let async_eff = make_nominal_ref(&mut tys, "scoop.test.Async", Vec::new());
        let yield_eff = make_nominal_ref(&mut tys, "scoop.test.Yield", Vec::new());
        let foo_with_eff_row = make_nominal_ref_with_eff(
            &mut tys,
            "scoop.test.Foo",
            Vec::new(),
            EffectRow::new(vec![async_eff, yield_eff]),
        );
        let mono_foo = tys.as_mono(foo_with_eff_row).unwrap();
        match tys.kind_mono(mono_foo) {
            MonoTypeKind::Ref(MonoRefKind::Nominal(n)) => {
                assert_eq!(n.fqn, "scoop.test.Foo");
                assert!(n.args.is_empty());
                let eff = n.eff.expect("use-site eff row must surface");
                // EffectRow::new 会排序 + 去重；只确认 terms 数量与 inner 一致即可。
                assert_eq!(eff.terms.len(), 2);
                let term_ids: Vec<TypeId> = eff.terms.iter().map(|m| m.inner()).collect();
                assert!(term_ids.contains(&async_eff));
                assert!(term_ids.contains(&yield_eff));
            }
            other => panic!("expected MonoRefKind::Nominal with eff row, got {other:?}"),
        }
    }

    #[test]
    fn as_mono_is_idempotent_for_accept_and_reject() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();
        let t = make_param(&mut tys, "T");
        let box_int = make_nominal_ref(&mut tys, "scoop.test.Box", vec![builtins.int]);
        let box_t = make_nominal_ref(&mut tys, "scoop.test.Box", vec![t]);

        // 通过路径：连续两次 as_mono 行为一致。
        let m1 = tys.as_mono(box_int).unwrap();
        let m2 = tys.as_mono(box_int).unwrap();
        assert_eq!(m1, m2);

        // 拒绝路径：连续两次 as_mono 给出相同 leak_path。
        let e1 = tys.as_mono(box_t).unwrap_err();
        let e2 = tys.as_mono(box_t).unwrap_err();
        assert_eq!(e1, e2);
    }
}
