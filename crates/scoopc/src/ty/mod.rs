//! 编译器内部类型表示（early stage）。
//!
//! 目标（T0401）：
//! - 在编译器内部引入稳定的 `TypeId`/`TypeKind` 结构，作为 typecheck 的基础设施
//! - 显式区分引用类型（GC-managed）与值类型（copy 语义）
//! - 支持最小 builtin：`Any`/`String`/`Nothing`/`Unit`/`Bool`/`Option<T>` 与整数族 `Int/UInt/IntN/UIntN`
//! - （T0435）支持函数类型：`(A, B) -> C / R` 与 receiver function type `T.(...) -> ... / R`
//!
//! 当前阶段只提供数据结构与格式化输出；类型推断/求解、subtyping 等语义在后续任务实现。

pub mod layout;

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

/// `TypeStore` 内部类型表的索引。
///
/// 说明：
/// - 目前用 `u32` 足够覆盖编译期需要的类型数量
/// - 后续若引入跨 session 的类型缓存或增量编译，可再调整表示
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeKind {
    Ref(RefTypeKind),
    Value(ValueTypeKind),
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnionType {
    /// 已规范化：排序 + 去重 + 无嵌套 union + 不包含 `Nothing`。
    pub variants: Vec<TypeId>,
}

/// 名义类型（nominal type）的最小表示。
///
/// 说明：
/// - 早期阶段（T0403）仅需要 “FQN + type args” 来完成 TypeRef lowering 与 arity 检查；
/// - 更丰富的信息（字段/方法、布局、vtable 等）会在后续阶段逐步接入。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

/// 类型参数类型（`T`）。
///
/// 注意：同名的 `T` 在不同声明里应当视为不同的类型参数，因此这里用
/// `(decl_file, decl_span)` 来唯一标识其来源（用于 Hash/Eq 与 interning）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeParamType {
    pub name: String,
    pub decl_file: PathBuf,
    pub decl_span: crate::span::Span,
}

/// effect row（spec §5.8）的内部表示。
///
/// 当前阶段（T0435）先把 row expression 限制为“显式项的并集”（集合语义）：
/// - `Pure` 由 `terms.is_empty()` 表示
/// - `A + B + A` 会被 canonicalize 为去重后的集合
///
/// 注意：更完整的 effect polymorphism（row 变量、推断、约束求解）留给后续任务（PLAN §6）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ValueTypeKind {
    /// `Unit`：0 元 tuple 的语义等价物（spec §2.3.3）。
    Unit,
    /// `Nothing`：bottom / uninhabited（例如 `Raise.raise` 的返回类型）。
    Nothing,

    /// `Bool`：内建布尔类型（值类型）。
    ///
    /// 说明：该类型在源级可由 sysroot 声明，但其布局/语义由编译器与运行时固定。
    Bool,

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

/// 类型表：负责分配 `TypeId` 并存储 `TypeKind`。
///
/// 当前阶段采用“push-only arena + 简单去重（hash-cons）”：
/// - 对同构 `TypeKind` 复用同一个 `TypeId`，让早期 typecheck 可以直接用 `TypeId` 做相等比较；
/// - 更复杂的跨 session/增量 interning 可在后续需要时再演进。
#[derive(Debug, Default)]
pub struct TypeStore {
    kinds: Vec<TypeKind>,
    index: HashMap<TypeKind, TypeId>,
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
        }
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

/// `TypeStore` 中 builtin 类型的 ID 集合。
#[derive(Debug, Clone, Copy)]
pub struct BuiltinTypes {
    pub any: TypeId,
    pub string: TypeId,
    pub unit: TypeId,
    pub nothing: TypeId,
    pub bool_: TypeId,
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
        TypeKind::Value(ValueTypeKind::Unit) => write!(f, "Unit"),
        TypeKind::Value(ValueTypeKind::Nothing) => write!(f, "Nothing"),
        TypeKind::Value(ValueTypeKind::Bool) => write!(f, "Bool"),
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
}
