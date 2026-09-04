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
//! methods never collide. A direct `super` call carries a distinct HIR proof
//! and always becomes `CallKind::Direct`. Every value type that reaches `Any` / an
//! interface (`Box`, `is`, `as`) gets a boxed `ClassDef` (`box$<ty>`):
//! its vtable contains only ordinary virtual methods, and its itable
//! slots point at adjust thunks that either unbox `this` for a value method or
//! retype the box for an interface default body. The boxed itables cover the value
//! type's *declared* interfaces (spec 4.4.3) no matter what it was
//! boxed to. `as` throws `ClassCastException` on failure (M8); `as?`
//! wraps in `Option` like `!!` does. Reference identity is expressed
//! only by `===` / `!==` (`RefEq` / `RefNe`) and maps to a primitive
//! pointer comparison. M19 class construction evaluates all arguments, emits
//! one exact `ClassAlloc`, and direct-calls a Unit initializer with the new
//! object as its hidden first parameter. Base and `this` initializer edges
//! call another typed initializer on that same object. Struct constructors
//! are separate value-returning hidden callables.

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
mod globals;
mod initialization;
mod instances;
mod lowering_support;
mod members;
mod nominals;
mod pipeline;
mod singletons;
mod structured;
mod symbols;
mod types;

use globals::*;
use lowering_support::*;
use symbols::*;

use coroutine_registry::{CoroutineRegistry, SuspendSource};
use instances::{InstanceRegistry, function_instance};
use structured as smir;
use types::{
    BoxedRegistry, EnumRegistry, InterfaceRegistry, StructRegistry, Types, is_boxable,
    is_reference_mir, mir_type_gc_free,
};

/// Lower HIR to MIR.
pub fn lower(module: &hir::Module) -> mir::Module {
    Lowerer {
        functions: Arena::new(),
        extern_functions: Arena::new(),
        extern_map: HashMap::new(),
        globals: Arena::new(),
        global_map: HashMap::new(),
        initialization_units: Arena::new(),
        initialization_failure_roots: Arena::new(),
        objects: Arena::new(),
        object_types: Arena::new(),
        singleton_values: Arena::new(),
        singleton_published_roots: Arena::new(),
        singleton_root_map: HashMap::new(),
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
        struct_ctors: HashMap::new(),
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
    initialization_units: Arena<mir::InitializationUnit>,
    initialization_failure_roots: Arena<mir::InitializationFailureRoot>,
    objects: Arena<mir::ObjectDef>,
    object_types: Arena<mir::ObjectType>,
    singleton_values: Arena<mir::SingletonValue>,
    singleton_published_roots: Arena<mir::SingletonPublishedRoot>,
    singleton_root_map: HashMap<hir::SingletonPublishedRootId, mir::SingletonPublishedRootId>,
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
    /// Local-concrete value constructor -> MIR hidden callable.
    struct_ctors: HashMap<hir::StructConstructorId, mir::FunctionId>,
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

mod body;

use body::BodyLowerer;

#[cfg(test)]
mod tests;
