//! Private construction IR used before MIR CFG and call normalization.

use la_arena::Arena;
use scoop_ast::Span;
use scoop_mir as mir;

#[derive(Debug)]
pub(crate) struct Body {
    pub(crate) locals: Arena<mir::Local>,
    pub(crate) statements: Vec<Statement>,
}

#[derive(Debug)]
pub(crate) struct Statement {
    pub(crate) kind: StatementKind,
    pub(crate) span: Span,
}

#[derive(Debug)]
pub(crate) struct Try {
    pub(crate) body: Vec<Statement>,
    pub(crate) catches: Vec<CatchClause>,
    pub(crate) finally_body: Option<Vec<Statement>>,
}

#[derive(Debug)]
pub(crate) struct CatchClause {
    pub(crate) local: mir::LocalId,
    pub(crate) ty: Box<mir::Type>,
    pub(crate) body: Vec<Statement>,
    pub(crate) span: Span,
}

#[derive(Debug)]
pub(crate) enum StatementKind {
    Expr(Expr),
    Return {
        value: Option<Expr>,
    },
    ValDecl {
        local: mir::LocalId,
        init: Expr,
    },
    Assign {
        local: mir::LocalId,
        value: Expr,
    },
    ArraySet {
        array: Expr,
        index: Expr,
        value: Expr,
    },
    FieldSet {
        object: Expr,
        index: u32,
        value: Expr,
    },
    Try(Try),
    Throw(Expr),
    If {
        cond: Expr,
        then_body: Vec<Statement>,
        else_body: Option<Vec<Statement>>,
    },
    While {
        cond: Expr,
        body: Vec<Statement>,
    },
}

#[derive(Debug, Clone)]
pub(crate) enum Expr {
    StringConst(mir::StringConstId),
    IntLiteral(i64),
    BoolLiteral(bool),
    UnitLiteral,
    TupleLiteral(Vec<Expr>),
    StructInit {
        struct_id: mir::StructId,
        args: Vec<Expr>,
    },
    ClassInit {
        class_id: mir::ClassId,
        args: Vec<Expr>,
    },
    ClosureAlloc {
        class: mir::ClosureClassId,
        captures: Vec<Expr>,
    },
    Local(mir::LocalId),
    Retype {
        operand: Box<Expr>,
        ty: Box<mir::Type>,
    },
    FieldAccess {
        receiver: Box<Expr>,
        index: u32,
    },
    Call(Call),
    Box(Box<Expr>),
    Unbox(Box<Expr>),
    IsInstance {
        operand: Box<Expr>,
        check_ty: Box<mir::Type>,
    },
    ArrayLiteral(Vec<Expr>),
    ArrayGet {
        array: Box<Expr>,
        index: Box<Expr>,
    },
    ArrayLen(Box<Expr>),
    ArrayClone(Box<Expr>),
    Binary {
        op: mir::BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    ShortCircuit {
        op: LogicOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Unary {
        op: mir::UnOp,
        operand: Box<Expr>,
    },
    VariantConstruct {
        ty: mir::Type,
        variant: u32,
        fields: Vec<Expr>,
    },
    EnumTag(Box<Expr>),
    EnumField {
        operand: Box<Expr>,
        variant: u32,
        index: u32,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum LogicOp {
    And,
    Or,
}

#[derive(Debug, Clone)]
pub(crate) struct Call {
    pub(crate) target: mir::CallTarget,
    pub(crate) args: Vec<Expr>,
    pub(crate) return_ty: mir::Type,
}
