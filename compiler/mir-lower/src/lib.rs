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
//!
//! M3: monomorphization and Option. Generic functions have no MIR body
//! of their own; each instantiation request `(generic fn, concrete
//! type args)` produces one instance whose body is the generic body
//! with `Param(i)` substituted by `type_args[i]` (recursively through
//! tuple / Option types). Requests discovered while lowering an
//! instance body (generic functions calling generic functions) extend
//! a worklist that is drained to a fixed point; identical requests are
//! deduplicated by mangled symbol. Function parameters, return types
//! and `return` statements pass through, as do the Option nodes
//! (`SomeWrap` / `NoneLiteral` / `IsSome` / `Unwrap`) — the Option
//! representation is LIR's layout decision. Equality on `Option<T>`
//! is expanded here into tag tests plus a payload comparison.

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
        instances: InstanceRegistry::default(),
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
    /// HIR user function -> MIR function (non-generic functions only;
    /// generic functions resolve through `instances`).
    function_map: HashMap<hir::FunctionId, mir::FunctionId>,
    instances: InstanceRegistry,
}

impl Lowerer {
    fn run(mut self, module: &hir::Module) -> mir::Module {
        self.lower_structs(module);
        let shell = mangling_shell(&self.structs);

        // Declare non-generic user functions first, so calls resolve
        // regardless of declaration order. Builtins have no body; their
        // callsites map to `Callee::Runtime` shims (see `print_fn`).
        // Generic functions have no MIR body of their own — only their
        // monomorphized instances do.
        let mut user_functions = Vec::new();
        for &hir_id in &module.top_level {
            let function = &module.functions[hir_id];
            if !matches!(function.kind, hir::FunctionKind::User(_)) {
                continue;
            }
            if !function.type_params.is_empty() {
                continue;
            }
            let id = self.functions.alloc(mir::Function {
                name: function.name.clone(),
                // Non-generic mangling: `scoop.<name>`, or the fixed
                // entry symbol `scoop_main` that the C runtime calls
                // (`main` is never generic, hir-lower guarantees it).
                symbol: mir::mangle_function(&function.name, hir_id == module.entry),
                // Filled in when the body is lowered below.
                params: Vec::new(),
                return_ty: mir::Type::Unit,
                body: mir::Body {
                    locals: Arena::new(),
                    statements: Vec::new(),
                },
            });
            self.top_level.push(id);
            self.function_map.insert(hir_id, id);
            user_functions.push((hir_id, id));
        }

        for (hir_id, mir_id) in user_functions {
            let (params, return_ty, body) = self.lower_user_function(module, hir_id, None, &shell);
            let function = &mut self.functions[mir_id];
            function.params = params;
            function.return_ty = return_ty;
            function.body = body;
        }

        // Seed the instance worklist from HIR's instantiation requests.
        // Requests whose type arguments still mention `Param` come from
        // generic bodies calling generic functions; they are
        // rediscovered in concrete form when the enclosing instance
        // body is lowered, so only concrete requests are seeded here.
        for instantiation in &module.instantiations {
            if !instantiation
                .type_args
                .iter()
                .all(|&ty| is_concrete(module, ty))
            {
                continue;
            }
            let type_args: Vec<mir::Type> = instantiation
                .type_args
                .iter()
                .map(|&ty| lower_type(module, &self.struct_map, ty, None))
                .collect();
            self.instances.get_or_create(
                module,
                &mut self.functions,
                &mut self.top_level,
                &shell,
                instantiation.function,
                type_args,
            );
        }

        // Drain the worklist: lowering an instance body can discover
        // further instances (generic functions calling generic
        // functions), which get appended to `pending`.
        let mut next = 0;
        while next < self.instances.pending.len() {
            let (hir_id, type_args, mir_id) = self.instances.pending[next].clone();
            next += 1;
            let (params, return_ty, body) =
                self.lower_user_function(module, hir_id, Some(&type_args), &shell);
            let function = &mut self.functions[mir_id];
            function.params = params;
            function.return_ty = return_ty;
            function.body = body;
        }

        // The entry point is a non-generic user function, hence always
        // in the map.
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

    /// Lower one user function; `subst` is the concrete type argument
    /// list when lowering a monomorphized instance (`None` for
    /// non-generic functions).
    fn lower_user_function(
        &mut self,
        module: &hir::Module,
        hir_id: hir::FunctionId,
        subst: Option<&[mir::Type]>,
        shell: &mir::Module,
    ) -> (Vec<mir::Param>, mir::Type, mir::Body) {
        let function = &module.functions[hir_id];
        let hir::FunctionKind::User(body) = &function.kind else {
            unreachable!("only user functions have MIR bodies")
        };
        BodyLowerer {
            module,
            struct_map: &self.struct_map,
            structs: &self.structs,
            function_map: &self.function_map,
            strings: &mut self.strings,
            functions: &mut self.functions,
            top_level: &mut self.top_level,
            instances: &mut self.instances,
            shell,
            subst,
            local_map: HashMap::new(),
        }
        .lower_function(function, body)
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
                    // Struct declarations are not generic in M3, so
                    // field types never mention `Param`.
                    ty: lower_type(module, &self.struct_map, field.ty, None),
                })
                .collect();
            self.structs[self.struct_map[&hir_id]].fields = fields;
        }
    }
}

/// `mir::mangle_instance` takes `&mir::Module` but only ever reads
/// struct names (via `encode_type`); this shell provides exactly those.
/// Its struct arena shares the real arena's declaration order, so
/// struct ids align.
fn mangling_shell(structs: &Arena<mir::StructDef>) -> mir::Module {
    let mut shell_structs = Arena::new();
    for (_, def) in structs.iter() {
        shell_structs.alloc(mir::StructDef {
            name: def.name.clone(),
            fields: Vec::new(),
        });
    }
    let mut functions = Arena::new();
    let entry = functions.alloc(mir::Function {
        name: String::new(),
        symbol: String::new(),
        params: Vec::new(),
        return_ty: mir::Type::Unit,
        body: mir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    });
    mir::Module {
        functions,
        top_level: Vec::new(),
        strings: Arena::new(),
        structs: shell_structs,
        entry,
        meta: mir::MirMeta::default(),
    }
}

/// Whether a HIR type mentions no type parameters.
fn is_concrete(module: &hir::Module, ty: hir::TypeId) -> bool {
    match &module.types[ty] {
        hir::Type::Param(_) => false,
        hir::Type::Tuple(elements) => elements.iter().all(|&e| is_concrete(module, e)),
        hir::Type::Option(inner) => is_concrete(module, *inner),
        _ => true,
    }
}

/// Monomorphized instances: creation, deduplication, and the body
/// worklist (DESIGN 2.3).
#[derive(Default)]
struct InstanceRegistry {
    /// Mangled symbol -> instance. The symbol encodes the function and
    /// its type arguments, so it is the deduplication key: one
    /// instance per `(generic fn, concrete type args)` per Cone.
    by_symbol: HashMap<String, mir::FunctionId>,
    /// Instances whose bodies still have to be lowered: (source
    /// function, concrete type arguments, instance id).
    pending: Vec<(hir::FunctionId, Vec<mir::Type>, mir::FunctionId)>,
}

impl InstanceRegistry {
    fn get_or_create(
        &mut self,
        module: &hir::Module,
        functions: &mut Arena<mir::Function>,
        top_level: &mut Vec<mir::FunctionId>,
        shell: &mir::Module,
        hir_id: hir::FunctionId,
        type_args: Vec<mir::Type>,
    ) -> mir::FunctionId {
        let function = &module.functions[hir_id];
        let symbol = mir::mangle_instance(shell, &function.name, &type_args);
        if let Some(&id) = self.by_symbol.get(&symbol) {
            return id;
        }
        let id = functions.alloc(mir::Function {
            name: function.name.clone(),
            symbol: symbol.clone(),
            // Filled in when the instance body is lowered.
            params: Vec::new(),
            return_ty: mir::Type::Unit,
            body: mir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            },
        });
        top_level.push(id);
        self.by_symbol.insert(symbol, id);
        self.pending.push((hir_id, type_args, id));
        id
    }
}

/// Map a HIR type onto its MIR type. Aggregate shapes are preserved:
/// structs keep their (remapped) id, tuples and Options their mapped
/// element / payload types. `Param(i)` resolves through `subst`, the
/// concrete type arguments of the instance being lowered; non-generic
/// bodies never contain it.
fn lower_type(
    module: &hir::Module,
    struct_map: &HashMap<hir::StructId, mir::StructId>,
    ty: hir::TypeId,
    subst: Option<&[mir::Type]>,
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
                .map(|&element| lower_type(module, struct_map, element, subst))
                .collect(),
        ),
        hir::Type::Option(inner) => {
            mir::Type::Option(Box::new(lower_type(module, struct_map, *inner, subst)))
        }
        hir::Type::Param(index) => subst
            .expect("hir::Type::Param only appears in generic function bodies")[*index as usize]
            .clone(),
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
    /// MIR struct arena (field types for the equality expansion).
    structs: &'a Arena<mir::StructDef>,
    function_map: &'a HashMap<hir::FunctionId, mir::FunctionId>,
    strings: &'a mut Arena<mir::StringConst>,
    functions: &'a mut Arena<mir::Function>,
    top_level: &'a mut Vec<mir::FunctionId>,
    instances: &'a mut InstanceRegistry,
    shell: &'a mir::Module,
    /// Concrete type arguments of the instance being lowered; `None`
    /// for non-generic bodies (which never mention `Param`).
    subst: Option<&'a [mir::Type]>,
    /// HIR local -> MIR local (same declaration order per body).
    local_map: HashMap<hir::LocalId, mir::LocalId>,
}

/// A step from a compared operand down to the sub-value at an equality
/// leaf: a struct field / tuple element access, or `unwrap` into an
/// `Option` payload.
#[derive(Clone, Copy)]
enum Access {
    Field(u32),
    Unwrap,
}

impl BodyLowerer<'_> {
    fn lower_function(
        mut self,
        function: &hir::Function,
        body: &hir::Body,
    ) -> (Vec<mir::Param>, mir::Type, mir::Body) {
        let mut locals = Arena::new();
        for (hir_id, local) in body.locals.iter() {
            let mir_id = locals.alloc(mir::Local {
                name: local.name.clone(),
                ty: self.lower_type(local.ty),
                mutable: local.mutable,
            });
            self.local_map.insert(hir_id, mir_id);
        }
        let params = function
            .params
            .iter()
            .map(|param| mir::Param {
                name: param.name.clone(),
                ty: self.lower_type(param.ty),
                local: self.local_map[&param.local],
            })
            .collect();
        let return_ty = self.lower_type(function.return_ty);
        let statements = self.lower_statements(&body.statements);
        (params, return_ty, mir::Body { locals, statements })
    }

    fn lower_type(&self, ty: hir::TypeId) -> mir::Type {
        lower_type(self.module, self.struct_map, ty, self.subst)
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
            hir::StatementKind::Return { value } => mir::StatementKind::Return {
                value: value.as_ref().map(|value| self.lower_expr(value)),
            },
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
            hir::ExprKind::Call {
                function,
                type_args,
                args,
            } => self.lower_call(*function, type_args, args),
            hir::ExprKind::Binary { op, lhs, rhs } => self.lower_binary(*op, lhs, rhs),
            hir::ExprKind::Unary { op, operand } => {
                let operand = Box::new(self.lower_expr(operand));
                let op = match op {
                    hir::UnOp::Neg => mir::UnOp::IntNeg,
                    hir::UnOp::Not => mir::UnOp::BoolNot,
                };
                mir::Expr::Unary { op, operand }
            }
            // The Option nodes pass through unchanged; the
            // representation (niche vs. tagged) is LIR's layout
            // decision (DESIGN 2.3).
            hir::ExprKind::SomeWrap(operand) => {
                mir::Expr::SomeWrap(Box::new(self.lower_expr(operand)))
            }
            hir::ExprKind::NoneLiteral => mir::Expr::NoneLiteral,
            hir::ExprKind::IsSome(operand) => {
                // `isSome(None)` folds to `false`; this also keeps MIR
                // `IsSome` from ever wrapping `NoneLiteral`, whose type
                // is not recoverable at LIR (see `tag_leaf`).
                let operand = self.lower_expr(operand);
                if matches!(operand, mir::Expr::NoneLiteral) {
                    mir::Expr::BoolLiteral(false)
                } else {
                    mir::Expr::IsSome(Box::new(operand))
                }
            }
            hir::ExprKind::Unwrap {
                operand,
                trap_on_none,
            } => mir::Expr::Unwrap {
                operand: Box::new(self.lower_expr(operand)),
                trap_on_none: *trap_on_none,
            },
        }
    }

    fn lower_call(
        &mut self,
        function: hir::FunctionId,
        type_args: &[hir::TypeId],
        args: &[hir::Expr],
    ) -> mir::Expr {
        let callee = if function == self.module.print || function == self.module.println {
            // hir-lower enforces exactly one argument (DESIGN 5.2).
            let newline = function == self.module.println;
            mir::Callee::Runtime(print_fn(self.module, args[0].ty, newline))
        } else if self.module.functions[function].type_params.is_empty() {
            mir::Callee::User(self.function_map[&function])
        } else {
            // Generic callee: the call's type arguments may mention the
            // enclosing instance's `Param`s; substitution concretizes
            // them, and the instance is created on demand (its body is
            // lowered when the worklist drains).
            let type_args: Vec<mir::Type> =
                type_args.iter().map(|&ty| self.lower_type(ty)).collect();
            mir::Callee::User(self.instances.get_or_create(
                self.module,
                self.functions,
                self.top_level,
                self.shell,
                function,
                type_args,
            ))
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
            // Structural equality dispatches on the concrete
            // (monomorphized) operand type.
            hir::BinOp::Eq => {
                let ty = self.lower_type(lhs.ty);
                self.expand_equality(lhs, rhs, &ty, &[], false)
            }
            hir::BinOp::Ne => {
                let ty = self.lower_type(lhs.ty);
                self.expand_equality(lhs, rhs, &ty, &[], true)
            }
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

    /// Expand `==` / `!=` on operands of the concrete (monomorphized)
    /// type `ty` (DESIGN 2.3):
    ///
    /// - Int / Boolean: the primitive MIR comparison;
    /// - String: a `scoop_rt_string_eq` call (`!=` wraps it in `!`);
    /// - struct / tuple: per-field comparisons, folded with `&&` for
    ///   `==`; `!=` folds per-field `!=` with `||` — the De Morgan
    ///   dual of the `==` tree, equivalent to negating it because
    ///   field access is pure;
    /// - `Option<T>`: `(isSome(a) && isSome(b) && unwrap(a) == unwrap(b))
    ///   || (!isSome(a) && !isSome(b))`; `!=` is again the De Morgan
    ///   dual. The payload comparison recurses through this same
    ///   expansion at `path + [Unwrap]`;
    /// - Unit (the empty tuple): a constant — `() == ()` is always
    ///   `true`, `() != ()` always `false`.
    ///
    /// `path` is the chain of field accesses and `unwrap`s from the
    /// top-level operands down to the values compared at this level.
    /// The HIR operands are re-lowered at each leaf; this duplicates
    /// structure, not effects, because everything that can appear as
    /// an operand here is pure (M2 assumption, still valid in M3):
    /// field access, `isSome` / `unwrap`, and calls — value-returning
    /// calls (new in M3) are treated as pure by convention, and
    /// Unit-returning calls cannot appear because both operands share
    /// the compared type. If impure value-returning calls ever become
    /// observable, this expansion must route the operands through
    /// hidden temporaries instead of re-lowering them.
    fn expand_equality(
        &mut self,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
        ty: &mir::Type,
        path: &[Access],
        negate: bool,
    ) -> mir::Expr {
        match ty {
            mir::Type::Int => {
                let op = if negate {
                    mir::BinOp::IntNe
                } else {
                    mir::BinOp::IntEq
                };
                self.comparison(op, lhs, rhs, path)
            }
            mir::Type::Boolean => {
                let op = if negate {
                    mir::BinOp::BoolNe
                } else {
                    mir::BinOp::BoolEq
                };
                self.comparison(op, lhs, rhs, path)
            }
            mir::Type::String => {
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
            mir::Type::Unit => mir::Expr::BoolLiteral(!negate),
            mir::Type::Struct(id) => {
                let field_types: Vec<mir::Type> = self.structs[*id]
                    .fields
                    .iter()
                    .map(|field| field.ty.clone())
                    .collect();
                self.expand_fields(lhs, rhs, &field_types, path, negate)
            }
            mir::Type::Tuple(elements) => {
                let elements = elements.clone();
                self.expand_fields(lhs, rhs, &elements, path, negate)
            }
            mir::Type::Option(payload) => {
                let payload = payload.as_ref().clone();
                self.expand_option_equality(lhs, rhs, &payload, path, negate)
            }
        }
    }

    /// `Option<T>` equality: both `Some` compares the payloads, both
    /// `None` is equal, a mix is unequal —
    /// `(isSome(a) && isSome(b) && unwrap(a) == unwrap(b))
    ///  || (!isSome(a) && !isSome(b))`.
    /// For `!=` the whole tree is dualized: `And` / `Or` swapped, the
    /// `isSome` leaves negated, the payload compared with `!=`.
    fn expand_option_equality(
        &mut self,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
        payload: &mir::Type,
        path: &[Access],
        negate: bool,
    ) -> mir::Expr {
        let mut payload_path = path.to_vec();
        payload_path.push(Access::Unwrap);
        let payload_comparison = self.expand_equality(lhs, rhs, payload, &payload_path, negate);
        let combine = |a: mir::Expr, b: mir::Expr| mir::Expr::Binary {
            op: if negate {
                mir::BinOp::Or
            } else {
                mir::BinOp::And
            },
            lhs: Box::new(a),
            rhs: Box::new(b),
        };
        let both_some = combine(
            combine(
                self.tag_leaf(lhs, path, !negate),
                self.tag_leaf(rhs, path, !negate),
            ),
            payload_comparison,
        );
        let both_none = combine(
            self.tag_leaf(lhs, path, negate),
            self.tag_leaf(rhs, path, negate),
        );
        mir::Expr::Binary {
            op: if negate {
                mir::BinOp::And
            } else {
                mir::BinOp::Or
            },
            lhs: Box::new(both_some),
            rhs: Box::new(both_none),
        }
    }

    /// An `isSome` leaf of the Option equality tree: `isSome(operand)`
    /// when `positive`, `!isSome(operand)` otherwise. Constant-folded
    /// for a textual `None` operand — which also keeps MIR `IsSome`
    /// from ever wrapping `NoneLiteral`, whose type is not recoverable
    /// at LIR (the node carries no type and the context does not
    /// provide one there).
    fn tag_leaf(&mut self, expr: &hir::Expr, path: &[Access], positive: bool) -> mir::Expr {
        let operand = self.accessed(expr, path);
        if matches!(operand, mir::Expr::NoneLiteral) {
            return mir::Expr::BoolLiteral(!positive);
        }
        let is_some = mir::Expr::IsSome(Box::new(operand));
        if positive {
            is_some
        } else {
            mir::Expr::Unary {
                op: mir::UnOp::BoolNot,
                operand: Box::new(is_some),
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
        field_types: &[mir::Type],
        path: &[Access],
        negate: bool,
    ) -> mir::Expr {
        let mut folded: Option<mir::Expr> = None;
        for (index, field_ty) in field_types.iter().enumerate() {
            let mut field_path = path.to_vec();
            field_path.push(Access::Field(index as u32));
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
        path: &[Access],
    ) -> mir::Expr {
        let lhs = Box::new(self.accessed(lhs, path));
        let rhs = Box::new(self.accessed(rhs, path));
        mir::Expr::Binary { op, lhs, rhs }
    }

    /// Lower an equality operand and wrap it in the `path` accesses.
    /// `Unwrap` steps never trap: the equality tree only evaluates
    /// them once both operands are known to be `Some`.
    fn accessed(&mut self, expr: &hir::Expr, path: &[Access]) -> mir::Expr {
        let mut lowered = self.lower_expr(expr);
        for access in path {
            lowered = match access {
                Access::Field(index) => mir::Expr::FieldAccess {
                    receiver: Box::new(lowered),
                    index: *index,
                },
                Access::Unwrap => mir::Expr::Unwrap {
                    operand: Box::new(lowered),
                    trap_on_none: false,
                },
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
        instantiations: Vec<hir::Instantiation>,
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
                type_params: Vec::new(),
                params: Vec::new(),
                return_ty: unit,
                kind: hir::FunctionKind::Builtin(hir::Builtin::Print),
                span: SPAN,
            });
            let println = functions.alloc(hir::Function {
                name: "println".to_string(),
                type_params: Vec::new(),
                params: Vec::new(),
                return_ty: unit,
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
                instantiations: Vec::new(),
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
            let unit = self.unit;
            self.user_fn_full(name, Vec::new(), Vec::new(), unit, body)
        }

        fn user_fn_full(
            &mut self,
            name: &str,
            type_params: Vec<String>,
            params: Vec<hir::Param>,
            return_ty: hir::TypeId,
            body: hir::Body,
        ) -> hir::FunctionId {
            let id = self.functions.alloc(hir::Function {
                name: name.to_string(),
                type_params,
                params,
                return_ty,
                kind: hir::FunctionKind::User(body),
                span: SPAN,
            });
            self.top_level.push(id);
            id
        }

        fn instantiate(&mut self, function: hir::FunctionId, type_args: Vec<hir::TypeId>) {
            self.instantiations.push(hir::Instantiation {
                function,
                type_args,
            });
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
                instantiations: self.instantiations,
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
        expr(
            hir::ExprKind::Call {
                function,
                type_args: Vec::new(),
                args,
            },
            h.unit,
        )
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
  fun helper @scoop.helper() -> Unit
    Call @scoop_rt_print direct
      StringConst @scoop.str.0
  fun main @scoop_main() -> Unit
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
                    type_args: Vec::new(),
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
  fun main @scoop_main() -> Unit
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
  fun main @scoop_main() -> Unit
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
  fun main @scoop_main() -> Unit
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

    fn generic_call(
        function: hir::FunctionId,
        type_args: Vec<hir::TypeId>,
        args: Vec<hir::Expr>,
        ty: hir::TypeId,
    ) -> hir::Expr {
        expr(
            hir::ExprKind::Call {
                function,
                type_args,
                args,
            },
            ty,
        )
    }

    fn param(name: &str, ty: hir::TypeId, local: hir::LocalId) -> hir::Param {
        hir::Param {
            name: name.to_string(),
            ty,
            local,
        }
    }

    /// `fun <T> name(x: T): T { return x }`.
    fn identity_fn(h: &mut Harness, name: &str) -> hir::FunctionId {
        let t = h.types.alloc(hir::Type::Param(0));
        let mut locals = Arena::new();
        let x = locals.alloc(local("x", t));
        h.user_fn_full(
            name,
            vec!["T".to_string()],
            vec![param("x", t, x)],
            t,
            hir::Body {
                locals,
                statements: vec![stmt(hir::StatementKind::Return {
                    value: Some(local_ref(x, t)),
                })],
            },
        )
    }

    #[test]
    fn params_and_return_translate() {
        let mut h = Harness::new();
        let int = h.int;
        let mut locals = Arena::new();
        let x = locals.alloc(local("x", int));
        let y = locals.alloc(local("y", int));
        // fun add(x: Int, y: Int): Int { return x + y }
        let add = h.user_fn_full(
            "add",
            Vec::new(),
            vec![param("x", int, x), param("y", int, y)],
            int,
            hir::Body {
                locals,
                statements: vec![stmt(hir::StatementKind::Return {
                    value: Some(binary(
                        hir::BinOp::Add,
                        local_ref(x, int),
                        local_ref(y, int),
                        int,
                    )),
                })],
            },
        );
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: vec![expr_stmt(call(
                    &h,
                    add,
                    vec![int_lit(&h, 1), int_lit(&h, 2)],
                ))],
            },
        );
        let module = lower(&h.finish(main));

        let add_fn = &module.functions[module.top_level[0]];
        assert_eq!(add_fn.symbol, "scoop.add");
        assert_eq!(add_fn.params.len(), 2);
        assert_eq!(add_fn.params[0].ty, mir::Type::Int);
        assert_eq!(add_fn.params[1].ty, mir::Type::Int);
        assert_eq!(add_fn.return_ty, mir::Type::Int);
        // Parameters are (the first) locals of the body.
        let px = add_fn.params[0].local;
        assert_eq!(add_fn.body.locals[px].name, "x");
        assert!(matches!(
            &add_fn.body.statements[0].kind,
            mir::StatementKind::Return {
                value: Some(mir::Expr::Binary {
                    op: mir::BinOp::IntAdd,
                    ..
                })
            }
        ));
    }

    #[test]
    fn monomorphizes_generic_functions() {
        let mut h = Harness::new();
        let identity = identity_fn(&mut h, "identity");
        let (int, string) = (h.int, h.string);
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: vec![
                    expr_stmt(generic_call(
                        identity,
                        vec![int],
                        vec![int_lit(&h, 41)],
                        int,
                    )),
                    expr_stmt(generic_call(
                        identity,
                        vec![string],
                        vec![str_lit(&h, "hi")],
                        string,
                    )),
                ],
            },
        );
        h.instantiate(identity, vec![int]);
        h.instantiate(identity, vec![string]);
        let module = lower(&h.finish(main));

        // main first (declaration order), then the instances in
        // creation order. The generic function itself has no MIR body.
        assert_eq!(module.top_level.len(), 3);
        let int_instance = &module.functions[module.top_level[1]];
        let string_instance = &module.functions[module.top_level[2]];
        assert_eq!(int_instance.symbol, "scoop.identity$I");
        assert_eq!(string_instance.symbol, "scoop.identity$S");

        // The instance signature, locals and body are fully
        // substituted — no `Param` survives.
        assert_eq!(int_instance.params.len(), 1);
        assert_eq!(int_instance.params[0].ty, mir::Type::Int);
        assert_eq!(int_instance.return_ty, mir::Type::Int);
        let x = int_instance.params[0].local;
        assert_eq!(int_instance.body.locals[x].ty, mir::Type::Int);
        assert!(matches!(
            &int_instance.body.statements[0].kind,
            mir::StatementKind::Return {
                value: Some(mir::Expr::Local(local))
            } if *local == x
        ));
        assert_eq!(string_instance.params[0].ty, mir::Type::String);
        assert_eq!(string_instance.return_ty, mir::Type::String);

        // The calls in main resolve to the two instances.
        let main_fn = &module.functions[module.entry];
        for (statement, instance) in main_fn
            .body
            .statements
            .iter()
            .zip([module.top_level[1], module.top_level[2]])
        {
            let mir::StatementKind::Expr(mir::Expr::Call(call)) = &statement.kind else {
                panic!("expected a call statement")
            };
            assert_eq!(call.target.callee, mir::Callee::User(instance));
        }
    }

    #[test]
    fn duplicate_requests_produce_one_instance() {
        let mut h = Harness::new();
        let identity = identity_fn(&mut h, "identity");
        let int = h.int;
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: vec![
                    expr_stmt(generic_call(identity, vec![int], vec![int_lit(&h, 1)], int)),
                    expr_stmt(generic_call(identity, vec![int], vec![int_lit(&h, 2)], int)),
                ],
            },
        );
        // HIR dedups its list, but be robust: the same request listed
        // twice, plus two calls with the same type arguments.
        h.instantiate(identity, vec![int]);
        h.instantiate(identity, vec![int]);
        let module = lower(&h.finish(main));

        assert_eq!(module.top_level.len(), 2);
        let instance = module.top_level[1];
        let main_fn = &module.functions[module.entry];
        for statement in &main_fn.body.statements {
            let mir::StatementKind::Expr(mir::Expr::Call(call)) = &statement.kind else {
                panic!("expected a call statement")
            };
            assert_eq!(call.target.callee, mir::Callee::User(instance));
        }
    }

    #[test]
    fn nested_generic_calls_extend_the_worklist() {
        let mut h = Harness::new();
        // fun <T> inner(x: T): T { return x }
        let inner = identity_fn(&mut h, "inner");
        // fun <T> forward(x: T): T { return inner(x) }
        let t = h.types.alloc(hir::Type::Param(0));
        let mut locals = Arena::new();
        let x = locals.alloc(local("x", t));
        let forward = h.user_fn_full(
            "forward",
            vec!["T".to_string()],
            vec![param("x", t, x)],
            t,
            hir::Body {
                locals,
                statements: vec![stmt(hir::StatementKind::Return {
                    value: Some(generic_call(inner, vec![t], vec![local_ref(x, t)], t)),
                })],
            },
        );
        let int = h.int;
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: vec![expr_stmt(generic_call(
                    forward,
                    vec![int],
                    vec![int_lit(&h, 1)],
                    int,
                ))],
            },
        );
        // The nested request is still parameterized in HIR's list;
        // mir-lower concretizes it while lowering forward$I.
        h.instantiate(forward, vec![int]);
        h.instantiate(inner, vec![t]);
        let module = lower(&h.finish(main));

        // main, forward$I, then inner$I (discovered via the worklist).
        assert_eq!(module.top_level.len(), 3);
        let forward_i = &module.functions[module.top_level[1]];
        let inner_i = &module.functions[module.top_level[2]];
        assert_eq!(forward_i.symbol, "scoop.forward$I");
        assert_eq!(inner_i.symbol, "scoop.inner$I");
        let mir::StatementKind::Return {
            value: Some(mir::Expr::Call(call)),
        } = &forward_i.body.statements[0].kind
        else {
            panic!("forward$I must return the inner$I call")
        };
        assert_eq!(call.target.callee, mir::Callee::User(module.top_level[2]));
        assert_eq!(inner_i.params[0].ty, mir::Type::Int);
        assert_eq!(inner_i.return_ty, mir::Type::Int);
    }

    #[test]
    fn instance_symbols_encode_option_and_tuple_arguments() {
        let mut h = Harness::new();
        let f = identity_fn(&mut h, "f");
        let (int, string) = (h.int, h.string);
        let option_int = h.types.alloc(hir::Type::Option(int));
        let pair = h.tuple(&[int, string]);
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            },
        );
        h.instantiate(f, vec![option_int]);
        h.instantiate(f, vec![pair]);
        let module = lower(&h.finish(main));

        let symbols: Vec<&str> = module.top_level[1..]
            .iter()
            .map(|&id| module.functions[id].symbol.as_str())
            .collect();
        assert_eq!(symbols, ["scoop.f$OIX", "scoop.f$TI_SX"]);
        // Substitution recurses into Option / tuple types.
        let option_instance = &module.functions[module.top_level[1]];
        assert_eq!(
            option_instance.params[0].ty,
            mir::Type::Option(Box::new(mir::Type::Int))
        );
        let tuple_instance = &module.functions[module.top_level[2]];
        assert_eq!(
            tuple_instance.return_ty,
            mir::Type::Tuple(vec![mir::Type::Int, mir::Type::String])
        );
    }

    #[test]
    fn option_nodes_pass_through() {
        let mut h = Harness::new();
        let (int, boolean) = (h.int, h.boolean);
        let option_int = h.types.alloc(hir::Type::Option(int));
        let mut locals = Arena::new();
        let o = locals.alloc(local("o", option_int));
        let b = locals.alloc(local("b", boolean));
        let y = locals.alloc(local("y", int));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![
                    val_decl(
                        o,
                        expr(
                            hir::ExprKind::SomeWrap(Box::new(int_lit(&h, 41))),
                            option_int,
                        ),
                    ),
                    val_decl(
                        b,
                        expr(
                            hir::ExprKind::IsSome(Box::new(local_ref(o, option_int))),
                            boolean,
                        ),
                    ),
                    val_decl(
                        y,
                        expr(
                            hir::ExprKind::Unwrap {
                                operand: Box::new(local_ref(o, option_int)),
                                trap_on_none: true,
                            },
                            int,
                        ),
                    ),
                ],
            },
        );
        let module = lower(&h.finish(main));

        let body = &module.functions[module.entry].body;
        assert!(matches!(
            &body.statements[0].kind,
            mir::StatementKind::ValDecl {
                init: mir::Expr::SomeWrap(operand),
                ..
            } if matches!(operand.as_ref(), mir::Expr::IntLiteral(41))
        ));
        assert!(matches!(
            &body.statements[1].kind,
            mir::StatementKind::ValDecl {
                init: mir::Expr::IsSome(_),
                ..
            }
        ));
        assert!(matches!(
            &body.statements[2].kind,
            mir::StatementKind::ValDecl {
                init: mir::Expr::Unwrap {
                    trap_on_none: true,
                    ..
                },
                ..
            }
        ));
        // The local carrying the Option keeps its type.
        let (_, o_local) = body.locals.iter().next().expect("the Option local");
        assert_eq!(o_local.ty, mir::Type::Option(Box::new(mir::Type::Int)));
    }

    /// `val a: Option<Int> = None; val b = Some(1); val r = a <op> b`
    /// — the shared shell of the Option equality tests.
    fn option_comparison(mut h: Harness, op: hir::BinOp) -> mir::Module {
        let (int, boolean) = (h.int, h.boolean);
        let option_int = h.types.alloc(hir::Type::Option(int));
        let mut locals = Arena::new();
        let a = locals.alloc(local("a", option_int));
        let b = locals.alloc(local("b", option_int));
        let r = locals.alloc(local("r", boolean));
        let statements = vec![
            val_decl(a, expr(hir::ExprKind::NoneLiteral, option_int)),
            val_decl(
                b,
                expr(
                    hir::ExprKind::SomeWrap(Box::new(int_lit(&h, 1))),
                    option_int,
                ),
            ),
            val_decl(
                r,
                binary(
                    op,
                    local_ref(a, option_int),
                    local_ref(b, option_int),
                    boolean,
                ),
            ),
        ];
        let main = h.user_fn("main", hir::Body { locals, statements });
        lower(&h.finish(main))
    }

    #[test]
    fn option_equality_expands_through_tags() {
        let h = Harness::new();
        let module = option_comparison(h, hir::BinOp::Eq);

        // Both Some compares the payloads, both None is equal.
        let expected = "\
Module
  fun main @scoop_main() -> Unit
    val a: Option<Int>
      NoneLiteral
    val b: Option<Int>
      SomeWrap
        IntLiteral 1
    val r: Boolean
      Binary Or
        Binary And
          Binary And
            IsSome
              Local a
            IsSome
              Local b
          Binary IntEq
            Unwrap trap=false
              Local a
            Unwrap trap=false
              Local b
        Binary And
          Unary BoolNot
            IsSome
              Local a
          Unary BoolNot
            IsSome
              Local b
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }

    #[test]
    fn option_inequality_is_the_dual_tree() {
        let h = Harness::new();
        let module = option_comparison(h, hir::BinOp::Ne);

        // The De Morgan dual: `And` / `Or` swapped, `isSome` leaves
        // negated, the payload compared with `!=`.
        let expected = "\
Module
  fun main @scoop_main() -> Unit
    val a: Option<Int>
      NoneLiteral
    val b: Option<Int>
      SomeWrap
        IntLiteral 1
    val r: Boolean
      Binary And
        Binary Or
          Binary Or
            Unary BoolNot
              IsSome
                Local a
            Unary BoolNot
              IsSome
                Local b
          Binary IntNe
            Unwrap trap=false
              Local a
            Unwrap trap=false
              Local b
        Binary Or
          IsSome
            Local a
          IsSome
            Local b
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }

    #[test]
    fn none_comparison_folds_is_some_on_the_literal() {
        let mut h = Harness::new();
        let int = h.int;
        let boolean = h.boolean;
        let option_int = h.types.alloc(hir::Type::Option(int));
        let mut locals = Arena::new();
        let a = locals.alloc(local("a", option_int));
        let r = locals.alloc(local("r", boolean));
        // val r = a == None
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![val_decl(
                    r,
                    binary(
                        hir::BinOp::Eq,
                        local_ref(a, option_int),
                        expr(hir::ExprKind::NoneLiteral, option_int),
                        boolean,
                    ),
                )],
            },
        );
        let module = lower(&h.finish(main));

        // `isSome(None)` folds to `false` / `!isSome(None)` to `true`;
        // MIR `IsSome` therefore never wraps `NoneLiteral` (its type
        // would not be recoverable at LIR). The payload comparison is
        // dead code behind `false && ...` but still well-formed.
        let expected = "\
Module
  fun main @scoop_main() -> Unit
    val r: Boolean
      Binary Or
        Binary And
          Binary And
            IsSome
              Local a
            BoolLiteral false
          Binary IntEq
            Unwrap trap=false
              Local a
            Unwrap trap=false
              NoneLiteral
        Binary And
          Unary BoolNot
            IsSome
              Local a
          BoolLiteral true
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }
}
