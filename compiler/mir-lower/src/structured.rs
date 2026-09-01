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
    GlobalAssign {
        global: mir::GlobalId,
        value: Expr,
    },
    ArraySet {
        array_type: mir::ClassId,
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

/// Call-preserving structured expression used before CFG normalization.
/// It is fully typed at creation so CFG construction never recovers a result
/// type from an enclosing statement or expected context.
#[derive(Debug, Clone)]
pub(crate) struct Expr {
    pub(crate) ty: mir::Type,
    pub(crate) kind: ExprKind,
}

impl Expr {
    pub(crate) fn new(ty: mir::Type, kind: ExprKind) -> Self {
        Self { ty, kind }
    }

    pub(crate) fn local(local: mir::LocalId, ty: mir::Type) -> Self {
        Self::new(ty, ExprKind::Local(local))
    }

    pub(crate) fn int(value: i64) -> Self {
        Self::new(mir::Type::Int, ExprKind::IntLiteral(value))
    }

    pub(crate) fn bool(value: bool) -> Self {
        Self::new(mir::Type::Boolean, ExprKind::BoolLiteral(value))
    }

    pub(crate) fn unit() -> Self {
        Self::new(mir::Type::Unit, ExprKind::UnitLiteral)
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ExprKind {
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
    ClosureCapture {
        closure: Box<Expr>,
        class: mir::ClosureClassId,
        index: u32,
    },
    Local(mir::LocalId),
    GlobalRead(mir::GlobalId),
    PtrFromUInt {
        operand: Box<Expr>,
        pointee: Box<mir::Type>,
    },
    PtrToUInt(Box<Expr>),
    PtrCast {
        operand: Box<Expr>,
        pointee: Box<mir::Type>,
    },
    PtrLoad {
        pointer: Box<Expr>,
        pointee: Box<mir::Type>,
        offset: Option<Box<Expr>>,
    },
    PtrStore {
        pointer: Box<Expr>,
        pointee: Box<mir::Type>,
        offset: Option<Box<Expr>>,
        value: Box<Expr>,
    },
    PtrOffset {
        pointer: Box<Expr>,
        pointee: Box<mir::Type>,
        offset: Box<Expr>,
        subtract: bool,
    },
    AddressOf {
        local: mir::LocalId,
        pointee: Box<mir::Type>,
    },
    GlobalAddress {
        global: mir::GlobalId,
        pointee: Box<mir::Type>,
    },
    SizeOf(Box<mir::Type>),
    AlignOf(Box<mir::Type>),
    FunPtrNull(mir::FunctionTypeId),
    FunctionAddress {
        callback: mir::CallbackBridgeId,
    },
    ForeignCallbackRegister {
        bridge: mir::ForeignCallbackBridgeId,
        closure: Box<Expr>,
    },
    ForeignCallbackOperation {
        operation: mir::ForeignCallbackOperation,
        callback: Box<Expr>,
        result_ty: Box<mir::Type>,
    },
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
    ArrayLiteral {
        array_type: mir::ClassId,
        elements: Vec<Expr>,
    },
    ArrayGet {
        array_type: mir::ClassId,
        array: Box<Expr>,
        index: Box<Expr>,
    },
    ArrayLen {
        array_type: mir::ClassId,
        operand: Box<Expr>,
    },
    ArrayClone {
        source_type: mir::ClassId,
        target_type: mir::ClassId,
        operand: Box<Expr>,
    },
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
