//! 表达式（§8）与语句 / 块（§7）。
//!
//! 两个值得注意的建模决策：
//!
//! - **赋值只在语句位**（§7 `exprStmt`）：[`StmtKind::Assign`] 的 LHS 用
//!   [`AssignTarget`] 三态（Ident / Member / Index）建模，而不是裸 `Expr`，
//!   这样 `a[i, j] = v`（IndexAssign）与 `?.` 链等非法 LHS 在类型层面就被区分；
//!   parser 负责把非法 LHS 报为 `assignment_expression_not_allowed`。
//! - **`try/catch/finally` 没有 AST 节点**：parser 按 §8.6 直接脱糖为
//!   [`ExprKind::Handle`] over `scoop.core.Raise.raise`（每个 catch 一个
//!   non-resuming arm，合成标识符取 `catch` 关键字 span）。这与 legacy
//!   parser 的设计一致，AST 不再保留 `try` 的表面语法。

use scoop2_base::{NodeId, Span};

use super::decl::ValDecl;
use super::pattern::Pattern;
use super::types::{TypeArg, TypeRef};
use super::{AnnotationUse, CharLit, FloatLit, Ident, IntLit, StringLit, TypePath};

/// 块：`{ stmt* }`（§7）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Block {
    pub id: NodeId,
    pub span: Span,
    pub stmts: Vec<Stmt>,
    /// 最后一条语句是否带 `;`（影响 block 值类型：带 `;` 的尾部 expr → Unit）。
    pub last_trailing_semi: bool,
}

/// 语句（§7）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Stmt {
    pub id: NodeId,
    pub span: Span,
    pub kind: StmtKind,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum StmtKind {
    /// 空语句（孤立的 `;`）。
    Empty,
    /// 表达式语句。
    Expr(Expr),
    /// 赋值语句（仅语句位，§7）：`target = value`。
    ///
    /// LHS 只允许三种形态，见 [`AssignTargetKind`]；没有复合赋值（`+=` 不 lex）。
    Assign { target: AssignTarget, value: Expr },
    /// 局部 `val` / `var`（含解构模式，§7 `localValDecl`）。
    LocalVal(Box<ValDecl>),
    /// `return expr?`。
    Return { value: Option<Expr> },
    /// `while (cond) { ... }`（body 必须是块）。
    While { cond: Expr, body: Block },
    /// `for (x in xs) { ... }`（binder 是单个标识符，不支持解构，§7/§11）。
    For {
        binder: Ident,
        iter: Expr,
        body: Block,
    },
    /// `break`（无 label，§11）。
    Break,
    /// `continue`（无 label，§11）。
    Continue,
}

/// 赋值目标（§7：合法 LHS 只有 `x`、`a.b`、`a[i, j]` 三种）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AssignTarget {
    pub id: NodeId,
    pub span: Span,
    pub kind: AssignTargetKind,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum AssignTargetKind {
    /// `x = v`。
    Ident(Ident),
    /// `a.b = v`（含元组段形态 `t.0 = v`，其合法性由 typecheck 判定）。
    Member {
        receiver: Box<Expr>,
        member: MemberName,
    },
    /// `a[i, j] = v`（IndexAssign；operator set 解析是 typecheck 的事）。
    Index {
        receiver: Box<Expr>,
        indices: Vec<Expr>,
    },
}

/// 成员段名：`memberSeg ::= IDENT | INT_LIT`（§8.4）。
///
/// 同时用于成员访问（`a.b`、`t.0`）与 `with` 更新的字段路径段。
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum MemberName {
    Named(Ident),
    /// 元组索引段（`t.0` 的 `0`；值已由 lexer 校验）。
    TupleIndex {
        value: u128,
        span: Span,
    },
}

/// 表达式（§8）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Expr {
    pub id: NodeId,
    pub span: Span,
    pub kind: ExprKind,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ExprKind {
    /// 标识符引用（`true` / `false` 也是普通 Ident，由 typecheck 解析）。
    Ident(Ident),
    IntLit(IntLit),
    FloatLit(FloatLit),
    CharLit(CharLit),
    StringLit(StringLit),
    /// 插值字符串 `f"..${x}.."`（§8.2）；`raw` 表示 `f"""..."""`。
    InterpolatedString {
        raw: bool,
        parts: Vec<StringPart>,
    },
    /// `()` Unit 字面量。
    UnitLit,
    /// 元组字面量 `(a, b)`（`(a,)` 是 1 元组）。
    TupleLit(Vec<Expr>),
    /// 数组字面量 `[a, b]`。
    ArrayLit(Vec<Expr>),
    /// struct 字面量 `Point { x: 1 }`（§8.2：名单段、无类型实参、仅 `name: expr`）。
    StructLit {
        name: Ident,
        fields: Vec<StructLitField>,
    },
    /// 块表达式。
    ///
    /// 注意：表达式位置的裸 `{ ... }` 一律解析为 lambda（§8.2）；
    /// 本变体只作为 `if` / `when` / `handle` 等 control body 的内部形态出现。
    Block(Block),
    /// `do { ... }`（§8.2 `doBlock`）。
    DoBlock(Block),
    /// `@Unsafe do { ... }`（§8.3）。
    UnsafeBlock(Block),
    /// `@Safe do { ... }`（§8.3；`@Safe { ... }` 闭包见 [`LambdaExpr::is_safe`]）。
    SafeBlock(Block),
    Lambda(LambdaExpr),
    /// `if (cond) then else?`（control body 为块时内层是 [`ExprKind::Block`]）。
    If {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Option<Box<Expr>>,
    },
    /// `when (subject) { arms }`（subject 必有，§8.5/§11）。
    When {
        subject: Box<Expr>,
        arms: Vec<WhenArm>,
    },
    /// `handle { body } on { arms } finally?`（§8.6）。
    ///
    /// **这也是 `try/catch/finally` 的脱糖形态**：`try` 没有自己的节点；
    /// parser 为每个 `catch (e: T) { .. }` 生成一个对合成路径
    /// `scoop.core.Raise.raise` 的 non-resuming arm（binder 为 `e: T`，
    /// 合成标识符取 `catch` 关键字 span），`finally` 原样保留。
    Handle {
        body: Block,
        arms: Vec<HandleArm>,
        finally: Option<Block>,
    },
    /// `receiver.member`（含元组段 `t.0`）。
    MemberAccess {
        receiver: Box<Expr>,
        member: MemberName,
    },
    /// `receiver?.member`。
    SafeMemberAccess {
        receiver: Box<Expr>,
        member: MemberName,
    },
    /// splice 字段访问 `receiver.[field]`（§8.4，spec §6.4）。
    SpliceField {
        receiver: Box<Expr>,
        field: Box<Expr>,
    },
    /// 下标读取 `a[i, j]`（§8.4 `indexPostfix`，多下标；
    /// operator get 解析是 typecheck 的事）。
    Index {
        receiver: Box<Expr>,
        indices: Vec<Expr>,
    },
    /// 非空断言 `expr!!`。
    NotNullAssert {
        expr: Box<Expr>,
    },
    /// 显式类型应用 `expr<T, eff E>`（§8.4）。
    TypeApply {
        callee: Box<Expr>,
        args: Vec<TypeArg>,
    },
    /// 调用 `callee(args...)`。
    ///
    /// Kotlin 风格 trailing lambda 已由 parser 折叠进 `args`
    /// （`combine(1) { .. } { .. }` 是一个带两个 lambda 实参的 Call，§8.4）。
    Call {
        callee: Box<Expr>,
        args: Vec<CallArg>,
    },
    /// class 字面量 `T::class`（receiver 必须是类型路径，§8.4）。
    ClassLit {
        path: TypePath,
    },
    /// 前缀一元运算：`!x` / `-x` / `~x`（没有一元 `+`，§8.3）。
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    /// 二元运算（§8.1 完整运算符表；`?:` 见 [`BinaryOp::Elvis`]）。
    Binary {
        lhs: Box<Expr>,
        op: BinaryOp,
        rhs: Box<Expr>,
    },
    /// 上下文中缀调用：`a until b` / `a downTo b` / `x step n`（§8.1.1）。
    ///
    /// 等价于方法调用糖 `a.until(b)`；`name` 的文本是
    /// `until` / `downTo` / `step` 之一，解析规则与 operator 重载一致。
    InfixCall {
        receiver: Box<Expr>,
        name: Ident,
        arg: Box<Expr>,
    },
    /// 运行期类型判断：`expr is T` / `expr !is T`。
    TypeCheck {
        expr: Box<Expr>,
        op: TypeCheckOp,
        ty: TypeRef,
    },
    /// 显式类型转换：`expr as T` / `expr as? T`。
    Cast {
        expr: Box<Expr>,
        op: CastOp,
        ty: TypeRef,
    },
    /// 值类型 with 更新：`expr with { path: v, ... }`（§8.4，spec §2.6）。
    WithUpdate {
        base: Box<Expr>,
        updates: Vec<WithUpdateField>,
    },
    /// 注解前缀表达式：`@Ann expr`（§8.3 `annotationUse+ prefixExpr`）。
    Annotated {
        annotations: Vec<AnnotationUse>,
        expr: Box<Expr>,
    },
}

/// 插值字符串片段（§8.2：只有 `${ expr }` 是 hole，`$name` 不支持）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum StringPart {
    /// 已解码的文本片段。
    Text(String),
    /// `${ expr }` hole。
    Expr(Expr),
}

/// struct 字面量的字段初始化项：`name: expr`。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StructLitField {
    pub id: NodeId,
    pub span: Span,
    pub name: Ident,
    pub value: Expr,
}

/// lambda 表达式（§8.2）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LambdaExpr {
    /// 是否为 `@Safe` 闭包（`@Safe { ... }`）。
    pub is_safe: bool,
    pub params: Vec<LambdaParam>,
    /// 主体：按 §8.2 解包规则——若主体是**无尾 `;`** 的单条表达式语句，
    /// 则解包为该表达式（lambda 的值）；否则是块（值为 Unit）。
    pub body: LambdaBody,
}

/// lambda 参数：`name` 或 `name: Type`。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LambdaParam {
    pub id: NodeId,
    pub span: Span,
    pub name: Ident,
    pub ty: Option<TypeRef>,
}

/// lambda 主体（§8.2 解包规则的两种结果）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LambdaBody {
    Block(Block),
    Expr(Box<Expr>),
}

/// 调用实参：`(IDENT '=')? ('*' expr | expr)`（§8.4 `callArg`）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CallArg {
    pub id: NodeId,
    pub span: Span,
    /// 命名实参 `name = ...`（仅参数列表内合法）。
    pub name: Option<Ident>,
    /// spread：`*expr` 或命名 spread `name = *expr`。
    pub is_spread: bool,
    pub value: Expr,
}

/// `when` 分支：`pat (if guard)? -> body`（§8.5）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WhenArm {
    pub id: NodeId,
    pub span: Span,
    pub pat: Pattern,
    pub guard: Option<Expr>,
    /// body 为块时内层是 [`ExprKind::Block`]。
    pub body: Expr,
}

/// `handle` 的 handler arm（§8.6）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HandleArm {
    pub id: NodeId,
    pub span: Span,
    pub op: HandleOp,
    /// 逃逸 continuation binder（`, k ->` 形式）；`None` 为 non-resuming arm。
    pub escape_continuation: Option<Ident>,
    /// `->` 箭头的 span（用于不可达 arm 诊断定位）。
    pub arrow_span: Span,
    /// body 为块时内层是 [`ExprKind::Block`]。
    pub body: Expr,
}

/// handler arm 头部的 effect operation（§8.6）：
/// `Path<Args>.op<Args>(binders...)`。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HandleOp {
    pub id: NodeId,
    pub span: Span,
    /// effect 路径（至少 `X.op` 中的 `X`，裸 `op(...)` 会被 parser 拒绝）。
    pub effect_path: TypePath,
    /// effect 路径上的类型实参（仅 `Path<Args>.op(...)` 形式）。
    pub effect_args: Vec<TypeArg>,
    pub op: Ident,
    /// op 自己的类型实参（`Query.ask<Int>(...)`）。
    pub op_type_args: Vec<TypeArg>,
    pub binders: Vec<HandleBinder>,
}

/// handler arm 的一个参数绑定：`name` 或 `name: Type`。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HandleBinder {
    pub id: NodeId,
    pub span: Span,
    pub name: Ident,
    pub ty: Option<TypeRef>,
}

/// `with` 更新的一项：`path: value`。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WithUpdateField {
    pub id: NodeId,
    pub span: Span,
    pub path: FieldPath,
    pub value: Expr,
}

/// `with` 更新的字段路径：`a.b`、`0.1`（§8.4 `fieldPath`）。
///
/// `with { 0.1: v }` 中的 float token 已由 parser 拆成两个整数段
/// （[`MemberName::TupleIndex`]），AST 不保留 float 形态。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FieldPath {
    pub span: Span,
    pub segments: Vec<MemberName>,
}

/// 二元运算符（§8.1 完整表）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BinaryOp {
    Mul,
    Div,
    Rem,
    Add,
    Sub,
    Shl,
    Shr,
    /// 闭区间 `..`（与比较运算同级，§8.1 normative）。
    Range,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    BitAnd,
    BitXor,
    BitOr,
    LogAnd,
    LogOr,
    /// Elvis `?:`（唯一的右结合二元运算）。
    Elvis,
}

/// 前缀一元运算符（§8.3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UnaryOp {
    /// `!x`
    Not,
    /// `-x`
    Neg,
    /// `~x`
    BitNot,
}

/// 运行期类型判断运算符（§8.1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TypeCheckOp {
    /// `is`
    Is,
    /// `!is`
    NotIs,
}

/// 显式类型转换运算符（§8.1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CastOp {
    /// `as`
    As,
    /// `as?`
    AsSafe,
}
