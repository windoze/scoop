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
pub type StructId = Idx<StructDecl>;
pub type EnumId = Idx<EnumDecl>;
pub type LocalId = Idx<Local>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Unit,
    Int,
    Boolean,
    String,
    Struct(StructId),
    Tuple(Vec<TypeId>),
    /// An enum type with resolved type arguments (empty for
    /// non-generic enums). `Option<T>` is one of these since M4
    /// (defined in `scoop.core`).
    Enum(EnumId, Vec<TypeId>),
    /// A function type parameter, by index into
    /// `Function::type_params`. Only appears inside generic function
    /// bodies; instantiated MIR never contains it.
    Param(u32),
}

/// Structural type equality (tuple types are compared by elements,
/// enum types by identity plus arguments).
pub fn types_equal(module: &Module, a: TypeId, b: TypeId) -> bool {
    match (&module.types[a], &module.types[b]) {
        (Type::Unit, Type::Unit)
        | (Type::Int, Type::Int)
        | (Type::Boolean, Type::Boolean)
        | (Type::String, Type::String) => true,
        (Type::Struct(x), Type::Struct(y)) => x == y,
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
        Type::Boolean => "Boolean".to_string(),
        Type::String => "String".to_string(),
        Type::Struct(id) => module.structs[*id].name.clone(),
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
        Type::Param(index) => format!("T{index}"),
    }
}

#[derive(Debug)]
pub struct Module {
    pub types: Arena<Type>,
    pub functions: Arena<Function>,
    pub structs: Arena<StructDecl>,
    pub enums: Arena<EnumDecl>,
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
    /// Instantiation requests: (generic function, resolved type
    /// arguments), deduplicated, including nested requests from
    /// generic function bodies (impl spec 2.2).
    pub instantiations: Vec<Instantiation>,
}

/// A monomorphization request produced by HIR and materialized by MIR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instantiation {
    pub function: FunctionId,
    pub type_args: Vec<TypeId>,
}

#[derive(Debug)]
pub struct StructDecl {
    pub name: String,
    pub fields: Vec<Field>,
    pub span: Span,
}

#[derive(Debug)]
pub struct EnumDecl {
    pub name: String,
    pub type_params: Vec<String>,
    pub variants: Vec<Variant>,
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
        local: LocalId,
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
    Call {
        function: FunctionId,
        /// Resolved type arguments; empty for non-generic callees.
        type_args: Vec<TypeId>,
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
pub const INTRINSIC_REGISTRY: &[&str] = &["rt_print", "rt_println"];

/// Indented text dump for golden tests (`scoopc build --emit=hir`).
pub fn dump(module: &Module) -> String {
    let mut out = String::from("Module\n");
    for (_, decl) in module.structs.iter() {
        out.push_str(&format!("  struct {}\n", decl.name));
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
    for instantiation in &module.instantiations {
        let args: Vec<String> = instantiation
            .type_args
            .iter()
            .map(|t| type_name(module, *t))
            .collect();
        out.push_str(&format!(
            "  instance {}<{}>\n",
            module.functions[instantiation.function].name,
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
            StatementKind::Assign { local, value } => {
                out.push_str(&format!("{pad}assign {}\n", locals[*local].name));
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
            };
            out.push_str(&format!("{pad}FieldAccess {field} : {ty}\n"));
            dump_expr(module, locals, receiver, indent + 1, out);
        }
        ExprKind::Call {
            function,
            type_args,
            args,
        } => {
            let callee = &module.functions[*function];
            let type_args = if type_args.is_empty() {
                String::new()
            } else {
                let args: Vec<String> = type_args.iter().map(|t| type_name(module, *t)).collect();
                format!("<{}>", args.join(", "))
            };
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
