//! MIR stage: monomorphization, name mangling, call-kind annotation,
//! vtable/itable construction, suspend-to-state-machine lowering.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.3 and
//! `docs/milestone2/DESIGN.md` section 2.3.
//!
//! M2: value types. HIR types are mapped onto MIR types (the struct
//! arena is transposed in declaration order, field types recursively);
//! structural equality on aggregates is expanded into primitive
//! comparisons and runtime calls; `print` / `println` map onto the
//! per-type runtime shims; String `+` becomes `scoop_rt_string_concat`.
//! Control flow stays structured (`If` / `While`) and `&&` / `||` stay
//! single MIR operators — basic blocks and short-circuit expansion are
//! LIR's job. This stage never fails: all errors were already reported
//! by hir-lower.

use std::collections::HashMap;

use la_arena::Arena;
use scoop_hir as hir;
use scoop_mir as mir;

/// Lower HIR to MIR.
pub fn lower(module: &hir::Module) -> mir::Module {
    Lowerer {
        functions: Arena::new(),
        top_level: Vec::new(),
        strings: Arena::new(),
        structs: Arena::new(),
        struct_map: HashMap::new(),
        function_map: HashMap::new(),
    }
    .run(module)
}

struct Lowerer {
    functions: Arena<mir::Function>,
    /// User functions in declaration order (builtins have no MIR body).
    top_level: Vec<mir::FunctionId>,
    strings: Arena<mir::StringConst>,
    structs: Arena<mir::StructDef>,
    /// HIR struct -> MIR struct (arena transposed in declaration order).
    struct_map: HashMap<hir::StructId, mir::StructId>,
    /// HIR user function -> MIR function.
    function_map: HashMap<hir::FunctionId, mir::FunctionId>,
}

impl Lowerer {
    fn run(mut self, module: &hir::Module) -> mir::Module {
        self.lower_structs(module);

        // Declare user functions first, so calls resolve regardless of
        // declaration order. Builtins have no body; their callsites map
        // to `Callee::Runtime` shims (see `print_fn`).
        let mut user_functions = Vec::new();
        for &hir_id in &module.top_level {
            let function = &module.functions[hir_id];
            if matches!(function.kind, hir::FunctionKind::User(_)) {
                let id = self.functions.alloc(mir::Function {
                    name: function.name.clone(),
                    // M2 mangling: `scoop.<name>`, or the fixed entry
                    // symbol `scoop_main` that the C runtime calls.
                    symbol: mir::mangle_function(&function.name, hir_id == module.entry),
                    body: mir::Body {
                        locals: Arena::new(),
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
            let body = BodyLowerer {
                module,
                struct_map: &self.struct_map,
                function_map: &self.function_map,
                strings: &mut self.strings,
                local_map: HashMap::new(),
            }
            .lower_body(body);
            self.functions[mir_id].body = body;
        }

        // The entry point is a user function, hence always in the map.
        let entry = self.function_map[&module.entry];
        mir::Module {
            functions: self.functions,
            top_level: self.top_level,
            strings: self.strings,
            structs: self.structs,
            entry,
            meta: mir::MirMeta::default(),
        }
    }

    /// Transpose the HIR struct arena into MIR in declaration order.
    /// Field types are mapped in a second pass, so a struct field can
    /// reference any struct regardless of declaration order.
    fn lower_structs(&mut self, module: &hir::Module) {
        for (hir_id, decl) in module.structs.iter() {
            let mir_id = self.structs.alloc(mir::StructDef {
                name: decl.name.clone(),
                fields: Vec::new(),
            });
            self.struct_map.insert(hir_id, mir_id);
        }
        for (hir_id, decl) in module.structs.iter() {
            let fields = decl
                .fields
                .iter()
                .map(|field| mir::Field {
                    name: field.name.clone(),
                    ty: lower_type(module, &self.struct_map, field.ty),
                })
                .collect();
            self.structs[self.struct_map[&hir_id]].fields = fields;
        }
    }
}

/// Map a HIR type onto its MIR type. Aggregate shapes are preserved:
/// structs keep their (remapped) id, tuples their mapped elements.
fn lower_type(
    module: &hir::Module,
    struct_map: &HashMap<hir::StructId, mir::StructId>,
    ty: hir::TypeId,
) -> mir::Type {
    match &module.types[ty] {
        hir::Type::Unit => mir::Type::Unit,
        hir::Type::Int => mir::Type::Int,
        hir::Type::Boolean => mir::Type::Boolean,
        hir::Type::String => mir::Type::String,
        hir::Type::Struct(id) => mir::Type::Struct(struct_map[id]),
        hir::Type::Tuple(elements) => mir::Type::Tuple(
            elements
                .iter()
                .map(|&element| lower_type(module, struct_map, element))
                .collect(),
        ),
    }
}

/// Map a `print` / `println` call onto the per-type runtime shim
/// (DESIGN 5.2). hir-lower rejects arguments of any other type, so
/// only String / Int / Boolean can reach this stage.
fn print_fn(module: &hir::Module, ty: hir::TypeId, newline: bool) -> mir::RuntimeFn {
    use mir::RuntimeFn::*;
    match (&module.types[ty], newline) {
        (hir::Type::String, false) => PrintString,
        (hir::Type::String, true) => PrintlnString,
        (hir::Type::Int, false) => PrintInt,
        (hir::Type::Int, true) => PrintlnInt,
        (hir::Type::Boolean, false) => PrintBoolean,
        (hir::Type::Boolean, true) => PrintlnBoolean,
        _ => unreachable!("hir-lower rejects print arguments that are not String/Int/Boolean"),
    }
}

/// Per-function-body lowering state.
struct BodyLowerer<'a> {
    module: &'a hir::Module,
    struct_map: &'a HashMap<hir::StructId, mir::StructId>,
    function_map: &'a HashMap<hir::FunctionId, mir::FunctionId>,
    strings: &'a mut Arena<mir::StringConst>,
    /// HIR local -> MIR local (same declaration order per body).
    local_map: HashMap<hir::LocalId, mir::LocalId>,
}

impl BodyLowerer<'_> {
    fn lower_body(mut self, body: &hir::Body) -> mir::Body {
        let mut locals = Arena::new();
        for (hir_id, local) in body.locals.iter() {
            let mir_id = locals.alloc(mir::Local {
                name: local.name.clone(),
                ty: lower_type(self.module, self.struct_map, local.ty),
                mutable: local.mutable,
            });
            self.local_map.insert(hir_id, mir_id);
        }
        let statements = self.lower_statements(&body.statements);
        mir::Body { locals, statements }
    }

    fn lower_statements(&mut self, statements: &[hir::Statement]) -> Vec<mir::Statement> {
        statements
            .iter()
            .map(|statement| self.lower_statement(statement))
            .collect()
    }

    fn lower_statement(&mut self, statement: &hir::Statement) -> mir::Statement {
        let kind = match &statement.kind {
            hir::StatementKind::Expr(expr) => mir::StatementKind::Expr(self.lower_expr(expr)),
            hir::StatementKind::ValDecl { local, init } => mir::StatementKind::ValDecl {
                local: self.local_map[local],
                init: self.lower_expr(init),
            },
            hir::StatementKind::Assign { local, value } => mir::StatementKind::Assign {
                local: self.local_map[local],
                value: self.lower_expr(value),
            },
            hir::StatementKind::If {
                cond,
                then_body,
                else_body,
            } => mir::StatementKind::If {
                cond: self.lower_expr(cond),
                then_body: self.lower_statements(then_body),
                else_body: else_body.as_ref().map(|body| self.lower_statements(body)),
            },
            hir::StatementKind::While { cond, body } => mir::StatementKind::While {
                cond: self.lower_expr(cond),
                body: self.lower_statements(body),
            },
        };
        mir::Statement {
            kind,
            span: statement.span,
        }
    }

    fn lower_expr(&mut self, expr: &hir::Expr) -> mir::Expr {
        match &expr.kind {
            hir::ExprKind::StringLiteral(value) => {
                // One global constant per literal occurrence, numbered
                // in order of appearance (deterministic).
                let symbol = format!("scoop.str.{}", self.strings.len());
                let id = self.strings.alloc(mir::StringConst {
                    value: value.clone(),
                    symbol,
                });
                mir::Expr::StringConst(id)
            }
            hir::ExprKind::IntLiteral(value) => mir::Expr::IntLiteral(*value),
            hir::ExprKind::BoolLiteral(value) => mir::Expr::BoolLiteral(*value),
            hir::ExprKind::UnitLiteral => mir::Expr::UnitLiteral,
            hir::ExprKind::TupleLiteral(elements) => {
                mir::Expr::TupleLiteral(elements.iter().map(|e| self.lower_expr(e)).collect())
            }
            hir::ExprKind::StructInit { struct_id, args } => mir::Expr::StructInit {
                struct_id: self.struct_map[struct_id],
                args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
            },
            hir::ExprKind::Local(local) => mir::Expr::Local(self.local_map[local]),
            hir::ExprKind::FieldAccess { receiver, field } => {
                // Struct fields and tuple elements are both 0-based here.
                let index = match field {
                    hir::FieldRef::StructField { index, .. } | hir::FieldRef::TupleIndex(index) => {
                        *index
                    }
                };
                mir::Expr::FieldAccess {
                    receiver: Box::new(self.lower_expr(receiver)),
                    index,
                }
            }
            hir::ExprKind::Call { function, args } => self.lower_call(*function, args),
            hir::ExprKind::Binary { op, lhs, rhs } => self.lower_binary(*op, lhs, rhs),
            hir::ExprKind::Unary { op, operand } => {
                let operand = Box::new(self.lower_expr(operand));
                let op = match op {
                    hir::UnOp::Neg => mir::UnOp::IntNeg,
                    hir::UnOp::Not => mir::UnOp::BoolNot,
                };
                mir::Expr::Unary { op, operand }
            }
        }
    }

    fn lower_call(&mut self, function: hir::FunctionId, args: &[hir::Expr]) -> mir::Expr {
        let callee = if function == self.module.print || function == self.module.println {
            // hir-lower enforces exactly one argument (DESIGN 5.2).
            let newline = function == self.module.println;
            mir::Callee::Runtime(print_fn(self.module, args[0].ty, newline))
        } else {
            mir::Callee::User(self.function_map[&function])
        };
        self.call(callee, &args.iter().collect::<Vec<_>>())
    }

    fn call(&mut self, callee: mir::Callee, args: &[&hir::Expr]) -> mir::Expr {
        mir::Expr::Call(mir::Call {
            target: mir::CallTarget {
                kind: mir::CallKind::Direct,
                callee,
            },
            args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
        })
    }

    fn lower_binary(&mut self, op: hir::BinOp, lhs: &hir::Expr, rhs: &hir::Expr) -> mir::Expr {
        use mir::BinOp::*;
        match op {
            // String `+` is runtime concatenation (DESIGN 2.3); hir-lower
            // type checking makes both operands String here.
            hir::BinOp::Add if matches!(self.module.types[lhs.ty], hir::Type::String) => self.call(
                mir::Callee::Runtime(mir::RuntimeFn::StringConcat),
                &[lhs, rhs],
            ),
            hir::BinOp::Add => self.primitive(IntAdd, lhs, rhs),
            hir::BinOp::Sub => self.primitive(IntSub, lhs, rhs),
            hir::BinOp::Mul => self.primitive(IntMul, lhs, rhs),
            hir::BinOp::Div => self.primitive(IntDiv, lhs, rhs),
            hir::BinOp::Lt => self.primitive(IntLt, lhs, rhs),
            hir::BinOp::Le => self.primitive(IntLe, lhs, rhs),
            hir::BinOp::Gt => self.primitive(IntGt, lhs, rhs),
            hir::BinOp::Ge => self.primitive(IntGe, lhs, rhs),
            // Structural equality dispatches on the operand type.
            hir::BinOp::Eq => self.expand_equality(lhs, rhs, lhs.ty, &[], false),
            hir::BinOp::Ne => self.expand_equality(lhs, rhs, lhs.ty, &[], true),
            // `&&` / `||` stay single operators; LIR expands the
            // short-circuit into basic blocks (DESIGN 2.4).
            hir::BinOp::And => self.primitive(And, lhs, rhs),
            hir::BinOp::Or => self.primitive(Or, lhs, rhs),
        }
    }

    fn primitive(&mut self, op: mir::BinOp, lhs: &hir::Expr, rhs: &hir::Expr) -> mir::Expr {
        let lhs = Box::new(self.lower_expr(lhs));
        let rhs = Box::new(self.lower_expr(rhs));
        mir::Expr::Binary { op, lhs, rhs }
    }

    /// Expand `==` / `!=` on operands of type `ty` (DESIGN 2.3):
    ///
    /// - Int / Boolean: the primitive MIR comparison;
    /// - String: a `scoop_rt_string_eq` call (`!=` wraps it in `!`);
    /// - struct / tuple: per-field comparisons, folded with `&&` for
    ///   `==`; `!=` folds per-field `!=` with `||` — the De Morgan
    ///   dual of the `==` tree, equivalent to negating it because
    ///   field access is pure;
    /// - Unit (the empty tuple): a constant — `() == ()` is always
    ///   `true`, `() != ()` always `false`.
    ///
    /// `path` is the chain of field / element indices from the
    /// top-level operands down to the values compared at this level.
    /// The HIR operands are re-lowered at each leaf; in M2
    /// aggregate-typed expressions are pure (side-effecting calls
    /// return Unit), so re-lowering duplicates structure, not effects.
    fn expand_equality(
        &mut self,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
        ty: hir::TypeId,
        path: &[u32],
        negate: bool,
    ) -> mir::Expr {
        let module = self.module;
        match &module.types[ty] {
            hir::Type::Int => {
                let op = if negate {
                    mir::BinOp::IntNe
                } else {
                    mir::BinOp::IntEq
                };
                self.comparison(op, lhs, rhs, path)
            }
            hir::Type::Boolean => {
                let op = if negate {
                    mir::BinOp::BoolNe
                } else {
                    mir::BinOp::BoolEq
                };
                self.comparison(op, lhs, rhs, path)
            }
            hir::Type::String => {
                let call = mir::Expr::Call(mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::Runtime(mir::RuntimeFn::StringEq),
                    },
                    args: vec![self.accessed(lhs, path), self.accessed(rhs, path)],
                });
                if negate {
                    mir::Expr::Unary {
                        op: mir::UnOp::BoolNot,
                        operand: Box::new(call),
                    }
                } else {
                    call
                }
            }
            hir::Type::Unit => mir::Expr::BoolLiteral(!negate),
            hir::Type::Struct(id) => {
                let field_types: Vec<hir::TypeId> = module.structs[*id]
                    .fields
                    .iter()
                    .map(|field| field.ty)
                    .collect();
                self.expand_fields(lhs, rhs, &field_types, path, negate)
            }
            hir::Type::Tuple(elements) => {
                let elements = elements.clone();
                self.expand_fields(lhs, rhs, &elements, path, negate)
            }
        }
    }

    /// Fold the per-field comparisons of an aggregate equality: `&&`
    /// over `==` leaves for `==`, `||` over `!=` leaves for `!=`; an
    /// empty aggregate compares as the corresponding constant.
    fn expand_fields(
        &mut self,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
        field_types: &[hir::TypeId],
        path: &[u32],
        negate: bool,
    ) -> mir::Expr {
        let mut folded: Option<mir::Expr> = None;
        for (index, &field_ty) in field_types.iter().enumerate() {
            let mut field_path = path.to_vec();
            field_path.push(index as u32);
            let comparison = self.expand_equality(lhs, rhs, field_ty, &field_path, negate);
            folded = Some(match folded {
                None => comparison,
                Some(acc) => mir::Expr::Binary {
                    op: if negate {
                        mir::BinOp::Or
                    } else {
                        mir::BinOp::And
                    },
                    lhs: Box::new(acc),
                    rhs: Box::new(comparison),
                },
            });
        }
        folded.unwrap_or(mir::Expr::BoolLiteral(!negate))
    }

    /// Primitive comparison of the operand sub-values at `path`.
    fn comparison(
        &mut self,
        op: mir::BinOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
        path: &[u32],
    ) -> mir::Expr {
        let lhs = Box::new(self.accessed(lhs, path));
        let rhs = Box::new(self.accessed(rhs, path));
        mir::Expr::Binary { op, lhs, rhs }
    }

    /// Lower an equality operand and wrap it in the `path` accesses.
    fn accessed(&mut self, expr: &hir::Expr, path: &[u32]) -> mir::Expr {
        let mut lowered = self.lower_expr(expr);
        for &index in path {
            lowered = mir::Expr::FieldAccess {
                receiver: Box::new(lowered),
                index,
            };
        }
        lowered
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoop_ast::Span;

    const SPAN: Span = Span { start: 0, end: 0 };

    /// HIR module shell as hir-lower produces it: well-known types and
    /// the builtin output functions allocated first.
    struct Harness {
        types: Arena<hir::Type>,
        functions: Arena<hir::Function>,
        structs: Arena<hir::StructDecl>,
        top_level: Vec<hir::FunctionId>,
        unit: hir::TypeId,
        int: hir::TypeId,
        boolean: hir::TypeId,
        string: hir::TypeId,
        print: hir::FunctionId,
        println: hir::FunctionId,
    }

    impl Harness {
        fn new() -> Self {
            let mut types = Arena::new();
            let unit = types.alloc(hir::Type::Unit);
            let int = types.alloc(hir::Type::Int);
            let boolean = types.alloc(hir::Type::Boolean);
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
            Harness {
                types,
                functions,
                structs: Arena::new(),
                top_level: vec![print, println],
                unit,
                int,
                boolean,
                string,
                print,
                println,
            }
        }

        fn strukt(&mut self, name: &str, fields: &[(&str, hir::TypeId)]) -> hir::StructId {
            self.structs.alloc(hir::StructDecl {
                name: name.to_string(),
                fields: fields
                    .iter()
                    .map(|(name, ty)| hir::Field {
                        name: name.to_string(),
                        ty: *ty,
                    })
                    .collect(),
                span: SPAN,
            })
        }

        fn tuple(&mut self, elements: &[hir::TypeId]) -> hir::TypeId {
            self.types.alloc(hir::Type::Tuple(elements.to_vec()))
        }

        fn user_fn(&mut self, name: &str, body: hir::Body) -> hir::FunctionId {
            let id = self.functions.alloc(hir::Function {
                name: name.to_string(),
                kind: hir::FunctionKind::User(body),
                span: SPAN,
            });
            self.top_level.push(id);
            id
        }

        fn finish(self, entry: hir::FunctionId) -> hir::Module {
            hir::Module {
                types: self.types,
                functions: self.functions,
                structs: self.structs,
                top_level: self.top_level,
                unit: self.unit,
                int: self.int,
                boolean: self.boolean,
                string: self.string,
                print: self.print,
                println: self.println,
                entry,
            }
        }
    }

    fn local(name: &str, ty: hir::TypeId) -> hir::Local {
        hir::Local {
            name: name.to_string(),
            ty,
            mutable: false,
        }
    }

    fn expr(kind: hir::ExprKind, ty: hir::TypeId) -> hir::Expr {
        hir::Expr {
            kind,
            ty,
            span: SPAN,
        }
    }

    fn stmt(kind: hir::StatementKind) -> hir::Statement {
        hir::Statement { kind, span: SPAN }
    }

    fn val_decl(local: hir::LocalId, init: hir::Expr) -> hir::Statement {
        stmt(hir::StatementKind::ValDecl { local, init })
    }

    fn expr_stmt(expr: hir::Expr) -> hir::Statement {
        stmt(hir::StatementKind::Expr(expr))
    }

    fn int_lit(h: &Harness, value: i64) -> hir::Expr {
        expr(hir::ExprKind::IntLiteral(value), h.int)
    }

    fn bool_lit(h: &Harness, value: bool) -> hir::Expr {
        expr(hir::ExprKind::BoolLiteral(value), h.boolean)
    }

    fn str_lit(h: &Harness, value: &str) -> hir::Expr {
        expr(hir::ExprKind::StringLiteral(value.to_string()), h.string)
    }

    fn local_ref(id: hir::LocalId, ty: hir::TypeId) -> hir::Expr {
        expr(hir::ExprKind::Local(id), ty)
    }

    fn binary(op: hir::BinOp, lhs: hir::Expr, rhs: hir::Expr, ty: hir::TypeId) -> hir::Expr {
        expr(
            hir::ExprKind::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            ty,
        )
    }

    fn call(h: &Harness, function: hir::FunctionId, args: Vec<hir::Expr>) -> hir::Expr {
        expr(hir::ExprKind::Call { function, args }, h.unit)
    }

    fn struct_init(struct_id: hir::StructId, ty: hir::TypeId, args: Vec<hir::Expr>) -> hir::Expr {
        expr(hir::ExprKind::StructInit { struct_id, args }, ty)
    }

    /// `main` calls `println("hello, world")` then `helper()`, which
    /// calls `print("!")`.
    fn hello_world() -> hir::Module {
        let mut h = Harness::new();
        let helper = h.user_fn(
            "helper",
            hir::Body {
                locals: Arena::new(),
                statements: vec![expr_stmt(call(&h, h.print, vec![str_lit(&h, "!")]))],
            },
        );
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: vec![
                    expr_stmt(call(&h, h.println, vec![str_lit(&h, "hello, world")])),
                    expr_stmt(call(&h, helper, vec![])),
                ],
            },
        );
        h.finish(main)
    }

    #[test]
    fn lowers_hello_world() {
        let module = lower(&hello_world());

        // Builtins are excluded from `top_level`; declaration order kept.
        assert_eq!(module.top_level.len(), 2);
        let helper = &module.functions[module.top_level[0]];
        let main = &module.functions[module.top_level[1]];
        assert_eq!(helper.name, "helper");
        assert_eq!(main.name, "main");

        // Mangling: entry is the fixed `scoop_main`, others `scoop.<name>`.
        assert_eq!(main.symbol, mir::ENTRY_SYMBOL);
        assert_eq!(helper.symbol, "scoop.helper");
        assert_eq!(module.entry, module.top_level[1]);

        // String literals became numbered global constants (in lowering
        // order: function bodies are lowered in declaration order).
        let strings: Vec<(&str, &str)> = module
            .strings
            .iter()
            .map(|(_, s)| (s.value.as_str(), s.symbol.as_str()))
            .collect();
        assert_eq!(
            strings,
            [("!", "scoop.str.0"), ("hello, world", "scoop.str.1")]
        );

        // M2 meta exists but is empty.
        assert!(module.meta.dispatch_tables.is_empty());

        // Golden dump locks the output structure.
        let expected = "\
Module
  fun helper @scoop.helper
    Call @scoop_rt_print direct
      StringConst @scoop.str.0
  fun main @scoop_main
    Call @scoop_rt_println direct
      StringConst @scoop.str.1
    Call @scoop.helper direct
  str @scoop.str.0 \"!\"
  str @scoop.str.1 \"hello, world\"
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }

    #[test]
    fn repeated_literals_get_separate_constants_deterministically() {
        let mut hir_module = hello_world();
        // Add another `println("hello, world")` to `main`.
        let println = hir_module.println;
        let string = hir_module.string;
        let unit = hir_module.unit;
        let main_id = hir_module.entry;
        let hir::FunctionKind::User(body) = &mut hir_module.functions[main_id].kind else {
            unreachable!()
        };
        body.statements.push(hir::Statement {
            kind: hir::StatementKind::Expr(hir::Expr {
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
            }),
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

    #[test]
    fn print_and_println_map_to_per_type_runtime_shims() {
        let mut h = Harness::new();
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: vec![
                    expr_stmt(call(&h, h.print, vec![str_lit(&h, "s")])),
                    expr_stmt(call(&h, h.print, vec![int_lit(&h, 1)])),
                    expr_stmt(call(&h, h.print, vec![bool_lit(&h, true)])),
                    expr_stmt(call(&h, h.println, vec![str_lit(&h, "t")])),
                    expr_stmt(call(&h, h.println, vec![int_lit(&h, 2)])),
                    expr_stmt(call(&h, h.println, vec![bool_lit(&h, false)])),
                ],
            },
        );
        let module = lower(&h.finish(main));

        let body = &module.functions[module.entry].body;
        let shims: Vec<mir::RuntimeFn> = body
            .statements
            .iter()
            .map(|statement| {
                let mir::StatementKind::Expr(mir::Expr::Call(call)) = &statement.kind else {
                    panic!("expected a call statement")
                };
                let mir::Callee::Runtime(function) = call.target.callee else {
                    panic!("expected a runtime callee")
                };
                function
            })
            .collect();
        assert_eq!(
            shims,
            [
                mir::RuntimeFn::PrintString,
                mir::RuntimeFn::PrintInt,
                mir::RuntimeFn::PrintBoolean,
                mir::RuntimeFn::PrintlnString,
                mir::RuntimeFn::PrintlnInt,
                mir::RuntimeFn::PrintlnBoolean,
            ]
        );
    }

    #[test]
    fn string_plus_lowers_to_runtime_concat() {
        let mut h = Harness::new();
        let mut locals = Arena::new();
        let s = locals.alloc(local("s", h.string));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![val_decl(
                    s,
                    binary(
                        hir::BinOp::Add,
                        str_lit(&h, "a"),
                        str_lit(&h, "b"),
                        h.string,
                    ),
                )],
            },
        );
        let module = lower(&h.finish(main));

        let body = &module.functions[module.entry].body;
        let mir::StatementKind::ValDecl { init, .. } = &body.statements[0].kind else {
            panic!("expected a val declaration")
        };
        let mir::Expr::Call(call) = init else {
            panic!("String `+` must become a runtime call")
        };
        assert_eq!(
            call.target.callee,
            mir::Callee::Runtime(mir::RuntimeFn::StringConcat)
        );
        assert!(matches!(
            call.args.as_slice(),
            [mir::Expr::StringConst(_), mir::Expr::StringConst(_)]
        ));
    }

    #[test]
    fn primitive_operators_map_to_primitive_mir_ops() {
        let mut h = Harness::new();
        let mut statements = Vec::new();
        let int_cases = [
            (hir::BinOp::Add, mir::BinOp::IntAdd),
            (hir::BinOp::Sub, mir::BinOp::IntSub),
            (hir::BinOp::Mul, mir::BinOp::IntMul),
            (hir::BinOp::Div, mir::BinOp::IntDiv),
            (hir::BinOp::Lt, mir::BinOp::IntLt),
            (hir::BinOp::Le, mir::BinOp::IntLe),
            (hir::BinOp::Gt, mir::BinOp::IntGt),
            (hir::BinOp::Ge, mir::BinOp::IntGe),
            (hir::BinOp::Eq, mir::BinOp::IntEq),
            (hir::BinOp::Ne, mir::BinOp::IntNe),
        ];
        for (hir_op, _) in &int_cases {
            let ty = if matches!(
                hir_op,
                hir::BinOp::Add | hir::BinOp::Sub | hir::BinOp::Mul | hir::BinOp::Div
            ) {
                h.int
            } else {
                h.boolean
            };
            statements.push(expr_stmt(binary(
                *hir_op,
                int_lit(&h, 1),
                int_lit(&h, 2),
                ty,
            )));
        }
        let bool_cases = [
            (hir::BinOp::Eq, mir::BinOp::BoolEq),
            (hir::BinOp::Ne, mir::BinOp::BoolNe),
            (hir::BinOp::And, mir::BinOp::And),
            (hir::BinOp::Or, mir::BinOp::Or),
        ];
        for (hir_op, _) in &bool_cases {
            statements.push(expr_stmt(binary(
                *hir_op,
                bool_lit(&h, true),
                bool_lit(&h, false),
                h.boolean,
            )));
        }
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements,
            },
        );
        let module = lower(&h.finish(main));

        let expected: Vec<mir::BinOp> = int_cases
            .iter()
            .chain(bool_cases.iter())
            .map(|(_, mir_op)| *mir_op)
            .collect();
        let body = &module.functions[module.entry].body;
        let ops: Vec<mir::BinOp> = body
            .statements
            .iter()
            .map(|statement| {
                let mir::StatementKind::Expr(mir::Expr::Binary { op, .. }) = &statement.kind else {
                    panic!("expected a binary expression")
                };
                *op
            })
            .collect();
        assert_eq!(ops, expected);
    }

    #[test]
    fn unary_operators_map_to_mir_unops() {
        let mut h = Harness::new();
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: vec![
                    expr_stmt(expr(
                        hir::ExprKind::Unary {
                            op: hir::UnOp::Neg,
                            operand: Box::new(int_lit(&h, 1)),
                        },
                        h.int,
                    )),
                    expr_stmt(expr(
                        hir::ExprKind::Unary {
                            op: hir::UnOp::Not,
                            operand: Box::new(bool_lit(&h, true)),
                        },
                        h.boolean,
                    )),
                ],
            },
        );
        let module = lower(&h.finish(main));

        let body = &module.functions[module.entry].body;
        let ops: Vec<mir::UnOp> = body
            .statements
            .iter()
            .map(|statement| {
                let mir::StatementKind::Expr(mir::Expr::Unary { op, .. }) = &statement.kind else {
                    panic!("expected a unary expression")
                };
                *op
            })
            .collect();
        assert_eq!(ops, [mir::UnOp::IntNeg, mir::UnOp::BoolNot]);
    }

    #[test]
    fn string_equality_lowers_to_runtime_eq() {
        let mut h = Harness::new();
        let mut locals = Arena::new();
        let e = locals.alloc(local("e", h.boolean));
        let n = locals.alloc(local("n", h.boolean));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![
                    val_decl(
                        e,
                        binary(
                            hir::BinOp::Eq,
                            str_lit(&h, "a"),
                            str_lit(&h, "b"),
                            h.boolean,
                        ),
                    ),
                    val_decl(
                        n,
                        binary(
                            hir::BinOp::Ne,
                            str_lit(&h, "a"),
                            str_lit(&h, "b"),
                            h.boolean,
                        ),
                    ),
                ],
            },
        );
        let module = lower(&h.finish(main));

        let body = &module.functions[module.entry].body;
        let mir::StatementKind::ValDecl { init: eq, .. } = &body.statements[0].kind else {
            panic!("expected a val declaration")
        };
        let mir::Expr::Call(call) = eq else {
            panic!("String `==` must become a runtime call")
        };
        assert_eq!(
            call.target.callee,
            mir::Callee::Runtime(mir::RuntimeFn::StringEq)
        );

        // `!=` wraps the same call in a boolean negation.
        let mir::StatementKind::ValDecl { init: ne, .. } = &body.statements[1].kind else {
            panic!("expected a val declaration")
        };
        let mir::Expr::Unary {
            op: mir::UnOp::BoolNot,
            operand,
        } = ne
        else {
            panic!("String `!=` must negate the equality call")
        };
        assert!(matches!(
            operand.as_ref(),
            mir::Expr::Call(mir::Call {
                target: mir::CallTarget {
                    callee: mir::Callee::Runtime(mir::RuntimeFn::StringEq),
                    ..
                },
                ..
            })
        ));
    }

    #[test]
    fn unit_equality_is_constant() {
        let mut h = Harness::new();
        let mut locals = Arena::new();
        let b = locals.alloc(local("b", h.boolean));
        let c = locals.alloc(local("c", h.boolean));
        let unit_lit = |h: &Harness| expr(hir::ExprKind::UnitLiteral, h.unit);
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![
                    val_decl(
                        b,
                        binary(hir::BinOp::Eq, unit_lit(&h), unit_lit(&h), h.boolean),
                    ),
                    val_decl(
                        c,
                        binary(hir::BinOp::Ne, unit_lit(&h), unit_lit(&h), h.boolean),
                    ),
                ],
            },
        );
        let module = lower(&h.finish(main));

        let body = &module.functions[module.entry].body;
        let mir::StatementKind::ValDecl { init: eq, .. } = &body.statements[0].kind else {
            panic!("expected a val declaration")
        };
        assert!(matches!(eq, mir::Expr::BoolLiteral(true)));
        let mir::StatementKind::ValDecl { init: ne, .. } = &body.statements[1].kind else {
            panic!("expected a val declaration")
        };
        assert!(matches!(ne, mir::Expr::BoolLiteral(false)));
    }

    #[test]
    fn struct_equality_expands_into_per_field_comparisons() {
        let mut h = Harness::new();
        let point = h.strukt("Point", &[("x", h.int), ("y", h.int)]);
        let point_ty = h.types.alloc(hir::Type::Struct(point));
        let mut locals = Arena::new();
        let p = locals.alloc(local("p", point_ty));
        let q = locals.alloc(local("q", point_ty));
        let b = locals.alloc(local("b", h.boolean));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![
                    val_decl(
                        p,
                        struct_init(point, point_ty, vec![int_lit(&h, 1), int_lit(&h, 2)]),
                    ),
                    val_decl(
                        q,
                        struct_init(point, point_ty, vec![int_lit(&h, 3), int_lit(&h, 4)]),
                    ),
                    val_decl(
                        b,
                        binary(
                            hir::BinOp::Eq,
                            local_ref(p, point_ty),
                            local_ref(q, point_ty),
                            h.boolean,
                        ),
                    ),
                ],
            },
        );
        let module = lower(&h.finish(main));

        // The struct arena is transposed in declaration order.
        assert_eq!(module.structs.len(), 1);

        let expected = "\
Module
  struct Point (x: Int, y: Int)
  fun main @scoop_main
    val p: Point
      StructInit Point
        IntLiteral 1
        IntLiteral 2
    val q: Point
      StructInit Point
        IntLiteral 3
        IntLiteral 4
    val b: Boolean
      Binary And
        Binary IntEq
          FieldAccess 0
            Local p
          FieldAccess 0
            Local q
        Binary IntEq
          FieldAccess 1
            Local p
          FieldAccess 1
            Local q
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }

    #[test]
    fn nested_aggregate_inequality_expands_recursively() {
        // struct Wrap(val tag: String, val pair: (Int, Boolean))
        let mut h = Harness::new();
        let pair = h.tuple(&[h.int, h.boolean]);
        let wrap = h.strukt("Wrap", &[("tag", h.string), ("pair", pair)]);
        let wrap_ty = h.types.alloc(hir::Type::Struct(wrap));
        let mut locals = Arena::new();
        let w1 = locals.alloc(local("w1", wrap_ty));
        let w2 = locals.alloc(local("w2", wrap_ty));
        let r = locals.alloc(local("r", h.boolean));
        let tuple_lit =
            |elements: Vec<hir::Expr>| expr(hir::ExprKind::TupleLiteral(elements), pair);
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![
                    val_decl(
                        w1,
                        struct_init(
                            wrap,
                            wrap_ty,
                            vec![
                                str_lit(&h, "a"),
                                tuple_lit(vec![int_lit(&h, 1), bool_lit(&h, true)]),
                            ],
                        ),
                    ),
                    val_decl(
                        w2,
                        struct_init(
                            wrap,
                            wrap_ty,
                            vec![
                                str_lit(&h, "b"),
                                tuple_lit(vec![int_lit(&h, 2), bool_lit(&h, false)]),
                            ],
                        ),
                    ),
                    val_decl(
                        r,
                        binary(
                            hir::BinOp::Ne,
                            local_ref(w1, wrap_ty),
                            local_ref(w2, wrap_ty),
                            h.boolean,
                        ),
                    ),
                ],
            },
        );
        let module = lower(&h.finish(main));

        // `!=` folds per-field `!=` with `||`; the String field goes
        // through `scoop_rt_string_eq` negated, the nested tuple
        // recurses into per-element comparisons.
        let expected = "\
Module
  struct Wrap (tag: String, pair: (Int, Boolean))
  fun main @scoop_main
    val w1: Wrap
      StructInit Wrap
        StringConst @scoop.str.0
        TupleLiteral
          IntLiteral 1
          BoolLiteral true
    val w2: Wrap
      StructInit Wrap
        StringConst @scoop.str.1
        TupleLiteral
          IntLiteral 2
          BoolLiteral false
    val r: Boolean
      Binary Or
        Unary BoolNot
          Call @scoop_rt_string_eq direct
            FieldAccess 0
              Local w1
            FieldAccess 0
              Local w2
        Binary Or
          Binary IntNe
            FieldAccess 0
              FieldAccess 1
                Local w1
            FieldAccess 0
              FieldAccess 1
                Local w2
          Binary BoolNe
            FieldAccess 1
              FieldAccess 1
                Local w1
            FieldAccess 1
              FieldAccess 1
                Local w2
  str @scoop.str.0 \"a\"
  str @scoop.str.1 \"b\"
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }

    #[test]
    fn tuple_equality_expands_per_element() {
        let mut h = Harness::new();
        let pair = h.tuple(&[h.int, h.string]);
        let mut locals = Arena::new();
        let t1 = locals.alloc(local("t1", pair));
        let t2 = locals.alloc(local("t2", pair));
        let b = locals.alloc(local("b", h.boolean));
        let tuple_lit =
            |elements: Vec<hir::Expr>| expr(hir::ExprKind::TupleLiteral(elements), pair);
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![
                    val_decl(t1, tuple_lit(vec![int_lit(&h, 1), str_lit(&h, "x")])),
                    val_decl(t2, tuple_lit(vec![int_lit(&h, 2), str_lit(&h, "y")])),
                    val_decl(
                        b,
                        binary(
                            hir::BinOp::Eq,
                            local_ref(t1, pair),
                            local_ref(t2, pair),
                            h.boolean,
                        ),
                    ),
                ],
            },
        );
        let module = lower(&h.finish(main));

        let expected = "\
Module
  fun main @scoop_main
    val t1: (Int, String)
      TupleLiteral
        IntLiteral 1
        StringConst @scoop.str.0
    val t2: (Int, String)
      TupleLiteral
        IntLiteral 2
        StringConst @scoop.str.1
    val b: Boolean
      Binary And
        Binary IntEq
          FieldAccess 0
            Local t1
          FieldAccess 0
            Local t2
        Call @scoop_rt_string_eq direct
          FieldAccess 1
            Local t1
          FieldAccess 1
            Local t2
  str @scoop.str.0 \"x\"
  str @scoop.str.1 \"y\"
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }

    #[test]
    fn field_access_uses_zero_based_indices() {
        let mut h = Harness::new();
        let point = h.strukt("Point", &[("x", h.int), ("y", h.int)]);
        let point_ty = h.types.alloc(hir::Type::Struct(point));
        let pair = h.tuple(&[h.int, h.string]);
        let mut locals = Arena::new();
        let p = locals.alloc(local("p", point_ty));
        let t = locals.alloc(local("t", pair));
        let y = locals.alloc(local("y", h.int));
        let s = locals.alloc(local("s", h.string));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![
                    // `p.y`
                    val_decl(
                        y,
                        expr(
                            hir::ExprKind::FieldAccess {
                                receiver: Box::new(local_ref(p, point_ty)),
                                field: hir::FieldRef::StructField {
                                    struct_id: point,
                                    index: 1,
                                },
                            },
                            h.int,
                        ),
                    ),
                    // `t._2`
                    val_decl(
                        s,
                        expr(
                            hir::ExprKind::FieldAccess {
                                receiver: Box::new(local_ref(t, pair)),
                                field: hir::FieldRef::TupleIndex(1),
                            },
                            h.string,
                        ),
                    ),
                ],
            },
        );
        let module = lower(&h.finish(main));

        let body = &module.functions[module.entry].body;
        for statement in &body.statements {
            let mir::StatementKind::ValDecl { init, .. } = &statement.kind else {
                panic!("expected a val declaration")
            };
            assert!(matches!(init, mir::Expr::FieldAccess { index: 1, .. }));
        }
    }

    #[test]
    fn control_flow_stays_structured() {
        let mut h = Harness::new();
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: vec![
                    stmt(hir::StatementKind::If {
                        cond: bool_lit(&h, true),
                        then_body: vec![expr_stmt(call(&h, h.println, vec![str_lit(&h, "a")]))],
                        else_body: Some(vec![expr_stmt(call(
                            &h,
                            h.println,
                            vec![str_lit(&h, "b")],
                        ))]),
                    }),
                    stmt(hir::StatementKind::While {
                        cond: bool_lit(&h, false),
                        body: vec![],
                    }),
                ],
            },
        );
        let module = lower(&h.finish(main));

        let body = &module.functions[module.entry].body;
        let mir::StatementKind::If {
            then_body,
            else_body,
            ..
        } = &body.statements[0].kind
        else {
            panic!("if must stay a structured MIR statement")
        };
        assert_eq!(then_body.len(), 1);
        assert_eq!(else_body.as_ref().map(Vec::len), Some(1));
        assert!(matches!(
            &body.statements[1].kind,
            mir::StatementKind::While { body, .. } if body.is_empty()
        ));
    }
}
