//! HIR definitions and HIR meta: the data channel between HIR and MIR.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.2 and
//! `docs/milestone1/DESIGN.md` section 2.3.
//!
//! Structural completeness rules (see AGENTS.md): every expression
//! carries its resolved type (`Expr::ty`), every call carries its
//! resolved target (`ExprKind::Call::function`), and a module always
//! has an entry point (`Module::entry`) — a module without `main`
//! never reaches HIR, it is a diagnostic in hir-lower.

use la_arena::{Arena, Idx};
use scoop_ast::Span;

pub type TypeId = Idx<Type>;
pub type FunctionId = Idx<Function>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Unit,
    String,
}

#[derive(Debug)]
pub struct Module {
    pub types: Arena<Type>,
    pub functions: Arena<Function>,
    /// Top-level functions in declaration order (builtins included).
    pub top_level: Vec<FunctionId>,
    /// Well-known types, allocated first by hir-lower.
    pub unit: TypeId,
    pub string: TypeId,
    /// Builtin output functions (temporary until M11, see DESIGN.md 5.2).
    pub print: FunctionId,
    pub println: FunctionId,
    /// Entry point: `fun main()`. Guaranteed present.
    pub entry: FunctionId,
}

#[derive(Debug)]
pub struct Function {
    pub name: String,
    pub kind: FunctionKind,
    pub span: Span,
}

#[derive(Debug)]
pub enum FunctionKind {
    User(Body),
    Builtin(Builtin),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Builtin {
    Print,
    Println,
}

#[derive(Debug)]
pub struct Body {
    pub statements: Vec<Statement>,
}

#[derive(Debug)]
pub struct Statement {
    pub expr: Expr,
    pub span: Span,
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
    Call {
        function: FunctionId,
        args: Vec<Expr>,
    },
}

/// Indented text dump for golden tests (`scoopc build --emit=hir`).
pub fn dump(module: &Module) -> String {
    let mut out = String::from("Module\n");
    for &id in &module.top_level {
        let function = &module.functions[id];
        match &function.kind {
            FunctionKind::Builtin(builtin) => {
                out.push_str(&format!("  fun {} <builtin {builtin:?}>\n", function.name));
            }
            FunctionKind::User(body) => {
                out.push_str(&format!("  fun {}\n", function.name));
                for statement in &body.statements {
                    dump_expr(module, &statement.expr, 2, &mut out);
                }
            }
        }
    }
    out.push_str(&format!(
        "  entry {}\n",
        module.functions[module.entry].name
    ));
    out
}

fn type_name(module: &Module, ty: TypeId) -> &'static str {
    match module.types[ty] {
        Type::Unit => "Unit",
        Type::String => "String",
    }
}

fn dump_expr(module: &Module, expr: &Expr, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    let ty = type_name(module, expr.ty);
    match &expr.kind {
        ExprKind::StringLiteral(value) => {
            out.push_str(&format!("{pad}StringLiteral {value:?} : {ty}\n"));
        }
        ExprKind::Call { function, args } => {
            let callee = &module.functions[*function];
            out.push_str(&format!("{pad}Call {} : {ty}\n", callee.name));
            for arg in args {
                dump_expr(module, arg, indent + 1, out);
            }
        }
    }
}
