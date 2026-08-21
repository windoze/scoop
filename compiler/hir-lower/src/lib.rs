//! HIR stage: desugaring, type check, overload resolution, instantiation
//! requests. All compile-time errors are reported here.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.2.
//!
//! M1 (DESIGN.md 2.3): allocates the well-known types (`Unit`, `String`)
//! and the builtin output functions (`print` / `println`) first, then
//! resolves top-level names and lowers bodies. Every expression leaves
//! this stage with its type filled in, every call with its target
//! resolved to a `FunctionId`.

use std::collections::HashMap;

use la_arena::Arena;
use scoop_ast as ast;
use scoop_hir as hir;

use ast::{Diagnostic, Span, StatementKind};
use hir::{Builtin, Function, FunctionId, FunctionKind, Type, TypeId};

/// Lower a parsed source file to HIR.
///
/// All semantic errors of the M1 subset are diagnosed here with spans;
/// downstream stages (MIR, LIR) never fail.
pub fn lower(file: &ast::SourceFile) -> Result<hir::Module, Vec<Diagnostic>> {
    Lowerer::new(file.span).run(file)
}

struct Lowerer {
    types: Arena<Type>,
    functions: Arena<Function>,
    top_level: Vec<FunctionId>,
    unit: TypeId,
    string: TypeId,
    print: FunctionId,
    println: FunctionId,
    /// Top-level function symbol table (builtins included).
    symbols: HashMap<String, FunctionId>,
    diagnostics: Vec<Diagnostic>,
}

impl Lowerer {
    fn new(file_span: Span) -> Self {
        // Well-known types are allocated first (impl spec 2.2).
        let mut types = Arena::new();
        let unit = types.alloc(Type::Unit);
        let string = types.alloc(Type::String);

        // Builtin output functions (temporary until M11, DESIGN.md 5.2).
        // They have no source span; attribute them to the whole file.
        let mut functions = Arena::new();
        let print = functions.alloc(Function {
            name: "print".to_string(),
            kind: FunctionKind::Builtin(Builtin::Print),
            span: file_span,
        });
        let println = functions.alloc(Function {
            name: "println".to_string(),
            kind: FunctionKind::Builtin(Builtin::Println),
            span: file_span,
        });

        let symbols = HashMap::from([
            ("print".to_string(), print),
            ("println".to_string(), println),
        ]);

        Lowerer {
            types,
            functions,
            top_level: vec![print, println],
            unit,
            string,
            print,
            println,
            symbols,
            diagnostics: Vec::new(),
        }
    }

    fn run(mut self, file: &ast::SourceFile) -> Result<hir::Module, Vec<Diagnostic>> {
        // Pass 1: declare user functions, so calls resolve regardless of
        // declaration order.
        let mut user_functions = Vec::new();
        for decl in &file.functions {
            if self.symbols.contains_key(&decl.name.text) {
                self.error(
                    decl.name.span,
                    format!("duplicate function `{}`", decl.name.text),
                );
                continue;
            }
            let id = self.functions.alloc(Function {
                name: decl.name.text.clone(),
                kind: FunctionKind::User(hir::Body {
                    statements: Vec::new(),
                }),
                span: decl.span,
            });
            self.top_level.push(id);
            self.symbols.insert(decl.name.text.clone(), id);
            user_functions.push((id, decl));
        }

        // Pass 2: lower bodies.
        for (id, decl) in user_functions {
            let body = self.lower_body(decl);
            self.functions[id].kind = FunctionKind::User(body);
        }

        // A module without `main` never reaches HIR (hir docs); it is a
        // diagnostic here, attributed to the whole file.
        let entry = match self.symbols.get("main") {
            Some(&id) => Some(id),
            None => {
                self.error(
                    file.span,
                    "missing entry point: declare `fun main()`".to_string(),
                );
                None
            }
        };

        if !self.diagnostics.is_empty() {
            return Err(self.diagnostics);
        }
        // Invariant: empty diagnostics implies `main` was found above.
        let entry = entry.expect("missing `main` is always diagnosed");
        Ok(hir::Module {
            types: self.types,
            functions: self.functions,
            top_level: self.top_level,
            unit: self.unit,
            string: self.string,
            print: self.print,
            println: self.println,
            entry,
        })
    }

    fn lower_body(&mut self, decl: &ast::FunctionDecl) -> hir::Body {
        let mut statements = Vec::new();
        for statement in &decl.body.statements {
            match &statement.kind {
                StatementKind::Expr(expr) => {
                    let Some(lowered) = self.lower_expr(expr) else {
                        continue; // diagnostic already recorded
                    };
                    // M1 statements are function calls (DESIGN.md 1).
                    if matches!(lowered.kind, hir::ExprKind::Call { .. }) {
                        statements.push(hir::Statement {
                            expr: lowered,
                            span: statement.span,
                        });
                    } else {
                        self.error(
                            statement.span,
                            "statement must be a function call".to_string(),
                        );
                    }
                }
            }
        }
        hir::Body { statements }
    }

    /// Lower an expression, recording a diagnostic and returning `None`
    /// on error.
    fn lower_expr(&mut self, expr: &ast::Expr) -> Option<hir::Expr> {
        match expr {
            ast::Expr::StringLiteral { value, span } => Some(hir::Expr {
                kind: hir::ExprKind::StringLiteral(value.clone()),
                ty: self.string,
                span: *span,
            }),
            ast::Expr::Call(call) => self.lower_call(call),
        }
    }

    fn lower_call(&mut self, call: &ast::CallExpr) -> Option<hir::Expr> {
        let function = match self.symbols.get(&call.callee.text) {
            Some(&id) => id,
            None => {
                self.error(
                    call.callee.span,
                    format!("unknown function `{}`", call.callee.text),
                );
                return None;
            }
        };
        let name = self.functions[function].name.clone();
        let is_builtin = matches!(self.functions[function].kind, FunctionKind::Builtin(_));

        // M1: builtins take exactly one `String` argument, user functions
        // take none; every function returns `Unit`.
        let args = if is_builtin {
            if call.args.len() != 1 {
                let supplied = call.args.len();
                self.error(
                    call.span,
                    format!("`{name}` takes exactly 1 argument, but {supplied} were supplied"),
                );
                return None;
            }
            let arg = self.lower_expr(&call.args[0])?;
            if arg.ty != self.string {
                let found = self.type_name(arg.ty);
                self.error(
                    arg.span,
                    format!("argument of `{name}` must be of type String, found {found}"),
                );
                return None;
            }
            vec![arg]
        } else {
            if !call.args.is_empty() {
                let supplied = call.args.len();
                self.error(
                    call.span,
                    format!("function `{name}` takes no arguments, but {supplied} were supplied"),
                );
                return None;
            }
            Vec::new()
        };

        Some(hir::Expr {
            kind: hir::ExprKind::Call { function, args },
            ty: self.unit,
            span: call.span,
        })
    }

    fn type_name(&self, ty: TypeId) -> &'static str {
        match self.types[ty] {
            Type::Unit => "Unit",
            Type::String => "String",
        }
    }

    fn error(&mut self, span: Span, message: String) {
        self.diagnostics.push(Diagnostic::at(span, message));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ast::{Block, CallExpr, Expr, FunctionDecl, Ident, SourceFile, Statement};

    fn ident(text: &str, span: Span) -> Ident {
        Ident {
            text: text.to_string(),
            span,
        }
    }

    fn str_lit(value: &str) -> Expr {
        Expr::StringLiteral {
            value: value.to_string(),
            span: Span::new(0, 0),
        }
    }

    fn call_at(name: &str, args: Vec<Expr>, callee_span: Span, call_span: Span) -> Expr {
        Expr::Call(CallExpr {
            callee: ident(name, callee_span),
            args,
            span: call_span,
        })
    }

    fn call(name: &str, args: Vec<Expr>) -> Expr {
        call_at(name, args, Span::new(0, 0), Span::new(0, 0))
    }

    fn stmt(expr: Expr) -> Statement {
        Statement {
            kind: StatementKind::Expr(expr),
            span: Span::new(0, 0),
        }
    }

    fn fun(name: &str, statements: Vec<Statement>) -> FunctionDecl {
        FunctionDecl {
            name: ident(name, Span::new(0, 0)),
            body: Block {
                statements,
                span: Span::new(0, 0),
            },
            span: Span::new(0, 0),
        }
    }

    fn file(functions: Vec<FunctionDecl>) -> SourceFile {
        SourceFile {
            functions,
            span: Span::new(0, 100),
        }
    }

    /// `main` calls `println("hello, world")` then `helper()`, which
    /// calls `print("!")` — the M1 hello world shape (DESIGN.md 1).
    fn hello_world() -> SourceFile {
        file(vec![
            fun(
                "main",
                vec![
                    stmt(call("println", vec![str_lit("hello, world")])),
                    stmt(call("helper", vec![])),
                ],
            ),
            fun("helper", vec![stmt(call("print", vec![str_lit("!")]))]),
        ])
    }

    #[test]
    fn lowers_hello_world() {
        let module = lower(&hello_world()).expect("hello world must lower");

        // Well-known types and builtins are allocated first.
        assert_eq!(module.types[module.unit], Type::Unit);
        assert_eq!(module.types[module.string], Type::String);
        assert!(matches!(
            module.functions[module.print].kind,
            FunctionKind::Builtin(Builtin::Print)
        ));
        assert!(matches!(
            module.functions[module.println].kind,
            FunctionKind::Builtin(Builtin::Println)
        ));

        // Entry point is `main`.
        assert_eq!(module.functions[module.entry].name, "main");

        // Golden dump locks the output structure.
        let expected = "\
Module
  fun print <builtin Print>
  fun println <builtin Println>
  fun main
    Call println : Unit
      StringLiteral \"hello, world\" : String
    Call helper : Unit
  fun helper
    Call print : Unit
      StringLiteral \"!\" : String
  entry main
";
        assert_eq!(hir::dump(&module), expected);
    }

    #[test]
    fn duplicate_function_is_an_error() {
        let file = file(vec![fun("main", vec![]), fun("main", vec![])]);
        let errors = lower(&file).expect_err("duplicate `main` must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "duplicate function `main`");
    }

    #[test]
    fn redeclaring_a_builtin_is_an_error() {
        let file = file(vec![fun("main", vec![]), fun("print", vec![])]);
        let errors = lower(&file).expect_err("redeclaring `print` must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "duplicate function `print`");
    }

    #[test]
    fn unknown_function_is_an_error_with_callee_span() {
        let callee_span = Span::new(10, 15);
        let file = file(vec![fun(
            "main",
            vec![stmt(call_at("hello", vec![], callee_span, Span::new(0, 0)))],
        )]);
        let errors = lower(&file).expect_err("unknown callee must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "unknown function `hello`");
        assert_eq!(errors[0].span, Some(callee_span));
    }

    #[test]
    fn print_requires_exactly_one_argument() {
        for args in [vec![], vec![str_lit("a"), str_lit("b")]] {
            let supplied = args.len();
            let file = file(vec![fun("main", vec![stmt(call("print", args))])]);
            let errors = lower(&file).expect_err("wrong arity must fail");
            assert_eq!(errors.len(), 1);
            assert_eq!(
                errors[0].message,
                format!("`print` takes exactly 1 argument, but {supplied} were supplied")
            );
        }
    }

    #[test]
    fn print_argument_must_be_a_string() {
        // `helper()` has type `Unit`, not `String`.
        let file = file(vec![
            fun(
                "main",
                vec![stmt(call("println", vec![call("helper", vec![])]))],
            ),
            fun("helper", vec![]),
        ]);
        let errors = lower(&file).expect_err("non-String argument must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "argument of `println` must be of type String, found Unit"
        );
    }

    #[test]
    fn user_function_takes_no_arguments() {
        let file = file(vec![
            fun("main", vec![stmt(call("helper", vec![str_lit("x")]))]),
            fun("helper", vec![]),
        ]);
        let errors = lower(&file).expect_err("argument to `helper` must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "function `helper` takes no arguments, but 1 were supplied"
        );
    }

    #[test]
    fn missing_main_is_an_error_with_file_span() {
        let file = file(vec![fun("helper", vec![])]);
        let errors = lower(&file).expect_err("missing `main` must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "missing entry point: declare `fun main()`"
        );
        assert_eq!(errors[0].span, Some(file.span));
    }

    #[test]
    fn bare_literal_statement_is_an_error() {
        let file = file(vec![fun("main", vec![stmt(str_lit("dangling"))])]);
        let errors = lower(&file).expect_err("literal statement must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "statement must be a function call");
    }

    #[test]
    fn collects_multiple_diagnostics() {
        let file = file(vec![fun(
            "main",
            vec![
                stmt(call("missing_one", vec![])),
                stmt(call("missing_two", vec![])),
            ],
        )]);
        let errors = lower(&file).expect_err("unknown callees must fail");
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].message, "unknown function `missing_one`");
        assert_eq!(errors[1].message, "unknown function `missing_two`");
    }
}
