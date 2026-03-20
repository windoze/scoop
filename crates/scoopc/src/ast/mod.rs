//! AST（抽象语法树）。
//!
//! 目前阶段的 AST 目标：
//! - 足够表达“文件头（package/import）+ 顶层声明（fun/val/var 等）”的结构
//! - 节点主要用 `Span` 指回源文本，避免早期过度分配
//!
//! 注意：随着 parser/typechecker 完善，AST 结构可能会演进。

use crate::span::Span;

#[derive(Debug, Clone)]
pub struct File {
    pub package: Option<PackageDecl>,
    pub imports: Vec<ImportDecl>,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone)]
pub struct PackageDecl {
    pub span: Span,
    pub path: Vec<Ident>,
}

#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub span: Span,
    pub path: Vec<Ident>,
    pub has_star: bool,
    pub alias: Option<Ident>,
}

#[derive(Debug, Clone)]
pub enum Item {
    TypeAlias(TypeAliasDecl),
    Fun(FunDecl),
    Type(TypeDecl),
    Val(ValDecl),
}

/// 类型别名声明：`typealias Name = Type`（Appendix B.10）。
///
/// 当前阶段（T0251）仅支持：
/// - 顶层声明
/// - 非泛型 typealias（不支持 `typealias Name<T> = ...`）
/// - 语义（展开/循环检测）留给 resolver/typecheck
#[derive(Clone)]
pub struct TypeAliasDecl {
    pub span: Span,
    pub modifiers: Vec<Modifier>,
    pub name: Ident,
    pub ty: TypeRef,
}

impl std::fmt::Debug for TypeAliasDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("TypeAliasDecl");
        s.field("span", &self.span);
        if !self.modifiers.is_empty() {
            s.field("modifiers", &self.modifiers);
        }
        s.field("name", &self.name);
        s.field("ty", &self.ty);
        s.finish()
    }
}

/// 声明修饰符（modifiers）。
///
/// 说明：
/// - 目前阶段（T0245）仅做语法层“解析并存储”，不做合法性/组合校验；
/// - 解析时会做去重与排序，因此**顺序无关**（便于 fixtures/AST snapshot 稳定回归）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Modifier {
    // visibility
    Public,
    Internal,
    Private,
    // inheritance / dispatch
    Open,
    Abstract,
    Sealed,
    // misc
    Inline,
    Override,
    /// 编译期可求值/可用于编译期执行的标记（spec §6）。
    ///
    /// 说明：当前阶段仅做语法层解析与存储；语义检查与执行由后续阶段实现。
    Const,
    /// 注解类标记：`annotation class`（spec §15.2）。
    ///
    /// 当前阶段仅用于 parser 把 `annotation` 作为修饰符解析并存储；
    /// 语义限制（例如只允许用于 class、参数必须是 `val` 等）由后续阶段实现。
    Annotation,
}

/// 类型参数的变型标记（spec §3.2~§3.3）。
///
/// 说明：当前阶段（T0249）仅做语法层“解析并存储”，不做任何合法性校验。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeParamVariance {
    In,
    Out,
}

/// 声明处的类型参数（type parameter）。
///
/// 当前阶段：
/// - (T0218) 支持无约束的 `T` / `U`
/// - (T0249) 额外支持 `in T` / `out T` 声明处变型
/// - 不支持上界/下界（`:` / `where`）（留给后续任务）
#[derive(Clone)]
pub struct TypeParam {
    pub span: Span,
    pub variance: Option<TypeParamVariance>,
    pub name: Ident,
}

impl std::fmt::Debug for TypeParam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 为了保持 fixtures 的 AST snapshot 稳定回归：
        // - `variance` 为 None 时不输出该字段（与 T0218 之前保持一致）
        let mut s = f.debug_struct("TypeParam");
        s.field("span", &self.span);
        if self.variance.is_some() {
            s.field("variance", &self.variance);
        }
        s.field("name", &self.name);
        s.finish()
    }
}

/// effect row 参数（`eff E = Pure`）（spec §3.4 / §5.8）。
///
/// 说明：
/// - `eff` 是上下文关键字：只在 `<...>` 泛型参数/实参列表内部被当作关键字处理；
/// - 当前阶段（T0250）仅做语法层解析与存储：记录参数名与可选默认值；
/// - 复杂 row 约束、推断与实例化由后续阶段实现。
#[derive(Clone)]
pub struct EffectRowParam {
    pub span: Span,
    pub name: Ident,
    pub default: Option<EffectRowExpr>,
}

impl std::fmt::Debug for EffectRowParam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("EffectRowParam");
        s.field("span", &self.span);
        s.field("name", &self.name);
        if self.default.is_some() {
            s.field("default", &self.default);
        }
        s.finish()
    }
}

/// 类型声明的主构造头（primary constructor header）。
///
/// 语法形态（Kotlin 风格，简化版）：
/// - `class Name(param: T, ...)`
///
/// 说明：当前阶段（T0248）只做解析与结构化存储；
/// `val/var` 参数、参数默认值等更完整的语义会在后续阶段逐步补齐。
#[derive(Debug, Clone)]
pub struct PrimaryCtorDecl {
    pub params_span: Span,
    pub params: Vec<Param>,
}

/// 超类型（supertype）条目：`BaseType(...)` / `IInterface`。
#[derive(Clone)]
pub struct SuperType {
    pub span: Span,
    pub ty: TypeRef,
    /// 基类构造调用参数列表的 span（仅保留括号范围，不解析其中表达式）。
    pub ctor_args_span: Option<Span>,
}

impl std::fmt::Debug for SuperType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("SuperType");
        s.field("span", &self.span);
        s.field("ty", &self.ty);
        if self.ctor_args_span.is_some() {
            s.field("ctor_args_span", &self.ctor_args_span);
        }
        s.finish()
    }
}

#[derive(Clone)]
pub struct TypeDecl {
    pub span: Span,
    pub modifiers: Vec<Modifier>,
    pub kind: TypeKind,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub eff_param: Option<EffectRowParam>,
    /// 主构造头参数列表：`class Name(...)`。
    pub primary_ctor: Option<PrimaryCtorDecl>,
    /// 继承/实现列表：`class Dog(...) : Animal(...), IFoo`。
    pub supertypes: Vec<SuperType>,
    /// 类型体（`{ ... }`）。
    ///
    /// 当前阶段：
    /// - parser 仍可能仅保证括号平衡与 span 正确
    /// - 成员列表的解析会在后续任务中逐步补齐
    pub body: Option<TypeBody>,
}

impl std::fmt::Debug for TypeDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("TypeDecl");
        s.field("span", &self.span);
        if !self.modifiers.is_empty() {
            s.field("modifiers", &self.modifiers);
        }
        s.field("kind", &self.kind);
        s.field("name", &self.name);
        s.field("type_params", &self.type_params);
        if self.eff_param.is_some() {
            s.field("eff_param", &self.eff_param);
        }
        if self.primary_ctor.is_some() {
            s.field("primary_ctor", &self.primary_ctor);
        }
        if !self.supertypes.is_empty() {
            s.field("supertypes", &self.supertypes);
        }
        s.field("body", &self.body);
        s.finish()
    }
}

/// 类型体（`{ ... }`）——可包含成员列表。
///
/// 注意：这里与 `Block` 不同：
/// - `Block` 用于函数体/表达式块（后续会包含语句）
/// - `TypeBody` 用于 `class/interface/struct/enum/effect` 的成员声明列表
#[derive(Debug, Clone)]
pub struct TypeBody {
    pub span: Span,
    pub members: Vec<TypeMember>,
}

/// enum variant 声明（spec §2.3.2）。
///
/// 说明：
/// - variant 语法形态：`Name` / `Name(val field: T, ...)`
/// - 当前阶段仅做语法建模与结构化存储；更完整的 enum 语义由 typecheck/lowering 补齐（见 TODO T0425+）。
#[derive(Clone)]
pub struct EnumVariantDecl {
    pub span: Span,
    pub name: Ident,
    /// variant 携带的字段列表（用 `Param` 复用 `name + ty + default_value` 的结构）。
    ///
    /// 注意：语法上要求 `val field: T`；parser 会消费 `val` 关键字但目前不在 AST 中表达它。
    pub params: Vec<Param>,
}

impl std::fmt::Debug for EnumVariantDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("EnumVariantDecl");
        s.field("span", &self.span);
        s.field("name", &self.name);
        if !self.params.is_empty() {
            s.field("params", &self.params);
        }
        s.finish()
    }
}

/// 类型体中的成员声明（最小骨架）。
#[derive(Debug, Clone)]
pub enum TypeMember {
    EnumVariant(EnumVariantDecl),
    Property(PropertyDecl),
    Fun(FunDecl),
    Type(TypeDecl),
}

/// 属性声明（spec §10.1）。
///
/// 当前阶段（T0234）仅用于 type body 内（class/interface/struct/enum/effect）的成员；
/// 顶层/局部 `val/var` 仍使用 `ValDecl`。
#[derive(Clone)]
pub struct PropertyDecl {
    pub span: Span,
    pub modifiers: Vec<Modifier>,
    pub kind: ValKind,
    pub name: Ident,
    pub ty: Option<TypeRef>,
    /// 属性初始化表达式（例如 `var x: Int = 1`）。
    ///
    /// 注意：属性也可以是“纯计算属性”（无 backing field），此时 `init` 可能为 None。
    pub init: Option<Expr>,
    /// 自定义 getter（`get()`）。
    pub getter: Option<AccessorDecl>,
    /// 自定义 setter（`set(value)`）。
    pub setter: Option<AccessorDecl>,
}

impl std::fmt::Debug for PropertyDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("PropertyDecl");
        s.field("span", &self.span);
        if !self.modifiers.is_empty() {
            s.field("modifiers", &self.modifiers);
        }
        s.field("kind", &self.kind);
        s.field("name", &self.name);
        s.field("ty", &self.ty);
        s.field("init", &self.init);
        s.field("getter", &self.getter);
        s.field("setter", &self.setter);
        s.finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessorKind {
    Get,
    Set,
}

/// 属性 accessor 声明：`get()` / `set(value)`。
#[derive(Debug, Clone)]
pub struct AccessorDecl {
    pub span: Span,
    pub kind: AccessorKind,
    /// setter 的参数名（getter 为 None）。
    pub param: Option<Ident>,
    pub body: AccessorBody,
}

#[derive(Debug, Clone)]
pub enum AccessorBody {
    /// `{ ... }`
    Block(Block),
    /// `= expr`
    Expr(Expr),
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    Class,
    Interface,
    Struct,
    Enum,
    Effect,
}

#[derive(Clone)]
pub struct FunDecl {
    pub span: Span,
    pub modifiers: Vec<Modifier>,
    /// 扩展函数 receiver（`fun T.name(...)` 中的 `T`）。
    ///
    /// 当前阶段（T0233）仅在 parser 中解析并保留该 TypeRef；
    /// 分发规则与 codegen 会在后续任务中补齐。
    pub receiver: Option<TypeRef>,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub eff_param: Option<EffectRowParam>,
    pub params_span: Span,
    pub params: Vec<Param>,
    pub return_ty: Option<TypeRef>,
    /// 函数声明处的 effect row 标注：`/ Pure` / `/ E` / `/ (E1 + E2)`（spec §5.8）。
    pub effects: Option<EffectRowExpr>,
    pub body: FunBody,
}

impl std::fmt::Debug for FunDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("FunDecl");
        s.field("span", &self.span);
        if !self.modifiers.is_empty() {
            s.field("modifiers", &self.modifiers);
        }
        s.field("receiver", &self.receiver);
        s.field("name", &self.name);
        s.field("type_params", &self.type_params);
        if self.eff_param.is_some() {
            s.field("eff_param", &self.eff_param);
        }
        s.field("params_span", &self.params_span);
        s.field("params", &self.params);
        s.field("return_ty", &self.return_ty);
        if self.effects.is_some() {
            s.field("effects", &self.effects);
        }
        s.field("body", &self.body);
        s.finish()
    }
}

#[derive(Debug, Clone)]
pub enum FunBody {
    Block(Block),
    Missing,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub span: Span,
    /// 块内语句列表。
    ///
    /// 当前阶段（T0207）仅保证：
    /// - 能解析空语句（`;`）与表达式语句（原子表达式）
    /// - 能解析局部 `val/var` 绑定语句（T0208）
    /// - 能解析 `return` / `return expr` 语句（T0226）
    /// - 其它语句形态会以 `StmtKind::Missing` 占位（保持 cursor 前进与括号平衡）
    pub stmts: Vec<Stmt>,
}

/// 表达式（最小子集）。
///
/// 说明：当前阶段只需要支撑 initializer 与函数体解析的增量推进，因此先保留一个非常小的集合。
/// 后续任务会逐步补齐调用、成员访问、二元运算、控制流等表达式节点。
#[derive(Debug, Clone)]
pub struct Expr {
    pub span: Span,
    pub kind: ExprKind,
}

/// 字段路径：`a.b.c`（用于值类型更新 `with` 表达式等场景）。
#[derive(Debug, Clone)]
pub struct FieldPath {
    pub span: Span,
    pub segments: Vec<Ident>,
}

/// `with` 更新项：`path: value`。
#[derive(Debug, Clone)]
pub struct WithUpdateField {
    pub span: Span,
    pub path: FieldPath,
    pub colon_span: Span,
    pub value: Expr,
}

/// struct literal 的字段初始化项：`name: expr`（spec §12）。
#[derive(Debug, Clone)]
pub struct StructLitField {
    pub span: Span,
    pub name: Ident,
    pub colon_span: Span,
    pub value: Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    // arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Rem,

    // shifts
    Shl,
    Shr,

    // bitwise
    BitAnd,
    BitXor,
    BitOr,

    // comparisons
    Lt,
    Le,
    Gt,
    Ge,

    // equality
    Eq,
    Ne,

    // boolean logic
    LogAnd,
    LogOr,

    // null-coalescing / elvis
    Elvis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// `!expr`
    Not,
    /// `-expr`
    Neg,
    /// `~expr`
    BitNot,
}

/// 类型相关的表达式操作符：运行期类型判断。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeCheckOp {
    /// `expr is Type`
    Is,
    /// `expr !is Type`
    NotIs,
}

/// 类型相关的表达式操作符：显式转换。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastOp {
    /// `expr as Type`（失败时抛出/raise，由后续阶段决定）
    As,
    /// `expr as? Type`（失败时返回 `None`，由后续阶段决定）
    AsQ,
}

/// 插值字符串的片段（spec §8.2）。
#[derive(Debug, Clone)]
pub enum InterpolatedStringPart {
    /// 纯文本片段（保持源码 span；转义/去重写回等语义留给后续阶段）。
    Text { span: Span },
    /// 插值表达式片段：`{ expr }`。
    Expr { expr: Expr },
}

/// Lambda 表达式（spec §12）。
///
/// 语法形态（Kotlin 风格）：
/// - `{ params -> body }`
/// - `{ body }`
///
/// 说明：
/// - 参数类型注解可省略，通常由期望函数类型向下传播推断（spec §14.4）。
/// - `{ body }` 形式不含 `->`，因此 `arrow_span` 为 `None` 且 `params` 为空；隐式 `it` 等语义由后续 typecheck 决定。
#[derive(Debug, Clone)]
pub struct LambdaExpr {
    /// 参数列表（参数类型可省略）。
    pub params: Vec<Param>,
    /// `->` 的 span；`{ body }` 形式为 `None`。
    pub arrow_span: Option<Span>,
    /// Lambda 主体表达式。若解析为 block body，可使用 `ExprKind::Block` 表示。
    pub body: Box<Expr>,
}

/// 值上下文中的标识符引用（expression ident）。
///
/// 说明：
/// - parser 阶段仅记录 `span`，不做任何名字解析；
/// - resolve 阶段（T0305）会把解析结果写回 `resolved`，用于后续 lowering/typecheck。
#[derive(Clone, PartialEq, Eq)]
pub struct ValueIdent {
    pub span: Span,
    pub resolved: Option<ResolvedValueRef>,
}

impl ValueIdent {
    pub fn new(span: Span) -> Self {
        Self {
            span,
            resolved: None,
        }
    }
}

impl std::fmt::Debug for ValueIdent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 为了保持 parse fixtures 的 AST snapshot 稳定回归：
        // - 未解析时输出应与旧版 `Ident { span }` 完全一致；
        // - 只有在 resolver 写回 `resolved` 后才额外打印该字段。
        let mut s = f.debug_struct("Ident");
        s.field("span", &self.span);
        if self.resolved.is_some() {
            s.field("resolved", &self.resolved);
        }
        s.finish()
    }
}

/// Resolver 写回到 AST 的“值引用”解析结果（T0305）。
///
/// 当前阶段的简化：
/// - `TopLevel` 暂只记录 FQN（Fully Qualified Name），不区分 fun/value；
/// - `Local` 使用声明处 binder span 作为最小“身份标识”。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedValueRef {
    /// 局部绑定（函数参数、block 内 `val/var`）。
    Local { name: String, decl_span: Span },
    /// 顶层符号（同包或通过 import 引入）。
    TopLevel { fqn: String },
}

/// 成员访问中的标识符（`receiver.member` / `receiver?.member`）。
///
/// 说明：
/// - parser 阶段仅记录 `span`，不做任何名字解析；
/// - resolve 阶段（T0310）会把解析结果写回 `resolved`，用于后续 lowering/typecheck。
#[derive(Clone, PartialEq, Eq)]
pub struct MemberIdent {
    pub span: Span,
    pub resolved: Option<ResolvedMemberRef>,
}

impl MemberIdent {
    pub fn new(span: Span) -> Self {
        Self {
            span,
            resolved: None,
        }
    }
}

impl std::fmt::Debug for MemberIdent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 保持 parse fixtures 的 AST snapshot 稳定回归：
        // - 未解析时输出应与旧版 `Ident { span }` 完全一致；
        // - 只有在 resolver 写回 `resolved` 后才额外打印该字段。
        let mut s = f.debug_struct("Ident");
        s.field("span", &self.span);
        if self.resolved.is_some() {
            s.field("resolved", &self.resolved);
        }
        s.finish()
    }
}

/// Resolver 写回到 AST 的“成员引用”解析结果（T0310）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedMemberRef {
    /// 解析到类型体中的字段/属性（value namespace）。
    Value { fqn: String },
    /// 解析到类型体中的方法（fun namespace）。
    Fun { fqn: String },
    /// 解析到扩展属性（extension property）。
    ///
    /// 注意：当前阶段（T0312）主要用于 member access 的“同名优先级”与后续 lowering/typecheck；
    /// 具体 extension property 的语法与语义会在后续任务中逐步补齐。
    ExtensionValue { fqn: String },
    /// 解析到扩展函数（extension function）。
    ///
    /// 该变体表示 `receiver.member` 并非来自类型体成员，而是来自“同包可见”的扩展声明。
    ExtensionFun { fqn: String },
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    /// 解析失败或尚未实现时的占位节点（保持 span 以便诊断/回归）。
    Missing,
    Ident(ValueIdent),
    IntLit,
    StringLit,
    /// `()`：Unit 字面量（spec §2.3.3）。
    UnitLit,
    /// tuple 字面量：`(a, b, ...)`（spec §2.3.3）。
    ///
    /// 说明：
    /// - 空 `()` 由 `ExprKind::UnitLit` 表示；
    /// - 单元素 tuple 需写 trailing comma：`(x,)`。
    TupleLit { elements: Vec<Expr> },
    /// 插值字符串：`f"Hello, {name}!"` / `f"""...{x}..."""`（spec §8.2/§8.3）。
    ///
    /// lexer 会把整个 f-string 当作一个 token；parser 会把其拆分为 Text/Expr 片段列表。
    InterpolatedString {
        /// 是否为 raw f-string（`f"""..."""`）。
        raw: bool,
        parts: Vec<InterpolatedStringPart>,
    },
    Block(Block),
    /// Lambda 表达式：`{ params -> body }` / `{ body }`（spec §12）。
    ///
    /// 当前任务（T0221）仅引入 AST 节点建模；解析与 `{}` 歧义消解见后续任务（T0222/T0225）。
    Lambda(LambdaExpr),
    /// struct literal：`TypeName { field: expr, ... }`（spec §12）。
    ///
    /// 说明：
    /// - 当前任务（T0223）仅引入 AST 节点建模；
    /// - 解析见后续任务（T0224）；
    /// - 与 lambda 的 `{}` 歧义消解见后续任务（T0225）；
    /// - 当前阶段只支持显式字段初始化 `name: expr`（不支持省略写法）。
    StructLit {
        ty: TypePath,
        fields: Vec<StructLitField>,
    },
    /// `if (cond) thenExpr else elseExpr?`（表达式形式）。
    ///
    /// 说明：
    /// - 当前阶段（T0214）只支持括号条件；
    /// - `else` 允许缺省（语义由后续 typecheck 决定）。
    If {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Option<Box<Expr>>,
    },
    /// `when (subject) { pat -> expr; ... }`（表达式形式）。
    ///
    /// 当前阶段（T0215）仅支持 very small pattern 子集：
    /// - `is Type`
    /// - `else`
    /// - 常量字面量（int/string）
    ///
    /// 穷尽性检查与完整 pattern 语义留到后续 typecheck 阶段实现（spec §4）。
    When {
        subject: Box<Expr>,
        arms: Vec<WhenArm>,
    },
    /// 成员访问表达式：`receiver.member`（postfix）。
    ///
    /// 说明：
    /// - 当前阶段仅建模普通 `.` 成员访问；
    /// - safe-call（`?.`）使用单独的 `ExprKind::SafeMemberAccess` 表示。
    MemberAccess {
        receiver: Box<Expr>,
        member: MemberIdent,
    },
    /// Splice 字段访问：`receiver.[field]`（spec §6.4）。
    ///
    /// 说明：
    /// - 该语法用于在 `comptime` 语境下通过 `FieldMeta` 动态选择字段；
    /// - 当前阶段仅做语法建模；合法性（只能用于 `comptime for` 且 `field` 为 `FieldMeta`）由后续阶段实现。
    SpliceField {
        receiver: Box<Expr>,
        field: Box<Expr>,
    },
    /// safe-call 成员访问表达式：`receiver?.member`（postfix）（Appendix B.3.1）。
    ///
    /// 说明：仅做语法建模；desugar/运行期语义留到后续阶段（typecheck/lowering）决定。
    SafeMemberAccess {
        receiver: Box<Expr>,
        op_span: Span,
        member: MemberIdent,
    },
    /// 调用表达式：`callee(args...)`（postfix）。
    ///
    /// 当前阶段：
    /// - （T0209）支持位置参数与逗号分隔参数列表；
    /// - （T0231）支持命名参数实参：`name = expr`（仅在参数列表中生效）；
    /// - （T0232）支持 Kotlin 风格 trailing lambda：`callee { ... }` 与 `callee(args) { ... }`（作为最后一个实参）。
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    /// 命名参数实参：`name = expr`（Appendix B.5.3）。
    ///
    /// 说明：
    /// - 该节点**仅**应由调用参数列表解析产生（T0231），用于与赋值表达式 `ExprKind::Assign` 区分；
    /// - 参数重排、默认值补齐等调用语义留到后续阶段（typecheck/lowering）处理。
    NamedArg {
        name: Ident,
        eq_span: Span,
        value: Box<Expr>,
    },
    /// 非空断言：`expr!!`（postfix）。
    ///
    /// 说明：仅做语法建模；运行期异常语义留到后续阶段（typecheck/effect/codegen）决定。
    NotNullAssert {
        expr: Box<Expr>,
        op_span: Span,
    },
    /// 前缀一元运算：`!expr` / `-expr` / `~expr`（spec §2.3.4 / Appendix B.8）。
    Unary {
        op: UnaryOp,
        op_span: Span,
        expr: Box<Expr>,
    },
    /// 二元运算表达式：`lhs op rhs`。
    ///
    /// 说明：
    /// - 当前阶段（T0211）仅实现常见二元运算符的优先级与结合性（语法层面）；
    /// - 操作符重载与类型规则在 typecheck 阶段处理。
    Binary {
        lhs: Box<Expr>,
        op: BinaryOp,
        op_span: Span,
        rhs: Box<Expr>,
    },
    /// 赋值表达式：`lhs = rhs`。
    ///
    /// 说明：
    /// - 当前阶段（T0227）仅在语法层支持最小 lhs：标识符与成员访问（`a.b`）；
    /// - 复合赋值（`+=` 等）与解构赋值留到后续任务实现。
    Assign {
        lhs: Box<Expr>,
        eq_span: Span,
        rhs: Box<Expr>,
    },
    /// 运行期类型判断：`expr is Type` / `expr !is Type`。
    ///
    /// 说明：
    /// - 仅做语法建模；smart cast 与运行期语义留到后续阶段处理（typecheck/effect/codegen）。
    TypeCheck {
        expr: Box<Expr>,
        op: TypeCheckOp,
        op_span: Span,
        ty: TypeRef,
    },
    /// 显式类型转换：`expr as Type` / `expr as? Type`。
    ///
    /// 说明：仅做语法建模；失败语义（raise/返回 None）留到后续阶段处理。
    Cast {
        expr: Box<Expr>,
        op: CastOp,
        op_span: Span,
        ty: TypeRef,
    },
    /// 值类型更新表达式：`expr with { path: value, ... }`（spec §2.6）。
    ///
    /// 说明：
    /// - 当前阶段仅做语法建模；
    /// - 字段存在性、类型检查与 lowering 会在后续阶段实现（见 PLAN §4.5）。
    WithUpdate {
        base: Box<Expr>,
        with_span: Span,
        updates: Vec<WithUpdateField>,
    },
}

impl Expr {
    pub fn missing(span: Span) -> Self {
        Self {
            span,
            kind: ExprKind::Missing,
        }
    }
}

/// `when` 的一个分支（arm）：`pat -> body`。
#[derive(Debug, Clone)]
pub struct WhenArm {
    pub span: Span,
    pub pat: WhenPat,
    /// 可选 guard：`pat if <expr> -> body`。
    pub guard: Option<Expr>,
    pub arrow_span: Span,
    pub body: Expr,
}

/// `when` 分支的模式（早期最小子集）。
#[derive(Debug, Clone)]
pub enum WhenPat {
    Else { span: Span },
    Is { is_span: Span, ty: TypeRef },
    /// `_`：通配符模式（匹配任意值）。
    Wildcard { span: Span },
    /// 绑定变量模式：`x`（把匹配到的值绑定到变量 `x`）。
    ///
    /// 说明：该绑定仅在当前 when arm 的 body 作用域内可见（由 resolver 建立作用域）。
    Bind { ident: Ident },
    /// tuple 模式：`(p1, p2, ...)`。
    Tuple { span: Span, elements: Vec<WhenPat> },
    /// enum variant 模式：`Some(x)` / `None`（0 参数 variant）。
    ///
    /// 说明：
    /// - 早期阶段仅支持“位置参数”的 variant pattern；
    /// - `name` 目前不做解析写回，由 typecheck 基于 subject 的 enum 类型做约束与匹配。
    Variant {
        span: Span,
        name: Ident,
        args: Vec<WhenPat>,
    },
    IntLit { span: Span },
    StringLit { span: Span },
    /// `true` / `false`（当前阶段 lexer 仍以 ident token 承载）。
    BoolLit { span: Span },
}

impl WhenPat {
    pub fn span(&self) -> Span {
        match self {
            WhenPat::Else { span } => *span,
            WhenPat::Is { is_span, ty } => Span::new(is_span.start, ty.span().end),
            WhenPat::Wildcard { span } => *span,
            WhenPat::Bind { ident } => ident.span,
            WhenPat::Tuple { span, .. } => *span,
            WhenPat::Variant { span, .. } => *span,
            WhenPat::IntLit { span } => *span,
            WhenPat::StringLit { span } => *span,
            WhenPat::BoolLit { span } => *span,
        }
    }
}

/// 模式（pattern）——用于 `val` 解构绑定等语法位置。
///
/// 注意：当前阶段只实现解构绑定所需的最小子集（T0244）：
/// - `_` wildcard
/// - `..` rest（忽略剩余字段/元素；仅允许出现在 tuple/struct pattern 内）
/// - 绑定标识符（bind）
/// - tuple pattern：`(p1, p2, ...)`
/// - struct pattern：`TypeName { field, field: pat, .. }`
#[derive(Debug, Clone)]
pub struct Pattern {
    pub span: Span,
    pub kind: PatternKind,
}

#[derive(Debug, Clone)]
pub enum PatternKind {
    Wildcard,
    /// rest：`..`（仅用于 tuple/struct pattern 内的占位，表示忽略剩余元素/字段）。
    Rest,
    Bind(Ident),
    Tuple(Vec<Pattern>),
    Struct {
        path: TypePath,
        fields: Vec<StructPatternField>,
        /// `Some(..)` 表示出现了 `..` rest（并记录其 span，便于诊断）。
        rest: Option<Span>,
    },
    Missing,
}

impl Pattern {
    pub fn missing(span: Span) -> Self {
        Self {
            span,
            kind: PatternKind::Missing,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StructPatternField {
    pub span: Span,
    pub name: Ident,
    /// `None` 表示 shorthand：`Point { x }` 等价于 `Point { x: x }`（语义留给后续阶段）。
    pub value: Option<Box<Pattern>>,
}

/// `comptime if` 语句（spec §6.3）。
///
/// 说明：分支裁剪与“未选中分支不做类型检查”等语义由后续阶段实现；当前阶段仅做语法建模。
#[derive(Debug, Clone)]
pub struct ComptimeIf {
    pub span: Span,
    pub comptime_span: Span,
    pub if_span: Span,
    pub cond: Expr,
    pub then_branch: Block,
    pub else_branch: Option<Box<ComptimeIfElse>>,
}

#[derive(Debug, Clone)]
pub enum ComptimeIfElse {
    Block(Block),
    If(Box<ComptimeIf>),
}

/// `comptime for (x in xs) { ... }` 语句（spec §6.3）。
#[derive(Debug, Clone)]
pub struct ComptimeFor {
    pub span: Span,
    pub comptime_span: Span,
    pub for_span: Span,
    pub binder: Ident,
    pub in_span: Span,
    pub iter: Expr,
    pub body: Block,
}

/// 语句（最小骨架）。
///
/// 目前阶段仅为后续 block 解析预留结构；T0207/T0208 会逐步扩展其子集。
#[derive(Debug, Clone)]
pub struct Stmt {
    pub span: Span,
    pub kind: StmtKind,
}

#[derive(Debug, Clone)]
pub enum StmtKind {
    Empty,
    Expr(Expr),
    Val(ValDecl),
    /// `return` / `return expr`（spec §7.1/§7.3）。
    ///
    /// 说明：
    /// - 当前阶段仅支持 block 内的 return 语句；
    /// - label/non-local return 的语义留到后续阶段处理。
    Return {
        return_span: Span,
        value: Option<Expr>,
    },
    /// `while (cond) { ... }`（PLAN §4.6）。
    ///
    /// 说明：
    /// - 当前阶段只做语法解析；不在 parser 中检查 `break/continue` 的位置合法性。
    While {
        while_span: Span,
        cond: Expr,
        body: Block,
    },
    /// `break`（PLAN §4.6）。
    Break {
        break_span: Span,
    },
    /// `continue`（PLAN §4.6）。
    Continue {
        continue_span: Span,
    },
    /// `comptime { ... }` 执行块（spec §6）。
    ///
    /// 说明：当前阶段仅做语法建模；真正的编译期执行入口在后续阶段实现（见 TODO T12xx）。
    ComptimeBlock {
        comptime_span: Span,
        body: Block,
    },
    /// `comptime if (...) { ... } else ...`（spec §6.3）。
    ComptimeIf(ComptimeIf),
    /// `comptime for (x in xs) { ... }`（spec §6.3）。
    ComptimeFor(ComptimeFor),
    Missing,
}

#[derive(Clone)]
pub struct Param {
    pub name: Ident,
    pub ty: Option<TypeRef>,
    pub default_value: Option<Expr>,
}

impl std::fmt::Debug for Param {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("Param");
        s.field("name", &self.name);
        s.field("ty", &self.ty);
        if self.default_value.is_some() {
            s.field("default_value", &self.default_value);
        }
        s.finish()
    }
}

#[derive(Clone)]
pub struct ValDecl {
    pub span: Span,
    pub modifiers: Vec<Modifier>,
    pub kind: ValKind,
    pub binding: ValBinding,
    pub ty: Option<TypeRef>,
    /// 初始化表达式（当前阶段可能为 `ExprKind::Missing`，后续任务会逐步补齐解析）。
    pub init: Option<Expr>,
}

#[derive(Clone)]
pub enum ValBinding {
    Name(Ident),
    Pattern(Pattern),
}

impl ValDecl {
    pub fn name(&self) -> Option<Ident> {
        match self.binding {
            ValBinding::Name(name) => Some(name),
            ValBinding::Pattern(_) => None,
        }
    }
}

impl std::fmt::Debug for ValDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("ValDecl");
        s.field("span", &self.span);
        if !self.modifiers.is_empty() {
            s.field("modifiers", &self.modifiers);
        }
        s.field("kind", &self.kind);
        match &self.binding {
            ValBinding::Name(name) => s.field("name", name),
            ValBinding::Pattern(pat) => s.field("pattern", pat),
        };
        s.field("ty", &self.ty);
        s.field("init", &self.init);
        s.finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValKind {
    Val,
    Var,
}

#[derive(Debug, Clone)]
pub enum TypeRef {
    Path(TypePath),
    Tuple(TypeTuple),
    /// 星投影（star projection）：仅允许出现在类型实参位置，例如 `List<*>`。
    ///
    /// 说明：当前阶段（T0249）仅做语法层解析与存储；语义由后续 typecheck 实现。
    Star {
        span: Span,
    },
    /// use-site effect row 实参：仅允许出现在类型实参列表末尾，例如 `Disposable<eff Pure>`。
    ///
    /// 说明：
    /// - `eff` 是上下文关键字：仅在类型实参列表 `<...>` 内被当作关键字处理；
    /// - 当前阶段（T0253）仅做语法层解析与存储；与声明处 `eff` 参数的匹配与合法性检查留给 typecheck。
    EffectRowArg {
        span: Span,
        row: EffectRowExpr,
    },
    /// 函数类型（spec §7.5）：`(A, B) -> C / R` 或 `T.(A, B) -> C / R`
    Function(TypeFunction),
    Nullable {
        span: Span,
        inner: Box<TypeRef>,
    },
}

/// 函数类型（type position）。
///
/// 说明：
/// - `receiver` 对应 receiver function type：`T.(...) -> ...`
/// - `effects` 对应可选的 effect row：`/ Pure` 或 `/ (E1 + E2)`
#[derive(Debug, Clone)]
pub struct TypeFunction {
    pub span: Span,
    pub receiver: Option<Box<TypeRef>>,
    pub params_span: Span,
    pub params: Vec<TypeRef>,
    pub return_ty: Box<TypeRef>,
    pub effects: Option<EffectRowExpr>,
}

/// effect row 表达式（spec §5.8）。
///
/// 当前阶段（T0219）只需要语法结构：
/// - `Pure`（空 effect row）
/// - `E1 + E2 + ...`（并集；项为 effect 名/row 变量的路径）
#[derive(Debug, Clone)]
pub struct EffectRowExpr {
    pub span: Span,
    /// `terms.is_empty()` 表示 `Pure`。
    pub terms: Vec<TypePath>,
}

#[derive(Debug, Clone)]
pub struct TypePath {
    pub span: Span,
    pub segments: Vec<Ident>,
    pub args: Vec<TypeRef>,
}

#[derive(Debug, Clone)]
pub struct TypeTuple {
    pub span: Span,
    pub elements: Vec<TypeRef>,
}

impl TypeRef {
    pub fn span(&self) -> Span {
        match self {
            TypeRef::Path(p) => p.span,
            TypeRef::Tuple(t) => t.span,
            TypeRef::Star { span } => *span,
            TypeRef::EffectRowArg { span, .. } => *span,
            TypeRef::Function(f) => f.span,
            TypeRef::Nullable { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ident {
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lambda_ast_node_is_constructible() {
        let arrow_span = Span::new(3, 5);
        let body = Expr {
            span: Span::new(5, 6),
            kind: ExprKind::IntLit,
        };
        let lambda = Expr {
            span: Span::new(0, 6),
            kind: ExprKind::Lambda(LambdaExpr {
                params: vec![Param {
                    name: Ident {
                        span: Span::new(1, 2),
                    },
                    ty: None,
                    default_value: None,
                }],
                arrow_span: Some(arrow_span),
                body: Box::new(body),
            }),
        };

        match &lambda.kind {
            ExprKind::Lambda(l) => {
                assert_eq!(l.params.len(), 1);
                assert_eq!(l.arrow_span, Some(arrow_span));
            }
            other => panic!("expected Lambda, got {other:?}"),
        }
    }

    #[test]
    fn struct_lit_ast_node_is_constructible() {
        let ty = TypePath {
            span: Span::new(0, 5),
            segments: vec![Ident {
                span: Span::new(0, 5),
            }],
            args: vec![],
        };

        let field_name = Ident {
            span: Span::new(8, 9),
        };
        let value = Expr {
            span: Span::new(11, 12),
            kind: ExprKind::IntLit,
        };
        let field = StructLitField {
            span: Span::new(8, 12),
            name: field_name,
            colon_span: Span::new(9, 10),
            value,
        };

        let lit = Expr {
            span: Span::new(0, 13),
            kind: ExprKind::StructLit {
                ty,
                fields: vec![field],
            },
        };

        match &lit.kind {
            ExprKind::StructLit { ty, fields } => {
                assert_eq!(ty.segments.len(), 1);
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name.span, field_name.span);
            }
            other => panic!("expected StructLit, got {other:?}"),
        }
    }
}
