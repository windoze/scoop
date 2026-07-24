//! Item 与声明（§3、§4、§5.1）。
//!
//! 结构约定：
//!
//! - [`Item`] / [`TypeMember`] 与 `Expr` / `Stmt` 同构：`{ id, span, kind }`，
//!   payload 结构体不重复携带 `id` / `span`（由外层节点承载）。
//! - [`ValDecl`] 同时服务顶层 `val/var`（§3.3，经 [`ItemKind::Val`]）与局部
//!   `val/var`（§7，经 `StmtKind::LocalVal`）；局部的 `modifiers` 恒为空。
//! - `annotation class` 就是 `modifiers` 含 `Annotation` 的 [`TypeKind::Class`]；
//!   effect 操作是 effect body 中 `body: None` 的 [`FunDecl`]。

use scoop2_base::{NodeId, Span};

use super::expr::{Block, CallArg, Expr};
use super::pattern::Pattern;
use super::types::{EffectRowExpr, TypeRef};
use super::{AnnotationUse, Ident, Modifier};

/// 顶层 item（§3 `item`）。
#[derive(Debug, Clone)]
pub struct Item {
    pub id: NodeId,
    pub span: Span,
    pub kind: ItemKind,
}

#[derive(Debug, Clone)]
pub enum ItemKind {
    TypeAlias(TypeAliasDecl),
    Fun(FunDecl),
    /// 顶层 `val` / `var`（§3.3；extension property 会被 reroute 到
    /// [`ItemKind::ExtensionProperty`]）。
    Val(ValDecl),
    ExtensionProperty(ExtensionPropertyDecl),
    Object(ObjectDecl),
    Type(TypeDecl),
}

/// `val` / `var` 区分。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValKind {
    Val,
    Var,
}

/// `typealias Name<T> = TypeRef`（§3.1）。
///
/// `eff` 行参数在 typealias 中会被 parser 拒绝（dedicated error）。
#[derive(Debug, Clone)]
pub struct TypeAliasDecl {
    pub annotations: Vec<AnnotationUse>,
    pub modifiers: Vec<Modifier>,
    pub name: Ident,
    pub type_params: Option<TypeParamList>,
    pub ty: TypeRef,
}

/// 函数声明（§3.2），同时覆盖顶层函数、成员函数与 effect 操作（§3.5）。
#[derive(Debug, Clone)]
pub struct FunDecl {
    pub annotations: Vec<AnnotationUse>,
    pub modifiers: Vec<Modifier>,
    /// 类型参数列表（§3.2：name 前或 name 后只允许出现一处；
    /// 位置对语义无影响，这里不区分）。
    pub type_params: Option<TypeParamList>,
    /// 扩展接收者：`fun Receiver.Name(...)`（§3.2 `receiverAndName`）。
    pub receiver: Option<TypeRef>,
    pub name: Ident,
    pub params: Vec<Param>,
    pub return_ty: Option<TypeRef>,
    /// effect 注解 `/ Row`（§3.2 `effectAnn`）。
    pub effect: Option<EffectRowExpr>,
    pub where_clause: Option<WhereClause>,
    /// 函数体。
    ///
    /// `None` 仅对允许省略 body 的声明合法：abstract / interface 成员与
    /// effect 操作（§3.2、§3.5）；其他位置缺 body 是 parse error，
    /// 节点仍以 `body: None` 保留（partial-but-valid，无 Missing 节点）。
    pub body: Option<FunBody>,
}

/// 函数体（§3.2 `funBody`）：块体或表达式体 `= expr`。
#[derive(Debug, Clone)]
pub enum FunBody {
    Block(Block),
    Expr(Box<Expr>),
}

/// 函数参数（§3.8 `param`）。
#[derive(Debug, Clone)]
pub struct Param {
    pub id: NodeId,
    pub span: Span,
    pub annotations: Vec<AnnotationUse>,
    pub is_vararg: bool,
    /// 参数名（`var` 也允许作为名字，用于 sysroot intrinsics，§3.8）。
    pub name: Ident,
    pub ty: Option<TypeRef>,
    pub default: Option<Expr>,
}

/// 顶层 / 局部共用的 `val` / `var` 声明（§3.3、§7）。
///
/// `id` / `span` 由外层节点（[`Item`] 或 `Stmt`）承载。
#[derive(Debug, Clone)]
pub struct ValDecl {
    pub annotations: Vec<AnnotationUse>,
    /// 顶层声明的修饰符；局部 `val/var` 恒为空。
    pub modifiers: Vec<Modifier>,
    pub kind: ValKind,
    pub binding: ValBinding,
    /// 类型注解（仅 [`ValBinding::Name`] 可携带；模式绑定直接走 `=`，§3.3）。
    pub ty: Option<TypeRef>,
    pub init: Option<Expr>,
}

/// `val/var` 的绑定形态（§3.3 `valPattern`）。
#[derive(Debug, Clone)]
pub enum ValBinding {
    Name(Ident),
    /// 解构模式（仅 `val`；`var` 解构是 parse error）。要求 `= expr` 初始化。
    Pattern(Pattern),
}

/// 类型声明：`class` / `interface` / `struct` / `enum` / `effect`（§3.4）。
#[derive(Debug, Clone)]
pub struct TypeDecl {
    pub annotations: Vec<AnnotationUse>,
    pub modifiers: Vec<Modifier>,
    pub kind: TypeKind,
    pub name: Ident,
    pub type_params: Option<TypeParamList>,
    pub primary_ctor: Option<PrimaryCtorDecl>,
    /// 超类型列表 `: Base(args), Iface`（§3.4 `superTypeList`）。
    ///
    /// `enum E : Int` 的底层类型也走这里（spec §2.3.2.1）。
    pub supertypes: Vec<SuperType>,
    pub where_clause: Option<WhereClause>,
    pub body: Option<TypeBody>,
}

/// 类型种类（§3.4 `typeKind`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    Class,
    Interface,
    Struct,
    Enum,
    Effect,
}

/// 主构造参数列表（§3.4 `primaryCtor`）。
#[derive(Debug, Clone)]
pub struct PrimaryCtorDecl {
    pub id: NodeId,
    pub span: Span,
    pub params: Vec<CtorParam>,
}

/// 主构造参数（§3.4 `ctorParam`）：`annotationUse* ('val'|'var'|'vararg')* IDENT ...`。
#[derive(Debug, Clone)]
pub struct CtorParam {
    pub id: NodeId,
    pub span: Span,
    pub annotations: Vec<AnnotationUse>,
    /// `val` / `var`（同时声明属性）；`None` 为纯构造参数。
    pub property: Option<ValKind>,
    pub is_vararg: bool,
    pub name: Ident,
    pub ty: Option<TypeRef>,
    pub default: Option<Expr>,
}

/// 超类型项：`typeRef callArgList?`（§3.4 `superType`）。
#[derive(Debug, Clone)]
pub struct SuperType {
    pub id: NodeId,
    pub span: Span,
    pub ty: TypeRef,
    /// 构造实参 `: Base(args)`；无调用为 `vec![]`。
    pub args: Vec<CallArg>,
}

/// 类型体（§3.4 `typeBody`）。
#[derive(Debug, Clone)]
pub struct TypeBody {
    pub id: NodeId,
    pub span: Span,
    pub members: Vec<TypeMember>,
}

/// 类型体成员（§3.4 `typeMember`；孤立的 `;` 空成员不保留）。
#[derive(Debug, Clone)]
pub struct TypeMember {
    pub id: NodeId,
    pub span: Span,
    pub kind: TypeMemberKind,
}

#[derive(Debug, Clone)]
pub enum TypeMemberKind {
    /// `init { ... }`（仅 class / object）。
    InitBlock(InitBlockDecl),
    /// 次构造（仅 class）。
    SecondaryCtor(SecondaryCtorDecl),
    /// enum variant（仅 enum）。
    EnumVariant(EnumVariantDecl),
    /// 命名 object 或 companion object（见 [`ObjectDecl::companion`]）。
    Object(ObjectDecl),
    Property(PropertyDecl),
    /// 成员函数；在 effect body 中是 effect 操作（`body: None`，§3.5）。
    Fun(FunDecl),
    /// 嵌套类型。
    Type(TypeDecl),
}

/// `init { ... }` 块（§3.4 `initBlock`）。
#[derive(Debug, Clone)]
pub struct InitBlockDecl {
    pub annotations: Vec<AnnotationUse>,
    pub modifiers: Vec<Modifier>,
    pub body: Block,
}

/// 次构造（§3.4 `secondaryCtor`）：body 块必有。
#[derive(Debug, Clone)]
pub struct SecondaryCtorDecl {
    pub annotations: Vec<AnnotationUse>,
    /// `constructor` 关键字 span（成员定位用）。
    pub span: Span,
    pub modifiers: Vec<Modifier>,
    pub type_params: Option<TypeParamList>,
    pub params: Vec<Param>,
    pub where_clause: Option<WhereClause>,
    /// `this(...)` / `super(...)` 委托调用。
    pub delegation: Option<CtorDelegation>,
    pub body: Block,
}

/// 构造委托调用（§3.4）。
#[derive(Debug, Clone)]
pub struct CtorDelegation {
    pub span: Span,
    pub kind: CtorDelegationKind,
    pub args: Vec<CallArg>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CtorDelegationKind {
    This,
    Super,
}

/// enum variant（§3.4 `enumVariantDecl`）：`Name(val f: T, ...) = discriminant?`。
#[derive(Debug, Clone)]
pub struct EnumVariantDecl {
    pub annotations: Vec<AnnotationUse>,
    pub name: Ident,
    pub fields: Vec<EnumVariantField>,
    /// `= expr` 判别值。
    pub discriminant: Option<Expr>,
}

/// enum variant 字段（必须 `val name: T`，无默认值、无 `var`，§3.4）。
#[derive(Debug, Clone)]
pub struct EnumVariantField {
    pub id: NodeId,
    pub span: Span,
    pub name: Ident,
    pub ty: TypeRef,
}

/// 命名 object 或 companion object（§3.4 `objectDecl` / `companionObjectDecl`）。
#[derive(Debug, Clone)]
pub struct ObjectDecl {
    pub annotations: Vec<AnnotationUse>,
    pub modifiers: Vec<Modifier>,
    pub name: Option<Ident>,
    /// 是否 companion object（`companion object`；只有它可以省略名字）。
    pub companion: bool,
    pub supertypes: Vec<SuperType>,
    pub body: Option<TypeBody>,
}

/// 属性声明（§3.6 `propertyDecl`，类型体成员）。
#[derive(Debug, Clone)]
pub struct PropertyDecl {
    pub annotations: Vec<AnnotationUse>,
    pub modifiers: Vec<Modifier>,
    pub kind: ValKind,
    pub name: Ident,
    /// 类型注解：有 `= init` 时可省略（类型推断，§3.6 normative）；
    /// 无 init 以及 delegated（`by`）属性必须显式标注。
    pub ty: Option<TypeRef>,
    /// delegated property `by expr`（与 `init` / accessors 互斥，parser 检查）。
    pub delegate: Option<Expr>,
    pub init: Option<Expr>,
    pub accessors: Vec<AccessorDecl>,
}

/// 属性访问器（§3.6 `accessor`）。
#[derive(Debug, Clone)]
pub struct AccessorDecl {
    pub id: NodeId,
    pub span: Span,
    pub kind: AccessorKind,
    /// setter 参数（`set(v)` / `set(v: T)`）；getter 恒为 `None`。
    pub param: Option<Ident>,
    pub param_ty: Option<TypeRef>,
    pub body: AccessorBody,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessorKind {
    Get,
    Set,
}

/// 访问器体（§3.6 `accessorBody`）：`= expr` 或块。
#[derive(Debug, Clone)]
pub enum AccessorBody {
    Block(Block),
    Expr(Box<Expr>),
}

/// 扩展属性（§3.7 `extensionPropertyDecl`，仅顶层）。
#[derive(Debug, Clone)]
pub struct ExtensionPropertyDecl {
    pub annotations: Vec<AnnotationUse>,
    pub modifiers: Vec<Modifier>,
    pub kind: ValKind,
    pub type_params: Option<TypeParamList>,
    pub receiver: TypeRef,
    pub name: Ident,
    /// 类型注解**必有**（§3.7：扩展属性不适用 §3.6 的推断规则）。
    pub ty: TypeRef,
    pub init: Option<Expr>,
    pub accessors: Vec<AccessorDecl>,
}

// ---------------------------------------------------------------------------
// Generics（§5.1，声明位）
// ---------------------------------------------------------------------------

/// 类型参数列表（§5.1 `typeParamList`）：`<T: bound, eff E = Pure>`。
#[derive(Debug, Clone)]
pub struct TypeParamList {
    pub id: NodeId,
    pub span: Span,
    pub params: Vec<TypeParam>,
    /// `eff E (= Row)?`：至多一个且必须是最后一项。
    pub effect_row: Option<EffectRowParam>,
}

/// 类型参数（§5.1 `typeParam`）：`variance? IDENT (':' genericBound)?`。
#[derive(Debug, Clone)]
pub struct TypeParam {
    pub id: NodeId,
    pub span: Span,
    pub variance: Option<Variance>,
    pub name: Ident,
    pub bound: Option<GenericBound>,
}

/// 型变（§5.1；`in` / `out` 是硬关键字）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variance {
    In,
    Out,
}

/// 泛型约束（§5.1 `genericBound`）。
#[derive(Debug, Clone)]
pub enum GenericBound {
    /// `ref` 约束（contextual keyword；span 记录其位置）。
    Ref(Span),
    /// `value` 约束。
    Value(Span),
    /// 类型约束（`T: Comparable<T>`）。
    Type(TypeRef),
}

/// effect 行参数（§5.1 `effRowParam`）：`eff E (= Row)?`。
#[derive(Debug, Clone)]
pub struct EffectRowParam {
    pub id: NodeId,
    pub span: Span,
    pub name: Ident,
    /// 行默认值（`<eff E = Pure>`，spec §3.4）。
    pub default: Option<EffectRowExpr>,
}

/// where 子句（§5.1 `whereClause`）。
#[derive(Debug, Clone)]
pub struct WhereClause {
    pub id: NodeId,
    pub span: Span,
    pub constraints: Vec<WhereConstraint>,
}

/// where 约束：`IDENT ':' genericBound`。
#[derive(Debug, Clone)]
pub struct WhereConstraint {
    pub id: NodeId,
    pub span: Span,
    pub name: Ident,
    pub bound: GenericBound,
}
