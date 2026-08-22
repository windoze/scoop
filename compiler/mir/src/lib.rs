//! MIR definitions and MIR meta: the data channel between MIR and LIR.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.3 and
//! `docs/milestone2/DESIGN.md` section 2.3.
//!
//! M2 notes: structural equality on aggregates is already expanded by
//! mir-lower into primitive comparisons and runtime calls, so MIR
//! `BinOp` only contains primitive operations. Control flow stays
//! structured (if/while); basic blocks appear only in LIR.

use la_arena::{Arena, Idx};
use scoop_ast::Span;

pub type FunctionId = Idx<Function>;
pub type StringConstId = Idx<StringConst>;
pub type StructId = Idx<StructDef>;
pub type LocalId = Idx<Local>;

/// Mangled symbol of the program entry point (called by the C runtime).
pub const ENTRY_SYMBOL: &str = "scoop_main";

/// Mangle a user function name (entry point maps to `ENTRY_SYMBOL`).
pub fn mangle_function(name: &str, is_entry: bool) -> String {
    if is_entry {
        ENTRY_SYMBOL.to_string()
    } else {
        format!("scoop.{name}")
    }
}

/// Mangle a monomorphized instance: `scoop.<name>$<encoded type args>`.
pub fn mangle_instance(module: &Module, name: &str, type_args: &[Type]) -> String {
    let args: Vec<String> = type_args.iter().map(|t| encode_type(module, t)).collect();
    format!("scoop.{name}${}", args.join("_"))
}

/// Compact type encoding for mangling (e.g. `scoop.identity$I`).
pub fn encode_type(module: &Module, ty: &Type) -> String {
    match ty {
        Type::Unit => "U".to_string(),
        Type::Int => "I".to_string(),
        Type::Boolean => "B".to_string(),
        Type::String => "S".to_string(),
        Type::Struct(id) => module.structs[*id].name.clone(),
        Type::Tuple(elements) => {
            let inner: Vec<String> = elements.iter().map(|t| encode_type(module, t)).collect();
            format!("T{}X", inner.join("_"))
        }
        Type::Option(inner) => format!("O{}X", encode_type(module, inner)),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Unit,
    Int,
    Boolean,
    String,
    Struct(StructId),
    Tuple(Vec<Type>),
    /// Instantiated `Option<T>` (builtin until M4, see hir::Type).
    Option(Box<Type>),
}

#[derive(Debug)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug)]
pub struct Local {
    pub name: String,
    pub ty: Type,
    pub mutable: bool,
}

#[derive(Debug)]
pub struct Module {
    pub functions: Arena<Function>,
    /// User functions in declaration order (builtins have no MIR body).
    pub top_level: Vec<FunctionId>,
    pub strings: Arena<StringConst>,
    pub structs: Arena<StructDef>,
    pub entry: FunctionId,
    pub meta: MirMeta,
}

/// Per-Cone MIR metadata (impl spec 2.3). M2: still no class hierarchy,
/// hence no dispatch tables, but the structure exists.
#[derive(Debug, Default)]
pub struct MirMeta {
    pub dispatch_tables: Vec<DispatchTable>,
}

#[derive(Debug)]
pub struct DispatchTable {
    pub owner: FunctionId,
    pub entries: Vec<FunctionId>,
}

#[derive(Debug)]
pub struct StringConst {
    pub value: String,
    /// Mangled global symbol, e.g. `scoop.str.0`.
    pub symbol: String,
}

#[derive(Debug)]
pub struct Function {
    pub name: String,
    /// Mangled symbol; `scoop.<name>`, `scoop.<name>$<args>` for
    /// monomorphized instances, or `scoop_main` for the entry.
    pub symbol: String,
    pub params: Vec<Param>,
    pub return_ty: Type,
    pub body: Body,
}

#[derive(Debug)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    /// Parameters are (immutable) locals.
    pub local: LocalId,
}

#[derive(Debug)]
pub struct Body {
    pub locals: Arena<Local>,
    pub statements: Vec<Statement>,
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
        local: LocalId,
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
}

#[derive(Debug)]
pub enum Expr {
    StringConst(StringConstId),
    IntLiteral(i64),
    BoolLiteral(bool),
    UnitLiteral,
    TupleLiteral(Vec<Expr>),
    StructInit {
        struct_id: StructId,
        args: Vec<Expr>,
    },
    Local(LocalId),
    /// Field or element access; `index` is 0-based for both structs
    /// and tuples.
    FieldAccess {
        receiver: Box<Expr>,
        index: u32,
    },
    Call(Call),
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Unary {
        op: UnOp,
        operand: Box<Expr>,
    },
    /// `Some(value)`.
    SomeWrap(Box<Expr>),
    /// The `None` literal; the producing instruction carries the
    /// concrete `Option<T>` type at LIR.
    NoneLiteral,
    /// Test whether an `Option<T>` is `Some`.
    IsSome(Box<Expr>),
    /// Unwrap an `Option<T>`; `trap_on_none` comes from `!!`.
    Unwrap {
        operand: Box<Expr>,
        trap_on_none: bool,
    },
}

#[derive(Debug)]
pub struct Call {
    pub target: CallTarget,
    pub args: Vec<Expr>,
}

#[derive(Debug)]
pub struct CallTarget {
    pub kind: CallKind,
    /// Fully resolved callee.
    pub callee: Callee,
}

#[derive(Debug)]
pub enum CallKind {
    Direct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Callee {
    /// A user function defined in this Cone.
    User(FunctionId),
    /// A runtime function (see `RuntimeFn::symbol`).
    Runtime(RuntimeFn),
}

/// Runtime functions callable from generated code. The output shims
/// are temporary until M11 (docs/milestone1/DESIGN.md 5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeFn {
    PrintString,
    PrintlnString,
    PrintInt,
    PrintlnInt,
    PrintBoolean,
    PrintlnBoolean,
    StringConcat,
    StringEq,
}

impl RuntimeFn {
    pub fn symbol(self) -> &'static str {
        match self {
            RuntimeFn::PrintString => "scoop_rt_print",
            RuntimeFn::PrintlnString => "scoop_rt_println",
            RuntimeFn::PrintInt => "scoop_rt_print_int",
            RuntimeFn::PrintlnInt => "scoop_rt_println_int",
            RuntimeFn::PrintBoolean => "scoop_rt_print_boolean",
            RuntimeFn::PrintlnBoolean => "scoop_rt_println_boolean",
            RuntimeFn::StringConcat => "scoop_rt_string_concat",
            RuntimeFn::StringEq => "scoop_rt_string_eq",
        }
    }
}

/// Primitive operations only: aggregate equality has been expanded by
/// mir-lower (docs/milestone2/DESIGN.md 2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    IntAdd,
    IntSub,
    IntMul,
    IntDiv,
    IntLt,
    IntLe,
    IntGt,
    IntGe,
    IntEq,
    IntNe,
    BoolEq,
    BoolNe,
    /// Short-circuit boolean operators; LIR lowers them to branches.
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    IntNeg,
    BoolNot,
}

/// Indented text dump for golden tests (`scoopc build --emit=mir`).
pub fn dump(module: &Module) -> String {
    let mut out = String::from("Module\n");
    for (_, def) in module.structs.iter() {
        let fields: Vec<String> = def
            .fields
            .iter()
            .map(|f| format!("{}: {}", f.name, type_name(module, &f.ty)))
            .collect();
        out.push_str(&format!("  struct {} ({})\n", def.name, fields.join(", ")));
    }
    for &id in &module.top_level {
        let function = &module.functions[id];
        let params: Vec<String> = function
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, type_name(module, &p.ty)))
            .collect();
        out.push_str(&format!(
            "  fun {} @{}({}) -> {}\n",
            function.name,
            function.symbol,
            params.join(", "),
            type_name(module, &function.return_ty)
        ));
        dump_statements(
            module,
            &function.body.locals,
            &function.body.statements,
            2,
            &mut out,
        );
    }
    for (_, string) in module.strings.iter() {
        out.push_str(&format!("  str @{} {:?}\n", string.symbol, string.value));
    }
    out.push_str(&format!("  entry @{ENTRY_SYMBOL}\n"));
    out
}

/// Render a type for dumps.
pub fn type_name(module: &Module, ty: &Type) -> String {
    match ty {
        Type::Unit => "Unit".to_string(),
        Type::Int => "Int".to_string(),
        Type::Boolean => "Boolean".to_string(),
        Type::String => "String".to_string(),
        Type::Struct(id) => module.structs[*id].name.clone(),
        Type::Tuple(elements) => {
            let inner: Vec<String> = elements.iter().map(|t| type_name(module, t)).collect();
            format!("({})", inner.join(", "))
        }
        Type::Option(inner) => format!("Option<{}>", type_name(module, inner)),
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
            StatementKind::Return { value } => {
                out.push_str(&format!("{pad}return\n"));
                if let Some(value) = value {
                    dump_expr(module, locals, value, indent + 1, out);
                }
            }
            StatementKind::ValDecl { local, init } => {
                let local = &locals[*local];
                let keyword = if local.mutable { "var" } else { "val" };
                out.push_str(&format!(
                    "{pad}{keyword} {}: {}\n",
                    local.name,
                    type_name(module, &local.ty)
                ));
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
        }
    }
}

fn dump_expr(module: &Module, locals: &Arena<Local>, expr: &Expr, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    match expr {
        Expr::StringConst(id) => {
            out.push_str(&format!(
                "{pad}StringConst @{}\n",
                module.strings[*id].symbol
            ));
        }
        Expr::IntLiteral(value) => out.push_str(&format!("{pad}IntLiteral {value}\n")),
        Expr::BoolLiteral(value) => out.push_str(&format!("{pad}BoolLiteral {value}\n")),
        Expr::UnitLiteral => out.push_str(&format!("{pad}UnitLiteral\n")),
        Expr::TupleLiteral(elements) => {
            out.push_str(&format!("{pad}TupleLiteral\n"));
            for element in elements {
                dump_expr(module, locals, element, indent + 1, out);
            }
        }
        Expr::StructInit { struct_id, args } => {
            out.push_str(&format!(
                "{pad}StructInit {}\n",
                module.structs[*struct_id].name
            ));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        Expr::Local(local) => out.push_str(&format!("{pad}Local {}\n", locals[*local].name)),
        Expr::FieldAccess { receiver, index } => {
            out.push_str(&format!("{pad}FieldAccess {index}\n"));
            dump_expr(module, locals, receiver, indent + 1, out);
        }
        Expr::Call(call) => {
            let callee = match &call.target.callee {
                Callee::User(id) => format!("@{}", module.functions[*id].symbol),
                Callee::Runtime(function) => format!("@{}", function.symbol()),
            };
            out.push_str(&format!("{pad}Call {callee} direct\n"));
            for arg in &call.args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        Expr::Binary { op, lhs, rhs } => {
            out.push_str(&format!("{pad}Binary {op:?}\n"));
            dump_expr(module, locals, lhs, indent + 1, out);
            dump_expr(module, locals, rhs, indent + 1, out);
        }
        Expr::Unary { op, operand } => {
            out.push_str(&format!("{pad}Unary {op:?}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::SomeWrap(operand) => {
            out.push_str(&format!("{pad}SomeWrap\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::NoneLiteral => out.push_str(&format!("{pad}NoneLiteral\n")),
        Expr::IsSome(operand) => {
            out.push_str(&format!("{pad}IsSome\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::Unwrap {
            operand,
            trap_on_none,
        } => {
            out.push_str(&format!("{pad}Unwrap trap={trap_on_none}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
    }
}
