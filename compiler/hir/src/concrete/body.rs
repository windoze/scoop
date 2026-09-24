use super::*;
use crate::{ArrayAccessKind, PrimitiveBinaryKind, PrimitiveUnaryKind};

#[derive(Debug, Clone)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StatementKind {
    Expr(Expr),
    InitializationEnsure(InitializationUnitId),
    LocalFunction(LocalFunctionId),
    Return {
        /// Absent in `Unit` functions (bare `return`). If substitution makes
        /// a generic return expression exactly `Unit`, concretization emits
        /// that expression as a preceding statement to preserve evaluation.
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
        target: LoopId,
        condition_setup: Vec<Statement>,
        cond: Expr,
        body: Vec<Statement>,
    },
    Break {
        target: LoopId,
    },
    Continue {
        target: LoopId,
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
    SingletonPublishedRoot(SingletonPublishedRootId),
    Index {
        array: Box<Expr>,
        index: Box<Expr>,
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
    pub fallback: WhenFallback,
}

/// Local-concrete counterpart of the checked Export HIR fallback edge.
#[derive(Debug, Clone)]
pub enum WhenFallback {
    Else(Vec<Statement>),
    Impossible(ExhaustivenessProof),
}

/// A fully instantiated exhaustiveness witness.
#[derive(Debug, Clone)]
pub enum ExhaustivenessProof {
    IrrefutableArm {
        subject_ty: TypeId,
    },
    /// A tuple, struct, or exact integer domain is covered by the complete
    /// recursive pattern matrix.
    PatternMatrix {
        subject_ty: TypeId,
    },
    EnumPatternMatrix {
        subject_ty: TypeId,
        enum_id: EnumId,
    },
}

#[derive(Debug, Clone)]
pub struct WhenArm {
    pub pattern: Pattern,
    pub guard: Option<WhenGuard>,
    pub body: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct WhenGuard {
    pub setup: Vec<Statement>,
    pub condition: Expr,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Binding {
        local: LocalId,
    },
    Wildcard,
    Literal {
        value: Expr,
        /// Exact equality plan selected by Export HIR.
        equality: LiteralPatternEquality,
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

/// Fully instantiated literal-pattern equality plan. The mutually exclusive
/// variants prevent an integer intrinsic from being mistaken for a user
/// function by MIR lowering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteralPatternEquality {
    Integer { kind: IntegerKind },
    Ordinary { equals: Callable },
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: TypeId,
    pub span: Span,
    pub origin: ConcreteExpressionOrigin,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    StringLiteral {
        value: String,
        owner: StringConstantOwner<scoop_identity::PropertyOwner>,
    },
    IntegerLiteral(HirIntegerConstant),
    BoolLiteral(bool),
    UnitLiteral,
    TupleLiteral(Vec<Expr>),
    StructInit {
        struct_id: StructId,
        args: Vec<Expr>,
    },
    StructConstruct {
        struct_id: StructId,
        fields: Vec<Expr>,
    },
    StructConstructorCall {
        constructor: StructConstructorId,
        args: Vec<Expr>,
    },
    ClassNew {
        constructor: ClassConstructorId,
        args: Vec<Expr>,
    },
    ClassInitializerCall {
        receiver: Box<Expr>,
        initializer: ClassConstructorId,
        args: Vec<Expr>,
    },
    /// Ordinary, fully initialized receiver value inside a checked concrete
    /// constructor implementation. Source readiness capabilities are gone.
    ConstructorReceiver,
    ConstructorParam(ConstructorParamId),
    VariantConstruct {
        variant: EnumVariantRef,
        args: Vec<Expr>,
    },
    VariantTest {
        operand: Box<Expr>,
        variant: EnumVariantRef,
    },
    VariantPayloadProject {
        operand: Box<Expr>,
        field: EnumVariantFieldRef,
    },
    Local(LocalId),
    GlobalRead(GlobalId),
    SingletonValue(SingletonValueId),
    Capture(BindingId),
    Lambda(LambdaId),
    AnonymousFunction(AnonymousFunctionId),
    CallableReference(CallableReferenceId),
    FunctionCoercion {
        source: Box<Expr>,
        coercion: FunctionCoercionId,
        target_type: FunctionTypeId,
    },
    PtrFromNonZeroULong(Box<Expr>),
    PtrToULong(Box<Expr>),
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
    DirectSuperMethodCall {
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
        access: ArrayAccessKind,
        receiver: Box<Expr>,
        index: Box<Expr>,
    },
    ArraySet {
        access: ArrayAccessKind,
        receiver: Box<Expr>,
        index: Box<Expr>,
        value: Box<Expr>,
    },
    ArrayLen(Box<Expr>),
    ArrayClone(Box<Expr>),
    Call {
        callee: Callable,
        args: Vec<Expr>,
    },
    ImportedDependencyCall {
        callee: ImportedDependencyCallableUseId,
        binding: std::sync::Arc<crate::DirectImportedTargetBinding>,
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
    PrimitiveBinary {
        kind: PrimitiveBinaryKind,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    PrimitiveUnary {
        kind: PrimitiveUnaryKind,
        operand: Box<Expr>,
    },
    IntegerOperation {
        operation: HirIntegerOperation,
        arguments: HirIntegerOperationArguments,
    },
    IntegerConversion {
        conversion: HirIntegerConversion,
        operand: Box<Expr>,
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
pub enum HirIntegerOperationArguments {
    Unary(Box<Expr>),
    Binary { lhs: Box<Expr>, rhs: Box<Expr> },
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
    StructField(StructFieldRef),
    TupleIndex(u32),
    ClassField { class_id: ClassId, index: u32 },
}
