//! MIR stage: concrete-HIR lowering, name mangling, call-kind annotation,
//! vtable/itable construction, suspend-to-state-machine lowering.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.3 and
//! `docs/milestone4/DESIGN.md` section 3.3.
//!
//! M2: value types. HIR types are mapped onto MIR types (the struct
//! arena is transposed in declaration order, field types recursively);
//! String `+` becomes `scoop_rt_string_concat`. Equality has already
//! been resolved in HIR to an ordinary explicit or generated method;
//! MIR lowers that exact call and never reconstructs equality semantics.
//! Since M10, a private structured construction tree is normalized into
//! public MIR basic blocks: calls become explicit effects and `&&` / `||`
//! become branch edges before this stage returns. This stage never fails:
//! all errors were already reported by hir-lower.
//!
//! M3: generic templates are monomorphized by HIR lowering. MIR receives
//! only local-concrete functions and records their source type arguments
//! for metadata and stable symbol mangling; no generic template or
//! unresolved type parameter can enter this stage.
//!
//! M4: enums and pattern matching. Every concrete enum application has
//! already acquired its own HIR identity and complete GC classification;
//! MIR transposes it one-to-one. `when` becomes a structured decision sequence
//! (the subject is evaluated once into a hidden local; each arm is a
//! tag comparison, then the field bindings, then the guard nested so a
//! failed guard falls through to the next arm). The HIR Option nodes
//! (`SomeWrap` / `NoneLiteral` / `IsSome` / `Unwrap`) become generic
//! enum operations; a trapping `Unwrap` (`!!`) becomes an if/else whose
//! else branch throws `UnwrapException` (M8).
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
//! a plain call to the generated constructor function (M6). LocalConcrete
//! HIR supplies the complete typed exception/constructor identities; MIR
//! performs no class-arena name lookup or missing-core fallback.
//! `RuntimeFn::Trap` keeps exactly one generation path: the
//! abstract-method stub (a cannot-happen pure-virtual trap).
//!
//! M7/M14: print/println and primitive formatting/equality are ordinary core
//! functions. Their representation-level helpers are ordinary Scoop-ABI
//! extern declarations, so no formatting/equality runtime kind exists in the
//! compiler. Mangling is overload-aware: a name shared by
//! several plainly-mangled functions gets the parameter encoding
//! appended (`scoop.show.I`, `scoop.println.S`; the receiver is not
//! part of a method's overload signature), while unique names keep the
//! plain `scoop.<name>` form and instances keep `$` (`scoop.show$I`),
//! so overload and instance symbols never collide. Dispatch is keyed
//! by signature the same way: vtable / itable slots and call-kind
//! annotation use `name(<param encoding>)`, so each overload gets its
//! own slot and an override replaces the base slot with the matching
//! signature in place. `toString`, hashing, and equality are ordinary
//! Scoop declarations; this stage has no capability-specific channels.
//!
//! M5: arrays (docs/milestone5/DESIGN.md). `Array<T>` /
//! `MutableArray<T>` map onto the corresponding MIR types, and the
//! array nodes translate one-to-one: literals, subscript reads, `size`,
//! `m[i] = v` (an `ArraySet` statement), and constructor / method
//! conversions between the two kinds (`ArrayClone`). Whether an array
//! supports equality is determined solely by ordinary method resolution.
//!
//! M6: reference types (docs/milestone6/DESIGN.md). Classes land as
//! `mir::ClassDef` with the object layout flattened (base-class
//! fields first, then the constructor properties — the same indexing
//! HIR's `ClassField` uses) and the dispatch layout fixed (impl spec
//! 2.9): a root class starts with an empty vtable, and a derived vtable
//! starts from the base's (ordinary overrides replace the base slot in place,
//! new methods append in declaration order), and every implemented
//! interface gets an itable record whose slots follow the interface's
//! method declaration order. Method calls are annotated by the
//! receiver's static type: class receiver → `Virtual`, interface
//! receiver → `Interface`, value type → `Direct`; member functions
//! are mangled qualified (`scoop.Point.describe`) so same-named
//! methods never collide. Every value type that reaches `Any` / an
//! interface (`Box`, `is`, `as`) gets a boxed `ClassDef` (`box$<ty>`):
//! its vtable contains only ordinary virtual methods, and its itable
//! slots point at adjust thunks that unbox `this` and
//! tail-call the real value method. The boxed itables cover the value
//! type's *declared* interfaces (spec 4.4.3) no matter what it was
//! boxed to. `as` throws `ClassCastException` on failure (M8); `as?`
//! wraps in `Option` like `!!` does. Reference identity is expressed
//! only by `===` / `!==` (`RefEq` / `RefNe`) and maps to a primitive
//! pointer comparison. Class construction is function-ized: every
//! non-abstract class gets a `scoop.ctor.<Class>` function whose
//! parameters are the constructor properties and whose body returns a
//! raw `smir::ExprKind::ClassInit` over the flattened field values (the
//! base delegation arguments are evaluated in the ctor context —
//! hir-lower M6 lowers them in an empty scope — and expanded
//! recursively down the base chain; base ctors are never called, so
//! the object identity is a single allocation). A
//! `hir::ExprKind::ClassInit` at a use site becomes a plain `Direct`
//! call to that function.

use std::collections::{HashMap, HashSet};

use la_arena::Arena;
use scoop_ast::Span;
use scoop_hir::concrete as hir;
use scoop_mir as mir;

mod cfg;
mod closures;
mod coroutine;
mod coroutine_registry;
mod dispatch;
mod instances;
mod members;
mod nominals;
mod pipeline;
mod structured;

use coroutine_registry::{CoroutineRegistry, SuspendSource};
use instances::{InstanceRegistry, function_instance};
use structured as smir;

/// Lower HIR to MIR.
pub fn lower(module: &hir::Module) -> mir::Module {
    Lowerer {
        functions: Arena::new(),
        extern_functions: Arena::new(),
        extern_map: HashMap::new(),
        globals: Arena::new(),
        global_map: HashMap::new(),
        callback_bridges: Arena::new(),
        callback_by_target: HashMap::new(),
        foreign_callback_adapters: Arena::new(),
        foreign_callback_bridges: Arena::new(),
        foreign_callback_by_registration: HashMap::new(),
        top_level: Vec::new(),
        strings: Arena::new(),
        structs: StructRegistry::default(),
        struct_map: HashMap::new(),
        classes: Arena::new(),
        class_map: HashMap::new(),
        interfaces: InterfaceRegistry::default(),
        method_slots: HashMap::new(),
        function_map: HashMap::new(),
        instances: InstanceRegistry::default(),
        enums: EnumRegistry::default(),
        boxed: BoxedRegistry::default(),
        ctors: HashMap::new(),
        shell: mangling_shell(&Arena::new(), &Arena::new(), &Arena::new(), &Arena::new()),
        overloaded: overloaded_names(module),
        option_variants: (0, 0),
        coroutines: CoroutineRegistry::default(),
        suspend_sources: Vec::new(),
        closure_classes: Arena::new(),
        closure_invokes: Arena::new(),
        lambda_closures: HashMap::new(),
        anonymous_closures: HashMap::new(),
        reference_closures: HashMap::new(),
        closure_by_function: HashMap::new(),
        closure_capture_indices: HashMap::new(),
        closure_adapters: Arena::new(),
        closure_adapter_by_types: HashMap::new(),
        dynamic_closure_adapters: Arena::new(),
        dynamic_adapter_by_target: HashMap::new(),
        function_bridge_targets: Vec::new(),
        finalized_function_bridges: HashSet::new(),
    }
    .run(module)
}

struct Lowerer {
    functions: Arena<mir::Function>,
    extern_functions: Arena<mir::ExternFunction>,
    extern_map: HashMap<hir::ExternFunctionId, mir::ExternFunctionId>,
    globals: Arena<mir::Global>,
    global_map: HashMap<hir::GlobalId, mir::GlobalId>,
    callback_bridges: Arena<mir::CallbackBridge>,
    callback_by_target: HashMap<(mir::FunctionId, mir::FunctionTypeId), mir::CallbackBridgeId>,
    foreign_callback_adapters: Arena<mir::ForeignCallbackAdapter>,
    foreign_callback_bridges: Arena<mir::ForeignCallbackBridge>,
    foreign_callback_by_registration:
        HashMap<hir::ForeignCallbackRegistrationId, mir::ForeignCallbackBridgeId>,
    /// User functions in declaration order (intrinsics have no MIR body).
    top_level: Vec<mir::FunctionId>,
    strings: Arena<mir::StringConst>,
    /// MIR struct definitions transposed from local-concrete HIR.
    structs: StructRegistry,
    /// Local-concrete HIR struct -> MIR struct.
    struct_map: HashMap<hir::StructId, mir::StructId>,
    classes: Arena<mir::ClassDef>,
    /// HIR class -> MIR class (arena transposed in declaration order).
    class_map: HashMap<hir::ClassId, mir::ClassId>,
    /// Concrete MIR interface applications, created on demand from their
    /// already-specialized HIR definitions.
    interfaces: InterfaceRegistry,
    /// Typed HIR virtual-family identity -> vtable slot, per class.
    method_slots: HashMap<mir::ClassId, HashMap<hir::VirtualMethodId, u32>>,
    /// Local-concrete HIR user function -> MIR function.
    function_map: HashMap<hir::FunctionId, mir::FunctionId>,
    instances: InstanceRegistry,
    enums: EnumRegistry,
    /// Boxed value types discovered while lowering bodies.
    boxed: BoxedRegistry,
    /// Local-concrete hidden constructor callable -> MIR function.
    ctors: HashMap<hir::ClassConstructorId, mir::FunctionId>,
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
    coroutines: CoroutineRegistry,
    suspend_sources: Vec<SuspendSource>,
    closure_classes: Arena<mir::ClosureClass>,
    closure_invokes: Arena<mir::ClosureInvokeFunction>,
    lambda_closures: HashMap<hir::LambdaId, mir::ClosureClassId>,
    anonymous_closures: HashMap<hir::AnonymousFunctionId, mir::ClosureClassId>,
    reference_closures: HashMap<hir::CallableReferenceId, mir::ClosureClassId>,
    /// Generated HIR invoke body -> its concrete closure class.
    closure_by_function: HashMap<hir::FunctionId, mir::ClosureClassId>,
    /// HIR lexical binding -> concrete inline field index for one closure.
    closure_capture_indices: HashMap<(mir::ClosureClassId, hir::BindingId), u32>,
    closure_adapters: Arena<mir::ClosureAdapter>,
    closure_adapter_by_types:
        HashMap<(mir::FunctionTypeId, mir::FunctionTypeId), mir::ClosureAdapterId>,
    dynamic_closure_adapters: Arena<mir::DynamicClosureAdapter>,
    dynamic_adapter_by_target: HashMap<mir::FunctionTypeId, mir::DynamicClosureAdapterId>,
    function_bridge_targets: Vec<mir::FunctionTypeId>,
    finalized_function_bridges: HashSet<(mir::ClosureClassId, mir::FunctionTypeId)>,
}

/// A function's MIR name: hir-lower already qualifies member
/// functions (`Owner.method`), so the name is used as-is and
/// same-named methods of different types never share a mangled
/// symbol. Same-named *overloads* share this name; their symbols are
/// distinguished by the parameter encoding (`Lowerer::declare_symbol`).
fn fn_name(function: &hir::Function) -> String {
    // Extension receivers are structurally the first immutable HIR parameter
    // named `this`, while real members also carry `Method` metadata. Source
    // syntax cannot declare an ordinary parameter named `this`, so this is an
    // unambiguous discriminator. Keep extension symbols in a private namespace:
    // `fun f(x: Int)` and `fun Int.f()` otherwise have the same ABI parameter
    // shape and would collide despite belonging to different source layers.
    if function.method.is_none()
        && function
            .params
            .first()
            .is_some_and(|parameter| parameter.name == "this")
    {
        format!("$extension.{}", function.name)
    } else {
        function.name.clone()
    }
}

fn lower_gc_effect(effect: hir::GcEffect) -> mir::GcEffect {
    match effect {
        hir::GcEffect::Managed => mir::GcEffect::Managed,
        hir::GcEffect::NoGc => mir::GcEffect::NoGc,
    }
}

/// The names shared by more than one plainly-mangled function (M7
/// overloads), over the whole module including scoop.core. Only
/// functions that get a plain `scoop.<name>` symbol count: `User` functions
/// with no source type arguments (free functions, class members, interface
/// method shells). Intrinsics have no MIR symbol; instantiated functions use
/// `$`-mangled symbols, which cannot collide with the
/// overload encoding (`.`).
fn overloaded_names(module: &hir::Module) -> HashSet<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for (_, function) in module.functions.iter() {
        if !matches!(function.kind, hir::FunctionKind::User(_))
            || function_instance(module, function).is_some()
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

/// Whether a function is an abstract class method. HIR carries this
/// explicitly, including for `Unit`-returning methods.
fn is_abstract_bodiless(function: &hir::Function) -> bool {
    function
        .method
        .is_some_and(|method| method.modifier == hir::MethodModifier::Abstract)
}

/// Class ids (HIR) ordered base-before-derived (single inheritance:
/// depth in the base chain; ties keep declaration order).
fn topo_class_order(module: &hir::Module) -> Vec<hir::ClassId> {
    fn depth(module: &hir::Module, id: hir::ClassId) -> usize {
        match module.classes[id].base_class() {
            Some((base, _)) => depth(module, *base) + 1,
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
    own: Vec<smir::Expr>,
) -> Vec<smir::Expr> {
    let mut out = match module.classes[hir_id].base_class() {
        Some((base, delegation)) => {
            let base_own: Vec<smir::Expr> = delegation
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
    enums: &Arena<mir::EnumDef>,
    classes: &Arena<mir::ClassDef>,
    interfaces: &Arena<mir::InterfaceDef>,
) -> mir::Module {
    let mut shell_structs = Arena::new();
    for (_, def) in structs.iter() {
        let representation = match &def.representation {
            mir::StructRepresentation::Declared {
                c_layout,
                interior_mutable,
                ..
            } => mir::StructRepresentation::Declared {
                c_layout: *c_layout,
                interior_mutable: *interior_mutable,
                fields: Vec::new(),
            },
            mir::StructRepresentation::Intrinsic(representation) => {
                mir::StructRepresentation::Intrinsic(representation.clone())
            }
        };
        shell_structs.alloc(mir::StructDef {
            name: def.name.clone(),
            gc_free: def.gc_free,
            representation,
        });
    }
    let mut shell_enums = Arena::new();
    for (_, def) in enums.iter() {
        shell_enums.alloc(mir::EnumDef {
            name: def.name.clone(),
            gc_free: def.gc_free,
            variants: Vec::new(),
        });
    }
    let mut shell_classes = Arena::new();
    for (_, def) in classes.iter() {
        let representation = match &def.representation {
            mir::ClassRepresentation::Declared { .. } => mir::ClassRepresentation::Declared {
                fields: Vec::new(),
                base_class: None,
            },
            mir::ClassRepresentation::Intrinsic(representation) => {
                mir::ClassRepresentation::Intrinsic(representation.clone())
            }
        };
        shell_classes.alloc(mir::ClassDef {
            modifier: mir::ClassModifier::Final,
            name: def.name.clone(),
            representation,
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
        gc_effect: mir::GcEffect::Managed,
        name: String::new(),
        symbol: String::new(),
        params: Vec::new(),
        return_ty: mir::Type::Unit,
        body: mir::Body::unreachable(Arena::new()),
    });
    mir::Module {
        functions,
        extern_functions: Arena::new(),
        globals: Arena::new(),
        callback_bridges: Arena::new(),
        foreign_callback_adapters: Arena::new(),
        foreign_callback_bridges: Arena::new(),
        function_types: Arena::new(),
        closure_classes: Arena::new(),
        closure_invoke_functions: Arena::new(),
        top_level: Vec::new(),
        strings: Arena::new(),
        structs: shell_structs,
        enums: shell_enums,
        classes: shell_classes,
        interfaces: shell_interfaces,
        entry,
        meta: mir::MirMeta::default(),
    }
}

/// The declaration indices of `Option`'s `Some` / `None` variants.
/// hir-lower guarantees scoop.core defines a suitable `Option`.
fn option_variants(module: &hir::Module) -> (u32, u32) {
    let (some, none) = module.option_variants;
    (some.into_raw(), none.into_raw())
}

fn lower_global_constant(
    value: &hir::ConstantValue,
    ty: &mir::Type,
    structs: &Arena<mir::StructDef>,
) -> mir::ConstantValue {
    match (value, ty) {
        (hir::ConstantValue::Int(value), mir::Type::Int | mir::Type::UInt) => {
            mir::ConstantValue::Int(*value)
        }
        (hir::ConstantValue::Bool(value), mir::Type::Boolean) => mir::ConstantValue::Bool(*value),
        (hir::ConstantValue::NullPtr, mir::Type::Ptr(_)) => mir::ConstantValue::NullPtr,
        (hir::ConstantValue::NullFunPtr, mir::Type::FunPtr(_)) => mir::ConstantValue::NullFunPtr,
        (hir::ConstantValue::Struct { fields, .. }, mir::Type::Struct(struct_id)) => {
            let definition = &structs[*struct_id];
            let definition_fields = definition.declared_fields();
            assert_eq!(
                fields.len(),
                definition_fields.len(),
                "typed global struct constants preserve field arity"
            );
            mir::ConstantValue::Struct {
                struct_id: *struct_id,
                fields: fields
                    .iter()
                    .zip(definition_fields)
                    .map(|(field, definition)| {
                        lower_global_constant(field, &definition.ty, structs)
                    })
                    .collect(),
            }
        }
        _ => unreachable!("HIR global constants match their declared type"),
    }
}

fn remap_idx<S, T>(id: la_arena::Idx<S>) -> la_arena::Idx<T> {
    la_arena::Idx::from_raw(id.into_raw())
}

/// Concrete interface applications. The MIR identity includes every type
/// argument because it is also the runtime TypeDescriptor / itable lookup key.
#[derive(Default)]
struct InterfaceRegistry {
    defs: Arena<mir::InterfaceDef>,
    instances: HashMap<mir::InterfaceId, (hir::InterfaceId, Vec<mir::Type>)>,
    by_hir: HashMap<hir::InterfaceId, mir::InterfaceId>,
}

impl InterfaceRegistry {
    fn get_or_create(
        &mut self,
        module: &hir::Module,
        shell: &mut mir::Module,
        hir_id: hir::InterfaceId,
        args: Vec<mir::Type>,
    ) -> mir::InterfaceId {
        if let Some(&id) = self.by_hir.get(&hir_id) {
            return id;
        }
        let decl = &module.interfaces[hir_id];
        let name = decl.name.clone();
        let id = self.defs.alloc(mir::InterfaceDef {
            name: name.clone(),
            methods: Vec::new(),
        });
        shell.interfaces.alloc(mir::InterfaceDef {
            name: name.clone(),
            methods: Vec::new(),
        });
        self.instances.insert(id, (hir_id, args));
        self.by_hir.insert(hir_id, id);
        id
    }

    fn source(&self, id: mir::InterfaceId) -> (hir::InterfaceId, &[mir::Type]) {
        let (hir, args) = &self.instances[&id];
        (*hir, args)
    }

    fn mir_id(&self, id: hir::InterfaceId) -> mir::InterfaceId {
        self.by_hir[&id]
    }
}

/// Shared type-lowering context: the HIR type arena, the struct /
/// class / interface maps. Local-concrete HIR has no type parameters and no
/// substitution state.
#[derive(Clone, Copy)]
struct Types<'a> {
    module: &'a hir::Module,
    struct_map: &'a HashMap<hir::StructId, mir::StructId>,
    class_map: &'a HashMap<hir::ClassId, mir::ClassId>,
}

impl Types<'_> {
    /// Map a HIR type onto its MIR type. Aggregate shapes are
    /// preserved: structs keep their remapped concrete id, tuples keep their
    /// mapped element types, and concrete enum definitions are transposed on
    /// first reference. Reference types map onto their remapped ids.
    fn lower(
        &self,
        ty: hir::TypeId,
        enums: &mut EnumRegistry,
        structs: &mut StructRegistry,
        interfaces: &mut InterfaceRegistry,
        shell: &mut mir::Module,
    ) -> mir::Type {
        match &self.module.types[ty].kind {
            hir::TypeKind::Unit => mir::Type::Unit,
            hir::TypeKind::Int => mir::Type::Int,
            // UInt shares Int's machine word (M9, spec 11.2); the MIR
            // type stays distinct so checks can tell them apart.
            hir::TypeKind::UInt => mir::Type::UInt,
            hir::TypeKind::Boolean => mir::Type::Boolean,
            hir::TypeKind::String => mir::Type::String,
            hir::TypeKind::Struct(id) => mir::Type::Struct(self.struct_map[id]),
            hir::TypeKind::Class(id) => mir::Type::Class(self.class_map[id]),
            hir::TypeKind::Interface(id) => {
                let args = self.module.interfaces[*id]
                    .type_arguments
                    .iter()
                    .map(|&arg| self.lower(arg, enums, structs, interfaces, shell))
                    .collect();
                mir::Type::Interface(interfaces.get_or_create(self.module, shell, *id, args))
            }
            hir::TypeKind::Any => mir::Type::Any,
            hir::TypeKind::Tuple(elements) => mir::Type::Tuple(
                elements
                    .iter()
                    .map(|&element| self.lower(element, enums, structs, interfaces, shell))
                    .collect(),
            ),
            hir::TypeKind::Function(id) => mir::Type::Function(remap_idx(*id)),
            hir::TypeKind::Ptr(pointee) => mir::Type::Ptr(Box::new(
                self.lower(*pointee, enums, structs, interfaces, shell),
            )),
            hir::TypeKind::FunPtr(id) => mir::Type::FunPtr(remap_idx(*id)),
            hir::TypeKind::Enum(id) => {
                let args = self.module.enums[*id]
                    .type_arguments
                    .iter()
                    .map(|argument| self.lower(*argument, enums, structs, interfaces, shell))
                    .collect::<Vec<_>>();
                let enum_id = enums.get_or_create(self, structs, interfaces, shell, *id);
                mir::Type::Enum(enum_id, args)
            }
        }
    }
}

/// Concrete enum definitions (DESIGN 3.3), transposed once from distinct
/// local-concrete HIR identities (`Option$I`, or a plain non-generic name).
#[derive(Default)]
struct EnumRegistry {
    defs: Arena<mir::EnumDef>,
    /// Local-concrete HIR enum identity -> MIR enum identity.
    by_hir: HashMap<hir::EnumId, mir::EnumId>,
    /// MIR enum -> its local-concrete HIR source (boxed value types read the
    /// declared interfaces from that complete definition).
    hir_ids: HashMap<mir::EnumId, hir::EnumId>,
}

impl EnumRegistry {
    fn get_or_create(
        &mut self,
        types: &Types,
        structs: &mut StructRegistry,
        interfaces: &mut InterfaceRegistry,
        shell: &mut mir::Module,
        hir_id: hir::EnumId,
    ) -> mir::EnumId {
        if let Some(&id) = self.by_hir.get(&hir_id) {
            return id;
        }
        let decl = &types.module.enums[hir_id];
        let name = decl.name.clone();
        let id = self.defs.alloc(mir::EnumDef {
            name: name.clone(),
            gc_free: decl.gc_free,
            variants: Vec::new(),
        });
        // Keep the mangling shell's enum arena in sync (same ids) so
        // `encode_type` can render this instance inside another one.
        shell.enums.alloc(mir::EnumDef {
            name: name.clone(),
            gc_free: decl.gc_free,
            variants: Vec::new(),
        });
        self.by_hir.insert(hir_id, id);
        self.hir_ids.insert(id, hir_id);
        let variants = decl
            .variants
            .iter()
            .map(|variant| mir::VariantDef {
                name: variant.name.clone(),
                gc_free: variant.gc_free,
                fields: variant
                    .fields
                    .iter()
                    .map(|field| mir::Field {
                        name: field.name.clone(),
                        ty: types.lower(field.ty, self, structs, interfaces, shell),
                    })
                    .collect(),
            })
            .collect();
        self.defs[id].variants = variants;
        id
    }
}

/// Concrete struct definitions and their mandatory local-concrete HIR
/// provenance. Structs are transposed eagerly, so every MIR id has exactly one
/// source id before bodies can request boxing or interface lookup.
#[derive(Default)]
struct StructRegistry {
    defs: Arena<mir::StructDef>,
    hir_ids: HashMap<mir::StructId, hir::StructId>,
}

/// Classify a MIR type while constructing compiler-synthesized concrete
/// aggregates. Source aggregates copy this mandatory bit from concrete HIR;
/// synthesized aggregates must derive it atomically with their definition.
fn mir_type_gc_free(ty: &mir::Type, structs: &StructRegistry, enums: &EnumRegistry) -> bool {
    match ty {
        mir::Type::Unit
        | mir::Type::Int
        | mir::Type::UInt
        | mir::Type::Boolean
        | mir::Type::Ptr(_)
        | mir::Type::FunPtr(_) => true,
        mir::Type::Struct(id) => structs.defs[*id].gc_free,
        mir::Type::Enum(id, _) => enums.defs[*id].gc_free,
        mir::Type::Tuple(elements) => elements
            .iter()
            .all(|element| mir_type_gc_free(element, structs, enums)),
        mir::Type::String
        | mir::Type::Class(_)
        | mir::Type::Interface(_)
        | mir::Type::Any
        | mir::Type::Function(_) => false,
    }
}

/// Boxed value types (DESIGN 2.3): one `mir::ClassDef` per boxed
/// value type (`box$<encoded>`), deduplicated by typed payload identity. The
/// vtable / itables are filled by `finalize_boxed` once
/// every body has been lowered (all `Box` / `is` / `as` sites seen).
#[derive(Default)]
struct BoxedRegistry {
    /// Typed payload -> boxed class. The vector is small and avoids making the
    /// emitted link name part of semantic identity.
    by_type: Vec<(mir::Type, mir::ClassId)>,
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
        if let Some((_, id)) = self.by_type.iter().find(|(found, _)| found == payload) {
            return *id;
        }
        let name = format!("box${}", mir::encode_type(shell, payload));
        let id = classes.alloc(mir::ClassDef {
            modifier: mir::ClassModifier::Final,
            name: name.clone(),
            // The object layout is the header plus the inline payload.
            representation: mir::ClassRepresentation::Declared {
                fields: vec![mir::Field {
                    name: "value".to_string(),
                    ty: payload.clone(),
                }],
                base_class: None,
            },
            interfaces: Vec::new(),
            // Filled by `finalize_boxed`.
            vtable: Vec::new(),
            itables: Vec::new(),
        });
        // Keep the mangling shell's class arena in sync (same ids).
        shell.classes.alloc(mir::ClassDef {
            modifier: mir::ClassModifier::Final,
            name: name.clone(),
            representation: mir::ClassRepresentation::Declared {
                fields: Vec::new(),
                base_class: None,
            },
            interfaces: Vec::new(),
            vtable: Vec::new(),
            itables: Vec::new(),
        });
        self.by_type.push((payload.clone(), id));
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
            | mir::Type::UInt
            | mir::Type::Boolean
            | mir::Type::Unit
    )
}

fn is_reference_mir(ty: &mir::Type) -> bool {
    matches!(
        ty,
        mir::Type::String
            | mir::Type::Class(_)
            | mir::Type::Interface(_)
            | mir::Type::Function(_)
            | mir::Type::Any
    )
}

mod body;

use body::BodyLowerer;

#[cfg(test)]
mod tests;
