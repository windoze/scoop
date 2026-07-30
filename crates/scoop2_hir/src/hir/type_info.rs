//! [`TypeInfo`]：freeze 后的类型声明信息。
//!
//! 每个 [`TypeId`](crate::ty::TypeId) 在 frozen TypeDb 中对应一个 `TypeInfo`，
//! 描述该类型的完整声明信息（字段/方法/超类型/构造器等）。
//!
//! 设计原则：
//! - `TypeInfo` 是 enum，按类型类别分变体——每个变体只含该类别需要的信息，无 `Option`。
//! - 子信息用独立 struct（`StructTypeInfo` 等），match 时拿到完整 struct。
//! - supertypes/implements 存**直接**关系（非传递闭包）——逐级查找是查询规则。
//! - freeze 后不可变，保证完整无缺。

use scoop2_base::{FileId, Span, Symbol};

use crate::ty::{EffectRow, TypeId};

/// 类型声明信息。freeze 后的 TypeDb 中每个 TypeId 对应一个。
#[derive(Clone, Debug)]
pub enum TypeInfo {
    Primitive(PrimitiveTypeInfo),
    Tuple(TupleTypeInfo),
    Struct(StructTypeInfo),
    Class(ClassTypeInfo),
    Interface(InterfaceTypeInfo),
    Enum(EnumTypeInfo),
    Function(FunctionTypeInfo),
    Effect,
}

// ---- Primitive ----

/// 基础值类型的声明信息。
/// 标量类型可以 implement interface（如 Int : Hashable, ToString）。
#[derive(Clone, Debug)]
pub struct PrimitiveTypeInfo {
    pub kind: PrimitiveKind,
    /// 直接实现的 interface（如 Hashable/ToString/Comparable）。
    pub direct_implements: Vec<TypeId>,
}

/// 具体的基础标量种类。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrimitiveKind {
    Unit,
    Bool,
    Char,
    Int,
    UInt,
    Int8,
    Int16,
    Int32,
    Int64,
    UInt8,
    UInt16,
    UInt32,
    UInt64,
    Float32,
    Float64,
}

// ---- Tuple ----

/// 元组类型的声明信息。值结构类型（按位置），可 implement interface。
#[derive(Clone, Debug)]
pub struct TupleTypeInfo {
    pub members: Vec<TypeId>,
    pub direct_implements: Vec<TypeId>,
}

// ---- Struct ----

/// struct 类型的声明信息。值类型，无 ctor（`with` 是 memcpy + 字段写入）。
#[derive(Clone, Debug)]
pub struct StructTypeInfo {
    pub type_params: Vec<TypeParamDecl>,
    pub fields: Vec<(Symbol, TypeId)>,
    pub methods: Vec<(Symbol, Vec<Signature>)>,
    pub direct_implements: Vec<TypeId>,
}

// ---- Class ----

/// class 类型的声明信息。引用类型，有继承/构造器。
#[derive(Clone, Debug)]
pub struct ClassTypeInfo {
    pub type_params: Vec<TypeParamDecl>,
    pub fields: Vec<(Symbol, TypeId)>,
    pub methods: Vec<(Symbol, Vec<Signature>)>,
    /// 唯一基类（直接，非传递）。无基类为 None。
    pub base_class: Option<TypeId>,
    /// 直接实现的 interface（非传递）。
    pub direct_implements: Vec<TypeId>,
    pub ctor: ClassCtor,
    /// 是否 final（不可继承）。
    pub is_final: bool,
}

/// class 构造器信息（primary + secondary + super delegation）。
#[derive(Clone, Debug)]
pub struct ClassCtor {
    /// 主构造器参数布局（含非属性参数；MIR 构造链展开用）。
    pub primary_params: Vec<ClassCtorParamInfo>,
    /// 次构造器签名重载集。
    pub secondary: Vec<Signature>,
    /// `: Super(args)` 委托（可静态解析时记录）。
    pub super_delegation: Option<SuperCtorDelegation>,
}

/// class 主构造器参数信息。
#[derive(Clone, Debug)]
pub struct ClassCtorParamInfo {
    pub name: Symbol,
    pub ty: TypeId,
    /// 是否 `val`/`var` 属性参数（为 true 才贡献对象字段）。
    pub is_property: bool,
}

/// `: Super(args)` 主构造器委托的解析结果。
#[derive(Clone, Debug)]
pub struct SuperCtorDelegation {
    /// 超类 FQN。
    pub super_fqn: Symbol,
    /// base supertype 在 `TypeDecl.supertypes` 中的索引。
    pub base_index: usize,
    /// 实参类型序列。
    pub arg_tys: Vec<TypeId>,
}

// ---- Interface ----

/// interface 类型的声明信息。
#[derive(Clone, Debug)]
pub struct InterfaceTypeInfo {
    pub type_params: Vec<TypeParamDecl>,
    pub methods: Vec<(Symbol, Vec<Signature>)>,
    /// 直接扩展的父 interface（非传递）。
    pub direct_extends: Vec<TypeId>,
}

// ---- Enum ----

/// enum 类型的声明信息。值类型，可带 variant payload + 方法。
#[derive(Clone, Debug)]
pub struct EnumTypeInfo {
    pub type_params: Vec<TypeParamDecl>,
    pub variants: Vec<EnumVariantInfo>,
    pub methods: Vec<(Symbol, Vec<Signature>)>,
    pub direct_implements: Vec<TypeId>,
}

/// enum variant 的声明信息。
#[derive(Clone, Debug)]
pub struct EnumVariantInfo {
    pub name: Symbol,
    /// variant payload 字段（无 payload 为空）。
    pub fields: Vec<(Symbol, TypeId)>,
}

// ---- Function ----

/// 函数类型的声明信息。非具名类型，不能 implement interface。
/// HOF 参数/返回值为函数类型时，查 TypeInfo 得到此结构。
///
/// TODO: 后续添加 ABI 信息（调用约定等）。
#[derive(Clone, Debug)]
pub struct FunctionTypeInfo {
    /// 接收者函数类型 `T.(A) -> R` 的 `T`；普通函数为 None。
    pub receiver: Option<TypeId>,
    pub params: Vec<TypeId>,
    pub return_ty: TypeId,
    pub effects: EffectRow,
    /// `/ R!` 闭合行。
    pub closed: bool,
}

// ---- 公共子结构 ----

/// 类型参数声明信息（与 `crate::ty::TypeParamDecl` 同构，但 owned + 不含运行时 id）。
/// TODO: 考虑直接复用 `crate::ty::TypeParamDecl`。
#[derive(Clone, Debug)]
pub struct TypeParamDecl {
    /// 参数名（仅诊断/显示）。
    pub name: Symbol,
    pub span: Span,
    pub file: FileId,
    /// 变型（`in`/`out`）。
    pub variance: Option<crate::ty::Variance>,
    /// 声明 bound（降级后的类型；无 bound 为 None）。
    pub bound: Option<TypeId>,
    /// 普通类型参数 vs effect 行参数。
    pub kind: crate::ty::TypeParamKind,
}

/// 函数签名（freeze 后的 TypedSignature 等价物）。
/// TODO: 后续考虑直接复用 `crate::hir::TypedSignature` 或统一。
#[derive(Clone, Debug)]
pub struct Signature {
    pub param_types: Vec<TypeId>,
    pub return_ty: TypeId,
    pub type_param_count: usize,
    pub param_names: Vec<Symbol>,
    pub has_defaults: Vec<bool>,
    pub default_exprs: Vec<Option<crate::syntax::ast::Expr>>,
    pub effect_row: EffectRow,
    pub has_vararg: bool,
    pub decl_span: Span,
    pub decl_file: FileId,
}
