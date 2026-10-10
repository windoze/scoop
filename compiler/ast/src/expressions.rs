use crate::{
    AnonymousFunctionId, Block, CallableReferenceId, Ident, If, IntegerLiteralSyntax, LambdaId,
    Param, Pattern, PlaceExpr, Span, Try, TypeRef, When,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Return {
        value: Option<Box<Expr>>,
        span: Span,
    },
    Throw {
        value: Box<Expr>,
        span: Span,
    },
    /// A binding scope is a lexical block in the enclosing callable.
    ContextScope {
        value: Box<Expr>,
        body: Block,
        span: Span,
    },
    CharLiteral {
        value: char,
        span: Span,
    },
    StringLiteral {
        value: String,
        span: Span,
    },
    InterpolatedString {
        parts: Vec<StringPart>,
        span: Span,
    },
    IntLiteral(IntegerLiteralSyntax),
    FloatLiteral(crate::FloatLiteralSyntax),
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
        args: Vec<CallArgument>,
        span: Span,
    },
    Var(Ident),
    /// An explicitly applied type used as a member qualifier, never a value.
    TypeQualifier(TypeRef),
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
    /// `base.{ field: value, ... }`. The parser guarantees at least one
    /// direct field update; target/type resolution belongs to HIR.
    CopyUpdate {
        base: Box<Expr>,
        fields: NonEmptyVec<FieldUpdate>,
        span: Span,
    },
    Call(CallExpr),
    /// General function-value invocation. Bare `name(args)` remains
    /// `CallExpr` so HIR can apply the local-value shadowing rule before
    /// falling back to named overload resolution.
    Invoke {
        callee: Box<Expr>,
        type_args: Vec<CallTypeArgument>,
        args: Vec<CallArgument>,
        span: Span,
    },
    /// `lhs name rhs` or property-like `lhs rhs`; HIR validates the
    /// selected callable's typed infix role.
    InfixCall {
        lhs: Box<Expr>,
        target: InfixTarget,
        rhs: Box<Expr>,
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
    /// Prefix/postfix `++` / `--`. The parser accepts only a syntactic place,
    /// so HIR never has to recover lvalue shape from an arbitrary expression.
    Update {
        place: PlaceExpr,
        op: UpdateOp,
        notation: UpdateNotation,
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
        navigation: Navigation,
        /// Explicit call-site type arguments (`receiver.name<T>(...)`).
        type_args: Vec<CallTypeArgument>,
        args: Vec<CallArgument>,
        span: Span,
    },
    /// `super.name<TypeArgs>(arguments...)`. `super` itself never becomes an
    /// expression; HIR resolves this dedicated form against the direct base.
    SuperMethodCall {
        super_span: Span,
        name: Ident,
        type_args: Vec<CallTypeArgument>,
        args: Vec<CallArgument>,
        span: Span,
    },
    /// `super<I>.name` — a direct access to one direct interface default
    /// property. The qualifier is kept distinct from ordinary type arguments.
    QualifiedInterfaceSuperAccess {
        super_span: Span,
        qualifier: TypeRef,
        name: Ident,
        span: Span,
    },
    /// `super<I>.name<TypeArgs>(arguments...)` — a direct call to one
    /// interface default implementation.
    QualifiedInterfaceSuperMethodCall {
        super_span: Span,
        qualifier: TypeRef,
        name: Ident,
        type_args: Vec<CallTypeArgument>,
        args: Vec<CallArgument>,
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
        indices: NonEmptyVec<Expr>,
        span: Span,
    },
    /// Structured control expressions. Their branch blocks use the same
    /// syntax trees as the statement forms; HIR interprets the trailing
    /// expression of each normally completing block as its value.
    If(Box<If>),
    When(Box<When>),
    Try(Box<Try>),
}

/// The two source forms that share the infix precedence tier.
#[derive(Debug, Clone, PartialEq)]
pub enum InfixTarget {
    Named(Ident),
    Invoke,
}

impl Expr {
    /// Recover a type path only when it contains an explicit application.
    pub fn applied_qualifier_type(&self) -> Option<TypeRef> {
        match self {
            Self::TypeQualifier(ty) => Some(ty.clone()),
            Self::FieldAccess(access) if access.navigation == Navigation::Direct => {
                let FieldSelector::Name(name) = &access.selector else {
                    return None;
                };
                Some(access.receiver.applied_qualifier_type()?.with_member(
                    name.clone(),
                    Vec::new(),
                    access.span.end,
                ))
            }
            _ => None,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Expr::IntLiteral(literal) => literal.span,
            Expr::FloatLiteral(literal) => literal.span,
            Expr::CharLiteral { span, .. }
            | Expr::Return { span, .. }
            | Expr::Throw { span, .. }
            | Expr::ContextScope { span, .. }
            | Expr::StringLiteral { span, .. }
            | Expr::InterpolatedString { span, .. }
            | Expr::BoolLiteral { span, .. }
            | Expr::UnitLiteral { span }
            | Expr::TupleLiteral { span, .. }
            | Expr::StructInit { span, .. }
            | Expr::Lambda { span, .. }
            | Expr::AnonymousFunction { span, .. }
            | Expr::CallableReference { span, .. }
            | Expr::CopyUpdate { span, .. }
            | Expr::Invoke { span, .. }
            | Expr::InfixCall { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Update { span, .. }
            | Expr::NullAssert { span, .. }
            | Expr::Elvis { span, .. }
            | Expr::This { span }
            | Expr::MethodCall { span, .. }
            | Expr::SuperMethodCall { span, .. }
            | Expr::QualifiedInterfaceSuperAccess { span, .. }
            | Expr::QualifiedInterfaceSuperMethodCall { span, .. }
            | Expr::Is { span, .. }
            | Expr::Cast { span, .. }
            | Expr::ArrayLiteral { span, .. }
            | Expr::Index { span, .. } => *span,
            Expr::If(if_) => if_.span,
            Expr::When(when) => when.span,
            Expr::Try(try_) => try_.span,
            Expr::Var(ident) => ident.span,
            Expr::TypeQualifier(ty) => ty.span,
            Expr::FieldAccess(access) => access.span,
            Expr::Call(call) => call.span,
        }
    }
}

/// Ordered f-string fragments with positions in the original source file.
#[derive(Debug, Clone, PartialEq)]
pub enum StringPart {
    Text { value: String, span: Span },
    Expression { value: Box<Expr>, span: Span },
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
    pub navigation: Navigation,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldUpdate {
    pub field: Ident,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Navigation {
    Direct,
    Safe,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NonEmptyVec<T> {
    values: Vec<T>,
}

impl<T> NonEmptyVec<T> {
    pub fn new(first: T, rest: Vec<T>) -> Self {
        let mut values = Vec::with_capacity(1 + rest.len());
        values.push(first);
        values.extend(rest);
        Self { values }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub const fn is_empty(&self) -> bool {
        false
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.values.iter()
    }

    pub fn as_slice(&self) -> &[T] {
        &self.values
    }

    pub fn first(&self) -> &T {
        &self.values[0]
    }
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
    pub type_args: Vec<CallTypeArgument>,
    pub args: Vec<CallArgument>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CallTypeArgument {
    Explicit(TypeRef),
    Infer { span: Span },
}

impl CallTypeArgument {
    pub fn span(&self) -> Span {
        match self {
            Self::Explicit(ty) => ty.span,
            Self::Infer { span } => *span,
        }
    }
}

/// One explicit source argument. Name and spread are independent closed sums:
/// `name = *value` is represented without overloading expression syntax.
#[derive(Debug, Clone, PartialEq)]
pub struct CallArgument {
    pub name: CallArgumentName,
    pub spread: SpreadSyntax,
    pub expression: Expr,
    pub span: Span,
}

impl CallArgument {
    pub fn positional(expression: Expr) -> Self {
        let span = expression.span();
        Self {
            name: CallArgumentName::Positional,
            spread: SpreadSyntax::Plain,
            expression,
            span,
        }
    }

    pub const fn span(&self) -> Span {
        self.span
    }
}

impl std::ops::Deref for CallArgument {
    type Target = Expr;

    fn deref(&self) -> &Self::Target {
        &self.expression
    }
}

impl std::ops::DerefMut for CallArgument {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.expression
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallArgumentName {
    Positional,
    Named(Ident),
    /// The external lambda binds the candidate's final parameter.
    TrailingLambda,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpreadSyntax {
    Plain,
    Spread(Span),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    RangeTo,
    RangeUntil,
    Contains,
    NotContains,
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
    Plus,
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateOp {
    Increment,
    Decrement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateNotation {
    Prefix,
    Postfix,
}
