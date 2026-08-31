//! HIR definitions and HIR meta: the data channel between HIR and MIR.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.2 and
//! `docs/milestone4/DESIGN.md` section 3.2.
//!
//! Structural completeness rules (see AGENTS.md): every expression
//! carries its resolved type (`Expr::ty`), every call carries its
//! resolved target and type arguments, patterns carry resolved
//! variant/field indices and binding locals, and a module always has
//! an entry point (`Module::entry`).

use la_arena::{Arena, Idx};
use scoop_ast::Span;

pub type TypeId = Idx<Type>;
pub type FunctionTypeId = Idx<FunctionType>;
pub type LambdaId = Idx<Lambda>;
pub type AnonymousFunctionId = Idx<AnonymousFunction>;
pub type LocalFunctionId = Idx<LocalFunction>;
pub type CallableReferenceId = Idx<CallableReference>;
pub type FunctionCoercionId = Idx<FunctionCoercion>;
pub type FunctionId = Idx<Function>;
pub type ExternFunctionId = Idx<ExternFunction>;
pub type GlobalId = Idx<Global>;
pub type GenericFunctionId = Idx<GenericFunction>;
pub type ResolvedGenericFunctionId = Idx<ResolvedGenericFunction>;
pub type StructId = Idx<StructDecl>;
pub type EnumId = Idx<EnumDecl>;
pub type ClassId = Idx<ClassDecl>;
pub type InterfaceId = Idx<InterfaceDecl>;
pub type LocalId = Idx<Local>;

/// Cone-wide identity of a lexical value binding. Unlike `LocalId`, which is
/// only meaningful inside one function body's local arena, this identity is
/// stable across nested callable bodies and can therefore name a capture
/// without falling back to a source name.
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

/// A function- or generic-type-local type-parameter index. This is a
/// distinct id type so it cannot be mixed with field, variant or
/// arena indices by accident.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeParamId(u32);

impl TypeParamId {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn into_raw(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Unit,
    Int,
    /// Unsigned 64-bit integer (`UInt`, spec 11.2).
    UInt,
    Boolean,
    String,
    /// A struct type with resolved type arguments (empty for
    /// non-generic structs). Keeping the arguments in the type itself
    /// makes every `TypeId` structurally complete.
    Struct(StructId, Vec<TypeId>),
    /// A reference type declared with `class` (spec 9.1).
    Class(ClassId),
    /// An interface application with complete type arguments (empty for a
    /// non-generic interface). Values behind it are references.
    Interface(InterfaceId, Vec<TypeId>),
    /// The root of all types (spec 3.1). Value types reaching it are
    /// boxed (spec 4.4.4).
    Any,
    /// Compiler-built-in array types (M5; class declarations arrive
    /// with M7, see docs/milestone5/DESIGN.md 5.1). Invariant in the
    /// element type (spec 10.4).
    Array(TypeId),
    MutableArray(TypeId),
    Tuple(Vec<TypeId>),
    /// A managed function value type. The referenced entry carries the
    /// complete structural signature and is canonical within the Cone.
    Function(FunctionTypeId),
    /// A GC-free typed raw data pointer. The pointee remains explicit in all
    /// IR stages; it is never recovered from an integer representation.
    Ptr(TypeId),
    /// A GC-free native C function pointer. Its signature reuses M11's
    /// canonical function-type identity but is not a managed function value.
    FunPtr(FunctionTypeId),
    /// An enum type with resolved type arguments (empty for
    /// non-generic enums). `Option<T>` is one of these since M4
    /// (defined in `scoop.core`).
    Enum(EnumId, Vec<TypeId>),
    /// A type parameter, by typed local index. Only appears inside a
    /// generic function/type definition; instantiated MIR never
    /// contains it.
    Param(TypeParamId),
}

/// Canonical structural identity of an ordinary or suspend function type.
/// Declaration-only metadata such as parameter names/defaults is absent by
/// construction (spec 8.1.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionType {
    pub is_suspend: bool,
    pub parameter_types: Vec<TypeId>,
    pub return_type: TypeId,
}

/// Structural type equality (tuple types are compared by elements,
/// enum types by identity plus arguments).
pub fn types_equal(module: &Module, a: TypeId, b: TypeId) -> bool {
    match (&module.types[a], &module.types[b]) {
        (Type::Unit, Type::Unit)
        | (Type::Int, Type::Int)
        | (Type::UInt, Type::UInt)
        | (Type::Boolean, Type::Boolean)
        | (Type::String, Type::String) => true,
        (Type::Struct(x, x_args), Type::Struct(y, y_args)) => {
            x == y
                && x_args.len() == y_args.len()
                && x_args
                    .iter()
                    .zip(y_args.iter())
                    .all(|(x, y)| types_equal(module, *x, *y))
        }
        (Type::Class(x), Type::Class(y)) => *x == *y,
        (Type::Interface(x, x_args), Type::Interface(y, y_args)) => {
            x == y
                && x_args.len() == y_args.len()
                && x_args
                    .iter()
                    .zip(y_args.iter())
                    .all(|(x, y)| types_equal(module, *x, *y))
        }
        (Type::Any, Type::Any) => true,
        (Type::Array(x), Type::Array(y)) | (Type::MutableArray(x), Type::MutableArray(y)) => {
            types_equal(module, *x, *y)
        }
        (Type::Tuple(xs), Type::Tuple(ys)) => {
            xs.len() == ys.len()
                && xs
                    .iter()
                    .zip(ys.iter())
                    .all(|(x, y)| types_equal(module, *x, *y))
        }
        (Type::Function(x), Type::Function(y)) => x == y,
        (Type::Ptr(x), Type::Ptr(y)) => types_equal(module, *x, *y),
        (Type::FunPtr(x), Type::FunPtr(y)) => x == y,
        (Type::Enum(x, x_args), Type::Enum(y, y_args)) => {
            x == y
                && x_args.len() == y_args.len()
                && x_args
                    .iter()
                    .zip(y_args.iter())
                    .all(|(x, y)| types_equal(module, *x, *y))
        }
        (Type::Param(x), Type::Param(y)) => x == y,
        _ => false,
    }
}

/// Render a type for diagnostics and dumps.
pub fn type_name(module: &Module, ty: TypeId) -> String {
    match &module.types[ty] {
        Type::Unit => "Unit".to_string(),
        Type::Int => "Int".to_string(),
        Type::UInt => "UInt".to_string(),
        Type::Boolean => "Boolean".to_string(),
        Type::String => "String".to_string(),
        Type::Struct(id, args) => {
            let name = &module.structs[*id].name;
            if args.is_empty() {
                name.clone()
            } else {
                let inner: Vec<String> = args.iter().map(|t| type_name(module, *t)).collect();
                format!("{}<{}>", name, inner.join(", "))
            }
        }
        Type::Class(id) => module.classes[*id].name.clone(),
        Type::Interface(id, args) => {
            let name = &module.interfaces[*id].name;
            if args.is_empty() {
                name.clone()
            } else {
                let inner: Vec<String> = args.iter().map(|t| type_name(module, *t)).collect();
                format!("{}<{}>", name, inner.join(", "))
            }
        }
        Type::Any => "Any".to_string(),
        Type::Array(inner) => format!("Array<{}>", type_name(module, *inner)),
        Type::MutableArray(inner) => format!("MutableArray<{}>", type_name(module, *inner)),
        Type::Enum(id, args) => {
            let name = &module.enums[*id].name;
            if args.is_empty() {
                name.clone()
            } else {
                let inner: Vec<String> = args.iter().map(|t| type_name(module, *t)).collect();
                format!("{}<{}>", name, inner.join(", "))
            }
        }
        Type::Tuple(elements) => {
            let inner: Vec<String> = elements.iter().map(|t| type_name(module, *t)).collect();
            format!("({})", inner.join(", "))
        }
        Type::Function(id) => {
            let function = &module.function_types[*id];
            let parameters: Vec<String> = function
                .parameter_types
                .iter()
                .map(|ty| type_name(module, *ty))
                .collect();
            let suspend = if function.is_suspend { "suspend " } else { "" };
            format!(
                "{suspend}({}) -> {}",
                parameters.join(", "),
                type_name(module, function.return_type)
            )
        }
        Type::Ptr(pointee) => format!("Ptr<{}>", type_name(module, *pointee)),
        Type::FunPtr(id) => {
            let function = &module.function_types[*id];
            let parameters = function
                .parameter_types
                .iter()
                .map(|ty| type_name(module, *ty))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "FunPtr<({parameters}) -> {}>",
                type_name(module, function.return_type)
            )
        }
        Type::Param(index) => format!("T{}", index.into_raw()),
    }
}

#[derive(Debug, Clone)]
pub struct Module {
    pub types: Arena<Type>,
    /// Canonical function signatures referenced by `Type::Function`.
    pub function_types: Arena<FunctionType>,
    /// Source callable-value entities. Their identities are intentionally
    /// separate from the generated invoke functions they own.
    pub lambdas: Arena<Lambda>,
    pub anonymous_functions: Arena<AnonymousFunction>,
    pub local_functions: Arena<LocalFunction>,
    pub callable_references: Arena<CallableReference>,
    /// Source/target signatures of every explicit function-value variance
    /// adaptation requested by HIR.
    pub function_coercions: Arena<FunctionCoercion>,
    pub functions: Arena<Function>,
    /// Native functions imported by source declarations. They have no HIR
    /// body and their identities never enter generic instantiation.
    pub extern_functions: Arena<ExternFunction>,
    /// Top-level storage declarations. Globals use an identity distinct from
    /// functions and locals, and every entry carries a complete storage kind.
    pub globals: Arena<Global>,
    /// Generic function definitions. Their ids are distinct from
    /// ordinary `FunctionId`s even though each entry points at the HIR
    /// function that owns the parameterized body.
    pub generic_functions: Arena<GenericFunction>,
    pub structs: Arena<StructDecl>,
    pub enums: Arena<EnumDecl>,
    pub classes: Arena<ClassDecl>,
    pub interfaces: Arena<InterfaceDecl>,
    /// Top-level functions in declaration order (core library first,
    /// then user code).
    pub top_level: Vec<FunctionId>,
    /// Well-known types, allocated first by hir-lower.
    pub unit: TypeId,
    pub int: TypeId,
    pub boolean: TypeId,
    pub string: TypeId,
    /// The `Option` enum from `scoop.core` (the desugar target of
    /// `T?`, spec 7.1). Guaranteed present: a core library without a
    /// suitable `Option` definition is a driver-level error.
    pub option_enum: EnumId,
    /// Compiler-known coroutine protocol entities. HIR lowering validates
    /// their exact declarations before constructing the module, so MIR never
    /// falls back to textual lookup for protocol types or methods.
    pub coroutine_core: CoroutineCore,
    /// Compiler-known pointer/FFI core entities. HIR lowering validates the
    /// unique source declarations and downstream stages use these typed ids,
    /// never textual names.
    pub ffi_core: FfiCore,
    /// Entry point: `fun main()`. Guaranteed present.
    pub entry: FunctionId,
    /// Resolved generic function applications, deduplicated in
    /// first-use order. The arena id is carried directly by call
    /// expressions and is the instantiation request consumed by MIR.
    pub instantiations: Arena<ResolvedGenericFunction>,
}

#[derive(Debug, Clone, Copy)]
pub struct FfiCore {
    pub ptr: StructId,
    pub fun_ptr: StructId,
    pub pinned_ptr: StructId,
    pub gc_handle: StructId,
    pub ptr_to_uint: FunctionId,
    pub ptr_cast: FunctionId,
    pub ptr_load: FunctionId,
    pub ptr_load_offset: FunctionId,
    pub ptr_store: FunctionId,
    pub ptr_store_offset: FunctionId,
    pub ptr_plus: FunctionId,
    pub ptr_minus: FunctionId,
    pub address_of: FunctionId,
    pub size_of: FunctionId,
    pub align_of: FunctionId,
    pub gc_pin_raw: FunctionId,
    pub gc_unpin_raw: FunctionId,
    pub gc_get_handle_raw: FunctionId,
    pub gc_release_handle_raw: FunctionId,
}

#[derive(Debug, Clone)]
pub struct Lambda {
    pub function: FunctionId,
    pub function_type: FunctionTypeId,
    /// Type parameters inherited from the enclosing generic callable. The
    /// generated invoke body is instantiated with this complete prefix.
    pub owner_type_param_count: usize,
    /// Structurally present even for no-capture lambdas; later M11 capture
    /// analysis fills this list rather than changing the entity shape.
    pub captures: Vec<Capture>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AnonymousFunction {
    pub function: FunctionId,
    pub function_type: FunctionTypeId,
    pub owner_type_param_count: usize,
    pub captures: Vec<Capture>,
    pub span: Span,
}

/// A block-local named function. `function` is its lifted body; direct calls
/// pass `captures` as hidden parameters, while taking `::name` materializes a
/// closure over the same body.
#[derive(Debug, Clone)]
pub struct LocalFunction {
    pub function: FunctionId,
    pub function_type: FunctionTypeId,
    pub captures: Vec<Capture>,
    /// Type parameters inherited from enclosing generic callables form the
    /// prefix of the lifted function's combined type-parameter namespace.
    pub owner_type_param_count: usize,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct CallableReference {
    pub target: CallableReferenceTarget,
    pub function_type: FunctionTypeId,
    /// Type parameters of the callable containing this reference expression.
    /// A non-zero value requires a concrete closure per enclosing instance.
    pub owner_type_param_count: usize,
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
    /// A member reference whose receiver expression is evaluated when the
    /// closure is created. The receiver's static type remains attached to the
    /// expression so MIR can preserve direct / virtual / interface dispatch.
    BoundMember {
        receiver: Box<Expr>,
        callee: Callable,
    },
    /// A bound extension reference. Unlike a member reference its invoke
    /// wrapper always direct-calls the extension body, prepending the saved
    /// receiver to the ordinary source arguments.
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
    /// Expression evaluated in the immediately enclosing callable when the
    /// closure object is created. It is either a local read or a transitive
    /// capture read, and therefore preserves by-value creation-time semantics.
    pub source: Expr,
}

#[derive(Debug, Clone)]
pub struct FunctionCoercion {
    pub source: FunctionTypeId,
    pub target: FunctionTypeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoroutineCore {
    pub throwable: ClassId,
    pub illegal_state_exception: ClassId,
    pub continuation: InterfaceId,
    pub continuation_resume: FunctionId,
    pub continuation_resume_with_exception: FunctionId,
    pub suspend_task: InterfaceId,
    pub suspend_task_run: FunctionId,
    pub suspend_registration: InterfaceId,
    pub suspend_registration_register: FunctionId,
    pub start_coroutine: FunctionId,
    pub suspend_coroutine: FunctionId,
}

impl Module {
    pub fn callable_function(&self, callable: Callable) -> FunctionId {
        callable_parts(self, callable).0
    }

    pub fn callable_type_args(&self, callable: Callable) -> &[TypeId] {
        callable_parts(self, callable).1.unwrap_or(&[])
    }
}

/// A generic HIR function definition. Generic identity is deliberately
/// separate from the underlying function identity (AGENTS.md).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericFunction {
    pub function: FunctionId,
}

/// A generic function with every call-site type argument resolved.
/// MIR consumes this entity to produce a separate monomorphized
/// function entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedGenericFunction {
    pub generic: GenericFunctionId,
    pub type_args: Vec<TypeId>,
}

/// The fully-resolved callable stored on HIR calls. A generic call
/// cannot be represented as a plain function plus an unrelated type
/// argument vector: it must reference a resolved generic entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Callable {
    Function(FunctionId),
    Generic(ResolvedGenericFunctionId),
}

#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: String,
    pub type_params: Vec<TypeParamDecl>,
    pub attributes: StructAttributes,
    pub fields: Vec<Field>,
    pub interfaces: Vec<TypeId>,
    pub span: Span,
}

/// Typed struct attributes. Raw annotation names and argument syntax never
/// cross the AST/HIR boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StructAttributes {
    pub c_layout: Option<CLayout>,
    pub interior_mutable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CLayout {
    pub aligned: u8,
    pub packed: u8,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub type_params: Vec<TypeParamDecl>,
    pub variants: Vec<Variant>,
    pub interfaces: Vec<TypeId>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassModifier {
    Final,
    Open,
    Abstract,
}

/// Effective dispatch modality of a member function (spec 9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodModifier {
    Final,
    Open,
    Abstract,
}

/// Member-only function metadata. Keeping owner and modality together
/// makes it impossible for a method to reach downstream stages without
/// a dispatch modality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Method {
    pub owner: TypeId,
    pub modifier: MethodModifier,
    /// Number of owner type parameters at the front of the containing
    /// function's combined type-parameter namespace. Method parameters
    /// follow this prefix.
    pub owner_type_param_count: u32,
}

#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub modifier: ClassModifier,
    pub name: String,
    /// Primary-constructor properties in declaration order.
    pub constructor: Vec<Field>,
    /// Base class and the resolved constructor argument expressions.
    pub base_class: Option<(ClassId, Vec<Expr>)>,
    pub interfaces: Vec<TypeId>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct InterfaceDecl {
    pub name: String,
    pub type_params: Vec<TypeParamDecl>,
    pub methods: Vec<MethodSig>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variance {
    Invariant,
    In,
    Out,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParamDecl {
    pub name: String,
    pub variance: Variance,
    pub kind: TypeParamKind,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeParamKind {
    Any,
    Value,
    Ref,
}

/// An interface method signature (M6: no body, no properties).
#[derive(Debug, Clone)]
pub struct MethodSig {
    pub name: String,
    /// Suspend is part of the callable contract and must match exactly
    /// across interface implementation and overriding relationships.
    pub is_suspend: bool,
    pub attributes: FunctionAttributes,
    /// Type parameters declared by this method (the owning interface's
    /// parameters are stored on `InterfaceDecl`). An empty list means the
    /// method occupies an itable slot; generic methods are static-only.
    pub type_params: Vec<TypeParamDecl>,
    pub params: Vec<Param>,
    pub return_ty: TypeId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    /// Fields in declaration order; unit variants have none. Named and
    /// constructor-style fields carry their names (and defaults),
    /// positional fields have generated `_1`-style names.
    pub fields: Vec<Field>,
    /// Constructor-style default values (constant expressions in M4).
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

/// A typed, GC-free initializer accepted for local global storage.
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
    /// Whether calls use the coroutine ABI rather than the ordinary ABI.
    pub is_suspend: bool,
    /// Typed generic parameters; empty for non-generic functions. For
    /// methods this is one combined namespace: owner parameters first,
    /// method-declared parameters second (`Method::owner_type_param_count`
    /// separates the two groups).
    pub type_params: Vec<TypeParamDecl>,
    pub params: Vec<Param>,
    pub return_ty: TypeId,
    pub attributes: FunctionAttributes,
    pub kind: FunctionKind,
    /// Member metadata; the receiver of a method is the first entry of
    /// `params` (named `this`). Top-level functions have `None`.
    pub method: Option<Method>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionAttributes {
    pub safety: Safety,
    pub gc_effect: GcEffect,
    /// M12 currently supports only cdecl for native-addressable functions.
    pub calling_convention: CallingConvention,
}

impl Default for FunctionAttributes {
    fn default() -> Self {
        Self {
            safety: Safety::Safe,
            gc_effect: GcEffect::Managed,
            calling_convention: CallingConvention::Cdecl,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Safety {
    Safe,
    Unsafe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcEffect {
    Managed,
    NoGc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallingConvention {
    Cdecl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternAbi {
    C,
    Scoop,
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
    /// Parameters are (immutable) locals.
    pub local: LocalId,
}

#[derive(Debug, Clone)]
pub enum FunctionKind {
    User(Body),
    /// A `@Intrinsic("name")` function (spec 13.1); the name is
    /// guaranteed to be in the compiler's intrinsic registry.
    Intrinsic(String),
    /// A bodyless native declaration. Complete ABI metadata lives in the
    /// independent extern arena and is referenced by a typed id.
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
        array: Expr,
        index: Expr,
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
    /// A literal matched by equality (the expression is a literal).
    Literal(Expr),
    Variant {
        enum_id: EnumId,
        /// Variant index in declaration order.
        variant: u32,
        /// `(field index, subpattern)` in declaration order.
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
    /// Class instantiation `Point(1, 2)`: constructor properties in
    /// declaration order. Base-class delegation is part of the
    /// generated constructor (see mir-lower).
    ClassInit {
        class_id: ClassId,
        args: Vec<Expr>,
    },
    /// Variant construction (`Some(x)`, `Color.Red`, `E.Named(f = 1)`);
    /// `args` are the variant's fields in declaration order, with
    /// constructor-style defaults already filled in.
    VariantConstruct {
        enum_id: EnumId,
        variant: u32,
        type_args: Vec<TypeId>,
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
    FieldAccess {
        receiver: Box<Expr>,
        field: FieldRef,
    },
    /// A resolved method call; the dispatch kind (direct / virtual /
    /// interface) is decided at MIR from the receiver's static type.
    MethodCall {
        receiver: Box<Expr>,
        callee: Callable,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Local(LocalId),
    Global(GlobalId),
}

/// A fully resolved field access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldRef {
    /// Field `index` of the struct type `struct_id`.
    StructField { struct_id: StructId, index: u32 },
    /// Element `index` (0-based) of a tuple.
    TupleIndex(u32),
    /// Constructor property `index` of the class `class_id` (a heap
    /// object load).
    ClassField { class_id: ClassId, index: u32 },
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

/// The compiler's intrinsic registry (impl spec 2.10). Signature rules
/// live with hir-lower; this table is the single source of truth for
/// valid names, expansion stage, and backend kind.
pub const INTRINSIC_REGISTRY: &[IntrinsicSpec] = &[
    IntrinsicSpec {
        name: "gc_pin_raw",
        stage: IntrinsicStage::Mir,
        kind: IntrinsicKind::Runtime("scoop_rt_pin"),
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::UNSAFE,
    },
    IntrinsicSpec {
        name: "gc_unpin_raw",
        stage: IntrinsicStage::Mir,
        kind: IntrinsicKind::Runtime("scoop_rt_unpin"),
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::UNSAFE,
    },
    IntrinsicSpec {
        name: "gc_get_handle_raw",
        stage: IntrinsicStage::Mir,
        kind: IntrinsicKind::Runtime("scoop_rt_get_handle"),
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::UNSAFE,
    },
    IntrinsicSpec {
        name: "gc_release_handle_raw",
        stage: IntrinsicStage::Mir,
        kind: IntrinsicKind::Runtime("scoop_rt_release_handle"),
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::UNSAFE,
    },
    IntrinsicSpec {
        name: "rt_gc_collect",
        stage: IntrinsicStage::Mir,
        kind: IntrinsicKind::Runtime("scoop_rt_gc_collect"),
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::NONE,
    },
    IntrinsicSpec {
        name: "rt_gc_stats",
        stage: IntrinsicStage::Mir,
        kind: IntrinsicKind::Runtime("scoop_rt_gc_stats"),
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::NONE,
    },
    IntrinsicSpec {
        name: "coroutine_start",
        stage: IntrinsicStage::Mir,
        kind: IntrinsicKind::CoroutineStart,
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::NONE,
    },
    IntrinsicSpec {
        name: "coroutine_suspend",
        stage: IntrinsicStage::Mir,
        kind: IntrinsicKind::CoroutineSuspend,
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::NONE,
    },
    IntrinsicSpec {
        name: "ptr_to_uint",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicKind::Pointer(PointerIntrinsic::ToUInt),
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NO_GC_UNSAFE,
    },
    IntrinsicSpec {
        name: "ptr_cast",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicKind::Pointer(PointerIntrinsic::Cast),
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NO_GC_UNSAFE,
    },
    IntrinsicSpec {
        name: "ptr_load",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicKind::Pointer(PointerIntrinsic::Load),
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NO_GC_UNSAFE,
    },
    IntrinsicSpec {
        name: "ptr_load_offset",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicKind::Pointer(PointerIntrinsic::LoadOffset),
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NO_GC_UNSAFE,
    },
    IntrinsicSpec {
        name: "ptr_store",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicKind::Pointer(PointerIntrinsic::Store),
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NO_GC_UNSAFE,
    },
    IntrinsicSpec {
        name: "ptr_store_offset",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicKind::Pointer(PointerIntrinsic::StoreOffset),
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NO_GC_UNSAFE,
    },
    IntrinsicSpec {
        name: "ptr_plus",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicKind::Pointer(PointerIntrinsic::Plus),
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NO_GC_UNSAFE,
    },
    IntrinsicSpec {
        name: "ptr_minus",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicKind::Pointer(PointerIntrinsic::Minus),
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NO_GC_UNSAFE,
    },
    IntrinsicSpec {
        name: "address_of",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicKind::Pointer(PointerIntrinsic::AddressOf),
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::UNSAFE,
    },
    IntrinsicSpec {
        name: "size_of",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicKind::Pointer(PointerIntrinsic::SizeOf),
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::NO_GC,
    },
    IntrinsicSpec {
        name: "align_of",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicKind::Pointer(PointerIntrinsic::AlignOf),
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::NO_GC,
    },
];

/// One entry of the intrinsic registry.
pub struct IntrinsicSpec {
    pub name: &'static str,
    pub stage: IntrinsicStage,
    pub kind: IntrinsicKind,
    pub target: IntrinsicTarget,
    pub effects: IntrinsicEffects,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntrinsicStage {
    Hir,
    Mir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntrinsicKind {
    Runtime(&'static str),
    CoroutineStart,
    CoroutineSuspend,
    Pointer(PointerIntrinsic),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerIntrinsic {
    ToUInt,
    Cast,
    Load,
    LoadOffset,
    Store,
    StoreOffset,
    Plus,
    Minus,
    AddressOf,
    SizeOf,
    AlignOf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntrinsicTarget {
    TopLevel,
    Member,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntrinsicEffects {
    pub no_gc: bool,
    pub unsafe_: bool,
}

impl IntrinsicEffects {
    pub const NONE: Self = Self {
        no_gc: false,
        unsafe_: false,
    };
    pub const NO_GC: Self = Self {
        no_gc: true,
        unsafe_: false,
    };
    pub const UNSAFE: Self = Self {
        no_gc: false,
        unsafe_: true,
    };
    pub const NO_GC_UNSAFE: Self = Self {
        no_gc: true,
        unsafe_: true,
    };
}

pub fn intrinsic_spec(name: &str) -> Option<&'static IntrinsicSpec> {
    INTRINSIC_REGISTRY.iter().find(|spec| spec.name == name)
}

/// Indented text dump for golden tests (`scoopc build --emit=hir`).
pub fn dump(module: &Module) -> String {
    let mut out = String::from("Module\n");
    for (id, decl) in module.structs.iter() {
        if id == module.ffi_core.ptr
            || id == module.ffi_core.fun_ptr
            || id == module.ffi_core.pinned_ptr
            || id == module.ffi_core.gc_handle
        {
            continue;
        }
        let type_params = if decl.type_params.is_empty() {
            String::new()
        } else {
            dump_type_params(&decl.type_params)
        };
        let interfaces = dump_interface_list(module, &decl.interfaces);
        let attributes = dump_struct_attributes(decl.attributes);
        out.push_str(&format!(
            "  struct {}{}{}{}\n",
            decl.name, type_params, interfaces, attributes
        ));
        for field in &decl.fields {
            out.push_str(&format!(
                "    field {}: {}\n",
                field.name,
                type_name(module, field.ty)
            ));
        }
    }
    for (_, decl) in module.enums.iter() {
        let type_params = if decl.type_params.is_empty() {
            String::new()
        } else {
            dump_type_params(&decl.type_params)
        };
        let interfaces = dump_interface_list(module, &decl.interfaces);
        out.push_str(&format!(
            "  enum {}{}{}\n",
            decl.name, type_params, interfaces
        ));
        for variant in &decl.variants {
            let fields: Vec<String> = variant
                .fields
                .iter()
                .map(|f| format!("{}: {}", f.name, type_name(module, f.ty)))
                .collect();
            out.push_str(&format!("    {}({})\n", variant.name, fields.join(", ")));
        }
    }
    for (_, decl) in module.classes.iter() {
        let modifier = match decl.modifier {
            ClassModifier::Final => "",
            ClassModifier::Open => "open ",
            ClassModifier::Abstract => "abstract ",
        };
        let ctor: Vec<String> = decl
            .constructor
            .iter()
            .map(|f| format!("{}: {}", f.name, type_name(module, f.ty)))
            .collect();
        let interfaces = dump_interface_list(module, &decl.interfaces);
        out.push_str(&format!(
            "  {modifier}class {}({}){}\n",
            decl.name,
            ctor.join(", "),
            interfaces
        ));
    }
    for (_, decl) in module.interfaces.iter() {
        let type_params = if decl.type_params.is_empty() {
            String::new()
        } else {
            dump_type_params(&decl.type_params)
        };
        out.push_str(&format!("  interface {}{}\n", decl.name, type_params));
        for method in &decl.methods {
            let method_type_params = if method.type_params.is_empty() {
                String::new()
            } else {
                dump_type_params(&method.type_params)
            };
            let params: Vec<String> = method
                .params
                .iter()
                .map(|param| format!("{}: {}", param.name, type_name(module, param.ty)))
                .collect();
            out.push_str(&format!(
                "    {}fun {}{}({}): {}{}\n",
                if method.is_suspend { "suspend " } else { "" },
                method.name,
                method_type_params,
                params.join(", "),
                type_name(module, method.return_ty),
                dump_function_attributes(method.attributes)
            ));
        }
    }
    for (id, global) in module.globals.iter() {
        let storage = match &global.storage {
            GlobalStorage::Local {
                thread_local: false,
                ..
            } => "global".to_string(),
            GlobalStorage::Local {
                thread_local: true, ..
            } => "thread_local".to_string(),
            GlobalStorage::Extern {
                native_symbol,
                library,
                thread_local,
            } => format!(
                "extern symbol={native_symbol}{}{}",
                if library.is_empty() {
                    String::new()
                } else {
                    format!(" lib={library}")
                },
                if *thread_local { " thread_local" } else { "" }
            ),
        };
        out.push_str(&format!(
            "  {} {}: {} <global{} {storage}>\n",
            if global.mutable { "var" } else { "val" },
            global.name,
            type_name(module, global.ty),
            id.into_raw()
        ));
    }
    for &id in &module.top_level {
        if [
            module.ffi_core.address_of,
            module.ffi_core.size_of,
            module.ffi_core.align_of,
            module.ffi_core.gc_pin_raw,
            module.ffi_core.gc_unpin_raw,
            module.ffi_core.gc_get_handle_raw,
            module.ffi_core.gc_release_handle_raw,
        ]
        .contains(&id)
        {
            continue;
        }
        let function = &module.functions[id];
        let type_params = if function.type_params.is_empty() {
            String::new()
        } else {
            dump_type_params(&function.type_params)
        };
        let params: Vec<String> = match function.kind {
            FunctionKind::Extern(id) => module.extern_functions[id]
                .params
                .iter()
                .enumerate()
                .map(|(index, &ty)| format!("arg{}: {}", index + 1, type_name(module, ty)))
                .collect(),
            _ => function
                .params
                .iter()
                .map(|p| format!("{}: {}", p.name, type_name(module, p.ty)))
                .collect(),
        };
        let signature = format!(
            "{}{}({}): {}",
            function.name,
            type_params,
            params.join(", "),
            type_name(module, function.return_ty)
        );
        let suspend = if function.is_suspend { "suspend " } else { "" };
        let attributes = dump_function_attributes(function.attributes);
        match &function.kind {
            FunctionKind::Intrinsic(name) => {
                out.push_str(&format!(
                    "  {suspend}fun {signature}{attributes} <intrinsic {name}>\n"
                ));
            }
            FunctionKind::User(body) => {
                out.push_str(&format!("  {suspend}fun {signature}{attributes}\n"));
                dump_statements(module, &body.locals, &body.statements, 2, &mut out);
            }
            FunctionKind::Extern(id) => {
                let extern_ = &module.extern_functions[*id];
                let abi = match extern_.abi {
                    ExternAbi::C => "c",
                    ExternAbi::Scoop => "scoop",
                };
                let library = if extern_.library.is_empty() {
                    String::new()
                } else {
                    format!(" lib={}", extern_.library)
                };
                out.push_str(&format!(
                    "  fun {signature}{attributes} <extern{} abi={abi} symbol={}{}>\n",
                    id.into_raw(),
                    extern_.native_symbol,
                    library
                ));
            }
        }
    }
    out.push_str(&format!(
        "  entry {}\n",
        module.functions[module.entry].name
    ));
    for (_, instantiation) in module.instantiations.iter() {
        let function = module.generic_functions[instantiation.generic].function;
        if [
            module.ffi_core.gc_pin_raw,
            module.ffi_core.gc_unpin_raw,
            module.ffi_core.gc_get_handle_raw,
            module.ffi_core.gc_release_handle_raw,
        ]
        .contains(&function)
        {
            continue;
        }
        let args: Vec<String> = instantiation
            .type_args
            .iter()
            .map(|t| type_name(module, *t))
            .collect();
        out.push_str(&format!(
            "  instance {}<{}>\n",
            module.functions[function].name,
            args.join(", ")
        ));
    }
    out
}

fn dump_type_params(params: &[TypeParamDecl]) -> String {
    let params = params
        .iter()
        .map(|param| {
            let variance = match param.variance {
                Variance::Invariant => "",
                Variance::In => "in ",
                Variance::Out => "out ",
            };
            let kind = match param.kind {
                TypeParamKind::Any => "",
                TypeParamKind::Value => " : value",
                TypeParamKind::Ref => " : ref",
            };
            format!("{variance}{}{kind}", param.name)
        })
        .collect::<Vec<_>>();
    format!("<{}>", params.join(", "))
}

fn dump_function_attributes(attributes: FunctionAttributes) -> String {
    let mut values = Vec::new();
    if attributes.safety == Safety::Unsafe {
        values.push("unsafe");
    }
    if attributes.gc_effect == GcEffect::NoGc {
        values.push("no-gc");
        values.push(match attributes.calling_convention {
            CallingConvention::Cdecl => "cdecl",
        });
    }
    if values.is_empty() {
        String::new()
    } else {
        format!(" <{}>", values.join(" "))
    }
}

fn dump_struct_attributes(attributes: StructAttributes) -> String {
    let mut values = Vec::new();
    if let Some(layout) = attributes.c_layout {
        values.push(format!(
            "c-layout aligned={} packed={}",
            layout.aligned, layout.packed
        ));
    }
    if attributes.interior_mutable {
        values.push("interior-mutable".to_string());
    }
    if values.is_empty() {
        String::new()
    } else {
        format!(" <{}>", values.join(" "))
    }
}

fn dump_interface_list(module: &Module, interfaces: &[TypeId]) -> String {
    if interfaces.is_empty() {
        String::new()
    } else {
        let names: Vec<String> = interfaces.iter().map(|&ty| type_name(module, ty)).collect();
        format!(" : {}", names.join(", "))
    }
}

fn dump_statements(
    module: &Module,
    locals: &Arena<Local>,
    statements: &[Statement],
    indent: usize,
    out: &mut String,
) {
    for statement in statements {
        let pad = "  ".repeat(indent);
        match &statement.kind {
            StatementKind::Expr(expr) => dump_expr(module, locals, expr, indent, out),
            StatementKind::LocalFunction(id) => {
                let local = &module.local_functions[*id];
                out.push_str(&format!(
                    "{pad}LocalFunction local{} body={} captures={}\n",
                    id.into_raw(),
                    module.functions[local.function].name,
                    local.captures.len()
                ));
            }
            StatementKind::Return { value } => {
                out.push_str(&format!("{pad}return\n"));
                if let Some(value) = value {
                    dump_expr(module, locals, value, indent + 1, out);
                }
            }
            StatementKind::ValDecl { pattern, init } => {
                out.push_str(&format!("{pad}val {}\n", dump_pattern(pattern)));
                dump_expr(module, locals, init, indent + 1, out);
            }
            StatementKind::Assign { target, value } => {
                match target {
                    AssignTarget::Local(local) => {
                        out.push_str(&format!("{pad}assign {}\n", locals[*local].name))
                    }
                    AssignTarget::Global(global) => out.push_str(&format!(
                        "{pad}assign global {}\n",
                        module.globals[*global].name
                    )),
                    AssignTarget::Field { receiver, .. } => {
                        out.push_str(&format!("{pad}assign .field\n"));
                        dump_expr(module, locals, receiver, indent + 1, out);
                    }
                    AssignTarget::Index { array, index } => {
                        out.push_str(&format!("{pad}assign []\n"));
                        dump_expr(module, locals, array, indent + 1, out);
                        dump_expr(module, locals, index, indent + 1, out);
                    }
                }
                dump_expr(module, locals, value, indent + 1, out);
            }
            StatementKind::If {
                cond,
                then_body,
                else_body,
            } => {
                out.push_str(&format!("{pad}if\n"));
                dump_expr(module, locals, cond, indent + 1, out);
                dump_statements(module, locals, then_body, indent + 1, out);
                if let Some(else_body) = else_body {
                    out.push_str(&format!("{pad}else\n"));
                    dump_statements(module, locals, else_body, indent + 1, out);
                }
            }
            StatementKind::While { cond, body } => {
                out.push_str(&format!("{pad}while\n"));
                dump_expr(module, locals, cond, indent + 1, out);
                dump_statements(module, locals, body, indent + 1, out);
            }
            StatementKind::Try(try_) => {
                out.push_str(&format!("{pad}try\n"));
                dump_statements(module, locals, &try_.body, indent + 1, out);
                for catch in &try_.catches {
                    out.push_str(&format!(
                        "{pad}catch {}: {}\n",
                        locals[catch.local].name,
                        type_name(module, catch.ty)
                    ));
                    dump_statements(module, locals, &catch.body, indent + 1, out);
                }
                if let Some(finally_body) = &try_.finally_body {
                    out.push_str(&format!("{pad}finally\n"));
                    dump_statements(module, locals, finally_body, indent + 1, out);
                }
            }
            StatementKind::Throw(expr) => {
                out.push_str(&format!("{pad}throw\n"));
                dump_expr(module, locals, expr, indent + 1, out);
            }
            StatementKind::When(when) => {
                out.push_str(&format!("{pad}when\n"));
                dump_expr(module, locals, &when.subject, indent + 1, out);
                for arm in &when.arms {
                    out.push_str(&format!(
                        "{}  arm {}{}\n",
                        pad,
                        dump_pattern(&arm.pattern),
                        if arm.guard.is_some() {
                            " if <guard>"
                        } else {
                            ""
                        }
                    ));
                    dump_statements(module, locals, &arm.body, indent + 2, out);
                }
                if let Some(else_body) = &when.else_body {
                    out.push_str(&format!("{pad}  else\n"));
                    dump_statements(module, locals, else_body, indent + 2, out);
                }
            }
        }
    }
}

/// Compact one-line pattern rendering for dumps.
pub fn dump_pattern(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Binding { local } => format!("local{}", local.into_raw()),
        Pattern::Wildcard => "_".to_string(),
        Pattern::Literal(expr) => format!("<lit {:?}>", expr.kind).chars().take(40).collect(),
        Pattern::Variant {
            variant, fields, ..
        } => {
            let fields: Vec<String> = fields
                .iter()
                .map(|(i, p)| format!("{i}: {}", dump_pattern(p)))
                .collect();
            format!("variant{}({})", variant, fields.join(", "))
        }
        Pattern::Tuple(elements) => {
            let parts: Vec<String> = elements.iter().map(dump_pattern).collect();
            format!("({})", parts.join(", "))
        }
        Pattern::Struct { fields, .. } => {
            let fields: Vec<String> = fields
                .iter()
                .map(|(i, p)| format!("{i}: {}", dump_pattern(p)))
                .collect();
            format!("struct({})", fields.join(", "))
        }
    }
}

fn callable_parts(module: &Module, callable: Callable) -> (FunctionId, Option<&[TypeId]>) {
    match callable {
        Callable::Function(function) => (function, None),
        Callable::Generic(id) => {
            let resolved = &module.instantiations[id];
            (
                module.generic_functions[resolved.generic].function,
                Some(&resolved.type_args),
            )
        }
    }
}

fn dump_expr(module: &Module, locals: &Arena<Local>, expr: &Expr, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    let ty = type_name(module, expr.ty);
    match &expr.kind {
        ExprKind::StringLiteral(value) => {
            out.push_str(&format!("{pad}StringLiteral {value:?} : {ty}\n"));
        }
        ExprKind::IntLiteral(value) => out.push_str(&format!("{pad}IntLiteral {value} : {ty}\n")),
        ExprKind::BoolLiteral(value) => out.push_str(&format!("{pad}BoolLiteral {value} : {ty}\n")),
        ExprKind::UnitLiteral => out.push_str(&format!("{pad}UnitLiteral : {ty}\n")),
        ExprKind::TupleLiteral(elements) => {
            out.push_str(&format!("{pad}TupleLiteral : {ty}\n"));
            for element in elements {
                dump_expr(module, locals, element, indent + 1, out);
            }
        }
        ExprKind::ClassInit { class_id, args } => {
            out.push_str(&format!(
                "{pad}ClassInit {} : {ty}\n",
                module.classes[*class_id].name
            ));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::StructInit { struct_id, args } => {
            out.push_str(&format!(
                "{pad}StructInit {} : {ty}\n",
                module.structs[*struct_id].name
            ));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::VariantConstruct {
            enum_id,
            variant,
            type_args,
            args,
        } => {
            let decl = &module.enums[*enum_id];
            let type_args = if type_args.is_empty() {
                String::new()
            } else {
                let args: Vec<String> = type_args.iter().map(|t| type_name(module, *t)).collect();
                format!("<{}>", args.join(", "))
            };
            out.push_str(&format!(
                "{pad}VariantConstruct {}.{}{type_args} : {ty}\n",
                decl.name, decl.variants[*variant as usize].name
            ));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::Local(local) => {
            out.push_str(&format!("{pad}Local {} : {ty}\n", locals[*local].name));
        }
        ExprKind::GlobalRead(global) => out.push_str(&format!(
            "{pad}GlobalRead {} : {ty}\n",
            module.globals[*global].name
        )),
        ExprKind::Capture(binding) => {
            out.push_str(&format!(
                "{pad}Capture binding{} : {ty}\n",
                binding.into_raw()
            ));
        }
        ExprKind::Lambda(id) => {
            let lambda = &module.lambdas[*id];
            out.push_str(&format!(
                "{pad}Lambda lambda{} invoke={} captures={} : {ty}\n",
                id.into_raw(),
                module.functions[lambda.function].name,
                lambda.captures.len()
            ));
            for capture in &lambda.captures {
                out.push_str(&format!(
                    "{}capture {} binding{} : {}\n",
                    "  ".repeat(indent + 1),
                    capture.name,
                    capture.binding.into_raw(),
                    type_name(module, capture.ty)
                ));
            }
        }
        ExprKind::AnonymousFunction(id) => {
            let anonymous = &module.anonymous_functions[*id];
            out.push_str(&format!(
                "{pad}AnonymousFunction anonymous{} invoke={} captures={} : {ty}\n",
                id.into_raw(),
                module.functions[anonymous.function].name,
                anonymous.captures.len()
            ));
            for capture in &anonymous.captures {
                out.push_str(&format!(
                    "{}capture {} binding{} : {}\n",
                    "  ".repeat(indent + 1),
                    capture.name,
                    capture.binding.into_raw(),
                    type_name(module, capture.ty)
                ));
            }
        }
        ExprKind::CallableReference(id) => {
            let reference = &module.callable_references[*id];
            let (callable, receiver) = match &reference.target {
                CallableReferenceTarget::Named(callable) => (*callable, None),
                CallableReferenceTarget::Local { callee, .. } => (*callee, None),
                CallableReferenceTarget::BoundMember { receiver, callee } => {
                    (*callee, Some(receiver.as_ref()))
                }
                CallableReferenceTarget::BoundExtension { receiver, callee } => {
                    (*callee, Some(receiver.as_ref()))
                }
            };
            let (function, _) = callable_parts(module, callable);
            out.push_str(&format!(
                "{pad}CallableReference reference{} target={} captures={} : {ty}\n",
                id.into_raw(),
                module.functions[function].name,
                reference.captures.len()
            ));
            if let Some(receiver) = receiver {
                dump_expr(module, locals, receiver, indent + 1, out);
            }
        }
        ExprKind::FunctionCoercion {
            source,
            coercion,
            target_type,
        } => {
            let conversion = &module.function_coercions[*coercion];
            debug_assert_eq!(conversion.target, *target_type);
            out.push_str(&format!(
                "{pad}FunctionCoercion coercion{} {} -> {} : {ty}\n",
                coercion.into_raw().into_u32(),
                type_name(
                    module,
                    module
                        .types
                        .iter()
                        .find_map(|(ty, value)| {
                            matches!(value, Type::Function(id) if *id == conversion.source)
                                .then_some(ty)
                        })
                        .expect("a function signature has a canonical type")
                ),
                type_name(
                    module,
                    module
                        .types
                        .iter()
                        .find_map(|(ty, value)| {
                            matches!(value, Type::Function(id) if *id == conversion.target)
                                .then_some(ty)
                        })
                        .expect("a function signature has a canonical type")
                )
            ));
            dump_expr(module, locals, source, indent + 1, out);
        }
        ExprKind::PtrFromUInt(operand) => {
            out.push_str(&format!("{pad}PtrFromUInt : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::PtrToUInt(operand) => {
            out.push_str(&format!("{pad}PtrToUInt : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::PtrCast(operand) => {
            out.push_str(&format!("{pad}PtrCast : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::PtrLoad { pointer, offset } => {
            out.push_str(&format!("{pad}PtrLoad : {ty}\n"));
            dump_expr(module, locals, pointer, indent + 1, out);
            if let Some(offset) = offset {
                dump_expr(module, locals, offset, indent + 1, out);
            }
        }
        ExprKind::PtrStore {
            pointer,
            offset,
            value,
        } => {
            out.push_str(&format!("{pad}PtrStore : {ty}\n"));
            dump_expr(module, locals, pointer, indent + 1, out);
            if let Some(offset) = offset {
                dump_expr(module, locals, offset, indent + 1, out);
            }
            dump_expr(module, locals, value, indent + 1, out);
        }
        ExprKind::PtrOffset {
            pointer,
            offset,
            subtract,
        } => {
            out.push_str(&format!("{pad}PtrOffset subtract={subtract} : {ty}\n"));
            dump_expr(module, locals, pointer, indent + 1, out);
            dump_expr(module, locals, offset, indent + 1, out);
        }
        ExprKind::AddressOf(Place::Local(local)) => {
            out.push_str(&format!("{pad}AddressOf {} : {ty}\n", locals[*local].name));
        }
        ExprKind::AddressOf(Place::Global(global)) => out.push_str(&format!(
            "{pad}AddressOf global {} : {ty}\n",
            module.globals[*global].name
        )),
        ExprKind::SizeOf(value_ty) => out.push_str(&format!(
            "{pad}SizeOf {} : {ty}\n",
            type_name(module, *value_ty)
        )),
        ExprKind::AlignOf(value_ty) => out.push_str(&format!(
            "{pad}AlignOf {} : {ty}\n",
            type_name(module, *value_ty)
        )),
        ExprKind::FunPtrNull => out.push_str(&format!("{pad}FunPtrNull : {ty}\n")),
        ExprKind::FunctionAddress(function) => out.push_str(&format!(
            "{pad}FunctionAddress {} : {ty}\n",
            module.functions[*function].name
        )),
        ExprKind::FieldAccess { receiver, field } => {
            let field = match field {
                FieldRef::StructField { index, .. } => format!("field {index}"),
                FieldRef::TupleIndex(index) => format!("_{}", index + 1),
                FieldRef::ClassField { index, .. } => format!("class field {index}"),
            };
            out.push_str(&format!("{pad}FieldAccess {field} : {ty}\n"));
            dump_expr(module, locals, receiver, indent + 1, out);
        }
        ExprKind::Call { callee, args } => {
            let (function, type_args) = callable_parts(module, *callee);
            let callee = &module.functions[function];
            let type_args = type_args.map_or_else(String::new, |type_args| {
                let args: Vec<String> = type_args.iter().map(|t| type_name(module, *t)).collect();
                format!("<{}>", args.join(", "))
            });
            out.push_str(&format!("{pad}Call {}{type_args} : {ty}\n", callee.name));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::LocalFunctionCall {
            local_function,
            callee,
            captures,
            args,
        } => {
            let (function, type_args) = callable_parts(module, *callee);
            let type_args = type_args.map_or_else(String::new, |type_args| {
                let args: Vec<String> = type_args.iter().map(|t| type_name(module, *t)).collect();
                format!("<{}>", args.join(", "))
            });
            out.push_str(&format!(
                "{pad}LocalFunctionCall local{} {}{type_args} captures={} : {ty}\n",
                local_function.into_raw(),
                module.functions[function].name,
                captures.len()
            ));
            for capture in captures {
                dump_expr(module, locals, capture, indent + 1, out);
            }
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::CallableCall {
            callee,
            function_type,
            args,
        } => {
            out.push_str(&format!(
                "{pad}CallableCall function_type{} : {ty}\n",
                function_type.into_raw()
            ));
            dump_expr(module, locals, callee, indent + 1, out);
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::Binary { op, lhs, rhs } => {
            out.push_str(&format!("{pad}Binary {op:?} : {ty}\n"));
            dump_expr(module, locals, lhs, indent + 1, out);
            dump_expr(module, locals, rhs, indent + 1, out);
        }
        ExprKind::Unary { op, operand } => {
            out.push_str(&format!("{pad}Unary {op:?} : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::MethodCall {
            receiver,
            callee,
            args,
        } => {
            let (function, _) = callable_parts(module, *callee);
            out.push_str(&format!(
                "{pad}MethodCall {} : {ty}\n",
                module.functions[function].name
            ));
            dump_expr(module, locals, receiver, indent + 1, out);
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::Box(operand) => {
            out.push_str(&format!("{pad}Box : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::Unbox(operand) => {
            out.push_str(&format!("{pad}Unbox : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::IsInstance { operand, check_ty } => {
            out.push_str(&format!(
                "{pad}IsInstance {} : {ty}\n",
                type_name(module, *check_ty)
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::Cast { operand, optional } => {
            out.push_str(&format!("{pad}Cast optional={optional} : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::ArrayLiteral(elements) => {
            out.push_str(&format!("{pad}ArrayLiteral : {ty}\n"));
            for element in elements {
                dump_expr(module, locals, element, indent + 1, out);
            }
        }
        ExprKind::Index { receiver, index } => {
            out.push_str(&format!("{pad}Index : {ty}\n"));
            dump_expr(module, locals, receiver, indent + 1, out);
            dump_expr(module, locals, index, indent + 1, out);
        }
        ExprKind::ArrayLen(operand) => {
            out.push_str(&format!("{pad}ArrayLen : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::ArrayClone(operand) => {
            out.push_str(&format!("{pad}ArrayClone : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::SomeWrap(operand) => {
            out.push_str(&format!("{pad}SomeWrap : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::NoneLiteral => out.push_str(&format!("{pad}NoneLiteral : {ty}\n")),
        ExprKind::IsSome(operand) => {
            out.push_str(&format!("{pad}IsSome : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::Unwrap {
            operand,
            trap_on_none,
        } => {
            out.push_str(&format!("{pad}Unwrap trap={trap_on_none} : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
    }
}
