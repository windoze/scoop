//! MIR stage: monomorphization, name mangling, call-kind annotation,
//! vtable/itable construction, suspend-to-state-machine lowering.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.3 and
//! `docs/milestone4/DESIGN.md` section 3.3.
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
//! M3: monomorphization. Generic functions have no MIR body of their
//! own; each instantiation request `(generic fn, concrete type args)`
//! produces one instance whose body is the generic body with `Param(i)`
//! substituted by `type_args[i]`. Requests discovered while lowering an
//! instance body extend a worklist that is drained to a fixed point;
//! identical requests are deduplicated by mangled symbol.
//!
//! M4: enums and pattern matching. Enum types are instantiated like
//! generic functions — one `mir::EnumDef` per `(enum, concrete type
//! args)`, named by the mangled instance name (`Option$I`) and
//! deduplicated on it; `when` becomes a structured decision sequence
//! (the subject is evaluated once into a hidden local; each arm is a
//! tag comparison, then the field bindings, then the guard nested so a
//! failed guard falls through to the next arm). The HIR Option nodes
//! (`SomeWrap` / `NoneLiteral` / `IsSome` / `Unwrap`) become generic
//! enum operations; a trapping `Unwrap` (`!!`) becomes an if/else whose
//! else branch calls the runtime trap. Equality on enums expands into a
//! tag comparison plus a per-variant payload comparison. `print` /
//! `println` arrive as `@Intrinsic` functions from scoop.core and still
//! map onto the M2/M3 per-type runtime shims.

use std::collections::HashMap;

use la_arena::Arena;
use scoop_ast::Span;
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
        enums: EnumRegistry::default(),
        shell: mangling_shell(&Arena::new()),
        option_variants: (0, 0),
    }
    .run(module)
}

struct Lowerer {
    functions: Arena<mir::Function>,
    /// User functions in declaration order (intrinsics have no MIR body).
    top_level: Vec<mir::FunctionId>,
    strings: Arena<mir::StringConst>,
    structs: Arena<mir::StructDef>,
    /// HIR struct -> MIR struct (arena transposed in declaration order).
    struct_map: HashMap<hir::StructId, mir::StructId>,
    /// HIR user function -> MIR function (non-generic functions only;
    /// generic functions resolve through `instances`).
    function_map: HashMap<hir::FunctionId, mir::FunctionId>,
    instances: InstanceRegistry,
    enums: EnumRegistry,
    /// Mangling shell: the struct / enum names `mir::encode_type`
    /// reads, kept in sync with the real arenas (same ids).
    shell: mir::Module,
    /// Declaration indices of `Option`'s `Some` / `None` variants.
    option_variants: (u32, u32),
}

impl Lowerer {
    fn run(mut self, module: &hir::Module) -> mir::Module {
        // Struct ids first (field types can reference any struct
        // regardless of declaration order), then the mangling shell
        // (struct names for `encode_type`), then the field types
        // themselves — which can instantiate enums.
        self.lower_structs(module);
        self.shell = mangling_shell(&self.structs);
        self.fill_struct_fields(module);
        self.option_variants = option_variants(module);

        // Declare non-generic user functions first, so calls resolve
        // regardless of declaration order. Intrinsics have no body;
        // their callsites map to `Callee::Runtime` shims (see
        // `BodyLowerer::lower_call`). Generic functions have no MIR
        // body of their own — only their monomorphized instances do.
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
            let (params, return_ty, body) = self.lower_user_function(module, hir_id, None);
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
            let types = Types {
                module,
                struct_map: &self.struct_map,
                subst: None,
            };
            let type_args: Vec<mir::Type> = instantiation
                .type_args
                .iter()
                .map(|&ty| types.lower(ty, &mut self.enums, &mut self.shell))
                .collect();
            self.instances.get_or_create(
                module,
                &mut self.functions,
                &mut self.top_level,
                &self.shell,
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
                self.lower_user_function(module, hir_id, Some(&type_args));
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
            enums: self.enums.defs,
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
            enums: &mut self.enums,
            shell: &mut self.shell,
            subst,
            local_map: HashMap::new(),
            locals: Arena::new(),
            hidden_count: 0,
            prelude: Vec::new(),
            option_variants: self.option_variants,
            function_name: &function.name,
        }
        .lower_function(function, body)
    }

    /// Transpose the HIR struct arena into MIR in declaration order
    /// (ids only; field types are filled by `fill_struct_fields`).
    fn lower_structs(&mut self, module: &hir::Module) {
        for (hir_id, decl) in module.structs.iter() {
            let mir_id = self.structs.alloc(mir::StructDef {
                name: decl.name.clone(),
                fields: Vec::new(),
            });
            self.struct_map.insert(hir_id, mir_id);
        }
    }

    /// Fill the MIR struct field types. This runs after the mangling
    /// shell exists, because field types can instantiate enums.
    fn fill_struct_fields(&mut self, module: &hir::Module) {
        for (hir_id, decl) in module.structs.iter() {
            let types = Types {
                module,
                struct_map: &self.struct_map,
                subst: None,
            };
            let fields = decl
                .fields
                .iter()
                .map(|field| mir::Field {
                    name: field.name.clone(),
                    // Struct declarations are not generic in M4, so
                    // field types never mention `Param`.
                    ty: types.lower(field.ty, &mut self.enums, &mut self.shell),
                })
                .collect();
            self.structs[self.struct_map[&hir_id]].fields = fields;
        }
    }
}

/// `mir::mangle_instance` / `mir::encode_type` take `&mir::Module` but
/// only ever read struct and enum names; this shell provides exactly
/// those. Its arenas share the real arenas' allocation order, so ids
/// align.
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
        enums: Arena::new(),
        entry,
        meta: mir::MirMeta::default(),
    }
}

/// The declaration indices of `Option`'s `Some` / `None` variants.
/// hir-lower guarantees scoop.core defines a suitable `Option`.
fn option_variants(module: &hir::Module) -> (u32, u32) {
    let decl = &module.enums[module.option_enum];
    let find = |name: &str| {
        decl.variants
            .iter()
            .position(|variant| variant.name == name)
            .unwrap_or_else(|| panic!("scoop.core's Option must have a `{name}` variant"))
            as u32
    };
    (find("Some"), find("None"))
}

/// Whether a HIR type mentions no type parameters.
fn is_concrete(module: &hir::Module, ty: hir::TypeId) -> bool {
    match &module.types[ty] {
        hir::Type::Param(_) => false,
        hir::Type::Tuple(elements) => elements.iter().all(|&e| is_concrete(module, e)),
        hir::Type::Enum(_, args) => args.iter().all(|&arg| is_concrete(module, arg)),
        _ => true,
    }
}

/// Shared type-lowering context: the HIR type arena, the struct map,
/// and the active substitution (`Param(i)` resolves through `subst`,
/// the concrete type arguments of the instance / enum being lowered;
/// non-generic bodies never contain it).
#[derive(Clone, Copy)]
struct Types<'a> {
    module: &'a hir::Module,
    struct_map: &'a HashMap<hir::StructId, mir::StructId>,
    subst: Option<&'a [mir::Type]>,
}

impl Types<'_> {
    /// Map a HIR type onto its MIR type. Aggregate shapes are
    /// preserved: structs keep their (remapped) id, tuples their mapped
    /// element types; enum types instantiate their definition on
    /// demand.
    fn lower(
        &self,
        ty: hir::TypeId,
        enums: &mut EnumRegistry,
        shell: &mut mir::Module,
    ) -> mir::Type {
        match &self.module.types[ty] {
            hir::Type::Unit => mir::Type::Unit,
            hir::Type::Int => mir::Type::Int,
            hir::Type::Boolean => mir::Type::Boolean,
            hir::Type::String => mir::Type::String,
            hir::Type::Struct(id) => mir::Type::Struct(self.struct_map[id]),
            hir::Type::Tuple(elements) => mir::Type::Tuple(
                elements
                    .iter()
                    .map(|&element| self.lower(element, enums, shell))
                    .collect(),
            ),
            hir::Type::Enum(id, args) => {
                let args: Vec<mir::Type> = args
                    .iter()
                    .map(|&arg| self.lower(arg, enums, shell))
                    .collect();
                let enum_id = enums.get_or_create(self, shell, *id, args.clone());
                mir::Type::Enum(enum_id, args)
            }
            hir::Type::Param(index) => self
                .subst
                .expect("hir::Type::Param only appears with a substitution")[*index as usize]
                .clone(),
        }
    }
}

/// Instantiated enum definitions (DESIGN 3.3): one `mir::EnumDef` per
/// `(enum, concrete type args)`, deduplicated by mangled instance name
/// (`Option$I`, or the plain name for non-generic enums).
#[derive(Default)]
struct EnumRegistry {
    defs: Arena<mir::EnumDef>,
    /// Mangled instance name -> enum. The name encodes the enum and
    /// its type arguments, so it is the deduplication key.
    by_name: HashMap<String, mir::EnumId>,
}

impl EnumRegistry {
    fn get_or_create(
        &mut self,
        types: &Types,
        shell: &mut mir::Module,
        hir_id: hir::EnumId,
        args: Vec<mir::Type>,
    ) -> mir::EnumId {
        let decl = &types.module.enums[hir_id];
        // Nested arguments are instantiated first (their shell entries
        // exist), so `encode_type` can render them here.
        let name = if args.is_empty() {
            decl.name.clone()
        } else {
            let encoded: Vec<String> = args.iter().map(|ty| mir::encode_type(shell, ty)).collect();
            format!("{}${}", decl.name, encoded.join("_"))
        };
        if let Some(&id) = self.by_name.get(&name) {
            return id;
        }
        let id = self.defs.alloc(mir::EnumDef {
            name: name.clone(),
            variants: Vec::new(),
        });
        // Keep the mangling shell's enum arena in sync (same ids) so
        // `encode_type` can render this instance inside another one.
        shell.enums.alloc(mir::EnumDef {
            name: name.clone(),
            variants: Vec::new(),
        });
        self.by_name.insert(name, id);
        // Fill the definition eagerly: the id is already registered, so
        // variant fields mentioning this same enum terminate. Variant
        // field types mention the enum's own type parameters, which the
        // instance's type arguments replace.
        let variant_types = Types {
            subst: Some(&args),
            ..*types
        };
        let variants = decl
            .variants
            .iter()
            .map(|variant| mir::VariantDef {
                name: variant.name.clone(),
                fields: variant
                    .fields
                    .iter()
                    .map(|field| mir::Field {
                        name: field.name.clone(),
                        ty: variant_types.lower(field.ty, self, shell),
                    })
                    .collect(),
            })
            .collect();
        self.defs[id].variants = variants;
        id
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

/// Map a `print` / `println` intrinsic call onto the per-type runtime
/// shim (DESIGN 1.3). hir-lower rejects arguments of any other type,
/// so only String / Int / Boolean can reach this stage.
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
    /// Instantiated enum definitions, filled on creation (variant
    /// field types feed pattern lowering and the equality expansion).
    enums: &'a mut EnumRegistry,
    /// Mangling shell (enum / struct names for `encode_type`).
    shell: &'a mut mir::Module,
    /// Concrete type arguments of the instance being lowered; `None`
    /// for non-generic bodies (which never mention `Param`).
    subst: Option<&'a [mir::Type]>,
    /// HIR local -> MIR local (same declaration order per body).
    local_map: HashMap<hir::LocalId, mir::LocalId>,
    /// MIR locals, including the hidden ones created during lowering
    /// (`when` subjects, destructuring slots, `!!` temporaries).
    locals: Arena<mir::Local>,
    hidden_count: usize,
    /// Statement kinds that must precede the statement currently being
    /// lowered (the trap test of `!!`); drained by the caller.
    prelude: Vec<mir::StatementKind>,
    /// Declaration indices of `Option::Some` / `Option::None`.
    option_variants: (u32, u32),
    /// Source name of the function, for trap messages.
    function_name: &'a str,
}

/// A step from a compared operand down to the sub-value at an equality
/// leaf: a struct field / tuple element access, or an enum variant
/// field extraction.
#[derive(Clone, Copy)]
enum Access {
    Field(u32),
    EnumField { variant: u32, index: u32 },
}

/// An equality operand: either a HIR expression (re-lowered at each
/// leaf — see `expand_equality`'s purity note) or the hidden local a
/// `when` subject / destructured value was evaluated into.
enum Opd<'a> {
    Hir(&'a hir::Expr),
    Local(mir::LocalId),
}

impl BodyLowerer<'_> {
    fn lower_function(
        mut self,
        function: &hir::Function,
        body: &hir::Body,
    ) -> (Vec<mir::Param>, mir::Type, mir::Body) {
        for (hir_id, local) in body.locals.iter() {
            let ty = self.lower_type(local.ty);
            let mir_id = self.locals.alloc(mir::Local {
                name: local.name.clone(),
                ty,
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
        (
            params,
            return_ty,
            mir::Body {
                locals: self.locals,
                statements,
            },
        )
    }

    fn lower_type(&mut self, ty: hir::TypeId) -> mir::Type {
        Types {
            module: self.module,
            struct_map: self.struct_map,
            subst: self.subst,
        }
        .lower(ty, self.enums, self.shell)
    }

    /// A fresh hidden local (`$<prefix>.<n>`), compiler-generated.
    fn new_hidden(&mut self, prefix: &str, ty: mir::Type, mutable: bool) -> mir::LocalId {
        self.hidden_count += 1;
        self.locals.alloc(mir::Local {
            name: format!("${prefix}.{}", self.hidden_count),
            ty,
            mutable,
        })
    }

    /// Emit the queued prelude statements (the trap tests of `!!`)
    /// before the statement they belong to.
    fn drain_prelude(&mut self, span: Span, out: &mut Vec<mir::Statement>) {
        out.extend(
            self.prelude
                .drain(..)
                .map(|kind| mir::Statement { kind, span }),
        );
    }

    fn lower_statements(&mut self, statements: &[hir::Statement]) -> Vec<mir::Statement> {
        let mut out = Vec::new();
        for statement in statements {
            self.lower_statement(statement, &mut out);
        }
        out
    }

    fn lower_statement(&mut self, statement: &hir::Statement, out: &mut Vec<mir::Statement>) {
        let span = statement.span;
        let kind = match &statement.kind {
            hir::StatementKind::Expr(expr) => {
                let expr = self.lower_expr(expr);
                self.drain_prelude(span, out);
                mir::StatementKind::Expr(expr)
            }
            hir::StatementKind::Return { value } => {
                let value = value.as_ref().map(|value| self.lower_expr(value));
                self.drain_prelude(span, out);
                mir::StatementKind::Return { value }
            }
            hir::StatementKind::ValDecl { pattern, init } => {
                self.lower_val_decl(pattern, init, span, out);
                return;
            }
            hir::StatementKind::Assign { local, value } => {
                let local = self.local_map[local];
                let value = self.lower_expr(value);
                self.drain_prelude(span, out);
                mir::StatementKind::Assign { local, value }
            }
            hir::StatementKind::If {
                cond,
                then_body,
                else_body,
            } => {
                let cond = self.lower_expr(cond);
                self.drain_prelude(span, out);
                let then_body = self.lower_statements(then_body);
                let else_body = else_body.as_ref().map(|body| self.lower_statements(body));
                mir::StatementKind::If {
                    cond,
                    then_body,
                    else_body,
                }
            }
            hir::StatementKind::While { cond, body } => {
                self.lower_while(cond, body, span, out);
                return;
            }
            hir::StatementKind::When(when) => {
                self.lower_when(when, span, out);
                return;
            }
        };
        out.push(mir::Statement { kind, span });
    }

    /// A `val` declaration: either the plain M1–M3 binding form, or a
    /// destructuring declaration (spec 4.6) whose init value is
    /// evaluated once into a hidden local that the pattern's bindings
    /// extract from.
    fn lower_val_decl(
        &mut self,
        pattern: &hir::Pattern,
        init: &hir::Expr,
        span: Span,
        out: &mut Vec<mir::Statement>,
    ) {
        if let hir::Pattern::Binding { local } = pattern {
            let local = self.local_map[local];
            let init = self.lower_expr(init);
            self.drain_prelude(span, out);
            out.push(mir::Statement {
                kind: mir::StatementKind::ValDecl { local, init },
                span,
            });
            return;
        }
        let ty = self.lower_type(init.ty);
        let init = self.lower_expr(init);
        self.drain_prelude(span, out);
        let slot = self.new_hidden("bind", ty.clone(), false);
        out.push(mir::Statement {
            kind: mir::StatementKind::ValDecl { local: slot, init },
            span,
        });
        let mut path = Vec::new();
        let mut bindings = Vec::new();
        let cond = self.lower_pattern(pattern, slot, &mut path, &ty, &mut bindings);
        // hir-lower only emits irrefutable patterns (tuple / struct /
        // binding / wildcard) in destructuring declarations.
        debug_assert!(cond.is_none(), "destructuring patterns are irrefutable");
        for (local, init) in bindings {
            out.push(mir::Statement {
                kind: mir::StatementKind::ValDecl { local, init },
                span,
            });
        }
    }

    fn lower_while(
        &mut self,
        cond: &hir::Expr,
        body: &[hir::Statement],
        span: Span,
        out: &mut Vec<mir::Statement>,
    ) {
        let cond_mir = self.lower_expr(cond);
        if self.prelude.is_empty() {
            let body = self.lower_statements(body);
            out.push(mir::Statement {
                kind: mir::StatementKind::While {
                    cond: cond_mir,
                    body,
                },
                span,
            });
            return;
        }
        // The condition contains a trap test (`!!`), which is a
        // statement sequence and must run on every iteration:
        // `P; while (C) B` becomes `P; var $c = C; while ($c) { B; P;
        // $c = C }`. The condition and its prelude are lowered twice;
        // each execution path still evaluates them exactly once per
        // iteration.
        self.drain_prelude(span, out);
        let cond_local = self.new_hidden("cond", mir::Type::Boolean, true);
        out.push(mir::Statement {
            kind: mir::StatementKind::ValDecl {
                local: cond_local,
                init: cond_mir,
            },
            span,
        });
        let mut body = self.lower_statements(body);
        let cond_again = self.lower_expr(cond);
        let prelude_again = std::mem::take(&mut self.prelude);
        body.extend(
            prelude_again
                .into_iter()
                .map(|kind| mir::Statement { kind, span }),
        );
        body.push(mir::Statement {
            kind: mir::StatementKind::Assign {
                local: cond_local,
                value: cond_again,
            },
            span,
        });
        out.push(mir::Statement {
            kind: mir::StatementKind::While {
                cond: mir::Expr::Local(cond_local),
                body,
            },
            span,
        });
    }

    /// `when` becomes a decision sequence (DESIGN 3.3): the subject is
    /// evaluated once into a hidden local, then the arms chain if/else
    /// tests; the `else` arm is the fallback.
    fn lower_when(&mut self, when: &hir::When, span: Span, out: &mut Vec<mir::Statement>) {
        let subject_ty = self.lower_type(when.subject.ty);
        let subject_init = self.lower_expr(&when.subject);
        self.drain_prelude(span, out);
        let subject = self.new_hidden("when", subject_ty.clone(), false);
        out.push(mir::Statement {
            kind: mir::StatementKind::ValDecl {
                local: subject,
                init: subject_init,
            },
            span,
        });
        let mut chain =
            self.lower_arms(&when.arms, subject, &subject_ty, when.else_body.as_deref());
        out.append(&mut chain);
    }

    /// Lower `arms` into the decision sequence: each arm is
    /// `if (<pattern condition>) { <bindings>; [if (<guard>) <body>
    /// else <next>] } else <next>` — a failed guard falls through to
    /// the next arm. With no guard the arm body is the then branch
    /// directly; an unconditionally matching arm (binding / wildcard,
    /// no guard) is inlined and makes the remaining arms unreachable
    /// (hir-lower rejects those). Exhaustiveness was checked at HIR,
    /// so the innermost else can only be reached via `else_body`.
    fn lower_arms(
        &mut self,
        arms: &[hir::WhenArm],
        subject: mir::LocalId,
        subject_ty: &mir::Type,
        else_body: Option<&[hir::Statement]>,
    ) -> Vec<mir::Statement> {
        let Some((arm, rest)) = arms.split_first() else {
            return else_body
                .map(|body| self.lower_statements(body))
                .unwrap_or_default();
        };
        let mut path = Vec::new();
        let mut bindings = Vec::new();
        let cond = self.lower_pattern(&arm.pattern, subject, &mut path, subject_ty, &mut bindings);
        let mut then: Vec<mir::Statement> = bindings
            .into_iter()
            .map(|(local, init)| mir::Statement {
                kind: mir::StatementKind::ValDecl { local, init },
                span: arm.span,
            })
            .collect();
        if let Some(guard) = &arm.guard {
            let guard_cond = self.lower_expr(guard);
            let guard_prelude = std::mem::take(&mut self.prelude);
            then.extend(guard_prelude.into_iter().map(|kind| mir::Statement {
                kind,
                span: arm.span,
            }));
            let body = self.lower_statements(&arm.body);
            let next = self.lower_arms(rest, subject, subject_ty, else_body);
            then.push(mir::Statement {
                kind: mir::StatementKind::If {
                    cond: guard_cond,
                    then_body: body,
                    else_body: non_empty(next),
                },
                span: arm.span,
            });
        } else {
            then.extend(self.lower_statements(&arm.body));
        }
        let Some(cond) = cond else {
            // Matches unconditionally; `rest` is unreachable.
            return then;
        };
        let next = self.lower_arms(rest, subject, subject_ty, else_body);
        vec![mir::Statement {
            kind: mir::StatementKind::If {
                cond,
                then_body: then,
                else_body: non_empty(next),
            },
            span: arm.span,
        }]
    }

    /// Lower a pattern matching the value at `root` + `path` (of MIR
    /// type `ty`): returns the match condition (`None` when the
    /// pattern matches unconditionally) and appends the binding
    /// initializers — `local = <value at path>` — in declaration
    /// order. The condition's `&&` chain short-circuits at LIR, so a
    /// variant field is only extracted once its tag test has passed.
    fn lower_pattern(
        &mut self,
        pattern: &hir::Pattern,
        root: mir::LocalId,
        path: &mut Vec<Access>,
        ty: &mir::Type,
        bindings: &mut Vec<(mir::LocalId, mir::Expr)>,
    ) -> Option<mir::Expr> {
        match pattern {
            hir::Pattern::Binding { local } => {
                let init = self.accessed(&Opd::Local(root), path);
                bindings.push((self.local_map[local], init));
                None
            }
            hir::Pattern::Wildcard => None,
            // A literal matches by equality (the M2/M3 expansion).
            hir::Pattern::Literal(literal) => {
                Some(self.expand_equality(&Opd::Local(root), &Opd::Hir(literal), ty, path, false))
            }
            hir::Pattern::Variant {
                variant, fields, ..
            } => {
                let mir::Type::Enum(enum_id, _) = ty else {
                    unreachable!("a variant pattern matches an enum value")
                };
                let enum_id = *enum_id;
                let variant = *variant;
                let tag = mir::Expr::EnumTag(Box::new(self.accessed(&Opd::Local(root), path)));
                let mut cond = mir::Expr::Binary {
                    op: mir::BinOp::IntEq,
                    lhs: Box::new(tag),
                    rhs: Box::new(mir::Expr::IntLiteral(i64::from(variant))),
                };
                for (index, sub) in fields {
                    let field_ty = self.enums.defs[enum_id].variants[variant as usize].fields
                        [*index as usize]
                        .ty
                        .clone();
                    path.push(Access::EnumField {
                        variant,
                        index: *index,
                    });
                    if let Some(sub_cond) = self.lower_pattern(sub, root, path, &field_ty, bindings)
                    {
                        cond = and(cond, sub_cond);
                    }
                    path.pop();
                }
                Some(cond)
            }
            hir::Pattern::Tuple(elements) => {
                let mir::Type::Tuple(element_types) = ty else {
                    unreachable!("a tuple pattern matches a tuple value")
                };
                let element_types = element_types.clone();
                let mut cond: Option<mir::Expr> = None;
                for (index, sub) in elements.iter().enumerate() {
                    path.push(Access::Field(index as u32));
                    if let Some(sub_cond) =
                        self.lower_pattern(sub, root, path, &element_types[index], bindings)
                    {
                        cond = Some(match cond {
                            None => sub_cond,
                            Some(acc) => and(acc, sub_cond),
                        });
                    }
                    path.pop();
                }
                cond
            }
            hir::Pattern::Struct { fields, .. } => {
                let mir::Type::Struct(struct_id) = ty else {
                    unreachable!("a struct pattern matches a struct value")
                };
                let field_types: Vec<mir::Type> = self.structs[*struct_id]
                    .fields
                    .iter()
                    .map(|field| field.ty.clone())
                    .collect();
                let mut cond: Option<mir::Expr> = None;
                for (index, sub) in fields {
                    path.push(Access::Field(*index));
                    if let Some(sub_cond) =
                        self.lower_pattern(sub, root, path, &field_types[*index as usize], bindings)
                    {
                        cond = Some(match cond {
                            None => sub_cond,
                            Some(acc) => and(acc, sub_cond),
                        });
                    }
                    path.pop();
                }
                cond
            }
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
            hir::ExprKind::VariantConstruct { variant, args, .. } => mir::Expr::VariantConstruct {
                ty: self.lower_type(expr.ty),
                variant: *variant,
                fields: args.iter().map(|arg| self.lower_expr(arg)).collect(),
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
            // The Option nodes (hir-lower's `?.` / `?:` / `!!`
            // desugars) become generic enum operations on core's
            // `Option` enum (DESIGN 3.3).
            hir::ExprKind::SomeWrap(operand) => {
                let (some, _) = self.option_variants;
                mir::Expr::VariantConstruct {
                    ty: self.lower_type(expr.ty),
                    variant: some,
                    fields: vec![self.lower_expr(operand)],
                }
            }
            hir::ExprKind::NoneLiteral => {
                let (_, none) = self.option_variants;
                mir::Expr::VariantConstruct {
                    ty: self.lower_type(expr.ty),
                    variant: none,
                    fields: Vec::new(),
                }
            }
            hir::ExprKind::IsSome(operand) => {
                let (some, _) = self.option_variants;
                let operand = self.lower_expr(operand);
                mir::Expr::Binary {
                    op: mir::BinOp::IntEq,
                    lhs: Box::new(mir::Expr::EnumTag(Box::new(operand))),
                    rhs: Box::new(mir::Expr::IntLiteral(i64::from(some))),
                }
            }
            hir::ExprKind::Unwrap {
                operand,
                trap_on_none,
            } => {
                let (some, _) = self.option_variants;
                if *trap_on_none {
                    self.trapping_unwrap(operand, expr.ty, expr.span, some)
                } else {
                    // The surrounding control flow already guarantees
                    // `Some` (`?.` / `?:` desugars, the equality
                    // expansion).
                    mir::Expr::EnumField {
                        operand: Box::new(self.lower_expr(operand)),
                        variant: some,
                        index: 0,
                    }
                }
            }
        }
    }

    /// `x!!`: the operand is evaluated once into a hidden local, then
    /// `if (tag == Some) { val $uw = <field 0> } else { trap }`. The
    /// if/else is queued in `prelude` — it must precede the statement
    /// this expression belongs to — and the expression itself becomes
    /// the result local. The trap message is an ordinary string
    /// constant; LIR turns the trap call into a `CString` global plus
    /// a noreturn `scoop_rt_trap` call.
    fn trapping_unwrap(
        &mut self,
        operand: &hir::Expr,
        result_ty: hir::TypeId,
        span: Span,
        some: u32,
    ) -> mir::Expr {
        let option_ty = self.lower_type(operand.ty);
        let payload_ty = self.lower_type(result_ty);
        let value = self.lower_expr(operand);
        let slot = self.new_hidden("opt", option_ty, false);
        let result = self.new_hidden("uw", payload_ty, false);
        let message = format!("unwrap on None (function {})", self.function_name);
        let symbol = format!("scoop.str.{}", self.strings.len());
        let message = self.strings.alloc(mir::StringConst {
            value: message,
            symbol,
        });
        self.prelude.push(mir::StatementKind::ValDecl {
            local: slot,
            init: value,
        });
        self.prelude.push(mir::StatementKind::If {
            cond: mir::Expr::Binary {
                op: mir::BinOp::IntEq,
                lhs: Box::new(mir::Expr::EnumTag(Box::new(mir::Expr::Local(slot)))),
                rhs: Box::new(mir::Expr::IntLiteral(i64::from(some))),
            },
            then_body: vec![mir::Statement {
                kind: mir::StatementKind::ValDecl {
                    local: result,
                    init: mir::Expr::EnumField {
                        operand: Box::new(mir::Expr::Local(slot)),
                        variant: some,
                        index: 0,
                    },
                },
                span,
            }],
            else_body: Some(vec![mir::Statement {
                kind: mir::StatementKind::Expr(mir::Expr::Call(mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::Runtime(mir::RuntimeFn::Trap),
                    },
                    args: vec![mir::Expr::StringConst(message)],
                })),
                span,
            }]),
        });
        mir::Expr::Local(result)
    }

    fn lower_call(
        &mut self,
        function: hir::FunctionId,
        type_args: &[hir::TypeId],
        args: &[hir::Expr],
    ) -> mir::Expr {
        let callee = match &self.module.functions[function].kind {
            // `@Intrinsic` functions (scoop.core, DESIGN 1.3): the
            // registry holds only the output intrinsics, whose calls
            // map onto the per-type runtime shims. hir-lower enforces
            // exactly one String / Int / Boolean argument.
            hir::FunctionKind::Intrinsic(name) => {
                let newline = match name.as_str() {
                    "rt_print" => false,
                    "rt_println" => true,
                    _ => unreachable!("hir-lower rejects unknown intrinsics"),
                };
                mir::Callee::Runtime(print_fn(self.module, args[0].ty, newline))
            }
            hir::FunctionKind::User(_)
                if self.module.functions[function].type_params.is_empty() =>
            {
                mir::Callee::User(self.function_map[&function])
            }
            // Generic callee: the call's type arguments may mention the
            // enclosing instance's `Param`s; substitution concretizes
            // them, and the instance is created on demand (its body is
            // lowered when the worklist drains).
            hir::FunctionKind::User(_) => {
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
            }
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
                self.expand_equality(&Opd::Hir(lhs), &Opd::Hir(rhs), &ty, &[], false)
            }
            hir::BinOp::Ne => {
                let ty = self.lower_type(lhs.ty);
                self.expand_equality(&Opd::Hir(lhs), &Opd::Hir(rhs), &ty, &[], true)
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
    /// type `ty` (DESIGN 2.3 / 3.3):
    ///
    /// - Int / Boolean: the primitive MIR comparison;
    /// - String: a `scoop_rt_string_eq` call (`!=` wraps it in `!`);
    /// - struct / tuple: per-field comparisons, folded with `&&` for
    ///   `==`; `!=` folds per-field `!=` with `||` — the De Morgan
    ///   dual of the `==` tree, equivalent to negating it because
    ///   field access is pure;
    /// - enum: the tags must be equal, and for every payload-carrying
    ///   variant `i`, `tag != i || <fields equal>` — the De Morgan
    ///   dual for `!=`. Unit variants carry no payload, so the tag
    ///   comparison covers them;
    /// - Unit (the empty tuple): a constant — `() == ()` is always
    ///   `true`, `() != ()` always `false`.
    ///
    /// `path` is the chain of field accesses and variant field
    /// extractions from the top-level operands down to the values
    /// compared at this level. HIR operands are re-lowered at each
    /// leaf; this duplicates structure, not effects, because
    /// everything that can appear as an operand here is pure (M2
    /// assumption, still valid in M4): field access, and calls —
    /// value-returning calls are treated as pure by convention, and
    /// Unit-returning calls cannot appear because both operands share
    /// the compared type. If impure value-returning calls ever become
    /// observable, this expansion must route the operands through
    /// hidden temporaries instead of re-lowering them.
    fn expand_equality(
        &mut self,
        lhs: &Opd,
        rhs: &Opd,
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
            mir::Type::Enum(id, _) => {
                let id = *id;
                self.expand_enum_equality(lhs, rhs, id, path, negate)
            }
        }
    }

    /// Enum equality (DESIGN 3.3): `tag(a) == tag(b)` and, for every
    /// payload-carrying variant `i`, `tag(a) != i || <fields equal>`.
    /// The fields are only extracted once the tag is known to match
    /// (`&&` / `||` short-circuit at LIR). For `!=` the whole tree is
    /// dualized: `And` / `Or` swapped, the tag leaves negated, the
    /// fields compared with `!=`.
    fn expand_enum_equality(
        &mut self,
        lhs: &Opd,
        rhs: &Opd,
        enum_id: mir::EnumId,
        path: &[Access],
        negate: bool,
    ) -> mir::Expr {
        let variants: Vec<Vec<mir::Type>> = self.enums.defs[enum_id]
            .variants
            .iter()
            .map(|variant| {
                variant
                    .fields
                    .iter()
                    .map(|field| field.ty.clone())
                    .collect()
            })
            .collect();
        let tag_comparison = mir::Expr::Binary {
            op: if negate {
                mir::BinOp::IntNe
            } else {
                mir::BinOp::IntEq
            },
            lhs: Box::new(mir::Expr::EnumTag(Box::new(self.accessed(lhs, path)))),
            rhs: Box::new(mir::Expr::EnumTag(Box::new(self.accessed(rhs, path)))),
        };
        let mut combined: Option<mir::Expr> = None;
        for (variant, field_types) in variants.iter().enumerate() {
            // Unit variants carry no payload; the tag comparison
            // covers them.
            if field_types.is_empty() {
                continue;
            }
            let variant = variant as u32;
            let guard = mir::Expr::Binary {
                op: if negate {
                    mir::BinOp::IntEq
                } else {
                    mir::BinOp::IntNe
                },
                lhs: Box::new(mir::Expr::EnumTag(Box::new(self.accessed(lhs, path)))),
                rhs: Box::new(mir::Expr::IntLiteral(i64::from(variant))),
            };
            let mut fields: Option<mir::Expr> = None;
            for (index, field_ty) in field_types.iter().enumerate() {
                let mut field_path = path.to_vec();
                field_path.push(Access::EnumField {
                    variant,
                    index: index as u32,
                });
                let comparison = self.expand_equality(lhs, rhs, field_ty, &field_path, negate);
                fields = Some(match fields {
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
            let fields = fields.expect("payload-carrying variant");
            let clause = mir::Expr::Binary {
                op: if negate {
                    mir::BinOp::And
                } else {
                    mir::BinOp::Or
                },
                lhs: Box::new(guard),
                rhs: Box::new(fields),
            };
            combined = Some(match combined {
                None => clause,
                Some(acc) => mir::Expr::Binary {
                    op: if negate {
                        mir::BinOp::Or
                    } else {
                        mir::BinOp::And
                    },
                    lhs: Box::new(acc),
                    rhs: Box::new(clause),
                },
            });
        }
        match combined {
            None => tag_comparison,
            Some(clauses) => mir::Expr::Binary {
                op: if negate {
                    mir::BinOp::Or
                } else {
                    mir::BinOp::And
                },
                lhs: Box::new(tag_comparison),
                rhs: Box::new(clauses),
            },
        }
    }

    /// Fold the per-field comparisons of an aggregate equality: `&&`
    /// over `==` leaves for `==`, `||` over `!=` leaves for `!=`; an
    /// empty aggregate compares as the corresponding constant.
    fn expand_fields(
        &mut self,
        lhs: &Opd,
        rhs: &Opd,
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
    fn comparison(&mut self, op: mir::BinOp, lhs: &Opd, rhs: &Opd, path: &[Access]) -> mir::Expr {
        let lhs = Box::new(self.accessed(lhs, path));
        let rhs = Box::new(self.accessed(rhs, path));
        mir::Expr::Binary { op, lhs, rhs }
    }

    /// Produce the operand value and wrap it in the `path` accesses.
    /// Variant field extractions never fail: the decision sequence and
    /// the equality tree only evaluate them once the tag is known to
    /// match.
    fn accessed(&mut self, opd: &Opd, path: &[Access]) -> mir::Expr {
        let mut lowered = match opd {
            Opd::Hir(expr) => self.lower_expr(expr),
            Opd::Local(local) => mir::Expr::Local(*local),
        };
        for access in path {
            lowered = match access {
                Access::Field(index) => mir::Expr::FieldAccess {
                    receiver: Box::new(lowered),
                    index: *index,
                },
                Access::EnumField { variant, index } => mir::Expr::EnumField {
                    operand: Box::new(lowered),
                    variant: *variant,
                    index: *index,
                },
            };
        }
        lowered
    }
}

/// Combine two conditions with `&&` (short-circuits at LIR).
fn and(lhs: mir::Expr, rhs: mir::Expr) -> mir::Expr {
    mir::Expr::Binary {
        op: mir::BinOp::And,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

/// `Some(statements)` unless empty (an absent else branch).
fn non_empty(statements: Vec<mir::Statement>) -> Option<Vec<mir::Statement>> {
    if statements.is_empty() {
        None
    } else {
        Some(statements)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use scoop_ast::Span;

    const SPAN: Span = Span { start: 0, end: 0 };

    /// HIR module shell as hir-lower produces it: well-known types,
    /// the intrinsic output functions and core's `Option` enum
    /// allocated first.
    struct Harness {
        types: Arena<hir::Type>,
        functions: Arena<hir::Function>,
        structs: Arena<hir::StructDecl>,
        enums: Arena<hir::EnumDecl>,
        top_level: Vec<hir::FunctionId>,
        unit: hir::TypeId,
        int: hir::TypeId,
        boolean: hir::TypeId,
        string: hir::TypeId,
        option_enum: hir::EnumId,
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
                kind: hir::FunctionKind::Intrinsic("rt_print".to_string()),
                span: SPAN,
            });
            let println = functions.alloc(hir::Function {
                name: "println".to_string(),
                type_params: Vec::new(),
                params: Vec::new(),
                return_ty: unit,
                kind: hir::FunctionKind::Intrinsic("rt_println".to_string()),
                span: SPAN,
            });
            // scoop.core's `enum Option<T> { Some(T), None }`.
            let t = types.alloc(hir::Type::Param(0));
            let mut enums = Arena::new();
            let option_enum = enums.alloc(hir::EnumDecl {
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    hir::Variant {
                        name: "Some".to_string(),
                        fields: vec![hir::Field {
                            name: "_1".to_string(),
                            ty: t,
                        }],
                        defaults: vec![None],
                    },
                    hir::Variant {
                        name: "None".to_string(),
                        fields: Vec::new(),
                        defaults: Vec::new(),
                    },
                ],
                span: SPAN,
            });
            Harness {
                types,
                functions,
                structs: Arena::new(),
                enums,
                top_level: vec![print, println],
                unit,
                int,
                boolean,
                string,
                option_enum,
                print,
                println,
                instantiations: Vec::new(),
            }
        }

        /// `Option<inner>` (core's enum applied to one argument).
        fn option(&mut self, inner: hir::TypeId) -> hir::TypeId {
            self.types
                .alloc(hir::Type::Enum(self.option_enum, vec![inner]))
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
                enums: self.enums,
                top_level: self.top_level,
                unit: self.unit,
                int: self.int,
                boolean: self.boolean,
                string: self.string,
                option_enum: self.option_enum,
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
        stmt(hir::StatementKind::ValDecl {
            pattern: hir::Pattern::Binding { local },
            init,
        })
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
        // Add another `println("hello, world")` to `main`. `println`
        // is the second intrinsic function in `top_level`.
        let println = hir_module.top_level[1];
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
    fn instance_symbols_encode_enum_and_tuple_arguments() {
        let mut h = Harness::new();
        let f = identity_fn(&mut h, "f");
        let (int, string) = (h.int, h.string);
        let option_int = h.option(int);
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
        // An enum argument encodes as `E<instance name>_<args>X`
        // (`mir::encode_type`); the instance name itself already
        // embeds the encoded arguments.
        assert_eq!(symbols, ["scoop.f$EOption$I_IX", "scoop.f$TI_SX"]);
        // Substitution recurses into enum / tuple types.
        let option_instance = &module.functions[module.top_level[1]];
        let mir::Type::Enum(enum_id, args) = &option_instance.params[0].ty else {
            panic!("the Option<Int> instance parameter must be an enum type")
        };
        assert_eq!(module.enums[*enum_id].name, "Option$I");
        assert_eq!(args.as_slice(), &[mir::Type::Int]);
        let tuple_instance = &module.functions[module.top_level[2]];
        assert_eq!(
            tuple_instance.return_ty,
            mir::Type::Tuple(vec![mir::Type::Int, mir::Type::String])
        );
    }

    #[test]
    fn enum_instances_are_created_once_with_substituted_fields() {
        let mut h = Harness::new();
        let (int, string) = (h.int, h.string);
        // A non-generic enum.
        let color = h.enums.alloc(hir::EnumDecl {
            name: "Color".to_string(),
            type_params: Vec::new(),
            variants: ["Red", "Green", "Blue"]
                .iter()
                .map(|name| hir::Variant {
                    name: name.to_string(),
                    fields: Vec::new(),
                    defaults: Vec::new(),
                })
                .collect(),
            span: SPAN,
        });
        let color_ty = h.types.alloc(hir::Type::Enum(color, Vec::new()));
        let option_int = h.option(int);
        let option_string = h.option(string);
        // f1 holds Option<Int> and Color; f2 holds Option<Int> again
        // (a duplicate request) and Option<String>.
        let mut locals1 = Arena::new();
        locals1.alloc(local("o", option_int));
        locals1.alloc(local("c", color_ty));
        let _f1 = h.user_fn(
            "f1",
            hir::Body {
                locals: locals1,
                statements: Vec::new(),
            },
        );
        let mut locals2 = Arena::new();
        locals2.alloc(local("o", option_int));
        locals2.alloc(local("s", option_string));
        let _f2 = h.user_fn(
            "f2",
            hir::Body {
                locals: locals2,
                statements: Vec::new(),
            },
        );
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            },
        );
        let module = lower(&h.finish(main));

        // One definition per (enum, type args), in creation order; the
        // duplicate Option<Int> request was deduplicated by name.
        let names: Vec<&str> = module
            .enums
            .iter()
            .map(|(_, def)| def.name.as_str())
            .collect();
        assert_eq!(names, ["Option$I", "Color", "Option$S"]);

        // The variant field types are substituted with the instance's
        // type arguments.
        let option_int_def = &module.enums[la_arena::Idx::from_raw(0.into())];
        assert_eq!(option_int_def.variants[0].name, "Some");
        assert_eq!(option_int_def.variants[0].fields[0].ty, mir::Type::Int);
        let option_string_def = &module.enums[la_arena::Idx::from_raw(2.into())];
        assert_eq!(
            option_string_def.variants[0].fields[0].ty,
            mir::Type::String
        );
        // Color's variants are all unit variants.
        let color_def = &module.enums[la_arena::Idx::from_raw(1.into())];
        assert_eq!(color_def.variants.len(), 3);
        assert!(color_def.variants.iter().all(|v| v.fields.is_empty()));
    }

    #[test]
    fn option_nodes_become_generic_enum_operations() {
        let mut h = Harness::new();
        let (int, boolean) = (h.int, h.boolean);
        let option_int = h.option(int);
        let mut locals = Arena::new();
        let o = locals.alloc(local("o", option_int));
        let n = locals.alloc(local("n", option_int));
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
                    val_decl(n, expr(hir::ExprKind::NoneLiteral, option_int)),
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
                                trap_on_none: false,
                            },
                            int,
                        ),
                    ),
                ],
            },
        );
        let module = lower(&h.finish(main));

        let expected = "\
Module
  enum Option$I
    Some(_1: Int)
    None()
  fun main @scoop_main() -> Unit
    val o: Option$I<Int>
      VariantConstruct Option$I<Int> v0
        IntLiteral 41
    val n: Option$I<Int>
      VariantConstruct Option$I<Int> v1
    val b: Boolean
      Binary IntEq
        EnumTag
          Local o
        IntLiteral 0
    val y: Int
      EnumField v0 f0
        Local o
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }

    #[test]
    fn trapping_unwrap_becomes_a_guarded_extraction() {
        // val o = Some(1); val y = o!!
        let mut h = Harness::new();
        let int = h.int;
        let option_int = h.option(int);
        let mut locals = Arena::new();
        let o = locals.alloc(local("o", option_int));
        let y = locals.alloc(local("y", int));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![
                    val_decl(
                        o,
                        expr(
                            hir::ExprKind::SomeWrap(Box::new(int_lit(&h, 1))),
                            option_int,
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

        // The operand is evaluated once into `$opt.1`; the tag test
        // guards the extraction, and the else branch calls the runtime
        // trap with the message string constant.
        let expected = "\
Module
  enum Option$I
    Some(_1: Int)
    None()
  fun main @scoop_main() -> Unit
    val o: Option$I<Int>
      VariantConstruct Option$I<Int> v0
        IntLiteral 1
    val $opt.1: Option$I<Int>
      Local o
    if
      Binary IntEq
        EnumTag
          Local $opt.1
        IntLiteral 0
      val $uw.2: Int
        EnumField v0 f0
          Local $opt.1
    else
      Call @scoop_rt_trap direct
        StringConst @scoop.str.0
    val y: Int
      Local $uw.2
  str @scoop.str.0 \"unwrap on None (function main)\"
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }

    /// `val a: Option<Int> = None; val b = Some(1); val r = a <op> b`
    /// — the shared shell of the enum equality tests.
    fn option_comparison(mut h: Harness, op: hir::BinOp) -> mir::Module {
        let (int, boolean) = (h.int, h.boolean);
        let option_int = h.option(int);
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
    fn enum_equality_compares_tags_then_payloads() {
        let h = Harness::new();
        let module = option_comparison(h, hir::BinOp::Eq);

        // Tags equal, and for the payload-carrying variant:
        // `tag != Some || payloads equal`. The unit variant (None) is
        // covered by the tag comparison alone.
        let expected = "\
Module
  enum Option$I
    Some(_1: Int)
    None()
  fun main @scoop_main() -> Unit
    val a: Option$I<Int>
      VariantConstruct Option$I<Int> v1
    val b: Option$I<Int>
      VariantConstruct Option$I<Int> v0
        IntLiteral 1
    val r: Boolean
      Binary And
        Binary IntEq
          EnumTag
            Local a
          EnumTag
            Local b
        Binary Or
          Binary IntNe
            EnumTag
              Local a
            IntLiteral 0
          Binary IntEq
            EnumField v0 f0
              Local a
            EnumField v0 f0
              Local b
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }

    #[test]
    fn enum_inequality_is_the_dual_tree() {
        let h = Harness::new();
        let module = option_comparison(h, hir::BinOp::Ne);

        // The De Morgan dual: `And` / `Or` swapped, the tag leaves
        // negated, the payload compared with `!=`.
        let expected = "\
Module
  enum Option$I
    Some(_1: Int)
    None()
  fun main @scoop_main() -> Unit
    val a: Option$I<Int>
      VariantConstruct Option$I<Int> v1
    val b: Option$I<Int>
      VariantConstruct Option$I<Int> v0
        IntLiteral 1
    val r: Boolean
      Binary Or
        Binary IntNe
          EnumTag
            Local a
          EnumTag
            Local b
        Binary And
          Binary IntEq
            EnumTag
              Local a
            IntLiteral 0
          Binary IntNe
            EnumField v0 f0
              Local a
            EnumField v0 f0
              Local b
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }

    fn when_stmt(
        subject: hir::Expr,
        arms: Vec<hir::WhenArm>,
        else_body: Option<Vec<hir::Statement>>,
    ) -> hir::Statement {
        stmt(hir::StatementKind::When(hir::When {
            subject,
            arms,
            else_body,
        }))
    }

    fn arm(
        pattern: hir::Pattern,
        guard: Option<hir::Expr>,
        body: Vec<hir::Statement>,
    ) -> hir::WhenArm {
        hir::WhenArm {
            pattern,
            guard,
            body,
            span: SPAN,
        }
    }

    #[test]
    fn when_lowers_to_a_decision_sequence() {
        // val o = Some(1); when (o) { Some(x) -> print(x); None -> println("none") }
        let mut h = Harness::new();
        let int = h.int;
        let option_enum = h.option_enum;
        let option_int = h.option(int);
        let mut locals = Arena::new();
        let o = locals.alloc(local("o", option_int));
        let x = locals.alloc(local("x", int));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![
                    val_decl(
                        o,
                        expr(
                            hir::ExprKind::SomeWrap(Box::new(int_lit(&h, 1))),
                            option_int,
                        ),
                    ),
                    when_stmt(
                        local_ref(o, option_int),
                        vec![
                            arm(
                                hir::Pattern::Variant {
                                    enum_id: option_enum,
                                    variant: 0,
                                    fields: vec![(0, hir::Pattern::Binding { local: x })],
                                },
                                None,
                                vec![expr_stmt(call(&h, h.print, vec![local_ref(x, int)]))],
                            ),
                            arm(
                                hir::Pattern::Variant {
                                    enum_id: option_enum,
                                    variant: 1,
                                    fields: Vec::new(),
                                },
                                None,
                                vec![expr_stmt(call(&h, h.println, vec![str_lit(&h, "none")]))],
                            ),
                        ],
                        None,
                    ),
                ],
            },
        );
        let module = lower(&h.finish(main));

        // The subject is evaluated once into `$when.1`; each arm is a
        // tag comparison, then the field bindings, then the body; a
        // failed tag test falls through to the next arm.
        let expected = "\
Module
  enum Option$I
    Some(_1: Int)
    None()
  fun main @scoop_main() -> Unit
    val o: Option$I<Int>
      VariantConstruct Option$I<Int> v0
        IntLiteral 1
    val $when.1: Option$I<Int>
      Local o
    if
      Binary IntEq
        EnumTag
          Local $when.1
        IntLiteral 0
      val x: Int
        EnumField v0 f0
          Local $when.1
      Call @scoop_rt_print_int direct
        Local x
    else
      if
        Binary IntEq
          EnumTag
            Local $when.1
          IntLiteral 1
        Call @scoop_rt_println direct
          StringConst @scoop.str.0
  str @scoop.str.0 \"none\"
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }

    #[test]
    fn a_failed_guard_falls_through_to_the_next_arm() {
        // when (o) { Some(x) if (x > 0) -> print(x); else -> println("neg") }
        let mut h = Harness::new();
        let int = h.int;
        let option_enum = h.option_enum;
        let option_int = h.option(int);
        let mut locals = Arena::new();
        let o = locals.alloc(local("o", option_int));
        let x = locals.alloc(local("x", int));
        let else_body = || vec![expr_stmt(call(&h, h.println, vec![str_lit(&h, "neg")]))];
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![when_stmt(
                    local_ref(o, option_int),
                    vec![arm(
                        hir::Pattern::Variant {
                            enum_id: option_enum,
                            variant: 0,
                            fields: vec![(0, hir::Pattern::Binding { local: x })],
                        },
                        Some(binary(
                            hir::BinOp::Gt,
                            local_ref(x, int),
                            int_lit(&h, 0),
                            h.boolean,
                        )),
                        vec![expr_stmt(call(&h, h.print, vec![local_ref(x, int)]))],
                    )],
                    Some(else_body()),
                )],
            },
        );
        let module = lower(&h.finish(main));

        // The guard nests inside the tag test's then branch; failing
        // it falls through to the next arm — the `else` body here,
        // which is lowered once per fallthrough edge.
        let expected = "\
Module
  enum Option$I
    Some(_1: Int)
    None()
  fun main @scoop_main() -> Unit
    val $when.1: Option$I<Int>
      Local o
    if
      Binary IntEq
        EnumTag
          Local $when.1
        IntLiteral 0
      val x: Int
        EnumField v0 f0
          Local $when.1
      if
        Binary IntGt
          Local x
          IntLiteral 0
        Call @scoop_rt_print_int direct
          Local x
      else
        Call @scoop_rt_println direct
          StringConst @scoop.str.0
    else
      Call @scoop_rt_println direct
        StringConst @scoop.str.1
  str @scoop.str.0 \"neg\"
  str @scoop.str.1 \"neg\"
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }

    #[test]
    fn literal_patterns_match_by_equality() {
        // when (n) { 1 -> println("one"); else -> println("other") }
        let mut h = Harness::new();
        let int = h.int;
        let mut locals = Arena::new();
        let n = locals.alloc(local("n", int));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![when_stmt(
                    local_ref(n, int),
                    vec![arm(
                        hir::Pattern::Literal(int_lit(&h, 1)),
                        None,
                        vec![expr_stmt(call(&h, h.println, vec![str_lit(&h, "one")]))],
                    )],
                    Some(vec![expr_stmt(call(
                        &h,
                        h.println,
                        vec![str_lit(&h, "other")],
                    ))]),
                )],
            },
        );
        let module = lower(&h.finish(main));

        let body = &module.functions[module.entry].body;
        // val $when.1 = n; if ($when.1 == 1) ... else ...
        let mir::StatementKind::If {
            cond:
                mir::Expr::Binary {
                    op: mir::BinOp::IntEq,
                    lhs,
                    rhs,
                },
            else_body: Some(_),
            ..
        } = &body.statements[1].kind
        else {
            panic!("a literal pattern must lower to an equality test")
        };
        assert!(matches!(
            lhs.as_ref(),
            mir::Expr::Local(local) if body.locals[*local].name == "$when.1"
        ));
        assert!(matches!(rhs.as_ref(), mir::Expr::IntLiteral(1)));
    }

    #[test]
    fn destructuring_val_declarations_extract_bindings() {
        // val (a, b) = (1, "x"); val Point { x, .. } = p
        let mut h = Harness::new();
        let (int, string) = (h.int, h.string);
        let point = h.strukt("Point", &[("x", int), ("y", int)]);
        let point_ty = h.types.alloc(hir::Type::Struct(point));
        let pair = h.tuple(&[int, string]);
        let mut locals = Arena::new();
        let a = locals.alloc(local("a", int));
        let b = locals.alloc(local("b", string));
        let p = locals.alloc(local("p", point_ty));
        let x = locals.alloc(local("x", int));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![
                    stmt(hir::StatementKind::ValDecl {
                        pattern: hir::Pattern::Tuple(vec![
                            hir::Pattern::Binding { local: a },
                            hir::Pattern::Binding { local: b },
                        ]),
                        init: expr(
                            hir::ExprKind::TupleLiteral(vec![int_lit(&h, 1), str_lit(&h, "x")]),
                            pair,
                        ),
                    }),
                    val_decl(
                        p,
                        struct_init(point, point_ty, vec![int_lit(&h, 3), int_lit(&h, 4)]),
                    ),
                    stmt(hir::StatementKind::ValDecl {
                        pattern: hir::Pattern::Struct {
                            struct_id: point,
                            fields: vec![(0, hir::Pattern::Binding { local: x })],
                        },
                        init: local_ref(p, point_ty),
                    }),
                ],
            },
        );
        let module = lower(&h.finish(main));

        // Each destructuring declaration evaluates its init once into
        // a hidden local, then binds the extracted fields.
        let expected = "\
Module
  struct Point (x: Int, y: Int)
  fun main @scoop_main() -> Unit
    val $bind.1: (Int, String)
      TupleLiteral
        IntLiteral 1
        StringConst @scoop.str.0
    val a: Int
      FieldAccess 0
        Local $bind.1
    val b: String
      FieldAccess 1
        Local $bind.1
    val p: Point
      StructInit Point
        IntLiteral 3
        IntLiteral 4
    val $bind.2: Point
      Local p
    val x: Int
      FieldAccess 0
        Local $bind.2
  str @scoop.str.0 \"x\"
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }
}
