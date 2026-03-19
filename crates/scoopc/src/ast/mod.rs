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
}

#[derive(Debug, Clone)]
pub enum Item {
    Fun(FunDecl),
    Type(TypeDecl),
    Val(ValDecl),
}

/// 声明处的类型参数（type parameter）。
///
/// 当前阶段（T0218）仅支持无约束的 `T` / `U`：
/// - 不支持 `in/out` 变型
/// - 不支持上界/下界（`:` / `where`）
#[derive(Debug, Clone)]
pub struct TypeParam {
    pub span: Span,
    pub name: Ident,
}

#[derive(Debug, Clone)]
pub struct TypeDecl {
    pub span: Span,
    pub kind: TypeKind,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    /// 类型体（`{ ... }`）。
    ///
    /// 当前阶段：
    /// - parser 仍可能仅保证括号平衡与 span 正确
    /// - 成员列表的解析会在后续任务中逐步补齐
    pub body: Option<TypeBody>,
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

/// 类型体中的成员声明（最小骨架）。
#[derive(Debug, Clone)]
pub enum TypeMember {
    Val(ValDecl),
    Fun(FunDecl),
    Type(TypeDecl),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    Class,
    Interface,
    Struct,
    Enum,
    Effect,
}

#[derive(Debug, Clone)]
pub struct FunDecl {
    pub span: Span,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub params_span: Span,
    pub params: Vec<Param>,
    pub return_ty: Option<TypeRef>,
    pub body: FunBody,
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

#[derive(Debug, Clone)]
pub enum ExprKind {
    /// 解析失败或尚未实现时的占位节点（保持 span 以便诊断/回归）。
    Missing,
    Ident(Ident),
    IntLit,
    StringLit,
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
        member: Ident,
    },
    /// safe-call 成员访问表达式：`receiver?.member`（postfix）（Appendix B.3.1）。
    ///
    /// 说明：仅做语法建模；desugar/运行期语义留到后续阶段（typecheck/lowering）决定。
    SafeMemberAccess {
        receiver: Box<Expr>,
        op_span: Span,
        member: Ident,
    },
    /// 调用表达式：`callee(args...)`（postfix）。
    ///
    /// 当前阶段（T0209）仅支持位置参数与逗号分隔参数列表；命名参数/trailing lambda 等语法后续再补齐。
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    /// 非空断言：`expr!!`（postfix）。
    ///
    /// 说明：仅做语法建模；运行期异常语义留到后续阶段（typecheck/effect/codegen）决定。
    NotNullAssert {
        expr: Box<Expr>,
        op_span: Span,
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
    pub arrow_span: Span,
    pub body: Expr,
}

/// `when` 分支的模式（早期最小子集）。
#[derive(Debug, Clone)]
pub enum WhenPat {
    Else { span: Span },
    Is { is_span: Span, ty: TypeRef },
    IntLit { span: Span },
    StringLit { span: Span },
}

impl WhenPat {
    pub fn span(&self) -> Span {
        match self {
            WhenPat::Else { span } => *span,
            WhenPat::Is { is_span, ty } => Span::new(is_span.start, ty.span().end),
            WhenPat::IntLit { span } => *span,
            WhenPat::StringLit { span } => *span,
        }
    }
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
    Break { break_span: Span },
    /// `continue`（PLAN §4.6）。
    Continue { continue_span: Span },
    Missing,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: Ident,
    pub ty: Option<TypeRef>,
}

#[derive(Debug, Clone)]
pub struct ValDecl {
    pub span: Span,
    pub kind: ValKind,
    pub name: Ident,
    pub ty: Option<TypeRef>,
    /// 初始化表达式（当前阶段可能为 `ExprKind::Missing`，后续任务会逐步补齐解析）。
    pub init: Option<Expr>,
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
