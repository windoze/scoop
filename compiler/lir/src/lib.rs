//! LIR definitions and LIR meta: the data channel between LIR and codegen.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.4 and
//! `docs/milestone1/DESIGN.md` section 2.5.
//!
//! LIR contains nothing Scoop-specific: functions are straight-line
//! instruction lists over globally visible symbols, and codegen
//! translates them mechanically.

use la_arena::{Arena, Idx};

pub type GlobalId = Idx<Global>;

/// Symbol of the TypeDescriptor global for `String` (runtime spec 2.2).
pub const STRING_TD_SYMBOL: &str = "scoop_td_String";

#[derive(Debug)]
pub struct Module {
    pub globals: Arena<Global>,
    pub functions: Vec<Function>,
    /// Symbol of the entry function (`scoop_main`).
    pub entry_symbol: String,
    pub meta: LirMeta,
}

/// Per-Cone LIR metadata (impl spec 2.4): type layouts.
#[derive(Debug)]
pub struct LirMeta {
    pub layouts: Vec<Layout>,
}

#[derive(Debug)]
pub struct Layout {
    pub name: String,
    pub size: u64,
    pub align: u64,
    /// Byte offsets of reference fields (for the GC bitmap). M1: only
    /// `String` exists and it has no reference fields.
    pub ref_field_offsets: Vec<u64>,
}

#[derive(Debug)]
pub struct Global {
    pub symbol: String,
    pub init: GlobalInit,
}

#[derive(Debug)]
pub enum GlobalInit {
    /// A `ScoopString` constant: header points at `STRING_TD_SYMBOL`.
    StringConst(String),
}

#[derive(Debug)]
pub struct Function {
    pub symbol: String,
    pub body: Vec<Instruction>,
}

#[derive(Debug)]
pub enum Instruction {
    /// Direct call to a `void(ptr...)` symbol: a user function or a
    /// runtime function.
    Call { symbol: String, args: Vec<Operand> },
}

#[derive(Debug, Clone, Copy)]
pub enum Operand {
    /// Address of a `ScoopString` global constant.
    StringGlobal(GlobalId),
}

/// Indented text dump for golden tests (`scoopc build --emit=lir`).
pub fn dump(module: &Module) -> String {
    let mut out = String::from("Module\n");
    for global in module.globals.iter().map(|(_, g)| g) {
        match &global.init {
            GlobalInit::StringConst(value) => {
                out.push_str(&format!("  global @{} = {:?}\n", global.symbol, value));
            }
        }
    }
    for function in &module.functions {
        out.push_str(&format!("  fun @{}\n", function.symbol));
        for instruction in &function.body {
            match instruction {
                Instruction::Call { symbol, args } => {
                    let args: Vec<String> = args
                        .iter()
                        .map(|arg| match arg {
                            Operand::StringGlobal(id) => {
                                format!("@{}", module.globals[*id].symbol)
                            }
                        })
                        .collect();
                    out.push_str(&format!("    call @{}({})\n", symbol, args.join(", ")));
                }
            }
        }
    }
    for layout in &module.meta.layouts {
        out.push_str(&format!(
            "  layout {} size={} align={} refs={:?}\n",
            layout.name, layout.size, layout.align, layout.ref_field_offsets
        ));
    }
    out.push_str(&format!("  entry @{}\n", module.entry_symbol));
    out
}
