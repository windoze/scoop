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
pub type FunctionId = Idx<Function>;
pub type GenericFunctionId = Idx<GenericFunction>;
pub type ResolvedGenericFunctionId = Idx<ResolvedGenericFunction>;
pub type StructId = Idx<StructDecl>;
pub type EnumId = Idx<EnumDecl>;
pub type ClassId = Idx<ClassDecl>;
pub type InterfaceId = Idx<InterfaceDecl>;
pub type LocalId = Idx<Local>;

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
    /// An interface type (spec 9.1); values behind it are references.
    Interface(InterfaceId),
    /// The root of all types (spec 3.1). Value types reaching it are
    /// boxed (spec 4.4.4).
    Any,
    /// Compiler-built-in array types (M5; class declarations arrive
    /// with M7, see docs/milestone5/DESIGN.md 5.1). Invariant in the
    /// element type (spec 10.4).
    Array(TypeId),
    MutableArray(TypeId),
    Tuple(Vec<TypeId>),
    /// An enum type with resolved type arguments (empty for
    /// non-generic enums). `Option<T>` is one of these since M4
    /// (defined in `scoop.core`).
    Enum(EnumId, Vec<TypeId>),
    /// A type parameter, by typed local index. Only appears inside a
    /// generic function/type definition; instantiated MIR never
    /// contains it.
    Param(TypeParamId),
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
        (Type::Interface(x), Type::Interface(y)) => *x == *y,
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
        Type::Interface(id) => module.interfaces[*id].name.clone(),
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
        Type::Param(index) => format!("T{}", index.into_raw()),
    }
}

#[derive(Debug)]
pub struct Module {
    pub types: Arena<Type>,
    pub functions: Arena<Function>,
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
    /// Entry point: `fun main()`. Guaranteed present.
    pub entry: FunctionId,
    /// Resolved generic function applications, deduplicated in
    /// first-use order. The arena id is carried directly by call
    /// expressions and is the instantiation request consumed by MIR.
    pub instantiations: Arena<ResolvedGenericFunction>,
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

#[derive(Debug)]
pub struct StructDecl {
    pub name: String,
    pub type_params: Vec<String>,
    pub fields: Vec<Field>,
    pub interfaces: Vec<InterfaceId>,
    pub span: Span,
}

#[derive(Debug)]
pub struct EnumDecl {
    pub name: String,
    pub type_params: Vec<String>,
    pub variants: Vec<Variant>,
    pub interfaces: Vec<InterfaceId>,
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
}

#[derive(Debug)]
pub struct ClassDecl {
    pub modifier: ClassModifier,
    pub name: String,
    /// Primary-constructor properties in declaration order.
    pub constructor: Vec<Field>,
    /// Base class and the resolved constructor argument expressions.
    pub base_class: Option<(ClassId, Vec<Expr>)>,
    pub interfaces: Vec<InterfaceId>,
    pub span: Span,
}

#[derive(Debug)]
pub struct InterfaceDecl {
    pub name: String,
    pub methods: Vec<MethodSig>,
    pub span: Span,
}

/// An interface method signature (M6: no body, no properties).
#[derive(Debug)]
pub struct MethodSig {
    pub name: String,
    pub params: Vec<Param>,
    pub return_ty: TypeId,
    pub span: Span,
}

#[derive(Debug)]
pub struct Variant {
    pub name: String,
    /// Fields in declaration order; unit variants have none. Named and
    /// constructor-style fields carry their names (and defaults),
    /// positional fields have generated `_1`-style names.
    pub fields: Vec<Field>,
    /// Constructor-style default values (constant expressions in M4).
    pub defaults: Vec<Option<Expr>>,
}

#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub ty: TypeId,
}

#[derive(Debug)]
pub struct Function {
    pub name: String,
    /// Generic type parameter names; empty for non-generic functions.
    pub type_params: Vec<String>,
    pub params: Vec<Param>,
    pub return_ty: TypeId,
    pub kind: FunctionKind,
    /// Member metadata; the receiver of a method is the first entry of
    /// `params` (named `this`). Top-level functions have `None`.
    pub method: Option<Method>,
    pub span: Span,
}

#[derive(Debug)]
pub struct Param {
    pub name: String,
    pub ty: TypeId,
    /// Parameters are (immutable) locals.
    pub local: LocalId,
}

#[derive(Debug)]
pub enum FunctionKind {
    User(Body),
    /// A `@Intrinsic("name")` function (spec 13.1); the name is
    /// guaranteed to be in the compiler's intrinsic registry.
    Intrinsic(String),
}

#[derive(Debug)]
pub struct Body {
    pub locals: Arena<Local>,
    pub statements: Vec<Statement>,
}

#[derive(Debug)]
pub struct Local {
    pub name: String,
    pub ty: TypeId,
    pub mutable: bool,
}

#[derive(Debug)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug)]
pub enum StatementKind {
    Expr(Expr),
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

#[derive(Debug)]
pub struct Try {
    pub body: Vec<Statement>,
    pub catches: Vec<CatchClause>,
    pub finally_body: Option<Vec<Statement>>,
}

#[derive(Debug)]
pub struct CatchClause {
    pub local: LocalId,
    pub ty: TypeId,
    pub body: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug)]
pub enum AssignTarget {
    Local(LocalId),
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

#[derive(Debug)]
pub struct When {
    pub subject: Expr,
    pub arms: Vec<WhenArm>,
    pub else_body: Option<Vec<Statement>>,
}

#[derive(Debug)]
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
#[derive(Debug)]
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

#[derive(Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: TypeId,
    pub span: Span,
}

#[derive(Debug)]
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

/// The compiler's intrinsic registry (impl spec 2.10, M4 slice):
/// signature rule + runtime symbol mapping live with the lowerers;
/// this table is the single source of truth for valid names.
pub const INTRINSIC_REGISTRY: &[IntrinsicSpec] = &[
    IntrinsicSpec {
        name: "rt_write",
        symbol: "scoop_rt_print",
    },
    IntrinsicSpec {
        name: "rt_pin",
        symbol: "scoop_rt_pin",
    },
    IntrinsicSpec {
        name: "rt_unpin",
        symbol: "scoop_rt_unpin",
    },
    IntrinsicSpec {
        name: "rt_get_handle",
        symbol: "scoop_rt_get_handle",
    },
    IntrinsicSpec {
        name: "rt_release_handle",
        symbol: "scoop_rt_release_handle",
    },
    IntrinsicSpec {
        name: "rt_gc_collect",
        symbol: "scoop_rt_gc_collect",
    },
    IntrinsicSpec {
        name: "rt_gc_stats",
        symbol: "scoop_rt_gc_stats",
    },
];

/// One entry of the intrinsic registry.
pub struct IntrinsicSpec {
    pub name: &'static str,
    pub symbol: &'static str,
}

/// Indented text dump for golden tests (`scoopc build --emit=hir`).
pub fn dump(module: &Module) -> String {
    let mut out = String::from("Module\n");
    for (_, decl) in module.structs.iter() {
        let type_params = if decl.type_params.is_empty() {
            String::new()
        } else {
            format!("<{}>", decl.type_params.join(", "))
        };
        out.push_str(&format!("  struct {}{}\n", decl.name, type_params));
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
            format!("<{}>", decl.type_params.join(", "))
        };
        out.push_str(&format!("  enum {}{}\n", decl.name, type_params));
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
        out.push_str(&format!(
            "  {modifier}class {}({})\n",
            decl.name,
            ctor.join(", ")
        ));
    }
    for (_, decl) in module.interfaces.iter() {
        out.push_str(&format!("  interface {}\n", decl.name));
        for method in &decl.methods {
            out.push_str(&format!(
                "    fun {}(..): {}\n",
                method.name,
                type_name(module, method.return_ty)
            ));
        }
    }
    for &id in &module.top_level {
        let function = &module.functions[id];
        let type_params = if function.type_params.is_empty() {
            String::new()
        } else {
            format!("<{}>", function.type_params.join(", "))
        };
        let params: Vec<String> = function
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, type_name(module, p.ty)))
            .collect();
        let signature = format!(
            "{}{}({}): {}",
            function.name,
            type_params,
            params.join(", "),
            type_name(module, function.return_ty)
        );
        match &function.kind {
            FunctionKind::Intrinsic(name) => {
                out.push_str(&format!("  fun {signature} <intrinsic {name}>\n"));
            }
            FunctionKind::User(body) => {
                out.push_str(&format!("  fun {signature}\n"));
                dump_statements(module, &body.locals, &body.statements, 2, &mut out);
            }
        }
    }
    out.push_str(&format!(
        "  entry {}\n",
        module.functions[module.entry].name
    ));
    for (_, instantiation) in module.instantiations.iter() {
        let function = module.generic_functions[instantiation.generic].function;
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
