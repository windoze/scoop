use super::*;

#[derive(Debug, Clone)]
pub struct Body {
    pub locals: Arena<Local>,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub struct Local {
    pub binding: BindingId,
    pub name: String,
    pub ty: TypeId,
    pub mutable: bool,
}

#[derive(Debug, Clone)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StatementKind {
    Expr(Expr),
    /// Compile-time declaration marker. The lifted body lives in
    /// `Module::local_functions`; executing this statement has no effect.
    LocalFunction(LocalFunctionId),
    Return {
        /// Absent in `Unit` functions (bare `return`).
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
        /// The else branch genuinely may not exist.
        else_body: Option<Vec<Statement>>,
    },
    While {
        condition_setup: Vec<Statement>,
        cond: Expr,
        body: Vec<Statement>,
    },
    /// Pattern `when` (spec 5); checked for exhaustiveness at HIR.
    When(When),
    /// `try { } catch ... finally { }`; catches are ordered.
    Try(Try),
    /// `throw expr`; the operand's type is a subtype of `Throwable`.
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
    /// `array[index] = value` (only `MutableArray`, checked at HIR).
    Index {
        array: Box<Expr>,
        index: Box<Expr>,
    },
    /// `obj.field = value` (only `var` properties of classes).
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

/// A fully resolved pattern (spec 4.6 / 5): variant/field positions
/// are declaration indices, bindings are locals. Named and positional
/// forms are both normalized to `(field index, subpattern)` pairs in
/// declaration order.
#[derive(Debug, Clone)]
pub enum Pattern {
    Binding {
        local: LocalId,
    },
    Wildcard,
    /// A literal matched by an exact ordinary `operator fun equals` target.
    /// The subject type is retained explicitly rather than reconstructed from
    /// the recursive pattern position by a downstream stage.
    Literal {
        value: Expr,
        equals: Callable,
        subject_ty: TypeId,
    },
    Variant {
        application: EnumApplicationId,
        /// Variant index in declaration order.
        variant: u32,
        /// `(field index, subpattern)` in declaration order.
        fields: Vec<(u32, Pattern)>,
    },
    Tuple(Vec<Pattern>),
    Struct {
        application: StructApplicationId,
        fields: Vec<(u32, Pattern)>,
    },
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: TypeId,
    pub span: Span,
    pub origin: ExpressionOrigin,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    StringLiteral(String),
    IntLiteral(i64),
    BoolLiteral(bool),
    UnitLiteral,
    TupleLiteral(Vec<Expr>),
    StructInit {
        application: StructApplicationId,
        args: Vec<Expr>,
    },
    /// Class instantiation `Point(1, 2)`: constructor properties in
    /// declaration order. Base-class delegation is part of the
    /// generated constructor (see mir-lower).
    ClassInit {
        /// The allocated application remains explicit when `Expr::ty` is
        /// adapted to a base class or interface at the use site.
        application: ClassApplicationId,
        args: Vec<Expr>,
    },
    /// Read of a primary-constructor parameter inside a base-constructor
    /// delegation expression.
    ConstructorParam(ConstructorParamId),
    /// Variant construction (`Some(x)`, `Color.Red`, `E.Named(f = 1)`);
    /// `args` are the variant's fields in declaration order, with
    /// constructor-style defaults already filled in.
    VariantConstruct {
        application: EnumApplicationId,
        variant: u32,
        args: Vec<Expr>,
    },
    Local(LocalId),
    GlobalRead(GlobalId),
    /// Read one immutable binding from the current closure environment. The
    /// binding identity is resolved to a concrete field by closure conversion.
    Capture(BindingId),
    Lambda(LambdaId),
    AnonymousFunction(AnonymousFunctionId),
    CallableReference(CallableReferenceId),
    /// A variance-preserving function-value adaptation. `Expr::ty` is the
    /// target type; the typed entity also records both concrete HIR
    /// signatures so MIR cannot lower this as a pointer-only retype.
    FunctionCoercion {
        source: Box<Expr>,
        coercion: FunctionCoercionId,
        target_type: FunctionTypeId,
    },
    /// `Ptr<T>(raw)`; the source constructor is unsafe and normalized here.
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
    /// Native C callback address selected contextually from `::name`.
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
    /// A resolved method call; the dispatch kind (direct / virtual /
    /// interface) is decided at MIR from the receiver's static type.
    MethodCall {
        receiver: Box<Expr>,
        callee: MethodCallee,
        args: Vec<Expr>,
    },
    /// Box a value type into `Any` / an interface (spec 4.4.4). The
    /// target type is `Expr::ty`.
    Box(Box<Expr>),
    /// Unbox a reference back to a value type (from `as` / `as?` /
    /// smart cast). The result type is `Expr::ty`.
    Unbox(Box<Expr>),
    /// `expr is T`; result is `Boolean`. The checked type is in
    /// `check_ty`.
    IsInstance {
        operand: Box<Expr>,
        check_ty: TypeId,
    },
    /// `as` (trap on failure; M8: `ClassCastException`) or `as?`
    /// (`optional` — result `Option<T>`). The target type is
    /// `Expr::ty` (or its payload for `as?`).
    Cast {
        operand: Box<Expr>,
        optional: bool,
    },
    /// `[e1, ...]`; the kind (Array vs MutableArray) is in `Expr::ty`.
    ArrayLiteral(Vec<Expr>),
    /// Fresh immutable-array assembly used by positional `vararg` calls.
    /// Every part is an already evaluated temporary read; `CopyArray` always
    /// copies, including the single-spread case.
    ArrayAssembly(ArrayAssembly),
    /// Subscript read `receiver[index]`; result is the element type.
    Index {
        receiver: Box<Expr>,
        index: Box<Expr>,
    },
    /// `array.size` (spec 10.5); result is `Int`.
    ArrayLen(Box<Expr>),
    /// `Array(m)` / `MutableArray(a)` or `m.toArray()` /
    /// `a.toMutableArray()` conversion (spec 10.4): a memcpy snapshot
    /// of the other array kind with the same element type. The target
    /// kind is in `Expr::ty`.
    ArrayClone(Box<Expr>),
    Call {
        callee: Callable,
        args: Vec<Expr>,
    },
    /// Direct call of a lifted local function. Hidden capture arguments are
    /// explicit and precede source arguments in the lowered ABI.
    LocalFunctionCall {
        local_function: LocalFunctionId,
        callee: Callable,
        captures: Vec<Expr>,
        args: Vec<Expr>,
    },
    /// Calling a managed function value. The callee expression is kept
    /// distinct from direct/virtual/interface named call targets.
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
    // The following are produced by hir-lower's Option desugaring
    // (`?.` / `?:` / `!!`), not directly by surface syntax. MIR turns
    // them into generic enum operations (spec 7.3).
    /// `Some(value)`.
    SomeWrap(Box<Expr>),
    /// The `None` literal; its type is `Expr::ty` (an `Option<T>`).
    NoneLiteral,
    /// Test whether an `Option<T>` is `Some`.
    IsSome(Box<Expr>),
    /// Unwrap an `Option<T>`; `trap_on_none` comes from `!!`
    /// (M3: trap; M8: `UnwrapException`).
    Unwrap {
        operand: Box<Expr>,
        trap_on_none: bool,
    },
}

#[derive(Debug, Clone)]
pub struct ArrayAssembly {
    pub element_type: TypeId,
    pub parts: Vec<ArrayAssemblyPart>,
    pub result_type: ClassApplicationId,
}

#[derive(Debug, Clone)]
pub enum ArrayAssemblyPart {
    Element(Expr),
    CopyArray(Expr),
}

/// Source-level method target. Ordinary receivers already name a resolved
/// callable. A type-parameter receiver instead names a typed upper-bound
/// member that must disappear during HIR concretization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodCallee {
    Callable(Callable),
    Bound(BoundCallableRefId),
    DerivedEquality(DerivedEqualityApplicationId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundCallableRef {
    pub receiver_parameter: TypeParamId,
    pub bound: InterfaceApplicationId,
    pub member: InterfaceMethodId,
    pub instantiated_signature: FunctionTypeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Local(LocalId),
    Global(GlobalId),
}

/// A fully resolved field access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldRef {
    /// Field `index` of one complete struct application.
    StructField {
        application: StructApplicationId,
        index: u32,
    },
    /// Element `index` (0-based) of a tuple.
    TupleIndex(u32),
    /// Constructor property `index` of one complete class application.
    ClassField {
        application: ClassApplicationId,
        index: u32,
    },
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
    /// `===` / `!==` — reference identity (spec 4.4.2).
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
