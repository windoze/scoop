use super::*;

#[derive(Debug, Clone)]
pub struct Body {
    pub locals: Arena<Local>,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub struct Local {
    pub binding: BindingId,
    /// Template-local semantic selector. LocalConcrete HIR combines this
    /// selector with the callable's exact materialization context instead of
    /// deriving persistent value identity from a name or arena position.
    pub selector: scoop_identity::LocalValueSelector,
    /// Closed classification of the local's definition site. Source-backed
    /// values retain their exact source origin; compiler-created temporaries
    /// cannot be mistaken for source definitions.
    pub definition: LocalValueDefinitionSite,
    pub name: String,
    pub ty: TypeId,
    pub mutable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalValueDefinitionSite {
    Source(DefinitionOrigin),
    Synthetic,
}

#[derive(Debug, Clone)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StatementKind {
    Expr(Expr),
    /// Enter the exactly-once gate before a runtime-backed accessor touches
    /// its storage. LocalConcrete's unit declaration carries the exact cycle
    /// throw target selected for the current core-authority branch.
    InitializationEnsure(InitializationUnitId),
    GenericDelegateEnsure(GenericDelegateReference),
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
    GenericDelegateStorage(GenericDelegateReference),
    /// Publish a fully constructed singleton into its moving-GC-aware root.
    SingletonPublishedRoot(SingletonPublishedRootId),
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
    /// Direct write through the non-escaping constructor receiver.
    /// Concretization supplies the hidden initializer receiver.
    InitializingClassField {
        field: InitializingClassFieldRef,
        origin: ExpressionOrigin,
    },
}

#[derive(Debug, Clone)]
pub struct When {
    pub subject: Expr,
    pub arms: Vec<WhenArm>,
    pub fallback: WhenFallback,
}

/// The total fallback edge of a checked `when`.
///
/// An absent source `else` is not enough to make the edge unreachable:
/// HIR carries the exhaustiveness proof that established this fact.
#[derive(Debug, Clone)]
pub enum WhenFallback {
    Else(Vec<Statement>),
    Impossible(ExhaustivenessProof),
}

/// A typed witness produced by the HIR exhaustiveness checker.
#[derive(Debug, Clone)]
pub enum ExhaustivenessProof {
    /// An unguarded recursively-irrefutable arm covers the subject type.
    IrrefutableArm { subject_ty: TypeId },
    /// A recursive pattern matrix covers a tuple, struct, or exact integer
    /// domain even though no individual arm is irrefutable.
    PatternMatrix { subject_ty: TypeId },
    /// Every constructor of this exact enum application, including each
    /// constructor's recursive payload matrix, is covered.
    EnumPatternMatrix { subject_ty: TypeId },
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
    /// A literal matched by an exact, already selected equality plan. The
    /// subject type is retained explicitly rather than reconstructed from the
    /// recursive pattern position by a downstream stage.
    Literal {
        value: Expr,
        equality: LiteralPatternEquality,
        subject_ty: TypeId,
    },
    Variant {
        application: EnumVariantApplication,
        /// Selected payload fields in declaration order.
        fields: Vec<(u32, Pattern)>,
    },
    Tuple(Vec<Pattern>),
    Struct {
        /// Complete subject application, preserving its original declaration.
        owner: TypeId,
        fields: Vec<(u32, Pattern)>,
    },
}

/// Equality selected for a literal pattern while its exact subject type is
/// available. Integer equality is representation-level and must reach MIR as
/// a typed comparison; every other literal kind keeps its ordinary callable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteralPatternEquality {
    Integer { kind: IntegerKind },
    Ordinary { equals: CallableTarget },
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
    StringLiteral {
        value: String,
        owner: StringConstantOwner<PropertyId>,
    },
    IntegerLiteral(HirIntegerConstant),
    BoolLiteral(bool),
    UnitLiteral,
    TupleLiteral(Vec<Expr>),
    StructInit {
        constructor: StructConstructorApplicationId,
        args: Vec<Expr>,
    },
    /// Compiler-only raw struct reconstruction in declaration field order.
    /// Unlike `StructInit`, this never invokes a source constructor.
    StructConstruct {
        application: StructApplicationId,
        fields: Vec<Expr>,
    },
    /// Class instantiation `Point(1, 2)`: constructor properties in
    /// declaration order. Base-class delegation is part of the
    /// generated constructor (see mir-lower).
    ClassInit {
        /// The allocated application remains explicit when `Expr::ty` is
        /// adapted to a base class or interface at the use site.
        constructor: ClassConstructorApplicationId,
        args: Vec<Expr>,
    },
    /// Read of a primary-constructor parameter inside a base-constructor
    /// delegation expression.
    ConstructorParam(ConstructorParamId),
    ConstructorReceiver,
    /// Variant construction (`Some(x)`, `Color.Red`, `E.Named(f = 1)`);
    /// `args` are the variant's fields in declaration order, with
    /// constructor-style defaults already filled in.
    VariantConstruct {
        variant: EnumVariantApplication,
        args: Vec<Expr>,
    },
    VariantTest {
        operand: Box<Expr>,
        variant: EnumVariantApplication,
    },
    VariantPayloadProject {
        operand: Box<Expr>,
        field: EnumVariantFieldApplication,
    },
    Local(LocalId),
    GlobalRead(GlobalId),
    GenericDelegateStorageRead(GenericDelegateReference),
    /// Read the unique value after passing its exactly-once gate.
    SingletonValue(SingletonValueTarget),
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
    /// `Ptr<T>(raw: ULong)`; the source constructor is unsafe and its nonzero
    /// precondition has already been checked when this node is constructed.
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
    /// Native C callback address selected contextually from `::name`.
    FunctionAddress(CallableTarget),
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
    /// Direct read through the non-escaping class initializer receiver.
    InitializingClassFieldAccess {
        field: InitializingClassFieldRef,
    },
    ReleaseFieldLoad(ReleaseFieldRef),
    /// Direct read from the fully formed struct value owned by a secondary
    /// constructor. The value itself never becomes an expression.
    InitializingStructFieldAccess {
        owner: TypeId,
        field: scoop_identity::PersistentFieldId,
    },
    /// A resolved method call; the dispatch kind (direct / virtual /
    /// interface) is decided at MIR from the receiver's static type.
    MethodCall {
        receiver: Box<Expr>,
        callee: MethodCallee,
        args: Vec<Expr>,
    },
    /// A call resolved solely against the direct base-class member layer.
    /// The dedicated variant is the proof that downstream dispatch must stay
    /// direct even when `callee` belongs to a virtual family.
    DirectSuperMethodCall {
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
    /// Preserve the operand's type while viewing its reference as a supertype.
    ReferenceUpcast(Box<Expr>),
    /// `expr is T`; result is `Boolean`. The checked type is in
    /// `check_ty`.
    IsInstance {
        operand: Box<Expr>,
        check_ty: TypeId,
    },
    /// `as` (trap on failure; M8: `ClassCastException`) or `as?`
    /// (`optional` — result `Option<T>`). The checked target remains
    /// explicit even when the result wraps that type in `Option`.
    Cast {
        operand: Box<Expr>,
        check_ty: TypeId,
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
        access: ArrayAccessKind,
        receiver: Box<Expr>,
        index: Box<Expr>,
    },
    /// A winning `MutableArray.set` intrinsic. It remains an expression
    /// because explicit method syntax has the ordinary `Unit` result.
    ArraySet {
        access: ArrayAccessKind,
        receiver: Box<Expr>,
        index: Box<Expr>,
        value: Box<Expr>,
    },
    /// `array.size` (spec 10.5); result is canonical `Long`.
    ArrayLen(Box<Expr>),
    /// `Array(m)` / `MutableArray(a)` or `m.toArray()` /
    /// `a.toMutableArray()` conversion (spec 10.4): a memcpy snapshot
    /// of the other array kind with the same element type. The target
    /// kind is in `Expr::ty`.
    ArrayClone(Box<Expr>),
    Call {
        callee: CallableTarget,
        /// Namespace lookup routes when this occurrence used an import.
        /// A member is resolved directly from its nominal declaration.
        binding: Option<std::sync::Arc<DirectImportedTargetBinding>>,
        args: Vec<Expr>,
        receiver: crate::SourceCallReceiver<TypeId>,
    },
    /// Calling a managed function value. The callee expression is kept
    /// distinct from direct/virtual/interface named call targets.
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
    /// A compiler-recognized integer member after ordinary call resolution.
    /// The registry entry retains the exact operand kind and an effect-refined
    /// source target; source intrinsic text is no longer representable.
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
    // The following are produced by hir-lower's Option desugaring
    // (`?.` / `?:` / `!!`), not directly by surface syntax. MIR turns
    // them into generic enum construction and representation-independent
    // checked variant operations (spec 7.3).
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

/// Semantic owner selected before a string constant reaches MIR.
///
/// Ordinary expression literals are completed with the concrete callable (or
/// initialization unit) that materializes their body. A folded property
/// constant instead retains the property declaration that owns its one
/// canonical immutable object, independent of how many use sites inline it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringConstantOwner<P> {
    CurrentDefinition,
    Property(P),
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
    pub result_type: TypeId,
}

#[derive(Debug, Clone)]
pub enum ArrayAssemblyPart {
    Element(Expr),
    CopyArray(Expr),
}

/// Dispatch requested by member syntax before its HIR node is built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberCallKind {
    Ordinary,
    DirectSuper,
}

/// Source-level method target. Ordinary receivers already name a resolved
/// callable. A type-parameter receiver instead names a typed upper-bound
/// member that must disappear during HIR concretization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodCallee {
    Callable(CallableTarget),
    Bound(BoundCallableRefId),
    DerivedEquality(DerivedEqualityApplicationId),
    ImportedDerivedEquality {
        target: ImportedDerivedEqualityUseId,
        owner: TypeId,
    },
}

impl MethodCallee {
    pub fn declared_callable(self, bounds: &Arena<BoundCallableRef>) -> Option<CallableTarget> {
        match self {
            Self::Callable(callable) => Some(callable),
            Self::Bound(bound) => Some(bounds[bound].declared_callable()),
            Self::DerivedEquality(_) | Self::ImportedDerivedEquality { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundCallableRef {
    pub receiver_type: TypeId,
    pub source: BoundCallableSource,
    pub instantiated_signature: FunctionTypeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundCallableSource {
    Class {
        bound: ClassApplicationId,
        callable: CallableTarget,
    },
    Interface {
        bound: InterfaceApplicationId,
        member: InterfaceMethodReference,
        declared: CallableTarget,
    },
}

impl BoundCallableRef {
    pub fn declared_callable(&self) -> CallableTarget {
        match self.source {
            BoundCallableSource::Class { callable, .. } => callable,
            BoundCallableSource::Interface { declared, .. } => declared,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    Local(LocalId),
    Global(GlobalId),
    /// A provider-owned extern property, using the ordinary native contract.
    ExternalGlobal {
        property: scoop_identity::PersistentPropertyId,
        source_contract: std::sync::Arc<scoop_identity::SourceNativeExternalContractRecord>,
        ty: TypeId,
    },
}

/// A ready backing field reached through the current initializer receiver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitializingClassFieldRef {
    pub owner: TypeId,
    pub field: scoop_identity::PersistentFieldId,
}

/// A fully resolved field access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldRef {
    StructField {
        owner: TypeId,
        field: scoop_identity::PersistentFieldId,
    },
    ClassField {
        owner: TypeId,
        field: scoop_identity::PersistentFieldId,
    },
    /// Element `index` (0-based) of a tuple.
    TupleIndex(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Lt,
    Le,
    Gt,
    Ge,
    /// `===` / `!==` — reference identity (spec 4.4.2).
    RefEq,
    RefNe,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Not,
}

/// An original variant in one complete, possibly symbolic enum application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnumVariantApplication {
    pub owner: TypeId,
    pub variant: scoop_identity::PersistentEnumVariantId,
}

/// A payload field retains its original variant and field identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnumVariantFieldApplication {
    pub variant: EnumVariantApplication,
    pub field: scoop_identity::PersistentEnumVariantFieldId,
}
