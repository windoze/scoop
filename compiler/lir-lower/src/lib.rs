//! LIR stage: type layout, statepoint insertion, exception lowering.
//! LIR contains nothing Scoop-specific and is mechanically translatable
//! to the target IR.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.4.
//!
//! M1 (DESIGN.md 2.5): MIR string constants become globals, function
//! bodies become straight-line `Call` instruction lists, and the meta
//! carries the single `String` runtime layout. This stage never fails:
//! all errors were already reported by hir-lower.

use std::collections::HashMap;

use la_arena::Arena;
use scoop_lir as lir;
use scoop_mir as mir;

/// Lower MIR to LIR.
pub fn lower(module: &mir::Module) -> lir::Module {
    // Every MIR string constant becomes a global with the same symbol.
    let mut globals = Arena::new();
    let mut global_map: HashMap<mir::StringConstId, lir::GlobalId> = HashMap::new();
    for (id, string) in module.strings.iter() {
        let global = globals.alloc(lir::Global {
            symbol: string.symbol.clone(),
            init: lir::GlobalInit::StringConst(string.value.clone()),
        });
        global_map.insert(id, global);
    }

    let functions = module
        .top_level
        .iter()
        .map(|&id| lower_function(module, &module.functions[id], &global_map))
        .collect();

    lir::Module {
        globals,
        functions,
        entry_symbol: mir::ENTRY_SYMBOL.to_string(),
        meta: lir::LirMeta {
            layouts: vec![string_layout()],
        },
    }
}

/// The single M1 runtime layout (runtime spec 2.4): object header (one
/// pointer, 8 bytes) + `len` (u64, 8 bytes). The string data is
/// variable-length and not counted in `size`.
fn string_layout() -> lir::Layout {
    lir::Layout {
        name: "String".to_string(),
        size: 16,
        align: 8,
        ref_field_offsets: Vec::new(),
    }
}

fn lower_function(
    module: &mir::Module,
    function: &mir::Function,
    global_map: &HashMap<mir::StringConstId, lir::GlobalId>,
) -> lir::Function {
    let mut body = Vec::new();
    for statement in &function.body.statements {
        lower_expr(module, global_map, &statement.expr, &mut body);
    }
    lir::Function {
        symbol: function.symbol.clone(),
        body,
    }
}

/// Lower an expression, appending its instructions to `body`. Returns
/// the operand for expressions usable as call arguments; calls return
/// `Unit`, which has no LIR operand.
fn lower_expr(
    module: &mir::Module,
    global_map: &HashMap<mir::StringConstId, lir::GlobalId>,
    expr: &mir::Expr,
    body: &mut Vec<lir::Instruction>,
) -> Option<lir::Operand> {
    match expr {
        mir::Expr::StringConst(id) => Some(lir::Operand::StringGlobal(global_map[id])),
        mir::Expr::Call(call) => {
            // Arguments are evaluated left to right, before the call.
            let args = call
                .args
                .iter()
                .filter_map(|arg| lower_expr(module, global_map, arg, body))
                .collect();
            let symbol = match &call.target.callee {
                mir::Callee::User(id) => module.functions[*id].symbol.clone(),
                mir::Callee::Print => mir::PRINT_SYMBOL.to_string(),
                mir::Callee::Println => mir::PRINTLN_SYMBOL.to_string(),
            };
            body.push(lir::Instruction::Call { symbol, args });
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build the MIR of the M1 hello world shape by hand: `main` calls
    /// `println("hello, world")` then `helper()`, which calls
    /// `print("!")`.
    fn hello_world_mir() -> mir::Module {
        use scoop_ast::Span;
        const SPAN: Span = Span { start: 0, end: 0 };

        let mut strings = Arena::new();
        let hello = strings.alloc(mir::StringConst {
            value: "hello, world".to_string(),
            symbol: "scoop.str.0".to_string(),
        });
        let bang = strings.alloc(mir::StringConst {
            value: "!".to_string(),
            symbol: "scoop.str.1".to_string(),
        });

        let call_stmt = |callee: mir::Callee, args: Vec<mir::Expr>| mir::Statement {
            expr: mir::Expr::Call(mir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee,
                },
                args,
            }),
            span: SPAN,
        };

        let mut functions = Arena::new();
        let helper = functions.alloc(mir::Function {
            name: "helper".to_string(),
            symbol: "scoop.helper".to_string(),
            body: mir::Body {
                statements: vec![call_stmt(
                    mir::Callee::Print,
                    vec![mir::Expr::StringConst(bang)],
                )],
            },
        });
        let main = functions.alloc(mir::Function {
            name: "main".to_string(),
            symbol: mir::ENTRY_SYMBOL.to_string(),
            body: mir::Body {
                statements: vec![
                    call_stmt(mir::Callee::Println, vec![mir::Expr::StringConst(hello)]),
                    call_stmt(mir::Callee::User(helper), vec![]),
                ],
            },
        });

        mir::Module {
            functions,
            top_level: vec![main, helper],
            strings,
            entry: main,
            meta: mir::MirMeta::default(),
        }
    }

    #[test]
    fn lowers_hello_world() {
        let module = lower(&hello_world_mir());

        // Globals: one per MIR string constant, same symbol and value.
        let globals: Vec<(&str, &str)> = module
            .globals
            .iter()
            .map(|(_, g)| match &g.init {
                lir::GlobalInit::StringConst(value) => (g.symbol.as_str(), value.as_str()),
            })
            .collect();
        assert_eq!(
            globals,
            [("scoop.str.0", "hello, world"), ("scoop.str.1", "!")]
        );

        // Functions keep their mangled symbols.
        let symbols: Vec<&str> = module.functions.iter().map(|f| f.symbol.as_str()).collect();
        assert_eq!(symbols, [mir::ENTRY_SYMBOL, "scoop.helper"]);

        // Entry symbol is the fixed `scoop_main`.
        assert_eq!(module.entry_symbol, mir::ENTRY_SYMBOL);

        // Exactly one layout: `String`.
        assert_eq!(module.meta.layouts.len(), 1);
        let layout = &module.meta.layouts[0];
        assert_eq!(layout.name, "String");
        assert_eq!(layout.size, 16);
        assert_eq!(layout.align, 8);
        assert!(layout.ref_field_offsets.is_empty());

        // Golden dump locks the output structure.
        let expected = "\
Module
  global @scoop.str.0 = \"hello, world\"
  global @scoop.str.1 = \"!\"
  fun @scoop_main
    call @scoop_rt_println(@scoop.str.0)
    call @scoop.helper()
  fun @scoop.helper
    call @scoop_rt_print(@scoop.str.1)
  layout String size=16 align=8 refs=[]
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn call_instructions_resolve_callee_symbols() {
        let module = lower(&hello_world_mir());
        let main = &module.functions[0];
        assert_eq!(main.body.len(), 2);
        match &main.body[0] {
            lir::Instruction::Call { symbol, args } => {
                assert_eq!(symbol, mir::PRINTLN_SYMBOL);
                assert_eq!(args.len(), 1);
            }
        }
        match &main.body[1] {
            lir::Instruction::Call { symbol, args } => {
                assert_eq!(symbol, "scoop.helper");
                assert!(args.is_empty());
            }
        }
        match &module.functions[1].body[0] {
            lir::Instruction::Call { symbol, args } => {
                assert_eq!(symbol, mir::PRINT_SYMBOL);
                assert_eq!(args.len(), 1);
            }
        }
    }
}
