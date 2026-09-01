//! Fully instantiated HIR consumed exclusively by this Cone's MIR stage.
//!
//! This module intentionally defines its own entity-id family.  No type
//! parameter, generic template, or export-side arena id can be represented
//! here.  HIR lowering must construct this graph completely before MIR starts.

use la_arena::{Arena, Idx};
use scoop_ast::Span;

pub use super::{
    BinOp, CLayout, CallingConvention, ClassModifier, ExternAbi, FunctionAttributes, GcEffect,
    IntrinsicFunction, IntrinsicFunctionKind, IntrinsicProviderId, IntrinsicTypeDeclaration,
    IntrinsicTypeKind, MethodModifier, Safety, StructAttributes, UnOp, Variance,
};

pub type TypeId = Idx<Type>;
pub type FunctionTypeId = Idx<FunctionType>;
pub type LambdaId = Idx<Lambda>;
pub type AnonymousFunctionId = Idx<AnonymousFunction>;
pub type LocalFunctionId = Idx<LocalFunction>;
pub type CallableReferenceId = Idx<CallableReference>;
pub type FunctionCoercionId = Idx<FunctionCoercion>;
pub type ForeignCallbackRegistrationId = Idx<ForeignCallbackRegistration>;
pub type FunctionId = Idx<Function>;
pub type ExternFunctionId = Idx<ExternFunction>;
pub type GlobalId = Idx<Global>;
pub type StructId = Idx<StructDef>;
pub type EnumId = Idx<EnumDef>;
pub type ClassId = Idx<ClassDef>;
pub type InterfaceId = Idx<InterfaceDef>;
pub type LocalId = Idx<Local>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConstructorParamId(u32);

impl ConstructorParamId {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn into_raw(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InterfaceFamilyId(u32);

impl InterfaceFamilyId {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BindingId(u32);

impl BindingId {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn into_raw(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VariantId(u32);

impl VariantId {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn into_raw(self) -> u32 {
        self.0
    }
}

/// A fully resolved type entity.  `gc_free` is mandatory by construction;
/// there is no unknown or deferred state in local-concrete HIR.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Type {
    pub kind: TypeKind,
    pub gc_free: bool,
}

/// A concrete type shape.  There is deliberately no `Param` variant.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeKind {
    Unit,
    Int,
    UInt,
    Boolean,
    String,
    Struct(StructId),
    Class(ClassId),
    Interface(InterfaceId),
    Any,
    Array(TypeId),
    MutableArray(TypeId),
    Tuple(Vec<TypeId>),
    Function(FunctionTypeId),
    Ptr(TypeId),
    FunPtr(FunctionTypeId),
    Enum(EnumId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionType {
    pub is_suspend: bool,
    pub parameter_types: Vec<TypeId>,
    pub return_type: TypeId,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub types: Arena<Type>,
    pub function_types: Arena<FunctionType>,
    pub lambdas: Arena<Lambda>,
    pub anonymous_functions: Arena<AnonymousFunction>,
    pub local_functions: Arena<LocalFunction>,
    pub callable_references: Arena<CallableReference>,
    pub function_coercions: Arena<FunctionCoercion>,
    pub foreign_callback_registrations: Arena<ForeignCallbackRegistration>,
    pub functions: Arena<Function>,
    pub extern_functions: Arena<ExternFunction>,
    pub globals: Arena<Global>,
    pub structs: Arena<StructDef>,
    pub enums: Arena<EnumDef>,
    pub classes: Arena<ClassDef>,
    pub interfaces: Arena<InterfaceDef>,
    pub top_level: Vec<FunctionId>,
    pub unit: TypeId,
    pub int: TypeId,
    pub boolean: TypeId,
    pub string: TypeId,
    pub option_variants: (VariantId, VariantId),
    pub exception_core: ExceptionCore,
    pub coroutine_protocols: Vec<CoroutineProtocol>,
    pub foreign_callback_core: ForeignCallbackCore,
    /// Nominal owners of the fixed compiler-represented types. Generic
    /// intrinsic families are represented by each concrete class instance,
    /// so no parameterized template can leak into this local graph.
    pub intrinsic_type_core: IntrinsicTypeCore,
    pub entry: FunctionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExceptionCore {
    pub throwable: ClassId,
    pub illegal_state_exception: ClassId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignCallbackCore {
    pub mode: EnumId,
    pub state: EnumId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntrinsicTypeCore {
    pub int: StructId,
    pub uint: StructId,
    pub boolean: StructId,
    pub string: ClassId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForeignCallbackMode {
    Reusable,
    OneShot,
}

#[derive(Debug, Clone)]
pub struct ForeignCallbackRegistration {
    pub callback: StructId,
    pub native_function_type: FunctionTypeId,
    pub managed_function_type: FunctionTypeId,
    pub context_index: u32,
    pub mode: ForeignCallbackMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForeignCallbackOperation {
    Retain,
    Release,
    State,
    Failure,
}

/// Fully specialized instances of the generic coroutine protocol for one
/// result type.  MIR consumes these concrete identities and never reads the
/// export-side protocol templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoroutineProtocol {
    pub result_type: TypeId,
    pub continuation: InterfaceId,
    pub suspend_task: InterfaceId,
    pub suspend_registration: InterfaceId,
    pub start_coroutine: FunctionId,
    pub suspend_coroutine: FunctionId,
    pub continuation_resume: FunctionId,
    pub continuation_resume_with_exception: FunctionId,
    pub suspend_task_run: FunctionId,
    pub suspend_registration_register: FunctionId,
}

impl Module {
    pub fn callable_function(&self, callable: Callable) -> FunctionId {
        match callable {
            Callable::Function(function) => function,
        }
    }

    pub fn coroutine_protocol_for_function(
        &self,
        function: FunctionId,
    ) -> Option<&CoroutineProtocol> {
        self.coroutine_protocols.iter().find(|protocol| {
            protocol.start_coroutine == function || protocol.suspend_coroutine == function
        })
    }
}

#[derive(Debug, Clone)]
pub struct Lambda {
    pub function: FunctionId,
    pub function_type: FunctionTypeId,
    pub captures: Vec<Capture>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AnonymousFunction {
    pub function: FunctionId,
    pub function_type: FunctionTypeId,
    pub captures: Vec<Capture>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct LocalFunction {
    pub function: FunctionId,
    pub function_type: FunctionTypeId,
    pub captures: Vec<Capture>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct CallableReference {
    pub target: CallableReferenceTarget,
    pub function_type: FunctionTypeId,
    pub captures: Vec<Capture>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum CallableReferenceTarget {
    Named(Callable),
    Local {
        local_function: LocalFunctionId,
        callee: Callable,
    },
    BoundMember {
        receiver: Box<Expr>,
        callee: Callable,
    },
    BoundExtension {
        receiver: Box<Expr>,
        callee: Callable,
    },
}

#[derive(Debug, Clone)]
pub struct Capture {
    pub binding: BindingId,
    pub name: String,
    pub ty: TypeId,
    pub first_use_span: Span,
    pub source: Expr,
}

#[derive(Debug, Clone)]
pub struct FunctionCoercion {
    pub source: FunctionTypeId,
    pub target: FunctionTypeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Callable {
    Function(FunctionId),
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub type_arguments: Vec<TypeId>,
    pub gc_free: bool,
    pub representation: StructRepresentation,
    pub interfaces: Vec<TypeId>,
    pub methods: Vec<FunctionId>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StructRepresentation {
    Declared {
        attributes: StructAttributes,
        fields: Vec<Field>,
    },
    Intrinsic {
        declaration: IntrinsicTypeDeclaration,
        application: IntrinsicTypeRepresentation,
    },
}

impl StructDef {
    pub fn declared_fields(&self) -> &[Field] {
        match &self.representation {
            StructRepresentation::Declared { fields, .. } => fields,
            StructRepresentation::Intrinsic { .. } => {
                panic!("an intrinsic struct has no source field representation")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    pub type_arguments: Vec<TypeId>,
    pub gc_free: bool,
    pub variants: Vec<Variant>,
    pub option_variants: Option<(VariantId, VariantId)>,
    pub interfaces: Vec<TypeId>,
    pub methods: Vec<FunctionId>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub modifier: ClassModifier,
    pub name: String,
    pub type_arguments: Vec<TypeId>,
    pub representation: ClassRepresentation,
    pub interfaces: Vec<TypeId>,
    pub methods: Vec<FunctionId>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ClassRepresentation {
    Declared {
        constructor: Vec<ConstructorField>,
        base_class: Option<(ClassId, Vec<Expr>)>,
    },
    Intrinsic {
        declaration: IntrinsicTypeDeclaration,
        application: IntrinsicTypeRepresentation,
    },
}

impl ClassDef {
    pub fn declared_constructor(&self) -> &[ConstructorField] {
        match &self.representation {
            ClassRepresentation::Declared { constructor, .. } => constructor,
            ClassRepresentation::Intrinsic { .. } => {
                panic!("an intrinsic class has no source constructor representation")
            }
        }
    }

    pub fn base_class(&self) -> Option<&(ClassId, Vec<Expr>)> {
        match &self.representation {
            ClassRepresentation::Declared { base_class, .. } => base_class.as_ref(),
            ClassRepresentation::Intrinsic { .. } => None,
        }
    }
}

/// Complete concrete representation selected by an intrinsic declaration and
/// this application's already-lowered arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntrinsicTypeRepresentation {
    Int,
    UInt,
    Boolean,
    String,
    Array { element: TypeId },
    MutableArray { element: TypeId },
}

#[derive(Debug, Clone)]
pub struct ConstructorField {
    pub parameter: ConstructorParamId,
    pub name: String,
    pub ty: TypeId,
    pub mutable: bool,
}

#[derive(Debug, Clone)]
pub struct InterfaceDef {
    pub name: String,
    pub family: InterfaceFamilyId,
    /// Variance and arguments are copied onto every concrete application.
    /// MIR therefore never consults the generic interface template.
    pub variances: Vec<Variance>,
    pub type_arguments: Vec<TypeId>,
    pub methods: Vec<MethodSig>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MethodSig {
    pub name: String,
    pub is_suspend: bool,
    pub attributes: FunctionAttributes,
    pub params: Vec<Param>,
    pub return_ty: TypeId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    pub gc_free: bool,
    pub fields: Vec<Field>,
    pub defaults: Vec<Option<Expr>>,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: TypeId,
}

#[derive(Debug, Clone)]
pub struct Global {
    pub name: String,
    pub ty: TypeId,
    pub mutable: bool,
    pub storage: GlobalStorage,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum GlobalStorage {
    Local {
        thread_local: bool,
        initializer: ConstantValue,
    },
    Extern {
        library: String,
        native_symbol: String,
        thread_local: bool,
    },
}

#[derive(Debug, Clone)]
pub enum ConstantValue {
    Int(i64),
    Bool(bool),
    NullPtr,
    NullFunPtr,
    Struct {
        struct_id: StructId,
        fields: Vec<ConstantValue>,
    },
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    /// Complete source/application category. MIR consumes this sum type
    /// directly and never infers genericity or method ownership from an
    /// argument vector, function name, or the optional `method` field.
    pub origin: FunctionOrigin,
    pub is_suspend: bool,
    pub params: Vec<Param>,
    pub return_ty: TypeId,
    pub attributes: FunctionAttributes,
    pub kind: FunctionKind,
    pub method: Option<Method>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionOrigin {
    Free(FreeFunctionOrigin),
    Method(MethodOrigin),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FreeFunctionOrigin {
    Plain,
    Generic {
        definition: super::GenericFunctionId,
        arguments: Vec<TypeId>,
        symbol: InstanceSymbol,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodOrigin {
    pub owner: MethodOwner,
    pub specialization: MethodSpecialization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MethodOwner {
    Class(ClassId),
    Struct(StructId),
    Enum(EnumId),
    Interface(InterfaceId),
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MethodSpecialization {
    Plain,
    OwnerParameterized {
        symbol: InstanceSymbol,
    },
    Generic {
        definition: super::GenericMethodId,
        method_arguments: super::NonEmptyVec<TypeId>,
        symbol: InstanceSymbol,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceSymbol {
    Unique,
    Overloaded { discriminator: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Method {
    pub owner: TypeId,
    pub modifier: MethodModifier,
}

#[derive(Debug, Clone)]
pub struct ExternFunction {
    pub source_name: String,
    pub native_symbol: String,
    pub library: String,
    pub abi: ExternAbi,
    pub calling_convention: CallingConvention,
    pub gc_effect: GcEffect,
    pub safety: Safety,
    pub params: Vec<TypeId>,
    pub return_type: TypeId,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: TypeId,
    pub local: LocalId,
}

#[derive(Debug, Clone)]
pub enum FunctionKind {
    User(Body),
    Intrinsic(IntrinsicFunction),
    Extern(ExternFunctionId),
}

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
    Literal(Expr),
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
        class_id: ClassId,
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
