//! MIR definitions and MIR meta: the data channel between MIR and LIR.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.3 and
//! `docs/milestone1/DESIGN.md` section 2.4.
//!
//! M1 notes: no generics, no suspend, all calls are direct. Name
//! mangling is centralized in `mangling` (mir-lower); the entry point
//! mangles to the fixed symbol `scoop_main` which the C runtime calls.

use la_arena::{Arena, Idx};
use scoop_ast::Span;

pub type FunctionId = Idx<Function>;
pub type StringConstId = Idx<StringConst>;

/// Mangled symbol of the program entry point (called by the C runtime).
pub const ENTRY_SYMBOL: &str = "scoop_main";

/// Runtime symbols backing the temporary builtin output functions
/// (see DESIGN.md 5.2).
pub const PRINT_SYMBOL: &str = "scoop_rt_print";
pub const PRINTLN_SYMBOL: &str = "scoop_rt_println";

#[derive(Debug)]
pub struct Module {
    pub functions: Arena<Function>,
    /// User functions in declaration order (builtins excluded — they
    /// have no body and no symbol of their own).
    pub top_level: Vec<FunctionId>,
    pub strings: Arena<StringConst>,
    pub entry: FunctionId,
    pub meta: MirMeta,
}

/// Per-Cone MIR metadata (impl spec 2.3). M1: the structure exists but
/// is always empty — no class hierarchy, hence no dispatch tables.
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
    /// Mangled symbol; `scoop.<name>`, or `scoop_main` for the entry.
    pub symbol: String,
    pub body: Body,
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
pub enum Expr {
    StringConst(StringConstId),
    Call(Call),
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

#[derive(Debug)]
pub enum Callee {
    /// A user function defined in this Cone.
    User(FunctionId),
    /// The temporary builtin output functions (DESIGN.md 5.2).
    Print,
    Println,
}

/// Indented text dump for golden tests (`scoopc build --emit=mir`).
pub fn dump(module: &Module) -> String {
    let mut out = String::from("Module\n");
    for &id in &module.top_level {
        let function = &module.functions[id];
        out.push_str(&format!("  fun {} @{}\n", function.name, function.symbol));
        for statement in &function.body.statements {
            dump_expr(module, &statement.expr, 2, &mut out);
        }
    }
    for (id, string) in module.strings.iter() {
        out.push_str(&format!(
            "  str {} @{}\n",
            string.symbol,
            escape(&string.value)
        ));
        let _ = id;
    }
    out.push_str(&format!("  entry @{ENTRY_SYMBOL}\n"));
    out
}

fn escape(value: &str) -> String {
    format!("{value:?}")
}

fn dump_expr(module: &Module, expr: &Expr, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    match expr {
        Expr::StringConst(id) => {
            out.push_str(&format!(
                "{pad}StringConst @{}\n",
                module.strings[*id].symbol
            ));
        }
        Expr::Call(call) => {
            let callee = match &call.target.callee {
                Callee::User(id) => format!("@{}", module.functions[*id].symbol),
                Callee::Print => format!("@{PRINT_SYMBOL}"),
                Callee::Println => format!("@{PRINTLN_SYMBOL}"),
            };
            out.push_str(&format!("{pad}Call {callee} direct\n"));
            for arg in &call.args {
                dump_expr(module, arg, indent + 1, out);
            }
        }
    }
}
