use crate::{
    AnonymousFunctionId, Block, CallableReferenceId, Ident, If, LambdaId, Param, Pattern, Span,
    Try, TypeRef, When,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    StringLiteral {
        value: String,
        span: Span,
    },
    IntLiteral {
        value: i64,
        span: Span,
    },
    BoolLiteral {
        value: bool,
        span: Span,
    },
    /// The `Unit` literal, written `()` or `Unit`.
    UnitLiteral {
        span: Span,
    },
    TupleLiteral {
        elements: Vec<Expr>,
        span: Span,
    },
    /// `Name(arg, ...)` where `Name` resolves to a struct.
    StructInit {
        name: Ident,
        args: Vec<Expr>,
        span: Span,
    },
    Var(Ident),
    /// `{ p: T -> body }` / `{ body }`. `parameters = None` means the
    /// parameter list was omitted; this is distinct from the explicit
    /// zero-parameter form `{ -> body }` for expected-type `it` inference.
    Lambda {
        id: LambdaId,
        is_suspend: bool,
        parameters: Option<Vec<LambdaParam>>,
        body: Block,
        span: Span,
    },
    /// `fun(p: T): R { ... }` / `suspend fun(...) { ... }`.
    AnonymousFunction {
        id: AnonymousFunctionId,
        is_suspend: bool,
        params: Vec<Param>,
        return_ty: Option<TypeRef>,
        body: Block,
        span: Span,
    },
    /// `::name` or `receiver::name`. Resolution is intentionally deferred
    /// to HIR, where overloads and receiver dispatch are known.
    CallableReference {
        id: CallableReferenceId,
        receiver: Option<Box<Expr>>,
        name: Ident,
        span: Span,
    },
    FieldAccess(FieldAccess),
    Call(CallExpr),
    /// General function-value invocation. Bare `name(args)` remains
    /// `CallExpr` so HIR can apply the local-value shadowing rule before
    /// falling back to named overload resolution.
    Invoke {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    Unary {
        op: UnOp,
        operand: Box<Expr>,
        span: Span,
    },
    /// `expr!!` — unwrap an `Option`, trapping on `None` (M3; real
    /// exception in M8).
    NullAssert {
        operand: Box<Expr>,
        span: Span,
    },
    /// `lhs ?: rhs` (spec 7.3).
    Elvis {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    /// `this` (inside member functions).
    This {
        span: Span,
    },
    /// `receiver.name(args)` — method call (M6).
    MethodCall {
        receiver: Box<Expr>,
        name: Ident,
        /// Explicit call-site type arguments (`receiver.name<T>(...)`).
        type_args: Vec<TypeRef>,
        args: Vec<Expr>,
        span: Span,
    },
    /// `expr is T` / `expr !is T`.
    Is {
        operand: Box<Expr>,
        ty: TypeRef,
        negated: bool,
        span: Span,
    },
    /// `expr as T` / `expr as? T` (`optional` = `as?`, spec 4.4.4).
    Cast {
        operand: Box<Expr>,
        ty: TypeRef,
        optional: bool,
        span: Span,
    },
    /// `[e1, e2, ...]` — array literal (spec 10.2).
    ArrayLiteral {
        elements: Vec<Expr>,
        span: Span,
    },
    /// `receiver[index]` — subscript read (spec 10.5).
    Index {
        receiver: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    /// Structured control expressions. Their branch blocks use the same
    /// syntax trees as the statement forms; HIR interprets the trailing
    /// expression of each normally completing block as its value.
    If(Box<If>),
    When(Box<When>),
    Try(Box<Try>),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::StringLiteral { span, .. }
            | Expr::IntLiteral { span, .. }
            | Expr::BoolLiteral { span, .. }
            | Expr::UnitLiteral { span }
            | Expr::TupleLiteral { span, .. }
            | Expr::StructInit { span, .. }
            | Expr::Lambda { span, .. }
            | Expr::AnonymousFunction { span, .. }
            | Expr::CallableReference { span, .. }
            | Expr::Invoke { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Unary { span, .. }
            | Expr::NullAssert { span, .. }
            | Expr::Elvis { span, .. }
            | Expr::This { span }
            | Expr::MethodCall { span, .. }
            | Expr::Is { span, .. }
            | Expr::Cast { span, .. }
            | Expr::ArrayLiteral { span, .. }
            | Expr::Index { span, .. } => *span,
            Expr::If(if_) => if_.span,
            Expr::When(when) => when.span,
            Expr::Try(try_) => try_.span,
            Expr::Var(ident) => ident.span,
            Expr::FieldAccess(access) => access.span,
            Expr::Call(call) => call.span,
        }
    }
}

/// One lambda parameter. Patterns are retained for the later capture/type
/// pass; the type is optional only when an expected function type supplies it.
#[derive(Debug, Clone, PartialEq)]
pub struct LambdaParam {
    pub target: Pattern,
    pub ty: Option<TypeRef>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldAccess {
    pub receiver: Box<Expr>,
    pub selector: FieldSelector,
    /// `?.` (spec 7.3) instead of `.`.
    pub safe: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FieldSelector {
    /// `.<name>` on a struct.
    Name(Ident),
    /// `._<n>` on a tuple (1-based).
    Index(u32, Span),
}

/// M1: the callee is always a bare identifier (direct top-level call).
#[derive(Debug, Clone, PartialEq)]
pub struct CallExpr {
    pub callee: Ident,
    /// Explicit call-site type arguments (`callee<T>(...)`).
    pub type_args: Vec<TypeRef>,
    pub args: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    /// `===` / `!==` — reference identity (spec 4.4.2; value types
    /// are a compile error).
    RefEq,
    RefNe,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}
