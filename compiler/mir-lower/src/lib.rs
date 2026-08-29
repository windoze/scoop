//! MIR stage: monomorphization, name mangling, call-kind annotation,
//! vtable/itable construction, suspend-to-state-machine lowering.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.3 and
//! `docs/milestone4/DESIGN.md` section 3.3.
//!
//! M2: value types. HIR types are mapped onto MIR types (the struct
//! arena is transposed in declaration order, field types recursively);
//! structural equality on aggregates is expanded into primitive
//! comparisons and runtime calls; String `+` becomes
//! `scoop_rt_string_concat`.
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
//! else branch throws `UnwrapException` (M8). Equality on enums expands
//! into a tag comparison plus a per-variant payload comparison.
//!
//! M8: exceptions (docs/milestone8/DESIGN.md section 3.3). `try` /
//! `catch` / `finally` and `throw` translate one-to-one — MIR keeps
//! them structured; the control-flow expansion (invoke / landingpad)
//! is LIR's job. The four trap paths of M3–M6 now throw core's
//! built-in exceptions instead of calling `scoop_rt_trap`: `!!`
//! throws `UnwrapException`, the array bounds checks (moved here from
//! codegen for `ArrayGet` / `ArraySet`) throw
//! `IndexOutOfBoundsException`, a failing `as` throws
//! `ClassCastException`, and integer division gains a divisor check
//! that throws `ArithmeticException`. The built-in exception classes
//! (core's throwable.scoop) are ordinary classes, so construction is
//! a plain call to the generated constructor function (M6); they are
//! resolved by name — hir-lower validates that core declares
//! `Throwable`, so a missing class is a core configuration error.
//! `RuntimeFn::Trap` keeps exactly one generation path: the
//! abstract-method stub (a cannot-happen pure-virtual trap).
//!
//! M7: print/println are ordinary core functions
//! (docs/milestone7/DESIGN.md section 2) — their calls go through the
//! normal function path. `@Intrinsic` calls map by intrinsic name onto
//! the runtime functions: `rt_write` → `Write` (`scoop_rt_print`),
//! `rt_int_to_string` → `IntToString`, `rt_bool_to_string` →
//! `BoolToString` (the latter two back the generated `toString`
//! bodies below). Mangling is overload-aware: a name shared by
//! several plainly-mangled functions gets the parameter encoding
//! appended (`scoop.show.I`, `scoop.println.S`; the receiver is not
//! part of a method's overload signature), while unique names keep the
//! plain `scoop.<name>` form and instances keep `$` (`scoop.show$I`),
//! so overload and instance symbols never collide. Dispatch is keyed
//! by signature the same way: vtable / itable slots and call-kind
//! annotation use `name(<param encoding>)`, so each overload gets its
//! own slot and an override replaces the base slot with the matching
//! signature in place. core's output goes through `Any.toString()`:
//! the synthesized `Any` members are signature-only shells (like
//! interface methods, never emitted), and a boxed primitive's vtable
//! slot 2 is a generated per-type `toString` (`scoop.tostring.I` /
//! `scoop.tostring.B`, converting through the runtime); String's
//! TypeDescriptor vtable is emitted by codegen with slot 2 bound to
//! the runtime String identity.
//!
//! M5: arrays (docs/milestone5/DESIGN.md). `Array<T>` /
//! `MutableArray<T>` map onto the corresponding MIR types, and the
//! array nodes translate one-to-one: literals, subscript reads, `size`,
//! `m[i] = v` (an `ArraySet` statement), and the `Array(m)` /
//! `MutableArray(a)` conversions (`ArrayClone`). There is no array
//! equality in M5 (DESIGN 6): hir-lower rejects `==` / `!=` on array
//! types, so the equality expansion treats them as unreachable.
//!
//! M6: reference types (docs/milestone6/DESIGN.md). Classes land as
//! `mir::ClassDef` with the object layout flattened (base-class
//! fields first, then the constructor properties — the same indexing
//! HIR's `ClassField` uses) and the dispatch layout fixed (impl spec
//! 2.9): vtable slots 0..2 are the `Any` defaults, a derived vtable
//! starts from the base's (overrides replace the base slot in place,
//! new methods append in declaration order), and every implemented
//! interface gets an itable record whose slots follow the interface's
//! method declaration order. Method calls are annotated by the
//! receiver's static type: class receiver → `Virtual`, interface
//! receiver → `Interface`, value type → `Direct`; member functions
//! are mangled qualified (`scoop.Point.describe`) so same-named
//! methods never collide. Every value type that reaches `Any` / an
//! interface (`Box`, `is`, `as`) gets a boxed `ClassDef` (`box$<ty>`):
//! vtable slot 0 is the compiler-generated structural equals
//! (`scoop.eq.<ty>`, built with the M2 equality expansion over the
//! unboxed payloads), slots 1/2 stay the `Any` defaults, and its
//! itable slots point at adjust thunks that unbox `this` and
//! tail-call the real value method. The boxed itables cover the value
//! type's *declared* interfaces (spec 4.4.3) no matter what it was
//! boxed to. `as` throws `ClassCastException` on failure (M8); `as?`
//! wraps in `Option` like `!!` does. Equality on references is
//! identity (the M6 `Any` default, milestone6 DESIGN 5.1) — a pointer
//! comparison; `===` / `!==` (`RefEq` / `RefNe`) map onto the same
//! primitive comparison. Class construction is function-ized: every
//! non-abstract class gets a `scoop.ctor.<Class>` function whose
//! parameters are the constructor properties and whose body returns a
//! raw `mir::Expr::ClassInit` over the flattened field values (the
//! base delegation arguments are evaluated in the ctor context —
//! hir-lower M6 lowers them in an empty scope — and expanded
//! recursively down the base chain; base ctors are never called, so
//! the object identity is a single allocation). A
//! `hir::ExprKind::ClassInit` at a use site becomes a plain `Direct`
//! call to that function.

use std::collections::{HashMap, HashSet};

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
        classes: Arena::new(),
        class_map: HashMap::new(),
        interfaces: Arena::new(),
        interface_map: HashMap::new(),
        method_slots: HashMap::new(),
        function_map: HashMap::new(),
        instances: InstanceRegistry::default(),
        enums: EnumRegistry::default(),
        boxed: BoxedRegistry::default(),
        ctors: HashMap::new(),
        shell: mangling_shell(&Arena::new(), &Arena::new(), &Arena::new()),
        overloaded: overloaded_names(module),
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
    classes: Arena<mir::ClassDef>,
    /// HIR class -> MIR class (arena transposed in declaration order).
    class_map: HashMap<hir::ClassId, mir::ClassId>,
    interfaces: Arena<mir::InterfaceDef>,
    /// HIR interface -> MIR interface (arena transposed in declaration
    /// order).
    interface_map: HashMap<hir::InterfaceId, mir::InterfaceId>,
    /// Method signature key (`fn_signature_key`) -> vtable slot, per
    /// class (computed by `compute_dispatch`; `BodyLowerer` reads it
    /// for call-kind annotation).
    method_slots: HashMap<mir::ClassId, HashMap<String, u32>>,
    /// HIR user function -> MIR function (non-generic functions only;
    /// generic functions resolve through `instances`).
    function_map: HashMap<hir::FunctionId, mir::FunctionId>,
    instances: InstanceRegistry,
    enums: EnumRegistry,
    /// Boxed value types discovered while lowering bodies.
    boxed: BoxedRegistry,
    /// HIR class -> its generated constructor function (every
    /// non-abstract class; see `declare_ctors`).
    ctors: HashMap<hir::ClassId, mir::FunctionId>,
    /// Mangling shell: the struct / enum / class / interface names
    /// `mir::encode_type` reads, kept in sync with the real arenas
    /// (same ids).
    shell: mir::Module,
    /// Names shared by more than one plainly-mangled function (M7
    /// overloads): each of them gets the parameter encoding appended
    /// to its symbol (see `declare_symbol`).
    overloaded: HashSet<String>,
    /// Declaration indices of `Option`'s `Some` / `None` variants.
    option_variants: (u32, u32),
}

impl Lowerer {
    fn run(mut self, module: &hir::Module) -> mir::Module {
        // Struct / interface / class ids first (types can reference
        // any of them regardless of declaration order), then the
        // mangling shell (their names for `encode_type`), then the
        // field types themselves — which can instantiate enums.
        self.lower_structs(module);
        self.lower_interfaces(module);
        self.declare_classes(module);
        self.shell = mangling_shell(&self.structs, &self.classes, &self.interfaces);
        self.fill_struct_fields(module);
        self.option_variants = option_variants(module);
        // Classes are processed base-before-derived: the object layout
        // and the vtable both keep the base's as a prefix.
        let class_order = topo_class_order(module);
        self.fill_class_fields(module, &class_order);

        // Declare non-generic user functions first, so calls resolve
        // regardless of declaration order. Intrinsics have no body;
        // their callsites map to `Callee::Runtime` shims (see
        // `BodyLowerer::lower_call`). Generic functions have no MIR
        // body of their own — only their monomorphized instances do.
        // Member functions are declared too (hir-lower keeps them out
        // of `top_level`); interface methods become signature-only
        // shells (M6 interfaces have no default implementations).
        let mut user_functions = Vec::new();
        for &hir_id in &module.top_level {
            let function = &module.functions[hir_id];
            if !matches!(function.kind, hir::FunctionKind::User(_)) {
                continue;
            }
            if !function.type_params.is_empty() {
                continue;
            }
            let id = self.declare_function(module, hir_id);
            user_functions.push((hir_id, id));
        }
        for (hir_id, function) in module.functions.iter() {
            if self.function_map.contains_key(&hir_id)
                || !matches!(function.kind, hir::FunctionKind::User(_))
                || !function.type_params.is_empty()
            {
                continue;
            }
            match function.method_of {
                Some(ty)
                    if matches!(module.types[ty], hir::Type::Interface(_) | hir::Type::Any) => {}
                Some(_) => {
                    let id = self.declare_function(module, hir_id);
                    user_functions.push((hir_id, id));
                }
                // A free function outside `top_level` cannot happen
                // (hir-lower lists them all).
                None => {}
            }
        }
        for (hir_id, function) in module.functions.iter() {
            let Some(ty) = function.method_of else {
                continue;
            };
            // Interface methods and the synthesized `Any` members
            // (`equals` / `hashCode` / `toString`) are signature-only
            // shells: dispatch goes through the vtable prefix / itable,
            // so the functions themselves are never emitted — their
            // declarations only give LIR the parameter / return types
            // for the indirect call.
            if matches!(module.types[ty], hir::Type::Interface(_) | hir::Type::Any) {
                self.declare_interface_method(module, hir_id);
            }
        }
        // Constructor functions: one per non-abstract class, declared
        // like ordinary functions so `ClassInit` call sites resolve.
        let mut ctor_functions = Vec::new();
        for (hir_id, decl) in module.classes.iter() {
            if decl.modifier == hir::ClassModifier::Abstract {
                continue;
            }
            let id = self.declare_ctor(module, hir_id);
            ctor_functions.push((hir_id, id));
        }

        // The dispatch layout (vtable / itable slots) needs every
        // method declared; the bodies need it for call-kind
        // annotation.
        self.compute_dispatch(module, &class_order);

        for (hir_id, mir_id) in user_functions {
            let (params, return_ty, body) = self.lower_user_function(module, hir_id, None);
            let function = &mut self.functions[mir_id];
            function.params = params;
            function.return_ty = return_ty;
            function.body = body;
        }
        for (hir_id, mir_id) in ctor_functions {
            let (params, return_ty, body) = self.lower_ctor(module, hir_id);
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
                class_map: &self.class_map,
                interface_map: &self.interface_map,
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

        // Boxed value types are discovered while lowering bodies
        // (`Box` / `is` / `as`); their generated members (the
        // structural equals and the adjust thunks) come last.
        self.finalize_boxed(module);

        // The entry point is a non-generic user function, hence always
        // in the map.
        let entry = self.function_map[&module.entry];
        mir::Module {
            functions: self.functions,
            top_level: self.top_level,
            strings: self.strings,
            structs: self.structs,
            enums: self.enums.defs,
            classes: self.classes,
            interfaces: self.interfaces,
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
            class_map: &self.class_map,
            interface_map: &self.interface_map,
            structs: &self.structs,
            method_slots: &self.method_slots,
            function_map: &self.function_map,
            ctors: &self.ctors,
            strings: &mut self.strings,
            functions: &mut self.functions,
            top_level: &mut self.top_level,
            instances: &mut self.instances,
            enums: &mut self.enums,
            boxed: &mut self.boxed,
            classes: &mut self.classes,
            shell: &mut self.shell,
            subst,
            local_map: HashMap::new(),
            locals: Arena::new(),
            hidden_count: 0,
            prelude: Vec::new(),
            option_variants: self.option_variants,
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
                class_map: &self.class_map,
                interface_map: &self.interface_map,
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

    /// Transpose the HIR interface arena into MIR in declaration
    /// order: the method names in declaration order are the itable
    /// slot indices.
    fn lower_interfaces(&mut self, module: &hir::Module) {
        for (hir_id, decl) in module.interfaces.iter() {
            let mir_id = self.interfaces.alloc(mir::InterfaceDef {
                name: decl.name.clone(),
                methods: decl
                    .methods
                    .iter()
                    .map(|method| method.name.clone())
                    .collect(),
            });
            self.interface_map.insert(hir_id, mir_id);
        }
    }

    /// Transpose the HIR class arena into MIR in declaration order.
    /// Fields / vtable / itables are filled later (they need the base
    /// class and the method list, respectively).
    fn declare_classes(&mut self, module: &hir::Module) {
        for (hir_id, decl) in module.classes.iter() {
            let modifier = match decl.modifier {
                hir::ClassModifier::Final => mir::ClassModifier::Final,
                hir::ClassModifier::Open => mir::ClassModifier::Open,
                hir::ClassModifier::Abstract => mir::ClassModifier::Abstract,
            };
            let mir_id = self.classes.alloc(mir::ClassDef {
                modifier,
                name: decl.name.clone(),
                fields: Vec::new(),
                base_class: None,
                interfaces: Vec::new(),
                vtable: Vec::new(),
                itables: Vec::new(),
            });
            self.class_map.insert(hir_id, mir_id);
        }
        for (hir_id, decl) in module.classes.iter() {
            let mir_id = self.class_map[&hir_id];
            let base_class = decl
                .base_class
                .as_ref()
                .map(|(base, _)| self.class_map[base]);
            let interfaces = decl
                .interfaces
                .iter()
                .map(|iface| self.interface_map[iface])
                .collect();
            let class = &mut self.classes[mir_id];
            class.base_class = base_class;
            class.interfaces = interfaces;
        }
    }

    /// A declared function's symbol: `scoop.<name>` (the fixed
    /// `scoop_main` for the entry point). When the name is shared by
    /// overloads (M7), the parameter encoding is appended so each
    /// overload gets a distinct LLVM symbol: `scoop.show.I`,
    /// `scoop.println.S`, `scoop.Doc.describe.I` for methods (see
    /// `mir::mangle_overload`). vtable / itable slots and thunk calls
    /// reference functions by id, so they pick the final symbol up
    /// from the arena automatically.
    fn declare_symbol(&mut self, module: &hir::Module, hir_id: hir::FunctionId) -> String {
        let function = &module.functions[hir_id];
        let name = fn_name(function);
        if hir_id == module.entry || !self.overloaded.contains(&name) {
            return mir::mangle_function(&name, hir_id == module.entry);
        }
        // A method's receiver (parameter 0, hir-lower's contract) is
        // not part of the overload signature: `Doc.describe(Int)`
        // encodes as `scoop.Doc.describe.I`.
        let skip = usize::from(function.method_of.is_some());
        let params = self.lower_params(module, &function.params[skip..]);
        mir::mangle_overload(&self.shell, &name, &params)
    }

    /// Lower a parameter list to MIR types (enum instantiations
    /// register on demand).
    fn lower_params(&mut self, module: &hir::Module, params: &[hir::Param]) -> Vec<mir::Type> {
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
            interface_map: &self.interface_map,
            subst: None,
        };
        params
            .iter()
            .map(|param| types.lower(param.ty, &mut self.enums, &mut self.shell))
            .collect()
    }

    /// The `_`-joined `mir::encode_type` encoding of a parameter list.
    fn param_encoding(&mut self, module: &hir::Module, params: &[hir::Param]) -> String {
        let params = self.lower_params(module, params);
        mir::encode_params(&self.shell, &params)
    }

    /// A method's dispatch signature key: `name(<param encoding>)`
    /// over the declared parameters (the receiver is not part of it)
    /// — `describe(I)`, `m(I_S)`, `f()`. An override shares the base
    /// method's key (hir-lower enforces exact-signature overriding),
    /// so keying vtable slots by it replaces the base slot in place,
    /// while overloads get distinct keys and thus distinct slots.
    fn fn_signature_key(&mut self, module: &hir::Module, function: &hir::Function) -> String {
        let skip = usize::from(function.method_of.is_some());
        let encoding = self.param_encoding(module, &function.params[skip..]);
        format!("{}({encoding})", short_name(&function.name))
    }

    /// The dispatch signature key of an interface method signature.
    fn sig_signature_key(&mut self, module: &hir::Module, sig: &hir::MethodSig) -> String {
        let encoding = self.param_encoding(module, &sig.params);
        format!("{}({encoding})", sig.name)
    }

    /// Declare one non-generic user function (body filled later):
    /// `scoop.<name>`, `scoop.<Type>.<name>` for members, or the fixed
    /// entry symbol `scoop_main` that the C runtime calls (`main` is
    /// never generic, hir-lower guarantees it); overloads get the
    /// parameter encoding appended (`declare_symbol`).
    fn declare_function(
        &mut self,
        module: &hir::Module,
        hir_id: hir::FunctionId,
    ) -> mir::FunctionId {
        let function = &module.functions[hir_id];
        let name = fn_name(function);
        let symbol = self.declare_symbol(module, hir_id);
        let id = self.functions.alloc(mir::Function {
            name,
            symbol,
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
        id
    }

    /// Declare an interface method or a synthesized `Any` member: a
    /// signature-only shell that is never emitted (not in
    /// `top_level`). It exists so virtual / interface calls can name a
    /// callee whose parameter / return types LIR reads for the
    /// indirect call.
    fn declare_interface_method(&mut self, module: &hir::Module, hir_id: hir::FunctionId) {
        let function = &module.functions[hir_id];
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
            interface_map: &self.interface_map,
            subst: None,
        };
        let mut locals = Arena::new();
        let params = function
            .params
            .iter()
            .map(|param| {
                let ty = types.lower(param.ty, &mut self.enums, &mut self.shell);
                let local = locals.alloc(mir::Local {
                    name: param.name.clone(),
                    ty: ty.clone(),
                    mutable: false,
                });
                mir::Param {
                    name: param.name.clone(),
                    ty,
                    local,
                }
            })
            .collect();
        let return_ty = types.lower(function.return_ty, &mut self.enums, &mut self.shell);
        let name = fn_name(function);
        let symbol = self.declare_symbol(module, hir_id);
        let id = self.functions.alloc(mir::Function {
            symbol,
            name,
            params,
            return_ty,
            body: mir::Body {
                locals,
                statements: Vec::new(),
            },
        });
        self.function_map.insert(hir_id, id);
    }

    /// Fill the MIR class fields: the base class's (already
    /// flattened) fields come first — the object layout and HIR's
    /// `ClassField` indices follow the same order — then the
    /// constructor properties in declaration order.
    fn fill_class_fields(&mut self, module: &hir::Module, order: &[hir::ClassId]) {
        for &hir_id in order {
            let decl = &module.classes[hir_id];
            let mir_id = self.class_map[&hir_id];
            let mut fields = match decl.base_class {
                Some((base, _)) => clone_fields(&self.classes[self.class_map[&base]].fields),
                None => Vec::new(),
            };
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
                interface_map: &self.interface_map,
                subst: None,
            };
            for field in &decl.constructor {
                fields.push(mir::Field {
                    name: field.name.clone(),
                    ty: types.lower(field.ty, &mut self.enums, &mut self.shell),
                });
            }
            self.classes[mir_id].fields = fields;
        }
    }

    /// Fix every class's vtable and itables (impl spec 2.9): vtable
    /// slots 0..2 are the `Any` defaults; a derived vtable starts
    /// from the base's (prefix preserved), an override replaces the
    /// base slot in place, new methods append in declaration order
    /// (generic methods never enter the vtable — impl spec 2.9).
    /// Slots are keyed by the method's signature (`fn_signature_key`):
    /// an override shares the base method's key and replaces its slot,
    /// while same-named overloads have distinct keys and get distinct
    /// slots (M7). itables cover the interfaces the base class covered
    /// (records first, in the base's order) plus the ones the class
    /// declares, each slot resolved to the implementation visible from
    /// the class (its own override first, then up the base chain) —
    /// again matched by signature, so an overloaded interface gets one
    /// slot per method signature.
    fn compute_dispatch(&mut self, module: &hir::Module, order: &[hir::ClassId]) {
        for &hir_id in order {
            let mir_id = self.class_map[&hir_id];
            let decl = &module.classes[hir_id];
            let (mut vtable, mut slots) = match decl.base_class {
                Some((base, _)) => {
                    let base = self.class_map[&base];
                    (
                        clone_slots(&self.classes[base].vtable),
                        self.method_slots[&base].clone(),
                    )
                }
                None => (
                    vec![
                        mir::TableSlot::Runtime(mir::RuntimeFn::AnyEquals),
                        mir::TableSlot::Runtime(mir::RuntimeFn::AnyHashCode),
                        mir::TableSlot::Runtime(mir::RuntimeFn::AnyToString),
                    ],
                    HashMap::new(),
                ),
            };
            for (fn_id, function) in module.functions.iter() {
                if method_class(module, function) != Some(hir_id)
                    || !function.type_params.is_empty()
                {
                    continue;
                }
                let mir_fn = self.function_map[&fn_id];
                let key = self.fn_signature_key(module, function);
                match slots.get(&key) {
                    Some(&slot) => vtable[slot as usize] = mir::TableSlot::Function(mir_fn),
                    None => {
                        slots.insert(key, vtable.len() as u32);
                        vtable.push(mir::TableSlot::Function(mir_fn));
                    }
                }
            }
            let mut covered: Vec<mir::InterfaceId> = match decl.base_class {
                Some((base, _)) => self.classes[self.class_map[&base]]
                    .itables
                    .iter()
                    .map(|record| record.interface)
                    .collect(),
                None => Vec::new(),
            };
            for &iface in &decl.interfaces {
                let mir_iface = self.interface_map[&iface];
                if !covered.contains(&mir_iface) {
                    covered.push(mir_iface);
                }
            }
            let mut itables = Vec::new();
            for mir_iface in covered {
                let hir_iface = self.hir_interface(mir_iface);
                let mut slots_for = Vec::new();
                for method in &module.interfaces[hir_iface].methods {
                    let key = self.sig_signature_key(module, method);
                    slots_for.push(mir::TableSlot::Function(
                        self.find_impl(module, hir_id, &key),
                    ));
                }
                itables.push(mir::ItableRecord {
                    interface: mir_iface,
                    slots: slots_for,
                });
            }
            let class = &mut self.classes[mir_id];
            class.vtable = vtable;
            class.itables = itables;
            self.method_slots.insert(mir_id, slots);
        }
    }

    /// Declare the constructor function of one class (`scoop.ctor.
    /// <Class>`): parameters are the constructor properties in
    /// declaration order; the body is filled by `lower_ctor`.
    fn declare_ctor(&mut self, module: &hir::Module, hir_id: hir::ClassId) -> mir::FunctionId {
        let decl = &module.classes[hir_id];
        let name = format!("ctor.{}", decl.name);
        let id = self.functions.alloc(mir::Function {
            symbol: format!("scoop.{name}"),
            name,
            params: Vec::new(),
            return_ty: mir::Type::Unit,
            body: mir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            },
        });
        self.top_level.push(id);
        self.ctors.insert(hir_id, id);
        id
    }

    /// Lower the constructor function's body: a single raw
    /// `mir::Expr::ClassInit` over the flattened field values — the
    /// base delegation arguments (evaluated here in the ctor context;
    /// hir-lower M6 lowers them in an empty scope, so they are closed
    /// expressions) expanded recursively down the base chain, then
    /// the class's own constructor properties. Base ctors are never
    /// called: the flattened fields are written in one shot, so the
    /// object identity is a single allocation.
    fn lower_ctor(
        &mut self,
        module: &hir::Module,
        hir_id: hir::ClassId,
    ) -> (Vec<mir::Param>, mir::Type, mir::Body) {
        let decl = &module.classes[hir_id];
        let mir_id = self.class_map[&hir_id];
        let field_count = self.classes[mir_id].fields.len();
        let mut lowerer = BodyLowerer {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
            interface_map: &self.interface_map,
            structs: &self.structs,
            method_slots: &self.method_slots,
            function_map: &self.function_map,
            ctors: &self.ctors,
            strings: &mut self.strings,
            functions: &mut self.functions,
            top_level: &mut self.top_level,
            instances: &mut self.instances,
            enums: &mut self.enums,
            boxed: &mut self.boxed,
            classes: &mut self.classes,
            shell: &mut self.shell,
            subst: None,
            // Delegation arguments are closed (hir-lower M6 lowers
            // them in an empty scope), so no locals are visible.
            local_map: HashMap::new(),
            locals: Arena::new(),
            hidden_count: 0,
            prelude: Vec::new(),
            option_variants: self.option_variants,
        };
        let mut params = Vec::new();
        let mut own = Vec::new();
        for field in &decl.constructor {
            let ty = lowerer.lower_type(field.ty);
            let local = lowerer.locals.alloc(mir::Local {
                name: field.name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            params.push(mir::Param {
                name: field.name.clone(),
                ty,
                local,
            });
            own.push(mir::Expr::Local(local));
        }
        let args = flattened_ctor_args(&mut lowerer, module, hir_id, own);
        assert_eq!(
            args.len(),
            field_count,
            "the flattened initializer covers every field"
        );
        let body = mir::Body {
            locals: lowerer.locals,
            statements: vec![mir::Statement {
                kind: mir::StatementKind::Return {
                    value: Some(mir::Expr::ClassInit {
                        class_id: mir_id,
                        args,
                    }),
                },
                span: decl.span,
            }],
        };
        (params, mir::Type::Class(mir_id), body)
    }

    /// The HIR id behind a MIR interface (the arenas are transposed
    /// 1:1).
    fn hir_interface(&self, mir_id: mir::InterfaceId) -> hir::InterfaceId {
        self.interface_map
            .iter()
            .find(|(_, mir)| **mir == mir_id)
            .map(|(&hir, _)| hir)
            .expect("every MIR interface comes from a HIR interface")
    }

    /// The function implementing the signature `key` for class
    /// `hir_id`: the class's own method first, then up the base chain
    /// (matched by signature — `fn_signature_key` — so overloads
    /// resolve to their own implementation).
    fn find_impl(
        &mut self,
        module: &hir::Module,
        hir_id: hir::ClassId,
        key: &str,
    ) -> mir::FunctionId {
        let mut current = Some(hir_id);
        while let Some(class) = current {
            for (fn_id, function) in module.functions.iter() {
                if method_class(module, function) != Some(class) || !function.type_params.is_empty()
                {
                    continue;
                }
                if self.fn_signature_key(module, function) == key {
                    return self.function_map[&fn_id];
                }
            }
            current = module.classes[class]
                .base_class
                .as_ref()
                .map(|(base, _)| *base);
        }
        unreachable!("hir-lower guarantees `{key}` is implemented")
    }

    /// Generate the boxed value types' dispatch members (DESIGN 2.3):
    /// slot 0 of a boxed vtable is the compiler-generated structural
    /// equals (`scoop.eq.<ty>`; `hashCode` stays the `Any` default),
    /// slot 2 is a per-type `toString` for the primitives with a
    /// runtime conversion (`scoop.tostring.<ty>`, M7 — `Int` /
    /// `Boolean`; aggregate value types keep the `Any` default
    /// `scoop_rt_any_tostring` until a spec'd structured format
    /// lands), and every interface the value type was boxed to gets
    /// an itable whose slots point at adjust thunks — the thunk's
    /// `this` is the boxed object; it unboxes and tail-calls the real
    /// value method.
    fn finalize_boxed(&mut self, module: &hir::Module) {
        for index in 0..self.boxed.order.len() {
            let class_id = self.boxed.order[index];
            let payload = self.classes[class_id].fields[0].ty.clone();
            let encoded = mir::encode_type(&self.shell, &payload);
            let equals = self.build_boxed_equals(module, &payload, &encoded);
            let tostring = match payload {
                mir::Type::Int => {
                    let f = self.build_boxed_tostring(&encoded, mir::RuntimeFn::IntToString);
                    mir::TableSlot::Function(f)
                }
                mir::Type::Boolean => {
                    let f = self.build_boxed_tostring(&encoded, mir::RuntimeFn::BoolToString);
                    mir::TableSlot::Function(f)
                }
                _ => mir::TableSlot::Runtime(mir::RuntimeFn::AnyToString),
            };
            self.classes[class_id].vtable = vec![
                mir::TableSlot::Function(equals),
                mir::TableSlot::Runtime(mir::RuntimeFn::AnyHashCode),
                tostring,
            ];
            let interfaces = self.classes[class_id].interfaces.clone();
            for iface in interfaces {
                let hir_iface = self.hir_interface(iface);
                let method_count = module.interfaces[hir_iface].methods.len();
                let mut slots = Vec::new();
                for index in 0..method_count {
                    let thunk =
                        self.build_thunk(module, &payload, &encoded, iface, hir_iface, index);
                    slots.push(mir::TableSlot::Function(thunk));
                }
                self.classes[class_id].itables.push(mir::ItableRecord {
                    interface: iface,
                    slots,
                });
            }
        }
    }

    /// `scoop.eq.<ty>`: the structural `equals` of a boxed value
    /// type — unbox both payloads and compare with the M2/M4
    /// equality expansion. The signature matches the `Any` vtable
    /// slot (`(this: Any, other: Any) -> Boolean`) so call sites
    /// dispatch uniformly.
    fn build_boxed_equals(
        &mut self,
        module: &hir::Module,
        payload: &mir::Type,
        encoded: &str,
    ) -> mir::FunctionId {
        let mut locals = Arena::new();
        let this = locals.alloc(mir::Local {
            name: "this".to_string(),
            ty: mir::Type::Any,
            mutable: false,
        });
        let other = locals.alloc(mir::Local {
            name: "other".to_string(),
            ty: mir::Type::Any,
            mutable: false,
        });
        let a = locals.alloc(mir::Local {
            name: "$a".to_string(),
            ty: payload.clone(),
            mutable: false,
        });
        let b = locals.alloc(mir::Local {
            name: "$b".to_string(),
            ty: payload.clone(),
            mutable: false,
        });
        let mut lowerer = BodyLowerer {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
            interface_map: &self.interface_map,
            structs: &self.structs,
            method_slots: &self.method_slots,
            function_map: &self.function_map,
            ctors: &self.ctors,
            strings: &mut self.strings,
            functions: &mut self.functions,
            top_level: &mut self.top_level,
            instances: &mut self.instances,
            enums: &mut self.enums,
            boxed: &mut self.boxed,
            classes: &mut self.classes,
            shell: &mut self.shell,
            subst: None,
            local_map: HashMap::new(),
            locals,
            hidden_count: 0,
            prelude: Vec::new(),
            option_variants: self.option_variants,
        };
        let equality = lowerer.expand_equality(&Opd::Local(a), &Opd::Local(b), payload, &[], false);
        let body = mir::Body {
            locals: lowerer.locals,
            statements: vec![
                mir::Statement {
                    kind: mir::StatementKind::ValDecl {
                        local: a,
                        init: mir::Expr::Unbox(Box::new(mir::Expr::Local(this))),
                    },
                    span: Span { start: 0, end: 0 },
                },
                mir::Statement {
                    kind: mir::StatementKind::ValDecl {
                        local: b,
                        init: mir::Expr::Unbox(Box::new(mir::Expr::Local(other))),
                    },
                    span: Span { start: 0, end: 0 },
                },
                mir::Statement {
                    kind: mir::StatementKind::Return {
                        value: Some(equality),
                    },
                    span: Span { start: 0, end: 0 },
                },
            ],
        };
        let name = format!("eq.{encoded}");
        let id = self.functions.alloc(mir::Function {
            symbol: format!("scoop.{name}"),
            name,
            params: vec![
                mir::Param {
                    name: "this".to_string(),
                    ty: mir::Type::Any,
                    local: this,
                },
                mir::Param {
                    name: "other".to_string(),
                    ty: mir::Type::Any,
                    local: other,
                },
            ],
            return_ty: mir::Type::Boolean,
            body,
        });
        self.top_level.push(id);
        id
    }

    /// `scoop.tostring.<ty>`: the `toString` implementation of a boxed
    /// primitive (`Int` / `Boolean`, M7) — unbox the payload and
    /// convert it through the runtime (`scoop_rt_int_to_string` /
    /// `scoop_rt_bool_to_string`). The signature matches the `Any`
    /// vtable slot (`(this: Any) -> String`) so `Any.toString()`
    /// dispatches uniformly; core's `print` / `println` rely on it.
    fn build_boxed_tostring(&mut self, encoded: &str, convert: mir::RuntimeFn) -> mir::FunctionId {
        let mut locals = Arena::new();
        let this = locals.alloc(mir::Local {
            name: "this".to_string(),
            ty: mir::Type::Any,
            mutable: false,
        });
        let name = format!("tostring.{encoded}");
        let id = self.functions.alloc(mir::Function {
            symbol: format!("scoop.{name}"),
            name,
            params: vec![mir::Param {
                name: "this".to_string(),
                ty: mir::Type::Any,
                local: this,
            }],
            return_ty: mir::Type::String,
            body: mir::Body {
                locals,
                statements: vec![mir::Statement {
                    kind: mir::StatementKind::Return {
                        value: Some(mir::Expr::Call(mir::Call {
                            target: mir::CallTarget {
                                kind: mir::CallKind::Direct,
                                callee: mir::Callee::Runtime(convert),
                            },
                            args: vec![mir::Expr::Unbox(Box::new(mir::Expr::Local(this)))],
                        })),
                    },
                    span: Span { start: 0, end: 0 },
                }],
            },
        });
        self.top_level.push(id);
        id
    }

    /// The adjust thunk for one (boxed value type, interface method)
    /// pair (impl spec 2.9): `this` is the boxed object; the thunk
    /// unboxes it and tail-calls the real value method (value-type
    /// methods take `this` by value at MIR; the pointer convention
    /// of the receiver is a codegen ABI matter). The implementation
    /// is matched by signature, so overloaded interface methods get
    /// one thunk each; the thunk symbol carries the parameter
    /// encoding when the interface overloads the name.
    fn build_thunk(
        &mut self,
        module: &hir::Module,
        payload: &mir::Type,
        encoded: &str,
        iface: mir::InterfaceId,
        hir_iface: hir::InterfaceId,
        method_index: usize,
    ) -> mir::FunctionId {
        let signature = &module.interfaces[hir_iface].methods[method_index];
        let encoding = self.param_encoding(module, &signature.params);
        let key = format!("{}({encoding})", signature.name);
        let impl_fn = self.value_method(module, payload, &key);
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
            interface_map: &self.interface_map,
            subst: None,
        };
        let mut locals = Arena::new();
        let this = locals.alloc(mir::Local {
            name: "this".to_string(),
            ty: mir::Type::Any,
            mutable: false,
        });
        let mut params = vec![mir::Param {
            name: "this".to_string(),
            ty: mir::Type::Any,
            local: this,
        }];
        let mut args = vec![mir::Expr::Unbox(Box::new(mir::Expr::Local(this)))];
        for param in &signature.params {
            let ty = types.lower(param.ty, &mut self.enums, &mut self.shell);
            let local = locals.alloc(mir::Local {
                name: param.name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            params.push(mir::Param {
                name: param.name.clone(),
                ty,
                local,
            });
            args.push(mir::Expr::Local(local));
        }
        let return_ty = types.lower(signature.return_ty, &mut self.enums, &mut self.shell);
        let call = mir::Expr::Call(mir::Call {
            target: mir::CallTarget {
                kind: mir::CallKind::Direct,
                callee: mir::Callee::User(impl_fn),
            },
            args,
        });
        let kind = if return_ty == mir::Type::Unit {
            mir::StatementKind::Expr(call)
        } else {
            mir::StatementKind::Return { value: Some(call) }
        };
        let iface_name = self.interfaces[iface].name.clone();
        // An interface overloading the method name needs the parameter
        // encoding to keep the thunk symbols distinct.
        let overloaded = module.interfaces[hir_iface]
            .methods
            .iter()
            .filter(|sig| sig.name == signature.name)
            .count()
            > 1;
        let name = if overloaded {
            format!("thunk.{encoded}.{iface_name}.{}.{encoding}", signature.name)
        } else {
            format!("thunk.{encoded}.{iface_name}.{}", signature.name)
        };
        let id = self.functions.alloc(mir::Function {
            symbol: format!("scoop.{name}"),
            name,
            params,
            return_ty,
            body: mir::Body {
                locals,
                statements: vec![mir::Statement {
                    kind,
                    span: signature.span,
                }],
            },
        });
        self.top_level.push(id);
        id
    }

    /// The value type's own method with the signature `key` (the
    /// implementation a boxed thunk tail-calls). HIR guarantees it
    /// exists: the value type was boxed to an interface that declares
    /// the method.
    fn value_method(
        &mut self,
        module: &hir::Module,
        payload: &mir::Type,
        key: &str,
    ) -> mir::FunctionId {
        for (fn_id, function) in module.functions.iter() {
            let Some(ty) = function.method_of else {
                continue;
            };
            let matches = match (&module.types[ty], payload) {
                (hir::Type::Struct(hir_id), mir::Type::Struct(mir_id)) => {
                    self.struct_map[hir_id] == *mir_id
                }
                (hir::Type::Enum(hir_id, _), mir::Type::Enum(mir_id, _)) => {
                    // Enum instances are named `<name>` or
                    // `<name>$<encoded args>` (see `EnumRegistry`).
                    let hir_name = &module.enums[*hir_id].name;
                    let mir_name = &self.enums.defs[*mir_id].name;
                    mir_name == hir_name || mir_name.starts_with(&format!("{hir_name}$"))
                }
                _ => false,
            };
            if matches
                && function.type_params.is_empty()
                && self.fn_signature_key(module, function) == key
            {
                return self.function_map[&fn_id];
            }
        }
        unreachable!("hir-lower guarantees `{key}` is implemented by the boxed value type")
    }
}

/// A function's MIR name: hir-lower already qualifies member
/// functions (`Owner.method`), so the name is used as-is and
/// same-named methods of different types never share a mangled
/// symbol. Same-named *overloads* share this name; their symbols are
/// distinguished by the parameter encoding (`Lowerer::declare_symbol`).
fn fn_name(function: &hir::Function) -> String {
    function.name.clone()
}

/// The names shared by more than one plainly-mangled function (M7
/// overloads), over the whole module including scoop.core. Only
/// functions that get a plain `scoop.<name>` symbol count: non-generic
/// `User` functions (free functions, class members, interface method
/// shells). Intrinsics have no MIR symbol; generic functions only
/// exist as `$`-mangled instances, which cannot collide with the
/// overload encoding (`.`).
fn overloaded_names(module: &hir::Module) -> HashSet<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for (_, function) in module.functions.iter() {
        if !matches!(function.kind, hir::FunctionKind::User(_)) || !function.type_params.is_empty()
        {
            continue;
        }
        *counts.entry(fn_name(function)).or_default() += 1;
    }
    counts
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(name, _)| name)
        .collect()
}

/// A method's short name: hir-lower qualifies member functions as
/// `Owner.method`; slot lookup, override matching and implementation
/// resolution all use the short name (M6 has no overloading).
fn short_name(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

/// Whether a function is an abstract class method as hir-lower
/// materializes it: a bodiless `User` function (parameters only, no
/// statements) with a non-`Unit` return type in a class. A concrete
/// non-`Unit` function without statements is a HIR error, so this
/// shape can only be an abstract method. (`Unit`-returning abstract
/// methods lower as ordinary empty functions: they are unreachable in
/// valid programs and behaviorally identical to an empty body.)
fn is_abstract_bodiless(module: &hir::Module, function: &hir::Function, body: &hir::Body) -> bool {
    body.statements.is_empty()
        && !matches!(module.types[function.return_ty], hir::Type::Unit)
        && matches!(function.method_of, Some(ty) if matches!(module.types[ty], hir::Type::Class(_)))
}

/// The class a function is a method of, if any.
fn method_class(module: &hir::Module, function: &hir::Function) -> Option<hir::ClassId> {
    match function.method_of {
        Some(ty) => match module.types[ty] {
            hir::Type::Class(id) => Some(id),
            _ => None,
        },
        None => None,
    }
}

/// Class ids (HIR) ordered base-before-derived (single inheritance:
/// depth in the base chain; ties keep declaration order).
fn topo_class_order(module: &hir::Module) -> Vec<hir::ClassId> {
    fn depth(module: &hir::Module, id: hir::ClassId) -> usize {
        match module.classes[id].base_class {
            Some((base, _)) => depth(module, base) + 1,
            None => 0,
        }
    }
    let mut order: Vec<hir::ClassId> = module.classes.iter().map(|(id, _)| id).collect();
    order.sort_by_key(|&id| depth(module, id));
    order
}

/// The values initializing `hir_id`'s flattened fields (base prefix
/// first), given `own` — the values for the class's own constructor
/// properties. The base delegation arguments are lowered in the
/// current (`lowerer`) context and the expansion recurses down the
/// base chain (see `Lowerer::lower_ctor`).
fn flattened_ctor_args(
    lowerer: &mut BodyLowerer,
    module: &hir::Module,
    hir_id: hir::ClassId,
    own: Vec<mir::Expr>,
) -> Vec<mir::Expr> {
    let mut out = match &module.classes[hir_id].base_class {
        Some((base, delegation)) => {
            let base_own: Vec<mir::Expr> = delegation
                .iter()
                .map(|expr| lowerer.lower_expr(expr))
                .collect();
            flattened_ctor_args(lowerer, module, *base, base_own)
        }
        None => Vec::new(),
    };
    out.extend(own);
    out
}

/// `mir::TableSlot` is not `Clone`; both payloads are `Copy`.
fn clone_slots(slots: &[mir::TableSlot]) -> Vec<mir::TableSlot> {
    slots
        .iter()
        .map(|slot| match slot {
            mir::TableSlot::Function(id) => mir::TableSlot::Function(*id),
            mir::TableSlot::Runtime(function) => mir::TableSlot::Runtime(*function),
        })
        .collect()
}

/// `mir::Field` is not `Clone`.
fn clone_fields(fields: &[mir::Field]) -> Vec<mir::Field> {
    fields
        .iter()
        .map(|field| mir::Field {
            name: field.name.clone(),
            ty: field.ty.clone(),
        })
        .collect()
}

/// `mir::mangle_instance` / `mir::encode_type` take `&mir::Module`
/// but only ever read struct / enum / class / interface names; this
/// shell provides exactly those. Its arenas share the real arenas'
/// allocation order, so ids align.
fn mangling_shell(
    structs: &Arena<mir::StructDef>,
    classes: &Arena<mir::ClassDef>,
    interfaces: &Arena<mir::InterfaceDef>,
) -> mir::Module {
    let mut shell_structs = Arena::new();
    for (_, def) in structs.iter() {
        shell_structs.alloc(mir::StructDef {
            name: def.name.clone(),
            fields: Vec::new(),
        });
    }
    let mut shell_classes = Arena::new();
    for (_, def) in classes.iter() {
        shell_classes.alloc(mir::ClassDef {
            modifier: mir::ClassModifier::Final,
            name: def.name.clone(),
            fields: Vec::new(),
            base_class: None,
            interfaces: Vec::new(),
            vtable: Vec::new(),
            itables: Vec::new(),
        });
    }
    let mut shell_interfaces = Arena::new();
    for (_, def) in interfaces.iter() {
        shell_interfaces.alloc(mir::InterfaceDef {
            name: def.name.clone(),
            methods: Vec::new(),
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
        classes: shell_classes,
        interfaces: shell_interfaces,
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
        hir::Type::Array(element) | hir::Type::MutableArray(element) => {
            is_concrete(module, *element)
        }
        hir::Type::Tuple(elements) => elements.iter().all(|&e| is_concrete(module, e)),
        hir::Type::Enum(_, args) => args.iter().all(|&arg| is_concrete(module, arg)),
        _ => true,
    }
}

/// Shared type-lowering context: the HIR type arena, the struct /
/// class / interface maps, and the active substitution (`Param(i)`
/// resolves through `subst`, the concrete type arguments of the
/// instance / enum being lowered; non-generic bodies never contain
/// it).
#[derive(Clone, Copy)]
struct Types<'a> {
    module: &'a hir::Module,
    struct_map: &'a HashMap<hir::StructId, mir::StructId>,
    class_map: &'a HashMap<hir::ClassId, mir::ClassId>,
    interface_map: &'a HashMap<hir::InterfaceId, mir::InterfaceId>,
    subst: Option<&'a [mir::Type]>,
}

impl Types<'_> {
    /// Map a HIR type onto its MIR type. Aggregate shapes are
    /// preserved: structs keep their (remapped) id, tuples their mapped
    /// element types; enum types instantiate their definition on
    /// demand. Reference types map onto their (remapped) ids.
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
            hir::Type::Class(id) => mir::Type::Class(self.class_map[id]),
            hir::Type::Interface(id) => mir::Type::Interface(self.interface_map[id]),
            hir::Type::Any => mir::Type::Any,
            hir::Type::Array(element) => {
                mir::Type::Array(Box::new(self.lower(*element, enums, shell)))
            }
            hir::Type::MutableArray(element) => {
                mir::Type::MutableArray(Box::new(self.lower(*element, enums, shell)))
            }
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
    /// MIR enum -> the HIR enum it instantiates (boxed value types
    /// read the declared interfaces from the declaration).
    hir_ids: HashMap<mir::EnumId, hir::EnumId>,
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
        self.hir_ids.insert(id, hir_id);
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

/// Boxed value types (DESIGN 2.3): one `mir::ClassDef` per boxed
/// value type (`box$<encoded>`), deduplicated by the encoded payload
/// name. The vtable / itables are filled by `finalize_boxed` once
/// every body has been lowered (all `Box` / `is` / `as` sites seen).
#[derive(Default)]
struct BoxedRegistry {
    /// `box$<encoded payload>` -> boxed class.
    by_name: HashMap<String, mir::ClassId>,
    /// Boxed classes in creation order.
    order: Vec<mir::ClassId>,
}

impl BoxedRegistry {
    fn get_or_create(
        &mut self,
        classes: &mut Arena<mir::ClassDef>,
        shell: &mut mir::Module,
        payload: &mir::Type,
    ) -> mir::ClassId {
        let name = format!("box${}", mir::encode_type(shell, payload));
        if let Some(&id) = self.by_name.get(&name) {
            return id;
        }
        let id = classes.alloc(mir::ClassDef {
            modifier: mir::ClassModifier::Final,
            name: name.clone(),
            // The object layout is the header plus the inline payload.
            fields: vec![mir::Field {
                name: "value".to_string(),
                ty: payload.clone(),
            }],
            base_class: None,
            interfaces: Vec::new(),
            // Filled by `finalize_boxed`.
            vtable: Vec::new(),
            itables: Vec::new(),
        });
        // Keep the mangling shell's class arena in sync (same ids).
        shell.classes.alloc(mir::ClassDef {
            modifier: mir::ClassModifier::Final,
            name: name.clone(),
            fields: Vec::new(),
            base_class: None,
            interfaces: Vec::new(),
            vtable: Vec::new(),
            itables: Vec::new(),
        });
        self.by_name.insert(name, id);
        self.order.push(id);
        id
    }
}

/// Whether values of the type are boxed when they reach `Any` / an
/// interface (reference types — String, arrays — are not).
fn is_boxable(ty: &mir::Type) -> bool {
    matches!(
        ty,
        mir::Type::Struct(_)
            | mir::Type::Enum(..)
            | mir::Type::Tuple(_)
            | mir::Type::Int
            | mir::Type::Boolean
            | mir::Type::Unit
    )
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
        let name = fn_name(function);
        let symbol = mir::mangle_instance(shell, &name, &type_args);
        if let Some(&id) = self.by_symbol.get(&symbol) {
            return id;
        }
        let id = functions.alloc(mir::Function {
            name,
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

/// Per-function-body lowering state.
struct BodyLowerer<'a> {
    module: &'a hir::Module,
    struct_map: &'a HashMap<hir::StructId, mir::StructId>,
    class_map: &'a HashMap<hir::ClassId, mir::ClassId>,
    interface_map: &'a HashMap<hir::InterfaceId, mir::InterfaceId>,
    /// MIR struct arena (field types for the equality expansion).
    structs: &'a Arena<mir::StructDef>,
    /// Method signature key -> vtable slot per class
    /// (`compute_dispatch`).
    method_slots: &'a HashMap<mir::ClassId, HashMap<String, u32>>,
    function_map: &'a HashMap<hir::FunctionId, mir::FunctionId>,
    /// HIR class -> its constructor function (`ClassInit` calls).
    ctors: &'a HashMap<hir::ClassId, mir::FunctionId>,
    strings: &'a mut Arena<mir::StringConst>,
    functions: &'a mut Arena<mir::Function>,
    top_level: &'a mut Vec<mir::FunctionId>,
    instances: &'a mut InstanceRegistry,
    /// Instantiated enum definitions, filled on creation (variant
    /// field types feed pattern lowering and the equality expansion).
    enums: &'a mut EnumRegistry,
    /// Boxed value types discovered in this body (`Box` / `is` / `as`).
    boxed: &'a mut BoxedRegistry,
    /// MIR class arena (boxed value types are appended here).
    classes: &'a mut Arena<mir::ClassDef>,
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
        let statements = if is_abstract_bodiless(self.module, function, body) {
            // An abstract method (hir-lower materializes it bodiless):
            // every override replaces its vtable slot and the class
            // cannot be instantiated, so the slot is never reached;
            // the emitted function traps like a pure-virtual stub.
            let message =
                self.trap_message(format!("call to abstract method `{}`", fn_name(function)));
            vec![mir::Statement {
                kind: mir::StatementKind::Expr(mir::Expr::Call(mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::Runtime(mir::RuntimeFn::Trap),
                    },
                    args: vec![mir::Expr::StringConst(message)],
                })),
                span: function.span,
            }]
        } else {
            self.lower_statements(&body.statements)
        };
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
            class_map: self.class_map,
            interface_map: self.interface_map,
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
            hir::StatementKind::Assign { target, value } => {
                let kind = match target {
                    hir::AssignTarget::Local(local) => {
                        let local = self.local_map[local];
                        let value = self.lower_expr(value);
                        mir::StatementKind::Assign { local, value }
                    }
                    // `m[i] = v` (only `MutableArray`, checked at HIR).
                    // M8: the bounds check moved here from codegen —
                    // the array and the index are evaluated once into
                    // hidden locals and checked before the store; the
                    // value expression stays inside the `ArraySet`
                    // node and is evaluated after the check.
                    hir::AssignTarget::Index { array, index } => {
                        let array_ty = self.lower_type(array.ty);
                        let array_slot = self.new_hidden("arr", array_ty, false);
                        let index_slot = self.new_hidden("idx", mir::Type::Int, false);
                        let array_value = self.lower_expr(array);
                        self.prelude.push(mir::StatementKind::ValDecl {
                            local: array_slot,
                            init: array_value,
                        });
                        let index_value = self.lower_expr(index);
                        self.prelude.push(mir::StatementKind::ValDecl {
                            local: index_slot,
                            init: index_value,
                        });
                        self.bounds_check(array_slot, index_slot, span);
                        let value = self.lower_expr(value);
                        mir::StatementKind::ArraySet {
                            array: mir::Expr::Local(array_slot),
                            index: mir::Expr::Local(index_slot),
                            value,
                        }
                    }
                    // `obj.field = v` (only `var` properties of
                    // classes, checked at HIR); the index is the
                    // flattened field index.
                    hir::AssignTarget::Field { receiver, field } => {
                        let hir::FieldRef::ClassField { index, .. } = field else {
                            unreachable!("hir-lower only allows assignment to class properties")
                        };
                        let object = self.lower_expr(receiver);
                        let value = self.lower_expr(value);
                        mir::StatementKind::FieldSet {
                            object,
                            index: *index,
                            value,
                        }
                    }
                };
                self.drain_prelude(span, out);
                kind
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
            // `try` / `catch` / `finally` stays structured in MIR
            // (M8, DESIGN 3.3); the control-flow expansion (invoke /
            // landingpad) is LIR's job.
            hir::StatementKind::Try(try_) => mir::StatementKind::Try(self.lower_try(try_)),
            hir::StatementKind::Throw(expr) => {
                let value = self.lower_expr(expr);
                self.drain_prelude(span, out);
                mir::StatementKind::Throw(value)
            }
        };
        out.push(mir::Statement { kind, span });
    }

    /// `try` translates one-to-one: body, ordered catches (the catch
    /// type is resolved to the concrete MIR type), and the optional
    /// finally body.
    fn lower_try(&mut self, try_: &hir::Try) -> mir::Try {
        let body = self.lower_statements(&try_.body);
        let catches = try_
            .catches
            .iter()
            .map(|catch| mir::CatchClause {
                local: self.local_map[&catch.local],
                ty: Box::new(self.lower_type(catch.ty)),
                body: self.lower_statements(&catch.body),
                span: catch.span,
            })
            .collect();
        let finally_body = try_
            .finally_body
            .as_ref()
            .map(|body| self.lower_statements(body));
        mir::Try {
            body,
            catches,
            finally_body,
        }
    }

    /// `throw <Name>()`: one of core's built-in exception classes
    /// (throwable.scoop). They are ordinary class declarations, so
    /// construction is a plain call to the generated constructor
    /// function — the same path a use-site `ClassInit` takes (M6).
    /// The class is resolved by name; hir-lower validates that core
    /// declares `Throwable`, so a missing class is a core
    /// configuration error, not a user error.
    fn throw_builtin(&mut self, name: &str, span: Span) -> mir::Statement {
        let class = self
            .module
            .classes
            .iter()
            .find(|(_, decl)| decl.name == name)
            .map(|(id, _)| id)
            .unwrap_or_else(|| panic!("core must declare `{name}` (throwable.scoop)"));
        let ctor = self.ctors[&class];
        mir::Statement {
            kind: mir::StatementKind::Throw(mir::Expr::Call(mir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee: mir::Callee::User(ctor),
                },
                args: Vec::new(),
            })),
            span,
        }
    }

    /// The M8 array bounds check (DESIGN section 1), shared by
    /// `ArrayGet` and `ArraySet`:
    /// `if (index < 0 || index >= array.size) throw IndexOutOfBoundsException()`.
    /// `||` stays a single operator; LIR expands the short-circuit.
    fn bounds_check(&mut self, array: mir::LocalId, index: mir::LocalId, span: Span) {
        let out_of_bounds = mir::Expr::Binary {
            op: mir::BinOp::Or,
            lhs: Box::new(mir::Expr::Binary {
                op: mir::BinOp::IntLt,
                lhs: Box::new(mir::Expr::Local(index)),
                rhs: Box::new(mir::Expr::IntLiteral(0)),
            }),
            rhs: Box::new(mir::Expr::Binary {
                op: mir::BinOp::IntGe,
                lhs: Box::new(mir::Expr::Local(index)),
                rhs: Box::new(mir::Expr::ArrayLen(Box::new(mir::Expr::Local(array)))),
            }),
        };
        let throw = self.throw_builtin("IndexOutOfBoundsException", span);
        self.prelude.push(mir::StatementKind::If {
            cond: out_of_bounds,
            then_body: vec![throw],
            else_body: None,
        });
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
            // Class construction calls the class's constructor
            // function (`scoop.ctor.<Class>`); the raw allocation and
            // field initialization live inside it (see `lower_ctor`).
            hir::ExprKind::ClassInit { class_id, args } => {
                let ctor = self.ctors[class_id];
                mir::Expr::Call(mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::User(ctor),
                    },
                    args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                })
            }
            hir::ExprKind::VariantConstruct { variant, args, .. } => mir::Expr::VariantConstruct {
                ty: self.lower_type(expr.ty),
                variant: *variant,
                fields: args.iter().map(|arg| self.lower_expr(arg)).collect(),
            },
            hir::ExprKind::Local(local) => mir::Expr::Local(self.local_map[local]),
            // The array nodes translate one-to-one (M5); the literal's
            // kind (Array vs MutableArray) is fixed by the producing
            // context — `lower_type(expr.ty)` records it where needed.
            hir::ExprKind::ArrayLiteral(elements) => {
                mir::Expr::ArrayLiteral(elements.iter().map(|e| self.lower_expr(e)).collect())
            }
            // Subscript read. M8: the bounds check moved here from
            // codegen — the array and the index are evaluated once
            // into hidden locals, then `IndexOutOfBoundsException`
            // throws when the index is out of range (the prelude
            // mechanism `!!` uses).
            hir::ExprKind::Index { receiver, index } => {
                let array_ty = self.lower_type(receiver.ty);
                let array_slot = self.new_hidden("arr", array_ty, false);
                let index_slot = self.new_hidden("idx", mir::Type::Int, false);
                let array = self.lower_expr(receiver);
                self.prelude.push(mir::StatementKind::ValDecl {
                    local: array_slot,
                    init: array,
                });
                let index = self.lower_expr(index);
                self.prelude.push(mir::StatementKind::ValDecl {
                    local: index_slot,
                    init: index,
                });
                self.bounds_check(array_slot, index_slot, expr.span);
                mir::Expr::ArrayGet {
                    array: Box::new(mir::Expr::Local(array_slot)),
                    index: Box::new(mir::Expr::Local(index_slot)),
                }
            }
            hir::ExprKind::ArrayLen(operand) => {
                mir::Expr::ArrayLen(Box::new(self.lower_expr(operand)))
            }
            hir::ExprKind::ArrayClone(operand) => {
                mir::Expr::ArrayClone(Box::new(self.lower_expr(operand)))
            }
            hir::ExprKind::FieldAccess { receiver, field } => {
                // Struct fields, tuple elements and class constructor
                // properties are all 0-based here (the class index
                // follows the flattened base-prefix layout; LIR turns
                // it into a heap object load).
                let index = match field {
                    hir::FieldRef::StructField { index, .. }
                    | hir::FieldRef::ClassField { index, .. }
                    | hir::FieldRef::TupleIndex(index) => *index,
                };
                mir::Expr::FieldAccess {
                    receiver: Box::new(self.lower_expr(receiver)),
                    index,
                }
            }
            hir::ExprKind::MethodCall {
                receiver,
                function,
                args,
            } => self.lower_method_call(receiver, *function, args),
            // `Box` / `Unbox` / `is` stay dedicated MIR nodes; LIR
            // lowers them (the runtime box call, the payload load,
            // the `scoop_rt_is_instance` call). Boxing registers the
            // boxed value type (and the target interface) on the way.
            hir::ExprKind::Box(operand) => {
                let payload = self.lower_type(operand.ty);
                self.register_boxed(&payload, Some(expr.ty));
                mir::Expr::Box(Box::new(self.lower_expr(operand)))
            }
            // Smart casts unbox inline wherever the narrowed local is
            // read (e.g. as a field-access receiver). LIR reconstructs
            // expression types structurally, and an `Any` operand does
            // not determine the payload type, so every unbox is bound
            // to a typed hidden local (the same prelude mechanism `!!`
            // uses).
            hir::ExprKind::Unbox(operand) => {
                let ty = self.lower_type(expr.ty);
                let operand = self.lower_expr(operand);
                let slot = self.new_hidden("ub", ty, false);
                self.prelude.push(mir::StatementKind::ValDecl {
                    local: slot,
                    init: mir::Expr::Unbox(Box::new(operand)),
                });
                mir::Expr::Local(slot)
            }
            hir::ExprKind::IsInstance { operand, check_ty } => {
                let check_ty = self.lower_type(*check_ty);
                self.register_check(&check_ty);
                mir::Expr::IsInstance {
                    operand: Box::new(self.lower_expr(operand)),
                    check_ty: Box::new(check_ty),
                }
            }
            hir::ExprKind::Cast { operand, optional } => {
                self.lower_cast(operand, *optional, expr.ty, expr.span)
            }
            hir::ExprKind::Call {
                function,
                type_args,
                args,
            } => self.lower_call(*function, type_args, args),
            hir::ExprKind::Binary { op, lhs, rhs } => self.lower_binary(*op, lhs, rhs, expr.span),
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
    /// `if (tag == Some) { val $uw = <field 0> } else { throw UnwrapException() }`.
    /// The if/else is queued in `prelude` — it must precede the
    /// statement this expression belongs to — and the expression
    /// itself becomes the result local. The exception is an ordinary
    /// constructor call (`throw_builtin`, M8).
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
        let throw = self.throw_builtin("UnwrapException", span);
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
            else_body: Some(vec![throw]),
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
            // `@Intrinsic` primitive functions (scoop.core, M7 DESIGN
            // section 2): the intrinsic name maps directly onto the
            // runtime function — `print` / `println` themselves are
            // ordinary overloaded core functions and take the `User`
            // path below. hir-lower rejects unknown intrinsic names.
            hir::FunctionKind::Intrinsic(name) => {
                let function = match name.as_str() {
                    "rt_write" => mir::RuntimeFn::Write,
                    "rt_int_to_string" => mir::RuntimeFn::IntToString,
                    "rt_bool_to_string" => mir::RuntimeFn::BoolToString,
                    _ => unreachable!("hir-lower rejects unknown intrinsics"),
                };
                mir::Callee::Runtime(function)
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

    /// A resolved method call (impl spec 2.9): the receiver becomes
    /// argument 0 (`this`), and the call kind is annotated from the
    /// receiver's static type — class receiver → `Virtual` (the M6
    /// simplification: class methods always dispatch through the
    /// vtable), interface receiver → `Interface` (the slot is the
    /// method signature's index in the interface declaration), value
    /// type → `Direct`. A method without a vtable slot (generic
    /// methods never enter the vtable) stays `Direct`. The slot is
    /// located by the callee's signature (`signature_key`), so
    /// overloads dispatch to their own slot and overrides hit the
    /// replaced base slot.
    fn lower_method_call(
        &mut self,
        receiver: &hir::Expr,
        function: hir::FunctionId,
        args: &[hir::Expr],
    ) -> mir::Expr {
        let module = self.module;
        let f = &module.functions[function];
        assert!(
            f.type_params.is_empty(),
            "HIR method calls carry no type arguments; hir-lower M6 rejects generic method calls"
        );
        let callee = mir::Callee::User(self.function_map[&function]);
        // The receiver's static type decides the dispatch kind.
        enum Receiver {
            Class(hir::ClassId),
            Interface(hir::InterfaceId),
            Any,
            Value,
        }
        let receiver_kind = match &module.types[receiver.ty] {
            hir::Type::Class(class) => Receiver::Class(*class),
            hir::Type::Interface(iface) => Receiver::Interface(*iface),
            hir::Type::Any => Receiver::Any,
            _ => Receiver::Value,
        };
        let key = self.signature_key(f);
        let kind = match receiver_kind {
            Receiver::Class(class) => match self.method_slots[&self.class_map[&class]].get(&key) {
                Some(&slot) => mir::CallKind::Virtual { slot },
                None => mir::CallKind::Direct,
            },
            Receiver::Interface(iface) => {
                let interface = self.interface_map[&iface];
                let mut slot = None;
                for (index, sig) in module.interfaces[iface].methods.iter().enumerate() {
                    if self.sig_key(sig) == key {
                        slot = Some(index as u32);
                        break;
                    }
                }
                mir::CallKind::Interface {
                    interface,
                    slot: slot.expect("hir-lower resolves interface calls to interface methods"),
                }
            }
            // The `Any` defaults dispatch through the fixed vtable
            // prefix; anything else hir-lower resolves on `Any` is a
            // plain direct call.
            Receiver::Any => match short_name(&f.name) {
                "equals" => mir::CallKind::Virtual { slot: 0 },
                "hashCode" => mir::CallKind::Virtual { slot: 1 },
                "toString" => mir::CallKind::Virtual { slot: 2 },
                _ => mir::CallKind::Direct,
            },
            Receiver::Value => mir::CallKind::Direct,
        };
        let mut call_args = Vec::with_capacity(args.len() + 1);
        call_args.push(self.lower_expr(receiver));
        call_args.extend(args.iter().map(|arg| self.lower_expr(arg)));
        mir::Expr::Call(mir::Call {
            target: mir::CallTarget { kind, callee },
            args: call_args,
        })
    }

    /// The callee's dispatch signature key (`name(<param encoding>)`,
    /// receiver excluded) — must agree with
    /// `Lowerer::fn_signature_key`, which keys the vtable slots.
    fn signature_key(&mut self, function: &hir::Function) -> String {
        let skip = usize::from(function.method_of.is_some());
        self.key_parts(short_name(&function.name), &function.params[skip..])
    }

    /// The signature key of an interface method signature.
    fn sig_key(&mut self, sig: &hir::MethodSig) -> String {
        self.key_parts(&sig.name, &sig.params)
    }

    fn key_parts(&mut self, name: &str, params: &[hir::Param]) -> String {
        let params: Vec<mir::Type> = params
            .iter()
            .map(|param| self.lower_type(param.ty))
            .collect();
        format!("{name}({})", mir::encode_params(self.shell, &params))
    }

    /// Register the boxed value type a `Box` produces. The boxed
    /// itables cover the value type's *declared* interfaces (spec
    /// 4.4.3) no matter what the value is boxed to; the box target,
    /// when an interface, is covered too (hir-lower guarantees the
    /// value type implements it, so it is normally already in the
    /// declared set). The itable slots — the adjust thunks — are
    /// generated by `finalize_boxed`.
    fn register_boxed(&mut self, payload: &mir::Type, target: Option<hir::TypeId>) {
        if !is_boxable(payload) {
            return;
        }
        let class_id = self.boxed.get_or_create(self.classes, self.shell, payload);
        let mut covered: Vec<mir::InterfaceId> = match payload {
            mir::Type::Struct(mir_id) => {
                let hir_id = self.hir_struct(*mir_id);
                self.module.structs[hir_id]
                    .interfaces
                    .iter()
                    .map(|iface| self.interface_map[iface])
                    .collect()
            }
            mir::Type::Enum(mir_id, _) => {
                let hir_id = self.enums.hir_ids[mir_id];
                self.module.enums[hir_id]
                    .interfaces
                    .iter()
                    .map(|iface| self.interface_map[iface])
                    .collect()
            }
            // Tuples and primitives implement no interfaces.
            _ => Vec::new(),
        };
        if let Some(target) = target {
            if let hir::Type::Interface(iface) = self.module.types[target] {
                covered.push(self.interface_map[&iface]);
            }
        }
        for iface in covered {
            let interfaces = &mut self.classes[class_id].interfaces;
            if !interfaces.contains(&iface) {
                interfaces.push(iface);
            }
        }
    }

    /// The HIR id behind a MIR struct (the arenas are transposed 1:1).
    fn hir_struct(&self, mir_id: mir::StructId) -> hir::StructId {
        self.struct_map
            .iter()
            .find(|(_, mir)| **mir == mir_id)
            .map(|(&hir, _)| hir)
            .expect("every MIR struct comes from a HIR struct")
    }

    /// Register the boxed value type an `is` / `as` check needs (the
    /// runtime compares against the boxed type's TypeDescriptor).
    fn register_check(&mut self, check_ty: &mir::Type) {
        if is_boxable(check_ty) {
            let check_ty = check_ty.clone();
            self.register_boxed(&check_ty, None);
        }
    }

    /// `as` / `as?` (DESIGN 2.3): the operand is evaluated once into
    /// a hidden local. `as` throws `ClassCastException` when the
    /// runtime check fails (M8); `as?` wraps the result
    /// in `Some` / `None` through core's `Option` — the same prelude
    /// mechanism `!!` uses. A target of `Any` is statically true and
    /// needs no check. Class / interface targets stay the same
    /// reference; value targets come out of the box (`Unbox`).
    fn lower_cast(
        &mut self,
        operand: &hir::Expr,
        optional: bool,
        expr_ty: hir::TypeId,
        span: Span,
    ) -> mir::Expr {
        let target_hir = if optional {
            let hir::Type::Enum(option, args) = &self.module.types[expr_ty] else {
                unreachable!("an `as?` result is an Option<T>")
            };
            assert_eq!(
                *option, self.module.option_enum,
                "an `as?` result is core's Option<T>"
            );
            args[0]
        } else {
            expr_ty
        };
        let target = self.lower_type(target_hir);
        self.register_check(&target);
        let operand_ty = self.lower_type(operand.ty);
        let value = self.lower_expr(operand);
        let slot = self.new_hidden("cast", operand_ty, false);
        self.prelude.push(mir::StatementKind::ValDecl {
            local: slot,
            init: value,
        });
        let cond = match &target {
            mir::Type::Any => mir::Expr::BoolLiteral(true),
            _ => mir::Expr::IsInstance {
                operand: Box::new(mir::Expr::Local(slot)),
                check_ty: Box::new(target.clone()),
            },
        };
        let unboxed = match &target {
            mir::Type::Class(_) | mir::Type::Interface(_) | mir::Type::Any => {
                mir::Expr::Local(slot)
            }
            // Only `as?` unwraps here: hir-lower wraps a value-typed
            // `as` in a hir-level `Unbox(Cast)` node, so the payload
            // extraction for `as` happens when that outer `Unbox` is
            // lowered — adding another one here would double-unwrap.
            _ => mir::Expr::Unbox(Box::new(mir::Expr::Local(slot))),
        };
        if !optional {
            let throw = self.throw_builtin("ClassCastException", span);
            self.prelude.push(mir::StatementKind::If {
                cond: mir::Expr::Unary {
                    op: mir::UnOp::BoolNot,
                    operand: Box::new(cond),
                },
                then_body: vec![throw],
                else_body: None,
            });
            // The hir-level `Unbox` around this `Cast` (value targets
            // only) performs the payload extraction; class / interface
            // targets just use the reference.
            return mir::Expr::Local(slot);
        }
        let option_ty = self.lower_type(expr_ty);
        let (some, none) = self.option_variants;
        let result = self.new_hidden("cast", option_ty.clone(), true);
        let some_value = mir::Expr::VariantConstruct {
            ty: option_ty.clone(),
            variant: some,
            fields: vec![unboxed],
        };
        let none_value = mir::Expr::VariantConstruct {
            ty: option_ty,
            variant: none,
            fields: Vec::new(),
        };
        self.prelude.push(mir::StatementKind::If {
            cond,
            then_body: vec![mir::Statement {
                kind: mir::StatementKind::Assign {
                    local: result,
                    value: some_value,
                },
                span,
            }],
            else_body: Some(vec![mir::Statement {
                kind: mir::StatementKind::Assign {
                    local: result,
                    value: none_value,
                },
                span,
            }]),
        });
        mir::Expr::Local(result)
    }

    /// A trap message string constant (`scoop.str.N`, numbered in
    /// order of appearance like the literal constants).
    fn trap_message(&mut self, message: String) -> mir::StringConstId {
        let symbol = format!("scoop.str.{}", self.strings.len());
        self.strings.alloc(mir::StringConst {
            value: message,
            symbol,
        })
    }

    fn lower_binary(
        &mut self,
        op: hir::BinOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
        span: Span,
    ) -> mir::Expr {
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
            // M8 (DESIGN section 1): integer division checks the
            // divisor — a zero divisor throws `ArithmeticException`
            // instead of hitting LLVM `sdiv` UB. Both operands are
            // evaluated once into hidden locals (left to right), so
            // the check and the division share one evaluation.
            hir::BinOp::Div => {
                let lhs_slot = self.new_hidden("div", mir::Type::Int, false);
                let rhs_slot = self.new_hidden("div", mir::Type::Int, false);
                let lhs = self.lower_expr(lhs);
                self.prelude.push(mir::StatementKind::ValDecl {
                    local: lhs_slot,
                    init: lhs,
                });
                let rhs = self.lower_expr(rhs);
                self.prelude.push(mir::StatementKind::ValDecl {
                    local: rhs_slot,
                    init: rhs,
                });
                let throw = self.throw_builtin("ArithmeticException", span);
                self.prelude.push(mir::StatementKind::If {
                    cond: mir::Expr::Binary {
                        op: mir::BinOp::IntEq,
                        lhs: Box::new(mir::Expr::Local(rhs_slot)),
                        rhs: Box::new(mir::Expr::IntLiteral(0)),
                    },
                    then_body: vec![throw],
                    else_body: None,
                });
                mir::Expr::Binary {
                    op: IntDiv,
                    lhs: Box::new(mir::Expr::Local(lhs_slot)),
                    rhs: Box::new(mir::Expr::Local(rhs_slot)),
                }
            }
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
            // `===` / `!==`: reference identity — the primitive
            // comparison on the two pointers.
            hir::BinOp::RefEq => self.primitive(IntEq, lhs, rhs),
            hir::BinOp::RefNe => self.primitive(IntNe, lhs, rhs),
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
    ///   `true`, `() != ()` always `false`;
    /// - arrays: none — M5 has no array equality semantics (DESIGN 6),
    ///   and hir-lower rejects `==` / `!=` on array types, so arrays
    ///   never reach this expansion (neither as operands nor nested
    ///   inside compared aggregates).
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
            // References compare by identity: the `Any` default
            // `equals` is reference equality and M6 has no
            // user-defined `equals` (milestone6 DESIGN 5.1). The
            // operands are pointers, so the primitive integer
            // comparison is a pointer comparison here.
            mir::Type::Class(_) | mir::Type::Interface(_) | mir::Type::Any => {
                let op = if negate {
                    mir::BinOp::IntNe
                } else {
                    mir::BinOp::IntEq
                };
                self.comparison(op, lhs, rhs, path)
            }
            // M5 defines no array equality semantics (DESIGN 6) and
            // hir-lower rejects `==` / `!=` on array types, so no array
            // type can reach the equality expansion.
            mir::Type::Array(_) | mir::Type::MutableArray(_) => {
                unreachable!("hir-lower rejects equality on array types")
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
    /// core's three intrinsic output primitives and the ordinary
    /// `print` / `println` overloads (M7), plus core's `Option` enum
    /// allocated first.
    struct Harness {
        types: Arena<hir::Type>,
        functions: Arena<hir::Function>,
        structs: Arena<hir::StructDecl>,
        enums: Arena<hir::EnumDecl>,
        classes: Arena<hir::ClassDecl>,
        interfaces: Arena<hir::InterfaceDecl>,
        top_level: Vec<hir::FunctionId>,
        unit: hir::TypeId,
        int: hir::TypeId,
        boolean: hir::TypeId,
        string: hir::TypeId,
        option_enum: hir::EnumId,
        write: hir::FunctionId,
        int_to_string: hir::FunctionId,
        bool_to_string: hir::FunctionId,
        /// core's `print` / `println` overloads (ordinary functions,
        /// M7), created on first use.
        print_string: Option<hir::FunctionId>,
        print_int: Option<hir::FunctionId>,
        print_boolean: Option<hir::FunctionId>,
        println_string: Option<hir::FunctionId>,
        println_int: Option<hir::FunctionId>,
        println_boolean: Option<hir::FunctionId>,
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
            // scoop.core's intrinsic output primitives (M7 DESIGN
            // section 2): `@Intrinsic("rt_write") fun write(...)`,
            // `@Intrinsic("rt_int_to_string") fun intToString(...)`,
            // `@Intrinsic("rt_bool_to_string") fun boolToString(...)`.
            let write = functions.alloc(hir::Function {
                name: "write".to_string(),
                type_params: Vec::new(),
                params: Vec::new(),
                return_ty: unit,
                kind: hir::FunctionKind::Intrinsic("rt_write".to_string()),
                method_of: None,
                span: SPAN,
            });
            let int_to_string = functions.alloc(hir::Function {
                name: "intToString".to_string(),
                type_params: Vec::new(),
                params: Vec::new(),
                return_ty: string,
                kind: hir::FunctionKind::Intrinsic("rt_int_to_string".to_string()),
                method_of: None,
                span: SPAN,
            });
            let bool_to_string = functions.alloc(hir::Function {
                name: "boolToString".to_string(),
                type_params: Vec::new(),
                params: Vec::new(),
                return_ty: string,
                kind: hir::FunctionKind::Intrinsic("rt_bool_to_string".to_string()),
                method_of: None,
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
                interfaces: Vec::new(),
                span: SPAN,
            });
            Harness {
                types,
                functions,
                structs: Arena::new(),
                enums,
                classes: Arena::new(),
                interfaces: Arena::new(),
                top_level: vec![write, int_to_string, bool_to_string],
                unit,
                int,
                boolean,
                string,
                option_enum,
                write,
                int_to_string,
                bool_to_string,
                print_string: None,
                print_int: None,
                print_boolean: None,
                println_string: None,
                println_int: None,
                println_boolean: None,
                instantiations: Vec::new(),
            }
        }

        /// core's `fun print(message: String) = write(message)`,
        /// created on first use (tests that never print keep core's
        /// overloads out of their MIR dumps).
        fn print_string(&mut self) -> hir::FunctionId {
            if let Some(id) = self.print_string {
                return id;
            }
            let (unit, string) = (self.unit, self.string);
            let write = self.write;
            let mut locals = Arena::new();
            let message = locals.alloc(local("message", string));
            let id = self.user_fn_full(
                "print",
                Vec::new(),
                vec![param("message", string, message)],
                unit,
                hir::Body {
                    locals,
                    statements: vec![expr_stmt(call_typed(
                        write,
                        vec![local_ref(message, string)],
                        unit,
                    ))],
                },
            );
            self.print_string = Some(id);
            id
        }

        /// core's `fun print(message: Int) = write(intToString(message))`.
        fn print_int(&mut self) -> hir::FunctionId {
            if let Some(id) = self.print_int {
                return id;
            }
            let (unit, int, string) = (self.unit, self.int, self.string);
            let (write, int_to_string) = (self.write, self.int_to_string);
            let mut locals = Arena::new();
            let message = locals.alloc(local("message", int));
            let id = self.user_fn_full(
                "print",
                Vec::new(),
                vec![param("message", int, message)],
                unit,
                hir::Body {
                    locals,
                    statements: vec![expr_stmt(call_typed(
                        write,
                        vec![call_typed(
                            int_to_string,
                            vec![local_ref(message, int)],
                            string,
                        )],
                        unit,
                    ))],
                },
            );
            self.print_int = Some(id);
            id
        }

        /// core's `fun print(message: Boolean) = write(boolToString(message))`.
        fn print_boolean(&mut self) -> hir::FunctionId {
            if let Some(id) = self.print_boolean {
                return id;
            }
            let (unit, boolean, string) = (self.unit, self.boolean, self.string);
            let (write, bool_to_string) = (self.write, self.bool_to_string);
            let mut locals = Arena::new();
            let message = locals.alloc(local("message", boolean));
            let id = self.user_fn_full(
                "print",
                Vec::new(),
                vec![param("message", boolean, message)],
                unit,
                hir::Body {
                    locals,
                    statements: vec![expr_stmt(call_typed(
                        write,
                        vec![call_typed(
                            bool_to_string,
                            vec![local_ref(message, boolean)],
                            string,
                        )],
                        unit,
                    ))],
                },
            );
            self.print_boolean = Some(id);
            id
        }

        /// core's `fun println(message: String) { write(message); write("\n") }`.
        fn println_string(&mut self) -> hir::FunctionId {
            if let Some(id) = self.println_string {
                return id;
            }
            let (unit, string) = (self.unit, self.string);
            let write = self.write;
            let mut locals = Arena::new();
            let message = locals.alloc(local("message", string));
            let id = self.user_fn_full(
                "println",
                Vec::new(),
                vec![param("message", string, message)],
                unit,
                hir::Body {
                    locals,
                    statements: vec![
                        expr_stmt(call_typed(write, vec![local_ref(message, string)], unit)),
                        expr_stmt(call_typed(
                            write,
                            vec![expr(hir::ExprKind::StringLiteral("\n".to_string()), string)],
                            unit,
                        )),
                    ],
                },
            );
            self.println_string = Some(id);
            id
        }

        /// core's `fun println(message: Int) = println(intToString(message))`.
        fn println_int(&mut self) -> hir::FunctionId {
            if let Some(id) = self.println_int {
                return id;
            }
            let println_string = self.println_string();
            let (unit, int, string) = (self.unit, self.int, self.string);
            let int_to_string = self.int_to_string;
            let mut locals = Arena::new();
            let message = locals.alloc(local("message", int));
            let id = self.user_fn_full(
                "println",
                Vec::new(),
                vec![param("message", int, message)],
                unit,
                hir::Body {
                    locals,
                    statements: vec![expr_stmt(call_typed(
                        println_string,
                        vec![call_typed(
                            int_to_string,
                            vec![local_ref(message, int)],
                            string,
                        )],
                        unit,
                    ))],
                },
            );
            self.println_int = Some(id);
            id
        }

        /// core's `fun println(message: Boolean) = println(boolToString(message))`.
        fn println_boolean(&mut self) -> hir::FunctionId {
            if let Some(id) = self.println_boolean {
                return id;
            }
            let println_string = self.println_string();
            let (unit, boolean, string) = (self.unit, self.boolean, self.string);
            let bool_to_string = self.bool_to_string;
            let mut locals = Arena::new();
            let message = locals.alloc(local("message", boolean));
            let id = self.user_fn_full(
                "println",
                Vec::new(),
                vec![param("message", boolean, message)],
                unit,
                hir::Body {
                    locals,
                    statements: vec![expr_stmt(call_typed(
                        println_string,
                        vec![call_typed(
                            bool_to_string,
                            vec![local_ref(message, boolean)],
                            string,
                        )],
                        unit,
                    ))],
                },
            );
            self.println_boolean = Some(id);
            id
        }

        /// `Option<inner>` (core's enum applied to one argument).
        fn option(&mut self, inner: hir::TypeId) -> hir::TypeId {
            self.types
                .alloc(hir::Type::Enum(self.option_enum, vec![inner]))
        }

        fn any(&mut self) -> hir::TypeId {
            self.types.alloc(hir::Type::Any)
        }

        fn class_ty(&mut self, id: hir::ClassId) -> hir::TypeId {
            self.types.alloc(hir::Type::Class(id))
        }

        fn interface_ty(&mut self, id: hir::InterfaceId) -> hir::TypeId {
            self.types.alloc(hir::Type::Interface(id))
        }

        fn interface(&mut self, name: &str, methods: &[&str]) -> hir::InterfaceId {
            let unit = self.unit;
            self.interfaces.alloc(hir::InterfaceDecl {
                name: name.to_string(),
                methods: methods
                    .iter()
                    .map(|name| hir::MethodSig {
                        name: name.to_string(),
                        params: Vec::new(),
                        return_ty: unit,
                        span: SPAN,
                    })
                    .collect(),
                span: SPAN,
            })
        }

        #[allow(clippy::too_many_arguments)]
        fn class(
            &mut self,
            name: &str,
            modifier: hir::ClassModifier,
            constructor: &[(&str, hir::TypeId)],
            base: Option<(hir::ClassId, Vec<hir::Expr>)>,
            interfaces: &[hir::InterfaceId],
        ) -> hir::ClassId {
            self.classes.alloc(hir::ClassDecl {
                modifier,
                name: name.to_string(),
                constructor: constructor
                    .iter()
                    .map(|(name, ty)| hir::Field {
                        name: name.to_string(),
                        ty: *ty,
                    })
                    .collect(),
                base_class: base,
                interfaces: interfaces.to_vec(),
                span: SPAN,
            })
        }

        /// core's built-in exception classes (throwable.scoop),
        /// declared flat — no constructor properties, no base:
        /// mir-lower only resolves them by name to call the
        /// generated constructor, so the test shell keeps the minimal
        /// shape. Tests declare exactly the exceptions they use, so
        /// unrelated dumps stay free of them.
        fn exception(&mut self, name: &str) -> hir::ClassId {
            self.class(name, hir::ClassModifier::Final, &[], None, &[])
        }

        /// A member function (kept out of `top_level`, as hir-lower
        /// does); `method_of` is the receiver type.
        /// A member function (kept out of `top_level`, as hir-lower
        /// does); `method_of` is the receiver type. The name is
        /// qualified `Owner.method`, as hir-lower names members.
        fn method_fn(
            &mut self,
            name: &str,
            method_of: hir::TypeId,
            params: Vec<hir::Param>,
            return_ty: hir::TypeId,
            body: hir::Body,
        ) -> hir::FunctionId {
            self.functions.alloc(hir::Function {
                name: name.to_string(),
                type_params: Vec::new(),
                params,
                return_ty,
                kind: hir::FunctionKind::User(body),
                method_of: Some(method_of),
                span: SPAN,
            })
        }

        fn strukt(&mut self, name: &str, fields: &[(&str, hir::TypeId)]) -> hir::StructId {
            self.strukt_with(name, fields, &[])
        }

        fn strukt_with(
            &mut self,
            name: &str,
            fields: &[(&str, hir::TypeId)],
            interfaces: &[hir::InterfaceId],
        ) -> hir::StructId {
            self.structs.alloc(hir::StructDecl {
                name: name.to_string(),
                fields: fields
                    .iter()
                    .map(|(name, ty)| hir::Field {
                        name: name.to_string(),
                        ty: *ty,
                    })
                    .collect(),
                interfaces: interfaces.to_vec(),
                span: SPAN,
            })
        }

        fn tuple(&mut self, elements: &[hir::TypeId]) -> hir::TypeId {
            self.types.alloc(hir::Type::Tuple(elements.to_vec()))
        }

        fn array(&mut self, element: hir::TypeId) -> hir::TypeId {
            self.types.alloc(hir::Type::Array(element))
        }

        fn mutable_array(&mut self, element: hir::TypeId) -> hir::TypeId {
            self.types.alloc(hir::Type::MutableArray(element))
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
                method_of: None,
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
                classes: self.classes,
                interfaces: self.interfaces,
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

    /// A call expression with an explicit result type (hir-lower
    /// annotates every expression; core's overloads call the
    /// String-returning conversion intrinsics).
    fn call_typed(function: hir::FunctionId, args: Vec<hir::Expr>, ty: hir::TypeId) -> hir::Expr {
        expr(
            hir::ExprKind::Call {
                function,
                type_args: Vec::new(),
                args,
            },
            ty,
        )
    }

    fn struct_init(struct_id: hir::StructId, ty: hir::TypeId, args: Vec<hir::Expr>) -> hir::Expr {
        expr(hir::ExprKind::StructInit { struct_id, args }, ty)
    }

    /// `main` calls `println("hello, world")` then `helper()`, which
    /// calls `print("!")`.
    fn hello_world() -> hir::Module {
        let mut h = Harness::new();
        let print = h.print_string();
        let println = h.println_string();
        let helper = h.user_fn(
            "helper",
            hir::Body {
                locals: Arena::new(),
                statements: vec![expr_stmt(call(&h, print, vec![str_lit(&h, "!")]))],
            },
        );
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: vec![
                    expr_stmt(call(&h, println, vec![str_lit(&h, "hello, world")])),
                    expr_stmt(call(&h, helper, vec![])),
                ],
            },
        );
        h.finish(main)
    }

    #[test]
    fn lowers_hello_world() {
        let module = lower(&hello_world());

        // Intrinsics are excluded from `top_level`; declaration order
        // kept: the two core overloads the test uses, then the user
        // functions.
        assert_eq!(module.top_level.len(), 4);
        let helper = &module.functions[module.top_level[2]];
        let main = &module.functions[module.top_level[3]];
        assert_eq!(helper.name, "helper");
        assert_eq!(main.name, "main");

        // Mangling: entry is the fixed `scoop_main`, others `scoop.<name>`.
        assert_eq!(main.symbol, mir::ENTRY_SYMBOL);
        assert_eq!(helper.symbol, "scoop.helper");
        assert_eq!(module.entry, module.top_level[3]);

        // String literals became numbered global constants (in lowering
        // order: function bodies are lowered in declaration order, so
        // core's `println(String)` contributes its `"\n"` first).
        let strings: Vec<(&str, &str)> = module
            .strings
            .iter()
            .map(|(_, s)| (s.value.as_str(), s.symbol.as_str()))
            .collect();
        assert_eq!(
            strings,
            [
                ("\n", "scoop.str.0"),
                ("!", "scoop.str.1"),
                ("hello, world", "scoop.str.2")
            ]
        );

        // M2 meta exists but is empty.
        assert!(module.meta.dispatch_tables.is_empty());

        // Golden dump locks the output structure.
        let expected = "\
Module
  fun print @scoop.print(message: String) -> Unit
    Call @scoop_rt_print direct
      Local message
  fun println @scoop.println(message: String) -> Unit
    Call @scoop_rt_print direct
      Local message
    Call @scoop_rt_print direct
      StringConst @scoop.str.0
  fun helper @scoop.helper() -> Unit
    Call @scoop.print direct
      StringConst @scoop.str.1
  fun main @scoop_main() -> Unit
    Call @scoop.println direct
      StringConst @scoop.str.2
    Call @scoop.helper direct
  str @scoop.str.0 \"\\n\"
  str @scoop.str.1 \"!\"
  str @scoop.str.2 \"hello, world\"
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }

    #[test]
    fn repeated_literals_get_separate_constants_deterministically() {
        let mut hir_module = hello_world();
        // Add another `println("hello, world")` to `main`. The
        // `println(String)` overload is the second function in
        // `top_level` (after `print(String)`).
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
        assert_eq!(
            symbols,
            ["scoop.str.0", "scoop.str.1", "scoop.str.2", "scoop.str.3"]
        );
    }

    #[test]
    fn intrinsic_names_map_to_runtime_functions() {
        // core's `print` / `println` overloads are ordinary user
        // functions (their forwarding is locked by the golden dumps);
        // only the three intrinsic primitives map onto runtime
        // functions, by intrinsic name.
        let mut h = Harness::new();
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: vec![
                    expr_stmt(call(&h, h.write, vec![str_lit(&h, "s")])),
                    expr_stmt(call_typed(h.int_to_string, vec![int_lit(&h, 1)], h.string)),
                    expr_stmt(call_typed(
                        h.bool_to_string,
                        vec![bool_lit(&h, true)],
                        h.string,
                    )),
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
                mir::RuntimeFn::Write,
                mir::RuntimeFn::IntToString,
                mir::RuntimeFn::BoolToString,
            ]
        );
    }

    #[test]
    fn print_overloads_are_ordinary_calls() {
        // M7: calls to core's `print` / `println` overloads resolve to
        // the overload's own MIR function (`Callee::User`); only the
        // intrinsic primitives inside their bodies are runtime calls.
        let mut h = Harness::new();
        let print_string = h.print_string();
        let print_int = h.print_int();
        let print_boolean = h.print_boolean();
        let println_string = h.println_string();
        let println_int = h.println_int();
        let println_boolean = h.println_boolean();
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: vec![
                    expr_stmt(call(&h, print_string, vec![str_lit(&h, "s")])),
                    expr_stmt(call(&h, print_int, vec![int_lit(&h, 1)])),
                    expr_stmt(call(&h, print_boolean, vec![bool_lit(&h, true)])),
                    expr_stmt(call(&h, println_string, vec![str_lit(&h, "t")])),
                    expr_stmt(call(&h, println_int, vec![int_lit(&h, 2)])),
                    expr_stmt(call(&h, println_boolean, vec![bool_lit(&h, false)])),
                ],
            },
        );
        let module = lower(&h.finish(main));

        let body = &module.functions[module.entry].body;
        let callees: Vec<mir::FunctionId> = body
            .statements
            .iter()
            .map(|statement| {
                let mir::StatementKind::Expr(mir::Expr::Call(call)) = &statement.kind else {
                    panic!("expected a call statement")
                };
                let mir::Callee::User(id) = call.target.callee else {
                    panic!("print/println calls must be ordinary user calls")
                };
                id
            })
            .collect();
        // The overloads are the first six MIR functions (declaration
        // order: the three `print`s, then the three `println`s), and
        // each overload's symbol carries the parameter encoding.
        assert_eq!(callees, module.top_level[..6]);
        let symbols: Vec<&str> = callees
            .iter()
            .map(|&id| module.functions[id].symbol.as_str())
            .collect();
        assert_eq!(
            symbols,
            [
                "scoop.print.S",
                "scoop.print.I",
                "scoop.print.B",
                "scoop.println.S",
                "scoop.println.I",
                "scoop.println.B",
            ]
        );
    }

    /// `fun <name>(<params>): String = <text>` — one overload each.
    fn string_fn(
        h: &mut Harness,
        name: &str,
        params: &[(&str, hir::TypeId)],
        text: &str,
    ) -> hir::FunctionId {
        let string = h.string;
        let mut locals = Arena::new();
        let params: Vec<hir::Param> = params
            .iter()
            .map(|(name, ty)| {
                let local_id = locals.alloc(local(name, *ty));
                param(name, *ty, local_id)
            })
            .collect();
        let init = str_lit(h, text);
        h.user_fn_full(
            name,
            Vec::new(),
            params,
            string,
            hir::Body {
                locals,
                statements: vec![stmt(hir::StatementKind::Return { value: Some(init) })],
            },
        )
    }

    fn top_level_symbols(module: &mir::Module) -> Vec<&str> {
        module
            .top_level
            .iter()
            .map(|&id| module.functions[id].symbol.as_str())
            .collect()
    }

    #[test]
    fn overloads_mangle_with_param_encoding() {
        let mut h = Harness::new();
        let (int, string) = (h.int, h.string);
        string_fn(&mut h, "show", &[("v", int)], "int");
        string_fn(&mut h, "show", &[("v", string)], "string");
        string_fn(&mut h, "show", &[("v", int), ("extra", int)], "two");
        // A unique name keeps the plain `scoop.<name>` symbol.
        string_fn(&mut h, "helper", &[], "h");
        let main = empty_main(&mut h);
        let module = lower(&h.finish(main));

        assert_eq!(
            top_level_symbols(&module),
            [
                "scoop.show.I",
                "scoop.show.S",
                "scoop.show.I_I",
                "scoop.helper",
                "scoop_main"
            ]
        );
    }

    #[test]
    fn zero_parameter_overload_mangles_with_an_empty_encoding() {
        let mut h = Harness::new();
        let int = h.int;
        string_fn(&mut h, "f", &[], "none");
        string_fn(&mut h, "f", &[("v", int)], "one");
        let main = empty_main(&mut h);
        let module = lower(&h.finish(main));

        assert_eq!(
            top_level_symbols(&module),
            ["scoop.f.", "scoop.f.I", "scoop_main"]
        );
    }

    #[test]
    fn overload_symbols_do_not_collide_with_instance_symbols() {
        // `show(Int)` / `show(String)` overloads plus a generic
        // `show<T>` instantiated with `Int`: `.` vs `$` keep the
        // symbols distinct.
        let mut h = Harness::new();
        let (int, string) = (h.int, h.string);
        string_fn(&mut h, "show", &[("v", int)], "int");
        string_fn(&mut h, "show", &[("v", string)], "string");
        let generic = identity_fn(&mut h, "show");
        h.instantiate(generic, vec![int]);
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: vec![expr_stmt(generic_call(
                    generic,
                    vec![int],
                    vec![int_lit(&h, 1)],
                    int,
                ))],
            },
        );
        let module = lower(&h.finish(main));

        let symbols = top_level_symbols(&module);
        for expected in ["scoop.show.I", "scoop.show.S", "scoop.show$I"] {
            assert!(
                symbols.contains(&expected),
                "missing {expected} in {symbols:?}"
            );
        }
    }

    #[test]
    fn method_overloads_mangle_with_param_encoding() {
        // `class Doc { fun describe(v: Int); fun describe(v: String) }`:
        // the receiver is not part of the overload encoding.
        let mut h = Harness::new();
        let (int, string, unit) = (h.int, h.string, h.unit);
        let doc = h.class("Doc", hir::ClassModifier::Final, &[], None, &[]);
        let doc_ty = h.class_ty(doc);
        for ty in [int, string] {
            let mut locals = Arena::new();
            let this = locals.alloc(local("this", doc_ty));
            let v = locals.alloc(local("v", ty));
            h.method_fn(
                "Doc.describe",
                doc_ty,
                vec![param("this", doc_ty, this), param("v", ty, v)],
                unit,
                hir::Body {
                    locals,
                    statements: Vec::new(),
                },
            );
        }
        let main = empty_main(&mut h);
        let module = lower(&h.finish(main));

        let symbols: std::collections::HashSet<&str> = module
            .functions
            .iter()
            .map(|(_, f)| f.symbol.as_str())
            .collect();
        assert!(symbols.contains("scoop.Doc.describe.I"));
        assert!(symbols.contains("scoop.Doc.describe.S"));
        // Each overload gets its own vtable slot (keyed by signature),
        // referencing the final (overload-encoded) symbol by id.
        let doc_def = &module.classes[class_index(0)];
        assert_eq!(doc_def.vtable.len(), 5);
        assert_eq!(slot_fn(&module, &doc_def.vtable[3]), "scoop.Doc.describe.I");
        assert_eq!(slot_fn(&module, &doc_def.vtable[4]), "scoop.Doc.describe.S");
    }

    /// A class method returning an Int constant:
    /// `fun <owner>.<name>(v: <param_ty>): Int = <value>` (param
    /// optional). Returns the HIR function id.
    fn int_method(
        h: &mut Harness,
        qualified: &str,
        receiver: hir::TypeId,
        param_ty: Option<hir::TypeId>,
        value: i64,
    ) -> hir::FunctionId {
        let int = h.int;
        let mut locals = Arena::new();
        let this = locals.alloc(local("this", receiver));
        let mut params = vec![param("this", receiver, this)];
        if let Some(ty) = param_ty {
            let v = locals.alloc(local("v", ty));
            params.push(param("v", ty, v));
        }
        h.method_fn(
            qualified,
            receiver,
            params,
            int,
            hir::Body {
                locals,
                statements: vec![stmt(hir::StatementKind::Return {
                    value: Some(int_lit(h, value)),
                })],
            },
        )
    }

    #[test]
    fn overridden_overload_replaces_the_base_slot_in_place() {
        // open class A { fun s(v: Int): Int = 1; fun s(v: String): Int = 2 }
        // class B : A() { override fun s(v: Int): Int = 3 }
        let mut h = Harness::new();
        let (int, string) = (h.int, h.string);
        let a = h.class("A", hir::ClassModifier::Open, &[], None, &[]);
        let a_ty = h.class_ty(a);
        int_method(&mut h, "A.s", a_ty, Some(int), 1);
        int_method(&mut h, "A.s", a_ty, Some(string), 2);
        let b = h.class(
            "B",
            hir::ClassModifier::Final,
            &[],
            Some((a, Vec::new())),
            &[],
        );
        let b_ty = h.class_ty(b);
        int_method(&mut h, "B.s", b_ty, Some(int), 3);
        let main = empty_main(&mut h);
        let module = lower(&h.finish(main));

        // A: two overload slots after the Any defaults.
        let a_def = &module.classes[class_index(0)];
        assert_eq!(a_def.vtable.len(), 5);
        assert_eq!(slot_fn(&module, &a_def.vtable[3]), "scoop.A.s.I");
        assert_eq!(slot_fn(&module, &a_def.vtable[4]), "scoop.A.s.S");
        // B: the `s(Int)` override replaces slot 3 in place; the
        // inherited `s(String)` keeps slot 4. (`B.s` is a unique name
        // in the module, so it keeps the plain symbol.)
        let b_def = &module.classes[class_index(1)];
        assert_eq!(b_def.vtable.len(), 5);
        assert_eq!(slot_fn(&module, &b_def.vtable[3]), "scoop.B.s");
        assert_eq!(slot_fn(&module, &b_def.vtable[4]), "scoop.A.s.S");
    }

    #[test]
    fn virtual_calls_annotate_the_overloads_own_slot() {
        // `val a: A = ...; a.s(1); a.s("x")` — the callee is the
        // signature resolved on the static type; each call annotates
        // its own overload's slot.
        let mut h = Harness::new();
        let (int, string, unit) = (h.int, h.string, h.unit);
        let a = h.class("A", hir::ClassModifier::Open, &[], None, &[]);
        let a_ty = h.class_ty(a);
        let s_int = int_method(&mut h, "A.s", a_ty, Some(int), 1);
        let s_string = int_method(&mut h, "A.s", a_ty, Some(string), 2);
        let method_call = |receiver: hir::Expr, function: hir::FunctionId, arg: hir::Expr| {
            expr(
                hir::ExprKind::MethodCall {
                    receiver: Box::new(receiver),
                    function,
                    args: vec![arg],
                },
                unit,
            )
        };
        let mut locals = Arena::new();
        let av = locals.alloc(local("a", a_ty));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![
                    expr_stmt(method_call(local_ref(av, a_ty), s_int, int_lit(&h, 1))),
                    expr_stmt(method_call(local_ref(av, a_ty), s_string, str_lit(&h, "x"))),
                ],
            },
        );
        let module = lower(&h.finish(main));

        let body = &module.functions[module.entry].body;
        let call_kind = |index: usize| {
            let mir::StatementKind::Expr(mir::Expr::Call(call)) = &body.statements[index].kind
            else {
                panic!("expected a call statement")
            };
            &call.target.kind
        };
        assert!(matches!(call_kind(0), mir::CallKind::Virtual { slot: 3 }));
        assert!(matches!(call_kind(1), mir::CallKind::Virtual { slot: 4 }));
    }

    /// `interface <name> { fun m(v: T)... }` — one `MethodSig` per
    /// `(name, param type)` entry, as hir-lower materializes them
    /// (interface methods carry no `this` in the signature).
    fn overloaded_interface(
        h: &mut Harness,
        name: &str,
        methods: &[(&str, hir::TypeId)],
    ) -> hir::InterfaceId {
        let unit = h.unit;
        let mut locals = Arena::new();
        h.interfaces.alloc(hir::InterfaceDecl {
            name: name.to_string(),
            methods: methods
                .iter()
                .map(|(name, ty)| {
                    let v = locals.alloc(local("v", *ty));
                    hir::MethodSig {
                        name: name.to_string(),
                        params: vec![param("v", *ty, v)],
                        return_ty: unit,
                        span: SPAN,
                    }
                })
                .collect(),
            span: SPAN,
        })
    }

    #[test]
    fn overloaded_interface_methods_get_one_itable_slot_each() {
        // interface Multi { fun m(v: Int); fun m(v: String) }
        // class C : Multi implements both overloads.
        let mut h = Harness::new();
        let (int, string) = (h.int, h.string);
        let multi = overloaded_interface(&mut h, "Multi", &[("m", int), ("m", string)]);
        let c = h.class("C", hir::ClassModifier::Final, &[], None, &[multi]);
        let c_ty = h.class_ty(c);
        int_method(&mut h, "C.m", c_ty, Some(int), 1);
        int_method(&mut h, "C.m", c_ty, Some(string), 2);
        let main = empty_main(&mut h);
        let module = lower(&h.finish(main));

        let c_def = &module.classes[class_index(0)];
        assert_eq!(c_def.itables.len(), 1);
        let record = &c_def.itables[0];
        assert_eq!(record.slots.len(), 2);
        assert_eq!(slot_fn(&module, &record.slots[0]), "scoop.C.m.I");
        assert_eq!(slot_fn(&module, &record.slots[1]), "scoop.C.m.S");
    }

    #[test]
    fn interface_calls_annotate_the_overloads_own_slot() {
        // `val i: Multi = ...; i.m(1); i.m("x")` — interface dispatch
        // locates the slot by the callee's signature.
        let mut h = Harness::new();
        let (int, string, unit) = (h.int, h.string, h.unit);
        let multi = overloaded_interface(&mut h, "Multi", &[("m", int), ("m", string)]);
        let multi_ty = h.interface_ty(multi);
        // Interface method shells, as hir-lower materializes them
        // (params include `this`).
        let shell = |h: &mut Harness, ty: hir::TypeId| {
            let mut locals = Arena::new();
            let this = locals.alloc(local("this", multi_ty));
            let v = locals.alloc(local("v", ty));
            h.method_fn(
                "Multi.m",
                multi_ty,
                vec![param("this", multi_ty, this), param("v", ty, v)],
                unit,
                hir::Body {
                    locals,
                    statements: Vec::new(),
                },
            )
        };
        let m_int = shell(&mut h, int);
        let m_string = shell(&mut h, string);
        let method_call = |receiver: hir::Expr, function: hir::FunctionId, arg: hir::Expr| {
            expr(
                hir::ExprKind::MethodCall {
                    receiver: Box::new(receiver),
                    function,
                    args: vec![arg],
                },
                unit,
            )
        };
        let mut locals = Arena::new();
        let i = locals.alloc(local("i", multi_ty));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![
                    expr_stmt(method_call(local_ref(i, multi_ty), m_int, int_lit(&h, 1))),
                    expr_stmt(method_call(
                        local_ref(i, multi_ty),
                        m_string,
                        str_lit(&h, "x"),
                    )),
                ],
            },
        );
        let module = lower(&h.finish(main));

        let body = &module.functions[module.entry].body;
        let call_kind = |index: usize| {
            let mir::StatementKind::Expr(mir::Expr::Call(call)) = &body.statements[index].kind
            else {
                panic!("expected a call statement")
            };
            &call.target.kind
        };
        let is_iface_slot = |kind: &mir::CallKind, slot: u32| matches!(kind, mir::CallKind::Interface { slot: s, .. } if *s == slot);
        assert!(is_iface_slot(call_kind(0), 0));
        assert!(is_iface_slot(call_kind(1), 1));
    }

    #[test]
    fn boxed_thunks_of_overloaded_interface_methods_are_disambiguated() {
        // struct S : Multi implements both `m` overloads; boxing to
        // `Multi` generates one thunk per signature.
        let mut h = Harness::new();
        let (int, string) = (h.int, h.string);
        let multi = overloaded_interface(&mut h, "Multi", &[("m", int), ("m", string)]);
        let multi_ty = h.interface_ty(multi);
        let s = h.strukt_with("S", &[("x", int)], &[multi]);
        let s_ty = h.types.alloc(hir::Type::Struct(s));
        int_method(&mut h, "S.m", s_ty, Some(int), 1);
        int_method(&mut h, "S.m", s_ty, Some(string), 2);
        let mut locals = Arena::new();
        let d = locals.alloc(local("d", multi_ty));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![val_decl(
                    d,
                    expr(
                        hir::ExprKind::Box(Box::new(struct_init(s, s_ty, vec![int_lit(&h, 1)]))),
                        multi_ty,
                    ),
                )],
            },
        );
        let module = lower(&h.finish(main));

        let boxed = &module.classes[class_index(0)];
        assert_eq!(boxed.itables.len(), 1);
        let record = &boxed.itables[0];
        assert_eq!(record.slots.len(), 2);
        assert_eq!(
            slot_fn(&module, &record.slots[0]),
            "scoop.thunk.S.Multi.m.I"
        );
        assert_eq!(
            slot_fn(&module, &record.slots[1]),
            "scoop.thunk.S.Multi.m.S"
        );
        // Each thunk tail-calls its own overload.
        let thunk_target = |slot: &mir::TableSlot| {
            let symbol = slot_fn(&module, slot);
            let thunk = module
                .functions
                .iter()
                .map(|(_, f)| f)
                .find(|f| f.symbol == symbol)
                .expect("the thunk is a MIR function");
            let mir::StatementKind::Expr(mir::Expr::Call(call)) = &thunk.body.statements[0].kind
            else {
                panic!("the thunk tail-calls the value method")
            };
            let mir::Callee::User(target) = call.target.callee else {
                panic!("the thunk calls a user function")
            };
            module.functions[target].symbol.clone()
        };
        assert_eq!(thunk_target(&record.slots[0]), "scoop.S.m.I");
        assert_eq!(thunk_target(&record.slots[1]), "scoop.S.m.S");
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
        // Division is not here: its divisor check (M8) makes it a
        // statement sequence — see
        // `division_by_zero_throws_arithmetic_exception`.
        let int_cases = [
            (hir::BinOp::Add, mir::BinOp::IntAdd),
            (hir::BinOp::Sub, mir::BinOp::IntSub),
            (hir::BinOp::Mul, mir::BinOp::IntMul),
            (hir::BinOp::Lt, mir::BinOp::IntLt),
            (hir::BinOp::Le, mir::BinOp::IntLe),
            (hir::BinOp::Gt, mir::BinOp::IntGt),
            (hir::BinOp::Ge, mir::BinOp::IntGe),
            (hir::BinOp::Eq, mir::BinOp::IntEq),
            (hir::BinOp::Ne, mir::BinOp::IntNe),
        ];
        for (hir_op, _) in &int_cases {
            let ty = if matches!(hir_op, hir::BinOp::Add | hir::BinOp::Sub | hir::BinOp::Mul) {
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
    fn division_by_zero_throws_arithmetic_exception() {
        // val q = 10 / 2 — M8: both operands are evaluated once into
        // hidden locals (left to right), the zero check precedes the
        // division, and a zero divisor throws `ArithmeticException`.
        let mut h = Harness::new();
        h.exception("ArithmeticException");
        let int = h.int;
        let mut locals = Arena::new();
        let q = locals.alloc(local("q", int));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![val_decl(
                    q,
                    binary(hir::BinOp::Div, int_lit(&h, 10), int_lit(&h, 2), int),
                )],
            },
        );
        let module = lower(&h.finish(main));

        let expected = "\
Module
  class ArithmeticException vtable=3 itables=0
  fun main @scoop_main() -> Unit
    val $div.1: Int
      IntLiteral 10
    val $div.2: Int
      IntLiteral 2
    if
      Binary IntEq
        Local $div.2
        IntLiteral 0
      throw
        Call @scoop.ctor.ArithmeticException direct
    val q: Int
      Binary IntDiv
        Local $div.1
        Local $div.2
  fun ctor.ArithmeticException @scoop.ctor.ArithmeticException() -> ArithmeticException
    return
      ClassInit ArithmeticException
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }

    #[test]
    fn try_and_throw_stay_structured() {
        // try { throw MyError() } catch (e: MyError) { 1 } finally { 2 }
        // — MIR keeps the structured form (DESIGN 3.3); the
        // control-flow expansion is LIR's job.
        let mut h = Harness::new();
        let my_error = h.exception("MyError");
        let error_ty = h.class_ty(my_error);
        let mut locals = Arena::new();
        let e = locals.alloc(local("e", error_ty));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![stmt(hir::StatementKind::Try(hir::Try {
                    body: vec![stmt(hir::StatementKind::Throw(expr(
                        hir::ExprKind::ClassInit {
                            class_id: my_error,
                            args: Vec::new(),
                        },
                        error_ty,
                    )))],
                    catches: vec![hir::CatchClause {
                        local: e,
                        ty: error_ty,
                        body: vec![expr_stmt(int_lit(&h, 1))],
                        span: SPAN,
                    }],
                    finally_body: Some(vec![expr_stmt(int_lit(&h, 2))]),
                }))],
            },
        );
        let module = lower(&h.finish(main));

        let expected = "\
Module
  class MyError vtable=3 itables=0
  fun main @scoop_main() -> Unit
    try
      throw
        Call @scoop.ctor.MyError direct
    catch e: MyError
      IntLiteral 1
    finally
      IntLiteral 2
  fun ctor.MyError @scoop.ctor.MyError() -> MyError
    return
      ClassInit MyError
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
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
        let println = h.println_string();
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: vec![
                    stmt(hir::StatementKind::If {
                        cond: bool_lit(&h, true),
                        then_body: vec![expr_stmt(call(&h, println, vec![str_lit(&h, "a")]))],
                        else_body: Some(vec![expr_stmt(call(&h, println, vec![str_lit(&h, "b")]))]),
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
            interfaces: Vec::new(),
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
        h.exception("UnwrapException");
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
        // guards the extraction, and the else branch throws
        // `UnwrapException()` (M8) — an ordinary constructor call.
        let expected = "\
Module
  enum Option$I
    Some(_1: Int)
    None()
  class UnwrapException vtable=3 itables=0
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
      throw
        Call @scoop.ctor.UnwrapException direct
    val y: Int
      Local $uw.2
  fun ctor.UnwrapException @scoop.ctor.UnwrapException() -> UnwrapException
    return
      ClassInit UnwrapException
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
        let print_int = h.print_int();
        let println_string = h.println_string();
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
                                vec![expr_stmt(call(&h, print_int, vec![local_ref(x, int)]))],
                            ),
                            arm(
                                hir::Pattern::Variant {
                                    enum_id: option_enum,
                                    variant: 1,
                                    fields: Vec::new(),
                                },
                                None,
                                vec![expr_stmt(call(
                                    &h,
                                    println_string,
                                    vec![str_lit(&h, "none")],
                                ))],
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
        // failed tag test falls through to the next arm. (`print` /
        // `println` are ordinary core functions — M7 — so the arms
        // call the overloads, not runtime shims.)
        let expected = "\
Module
  enum Option$I
    Some(_1: Int)
    None()
  fun print @scoop.print(message: Int) -> Unit
    Call @scoop_rt_print direct
      Call @scoop_rt_int_to_string direct
        Local message
  fun println @scoop.println(message: String) -> Unit
    Call @scoop_rt_print direct
      Local message
    Call @scoop_rt_print direct
      StringConst @scoop.str.0
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
      Call @scoop.print direct
        Local x
    else
      if
        Binary IntEq
          EnumTag
            Local $when.1
          IntLiteral 1
        Call @scoop.println direct
          StringConst @scoop.str.1
  str @scoop.str.0 \"\\n\"
  str @scoop.str.1 \"none\"
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }

    #[test]
    fn a_failed_guard_falls_through_to_the_next_arm() {
        // when (o) { Some(x) if (x > 0) -> print(x); else -> println("neg") }
        let mut h = Harness::new();
        let print_int = h.print_int();
        let println_string = h.println_string();
        let int = h.int;
        let option_enum = h.option_enum;
        let option_int = h.option(int);
        let mut locals = Arena::new();
        let o = locals.alloc(local("o", option_int));
        let x = locals.alloc(local("x", int));
        let else_body = || {
            vec![expr_stmt(call(
                &h,
                println_string,
                vec![str_lit(&h, "neg")],
            ))]
        };
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
                        vec![expr_stmt(call(&h, print_int, vec![local_ref(x, int)]))],
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
  fun print @scoop.print(message: Int) -> Unit
    Call @scoop_rt_print direct
      Call @scoop_rt_int_to_string direct
        Local message
  fun println @scoop.println(message: String) -> Unit
    Call @scoop_rt_print direct
      Local message
    Call @scoop_rt_print direct
      StringConst @scoop.str.0
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
        Call @scoop.print direct
          Local x
      else
        Call @scoop.println direct
          StringConst @scoop.str.1
    else
      Call @scoop.println direct
        StringConst @scoop.str.2
  str @scoop.str.0 \"\\n\"
  str @scoop.str.1 \"neg\"
  str @scoop.str.2 \"neg\"
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }

    #[test]
    fn literal_patterns_match_by_equality() {
        // when (n) { 1 -> println("one"); else -> println("other") }
        let mut h = Harness::new();
        let println = h.println_string();
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
                        vec![expr_stmt(call(&h, println, vec![str_lit(&h, "one")]))],
                    )],
                    Some(vec![expr_stmt(call(
                        &h,
                        println,
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

    #[test]
    fn array_nodes_translate_one_to_one() {
        // val a = [1, 2, 3]; val x = a[0]; val n = a.size
        // val m: MutableArray<Int> = MutableArray(a); m[0] = 40
        let mut h = Harness::new();
        h.exception("IndexOutOfBoundsException");
        let int = h.int;
        let array_int = h.array(int);
        let mutable_int = h.mutable_array(int);
        let mut locals = Arena::new();
        let a = locals.alloc(local("a", array_int));
        let x = locals.alloc(local("x", int));
        let n = locals.alloc(local("n", int));
        let m = locals.alloc(local("m", mutable_int));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![
                    val_decl(
                        a,
                        expr(
                            hir::ExprKind::ArrayLiteral(vec![
                                int_lit(&h, 1),
                                int_lit(&h, 2),
                                int_lit(&h, 3),
                            ]),
                            array_int,
                        ),
                    ),
                    val_decl(
                        x,
                        expr(
                            hir::ExprKind::Index {
                                receiver: Box::new(local_ref(a, array_int)),
                                index: Box::new(int_lit(&h, 0)),
                            },
                            int,
                        ),
                    ),
                    val_decl(
                        n,
                        expr(
                            hir::ExprKind::ArrayLen(Box::new(local_ref(a, array_int))),
                            int,
                        ),
                    ),
                    val_decl(
                        m,
                        expr(
                            hir::ExprKind::ArrayClone(Box::new(local_ref(a, array_int))),
                            mutable_int,
                        ),
                    ),
                    stmt(hir::StatementKind::Assign {
                        target: hir::AssignTarget::Index {
                            array: local_ref(m, mutable_int),
                            index: int_lit(&h, 0),
                        },
                        value: int_lit(&h, 40),
                    }),
                ],
            },
        );
        let module = lower(&h.finish(main));

        // The subscript read and the indexed store both get the M8
        // bounds check: array and index evaluated once into hidden
        // locals, then `IndexOutOfBoundsException` on failure.
        let expected = "\
Module
  class IndexOutOfBoundsException vtable=3 itables=0
  fun main @scoop_main() -> Unit
    val a: Array<Int>
      ArrayLiteral
        IntLiteral 1
        IntLiteral 2
        IntLiteral 3
    val $arr.1: Array<Int>
      Local a
    val $idx.2: Int
      IntLiteral 0
    if
      Binary Or
        Binary IntLt
          Local $idx.2
          IntLiteral 0
        Binary IntGe
          Local $idx.2
          ArrayLen
            Local $arr.1
      throw
        Call @scoop.ctor.IndexOutOfBoundsException direct
    val x: Int
      ArrayGet
        Local $arr.1
        Local $idx.2
    val n: Int
      ArrayLen
        Local a
    val m: MutableArray<Int>
      ArrayClone
        Local a
    val $arr.3: MutableArray<Int>
      Local m
    val $idx.4: Int
      IntLiteral 0
    if
      Binary Or
        Binary IntLt
          Local $idx.4
          IntLiteral 0
        Binary IntGe
          Local $idx.4
          ArrayLen
            Local $arr.3
      throw
        Call @scoop.ctor.IndexOutOfBoundsException direct
    array_set
      Local $arr.3
      Local $idx.4
      IntLiteral 40
  fun ctor.IndexOutOfBoundsException @scoop.ctor.IndexOutOfBoundsException() -> IndexOutOfBoundsException
    return
      ClassInit IndexOutOfBoundsException
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }

    #[test]
    fn instance_symbols_encode_array_arguments() {
        let mut h = Harness::new();
        let f = identity_fn(&mut h, "f");
        let int = h.int;
        let array_int = h.array(int);
        let mutable_int = h.mutable_array(int);
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            },
        );
        h.instantiate(f, vec![array_int]);
        h.instantiate(f, vec![mutable_int]);
        let module = lower(&h.finish(main));

        let symbols: Vec<&str> = module.top_level[1..]
            .iter()
            .map(|&id| module.functions[id].symbol.as_str())
            .collect();
        // `mir::encode_type`: `A<element>X` / `M<element>X`.
        assert_eq!(symbols, ["scoop.f$AIX", "scoop.f$MIX"]);
        // Substitution recurses into the array element types.
        let array_instance = &module.functions[module.top_level[1]];
        assert_eq!(
            array_instance.params[0].ty,
            mir::Type::Array(Box::new(mir::Type::Int))
        );
        let mutable_instance = &module.functions[module.top_level[2]];
        assert_eq!(
            mutable_instance.return_ty,
            mir::Type::MutableArray(Box::new(mir::Type::Int))
        );
    }

    #[test]
    #[should_panic(expected = "hir-lower rejects equality on array types")]
    fn array_equality_never_reaches_the_expansion() {
        // hir-lower rejects `==` / `!=` on arrays (M5 has no array
        // equality semantics, DESIGN 6); feed one anyway to lock the
        // expansion's unreachable arm.
        let mut h = Harness::new();
        let int = h.int;
        let boolean = h.boolean;
        let array_int = h.array(int);
        let mut locals = Arena::new();
        let a = locals.alloc(local("a", array_int));
        let b = locals.alloc(local("b", array_int));
        let r = locals.alloc(local("r", boolean));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![val_decl(
                    r,
                    binary(
                        hir::BinOp::Eq,
                        local_ref(a, array_int),
                        local_ref(b, array_int),
                        boolean,
                    ),
                )],
            },
        );
        let _ = lower(&h.finish(main));
    }

    // ---- M6: reference types ----

    /// The symbol a vtable / itable slot points at.
    fn slot_fn<'a>(module: &'a mir::Module, slot: &mir::TableSlot) -> &'a str {
        match slot {
            mir::TableSlot::Function(id) => &module.functions[*id].symbol,
            mir::TableSlot::Runtime(function) => function.symbol(),
        }
    }

    /// A `this`-taking method with an empty body, as hir-lower
    /// produces it for `fun m() {}`-style declarations; the name is
    /// qualified `Owner.method` like hir-lower qualifies members.
    fn empty_method(
        h: &mut Harness,
        owner: &str,
        name: &str,
        receiver: hir::TypeId,
    ) -> hir::FunctionId {
        let mut locals = Arena::new();
        let this = locals.alloc(local("this", receiver));
        let unit = h.unit;
        h.method_fn(
            &format!("{owner}.{name}"),
            receiver,
            vec![param("this", receiver, this)],
            unit,
            hir::Body {
                locals,
                statements: Vec::new(),
            },
        )
    }

    fn empty_main(h: &mut Harness) -> hir::FunctionId {
        h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            },
        )
    }

    fn class_index(raw: u32) -> mir::ClassId {
        la_arena::Idx::from_raw(raw.into())
    }

    #[test]
    fn class_fields_are_base_prefix_then_own() {
        let mut h = Harness::new();
        let (int, string) = (h.int, h.string);
        let base = h.class("Base", hir::ClassModifier::Open, &[("a", int)], None, &[]);
        let derived = h.class(
            "Derived",
            hir::ClassModifier::Final,
            &[("b", string)],
            Some((base, vec![int_lit(&h, 0)])),
            &[],
        );
        let derived_ty = h.class_ty(derived);
        let mut locals = Arena::new();
        let d = locals.alloc(local("d", derived_ty));
        let b = locals.alloc(local("b", string));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![val_decl(
                    b,
                    expr(
                        hir::ExprKind::FieldAccess {
                            receiver: Box::new(local_ref(d, derived_ty)),
                            field: hir::FieldRef::ClassField {
                                class_id: derived,
                                index: 1,
                            },
                        },
                        string,
                    ),
                )],
            },
        );
        let module = lower(&h.finish(main));

        assert_eq!(module.classes.len(), 2);
        let base_def = &module.classes[class_index(0)];
        let derived_def = &module.classes[class_index(1)];
        let field_names = |def: &mir::ClassDef| {
            def.fields
                .iter()
                .map(|field| field.name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(field_names(base_def), ["a"]);
        // The base prefix comes first; HIR's `ClassField` indices
        // follow the same flattened order.
        assert_eq!(field_names(derived_def), ["a", "b"]);
        assert_eq!(derived_def.fields[1].ty, mir::Type::String);
        assert_eq!(derived_def.base_class, Some(class_index(0)));
        assert_eq!(derived_def.modifier, mir::ClassModifier::Final);
        assert_eq!(base_def.modifier, mir::ClassModifier::Open);

        // The field access keeps its 0-based index into the flattened
        // layout.
        let body = &module.functions[module.entry].body;
        let mir::StatementKind::ValDecl { init, .. } = &body.statements[0].kind else {
            panic!("expected a val declaration")
        };
        assert!(matches!(init, mir::Expr::FieldAccess { index: 1, .. }));
    }

    #[test]
    fn vtable_layout_copies_the_base_prefix_and_replaces_overrides() {
        let mut h = Harness::new();
        let base = h.class("Base", hir::ClassModifier::Open, &[], None, &[]);
        let base_ty = h.class_ty(base);
        let _m1 = empty_method(&mut h, "Base", "m1", base_ty);
        let _m2 = empty_method(&mut h, "Base", "m2", base_ty);
        let derived = h.class(
            "Derived",
            hir::ClassModifier::Open,
            &[],
            Some((base, vec![])),
            &[],
        );
        let derived_ty = h.class_ty(derived);
        // `m2` overrides the base method (same slot), `m3` is new
        // (appended after the base's slots).
        let _m2_derived = empty_method(&mut h, "Derived", "m2", derived_ty);
        let _m3 = empty_method(&mut h, "Derived", "m3", derived_ty);
        let main = empty_main(&mut h);
        let module = lower(&h.finish(main));

        let vtable_symbols = |def: &mir::ClassDef| {
            def.vtable
                .iter()
                .map(|slot| slot_fn(&module, slot))
                .collect::<Vec<_>>()
        };
        // Slots 0..2 are the Any defaults; member functions are
        // mangled qualified with their class.
        assert_eq!(
            vtable_symbols(&module.classes[class_index(0)]),
            [
                "scoop_rt_any_equals",
                "scoop_rt_any_hashcode",
                "scoop_rt_any_tostring",
                "scoop.Base.m1",
                "scoop.Base.m2",
            ]
        );
        // The base prefix is preserved; the override replaces slot 4
        // in place; the new method appends at slot 5.
        assert_eq!(
            vtable_symbols(&module.classes[class_index(1)]),
            [
                "scoop_rt_any_equals",
                "scoop_rt_any_hashcode",
                "scoop_rt_any_tostring",
                "scoop.Base.m1",
                "scoop.Derived.m2",
                "scoop.Derived.m3",
            ]
        );
    }

    #[test]
    fn itables_follow_the_interface_method_order() {
        let mut h = Harness::new();
        let iface = h.interface("Describable", &["a", "b"]);
        let class = h.class("C", hir::ClassModifier::Final, &[], None, &[iface]);
        let class_ty = h.class_ty(class);
        // The implementations are declared in reverse order: the
        // itable slots follow the interface's declaration order.
        let _impl_b = empty_method(&mut h, "C", "b", class_ty);
        let _impl_a = empty_method(&mut h, "C", "a", class_ty);
        // The derived class inherits `a` and overrides `b`; the
        // interface is covered without being redeclared.
        let derived = h.class(
            "D",
            hir::ClassModifier::Final,
            &[],
            Some((class, vec![])),
            &[],
        );
        let derived_ty = h.class_ty(derived);
        let _impl_b_d = empty_method(&mut h, "D", "b", derived_ty);
        let main = empty_main(&mut h);
        let module = lower(&h.finish(main));

        let class_def = &module.classes[class_index(0)];
        assert_eq!(class_def.itables.len(), 1);
        let record = &class_def.itables[0];
        assert_eq!(record.interface, la_arena::Idx::from_raw(0.into()));
        let slots: Vec<&str> = record
            .slots
            .iter()
            .map(|slot| slot_fn(&module, slot))
            .collect();
        assert_eq!(slots, ["scoop.C.a", "scoop.C.b"]);

        let derived_def = &module.classes[class_index(1)];
        assert_eq!(derived_def.itables.len(), 1);
        let record = &derived_def.itables[0];
        let slots: Vec<&str> = record
            .slots
            .iter()
            .map(|slot| slot_fn(&module, slot))
            .collect();
        // The override dispatches to the derived implementation; the
        // inherited method keeps the base's.
        assert_eq!(slots, ["scoop.C.a", "scoop.D.b"]);
    }

    #[test]
    fn method_calls_are_annotated_by_the_receiver_static_type() {
        let mut h = Harness::new();
        let iface = h.interface("Describable", &["describe", "label"]);
        let iface_ty = h.interface_ty(iface);
        let class = h.class("C", hir::ClassModifier::Open, &[], None, &[iface]);
        let class_ty = h.class_ty(class);
        let class_describe = empty_method(&mut h, "C", "describe", class_ty);
        let _class_label = empty_method(&mut h, "C", "label", class_ty);
        // Interface method shells, as hir-lower materializes them.
        let _iface_describe = empty_method(&mut h, "Describable", "describe", iface_ty);
        let iface_label = empty_method(&mut h, "Describable", "label", iface_ty);
        // A value type method.
        let int = h.int;
        let s = h.strukt("S", &[("x", int)]);
        let s_ty = h.types.alloc(hir::Type::Struct(s));
        let s_describe = empty_method(&mut h, "S", "describe", s_ty);

        let unit = h.unit;
        let mut locals = Arena::new();
        let c = locals.alloc(local("c", class_ty));
        let i = locals.alloc(local("i", iface_ty));
        let sv = locals.alloc(local("sv", s_ty));
        let method_call = |receiver: hir::Expr, function: hir::FunctionId| {
            expr(
                hir::ExprKind::MethodCall {
                    receiver: Box::new(receiver),
                    function,
                    args: Vec::new(),
                },
                unit,
            )
        };
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![
                    expr_stmt(method_call(local_ref(c, class_ty), class_describe)),
                    expr_stmt(method_call(local_ref(i, iface_ty), iface_label)),
                    expr_stmt(method_call(local_ref(sv, s_ty), s_describe)),
                ],
            },
        );
        let module = lower(&h.finish(main));

        let body = &module.functions[module.entry].body;
        let call_kind = |index: usize| {
            let mir::StatementKind::Expr(mir::Expr::Call(call)) = &body.statements[index].kind
            else {
                panic!("expected a call statement")
            };
            // The receiver becomes argument 0 (`this`).
            assert!(!call.args.is_empty());
            &call.target.kind
        };
        // Class receiver: virtual through the vtable (slot 3 = the
        // first slot after the Any defaults).
        assert!(matches!(call_kind(0), mir::CallKind::Virtual { slot: 3 }));
        // Interface receiver: the method's declaration index is the
        // itable slot.
        assert!(matches!(
            call_kind(1),
            mir::CallKind::Interface { interface, slot: 1 } if *interface == la_arena::Idx::from_raw(0.into())
        ));
        // Value type receiver: direct.
        assert!(matches!(call_kind(2), mir::CallKind::Direct));
    }

    #[test]
    fn boxing_a_value_type_creates_a_boxed_class_with_structural_equals() {
        let mut h = Harness::new();
        let int = h.int;
        let s = h.strukt("S", &[("x", int)]);
        let s_ty = h.types.alloc(hir::Type::Struct(s));
        let any = h.any();
        let mut locals = Arena::new();
        let a = locals.alloc(local("a", any));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![val_decl(
                    a,
                    expr(
                        hir::ExprKind::Box(Box::new(struct_init(s, s_ty, vec![int_lit(&h, 1)]))),
                        any,
                    ),
                )],
            },
        );
        let module = lower(&h.finish(main));

        // The boxed class: one field (the payload), the generated
        // structural equals in vtable slot 0, the Any defaults in
        // slots 1/2 (aggregate value types keep the Any `toString`
        // default until a spec'd structured format lands).
        assert_eq!(module.classes.len(), 1);
        let boxed = &module.classes[class_index(0)];
        assert_eq!(boxed.name, "box$S");
        assert_eq!(boxed.fields.len(), 1);
        assert_eq!(boxed.fields[0].name, "value");
        assert_eq!(
            boxed.fields[0].ty,
            mir::Type::Struct(la_arena::Idx::from_raw(0.into()))
        );
        let vtable: Vec<&str> = boxed
            .vtable
            .iter()
            .map(|slot| slot_fn(&module, slot))
            .collect();
        assert_eq!(
            vtable,
            [
                "scoop.eq.S",
                "scoop_rt_any_hashcode",
                "scoop_rt_any_tostring"
            ]
        );
        assert!(boxed.itables.is_empty());

        // The generated equals unboxes both payloads and compares
        // field by field (the M2 expansion).
        let expected = "\
Module
  struct S (x: Int)
  class box$S vtable=3 itables=0
  fun main @scoop_main() -> Unit
    val a: Any
      Box
        StructInit S
          IntLiteral 1
  fun eq.S @scoop.eq.S(this: Any, other: Any) -> Boolean
    val $a: S
      Unbox
        Local this
    val $b: S
      Unbox
        Local other
    return
      Binary IntEq
        FieldAccess 0
          Local $a
        FieldAccess 0
          Local $b
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }

    #[test]
    fn boxed_primitives_get_a_real_tostring_slot() {
        // `val a: Any = 42; val b: Any = true` — M7: the boxed
        // primitives' vtable slot 2 is a generated per-type `toString`
        // converting through the runtime, so `Any.toString()`
        // (core's `print` / `println`) produces the value's text.
        let mut h = Harness::new();
        let any = h.any();
        let mut locals = Arena::new();
        let a = locals.alloc(local("a", any));
        let b = locals.alloc(local("b", any));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![
                    val_decl(a, expr(hir::ExprKind::Box(Box::new(int_lit(&h, 42))), any)),
                    val_decl(
                        b,
                        expr(hir::ExprKind::Box(Box::new(bool_lit(&h, true))), any),
                    ),
                ],
            },
        );
        let module = lower(&h.finish(main));

        let boxed_int = &module.classes[class_index(0)];
        assert_eq!(boxed_int.name, "box$I");
        let vtable: Vec<&str> = boxed_int
            .vtable
            .iter()
            .map(|slot| slot_fn(&module, slot))
            .collect();
        assert_eq!(
            vtable,
            ["scoop.eq.I", "scoop_rt_any_hashcode", "scoop.tostring.I"]
        );
        let boxed_bool = &module.classes[class_index(1)];
        assert_eq!(boxed_bool.name, "box$B");
        assert_eq!(slot_fn(&module, &boxed_bool.vtable[2]), "scoop.tostring.B");

        // The generated toString: `return call scoop_rt_int_to_string(Unbox(this))`.
        let tostring = module
            .functions
            .iter()
            .map(|(_, f)| f)
            .find(|f| f.symbol == "scoop.tostring.I")
            .expect("the generated toString is a MIR function");
        assert_eq!(tostring.return_ty, mir::Type::String);
        assert_eq!(tostring.params.len(), 1);
        assert_eq!(tostring.params[0].ty, mir::Type::Any);
        let mir::StatementKind::Return {
            value: Some(mir::Expr::Call(call)),
        } = &tostring.body.statements[0].kind
        else {
            panic!("the toString body is a single return-call")
        };
        assert_eq!(
            call.target.callee,
            mir::Callee::Runtime(mir::RuntimeFn::IntToString)
        );
        assert!(
            matches!(&call.args[0], mir::Expr::Unbox(operand) if matches!(operand.as_ref(), mir::Expr::Local(local) if *local == tostring.params[0].local))
        );
        let tostring_b = module
            .functions
            .iter()
            .map(|(_, f)| f)
            .find(|f| f.symbol == "scoop.tostring.B")
            .expect("the generated toString is a MIR function");
        let mir::StatementKind::Return {
            value: Some(mir::Expr::Call(call_b)),
        } = &tostring_b.body.statements[0].kind
        else {
            panic!("the toString body is a single return-call")
        };
        assert_eq!(
            call_b.target.callee,
            mir::Callee::Runtime(mir::RuntimeFn::BoolToString)
        );
    }

    #[test]
    fn boxed_interface_implementations_dispatch_through_adjust_thunks() {
        let mut h = Harness::new();
        let int = h.int;
        let iface = h.interface("Describable", &["describe"]);
        let iface_ty = h.interface_ty(iface);
        let s = h.strukt("S", &[("x", int)]);
        let s_ty = h.types.alloc(hir::Type::Struct(s));
        let _describe = empty_method(&mut h, "S", "describe", s_ty);
        // `val d: Describable = S(1)` — a Box whose target is the
        // interface.
        let mut locals = Arena::new();
        let d = locals.alloc(local("d", iface_ty));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![val_decl(
                    d,
                    expr(
                        hir::ExprKind::Box(Box::new(struct_init(s, s_ty, vec![int_lit(&h, 1)]))),
                        iface_ty,
                    ),
                )],
            },
        );
        let module = lower(&h.finish(main));

        let boxed = &module.classes[class_index(0)];
        assert_eq!(boxed.interfaces.len(), 1);
        assert_eq!(boxed.itables.len(), 1);
        let record = &boxed.itables[0];
        assert_eq!(record.interface, boxed.interfaces[0]);
        assert_eq!(record.slots.len(), 1);
        let thunk_symbol = slot_fn(&module, &record.slots[0]);
        assert_eq!(thunk_symbol, "scoop.thunk.S.Describable.describe");

        // The thunk takes the boxed object as `this`, unboxes it and
        // tail-calls the value method.
        let thunk = module
            .functions
            .iter()
            .map(|(_, f)| f)
            .find(|f| f.symbol == thunk_symbol)
            .expect("the thunk is a MIR function");
        assert_eq!(thunk.params.len(), 1);
        assert_eq!(thunk.params[0].ty, mir::Type::Any);
        assert_eq!(thunk.params[0].name, "this");
        let mir::StatementKind::Expr(mir::Expr::Call(call)) = &thunk.body.statements[0].kind else {
            panic!("the thunk tail-calls the value method")
        };
        assert!(matches!(call.target.kind, mir::CallKind::Direct));
        let mir::Callee::User(impl_id) = call.target.callee else {
            panic!("the thunk calls a user function")
        };
        assert_eq!(module.functions[impl_id].symbol, "scoop.S.describe");
        assert_eq!(call.args.len(), 1);
        assert!(
            matches!(&call.args[0], mir::Expr::Unbox(operand) if matches!(operand.as_ref(), mir::Expr::Local(local) if *local == thunk.params[0].local))
        );
    }

    #[test]
    fn is_instance_and_casts_lower_to_runtime_checks() {
        let mut h = Harness::new();
        h.exception("ClassCastException");
        let (int, boolean) = (h.int, h.boolean);
        let s = h.strukt("S", &[("x", int)]);
        let s_ty = h.types.alloc(hir::Type::Struct(s));
        let any = h.any();
        let option_s = h.option(s_ty);
        let mut locals = Arena::new();
        let a = locals.alloc(local("a", any));
        let is_s = locals.alloc(local("is_s", boolean));
        let s2 = locals.alloc(local("s2", s_ty));
        let maybe = locals.alloc(local("maybe", option_s));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![
                    val_decl(
                        is_s,
                        expr(
                            hir::ExprKind::IsInstance {
                                operand: Box::new(local_ref(a, any)),
                                check_ty: s_ty,
                            },
                            boolean,
                        ),
                    ),
                    val_decl(
                        s2,
                        // Mirror hir-lower's real shape: a value-typed
                        // `as` arrives as `Unbox(Cast)`; mir-lower's
                        // cast expansion only performs the check.
                        expr(
                            hir::ExprKind::Unbox(Box::new(expr(
                                hir::ExprKind::Cast {
                                    operand: Box::new(local_ref(a, any)),
                                    optional: false,
                                },
                                s_ty,
                            ))),
                            s_ty,
                        ),
                    ),
                    val_decl(
                        maybe,
                        expr(
                            hir::ExprKind::Cast {
                                operand: Box::new(local_ref(a, any)),
                                optional: true,
                            },
                            option_s,
                        ),
                    ),
                ],
            },
        );
        let module = lower(&h.finish(main));

        // `is` stays a dedicated node; `as` throws
        // `ClassCastException` on failure (M8); `as?` wraps in
        // Some / None. The value-type checks registered the boxed
        // class (and its equals function).
        let expected = "\
Module
  struct S (x: Int)
  enum Option$S
    Some(_1: S)
    None()
  class ClassCastException vtable=3 itables=0
  class box$S vtable=3 itables=0
  fun main @scoop_main() -> Unit
    val is_s: Boolean
      IsInstance S
        Local a
    val $cast.1: Any
      Local a
    if
      Unary BoolNot
        IsInstance S
          Local $cast.1
      throw
        Call @scoop.ctor.ClassCastException direct
    val $ub.2: S
      Unbox
        Local $cast.1
    val s2: S
      Local $ub.2
    val $cast.3: Any
      Local a
    if
      IsInstance S
        Local $cast.3
      assign $cast.4
        VariantConstruct Option$S<S> v0
          Unbox
            Local $cast.3
    else
      assign $cast.4
        VariantConstruct Option$S<S> v1
    val maybe: Option$S<S>
      Local $cast.4
  fun ctor.ClassCastException @scoop.ctor.ClassCastException() -> ClassCastException
    return
      ClassInit ClassCastException
  fun eq.S @scoop.eq.S(this: Any, other: Any) -> Boolean
    val $a: S
      Unbox
        Local this
    val $b: S
      Unbox
        Local other
    return
      Binary IntEq
        FieldAccess 0
          Local $a
        FieldAccess 0
          Local $b
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }

    #[test]
    fn reference_equality_is_a_pointer_comparison() {
        let mut h = Harness::new();
        let boolean = h.boolean;
        let c = h.class("C", hir::ClassModifier::Final, &[], None, &[]);
        let c_ty = h.class_ty(c);
        let mut locals = Arena::new();
        let x = locals.alloc(local("x", c_ty));
        let y = locals.alloc(local("y", c_ty));
        let eq = locals.alloc(local("eq", boolean));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![val_decl(
                    eq,
                    binary(
                        hir::BinOp::Eq,
                        local_ref(x, c_ty),
                        local_ref(y, c_ty),
                        boolean,
                    ),
                )],
            },
        );
        let module = lower(&h.finish(main));

        // `==` on references is identity (the M6 Any default): the
        // primitive comparison on the two pointers — no runtime call,
        // no vtable dispatch.
        let body = &module.functions[module.entry].body;
        let mir::StatementKind::ValDecl {
            init:
                mir::Expr::Binary {
                    op: mir::BinOp::IntEq,
                    lhs,
                    rhs,
                },
            ..
        } = &body.statements[0].kind
        else {
            panic!("reference equality must be a primitive comparison")
        };
        assert!(matches!(lhs.as_ref(), mir::Expr::Local(_)));
        assert!(matches!(rhs.as_ref(), mir::Expr::Local(_)));
    }

    #[test]
    fn constructor_functions_initialize_the_flattened_fields() {
        // open class Root(val label: String)
        // open class Base(val name: String) : Root("root")
        // class Point(val x: Int) : Base("point")
        let mut h = Harness::new();
        let (int, string) = (h.int, h.string);
        let root = h.class(
            "Root",
            hir::ClassModifier::Open,
            &[("label", string)],
            None,
            &[],
        );
        let base = h.class(
            "Base",
            hir::ClassModifier::Open,
            &[("name", string)],
            Some((root, vec![str_lit(&h, "root")])),
            &[],
        );
        let point = h.class(
            "Point",
            hir::ClassModifier::Final,
            &[("x", int)],
            Some((base, vec![str_lit(&h, "point")])),
            &[],
        );
        let point_ty = h.class_ty(point);
        let mut locals = Arena::new();
        let p = locals.alloc(local("p", point_ty));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![val_decl(
                    p,
                    expr(
                        hir::ExprKind::ClassInit {
                            class_id: point,
                            args: vec![int_lit(&h, 1)],
                        },
                        point_ty,
                    ),
                )],
            },
        );
        let module = lower(&h.finish(main));

        // One ctor per class; the use site is a plain direct call.
        // Each ctor returns a raw ClassInit over the flattened field
        // values: the base delegation arguments (re-evaluated in each
        // derived ctor — hence the repeated "root" constant), then the
        // own properties. No base ctor is called.
        let expected = "\
Module
  class Root vtable=3 itables=0
  class Base vtable=3 itables=0
  class Point vtable=3 itables=0
  fun main @scoop_main() -> Unit
    val p: Point
      Call @scoop.ctor.Point direct
        IntLiteral 1
  fun ctor.Root @scoop.ctor.Root(label: String) -> Root
    return
      ClassInit Root
        Local label
  fun ctor.Base @scoop.ctor.Base(name: String) -> Base
    return
      ClassInit Base
        StringConst @scoop.str.0
        Local name
  fun ctor.Point @scoop.ctor.Point(x: Int) -> Point
    return
      ClassInit Point
        StringConst @scoop.str.2
        StringConst @scoop.str.1
        Local x
  str @scoop.str.0 \"root\"
  str @scoop.str.1 \"point\"
  str @scoop.str.2 \"root\"
  entry @scoop_main
";
        assert_eq!(mir::dump(&module), expected);
    }

    #[test]
    fn abstract_classes_get_no_constructor() {
        let mut h = Harness::new();
        let _base = h.class("Base", hir::ClassModifier::Abstract, &[], None, &[]);
        let main = empty_main(&mut h);
        let module = lower(&h.finish(main));

        assert!(
            !module
                .functions
                .iter()
                .any(|(_, f)| f.symbol.starts_with("scoop.ctor."))
        );
    }

    #[test]
    fn field_assignment_lowers_to_field_set() {
        // `p.y = 3` on a class with two properties (index 1 in the
        // flattened layout).
        let mut h = Harness::new();
        let int = h.int;
        let c = h.class(
            "C",
            hir::ClassModifier::Final,
            &[("x", int), ("y", int)],
            None,
            &[],
        );
        let c_ty = h.class_ty(c);
        let mut locals = Arena::new();
        let p = locals.alloc(local("p", c_ty));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![stmt(hir::StatementKind::Assign {
                    target: hir::AssignTarget::Field {
                        receiver: Box::new(local_ref(p, c_ty)),
                        field: hir::FieldRef::ClassField {
                            class_id: c,
                            index: 1,
                        },
                    },
                    value: int_lit(&h, 3),
                })],
            },
        );
        let module = lower(&h.finish(main));

        let body = &module.functions[module.entry].body;
        let mir::StatementKind::FieldSet {
            object,
            index: 1,
            value,
        } = &body.statements[0].kind
        else {
            panic!("a class property assignment must lower to FieldSet")
        };
        assert!(matches!(object, mir::Expr::Local(_)));
        assert!(matches!(value, mir::Expr::IntLiteral(3)));
    }

    #[test]
    fn boxed_interfaces_come_from_the_declaration() {
        // `struct S(val x: Int) : Describable` boxed to `Any` — the
        // boxed itable covers the declared interface even though the
        // box target is not the interface.
        let mut h = Harness::new();
        let int = h.int;
        let iface = h.interface("Describable", &["describe"]);
        let s = h.strukt_with("S", &[("x", int)], &[iface]);
        let s_ty = h.types.alloc(hir::Type::Struct(s));
        let _describe = empty_method(&mut h, "S", "describe", s_ty);
        let any = h.any();
        let mut locals = Arena::new();
        let a = locals.alloc(local("a", any));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![val_decl(
                    a,
                    expr(
                        hir::ExprKind::Box(Box::new(struct_init(s, s_ty, vec![int_lit(&h, 1)]))),
                        any,
                    ),
                )],
            },
        );
        let module = lower(&h.finish(main));

        let boxed = &module.classes[class_index(0)];
        assert_eq!(boxed.interfaces.len(), 1);
        assert_eq!(boxed.itables.len(), 1);
        let record = &boxed.itables[0];
        assert_eq!(record.interface, boxed.interfaces[0]);
        assert_eq!(
            slot_fn(&module, &record.slots[0]),
            "scoop.thunk.S.Describable.describe"
        );
    }

    #[test]
    fn ref_equality_maps_to_a_primitive_pointer_comparison() {
        // `===` / `!==` are reference identity: the primitive
        // comparison on the two pointers.
        let mut h = Harness::new();
        let boolean = h.boolean;
        let c = h.class("C", hir::ClassModifier::Final, &[], None, &[]);
        let c_ty = h.class_ty(c);
        let mut locals = Arena::new();
        let x = locals.alloc(local("x", c_ty));
        let y = locals.alloc(local("y", c_ty));
        let same = locals.alloc(local("same", boolean));
        let other = locals.alloc(local("other", boolean));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![
                    val_decl(
                        same,
                        binary(
                            hir::BinOp::RefEq,
                            local_ref(x, c_ty),
                            local_ref(y, c_ty),
                            boolean,
                        ),
                    ),
                    val_decl(
                        other,
                        binary(
                            hir::BinOp::RefNe,
                            local_ref(x, c_ty),
                            local_ref(y, c_ty),
                            boolean,
                        ),
                    ),
                ],
            },
        );
        let module = lower(&h.finish(main));

        let body = &module.functions[module.entry].body;
        let op_of = |index: usize| {
            let mir::StatementKind::ValDecl {
                init: mir::Expr::Binary { op, .. },
                ..
            } = &body.statements[index].kind
            else {
                panic!("expected a binary expression")
            };
            *op
        };
        assert_eq!(op_of(0), mir::BinOp::IntEq);
        assert_eq!(op_of(1), mir::BinOp::IntNe);
    }

    #[test]
    fn abstract_methods_lower_to_trap_stubs() {
        // `abstract class Base { abstract fun id(): Int }` — hir-lower
        // materializes the abstract method as a params-only bodiless
        // function (`Base.id`, no statements).
        let mut h = Harness::new();
        let int = h.int;
        let base = h.class("Base", hir::ClassModifier::Abstract, &[], None, &[]);
        let base_ty = h.class_ty(base);
        let mut locals = Arena::new();
        let this = locals.alloc(local("this", base_ty));
        let _id = h.method_fn(
            "Base.id",
            base_ty,
            vec![param("this", base_ty, this)],
            int,
            hir::Body {
                locals,
                statements: Vec::new(),
            },
        );
        let main = empty_main(&mut h);
        let module = lower(&h.finish(main));

        // The abstract method is emitted (the abstract class's vtable
        // slot references it) and traps like a pure-virtual stub.
        let base_def = &module.classes[class_index(0)];
        assert_eq!(slot_fn(&module, &base_def.vtable[3]), "scoop.Base.id");
        let stub = module
            .functions
            .iter()
            .map(|(_, f)| f)
            .find(|f| f.symbol == "scoop.Base.id")
            .expect("the abstract method is emitted");
        assert!(
            module
                .top_level
                .iter()
                .any(|&id| module.functions[id].symbol == "scoop.Base.id")
        );
        let mir::StatementKind::Expr(mir::Expr::Call(call)) = &stub.body.statements[0].kind else {
            panic!("the abstract stub is a single trap call")
        };
        assert_eq!(
            call.target.callee,
            mir::Callee::Runtime(mir::RuntimeFn::Trap)
        );
    }

    #[test]
    fn interface_implementations_resolve_qualified_method_names() {
        // `class Doc(val title: String) : Describable { override fun
        // describe() }` — hir-lower names the member `Doc.describe`;
        // the itable / vtable resolve it by its short name.
        let mut h = Harness::new();
        let string = h.string;
        let iface = h.interface("Describable", &["describe"]);
        let doc = h.class(
            "Doc",
            hir::ClassModifier::Final,
            &[("title", string)],
            None,
            &[iface],
        );
        let doc_ty = h.class_ty(doc);
        let _describe = empty_method(&mut h, "Doc", "describe", doc_ty);
        let main = empty_main(&mut h);
        let module = lower(&h.finish(main));

        let doc_def = &module.classes[class_index(0)];
        assert_eq!(doc_def.itables.len(), 1);
        assert_eq!(
            slot_fn(&module, &doc_def.itables[0].slots[0]),
            "scoop.Doc.describe"
        );
        // The implementing method is a vtable method too.
        assert_eq!(slot_fn(&module, &doc_def.vtable[3]), "scoop.Doc.describe");
    }

    #[test]
    fn smart_cast_unboxes_bind_typed_hidden_locals() {
        // `if (a is S) { println(a.v) }` — the narrowed read arrives as
        // `FieldAccess { receiver: Unbox(Local a) }` (hir-lower's smart
        // cast). The unbox must be bound to a typed hidden local so LIR
        // never has to reconstruct its type from the `Any` operand.
        let mut h = Harness::new();
        let println_int = h.println_int();
        let (int, boolean) = (h.int, h.boolean);
        let s = h.strukt("S", &[("v", int)]);
        let s_ty = h.types.alloc(hir::Type::Struct(s));
        let any = h.any();
        let mut locals = Arena::new();
        let a = locals.alloc(local("a", any));
        let print_call = expr(
            hir::ExprKind::Call {
                function: println_int,
                type_args: Vec::new(),
                args: vec![expr(
                    hir::ExprKind::FieldAccess {
                        receiver: Box::new(expr(
                            hir::ExprKind::Unbox(Box::new(local_ref(a, any))),
                            s_ty,
                        )),
                        field: hir::FieldRef::StructField {
                            struct_id: s,
                            index: 0,
                        },
                    },
                    int,
                )],
            },
            h.unit,
        );
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![stmt(hir::StatementKind::If {
                    cond: expr(
                        hir::ExprKind::IsInstance {
                            operand: Box::new(local_ref(a, any)),
                            check_ty: s_ty,
                        },
                        boolean,
                    ),
                    then_body: vec![expr_stmt(print_call)],
                    else_body: None,
                })],
            },
        );
        let module = lower(&h.finish(main));

        let body = &module.functions[module.entry].body;
        let mir::StatementKind::If { then_body, .. } = &body.statements[0].kind else {
            panic!("expected an if statement")
        };
        let mir::StatementKind::ValDecl {
            local: ub,
            init: mir::Expr::Unbox(_),
        } = &then_body[0].kind
        else {
            panic!("the unbox must be bound to a typed hidden local")
        };
        assert_eq!(
            body.locals[*ub].ty,
            mir::Type::Struct(la_arena::Idx::from_raw(0.into()))
        );
        let mir::StatementKind::Expr(mir::Expr::Call(call)) = &then_body[1].kind else {
            panic!("expected the println call")
        };
        assert!(
            matches!(&call.args[0], mir::Expr::FieldAccess { receiver, .. } if matches!(receiver.as_ref(), mir::Expr::Local(local) if local == ub))
        );
    }
}
