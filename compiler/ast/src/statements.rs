use crate::{Expr, FunctionDecl, Ident, NonEmptyVec, Span, TypeRef};

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StatementKind {
    Expr(Expr),
    /// A named function declared in a block. Its name is visible in its own
    /// body and from this statement to the end of the lexical scope.
    LocalFunction(FunctionDecl),
    /// `return` with an optional value (bare `return` in `Unit`
    /// functions).
    Return {
        value: Option<Expr>,
    },
    /// Pattern `when` used in statement position (spec 5). Value position
    /// uses `Expr::When` with the same payload.
    When(When),
    /// `try { } catch (e: T) { } finally { }` (spec 11.7).
    Try(Try),
    /// `throw expr` (spec 11.7).
    Throw(Expr),
    ValDecl(ValDecl),
    /// A reflection-free local delegated binding: `val/var name: T? by expr`.
    /// Unlike an ordinary local declaration it keeps the delegate expression
    /// syntactically distinct from a value initializer.
    LocalDelegatedProperty(LocalDelegatedPropertyDecl),
    Assign(Assign),
    If(If),
    While(While),
    Block(Block),
    /// A lexical safety override. Unlike annotations on declarations, this is
    /// a dedicated syntax node and cannot be mistaken for a function call.
    SafetyBlock {
        mode: SafetyMode,
        block: Block,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyMode {
    Safe,
    Unsafe,
}

/// `when (subject) { arms... }` with an optional trailing `else`.
#[derive(Debug, Clone, PartialEq)]
pub struct When {
    pub subject: Expr,
    pub arms: Vec<WhenArm>,
    pub else_body: Option<Block>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhenArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Block,
    pub span: Span,
}

/// A pattern (spec 4.6 / 5). Syntactically, enum variant patterns and
/// struct patterns share their shapes; HIR resolves which is which.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// A plain binding name.
    Binding(Ident),
    /// `_`
    Wildcard { span: Span },
    /// A literal matched by equality (`0`, `-1u`, `"x"`, `true`).
    Literal { expr: Box<Expr>, span: Span },
    /// `Path?(p1, p2)` — enum positional variant or struct positional;
    /// `rest` is the `..` marker.
    Positional {
        /// `E.V` as `[E, V]`, or bare `[V]`.
        path: Vec<Ident>,
        elements: Vec<Pattern>,
        rest: Option<Span>,
        span: Span,
    },
    /// `Path?{ f1, f2: subpattern, .. }` — enum named-field variant or
    /// struct field pattern. Shorthand fields are normalized to a binding
    /// subpattern with the same identifier.
    Named {
        path: Vec<Ident>,
        fields: Vec<FieldPattern>,
        rest: Option<Span>,
        span: Span,
    },
    /// `(p1, p2)` — tuple pattern.
    Tuple {
        elements: Vec<Pattern>,
        rest: Option<Span>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldPattern {
    pub field: Ident,
    pub subpattern: Box<Pattern>,
    pub span: Span,
}

/// Shared payload of statement- and expression-form `try`.
#[derive(Debug, Clone, PartialEq)]
pub struct Try {
    pub body: Block,
    pub catches: Vec<CatchClause>,
    pub finally_body: Option<Block>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatchClause {
    pub name: Ident,
    pub ty: TypeRef,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValDecl {
    pub mutable: bool,
    /// Binding target: a plain identifier is `Pattern::Binding`;
    /// destructuring uses tuple/struct patterns (spec 4.6).
    pub target: Pattern,
    /// Type annotation; genuinely optional (inferred when absent).
    pub ty: Option<TypeRef>,
    pub init: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalDelegatedPropertyDecl {
    pub mutable: bool,
    pub name: Ident,
    pub ty: Option<TypeRef>,
    pub expression: Expr,
    pub by_span: Span,
    pub span: Span,
}

/// Assignment. `Local` targets a `var`; `Index` targets a
/// `MutableArray` element (spec 10.5).
#[derive(Debug, Clone, PartialEq)]
pub struct Assign {
    pub target: PlaceExpr,
    pub op: AssignmentOp,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlaceExpr {
    Name(Ident),
    /// `receiver[index] = value`
    Index {
        receiver: Box<Expr>,
        indices: NonEmptyVec<Expr>,
        span: Span,
    },
    /// `receiver.field = value` (only `var` properties of classes).
    Field {
        receiver: Box<Expr>,
        name: Ident,
        span: Span,
    },
    /// `super<I>.property = value` — direct interface default setter access.
    QualifiedInterfaceSuperProperty {
        qualifier: TypeRef,
        name: Ident,
        span: Span,
    },
}

/// Compatibility name for clients that treat every assignment target as a
/// place. New code should prefer [`PlaceExpr`].
pub type AssignTarget = PlaceExpr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentOp {
    Assign,
    Compound(CompoundAssignOp),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompoundAssignOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

#[derive(Debug, Clone, PartialEq)]
pub struct If {
    pub cond: Expr,
    pub then_block: Block,
    /// The else branch genuinely may not exist.
    pub else_block: Option<Block>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct While {
    pub cond: Expr,
    pub body: Block,
    pub span: Span,
}
