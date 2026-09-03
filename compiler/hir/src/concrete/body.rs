use super::*;

#[derive(Debug, Clone)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StatementKind {
    Expr(Expr),
    LocalFunction(LocalFunctionId),
    Return {
        value: Option<Expr>,
    },
    ValDecl {
        pattern: Pattern,
        init: Expr,
    },
    Assign {
        target: AssignTarget,
        value: Expr,
    },
    If {
        cond: Expr,
        then_body: Vec<Statement>,
        else_body: Option<Vec<Statement>>,
    },
    While {
        cond: Expr,
        body: Vec<Statement>,
    },
    When(When),
    Try(Try),
    Throw(Expr),
}

#[derive(Debug, Clone)]
pub struct Try {
    pub body: Vec<Statement>,
    pub catches: Vec<CatchClause>,
    pub finally_body: Option<Vec<Statement>>,
}

#[derive(Debug, Clone)]
pub struct CatchClause {
    pub local: LocalId,
    pub ty: TypeId,
    pub body: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum AssignTarget {
    Local(LocalId),
    Global(GlobalId),
    Index {
        array: Expr,
        index: Expr,
    },
    Field {
        receiver: Box<Expr>,
        field: FieldRef,
    },
}

#[derive(Debug, Clone)]
pub struct When {
    pub subject: Expr,
    pub arms: Vec<WhenArm>,
    pub else_body: Option<Vec<Statement>>,
}

#[derive(Debug, Clone)]
pub struct WhenArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Binding {
        local: LocalId,
    },
    Wildcard,
    Literal {
        value: Expr,
        /// Exact ordinary operator target selected by Export HIR.
        equals: Callable,
        /// Static subject type used to select dispatch. This is explicit so
        /// MIR never reconstructs it from its recursive pattern context.
        subject_ty: TypeId,
    },
    Variant {
        enum_id: EnumId,
        variant: VariantId,
        fields: Vec<(u32, Pattern)>,
    },
    Tuple(Vec<Pattern>),
    Struct {
        struct_id: StructId,
        fields: Vec<(u32, Pattern)>,
    },
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: TypeId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    StringLiteral(String),
    IntLiteral(i64),
    BoolLiteral(bool),
    UnitLiteral,
    TupleLiteral(Vec<Expr>),
    StructInit {
        struct_id: StructId,
        args: Vec<Expr>,
    },
    ClassInit {
        constructor: ClassConstructorId,
        args: Vec<Expr>,
    },
    ConstructorParam(ConstructorParamId),
    VariantConstruct {
        enum_id: EnumId,
        variant: VariantId,
        args: Vec<Expr>,
    },
    Local(LocalId),
    GlobalRead(GlobalId),
    Capture(BindingId),
    Lambda(LambdaId),
    AnonymousFunction(AnonymousFunctionId),
    CallableReference(CallableReferenceId),
    FunctionCoercion {
        source: Box<Expr>,
        coercion: FunctionCoercionId,
        target_type: FunctionTypeId,
    },
    PtrFromUInt(Box<Expr>),
    PtrToUInt(Box<Expr>),
    PtrCast(Box<Expr>),
    PtrLoad {
        pointer: Box<Expr>,
        offset: Option<Box<Expr>>,
    },
    PtrStore {
        pointer: Box<Expr>,
        offset: Option<Box<Expr>>,
        value: Box<Expr>,
    },
    PtrOffset {
        pointer: Box<Expr>,
        offset: Box<Expr>,
        subtract: bool,
    },
    AddressOf(Place),
    SizeOf(TypeId),
    AlignOf(TypeId),
    FunPtrNull,
    FunctionAddress(FunctionId),
    ForeignCallbackRegister {
        registration: ForeignCallbackRegistrationId,
        closure: Box<Expr>,
    },
    ForeignCallbackOperation {
        operation: ForeignCallbackOperation,
        callback: Box<Expr>,
    },
    FieldAccess {
        receiver: Box<Expr>,
        field: FieldRef,
    },
    MethodCall {
        receiver: Box<Expr>,
        callee: Callable,
        args: Vec<Expr>,
    },
    Box(Box<Expr>),
    Unbox(Box<Expr>),
    IsInstance {
        operand: Box<Expr>,
        check_ty: TypeId,
    },
    Cast {
        operand: Box<Expr>,
        optional: bool,
    },
    ArrayLiteral(Vec<Expr>),
    ArrayAssembly(ArrayAssembly),
    Index {
        receiver: Box<Expr>,
        index: Box<Expr>,
    },
    ArrayLen(Box<Expr>),
    ArrayClone(Box<Expr>),
    Call {
        callee: Callable,
        args: Vec<Expr>,
    },
    LocalFunctionCall {
        local_function: LocalFunctionId,
        callee: Callable,
        captures: Vec<Expr>,
        args: Vec<Expr>,
    },
    CallableCall {
        callee: Box<Expr>,
        function_type: FunctionTypeId,
        args: Vec<Expr>,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Unary {
        op: UnOp,
        operand: Box<Expr>,
    },
    SomeWrap(Box<Expr>),
    NoneLiteral,
    IsSome(Box<Expr>),
    Unwrap {
        operand: Box<Expr>,
        trap_on_none: bool,
    },
}

#[derive(Debug, Clone)]
pub struct ArrayAssembly {
    pub element_type: TypeId,
    pub parts: Vec<ArrayAssemblyPart>,
    pub result_type: ClassId,
}

#[derive(Debug, Clone)]
pub enum ArrayAssemblyPart {
    Element(Expr),
    CopyArray(Expr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Local(LocalId),
    Global(GlobalId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldRef {
    StructField { struct_id: StructId, index: u32 },
    TupleIndex(u32),
    ClassField { class_id: ClassId, index: u32 },
}
