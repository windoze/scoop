//! MIR stage: monomorphization, name mangling, call-kind annotation,
//! vtable/itable construction, suspend-to-state-machine lowering.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.3.
//!
//! M1 (DESIGN.md 2.4): no generics, no suspend, all calls are direct.
//! This stage collects string literals into global constants, mangles
//! function symbols, and resolves HIR call targets to MIR callees.
//! It never fails: all errors were already reported by hir-lower.

use std::collections::HashMap;

use la_arena::Arena;
use scoop_hir as hir;
use scoop_mir as mir;

use mir::ENTRY_SYMBOL;

/// Lower HIR to MIR.
pub fn lower(module: &hir::Module) -> mir::Module {
    Lowerer {
        functions: Arena::new(),
        top_level: Vec::new(),
        strings: Arena::new(),
        function_map: HashMap::new(),
    }
    .run(module)
}

/// M1 name mangling, centralized here (DESIGN.md 2.4, 5.5): user
/// functions mangle to `scoop.<name>`; the entry point mangles to the
/// fixed symbol `scoop_main` that the C runtime calls. M3 extends this
/// with type-argument encoding for monomorphized generics.
fn mangle_symbol(name: &str, is_entry: bool) -> String {
    if is_entry {
        ENTRY_SYMBOL.to_string()
    } else {
        format!("scoop.{name}")
    }
}

struct Lowerer {
    functions: Arena<mir::Function>,
    /// User functions in declaration order (builtins excluded).
    top_level: Vec<mir::FunctionId>,
    strings: Arena<mir::StringConst>,
    /// HIR user function -> MIR function.
    function_map: HashMap<hir::FunctionId, mir::FunctionId>,
}

impl Lowerer {
    fn run(mut self, module: &hir::Module) -> mir::Module {
        // Declare user functions first, so calls resolve regardless of
        // declaration order. Builtins have no body and no symbol of
        // their own; their callsites map to `Callee::Print`/`Println`.
        let mut user_functions = Vec::new();
        for &hir_id in &module.top_level {
            let function = &module.functions[hir_id];
            if matches!(function.kind, hir::FunctionKind::User(_)) {
                let id = self.functions.alloc(mir::Function {
                    name: function.name.clone(),
                    symbol: mangle_symbol(&function.name, hir_id == module.entry),
                    body: mir::Body {
                        statements: Vec::new(),
                    },
                });
                self.top_level.push(id);
                self.function_map.insert(hir_id, id);
                user_functions.push((hir_id, id));
            }
        }

        for (hir_id, mir_id) in user_functions {
            let hir::FunctionKind::User(body) = &module.functions[hir_id].kind else {
                continue; // filtered to user functions above
            };
            let body = self.lower_body(module, body);
            self.functions[mir_id].body = body;
        }

        // The entry point is a user function, hence always in the map.
        let entry = self.function_map[&module.entry];
        mir::Module {
            functions: self.functions,
            top_level: self.top_level,
            strings: self.strings,
            entry,
            meta: mir::MirMeta::default(),
        }
    }

    fn lower_body(&mut self, module: &hir::Module, body: &hir::Body) -> mir::Body {
        let statements = body
            .statements
            .iter()
            .map(|statement| mir::Statement {
                expr: self.lower_expr(module, &statement.expr),
                span: statement.span,
            })
            .collect();
        mir::Body { statements }
    }

    fn lower_expr(&mut self, module: &hir::Module, expr: &hir::Expr) -> mir::Expr {
        match &expr.kind {
            hir::ExprKind::StringLiteral(value) => {
                // One global constant per literal occurrence, numbered in
                // order of appearance (deterministic).
                let symbol = format!("scoop.str.{}", self.strings.len());
                let id = self.strings.alloc(mir::StringConst {
                    value: value.clone(),
                    symbol,
                });
                mir::Expr::StringConst(id)
            }
            hir::ExprKind::Call { function, args } => {
                let callee = if *function == module.print {
                    mir::Callee::Print
                } else if *function == module.println {
                    mir::Callee::Println
                } else {
                    mir::Callee::User(self.function_map[function])
                };
                mir::Expr::Call(mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee,
                    },
                    args: args
                        .iter()
                        .map(|arg| self.lower_expr(module, arg))
                        .collect(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoop_ast::Span;

    const SPAN: Span = Span { start: 0, end: 0 };

    /// Build the HIR of the M1 hello world shape by hand: `main` calls
    /// `println("hello, world")` then `helper()`, which calls
    /// `print("!")`.
    fn hello_world_hir() -> hir::Module {
        let mut types = Arena::new();
        let unit = types.alloc(hir::Type::Unit);
        let string = types.alloc(hir::Type::String);

        let mut functions = Arena::new();
        let print = functions.alloc(hir::Function {
            name: "print".to_string(),
            kind: hir::FunctionKind::Builtin(hir::Builtin::Print),
            span: SPAN,
        });
        let println = functions.alloc(hir::Function {
            name: "println".to_string(),
            kind: hir::FunctionKind::Builtin(hir::Builtin::Println),
            span: SPAN,
        });

        let string_expr = |value: &str| hir::Expr {
            kind: hir::ExprKind::StringLiteral(value.to_string()),
            ty: string,
            span: SPAN,
        };
        let call_expr = |function: hir::FunctionId, args: Vec<hir::Expr>| hir::Expr {
            kind: hir::ExprKind::Call { function, args },
            ty: unit,
            span: SPAN,
        };
        let stmt = |expr: hir::Expr| hir::Statement { expr, span: SPAN };

        let helper = functions.alloc(hir::Function {
            name: "helper".to_string(),
            kind: hir::FunctionKind::User(hir::Body {
                statements: vec![stmt(call_expr(print, vec![string_expr("!")]))],
            }),
            span: SPAN,
        });
        let main = functions.alloc(hir::Function {
            name: "main".to_string(),
            kind: hir::FunctionKind::User(hir::Body {
                statements: vec![
                    stmt(call_expr(println, vec![string_expr("hello, world")])),
                    stmt(call_expr(helper, vec![])),
                ],
            }),
            span: SPAN,
        });

        hir::Module {
            types,
            functions,
            top_level: vec![print, println, main, helper],
            unit,
            string,
            print,
            println,
            entry: main,
        }
    }

    #[test]
    fn lowers_hello_world() {
        let module = lower(&hello_world_hir());

        // Builtins are excluded from `top_level`; declaration order kept.
        assert_eq!(module.top_level.len(), 2);
        let main = &module.functions[module.top_level[0]];
        let helper = &module.functions[module.top_level[1]];
        assert_eq!(main.name, "main");
        assert_eq!(helper.name, "helper");

        // Mangling: entry is the fixed `scoop_main`, others `scoop.<name>`.
        assert_eq!(main.symbol, ENTRY_SYMBOL);
        assert_eq!(helper.symbol, "scoop.helper");
        assert_eq!(module.entry, module.top_level[0]);

        // String literals became numbered global constants.
        let strings: Vec<(&str, &str)> = module
            .strings
            .iter()
            .map(|(_, s)| (s.value.as_str(), s.symbol.as_str()))
            .collect();
        assert_eq!(
            strings,
            [("hello, world", "scoop.str.0"), ("!", "scoop.str.1")]
        );

        // M1 meta exists but is empty.
        assert!(module.meta.dispatch_tables.is_empty());

        // Golden dump locks the output structure.
        let expected = "\
Module
  fun main @scoop_main
    Call @scoop_rt_println direct
      StringConst @scoop.str.0
    Call @scoop.helper direct
  fun helper @scoop.helper
    Call @scoop_rt_print direct
      StringConst @scoop.str.1
  str scoop.str.0 @\"hello, world\"
  str scoop.str.1 @\"!\"
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }

    #[test]
    fn repeated_literals_get_separate_constants_deterministically() {
        let mut hir_module = hello_world_hir();
        // Add another `println("hello, world")` to `main`.
        let println = hir_module.println;
        let string = hir_module.string;
        let unit = hir_module.unit;
        let main_id = hir_module.entry;
        let hir::FunctionKind::User(body) = &mut hir_module.functions[main_id].kind else {
            unreachable!()
        };
        body.statements.push(hir::Statement {
            expr: hir::Expr {
                kind: hir::ExprKind::Call {
                    function: println,
                    args: vec![hir::Expr {
                        kind: hir::ExprKind::StringLiteral("hello, world".to_string()),
                        ty: string,
                        span: SPAN,
                    }],
                },
                ty: unit,
                span: SPAN,
            },
            span: SPAN,
        });

        let module = lower(&hir_module);
        let symbols: Vec<&str> = module
            .strings
            .iter()
            .map(|(_, s)| s.symbol.as_str())
            .collect();
        assert_eq!(symbols, ["scoop.str.0", "scoop.str.1", "scoop.str.2"]);
    }
}
