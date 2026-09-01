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
//! M7: print/println are ordinary core functions
//! (docs/milestone7/DESIGN.md section 2) — their calls go through the
//! normal function path. Validated `@Intrinsic` calls map by typed kind onto
//! the remaining runtime functions: `rt_int_to_string` → `IntToString`,
//! `rt_bool_to_string` →
//! `BoolToString`. These runtime primitives are reached only through
//! ordinary source declarations in `scoop.core`. Mangling is overload-aware: a name shared by
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
mod coroutine;
mod structured;

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
    /// Method signature key (`fn_signature_key`) -> vtable slot, per
    /// class (computed by `compute_dispatch`; `BodyLowerer` reads it
    /// for call-kind annotation).
    method_slots: HashMap<mir::ClassId, HashMap<String, u32>>,
    /// Local-concrete HIR user function -> MIR function.
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

#[derive(Clone)]
struct SuspendSource {
    function: mir::FunctionId,
    source_return: mir::Type,
    instance: Option<mir::MonomorphizedFunctionId>,
}

#[derive(Default)]
struct CoroutineRegistry {
    functions: Arena<mir::CoroutineFunction>,
    steps: Arena<mir::CoroutineStep>,
    steps_by_result: Vec<(mir::Type, mir::CoroutineStepId)>,
    slots: Arena<mir::CoroutineSlot>,
    slots_by_value: Vec<(mir::Type, mir::CoroutineSlotId)>,
    frames: Arena<mir::CoroutineFrame>,
    resume_points: Arena<mir::CoroutineResumePoint>,
    continuation_shells: Vec<(mir::Type, mir::FunctionId, mir::FunctionId)>,
    start_helpers: Vec<(mir::Type, mir::FunctionId)>,
}

impl CoroutineRegistry {
    fn step_for(
        &mut self,
        result: &mir::Type,
        structs: &StructRegistry,
        enums: &mut EnumRegistry,
        shell: &mut mir::Module,
    ) -> (mir::CoroutineStepId, mir::Type) {
        if let Some((_, id)) = self
            .steps_by_result
            .iter()
            .find(|(found, _)| found == result)
        {
            let step = &self.steps[*id];
            return (*id, mir::Type::Enum(step.enum_id, Vec::new()));
        }
        let name = format!("CoroutineStep${}", mir::encode_type(shell, result));
        let result_gc_free = mir_type_gc_free(result, structs, enums);
        let variants = vec![
            mir::VariantDef {
                name: "Completed".to_string(),
                gc_free: result_gc_free,
                fields: vec![mir::Field {
                    name: "value".to_string(),
                    ty: result.clone(),
                }],
            },
            mir::VariantDef {
                name: "Suspended".to_string(),
                gc_free: true,
                fields: Vec::new(),
            },
        ];
        let enum_id = enums.defs.alloc(mir::EnumDef {
            name: name.clone(),
            gc_free: result_gc_free,
            variants,
        });
        let shell_id = shell.enums.alloc(mir::EnumDef {
            name,
            gc_free: result_gc_free,
            variants: Vec::new(),
        });
        assert_eq!(enum_id, shell_id, "the mangling shell mirrors enum ids");
        let id = self.steps.alloc(mir::CoroutineStep {
            enum_id,
            result: result.clone(),
        });
        self.steps_by_result.push((result.clone(), id));
        (id, mir::Type::Enum(enum_id, Vec::new()))
    }

    fn step_type_for(&self, result: &mir::Type) -> Option<mir::Type> {
        self.steps_by_result
            .iter()
            .find(|(found, _)| found == result)
            .map(|(_, id)| mir::Type::Enum(self.steps[*id].enum_id, Vec::new()))
    }

    fn slot_for(
        &mut self,
        value: &mir::Type,
        structs: &StructRegistry,
        enums: &mut EnumRegistry,
        shell: &mut mir::Module,
    ) -> (mir::CoroutineSlotId, mir::Type) {
        if let Some((_, id)) = self.slots_by_value.iter().find(|(found, _)| found == value) {
            let slot = &self.slots[*id];
            return (*id, mir::Type::Enum(slot.enum_id, Vec::new()));
        }
        let name = format!("CoroutineSlot${}", mir::encode_type(shell, value));
        let value_gc_free = mir_type_gc_free(value, structs, enums);
        let variants = vec![
            mir::VariantDef {
                name: "Empty".to_string(),
                gc_free: true,
                fields: Vec::new(),
            },
            mir::VariantDef {
                name: "Value".to_string(),
                gc_free: value_gc_free,
                fields: vec![mir::Field {
                    name: "value".to_string(),
                    ty: value.clone(),
                }],
            },
        ];
        let enum_id = enums.defs.alloc(mir::EnumDef {
            name: name.clone(),
            gc_free: value_gc_free,
            variants,
        });
        let shell_id = shell.enums.alloc(mir::EnumDef {
            name,
            gc_free: value_gc_free,
            variants: Vec::new(),
        });
        assert_eq!(enum_id, shell_id, "the mangling shell mirrors enum ids");
        let id = self.slots.alloc(mir::CoroutineSlot {
            enum_id,
            value: value.clone(),
        });
        self.slots_by_value.push((value.clone(), id));
        (id, mir::Type::Enum(enum_id, Vec::new()))
    }

    fn continuation_shells(
        &mut self,
        result: &mir::Type,
        continuation: mir::InterfaceId,
        throwable: mir::Type,
        functions: &mut Arena<mir::Function>,
        shell: &mir::Module,
    ) -> (mir::FunctionId, mir::FunctionId) {
        if let Some((_, resume, failure)) = self
            .continuation_shells
            .iter()
            .find(|(found, _, _)| found == result)
        {
            return (*resume, *failure);
        }
        let encoded = mir::encode_type(shell, result);
        let mut resume_locals = Arena::new();
        let receiver = resume_locals.alloc(mir::Local {
            name: "this".to_string(),
            ty: mir::Type::Interface(continuation),
            mutable: false,
        });
        let value = resume_locals.alloc(mir::Local {
            name: "value".to_string(),
            ty: result.clone(),
            mutable: false,
        });
        let resume = functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: format!("Continuation.resume${encoded}"),
            symbol: format!("scoop.Continuation.resume${encoded}"),
            params: vec![
                mir::Param {
                    name: "this".to_string(),
                    ty: mir::Type::Interface(continuation),
                    local: receiver,
                },
                mir::Param {
                    name: "value".to_string(),
                    ty: result.clone(),
                    local: value,
                },
            ],
            return_ty: mir::Type::Unit,
            body: mir::Body::unreachable(resume_locals),
        });
        let mut failure_locals = Arena::new();
        let receiver = failure_locals.alloc(mir::Local {
            name: "this".to_string(),
            ty: mir::Type::Interface(continuation),
            mutable: false,
        });
        let exception = failure_locals.alloc(mir::Local {
            name: "exception".to_string(),
            ty: throwable.clone(),
            mutable: false,
        });
        let failure = functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: format!("Continuation.resumeWithException${encoded}"),
            symbol: format!("scoop.Continuation.resumeWithException${encoded}"),
            params: vec![
                mir::Param {
                    name: "this".to_string(),
                    ty: mir::Type::Interface(continuation),
                    local: receiver,
                },
                mir::Param {
                    name: "exception".to_string(),
                    ty: throwable,
                    local: exception,
                },
            ],
            return_ty: mir::Type::Unit,
            body: mir::Body::unreachable(failure_locals),
        });
        self.continuation_shells
            .push((result.clone(), resume, failure));
        (resume, failure)
    }

    #[allow(clippy::too_many_arguments)]
    fn start_helper(
        &mut self,
        result: &mir::Type,
        task_interface: mir::InterfaceId,
        continuation_interface: mir::InterfaceId,
        run: mir::MonomorphizedFunctionId,
        resume: mir::MonomorphizedFunctionId,
        resume_with_exception: mir::MonomorphizedFunctionId,
        step_ty: &mir::Type,
        throwable: mir::Type,
        functions: &mut Arena<mir::Function>,
        top_level: &mut Vec<mir::FunctionId>,
        shell: &mir::Module,
    ) -> mir::FunctionId {
        if let Some((_, function)) = self.start_helpers.iter().find(|(found, _)| found == result) {
            return *function;
        }

        let mut locals = Arena::new();
        let task = locals.alloc(mir::Local {
            name: "task".to_string(),
            ty: mir::Type::Interface(task_interface),
            mutable: false,
        });
        let completion = locals.alloc(mir::Local {
            name: "completion".to_string(),
            ty: mir::Type::Interface(continuation_interface),
            mutable: false,
        });
        let step = locals.alloc(mir::Local {
            name: "$step".to_string(),
            ty: step_ty.clone(),
            mutable: false,
        });
        let exception = locals.alloc(mir::Local {
            name: "$exception".to_string(),
            ty: throwable.clone(),
            mutable: false,
        });
        let span = Span { start: 0, end: 0 };
        let statement = |kind| mir::Statement { kind, span };
        let mut blocks = Arena::new();
        let suspended = blocks.alloc(mir::BasicBlock {
            name: "suspended".to_string(),
            statements: Vec::new(),
            terminator: mir::Terminator::Return { value: None },
            unwind: None,
        });
        let completed = blocks.alloc(mir::BasicBlock {
            name: "completed".to_string(),
            statements: vec![statement(mir::StatementKind::Call(mir::CallEffect::Unit(
                mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Interface {
                            interface: continuation_interface,
                            slot: 0,
                        },
                        callee: mir::Callee::Monomorphized(resume),
                    },
                    args: vec![
                        mir::Expr::local(completion, mir::Type::Interface(continuation_interface)),
                        mir::Expr::new(
                            result.clone(),
                            mir::ExprKind::EnumField {
                                operand: Box::new(mir::Expr::local(step, step_ty.clone())),
                                variant: 0,
                                index: 0,
                            },
                        ),
                    ],
                },
            )))],
            terminator: mir::Terminator::Return { value: None },
            unwind: None,
        });
        let failed = blocks.alloc(mir::BasicBlock {
            name: "failed".to_string(),
            statements: vec![statement(mir::StatementKind::Call(mir::CallEffect::Unit(
                mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Interface {
                            interface: continuation_interface,
                            slot: 1,
                        },
                        callee: mir::Callee::Monomorphized(resume_with_exception),
                    },
                    args: vec![
                        mir::Expr::local(completion, mir::Type::Interface(continuation_interface)),
                        mir::Expr::local(exception, throwable.clone()),
                    ],
                },
            )))],
            terminator: mir::Terminator::Return { value: None },
            unwind: None,
        });
        let catch_pad = blocks.alloc(mir::BasicBlock {
            name: "body_failure".to_string(),
            statements: vec![
                statement(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
                    cleanup: false,
                })),
                statement(mir::StatementKind::Eh(mir::EhStatement::BeginCatch)),
                statement(mir::StatementKind::Call(mir::CallEffect::Value {
                    destination: exception,
                    call: mir::Call {
                        target: mir::CallTarget {
                            kind: mir::CallKind::Direct,
                            callee: mir::Callee::Runtime(mir::RuntimeFn::MaterializeException),
                        },
                        args: vec![mir::Expr::caught_exception()],
                    },
                })),
                statement(mir::StatementKind::Eh(mir::EhStatement::EndCatch)),
            ],
            terminator: mir::Terminator::Goto(failed),
            unwind: None,
        });
        let entry = blocks.alloc(mir::BasicBlock {
            name: "entry".to_string(),
            statements: vec![statement(mir::StatementKind::Call(
                mir::CallEffect::Value {
                    destination: step,
                    call: mir::Call {
                        target: mir::CallTarget {
                            kind: mir::CallKind::Interface {
                                interface: task_interface,
                                slot: 0,
                            },
                            callee: mir::Callee::Monomorphized(run),
                        },
                        args: vec![
                            mir::Expr::local(task, mir::Type::Interface(task_interface)),
                            mir::Expr::local(
                                completion,
                                mir::Type::Interface(continuation_interface),
                            ),
                        ],
                    },
                },
            ))],
            terminator: mir::Terminator::Branch {
                cond: mir::Expr::new(
                    mir::Type::Boolean,
                    mir::ExprKind::Binary {
                        op: mir::BinOp::IntEq,
                        lhs: Box::new(mir::Expr::enum_tag(mir::Expr::local(step, step_ty.clone()))),
                        rhs: Box::new(mir::Expr::int(0)),
                    },
                ),
                then_block: completed,
                else_block: suspended,
            },
            unwind: Some(catch_pad),
        });
        let encoded = mir::encode_type(shell, result);
        let function = functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: format!("startCoroutine${encoded}"),
            symbol: format!("scoop.coroutine.start${encoded}"),
            params: vec![
                mir::Param {
                    name: "task".to_string(),
                    ty: mir::Type::Interface(task_interface),
                    local: task,
                },
                mir::Param {
                    name: "completion".to_string(),
                    ty: mir::Type::Interface(continuation_interface),
                    local: completion,
                },
            ],
            return_ty: mir::Type::Unit,
            body: mir::Body {
                locals,
                blocks,
                entry,
            },
        });
        top_level.push(function);
        self.start_helpers.push((result.clone(), function));
        function
    }
}

impl Lowerer {
    fn run(mut self, module: &hir::Module) -> mir::Module {
        // Struct / class / interface ids first (types can reference
        // any of them regardless of declaration order), then the
        // mangling shell (their names for `encode_type`), then the
        // field types themselves — which can instantiate enums.
        self.lower_structs(module);
        self.declare_classes(module);
        // Concrete interface type arguments may name classes, so every class
        // id must exist before interface applications are transposed.
        self.lower_interfaces(module);
        self.shell = mangling_shell(
            &self.structs.defs,
            &self.enums.defs,
            &self.classes,
            &self.interfaces.defs,
        );
        self.lower_function_types(module);
        self.fill_class_hierarchy(module);
        self.fill_struct_fields(module);
        self.lower_globals(module);
        self.option_variants = option_variants(module);
        // Classes are processed base-before-derived: the object layout
        // and the vtable both keep the base's as a prefix.
        let class_order = topo_class_order(module);
        self.fill_class_fields(module, &class_order);
        self.lower_extern_functions(module);

        // Declare every fully concrete user function first, so calls resolve
        // regardless of declaration order. Intrinsics have no body;
        // their callsites map to `Callee::Runtime` shims (see
        // `BodyLowerer::lower_call`). HIR has already closed and instantiated
        // every generic dependency before this stage starts.
        // Member functions are declared too (hir-lower keeps them out
        // of `top_level`); interface methods become signature-only
        // shells (M6 interfaces have no default implementations).
        let mut user_functions = Vec::new();
        for &hir_id in &module.top_level {
            let function = &module.functions[hir_id];
            if !matches!(function.kind, hir::FunctionKind::User(_)) {
                continue;
            }
            let id = self.declare_function(module, hir_id);
            user_functions.push((hir_id, id));
        }
        for (hir_id, function) in module.functions.iter() {
            let Some(method) = function.method else {
                continue;
            };
            let ty = method.owner;
            // Interface methods are signature-only shells: dispatch goes
            // through the itable, so their declarations only provide the
            // complete indirect-call signature.
            if matches!(module.types[ty].kind, hir::TypeKind::Interface(..)) {
                self.declare_interface_method(module, hir_id);
            }
        }
        // Bound callable-reference invoke bodies preserve virtual/interface
        // dispatch, so closure materialization needs completed slot tables.
        // Dispatch itself only depends on declared methods, not constructors
        // or lowered source bodies.
        self.compute_dispatch(module, &class_order);
        self.declare_closures(module);
        // Constructor functions: one per non-abstract class, declared
        // like ordinary functions so `ClassInit` call sites resolve.
        let mut ctor_functions = Vec::new();
        for (hir_id, decl) in module.classes.iter() {
            if decl.modifier == hir::ClassModifier::Abstract
                || matches!(
                    decl.representation,
                    hir::ClassRepresentation::Intrinsic { .. }
                )
            {
                continue;
            }
            let id = self.declare_ctor(module, hir_id);
            ctor_functions.push((hir_id, id));
        }

        for (hir_id, mir_id) in user_functions {
            let (params, return_ty, body) = self.lower_user_function(module, hir_id);
            let body = cfg::lower(body, return_ty.clone());
            if module.functions[hir_id].is_suspend {
                self.suspend_sources.push(SuspendSource {
                    function: mir_id,
                    source_return: return_ty.clone(),
                    instance: self.instances.get(hir_id),
                });
            }
            let function = &mut self.functions[mir_id];
            function.params = params;
            function.return_ty = return_ty;
            function.body = body;
        }
        for (hir_id, mir_id) in ctor_functions {
            let (params, return_ty, body) = self.lower_ctor(module, hir_id);
            let body = cfg::lower(body, return_ty.clone());
            let function = &mut self.functions[mir_id];
            function.params = params;
            function.return_ty = return_ty;
            function.body = body;
        }
        for (hir_id, function) in module.functions.iter() {
            if function.is_suspend
                && !module.top_level.contains(&hir_id)
                && self.function_map.contains_key(&hir_id)
            {
                let return_ty = self.functions[self.function_map[&hir_id]].return_ty.clone();
                self.suspend_sources.push(SuspendSource {
                    function: self.function_map[&hir_id],
                    source_return: return_ty,
                    instance: self.instances.get(hir_id),
                });
            }
        }

        // Finalize boxed value types to a fixed point. Variance and function
        // bridges can discover additional boxed payloads.
        let mut next_boxed = 0;
        loop {
            while next_boxed < self.boxed.order.len() {
                self.finalize_boxed(module, next_boxed);
                next_boxed += 1;
            }
            let added_variance = self.finalize_variance_itables(module);
            let added_function_bridges = self.finalize_function_bridges(module);
            if next_boxed == self.boxed.order.len() && !added_variance && !added_function_bridges {
                break;
            }
        }

        self.transform_suspend_abis(module);
        coroutine::transform(&mut self, module);

        // The entry point is a non-generic user function, hence always
        // in the map.
        let entry = self.function_map[&module.entry];
        let boxed_types = self
            .boxed
            .by_type
            .into_iter()
            .map(|(payload, class)| mir::BoxedType { payload, class })
            .collect();
        mir::Module {
            functions: self.functions,
            extern_functions: self.extern_functions,
            globals: self.globals,
            callback_bridges: self.callback_bridges,
            foreign_callback_adapters: self.foreign_callback_adapters,
            foreign_callback_bridges: self.foreign_callback_bridges,
            function_types: self.shell.function_types,
            closure_classes: self.closure_classes,
            closure_invoke_functions: self.closure_invokes,
            top_level: self.top_level,
            strings: self.strings,
            structs: self.structs.defs,
            enums: self.enums.defs,
            classes: self.classes,
            interfaces: self.interfaces.defs,
            entry,
            meta: mir::MirMeta {
                instances: self.instances.meta,
                coroutine_functions: self.coroutines.functions,
                coroutine_steps: self.coroutines.steps,
                coroutine_slots: self.coroutines.slots,
                coroutine_frames: self.coroutines.frames,
                coroutine_resume_points: self.coroutines.resume_points,
                closure_adapters: self.closure_adapters,
                dynamic_closure_adapters: self.dynamic_closure_adapters,
                boxed_types,
                ..mir::MirMeta::default()
            },
        }
    }

    fn lower_extern_functions(&mut self, module: &hir::Module) {
        for (hir_id, extern_) in module.extern_functions.iter() {
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let params = extern_
                .params
                .iter()
                .map(|&ty| {
                    types.lower(
                        ty,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    )
                })
                .collect();
            let return_type = types.lower(
                extern_.return_type,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
            let id = self.extern_functions.alloc(mir::ExternFunction {
                source_name: extern_.source_name.clone(),
                native_symbol: extern_.native_symbol.clone(),
                library: extern_.library.clone(),
                abi: match extern_.abi {
                    hir::ExternAbi::C => mir::ExternAbi::C,
                    hir::ExternAbi::Scoop => mir::ExternAbi::Scoop,
                },
                calling_convention: match extern_.calling_convention {
                    hir::CallingConvention::Cdecl => mir::CallingConvention::Cdecl,
                },
                gc_effect: match extern_.gc_effect {
                    hir::GcEffect::Managed => mir::GcEffect::Managed,
                    hir::GcEffect::NoGc => mir::GcEffect::NoGc,
                },
                params,
                return_type,
            });
            self.extern_map.insert(hir_id, id);
        }
    }

    /// Transpose the concrete HIR function-type arena one-to-one. MIR keeps
    /// the same typed identity order, so type lowering never scans signatures
    /// or re-interns an equal shape.
    fn lower_function_types(&mut self, module: &hir::Module) {
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        for (source_id, source) in module.function_types.iter() {
            let parameter_types = source
                .parameter_types
                .iter()
                .map(|parameter| {
                    types.lower(
                        *parameter,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    )
                })
                .collect();
            let return_type = types.lower(
                source.return_type,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
            let target_id = self.shell.function_types.alloc(mir::FunctionType {
                is_suspend: source.is_suspend,
                parameter_types,
                return_type,
            });
            assert_eq!(source_id.into_raw(), target_id.into_raw());
        }
    }

    fn lower_globals(&mut self, module: &hir::Module) {
        for (hir_id, global) in module.globals.iter() {
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let ty = types.lower(
                global.ty,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
            let storage = match &global.storage {
                hir::GlobalStorage::Local {
                    thread_local,
                    initializer,
                } => mir::GlobalStorage::Local {
                    thread_local: *thread_local,
                    initializer: lower_global_constant(initializer, &ty, &self.structs.defs),
                },
                hir::GlobalStorage::Extern {
                    library,
                    native_symbol,
                    thread_local,
                } => mir::GlobalStorage::Extern {
                    library: library.clone(),
                    native_symbol: native_symbol.clone(),
                    thread_local: *thread_local,
                },
            };
            let id = self.globals.alloc(mir::Global {
                name: global.name.clone(),
                symbol: mir::mangle_global(&global.name),
                ty,
                mutable: global.mutable,
                storage,
            });
            self.global_map.insert(hir_id, id);
        }
    }

    fn transform_suspend_abis(&mut self, module: &hir::Module) {
        for source in std::mem::take(&mut self.suspend_sources) {
            let (step, step_ty) = self.coroutines.step_for(
                &source.source_return,
                &self.structs,
                &mut self.enums,
                &mut self.shell,
            );
            let protocol = self.coroutine_protocol(module, &source.source_return);
            let continuation = self.interfaces.mir_id(protocol.continuation);
            let continuation_ty = mir::Type::Interface(continuation);
            let function = &mut self.functions[source.function];
            let completion = function.body.locals.alloc(mir::Local {
                name: "$completion".to_string(),
                ty: continuation_ty.clone(),
                mutable: false,
            });
            function.params.push(mir::Param {
                name: "$completion".to_string(),
                ty: continuation_ty,
                local: completion,
            });
            for (_, block) in function.body.blocks.iter_mut() {
                if let mir::Terminator::Return { value } = &mut block.terminator {
                    let completed = value.take().unwrap_or_else(mir::Expr::unit);
                    *value = Some(mir::Expr::new(
                        step_ty.clone(),
                        mir::ExprKind::VariantConstruct {
                            variant: 0,
                            fields: vec![completed],
                        },
                    ));
                }
            }
            function.return_ty = step_ty;
            function.symbol = mir::mangle_suspend(&function.symbol);
            if let Some(instance) = source.instance {
                self.instances.meta[instance].symbol = function.symbol.clone();
            }
            self.coroutines.functions.alloc(mir::CoroutineFunction {
                function: source.function,
                source_return: source.source_return,
                step,
                lowering: mir::CoroutineLowering::Immediate,
            });
        }
    }

    fn coroutine_protocol(
        &mut self,
        module: &hir::Module,
        result: &mir::Type,
    ) -> hir::CoroutineProtocol {
        for protocol in module.coroutine_protocols.iter().copied() {
            let lowered = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            }
            .lower(
                protocol.result_type,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
            if &lowered == result {
                return protocol;
            }
        }
        let protocols = module
            .coroutine_protocols
            .iter()
            .map(|protocol| format!("{:?}", module.types[protocol.result_type].kind))
            .collect::<Vec<_>>();
        panic!(
            "local-concrete HIR provides a protocol for every suspend result type; missing {result:?}, available {protocols:?}"
        )
    }

    fn finalize_variance_itables(&mut self, module: &hir::Module) -> bool {
        let targets: Vec<mir::InterfaceId> =
            self.interfaces.defs.iter().map(|(id, _)| id).collect();
        let classes: Vec<mir::ClassId> = self.classes.iter().map(|(id, _)| id).collect();
        let mut added = false;
        for class in classes {
            let sources: Vec<(mir::InterfaceId, Vec<mir::TableSlot>)> = self.classes[class]
                .itables
                .iter()
                .map(|record| (record.interface, clone_slots(&record.slots)))
                .collect();
            for &target in &targets {
                if self.classes[class]
                    .itables
                    .iter()
                    .any(|record| record.interface == target)
                {
                    continue;
                }
                let Some((source, slots)) = sources
                    .iter()
                    .find(|(source, _)| self.interface_is_subtype(module, *source, target))
                else {
                    continue;
                };
                let (source_id, _) = self.interfaces.source(*source);
                let method_indices: Vec<_> = module.interfaces[source_id]
                    .methods
                    .iter()
                    .enumerate()
                    .map(|(index, _)| index)
                    .collect();
                let bridge_slots = slots
                    .iter()
                    .enumerate()
                    .map(|(slot_index, slot)| {
                        mir::TableSlot::Function(self.build_variance_bridge(
                            module,
                            class,
                            *source,
                            target,
                            method_indices[slot_index],
                            slot,
                        ))
                    })
                    .collect();
                self.classes[class].interfaces.push(target);
                self.classes[class].itables.push(mir::ItableRecord {
                    interface: target,
                    slots: bridge_slots,
                });
                added = true;
            }
        }
        added
    }

    fn build_variance_bridge(
        &mut self,
        module: &hir::Module,
        class: mir::ClassId,
        source: mir::InterfaceId,
        target: mir::InterfaceId,
        method_index: usize,
        source_slot: &mir::TableSlot,
    ) -> mir::FunctionId {
        let (source_id, _) = self.interfaces.source(source);
        let (target_id, _) = self.interfaces.source(target);
        let source_signature = &module.interfaces[source_id].methods[method_index];
        let target_signature = &module.interfaces[target_id].methods[method_index];
        let source_types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        let target_types = Types { ..source_types };
        let mut source_params = Vec::new();
        let mut target_params = Vec::new();
        for (source_param, target_param) in
            source_signature.params.iter().zip(&target_signature.params)
        {
            source_params.push(source_types.lower(
                source_param.ty,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            ));
            target_params.push(target_types.lower(
                target_param.ty,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            ));
        }
        let source_return = source_types.lower(
            source_signature.return_ty,
            &mut self.enums,
            &mut self.structs,
            &mut self.interfaces,
            &mut self.shell,
        );
        let target_return = target_types.lower(
            target_signature.return_ty,
            &mut self.enums,
            &mut self.structs,
            &mut self.interfaces,
            &mut self.shell,
        );

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
        let mut args = vec![smir::Expr::local(this, mir::Type::Any)];
        for ((param, target_ty), source_ty) in target_signature
            .params
            .iter()
            .zip(target_params)
            .zip(source_params)
        {
            let local = locals.alloc(mir::Local {
                name: param.name.clone(),
                ty: target_ty.clone(),
                mutable: false,
            });
            params.push(mir::Param {
                name: param.name.clone(),
                ty: target_ty.clone(),
                local,
            });
            args.push(self.adapt_variance_bridge(
                smir::Expr::local(local, target_ty.clone()),
                &target_ty,
                &source_ty,
            ));
        }
        let callee = match source_slot {
            mir::TableSlot::Function(function) => mir::Callee::User(*function),
            mir::TableSlot::Runtime(function) => mir::Callee::Runtime(*function),
        };
        let call = smir::Expr::new(
            source_return.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee,
                },
                args,
                return_ty: source_return.clone(),
            }),
        );
        let kind = if target_return == mir::Type::Unit {
            smir::StatementKind::Expr(call)
        } else {
            let value = self.adapt_variance_bridge(call, &source_return, &target_return);
            smir::StatementKind::Return { value: Some(value) }
        };
        let class_name = &self.classes[class].name;
        let target_name = &self.interfaces.defs[target].name;
        let name = format!(
            "variance.{class_name}.{target_name}.{}.{}",
            target_signature.name, method_index
        );
        let body = cfg::lower(
            smir::Body {
                locals,
                statements: vec![smir::Statement {
                    kind,
                    span: target_signature.span,
                }],
            },
            target_return.clone(),
        );
        let id = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            symbol: format!("scoop.{name}"),
            name,
            params,
            return_ty: target_return.clone(),
            body,
        });
        self.top_level.push(id);
        if target_signature.is_suspend {
            self.suspend_sources.push(SuspendSource {
                function: id,
                source_return: target_return,
                instance: None,
            });
        }
        id
    }

    fn adapt_variance_bridge(
        &mut self,
        value: smir::Expr,
        source: &mir::Type,
        target: &mir::Type,
    ) -> smir::Expr {
        if source == target {
            return value;
        }
        if let (mir::Type::Function(source), mir::Type::Function(target_type)) = (source, target) {
            let adapter = self.ensure_function_adapter(*source, *target_type);
            return smir::Expr::new(
                mir::Type::Function(*target_type),
                smir::ExprKind::ClosureAlloc {
                    class: self.closure_adapters[adapter].class,
                    captures: vec![value],
                },
            );
        }
        if is_reference_mir(source) && is_reference_mir(target) {
            return value;
        }
        if is_boxable(source) && is_reference_mir(target) {
            let boxed = self
                .boxed
                .get_or_create(&mut self.classes, &mut self.shell, source);
            if let mir::Type::Interface(interface) = target {
                if !self.classes[boxed].interfaces.contains(interface) {
                    self.classes[boxed].interfaces.push(*interface);
                }
            }
            return smir::Expr::new(target.clone(), smir::ExprKind::Box(Box::new(value)));
        }
        unreachable!("variance bridge adaptations always follow a subtype conversion")
    }

    fn ensure_function_adapter(
        &mut self,
        source: mir::FunctionTypeId,
        target: mir::FunctionTypeId,
    ) -> mir::ClosureAdapterId {
        if let Some(&adapter) = self.closure_adapter_by_types.get(&(source, target)) {
            return adapter;
        }
        let source_signature = self.shell.function_types[source].clone();
        let target_signature = self.shell.function_types[target].clone();
        let source_name = mir::encode_type(&self.shell, &mir::Type::Function(source));
        let target_name = mir::encode_type(&self.shell, &mir::Type::Function(target));
        let function = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: format!("$adapter.{source_name}.{target_name}"),
            symbol: format!("scoop.$adapter.{source_name}.{target_name}"),
            params: Vec::new(),
            return_ty: target_signature.return_type.clone(),
            body: mir::Body::unreachable(Arena::new()),
        });
        self.top_level.push(function);
        let invoke = self
            .closure_invokes
            .alloc(mir::ClosureInvokeFunction { function });
        let class = self.closure_classes.alloc(mir::ClosureClass {
            name: format!("$Closure$adapter${source_name}${target_name}"),
            function_type: target,
            invoke,
            captures: vec![mir::Field {
                name: "$source".to_string(),
                ty: mir::Type::Function(source),
            }],
            bridges: Vec::new(),
        });
        let adapter = self.closure_adapters.alloc(mir::ClosureAdapter {
            class,
            source,
            target,
        });
        self.closure_adapter_by_types
            .insert((source, target), adapter);

        let span = Span::new(0, 0);
        let mut locals = Arena::new();
        let closure = locals.alloc(mir::Local {
            name: "$closure".to_string(),
            ty: mir::Type::Function(target),
            mutable: false,
        });
        let mut params = vec![mir::Param {
            name: "$closure".to_string(),
            ty: mir::Type::Function(target),
            local: closure,
        }];
        let mut args = vec![smir::Expr::new(
            mir::Type::Function(source),
            smir::ExprKind::ClosureCapture {
                closure: Box::new(smir::Expr::local(closure, mir::Type::Function(target))),
                class,
                index: 0,
            },
        )];
        for (index, (target_ty, source_ty)) in target_signature
            .parameter_types
            .iter()
            .zip(&source_signature.parameter_types)
            .enumerate()
        {
            let local = locals.alloc(mir::Local {
                name: format!("arg{index}"),
                ty: target_ty.clone(),
                mutable: false,
            });
            params.push(mir::Param {
                name: format!("arg{index}"),
                ty: target_ty.clone(),
                local,
            });
            args.push(self.adapt_variance_bridge(
                smir::Expr::local(local, target_ty.clone()),
                target_ty,
                source_ty,
            ));
        }
        let call = smir::Expr::new(
            source_signature.return_type.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Closure {
                        function_type: source,
                    },
                    callee: mir::Callee::Closure(source),
                },
                args,
                return_ty: source_signature.return_type.clone(),
            }),
        );
        let statements = if target_signature.return_type == mir::Type::Unit {
            vec![
                smir::Statement {
                    kind: smir::StatementKind::Expr(call),
                    span,
                },
                smir::Statement {
                    kind: smir::StatementKind::Return { value: None },
                    span,
                },
            ]
        } else {
            vec![smir::Statement {
                kind: smir::StatementKind::Return {
                    value: Some(self.adapt_variance_bridge(
                        call,
                        &source_signature.return_type,
                        &target_signature.return_type,
                    )),
                },
                span,
            }]
        };
        self.functions[function].params = params;
        self.functions[function].body = cfg::lower(
            smir::Body { locals, statements },
            target_signature.return_type.clone(),
        );
        if target_signature.is_suspend {
            self.suspend_sources.push(SuspendSource {
                function,
                source_return: target_signature.return_type,
                instance: None,
            });
        }
        adapter
    }

    fn interface_is_subtype(
        &mut self,
        module: &hir::Module,
        source: mir::InterfaceId,
        target: mir::InterfaceId,
    ) -> bool {
        if source == target {
            return true;
        }
        let (source_id, source_args) = self.interfaces.source(source);
        let source_args = source_args.to_vec();
        let (target_id, target_args) = self.interfaces.source(target);
        let target_args = target_args.to_vec();
        if module.interfaces[source_id].family != module.interfaces[target_id].family
            || source_args.len() != target_args.len()
        {
            return false;
        }
        module.interfaces[source_id]
            .variances
            .iter()
            .copied()
            .zip(&source_args)
            .zip(&target_args)
            .all(|((variance, source), target)| match variance {
                hir::Variance::Invariant => source == target,
                hir::Variance::Out => self.mir_type_is_subtype(module, source, target),
                hir::Variance::In => self.mir_type_is_subtype(module, target, source),
            })
    }

    fn mir_type_is_subtype(
        &mut self,
        module: &hir::Module,
        source: &mir::Type,
        target: &mir::Type,
    ) -> bool {
        if source == target || matches!(target, mir::Type::Any) {
            return true;
        }
        match (source, target) {
            (mir::Type::Class(source), mir::Type::Class(target)) => {
                let mut current = Some(*source);
                while let Some(class) = current {
                    if class == *target {
                        return true;
                    }
                    current = self.classes[class].base_class();
                }
                false
            }
            (mir::Type::Interface(source), mir::Type::Interface(target)) => {
                self.interface_is_subtype(module, *source, *target)
            }
            (mir::Type::Class(source), mir::Type::Interface(target)) => {
                let interfaces = self.classes[*source].interfaces.clone();
                interfaces
                    .into_iter()
                    .any(|interface| self.interface_is_subtype(module, interface, *target))
            }
            (mir::Type::Struct(_) | mir::Type::Enum(_, _), mir::Type::Interface(target)) => self
                .value_interfaces(module, source)
                .into_iter()
                .any(|interface| self.interface_is_subtype(module, interface, *target)),
            (mir::Type::Function(source), mir::Type::Function(target)) => {
                let source = self.shell.function_types[*source].clone();
                let target = self.shell.function_types[*target].clone();
                source.is_suspend == target.is_suspend
                    && source.parameter_types.len() == target.parameter_types.len()
                    && target
                        .parameter_types
                        .iter()
                        .zip(&source.parameter_types)
                        .all(|(target, source)| self.mir_type_is_subtype(module, target, source))
                    && self.mir_type_is_subtype(module, &source.return_type, &target.return_type)
            }
            _ => false,
        }
    }

    fn finalize_function_bridges(&mut self, module: &hir::Module) -> bool {
        let classes: Vec<_> = self.closure_classes.iter().map(|(id, _)| id).collect();
        let targets = self.function_bridge_targets.clone();
        let mut added = false;
        for class in classes {
            let source = self.closure_classes[class].function_type;
            for &target in &targets {
                if self.finalized_function_bridges.contains(&(class, target))
                    || !self.mir_type_is_subtype(
                        module,
                        &mir::Type::Function(source),
                        &mir::Type::Function(target),
                    )
                {
                    continue;
                }
                let function = if source == target {
                    let invoke = self.closure_classes[class].invoke;
                    self.closure_invokes[invoke].function
                } else {
                    self.build_function_bridge(class, source, target)
                };
                self.closure_classes[class]
                    .bridges
                    .push(mir::FunctionBridge { target, function });
                self.finalized_function_bridges.insert((class, target));
                added = true;
            }
        }
        added
    }

    fn build_function_bridge(
        &mut self,
        class: mir::ClosureClassId,
        source: mir::FunctionTypeId,
        target: mir::FunctionTypeId,
    ) -> mir::FunctionId {
        let source_signature = self.shell.function_types[source].clone();
        let target_signature = self.shell.function_types[target].clone();
        let mut locals = Arena::new();
        let closure = locals.alloc(mir::Local {
            name: "$source".to_string(),
            ty: mir::Type::Function(source),
            mutable: false,
        });
        let mut params = vec![mir::Param {
            name: "$source".to_string(),
            ty: mir::Type::Function(source),
            local: closure,
        }];
        let mut args = vec![smir::Expr::local(closure, mir::Type::Function(source))];
        for (index, (target_ty, source_ty)) in target_signature
            .parameter_types
            .iter()
            .zip(&source_signature.parameter_types)
            .enumerate()
        {
            let local = locals.alloc(mir::Local {
                name: format!("arg{index}"),
                ty: target_ty.clone(),
                mutable: false,
            });
            params.push(mir::Param {
                name: format!("arg{index}"),
                ty: target_ty.clone(),
                local,
            });
            args.push(self.adapt_variance_bridge(
                smir::Expr::local(local, target_ty.clone()),
                target_ty,
                source_ty,
            ));
        }
        let call = smir::Expr::new(
            source_signature.return_type.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Closure {
                        function_type: source,
                    },
                    callee: mir::Callee::Closure(source),
                },
                args,
                return_ty: source_signature.return_type.clone(),
            }),
        );
        let statements = if target_signature.return_type == mir::Type::Unit {
            vec![
                smir::Statement {
                    kind: smir::StatementKind::Expr(call),
                    span: Span { start: 0, end: 0 },
                },
                smir::Statement {
                    kind: smir::StatementKind::Return { value: None },
                    span: Span { start: 0, end: 0 },
                },
            ]
        } else {
            vec![smir::Statement {
                kind: smir::StatementKind::Return {
                    value: Some(self.adapt_variance_bridge(
                        call,
                        &source_signature.return_type,
                        &target_signature.return_type,
                    )),
                },
                span: Span { start: 0, end: 0 },
            }]
        };
        let source_name = &self.closure_classes[class].name;
        let target_name = mir::encode_type(&self.shell, &mir::Type::Function(target));
        let name = format!("function_bridge.{source_name}.{target_name}");
        let body = cfg::lower(
            smir::Body { locals, statements },
            target_signature.return_type.clone(),
        );
        let function = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            symbol: format!("scoop.{name}"),
            name,
            params,
            return_ty: target_signature.return_type.clone(),
            body,
        });
        self.top_level.push(function);
        if target_signature.is_suspend {
            self.suspend_sources.push(SuspendSource {
                function,
                source_return: target_signature.return_type,
                instance: None,
            });
        }
        function
    }

    /// Lower one fully concrete user function.
    fn lower_user_function(
        &mut self,
        module: &hir::Module,
        hir_id: hir::FunctionId,
    ) -> (Vec<mir::Param>, mir::Type, smir::Body) {
        let function = &module.functions[hir_id];
        let hir::FunctionKind::User(body) = &function.kind else {
            unreachable!("only user functions have MIR bodies")
        };
        let current_local_capture_params = module
            .local_functions
            .iter()
            .find(|(_, local)| local.function == hir_id)
            .map(|(_, local)| {
                local
                    .captures
                    .iter()
                    .zip(function.params.iter())
                    .map(|(capture, param)| (capture.binding, param.local))
                    .collect()
            })
            .unwrap_or_default();
        let current_closure = self.closure_by_function.get(&hir_id).copied();
        BodyLowerer {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
            interfaces: &mut self.interfaces,
            structs: &mut self.structs,
            method_slots: &self.method_slots,
            function_map: &self.function_map,
            extern_map: &self.extern_map,
            global_map: &self.global_map,
            callback_bridges: &mut self.callback_bridges,
            callback_by_target: &mut self.callback_by_target,
            foreign_callback_adapters: &mut self.foreign_callback_adapters,
            foreign_callback_bridges: &mut self.foreign_callback_bridges,
            foreign_callback_by_registration: &mut self.foreign_callback_by_registration,
            ctors: &self.ctors,
            strings: &mut self.strings,
            functions: &mut self.functions,
            top_level: &mut self.top_level,
            instances: &mut self.instances,
            enums: &mut self.enums,
            boxed: &mut self.boxed,
            classes: &mut self.classes,
            shell: &mut self.shell,
            local_map: HashMap::new(),
            constructor_param_map: HashMap::new(),
            locals: Arena::new(),
            hidden_count: 0,
            prelude: Vec::new(),
            option_variants: self.option_variants,
            coroutines: &mut self.coroutines,
            lambda_closures: &self.lambda_closures,
            anonymous_closures: &self.anonymous_closures,
            reference_closures: &self.reference_closures,
            closure_classes: &mut self.closure_classes,
            closure_invokes: &mut self.closure_invokes,
            closure_capture_indices: &mut self.closure_capture_indices,
            closure_adapters: &mut self.closure_adapters,
            closure_adapter_by_types: &mut self.closure_adapter_by_types,
            dynamic_closure_adapters: &mut self.dynamic_closure_adapters,
            dynamic_adapter_by_target: &mut self.dynamic_adapter_by_target,
            function_bridge_targets: &mut self.function_bridge_targets,
            suspend_sources: &mut self.suspend_sources,
            current_closure,
            current_closure_local: None,
            current_local_capture_params,
        }
        .lower_function(function, body)
    }

    /// Transpose local-concrete HIR structs into MIR in declaration order
    /// (ids only; field types are filled by `fill_struct_fields`).
    fn lower_structs(&mut self, module: &hir::Module) {
        for (hir_id, decl) in module.structs.iter() {
            let representation = match &decl.representation {
                hir::StructRepresentation::Declared { attributes, .. } => {
                    mir::StructRepresentation::Declared {
                        c_layout: attributes.c_layout.map(|layout| mir::CLayout {
                            aligned: layout.aligned,
                            packed: layout.packed,
                        }),
                        interior_mutable: attributes.interior_mutable,
                        fields: Vec::new(),
                    }
                }
                hir::StructRepresentation::Intrinsic { application, .. } => {
                    mir::StructRepresentation::Intrinsic(match application {
                        hir::IntrinsicTypeRepresentation::Int => {
                            mir::IntrinsicTypeRepresentation::Int
                        }
                        hir::IntrinsicTypeRepresentation::UInt => {
                            mir::IntrinsicTypeRepresentation::UInt
                        }
                        hir::IntrinsicTypeRepresentation::Boolean => {
                            mir::IntrinsicTypeRepresentation::Boolean
                        }
                        hir::IntrinsicTypeRepresentation::String
                        | hir::IntrinsicTypeRepresentation::Array { .. }
                        | hir::IntrinsicTypeRepresentation::MutableArray { .. } => {
                            unreachable!("the registry fixes intrinsic declaration targets")
                        }
                    })
                }
            };
            let mir_id = self.structs.defs.alloc(mir::StructDef {
                name: decl.name.clone(),
                gc_free: decl.gc_free,
                representation,
            });
            self.struct_map.insert(hir_id, mir_id);
            self.structs.instances.insert(mir_id, (hir_id, Vec::new()));
        }
    }

    /// Fill the MIR struct field types. This runs after the mangling shell
    /// exists because field types can reference concrete enums.
    fn fill_struct_fields(&mut self, module: &hir::Module) {
        for (hir_id, decl) in module.structs.iter() {
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let fields = match &decl.representation {
                hir::StructRepresentation::Declared { fields, .. } => fields
                    .iter()
                    .map(|field| mir::Field {
                        name: field.name.clone(),
                        ty: types.lower(
                            field.ty,
                            &mut self.enums,
                            &mut self.structs,
                            &mut self.interfaces,
                            &mut self.shell,
                        ),
                    })
                    .collect(),
                hir::StructRepresentation::Intrinsic { .. } => Vec::new(),
            };
            let mir_id = self.struct_map[&hir_id];
            let arguments = decl
                .type_arguments
                .iter()
                .map(|argument| {
                    types.lower(
                        *argument,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    )
                })
                .collect();
            match &mut self.structs.defs[mir_id].representation {
                mir::StructRepresentation::Declared {
                    fields: mir_fields, ..
                } => *mir_fields = fields,
                mir::StructRepresentation::Intrinsic(_) => debug_assert!(fields.is_empty()),
            }
            self.structs.instances.insert(mir_id, (hir_id, arguments));
        }
    }

    /// Materialize every local-concrete interface eagerly.
    fn lower_interfaces(&mut self, module: &hir::Module) {
        for (hir_id, decl) in module.interfaces.iter() {
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let arguments = decl
                .type_arguments
                .iter()
                .map(|argument| {
                    types.lower(
                        *argument,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    )
                })
                .collect();
            self.interfaces
                .get_or_create(module, &mut self.shell, hir_id, arguments);
        }
    }

    /// Transpose the HIR class arena into MIR in declaration order.
    /// Fields / vtable / itables are filled later (they need the base
    /// class and the method list, respectively).
    fn declare_classes(&mut self, module: &hir::Module) {
        for (hir_id, _) in module.classes.iter() {
            self.class_map
                .insert(hir_id, mir::ClassId::from_raw(hir_id.into_raw()));
        }
        for (hir_id, decl) in module.classes.iter() {
            let modifier = match decl.modifier {
                hir::ClassModifier::Final => mir::ClassModifier::Final,
                hir::ClassModifier::Open => mir::ClassModifier::Open,
                hir::ClassModifier::Abstract => mir::ClassModifier::Abstract,
            };
            let representation = match &decl.representation {
                hir::ClassRepresentation::Declared { .. } => mir::ClassRepresentation::Declared {
                    fields: Vec::new(),
                    base_class: None,
                },
                hir::ClassRepresentation::Intrinsic { application, .. } => {
                    let types = Types {
                        module,
                        struct_map: &self.struct_map,
                        class_map: &self.class_map,
                    };
                    mir::ClassRepresentation::Intrinsic(match application {
                        hir::IntrinsicTypeRepresentation::String => {
                            mir::IntrinsicTypeRepresentation::String
                        }
                        hir::IntrinsicTypeRepresentation::Array { element } => {
                            mir::IntrinsicTypeRepresentation::Array {
                                element: types.lower(
                                    *element,
                                    &mut self.enums,
                                    &mut self.structs,
                                    &mut self.interfaces,
                                    &mut self.shell,
                                ),
                            }
                        }
                        hir::IntrinsicTypeRepresentation::MutableArray { element } => {
                            mir::IntrinsicTypeRepresentation::MutableArray {
                                element: types.lower(
                                    *element,
                                    &mut self.enums,
                                    &mut self.structs,
                                    &mut self.interfaces,
                                    &mut self.shell,
                                ),
                            }
                        }
                        hir::IntrinsicTypeRepresentation::Int
                        | hir::IntrinsicTypeRepresentation::UInt
                        | hir::IntrinsicTypeRepresentation::Boolean => {
                            unreachable!("the registry fixes intrinsic declaration targets")
                        }
                    })
                }
            };
            let mir_id = self.classes.alloc(mir::ClassDef {
                modifier,
                name: decl.name.clone(),
                representation,
                interfaces: Vec::new(),
                vtable: Vec::new(),
                itables: Vec::new(),
            });
            assert_eq!(mir_id, self.class_map[&hir_id]);
        }
    }

    /// Resolve base classes and concrete interface applications after the
    /// mangling shell contains every class name. Interface type arguments may
    /// themselves be class types.
    fn fill_class_hierarchy(&mut self, module: &hir::Module) {
        for (hir_id, decl) in module.classes.iter() {
            let mir_id = self.class_map[&hir_id];
            let base_class = decl.base_class().map(|(base, _)| self.class_map[base]);
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let interfaces = decl
                .interfaces
                .iter()
                .map(|&interface_ty| {
                    let lowered = types.lower(
                        interface_ty,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    );
                    let mir::Type::Interface(interface) = lowered else {
                        unreachable!("HIR implementation lists contain only interfaces")
                    };
                    interface
                })
                .collect();
            let class = &mut self.classes[mir_id];
            match &mut class.representation {
                mir::ClassRepresentation::Declared {
                    base_class: mir_base,
                    ..
                } => *mir_base = base_class,
                mir::ClassRepresentation::Intrinsic(_) => debug_assert!(base_class.is_none()),
            }
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
        if let Some((type_arguments, symbol_identity)) = function_instance(module, function) {
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let arguments = type_arguments
                .iter()
                .map(|argument| {
                    types.lower(
                        *argument,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    )
                })
                .collect::<Vec<_>>();
            return match symbol_identity {
                hir::InstanceSymbol::Unique => mir::mangle_instance(&self.shell, &name, &arguments),
                hir::InstanceSymbol::Overloaded { discriminator } => {
                    mir::mangle_generic_overload(&self.shell, &name, &arguments, discriminator)
                }
            };
        }
        if hir_id == module.entry || !self.overloaded.contains(&name) {
            return mir::mangle_function(&name, hir_id == module.entry);
        }
        // A method's receiver (parameter 0, hir-lower's contract) is
        // not part of the overload signature: `Doc.describe(Int)`
        // encodes as `scoop.Doc.describe.I`.
        let skip = usize::from(function.method.is_some());
        let params = self.lower_params(module, &function.params[skip..]);
        mir::mangle_overload(&self.shell, &name, &params)
    }

    /// Lower a parameter list to MIR types (concrete enum definitions are
    /// transposed lazily on first reference).
    fn lower_params(&mut self, module: &hir::Module, params: &[hir::Param]) -> Vec<mir::Type> {
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        params
            .iter()
            .map(|param| {
                types.lower(
                    param.ty,
                    &mut self.enums,
                    &mut self.structs,
                    &mut self.interfaces,
                    &mut self.shell,
                )
            })
            .collect()
    }

    /// A method's dispatch signature key: `name(<param encoding>)`
    /// over the declared parameters (the receiver is not part of it)
    /// — `describe(I)`, `m(I_S)`, `f()`. An override shares the base
    /// method's key (hir-lower enforces exact-signature overriding),
    /// so keying vtable slots by it replaces the base slot in place,
    /// while overloads get distinct keys and thus distinct slots.
    fn fn_signature_key(&mut self, module: &hir::Module, function: &hir::Function) -> String {
        let skip = usize::from(function.method.is_some());
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        let params: Vec<_> = function.params[skip..]
            .iter()
            .map(|param| {
                types.lower(
                    param.ty,
                    &mut self.enums,
                    &mut self.structs,
                    &mut self.interfaces,
                    &mut self.shell,
                )
            })
            .collect();
        let encoding = mir::encode_params(&self.shell, &params);
        format!("{}({encoding})", short_name(&function.name))
    }

    /// The dispatch signature key of an interface method signature.
    fn sig_signature_key(&mut self, module: &hir::Module, sig: &hir::MethodSig) -> String {
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        let params: Vec<_> = sig
            .params
            .iter()
            .map(|param| {
                types.lower(
                    param.ty,
                    &mut self.enums,
                    &mut self.structs,
                    &mut self.interfaces,
                    &mut self.shell,
                )
            })
            .collect();
        let encoding = mir::encode_params(&self.shell, &params);
        format!("{}({encoding})", sig.name)
    }

    /// Declare one local-concrete user function (body filled later):
    /// `scoop.<name>`, `scoop.<Type>.<name>` for members, or the fixed
    /// entry symbol `scoop_main` that the C runtime calls (`main` is
    /// never instantiated from a generic template, hir-lower guarantees it);
    /// overloads get the
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
            gc_effect: lower_gc_effect(function.attributes.gc_effect),
            name,
            symbol,
            // Filled in when the body is lowered below.
            params: Vec::new(),
            return_ty: mir::Type::Unit,
            body: mir::Body::unreachable(Arena::new()),
        });
        self.top_level.push(id);
        self.function_map.insert(hir_id, id);
        if let Some((type_arguments, _)) = function_instance(module, function) {
            let type_args = type_arguments
                .iter()
                .map(|argument| {
                    Types {
                        module,
                        struct_map: &self.struct_map,
                        class_map: &self.class_map,
                    }
                    .lower(
                        *argument,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    )
                })
                .collect();
            self.instances.record(
                hir_id,
                id,
                self.functions[id].symbol.clone(),
                self.functions[id].name.clone(),
                type_args,
            );
        }
        id
    }

    fn lower_function_type_id(
        &mut self,
        module: &hir::Module,
        id: hir::FunctionTypeId,
    ) -> mir::FunctionTypeId {
        let ty = module.function_types[id].canonical_type;
        let lowered = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        }
        .lower(
            ty,
            &mut self.enums,
            &mut self.structs,
            &mut self.interfaces,
            &mut self.shell,
        );
        let mir::Type::Function(id) = lowered else {
            unreachable!("lowering a function type preserves its category")
        };
        id
    }

    /// Materialize the concrete closure classes before lowering any body, so
    /// every creation expression resolves directly to a typed class id.
    fn declare_closures(&mut self, module: &hir::Module) {
        for (id, lambda) in module.lambdas.iter() {
            let function_type = self.lower_function_type_id(module, lambda.function_type);
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let captures: Vec<_> = lambda
                .captures
                .iter()
                .map(|capture| mir::Field {
                    name: capture.name.clone(),
                    ty: types.lower(
                        capture.ty,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    ),
                })
                .collect();
            let invoke = self.closure_invokes.alloc(mir::ClosureInvokeFunction {
                function: self.function_map[&lambda.function],
            });
            let class = self.closure_classes.alloc(mir::ClosureClass {
                name: format!("$Closure$lambda{}", id.into_raw()),
                function_type,
                invoke,
                captures,
                bridges: Vec::new(),
            });
            self.closure_by_function.insert(lambda.function, class);
            for (index, capture) in lambda.captures.iter().enumerate() {
                self.closure_capture_indices
                    .insert((class, capture.binding), index as u32);
            }
            self.lambda_closures.insert(id, class);
        }
        for (id, anonymous) in module.anonymous_functions.iter() {
            let function_type = self.lower_function_type_id(module, anonymous.function_type);
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let captures: Vec<_> = anonymous
                .captures
                .iter()
                .map(|capture| mir::Field {
                    name: capture.name.clone(),
                    ty: types.lower(
                        capture.ty,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    ),
                })
                .collect();
            let invoke = self.closure_invokes.alloc(mir::ClosureInvokeFunction {
                function: self.function_map[&anonymous.function],
            });
            let class = self.closure_classes.alloc(mir::ClosureClass {
                name: format!("$Closure$anonymous{}", id.into_raw()),
                function_type,
                invoke,
                captures,
                bridges: Vec::new(),
            });
            self.closure_by_function.insert(anonymous.function, class);
            for (index, capture) in anonymous.captures.iter().enumerate() {
                self.closure_capture_indices
                    .insert((class, capture.binding), index as u32);
            }
            self.anonymous_closures.insert(id, class);
        }
        for (id, reference) in module.callable_references.iter() {
            let function_type = self.lower_function_type_id(module, reference.function_type);
            let (callable, receiver) = match &reference.target {
                hir::CallableReferenceTarget::Named(callable) => (*callable, None),
                hir::CallableReferenceTarget::Local { callee, .. } => (*callee, None),
                hir::CallableReferenceTarget::BoundMember { receiver, callee } => {
                    (*callee, Some(receiver.as_ref()))
                }
                hir::CallableReferenceTarget::BoundExtension { receiver, callee } => {
                    (*callee, Some(receiver.as_ref()))
                }
            };
            let target = self.lower_reference_callee(module, callable);
            let call_kind = match &reference.target {
                hir::CallableReferenceTarget::BoundMember { receiver, .. } => {
                    self.bound_reference_call_kind(module, receiver.ty, callable)
                }
                _ => mir::CallKind::Direct,
            };
            let signature = self.shell.function_types[function_type].clone();
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let mut capture_fields =
                Vec::with_capacity(reference.captures.len() + usize::from(receiver.is_some()));
            if let Some(receiver) = receiver {
                capture_fields.push(mir::Field {
                    name: "$receiver".to_string(),
                    ty: types.lower(
                        receiver.ty,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    ),
                });
            }
            capture_fields.extend(reference.captures.iter().map(|capture| mir::Field {
                name: capture.name.clone(),
                ty: types.lower(
                    capture.ty,
                    &mut self.enums,
                    &mut self.structs,
                    &mut self.interfaces,
                    &mut self.shell,
                ),
            }));
            let closure_ty = mir::Type::Function(function_type);
            let mut locals = Arena::new();
            let closure = locals.alloc(mir::Local {
                name: "$closure".to_string(),
                ty: closure_ty.clone(),
                mutable: false,
            });
            let mut params = vec![mir::Param {
                name: "$closure".to_string(),
                ty: closure_ty,
                local: closure,
            }];
            let mut source_args = Vec::with_capacity(signature.parameter_types.len());
            for (index, ty) in signature.parameter_types.iter().cloned().enumerate() {
                let local = locals.alloc(mir::Local {
                    name: format!("arg{index}"),
                    ty: ty.clone(),
                    mutable: false,
                });
                params.push(mir::Param {
                    name: format!("arg{index}"),
                    ty: ty.clone(),
                    local,
                });
                source_args.push(smir::Expr::local(local, ty));
            }
            let function = self.functions.alloc(mir::Function {
                gc_effect: mir::GcEffect::Managed,
                name: format!("$reference.{}", id.into_raw()),
                symbol: format!("scoop.$reference.{}", id.into_raw()),
                params: Vec::new(),
                return_ty: signature.return_type.clone(),
                body: mir::Body::unreachable(Arena::new()),
            });
            self.top_level.push(function);
            let invoke = self
                .closure_invokes
                .alloc(mir::ClosureInvokeFunction { function });
            let class = self.closure_classes.alloc(mir::ClosureClass {
                name: format!("$Closure$reference{}", id.into_raw()),
                function_type,
                invoke,
                captures: capture_fields,
                bridges: Vec::new(),
            });
            let capture_offset = u32::from(receiver.is_some());
            for (index, capture) in reference.captures.iter().enumerate() {
                self.closure_capture_indices
                    .insert((class, capture.binding), capture_offset + index as u32);
            }
            let mut args = Vec::with_capacity(
                usize::from(receiver.is_some()) + reference.captures.len() + source_args.len(),
            );
            if receiver.is_some() {
                args.push(smir::Expr::new(
                    self.closure_classes[class].captures[0].ty.clone(),
                    smir::ExprKind::ClosureCapture {
                        closure: Box::new(smir::Expr::local(
                            closure,
                            mir::Type::Function(function_type),
                        )),
                        class,
                        index: 0,
                    },
                ));
            } else if matches!(
                &reference.target,
                hir::CallableReferenceTarget::Local { .. }
            ) {
                for index in 0..reference.captures.len() {
                    args.push(smir::Expr::new(
                        self.closure_classes[class].captures[index].ty.clone(),
                        smir::ExprKind::ClosureCapture {
                            closure: Box::new(smir::Expr::local(
                                closure,
                                mir::Type::Function(function_type),
                            )),
                            class,
                            index: index as u32,
                        },
                    ));
                }
            } else {
                debug_assert!(reference.captures.is_empty());
            }
            args.extend(source_args);
            let call = smir::Expr::new(
                signature.return_type.clone(),
                smir::ExprKind::Call(smir::Call {
                    target: mir::CallTarget {
                        kind: call_kind,
                        callee: target,
                    },
                    args,
                    return_ty: signature.return_type.clone(),
                }),
            );
            let statement = if signature.return_type == mir::Type::Unit {
                smir::StatementKind::Expr(call)
            } else {
                smir::StatementKind::Return { value: Some(call) }
            };
            let mut statements = vec![smir::Statement {
                kind: statement,
                span: reference.span,
            }];
            if signature.return_type == mir::Type::Unit {
                statements.push(smir::Statement {
                    kind: smir::StatementKind::Return { value: None },
                    span: reference.span,
                });
            }
            let body = cfg::lower(
                smir::Body { locals, statements },
                signature.return_type.clone(),
            );
            self.functions[function].params = params;
            self.functions[function].body = body;
            if signature.is_suspend {
                self.suspend_sources.push(SuspendSource {
                    function,
                    source_return: signature.return_type,
                    instance: None,
                });
            }
            self.reference_closures.insert(id, class);
        }
    }

    fn lower_reference_callee(
        &mut self,
        _module: &hir::Module,
        callable: hir::Callable,
    ) -> mir::Callee {
        let hir::Callable::Function(function) = callable;
        self.instances.get(function).map_or_else(
            || mir::Callee::User(self.function_map[&function]),
            mir::Callee::Monomorphized,
        )
    }

    fn bound_reference_call_kind(
        &mut self,
        module: &hir::Module,
        receiver_ty: hir::TypeId,
        callable: hir::Callable,
    ) -> mir::CallKind {
        let function = module.callable_function(callable);
        let declaration = &module.functions[function];
        match module.types[receiver_ty].kind {
            hir::TypeKind::Class(_)
                if declaration
                    .method
                    .is_some_and(|method| method.modifier == hir::MethodModifier::Final) =>
            {
                mir::CallKind::Direct
            }
            hir::TypeKind::Class(class) => {
                let key = self.fn_signature_key(module, declaration);
                self.method_slots[&self.class_map[&class]]
                    .get(&key)
                    .map_or(mir::CallKind::Direct, |&slot| mir::CallKind::Virtual {
                        slot,
                    })
            }
            hir::TypeKind::Interface(..) => {
                let types = Types {
                    module,
                    struct_map: &self.struct_map,
                    class_map: &self.class_map,
                };
                let lowered = types.lower(
                    receiver_ty,
                    &mut self.enums,
                    &mut self.structs,
                    &mut self.interfaces,
                    &mut self.shell,
                );
                let mir::Type::Interface(interface) = lowered else {
                    unreachable!("an interface receiver lowers to an interface type")
                };
                let (source, _) = self.interfaces.source(interface);
                let key = self.fn_signature_key(module, declaration);
                let slot = module.interfaces[source]
                    .methods
                    .iter()
                    .enumerate()
                    .find_map(|(index, signature)| {
                        (self.sig_signature_key(module, signature) == key).then_some(index as u32)
                    })
                    .expect("hir-lower resolves interface references to interface methods");
                mir::CallKind::Interface { interface, slot }
            }
            hir::TypeKind::Any => unreachable!("Any has no methods"),
            _ => mir::CallKind::Direct,
        }
    }

    /// Declare an interface method as a signature-only shell that is never
    /// emitted. Virtual interface calls name it so LIR receives the complete
    /// indirect-call parameter and return types.
    fn declare_interface_method(&mut self, module: &hir::Module, hir_id: hir::FunctionId) {
        let function = &module.functions[hir_id];
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        let mut locals = Arena::new();
        let params = function
            .params
            .iter()
            .map(|param| {
                let ty = types.lower(
                    param.ty,
                    &mut self.enums,
                    &mut self.structs,
                    &mut self.interfaces,
                    &mut self.shell,
                );
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
        let return_ty = types.lower(
            function.return_ty,
            &mut self.enums,
            &mut self.structs,
            &mut self.interfaces,
            &mut self.shell,
        );
        let name = fn_name(function);
        let symbol = self.declare_symbol(module, hir_id);
        let id = self.functions.alloc(mir::Function {
            gc_effect: lower_gc_effect(function.attributes.gc_effect),
            symbol,
            name,
            params,
            return_ty: return_ty.clone(),
            body: mir::Body::unreachable(locals),
        });
        self.function_map.insert(hir_id, id);
        if let Some((type_arguments, _)) = function_instance(module, function) {
            let type_args = type_arguments
                .iter()
                .map(|argument| {
                    Types {
                        module,
                        struct_map: &self.struct_map,
                        class_map: &self.class_map,
                    }
                    .lower(
                        *argument,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    )
                })
                .collect();
            self.instances.record(
                hir_id,
                id,
                self.functions[id].symbol.clone(),
                self.functions[id].name.clone(),
                type_args,
            );
        }
    }

    /// Fill the MIR class fields: the base class's (already
    /// flattened) fields come first — the object layout and HIR's
    /// `ClassField` indices follow the same order — then the
    /// constructor properties in declaration order.
    fn fill_class_fields(&mut self, module: &hir::Module, order: &[hir::ClassId]) {
        for &hir_id in order {
            let decl = &module.classes[hir_id];
            let mir_id = self.class_map[&hir_id];
            if matches!(
                decl.representation,
                hir::ClassRepresentation::Intrinsic { .. }
            ) {
                debug_assert!(decl.base_class().is_none());
                continue;
            }
            let mut fields = match decl.base_class() {
                Some((base, _)) => {
                    clone_fields(self.classes[self.class_map[base]].declared_fields())
                }
                None => Vec::new(),
            };
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            for field in decl.declared_constructor() {
                fields.push(mir::Field {
                    name: field.name.clone(),
                    ty: types.lower(
                        field.ty,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    ),
                });
            }
            match &mut self.classes[mir_id].representation {
                mir::ClassRepresentation::Declared {
                    fields: mir_fields, ..
                } => *mir_fields = fields,
                mir::ClassRepresentation::Intrinsic(_) => debug_assert!(fields.is_empty()),
            }
        }
    }

    /// Fix every class's vtable and itables (impl spec 2.9): a derived
    /// vtable starts from the base's (prefix preserved), an override replaces
    /// the base slot in place, and new virtual methods append in declaration order
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
            let (mut vtable, mut slots) = match decl.base_class() {
                Some((base, _)) => {
                    let base = self.class_map[base];
                    (
                        clone_slots(&self.classes[base].vtable),
                        self.method_slots[&base].clone(),
                    )
                }
                None => (Vec::new(), HashMap::new()),
            };
            for (fn_id, function) in module.functions.iter() {
                if method_class(module, fn_id, function) != Some(hir_id)
                    || is_generic_method(function)
                {
                    continue;
                }
                let mir_fn = self.function_map[&fn_id];
                let key = self.fn_signature_key(module, function);
                let modifier = function.method.expect("class method metadata").modifier;
                match slots.get(&key) {
                    Some(&slot) => vtable[slot as usize] = mir::TableSlot::Function(mir_fn),
                    None if modifier != hir::MethodModifier::Final => {
                        slots.insert(key, vtable.len() as u32);
                        vtable.push(mir::TableSlot::Function(mir_fn));
                    }
                    // A fresh final method is statically dispatched and
                    // does not consume a vtable slot. A final override
                    // took the existing-slot arm above so base-typed
                    // calls still reach it.
                    None => {}
                }
            }
            let mut covered: Vec<mir::InterfaceId> = match decl.base_class() {
                Some((base, _)) => self.classes[self.class_map[base]]
                    .itables
                    .iter()
                    .map(|record| record.interface)
                    .collect(),
                None => Vec::new(),
            };
            for mir_iface in self.classes[mir_id].interfaces.clone() {
                if !covered.contains(&mir_iface) {
                    covered.push(mir_iface);
                }
            }
            let mut itables = Vec::new();
            for mir_iface in covered {
                let (hir_iface, _) = self.interfaces.source(mir_iface);
                let mut slots_for = Vec::new();
                for method in module.interfaces[hir_iface].methods.iter() {
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
            gc_effect: mir::GcEffect::Managed,
            symbol: format!("scoop.{name}"),
            name,
            params: Vec::new(),
            return_ty: mir::Type::Unit,
            body: mir::Body::unreachable(Arena::new()),
        });
        self.top_level.push(id);
        self.ctors.insert(hir_id, id);
        id
    }

    /// Lower the constructor function's body: a single raw
    /// `smir::ExprKind::ClassInit` over the flattened field values — the
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
    ) -> (Vec<mir::Param>, mir::Type, smir::Body) {
        let decl = &module.classes[hir_id];
        let mir_id = self.class_map[&hir_id];
        let field_count = self.classes[mir_id].declared_fields().len();
        let mut lowerer = BodyLowerer {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
            interfaces: &mut self.interfaces,
            structs: &mut self.structs,
            method_slots: &self.method_slots,
            function_map: &self.function_map,
            extern_map: &self.extern_map,
            global_map: &self.global_map,
            callback_bridges: &mut self.callback_bridges,
            callback_by_target: &mut self.callback_by_target,
            foreign_callback_adapters: &mut self.foreign_callback_adapters,
            foreign_callback_bridges: &mut self.foreign_callback_bridges,
            foreign_callback_by_registration: &mut self.foreign_callback_by_registration,
            ctors: &self.ctors,
            strings: &mut self.strings,
            functions: &mut self.functions,
            top_level: &mut self.top_level,
            instances: &mut self.instances,
            enums: &mut self.enums,
            boxed: &mut self.boxed,
            classes: &mut self.classes,
            shell: &mut self.shell,
            // Delegation arguments are closed (hir-lower M6 lowers
            // them in an empty scope), so no locals are visible.
            local_map: HashMap::new(),
            constructor_param_map: HashMap::new(),
            locals: Arena::new(),
            hidden_count: 0,
            prelude: Vec::new(),
            option_variants: self.option_variants,
            coroutines: &mut self.coroutines,
            lambda_closures: &self.lambda_closures,
            anonymous_closures: &self.anonymous_closures,
            reference_closures: &self.reference_closures,
            closure_classes: &mut self.closure_classes,
            closure_invokes: &mut self.closure_invokes,
            closure_capture_indices: &mut self.closure_capture_indices,
            closure_adapters: &mut self.closure_adapters,
            closure_adapter_by_types: &mut self.closure_adapter_by_types,
            dynamic_closure_adapters: &mut self.dynamic_closure_adapters,
            dynamic_adapter_by_target: &mut self.dynamic_adapter_by_target,
            function_bridge_targets: &mut self.function_bridge_targets,
            suspend_sources: &mut self.suspend_sources,
            current_closure: None,
            current_closure_local: None,
            current_local_capture_params: HashMap::new(),
        };
        let mut params = Vec::new();
        let mut own = Vec::new();
        for field in decl.declared_constructor() {
            let ty = lowerer.lower_type(field.ty);
            let local = lowerer.locals.alloc(mir::Local {
                name: field.name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            params.push(mir::Param {
                name: field.name.clone(),
                ty: ty.clone(),
                local,
            });
            lowerer.constructor_param_map.insert(field.parameter, local);
            own.push(smir::Expr::local(local, ty));
        }
        let args = flattened_ctor_args(&mut lowerer, module, hir_id, own);
        assert_eq!(
            args.len(),
            field_count,
            "the flattened initializer covers every field"
        );
        let body = smir::Body {
            locals: lowerer.locals,
            statements: vec![smir::Statement {
                kind: smir::StatementKind::Return {
                    value: Some(smir::Expr::new(
                        mir::Type::Class(mir_id),
                        smir::ExprKind::ClassInit {
                            class_id: mir_id,
                            args,
                        },
                    )),
                },
                span: decl.span,
            }],
        };
        (params, mir::Type::Class(mir_id), body)
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
                if method_class(module, fn_id, function) != Some(class)
                    || is_generic_method(function)
                {
                    continue;
                }
                if self.fn_signature_key(module, function) == key {
                    return self.function_map[&fn_id];
                }
            }
            current = module.classes[class].base_class().map(|(base, _)| *base);
        }
        unreachable!("hir-lower guarantees `{key}` is implemented")
    }

    /// Generate boxed value types' ordinary interface dispatch. A box has no
    /// universal vtable entries; every interface the value type implements
    /// gets an itable whose slots point at adjust thunks. The thunk's
    /// `this` is the boxed object; it unboxes and tail-calls the real
    /// value method.
    fn finalize_boxed(&mut self, module: &hir::Module, index: usize) {
        let class_id = self.boxed.order[index];
        let payload = self.classes[class_id].declared_fields()[0].ty.clone();
        let encoded = mir::encode_type(&self.shell, &payload);
        debug_assert!(self.classes[class_id].vtable.is_empty());
        let interfaces = self.classes[class_id].interfaces.clone();
        for iface in interfaces {
            let (hir_iface, _) = self.interfaces.source(iface);
            let method_indices: Vec<_> = module.interfaces[hir_iface]
                .methods
                .iter()
                .enumerate()
                .map(|(index, _)| index)
                .collect();
            let mut slots = Vec::new();
            for index in method_indices {
                let thunk = self.build_thunk(module, &payload, &encoded, iface, index);
                slots.push(mir::TableSlot::Function(thunk));
            }
            self.classes[class_id].itables.push(mir::ItableRecord {
                interface: iface,
                slots,
            });
        }
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
        method_index: usize,
    ) -> mir::FunctionId {
        let (hir_iface, _) = self.interfaces.source(iface);
        let signature = &module.interfaces[hir_iface].methods[method_index];
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
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
        let mut args = vec![smir::Expr::new(
            payload.clone(),
            smir::ExprKind::Unbox(Box::new(smir::Expr::local(this, mir::Type::Any))),
        )];
        let mut target_params = Vec::new();
        let mut argument_locals = Vec::new();
        for param in &signature.params {
            let ty = types.lower(
                param.ty,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
            target_params.push(ty.clone());
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
            argument_locals.push(local);
        }
        let return_ty = types.lower(
            signature.return_ty,
            &mut self.enums,
            &mut self.structs,
            &mut self.interfaces,
            &mut self.shell,
        );
        let source_interface = self
            .value_interfaces(module, payload)
            .into_iter()
            .find(|source| self.interface_is_subtype(module, *source, iface))
            .expect("HIR guarantees the boxed value implements the target interface");
        let (source_hir_interface, _) = self.interfaces.source(source_interface);
        let source_signature = &module.interfaces[source_hir_interface].methods[method_index];
        let source_types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        let mut source_params = Vec::new();
        for param in &source_signature.params {
            source_params.push(source_types.lower(
                param.ty,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            ));
        }
        let implementation = self.value_method(module, payload, &signature.name, &source_params);
        for ((local, target_ty), source_ty) in argument_locals
            .into_iter()
            .zip(&target_params)
            .zip(&implementation.1)
        {
            args.push(self.adapt_variance_bridge(
                smir::Expr::local(local, target_ty.clone()),
                target_ty,
                source_ty,
            ));
        }
        let call = smir::Expr::new(
            implementation.2.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee: mir::Callee::User(implementation.0),
                },
                args,
                return_ty: implementation.2.clone(),
            }),
        );
        let kind = if return_ty == mir::Type::Unit {
            smir::StatementKind::Expr(call)
        } else {
            smir::StatementKind::Return {
                value: Some(self.adapt_variance_bridge(call, &implementation.2, &return_ty)),
            }
        };
        let encoding = mir::encode_params(&self.shell, &target_params);
        let iface_name = self.interfaces.defs[iface].name.clone();
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
        let body = cfg::lower(
            smir::Body {
                locals,
                statements: vec![smir::Statement {
                    kind,
                    span: signature.span,
                }],
            },
            return_ty.clone(),
        );
        let id = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            symbol: format!("scoop.{name}"),
            name,
            params,
            return_ty: return_ty.clone(),
            body,
        });
        self.top_level.push(id);
        if signature.is_suspend {
            self.suspend_sources.push(SuspendSource {
                function: id,
                source_return: return_ty,
                instance: None,
            });
        }
        id
    }

    /// The value type's own method with the signature `key` (the
    /// implementation a boxed thunk tail-calls). HIR guarantees it
    /// exists: the value type was boxed to an interface that declares
    /// the method.
    fn value_interfaces(
        &mut self,
        module: &hir::Module,
        payload: &mir::Type,
    ) -> Vec<mir::InterfaceId> {
        let declared = match self.value_struct_source(module, payload) {
            Some(hir_id) => module.structs[hir_id].interfaces.clone(),
            None => match payload {
                mir::Type::Enum(mir_id, _) => {
                    let hir_id = self.enums.hir_ids[mir_id];
                    module.enums[hir_id].interfaces.clone()
                }
                _ => return Vec::new(),
            },
        };
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        declared
            .into_iter()
            .map(|ty| {
                let lowered = types.lower(
                    ty,
                    &mut self.enums,
                    &mut self.structs,
                    &mut self.interfaces,
                    &mut self.shell,
                );
                let mir::Type::Interface(interface) = lowered else {
                    unreachable!()
                };
                interface
            })
            .collect()
    }

    fn value_method(
        &mut self,
        module: &hir::Module,
        payload: &mir::Type,
        name: &str,
        expected_params: &[mir::Type],
    ) -> (mir::FunctionId, Vec<mir::Type>, mir::Type) {
        let source_methods = match self.value_struct_source(module, payload) {
            Some(source) => module.structs[source].methods.clone(),
            None => match payload {
                mir::Type::Enum(id, _) => module.enums[self.enums.hir_ids[id]].methods.clone(),
                _ => Vec::new(),
            },
        };
        let mut candidates = Vec::new();
        let mut owner_methods = Vec::new();
        for (fn_id, function) in module.functions.iter() {
            if !source_methods.contains(&fn_id) {
                continue;
            }
            owner_methods.push(function.name.clone());
            if short_name(&function.name) != name {
                continue;
            }
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let params: Vec<_> = function.params[1..]
                .iter()
                .map(|param| {
                    types.lower(
                        param.ty,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    )
                })
                .collect();
            candidates.push((function.name.clone(), params.clone()));
            if params != expected_params {
                continue;
            }
            let return_ty = types.lower(
                function.return_ty,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
            let function = self.function_map[&fn_id];
            return (function, params, return_ty);
        }
        let payload_name = match payload {
            mir::Type::Struct(id) => self.structs.defs[*id].name.clone(),
            mir::Type::Enum(id, _) => self.enums.defs[*id].name.clone(),
            _ => format!("{payload:?}"),
        };
        panic!(
            "concrete HIR guarantees `{name}` is implemented by boxed value type {payload_name} ({payload:?}) with parameters {expected_params:?}; owner methods: {owner_methods:?}; matching-name candidates: {candidates:?}"
        )
    }

    /// Exact source declaration for a MIR struct-like payload. Primitive
    /// representations use the typed relation emitted by HIR; ordinary
    /// struct instances use the explicit HIR→MIR instance maps.
    fn value_struct_source(
        &self,
        module: &hir::Module,
        payload: &mir::Type,
    ) -> Option<hir::StructId> {
        match payload {
            mir::Type::Int => Some(module.intrinsic_type_core.int),
            mir::Type::UInt => Some(module.intrinsic_type_core.uint),
            mir::Type::Boolean => Some(module.intrinsic_type_core.boolean),
            mir::Type::Struct(mir_id) => self
                .structs
                .instances
                .get(mir_id)
                .map(|(id, _)| *id)
                .or_else(|| {
                    self.struct_map
                        .iter()
                        .find_map(|(hir, mir)| (*mir == *mir_id).then_some(*hir))
                }),
            _ => None,
        }
    }
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

fn method_owner_type_arguments(module: &hir::Module, owner: hir::MethodOwner) -> &[hir::TypeId] {
    match owner {
        hir::MethodOwner::Class(id) => &module.classes[id].type_arguments,
        hir::MethodOwner::Struct(id) => &module.structs[id].type_arguments,
        hir::MethodOwner::Enum(id) => &module.enums[id].type_arguments,
        hir::MethodOwner::Interface(id) => &module.interfaces[id].type_arguments,
        hir::MethodOwner::Structural(_) => &[],
    }
}

/// Exact specialization data emitted by HIR. `None` means this declaration
/// uses ordinary overload mangling; it never means "unknown".
fn function_instance(
    module: &hir::Module,
    function: &hir::Function,
) -> Option<(Vec<hir::TypeId>, hir::InstanceSymbol)> {
    match &function.origin {
        hir::FunctionOrigin::Free(hir::FreeFunctionOrigin::Plain) => None,
        hir::FunctionOrigin::Free(hir::FreeFunctionOrigin::Generic {
            arguments, symbol, ..
        }) => Some((arguments.clone(), *symbol)),
        hir::FunctionOrigin::Method(method) => match &method.specialization {
            hir::MethodSpecialization::Plain => None,
            hir::MethodSpecialization::OwnerParameterized { symbol } => Some((
                method_owner_type_arguments(module, method.owner).to_vec(),
                *symbol,
            )),
            hir::MethodSpecialization::Generic {
                method_arguments,
                symbol,
                ..
            } => {
                let mut arguments = method_owner_type_arguments(module, method.owner).to_vec();
                arguments.extend(method_arguments.iter().copied());
                Some((arguments, *symbol))
            }
        },
    }
}

fn is_generic_method(function: &hir::Function) -> bool {
    matches!(
        function.origin,
        hir::FunctionOrigin::Method(hir::MethodOrigin {
            specialization: hir::MethodSpecialization::Generic { .. },
            ..
        })
    )
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

/// A method's short name: hir-lower qualifies member functions as
/// `Owner.method`; slot lookup, override matching and implementation
/// resolution all use the short name (M6 has no overloading).
fn short_name(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

/// Whether a function is an abstract class method. HIR carries this
/// explicitly, including for `Unit`-returning methods.
fn is_abstract_bodiless(function: &hir::Function) -> bool {
    function
        .method
        .is_some_and(|method| method.modifier == hir::MethodModifier::Abstract)
}

/// The class a function is a method of, if any.
fn method_class(
    module: &hir::Module,
    function_id: hir::FunctionId,
    function: &hir::Function,
) -> Option<hir::ClassId> {
    match function.method {
        Some(method) => match module.types[method.owner].kind {
            hir::TypeKind::Class(id) => Some(id),
            hir::TypeKind::String
                if module.classes[module.intrinsic_type_core.string]
                    .methods
                    .contains(&function_id) =>
            {
                Some(module.intrinsic_type_core.string)
            }
            _ => None,
        },
        None => None,
    }
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
        let methods = decl
            .methods
            .iter()
            .map(|method| method.name.clone())
            .collect();
        let id = self.defs.alloc(mir::InterfaceDef {
            name: name.clone(),
            methods,
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

/// Concrete struct definitions and source-instance metadata. Each HIR struct
/// is already specialized; the retained type arguments are only for boxing,
/// interface lookup, and MIR metadata.
#[derive(Default)]
struct StructRegistry {
    defs: Arena<mir::StructDef>,
    /// Concrete generic instance -> source declaration and arguments.
    instances: HashMap<mir::StructId, (hir::StructId, Vec<mir::Type>)>,
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

/// Metadata for functions that HIR has already fully instantiated. MIR never
/// owns an instantiation worklist; it only records the concrete source mapping.
#[derive(Default)]
struct InstanceRegistry {
    by_function: HashMap<hir::FunctionId, mir::MonomorphizedFunctionId>,
    meta: Arena<mir::MonomorphizedFunction>,
}

impl InstanceRegistry {
    fn record(
        &mut self,
        source: hir::FunctionId,
        function: mir::FunctionId,
        symbol: String,
        name: String,
        type_args: Vec<mir::Type>,
    ) -> mir::MonomorphizedFunctionId {
        let id = self.meta.alloc(mir::MonomorphizedFunction {
            function,
            symbol,
            source: name,
            type_args,
        });
        assert!(self.by_function.insert(source, id).is_none());
        id
    }

    fn get(&self, source: hir::FunctionId) -> Option<mir::MonomorphizedFunctionId> {
        self.by_function.get(&source).copied()
    }
}

/// Per-function-body lowering state.
struct BodyLowerer<'a> {
    module: &'a hir::Module,
    struct_map: &'a HashMap<hir::StructId, mir::StructId>,
    class_map: &'a HashMap<hir::ClassId, mir::ClassId>,
    interfaces: &'a mut InterfaceRegistry,
    /// MIR struct definitions used for representation and GC
    /// classification of compiler-synthesized aggregates.
    structs: &'a mut StructRegistry,
    /// Method signature key -> vtable slot per class
    /// (`compute_dispatch`).
    method_slots: &'a HashMap<mir::ClassId, HashMap<String, u32>>,
    function_map: &'a HashMap<hir::FunctionId, mir::FunctionId>,
    extern_map: &'a HashMap<hir::ExternFunctionId, mir::ExternFunctionId>,
    global_map: &'a HashMap<hir::GlobalId, mir::GlobalId>,
    callback_bridges: &'a mut Arena<mir::CallbackBridge>,
    callback_by_target:
        &'a mut HashMap<(mir::FunctionId, mir::FunctionTypeId), mir::CallbackBridgeId>,
    foreign_callback_adapters: &'a mut Arena<mir::ForeignCallbackAdapter>,
    foreign_callback_bridges: &'a mut Arena<mir::ForeignCallbackBridge>,
    foreign_callback_by_registration:
        &'a mut HashMap<hir::ForeignCallbackRegistrationId, mir::ForeignCallbackBridgeId>,
    /// HIR class -> its constructor function (`ClassInit` calls).
    ctors: &'a HashMap<hir::ClassId, mir::FunctionId>,
    strings: &'a mut Arena<mir::StringConst>,
    functions: &'a mut Arena<mir::Function>,
    top_level: &'a mut Vec<mir::FunctionId>,
    instances: &'a mut InstanceRegistry,
    /// Instantiated enum definitions, filled on creation; variant
    /// field types feed pattern lowering and representation.
    enums: &'a mut EnumRegistry,
    /// Boxed value types discovered in this body (`Box` / `is` / `as`).
    boxed: &'a mut BoxedRegistry,
    /// MIR class arena (boxed value types are appended here).
    classes: &'a mut Arena<mir::ClassDef>,
    /// Mangling shell (enum / struct names for `encode_type`).
    shell: &'a mut mir::Module,
    /// HIR local -> MIR local (same declaration order per body).
    local_map: HashMap<hir::LocalId, mir::LocalId>,
    /// Constructor-parameter identities available while lowering one
    /// generated class constructor's delegation expressions.
    constructor_param_map: HashMap<hir::ConstructorParamId, mir::LocalId>,
    /// MIR locals, including the hidden ones created during lowering
    /// (`when` subjects, destructuring slots, `!!` temporaries).
    locals: Arena<mir::Local>,
    hidden_count: usize,
    /// Statement kinds that must precede the statement currently being
    /// lowered (the trap test of `!!`); drained by the caller.
    prelude: Vec<smir::StatementKind>,
    /// Declaration indices of `Option::Some` / `Option::None`.
    option_variants: (u32, u32),
    coroutines: &'a mut CoroutineRegistry,
    lambda_closures: &'a HashMap<hir::LambdaId, mir::ClosureClassId>,
    anonymous_closures: &'a HashMap<hir::AnonymousFunctionId, mir::ClosureClassId>,
    reference_closures: &'a HashMap<hir::CallableReferenceId, mir::ClosureClassId>,
    closure_classes: &'a mut Arena<mir::ClosureClass>,
    closure_invokes: &'a mut Arena<mir::ClosureInvokeFunction>,
    closure_capture_indices: &'a mut HashMap<(mir::ClosureClassId, hir::BindingId), u32>,
    closure_adapters: &'a mut Arena<mir::ClosureAdapter>,
    closure_adapter_by_types:
        &'a mut HashMap<(mir::FunctionTypeId, mir::FunctionTypeId), mir::ClosureAdapterId>,
    dynamic_closure_adapters: &'a mut Arena<mir::DynamicClosureAdapter>,
    dynamic_adapter_by_target: &'a mut HashMap<mir::FunctionTypeId, mir::DynamicClosureAdapterId>,
    function_bridge_targets: &'a mut Vec<mir::FunctionTypeId>,
    suspend_sources: &'a mut Vec<SuspendSource>,
    current_closure: Option<mir::ClosureClassId>,
    current_closure_local: Option<mir::LocalId>,
    /// Hidden by-value parameters of a lifted local function, keyed by the
    /// global lexical binding they carry.
    current_local_capture_params: HashMap<hir::BindingId, hir::LocalId>,
}

/// A step from a pattern subject down to a nested field.
#[derive(Clone, Copy)]
enum Access {
    Field(u32),
    EnumField { variant: u32, index: u32 },
}

impl BodyLowerer<'_> {
    fn lower_function(
        mut self,
        function: &hir::Function,
        body: &hir::Body,
    ) -> (Vec<mir::Param>, mir::Type, smir::Body) {
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
        if self.current_closure.is_some() {
            self.current_closure_local = function
                .params
                .first()
                .map(|param| self.local_map[&param.local]);
        }
        let return_ty = self.lower_type(function.return_ty);
        let statements = if is_abstract_bodiless(function) {
            // An abstract method (hir-lower materializes it bodiless):
            // every override replaces its vtable slot and the class
            // cannot be instantiated, so the slot is never reached;
            // the emitted function traps like a pure-virtual stub.
            let message =
                self.trap_message(format!("call to abstract method `{}`", fn_name(function)));
            vec![smir::Statement {
                kind: smir::StatementKind::Expr(smir::Expr::new(
                    mir::Type::Unit,
                    smir::ExprKind::Call(smir::Call {
                        target: mir::CallTarget {
                            kind: mir::CallKind::Direct,
                            callee: mir::Callee::Runtime(mir::RuntimeFn::Trap),
                        },
                        args: vec![smir::Expr::new(
                            mir::Type::String,
                            smir::ExprKind::StringConst(message),
                        )],
                        return_ty: mir::Type::Unit,
                    }),
                )),
                span: function.span,
            }]
        } else {
            self.lower_statements(&body.statements)
        };
        (
            params,
            return_ty,
            smir::Body {
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
        }
        .lower(ty, self.enums, self.structs, self.interfaces, self.shell)
    }

    fn lower_function_type_id(
        &mut self,
        function_type: hir::FunctionTypeId,
    ) -> mir::FunctionTypeId {
        let ty = self.module.function_types[function_type].canonical_type;
        let mir::Type::Function(function_type) = self.lower_type(ty) else {
            unreachable!("lowering a function type preserves its category")
        };
        function_type
    }

    fn ensure_lambda_closure(&mut self, id: hir::LambdaId) -> mir::ClosureClassId {
        self.lambda_closures[&id]
    }

    fn ensure_anonymous_closure(&mut self, id: hir::AnonymousFunctionId) -> mir::ClosureClassId {
        self.anonymous_closures[&id]
    }

    fn ensure_reference_closure(&mut self, id: hir::CallableReferenceId) -> mir::ClosureClassId {
        self.reference_closures[&id]
    }

    fn adapt_function_value(
        &mut self,
        value: smir::Expr,
        source: mir::FunctionTypeId,
        target: mir::FunctionTypeId,
        span: Span,
    ) -> smir::Expr {
        if source == target {
            return value;
        }
        let adapter = self.ensure_function_adapter(source, target, span);
        smir::Expr::new(
            mir::Type::Function(target),
            smir::ExprKind::ClosureAlloc {
                class: self.closure_adapters[adapter].class,
                captures: vec![value],
            },
        )
    }

    fn adapt_mir_subtype(
        &mut self,
        value: smir::Expr,
        source: &mir::Type,
        target: &mir::Type,
        span: Span,
    ) -> smir::Expr {
        if source == target {
            return value;
        }
        if let (mir::Type::Function(source), mir::Type::Function(target)) = (source, target) {
            return self.adapt_function_value(value, *source, *target, span);
        }
        if is_boxable(source) && is_reference_mir(target) {
            self.register_boxed(source, None);
            if let mir::Type::Interface(interface) = target {
                let boxed = self.boxed.get_or_create(self.classes, self.shell, source);
                if !self.classes[boxed].interfaces.contains(interface) {
                    self.classes[boxed].interfaces.push(*interface);
                }
            }
            return smir::Expr::new(target.clone(), smir::ExprKind::Box(Box::new(value)));
        }
        if is_reference_mir(source) && is_reference_mir(target) {
            return smir::Expr::new(
                target.clone(),
                smir::ExprKind::Retype {
                    operand: Box::new(value),
                    ty: Box::new(target.clone()),
                },
            );
        }
        unreachable!("function adapter conversions follow the HIR subtype relation")
    }

    fn ensure_function_adapter(
        &mut self,
        source: mir::FunctionTypeId,
        target: mir::FunctionTypeId,
        span: Span,
    ) -> mir::ClosureAdapterId {
        if let Some(&adapter) = self.closure_adapter_by_types.get(&(source, target)) {
            return adapter;
        }
        let source_signature = self.shell.function_types[source].clone();
        let target_signature = self.shell.function_types[target].clone();
        assert_eq!(
            source_signature.is_suspend, target_signature.is_suspend,
            "ordinary and suspend function types never coerce"
        );
        assert_eq!(
            source_signature.parameter_types.len(),
            target_signature.parameter_types.len(),
            "function variance preserves arity"
        );
        let source_name = mir::encode_type(self.shell, &mir::Type::Function(source));
        let target_name = mir::encode_type(self.shell, &mir::Type::Function(target));
        let name = format!("$Closure$adapter${source_name}${target_name}");

        let function = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: format!("$adapter.{source_name}.{target_name}"),
            symbol: format!("scoop.$adapter.{source_name}.{target_name}"),
            params: Vec::new(),
            return_ty: target_signature.return_type.clone(),
            body: mir::Body::unreachable(Arena::new()),
        });
        self.top_level.push(function);
        let invoke = self
            .closure_invokes
            .alloc(mir::ClosureInvokeFunction { function });
        let class = self.closure_classes.alloc(mir::ClosureClass {
            name,
            function_type: target,
            invoke,
            captures: vec![mir::Field {
                name: "$source".to_string(),
                ty: mir::Type::Function(source),
            }],
            bridges: Vec::new(),
        });
        let adapter = self.closure_adapters.alloc(mir::ClosureAdapter {
            class,
            source,
            target,
        });
        self.closure_adapter_by_types
            .insert((source, target), adapter);

        let mut locals = Arena::new();
        let closure = locals.alloc(mir::Local {
            name: "$closure".to_string(),
            ty: mir::Type::Function(target),
            mutable: false,
        });
        let mut params = vec![mir::Param {
            name: "$closure".to_string(),
            ty: mir::Type::Function(target),
            local: closure,
        }];
        let mut args = vec![smir::Expr::new(
            mir::Type::Function(source),
            smir::ExprKind::ClosureCapture {
                closure: Box::new(smir::Expr::local(closure, mir::Type::Function(target))),
                class,
                index: 0,
            },
        )];
        for (index, (target_ty, source_ty)) in target_signature
            .parameter_types
            .iter()
            .zip(&source_signature.parameter_types)
            .enumerate()
        {
            let local = locals.alloc(mir::Local {
                name: format!("arg{index}"),
                ty: target_ty.clone(),
                mutable: false,
            });
            params.push(mir::Param {
                name: format!("arg{index}"),
                ty: target_ty.clone(),
                local,
            });
            args.push(self.adapt_mir_subtype(
                smir::Expr::local(local, target_ty.clone()),
                target_ty,
                source_ty,
                span,
            ));
        }
        let call = smir::Expr::new(
            source_signature.return_type.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Closure {
                        function_type: source,
                    },
                    callee: mir::Callee::Closure(source),
                },
                args,
                return_ty: source_signature.return_type.clone(),
            }),
        );
        let mut statements = if target_signature.return_type == mir::Type::Unit {
            vec![
                smir::Statement {
                    kind: smir::StatementKind::Expr(call),
                    span,
                },
                smir::Statement {
                    kind: smir::StatementKind::Return { value: None },
                    span,
                },
            ]
        } else {
            vec![smir::Statement {
                kind: smir::StatementKind::Return {
                    value: Some(self.adapt_mir_subtype(
                        call,
                        &source_signature.return_type,
                        &target_signature.return_type,
                        span,
                    )),
                },
                span,
            }]
        };
        self.functions[function].params = params;
        self.functions[function].body = cfg::lower(
            smir::Body {
                locals,
                statements: std::mem::take(&mut statements),
            },
            target_signature.return_type,
        );
        if target_signature.is_suspend {
            self.suspend_sources.push(SuspendSource {
                function,
                source_return: self.shell.function_types[target].return_type.clone(),
                instance: None,
            });
        }
        adapter
    }

    fn adapt_checked_function_value(
        &mut self,
        value: smir::Expr,
        target: mir::FunctionTypeId,
        span: Span,
    ) -> smir::Expr {
        let adapter = self.ensure_dynamic_function_adapter(target, span);
        smir::Expr::new(
            mir::Type::Function(target),
            smir::ExprKind::ClosureAlloc {
                class: self.dynamic_closure_adapters[adapter].class,
                captures: vec![value],
            },
        )
    }

    fn ensure_dynamic_function_adapter(
        &mut self,
        target: mir::FunctionTypeId,
        span: Span,
    ) -> mir::DynamicClosureAdapterId {
        if let Some(&adapter) = self.dynamic_adapter_by_target.get(&target) {
            return adapter;
        }
        if !self.function_bridge_targets.contains(&target) {
            self.function_bridge_targets.push(target);
        }
        let signature = self.shell.function_types[target].clone();
        let encoded = mir::encode_type(self.shell, &mir::Type::Function(target));
        let function = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: format!("$dynamic_adapter.{encoded}"),
            symbol: format!("scoop.$dynamic_adapter.{encoded}"),
            params: Vec::new(),
            return_ty: signature.return_type.clone(),
            body: mir::Body::unreachable(Arena::new()),
        });
        self.top_level.push(function);
        let invoke = self
            .closure_invokes
            .alloc(mir::ClosureInvokeFunction { function });
        let class = self.closure_classes.alloc(mir::ClosureClass {
            name: format!("$Closure$dynamic_adapter${encoded}"),
            function_type: target,
            invoke,
            captures: vec![mir::Field {
                name: "$source".to_string(),
                ty: mir::Type::Any,
            }],
            bridges: Vec::new(),
        });
        let adapter = self
            .dynamic_closure_adapters
            .alloc(mir::DynamicClosureAdapter { class, target });
        self.dynamic_adapter_by_target.insert(target, adapter);

        let mut locals = Arena::new();
        let closure = locals.alloc(mir::Local {
            name: "$closure".to_string(),
            ty: mir::Type::Function(target),
            mutable: false,
        });
        let mut params = vec![mir::Param {
            name: "$closure".to_string(),
            ty: mir::Type::Function(target),
            local: closure,
        }];
        let mut args = vec![smir::Expr::new(
            mir::Type::Any,
            smir::ExprKind::ClosureCapture {
                closure: Box::new(smir::Expr::local(closure, mir::Type::Function(target))),
                class,
                index: 0,
            },
        )];
        for (index, ty) in signature.parameter_types.iter().cloned().enumerate() {
            let local = locals.alloc(mir::Local {
                name: format!("arg{index}"),
                ty: ty.clone(),
                mutable: false,
            });
            params.push(mir::Param {
                name: format!("arg{index}"),
                ty: ty.clone(),
                local,
            });
            args.push(smir::Expr::local(local, ty));
        }
        let call = smir::Expr::new(
            signature.return_type.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::FunctionBridge {
                        function_type: target,
                    },
                    callee: mir::Callee::FunctionBridge(target),
                },
                args,
                return_ty: signature.return_type.clone(),
            }),
        );
        let statements = if signature.return_type == mir::Type::Unit {
            vec![
                smir::Statement {
                    kind: smir::StatementKind::Expr(call),
                    span,
                },
                smir::Statement {
                    kind: smir::StatementKind::Return { value: None },
                    span,
                },
            ]
        } else {
            vec![smir::Statement {
                kind: smir::StatementKind::Return { value: Some(call) },
                span,
            }]
        };
        self.functions[function].params = params;
        self.functions[function].body = cfg::lower(
            smir::Body { locals, statements },
            signature.return_type.clone(),
        );
        if signature.is_suspend {
            self.suspend_sources.push(SuspendSource {
                function,
                source_return: signature.return_type,
                instance: None,
            });
        }
        adapter
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
    fn drain_prelude(&mut self, span: Span, out: &mut Vec<smir::Statement>) {
        out.extend(
            self.prelude
                .drain(..)
                .map(|kind| smir::Statement { kind, span }),
        );
    }

    fn lower_statements(&mut self, statements: &[hir::Statement]) -> Vec<smir::Statement> {
        let mut out = Vec::new();
        for statement in statements {
            self.lower_statement(statement, &mut out);
        }
        out
    }

    fn lower_statement(&mut self, statement: &hir::Statement, out: &mut Vec<smir::Statement>) {
        let span = statement.span;
        let kind = match &statement.kind {
            hir::StatementKind::LocalFunction(_) => return,
            hir::StatementKind::Expr(expr) => {
                let expr = self.lower_expr(expr);
                self.drain_prelude(span, out);
                smir::StatementKind::Expr(expr)
            }
            hir::StatementKind::Return { value } => {
                let value = value.as_ref().map(|value| self.lower_expr(value));
                self.drain_prelude(span, out);
                smir::StatementKind::Return { value }
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
                        smir::StatementKind::Assign { local, value }
                    }
                    hir::AssignTarget::Global(global) => smir::StatementKind::GlobalAssign {
                        global: self.global_map[global],
                        value: self.lower_expr(value),
                    },
                    // `m[i] = v` (only `MutableArray`, checked at HIR).
                    // M8: the bounds check moved here from codegen —
                    // the array and the index are evaluated once into
                    // hidden locals and checked before the store; the
                    // value expression stays inside the `ArraySet`
                    // node and is evaluated after the check.
                    hir::AssignTarget::Index { array, index } => {
                        let array_ty = self.lower_type(array.ty);
                        let mir::Type::Class(array_type) = array_ty else {
                            unreachable!("an array assignment has an intrinsic class type")
                        };
                        let array_slot =
                            self.new_hidden("arr", mir::Type::Class(array_type), false);
                        let index_slot = self.new_hidden("idx", mir::Type::Int, false);
                        let array_value = self.lower_expr(array);
                        self.prelude.push(smir::StatementKind::ValDecl {
                            local: array_slot,
                            init: array_value,
                        });
                        let index_value = self.lower_expr(index);
                        self.prelude.push(smir::StatementKind::ValDecl {
                            local: index_slot,
                            init: index_value,
                        });
                        self.bounds_check(array_type, array_slot, index_slot, span);
                        let value = self.lower_expr(value);
                        smir::StatementKind::ArraySet {
                            array_type,
                            array: smir::Expr::local(array_slot, mir::Type::Class(array_type)),
                            index: smir::Expr::local(index_slot, mir::Type::Int),
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
                        smir::StatementKind::FieldSet {
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
                smir::StatementKind::If {
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
            hir::StatementKind::Try(try_) => smir::StatementKind::Try(self.lower_try(try_)),
            hir::StatementKind::Throw(expr) => {
                let value = self.lower_expr(expr);
                self.drain_prelude(span, out);
                smir::StatementKind::Throw(value)
            }
        };
        out.push(smir::Statement { kind, span });
    }

    /// `try` translates one-to-one: body, ordered catches (the catch
    /// type is resolved to the concrete MIR type), and the optional
    /// finally body.
    fn lower_try(&mut self, try_: &hir::Try) -> smir::Try {
        let body = self.lower_statements(&try_.body);
        let catches = try_
            .catches
            .iter()
            .map(|catch| smir::CatchClause {
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
        smir::Try {
            body,
            catches,
            finally_body,
        }
    }

    /// Construct and throw one compiler-known exception. The zero-argument
    /// constructor target is complete in LocalConcrete HIR, so this operation
    /// only transposes typed identities.
    fn throw_builtin(&mut self, exception: hir::CompilerException, span: Span) -> smir::Statement {
        let class = exception.class();
        let ctor = self.ctors[&class];
        let exception_ty = mir::Type::Class(self.class_map[&class]);
        smir::Statement {
            kind: smir::StatementKind::Throw(smir::Expr::new(
                exception_ty.clone(),
                smir::ExprKind::Call(smir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::User(ctor),
                    },
                    args: Vec::new(),
                    return_ty: exception_ty,
                }),
            )),
            span,
        }
    }

    /// The M8 array bounds check (DESIGN section 1), shared by
    /// `ArrayGet` and `ArraySet`:
    /// `if (index < 0 || index >= array.size) throw IndexOutOfBoundsException()`.
    /// CFG normalization expands the `||` into branch edges.
    fn bounds_check(
        &mut self,
        array_type: mir::ClassId,
        array: mir::LocalId,
        index: mir::LocalId,
        span: Span,
    ) {
        let out_of_bounds = logic(
            smir::LogicOp::Or,
            smir::Expr::new(
                mir::Type::Boolean,
                smir::ExprKind::Binary {
                    op: mir::BinOp::IntLt,
                    lhs: Box::new(smir::Expr::local(index, mir::Type::Int)),
                    rhs: Box::new(smir::Expr::int(0)),
                },
            ),
            smir::Expr::new(
                mir::Type::Boolean,
                smir::ExprKind::Binary {
                    op: mir::BinOp::IntGe,
                    lhs: Box::new(smir::Expr::local(index, mir::Type::Int)),
                    rhs: Box::new(smir::Expr::new(
                        mir::Type::Int,
                        smir::ExprKind::ArrayLen {
                            array_type,
                            operand: Box::new(smir::Expr::local(
                                array,
                                mir::Type::Class(array_type),
                            )),
                        },
                    )),
                },
            ),
        );
        let throw = self.throw_builtin(
            self.module.exception_core.index_out_of_bounds_exception,
            span,
        );
        self.prelude.push(smir::StatementKind::If {
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
        out: &mut Vec<smir::Statement>,
    ) {
        if let hir::Pattern::Binding { local } = pattern {
            let local = self.local_map[local];
            let init = self.lower_expr(init);
            self.drain_prelude(span, out);
            out.push(smir::Statement {
                kind: smir::StatementKind::ValDecl { local, init },
                span,
            });
            return;
        }
        let ty = self.lower_type(init.ty);
        let init = self.lower_expr(init);
        self.drain_prelude(span, out);
        let slot = self.new_hidden("bind", ty.clone(), false);
        out.push(smir::Statement {
            kind: smir::StatementKind::ValDecl { local: slot, init },
            span,
        });
        let mut path = Vec::new();
        let mut bindings = Vec::new();
        let cond = self.lower_pattern(pattern, slot, &mut path, &ty, &mut bindings);
        // hir-lower only emits irrefutable patterns (tuple / struct /
        // binding / wildcard) in destructuring declarations.
        debug_assert!(cond.is_none(), "destructuring patterns are irrefutable");
        for (local, init) in bindings {
            out.push(smir::Statement {
                kind: smir::StatementKind::ValDecl { local, init },
                span,
            });
        }
    }

    fn lower_while(
        &mut self,
        cond: &hir::Expr,
        body: &[hir::Statement],
        span: Span,
        out: &mut Vec<smir::Statement>,
    ) {
        let cond_mir = self.lower_expr(cond);
        if self.prelude.is_empty() {
            let body = self.lower_statements(body);
            out.push(smir::Statement {
                kind: smir::StatementKind::While {
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
        out.push(smir::Statement {
            kind: smir::StatementKind::ValDecl {
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
                .map(|kind| smir::Statement { kind, span }),
        );
        body.push(smir::Statement {
            kind: smir::StatementKind::Assign {
                local: cond_local,
                value: cond_again,
            },
            span,
        });
        out.push(smir::Statement {
            kind: smir::StatementKind::While {
                cond: smir::Expr::local(cond_local, mir::Type::Boolean),
                body,
            },
            span,
        });
    }

    /// `when` becomes a decision sequence (DESIGN 3.3): the subject is
    /// evaluated once into a hidden local, then the arms chain if/else
    /// tests; the `else` arm is the fallback.
    fn lower_when(&mut self, when: &hir::When, span: Span, out: &mut Vec<smir::Statement>) {
        let subject_ty = self.lower_type(when.subject.ty);
        let subject_init = self.lower_expr(&when.subject);
        self.drain_prelude(span, out);
        let subject = self.new_hidden("when", subject_ty.clone(), false);
        out.push(smir::Statement {
            kind: smir::StatementKind::ValDecl {
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
    ) -> Vec<smir::Statement> {
        let Some((arm, rest)) = arms.split_first() else {
            return else_body
                .map(|body| self.lower_statements(body))
                .unwrap_or_default();
        };
        let mut path = Vec::new();
        let mut bindings = Vec::new();
        let cond = self.lower_pattern(&arm.pattern, subject, &mut path, subject_ty, &mut bindings);
        let mut then: Vec<smir::Statement> = bindings
            .into_iter()
            .map(|(local, init)| smir::Statement {
                kind: smir::StatementKind::ValDecl { local, init },
                span: arm.span,
            })
            .collect();
        if let Some(guard) = &arm.guard {
            let guard_cond = self.lower_expr(guard);
            let guard_prelude = std::mem::take(&mut self.prelude);
            then.extend(guard_prelude.into_iter().map(|kind| smir::Statement {
                kind,
                span: arm.span,
            }));
            let body = self.lower_statements(&arm.body);
            let next = self.lower_arms(rest, subject, subject_ty, else_body);
            then.push(smir::Statement {
                kind: smir::StatementKind::If {
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
        vec![smir::Statement {
            kind: smir::StatementKind::If {
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
    /// order. CFG normalization expands the condition's `&&` chain, so a
    /// variant field is only extracted once its tag test has passed.
    fn lower_pattern(
        &mut self,
        pattern: &hir::Pattern,
        root: mir::LocalId,
        path: &mut Vec<Access>,
        ty: &mir::Type,
        bindings: &mut Vec<(mir::LocalId, smir::Expr)>,
    ) -> Option<smir::Expr> {
        match pattern {
            hir::Pattern::Binding { local } => {
                let init = self.accessed(root, path);
                bindings.push((self.local_map[local], init));
                None
            }
            hir::Pattern::Wildcard => None,
            hir::Pattern::Literal {
                value,
                equals,
                subject_ty,
            } => {
                debug_assert_eq!(self.lower_type(*subject_ty), *ty);
                let function = self.module.callable_function(*equals);
                let callee = self.instances.get(function).map_or_else(
                    || mir::Callee::User(self.function_map[&function]),
                    mir::Callee::Monomorphized,
                );
                Some(smir::Expr::new(
                    mir::Type::Boolean,
                    smir::ExprKind::Call(smir::Call {
                        target: mir::CallTarget {
                            kind: mir::CallKind::Direct,
                            callee,
                        },
                        args: vec![self.accessed(root, path), self.lower_expr(value)],
                        return_ty: mir::Type::Boolean,
                    }),
                ))
            }
            hir::Pattern::Variant {
                variant, fields, ..
            } => {
                let mir::Type::Enum(enum_id, _) = ty else {
                    unreachable!("a variant pattern matches an enum value")
                };
                let enum_id = *enum_id;
                let variant = variant.into_raw();
                let tag = smir::Expr::new(
                    mir::Type::Int,
                    smir::ExprKind::EnumTag(Box::new(self.accessed(root, path))),
                );
                let mut cond = smir::Expr::new(
                    mir::Type::Boolean,
                    smir::ExprKind::Binary {
                        op: mir::BinOp::IntEq,
                        lhs: Box::new(tag),
                        rhs: Box::new(smir::Expr::int(i64::from(variant))),
                    },
                );
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
                let mut cond: Option<smir::Expr> = None;
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
                let field_types: Vec<mir::Type> = self.structs.defs[*struct_id]
                    .declared_fields()
                    .iter()
                    .map(|field| field.ty.clone())
                    .collect();
                let mut cond: Option<smir::Expr> = None;
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

    fn lower_expr(&mut self, expr: &hir::Expr) -> smir::Expr {
        let ty = self.lower_type(expr.ty);
        let kind = match &expr.kind {
            hir::ExprKind::StringLiteral(value) => {
                // One global constant per literal occurrence, numbered
                // in order of appearance (deterministic).
                let symbol = format!("scoop.str.{}", self.strings.len());
                let id = self.strings.alloc(mir::StringConst {
                    value: value.clone(),
                    symbol,
                });
                smir::ExprKind::StringConst(id)
            }
            hir::ExprKind::IntLiteral(value) => smir::ExprKind::IntLiteral(*value),
            hir::ExprKind::BoolLiteral(value) => smir::ExprKind::BoolLiteral(*value),
            hir::ExprKind::UnitLiteral => smir::ExprKind::UnitLiteral,
            hir::ExprKind::TupleLiteral(elements) => {
                smir::ExprKind::TupleLiteral(elements.iter().map(|e| self.lower_expr(e)).collect())
            }
            hir::ExprKind::StructInit { args, .. } => {
                // The (possibly instantiated) struct def comes from
                // the expression's type: generic applications (M9)
                // resolve to their instance, plain structs to the
                // base definition.
                let mir::Type::Struct(struct_id) = self.lower_type(expr.ty) else {
                    unreachable!("a struct construction has a struct type")
                };
                smir::ExprKind::StructInit {
                    struct_id,
                    args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                }
            }
            // Class construction calls the class's constructor
            // function (`scoop.ctor.<Class>`); the raw allocation and
            // field initialization live inside it (see `lower_ctor`).
            hir::ExprKind::ClassInit { class_id, args } => {
                let ctor = self.ctors[class_id];
                smir::ExprKind::Call(smir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::User(ctor),
                    },
                    args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                    return_ty: self.lower_type(expr.ty),
                })
            }
            hir::ExprKind::VariantConstruct { variant, args, .. } => {
                smir::ExprKind::VariantConstruct {
                    variant: variant.into_raw(),
                    fields: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                }
            }
            hir::ExprKind::Local(local) => {
                let local = self.local_map[local];
                let narrowed = self.lower_type(expr.ty);
                if self.locals[local].ty == narrowed {
                    smir::ExprKind::Local(local)
                } else if let mir::Type::Function(function_type) = narrowed {
                    return self.adapt_checked_function_value(
                        smir::Expr::local(local, self.locals[local].ty.clone()),
                        function_type,
                        expr.span,
                    );
                } else {
                    smir::ExprKind::Retype {
                        operand: Box::new(smir::Expr::local(local, self.locals[local].ty.clone())),
                        ty: Box::new(narrowed),
                    }
                }
            }
            hir::ExprKind::ConstructorParam(parameter) => {
                smir::ExprKind::Local(self.constructor_param_map[parameter])
            }
            hir::ExprKind::GlobalRead(global) => {
                smir::ExprKind::GlobalRead(self.global_map[global])
            }
            hir::ExprKind::Capture(binding) => {
                if let Some(local) = self.current_local_capture_params.get(binding) {
                    return smir::Expr::new(ty, smir::ExprKind::Local(self.local_map[local]));
                }
                let class = self
                    .current_closure
                    .expect("capture reads only appear in closure invoke bodies");
                let index = self.closure_capture_indices[&(class, *binding)];
                let closure = self
                    .current_closure_local
                    .expect("a closure invoke body has its hidden receiver local");
                smir::ExprKind::ClosureCapture {
                    closure: Box::new(smir::Expr::local(closure, self.locals[closure].ty.clone())),
                    class,
                    index,
                }
            }
            hir::ExprKind::Lambda(id) => {
                let class = self.ensure_lambda_closure(*id);
                let sources: Vec<_> = self.module.lambdas[*id]
                    .captures
                    .iter()
                    .map(|capture| capture.source.clone())
                    .collect();
                smir::ExprKind::ClosureAlloc {
                    class,
                    captures: sources
                        .iter()
                        .map(|source| self.lower_expr(source))
                        .collect(),
                }
            }
            hir::ExprKind::AnonymousFunction(id) => {
                let class = self.ensure_anonymous_closure(*id);
                let sources: Vec<_> = self.module.anonymous_functions[*id]
                    .captures
                    .iter()
                    .map(|capture| capture.source.clone())
                    .collect();
                smir::ExprKind::ClosureAlloc {
                    class,
                    captures: sources
                        .iter()
                        .map(|source| self.lower_expr(source))
                        .collect(),
                }
            }
            hir::ExprKind::CallableReference(id) => {
                let class = self.ensure_reference_closure(*id);
                let reference = &self.module.callable_references[*id];
                let mut captures = Vec::with_capacity(
                    reference.captures.len()
                        + usize::from(matches!(
                            &reference.target,
                            hir::CallableReferenceTarget::BoundMember { .. }
                                | hir::CallableReferenceTarget::BoundExtension { .. }
                        )),
                );
                match &reference.target {
                    hir::CallableReferenceTarget::BoundMember { receiver, .. }
                    | hir::CallableReferenceTarget::BoundExtension { receiver, .. } => {
                        captures.push(self.lower_expr(receiver));
                    }
                    _ => {}
                }
                captures.extend(
                    reference
                        .captures
                        .iter()
                        .map(|capture| self.lower_expr(&capture.source)),
                );
                smir::ExprKind::ClosureAlloc { class, captures }
            }
            hir::ExprKind::FunctionCoercion {
                source,
                coercion,
                target_type,
            } => {
                let conversion = &self.module.function_coercions[*coercion];
                debug_assert_eq!(conversion.target, *target_type);
                let source_type = self.lower_function_type_id(conversion.source);
                let target_type = self.lower_function_type_id(*target_type);
                let value = self.lower_expr(source);
                return self.adapt_function_value(value, source_type, target_type, expr.span);
            }
            // Array nodes carry the exact concrete intrinsic class identity;
            // LIR never reconstructs it from an element layout or context.
            hir::ExprKind::ArrayLiteral(elements) => {
                let mir::Type::Class(array_type) = self.lower_type(expr.ty) else {
                    unreachable!("an array literal has an intrinsic class type")
                };
                smir::ExprKind::ArrayLiteral {
                    array_type,
                    elements: elements.iter().map(|e| self.lower_expr(e)).collect(),
                }
            }
            // Subscript read. M8: the bounds check moved here from
            // codegen — the array and the index are evaluated once
            // into hidden locals, then `IndexOutOfBoundsException`
            // throws when the index is out of range (the prelude
            // mechanism `!!` uses).
            hir::ExprKind::Index { receiver, index } => {
                let array_ty = self.lower_type(receiver.ty);
                let mir::Type::Class(array_type) = array_ty else {
                    unreachable!("an array subscript has an intrinsic class receiver")
                };
                let array_slot = self.new_hidden("arr", mir::Type::Class(array_type), false);
                let index_slot = self.new_hidden("idx", mir::Type::Int, false);
                let array = self.lower_expr(receiver);
                self.prelude.push(smir::StatementKind::ValDecl {
                    local: array_slot,
                    init: array,
                });
                let index = self.lower_expr(index);
                self.prelude.push(smir::StatementKind::ValDecl {
                    local: index_slot,
                    init: index,
                });
                self.bounds_check(array_type, array_slot, index_slot, expr.span);
                smir::ExprKind::ArrayGet {
                    array_type,
                    array: Box::new(smir::Expr::local(array_slot, mir::Type::Class(array_type))),
                    index: Box::new(smir::Expr::local(index_slot, mir::Type::Int)),
                }
            }
            hir::ExprKind::ArrayLen(operand) => {
                let mir::Type::Class(array_type) = self.lower_type(operand.ty) else {
                    unreachable!("array.size has an intrinsic class receiver")
                };
                smir::ExprKind::ArrayLen {
                    array_type,
                    operand: Box::new(self.lower_expr(operand)),
                }
            }
            hir::ExprKind::ArrayClone(operand) => {
                let mir::Type::Class(source_type) = self.lower_type(operand.ty) else {
                    unreachable!("an array conversion has an intrinsic class source")
                };
                let mir::Type::Class(target_type) = self.lower_type(expr.ty) else {
                    unreachable!("an array conversion has an intrinsic class target")
                };
                smir::ExprKind::ArrayClone {
                    source_type,
                    target_type,
                    operand: Box::new(self.lower_expr(operand)),
                }
            }
            hir::ExprKind::PtrFromUInt(operand) => {
                let mir::Type::Ptr(pointee) = self.lower_type(expr.ty) else {
                    unreachable!("PtrFromUInt has a pointer type")
                };
                smir::ExprKind::PtrFromUInt {
                    operand: Box::new(self.lower_expr(operand)),
                    pointee,
                }
            }
            hir::ExprKind::PtrToUInt(operand) => {
                smir::ExprKind::PtrToUInt(Box::new(self.lower_expr(operand)))
            }
            hir::ExprKind::PtrCast(operand) => {
                let mir::Type::Ptr(pointee) = self.lower_type(expr.ty) else {
                    unreachable!("PtrCast has a pointer type")
                };
                smir::ExprKind::PtrCast {
                    operand: Box::new(self.lower_expr(operand)),
                    pointee,
                }
            }
            hir::ExprKind::PtrLoad { pointer, offset } => {
                let hir::TypeKind::Ptr(pointee) = self.module.types[pointer.ty].kind else {
                    unreachable!("PtrLoad has a pointer operand")
                };
                smir::ExprKind::PtrLoad {
                    pointer: Box::new(self.lower_expr(pointer)),
                    pointee: Box::new(self.lower_type(pointee)),
                    offset: offset
                        .as_ref()
                        .map(|offset| Box::new(self.lower_expr(offset))),
                }
            }
            hir::ExprKind::PtrStore {
                pointer,
                offset,
                value,
            } => {
                let hir::TypeKind::Ptr(pointee) = self.module.types[pointer.ty].kind else {
                    unreachable!("PtrStore has a pointer operand")
                };
                smir::ExprKind::PtrStore {
                    pointer: Box::new(self.lower_expr(pointer)),
                    pointee: Box::new(self.lower_type(pointee)),
                    offset: offset
                        .as_ref()
                        .map(|offset| Box::new(self.lower_expr(offset))),
                    value: Box::new(self.lower_expr(value)),
                }
            }
            hir::ExprKind::PtrOffset {
                pointer,
                offset,
                subtract,
            } => {
                let hir::TypeKind::Ptr(pointee) = self.module.types[pointer.ty].kind else {
                    unreachable!("PtrOffset has a pointer operand")
                };
                smir::ExprKind::PtrOffset {
                    pointer: Box::new(self.lower_expr(pointer)),
                    pointee: Box::new(self.lower_type(pointee)),
                    offset: Box::new(self.lower_expr(offset)),
                    subtract: *subtract,
                }
            }
            hir::ExprKind::AddressOf(hir::Place::Local(local)) => {
                let local = self.local_map[local];
                smir::ExprKind::AddressOf {
                    local,
                    pointee: Box::new(self.locals[local].ty.clone()),
                }
            }
            hir::ExprKind::AddressOf(hir::Place::Global(global)) => smir::ExprKind::GlobalAddress {
                global: self.global_map[global],
                pointee: Box::new(self.lower_type(self.module.globals[*global].ty)),
            },
            hir::ExprKind::SizeOf(ty) => smir::ExprKind::SizeOf(Box::new(self.lower_type(*ty))),
            hir::ExprKind::AlignOf(ty) => smir::ExprKind::AlignOf(Box::new(self.lower_type(*ty))),
            hir::ExprKind::FunPtrNull => {
                let mir::Type::FunPtr(signature) = self.lower_type(expr.ty) else {
                    unreachable!("FunPtrNull has a FunPtr type")
                };
                smir::ExprKind::FunPtrNull(signature)
            }
            hir::ExprKind::FunctionAddress(function) => {
                let mir::Type::FunPtr(signature) = self.lower_type(expr.ty) else {
                    unreachable!("FunctionAddress has a FunPtr type")
                };
                let callback =
                    self.ensure_callback_bridge(self.function_map[function], signature, expr.span);
                smir::ExprKind::FunctionAddress { callback }
            }
            hir::ExprKind::ForeignCallbackRegister {
                registration,
                closure,
            } => {
                let bridge = self.ensure_foreign_callback_bridge(*registration, expr.span);
                smir::ExprKind::ForeignCallbackRegister {
                    bridge,
                    closure: Box::new(self.lower_expr(closure)),
                }
            }
            hir::ExprKind::ForeignCallbackOperation {
                operation,
                callback,
            } => smir::ExprKind::ForeignCallbackOperation {
                operation: match operation {
                    hir::ForeignCallbackOperation::Retain => mir::ForeignCallbackOperation::Retain,
                    hir::ForeignCallbackOperation::Release => {
                        mir::ForeignCallbackOperation::Release
                    }
                    hir::ForeignCallbackOperation::State => mir::ForeignCallbackOperation::State,
                    hir::ForeignCallbackOperation::Failure => {
                        mir::ForeignCallbackOperation::Failure
                    }
                },
                callback: Box::new(self.lower_expr(callback)),
                result_ty: Box::new(self.lower_type(expr.ty)),
            },
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
                smir::ExprKind::FieldAccess {
                    receiver: Box::new(self.lower_expr(receiver)),
                    index,
                }
            }
            hir::ExprKind::MethodCall {
                receiver,
                callee,
                args,
            } => return self.lower_method_call(receiver, *callee, args, expr.ty),
            // `Box` / `Unbox` / `is` stay dedicated MIR nodes; LIR
            // lowers them (the runtime box call, the payload load,
            // the `scoop_rt_is_instance` call). Boxing registers the
            // boxed value type (and the target interface) on the way.
            hir::ExprKind::Box(operand) => {
                let payload = self.lower_type(operand.ty);
                self.register_boxed(&payload, Some(expr.ty));
                smir::ExprKind::Box(Box::new(self.lower_expr(operand)))
            }
            // Smart casts unbox inline wherever the narrowed local is read
            // (e.g. as a field-access receiver). Every unbox is bound to a
            // typed hidden local so the synthetic expression and its result
            // local carry the same complete type.
            hir::ExprKind::Unbox(operand) => {
                let ty = self.lower_type(expr.ty);
                let operand = self.lower_expr(operand);
                let slot = self.new_hidden("ub", ty.clone(), false);
                self.prelude.push(smir::StatementKind::ValDecl {
                    local: slot,
                    init: smir::Expr::new(ty, smir::ExprKind::Unbox(Box::new(operand))),
                });
                smir::ExprKind::Local(slot)
            }
            hir::ExprKind::IsInstance { operand, check_ty } => {
                let check_ty = self.lower_type(*check_ty);
                self.register_check(&check_ty);
                smir::ExprKind::IsInstance {
                    operand: Box::new(self.lower_expr(operand)),
                    check_ty: Box::new(check_ty),
                }
            }
            hir::ExprKind::Cast { operand, optional } => {
                return self.lower_cast(operand, *optional, expr.ty, expr.span);
            }
            hir::ExprKind::Call { callee, args } => {
                return self.lower_call(*callee, args, expr.ty);
            }
            hir::ExprKind::LocalFunctionCall {
                callee,
                captures,
                args,
                ..
            } => {
                let callee = self.lower_user_callee(*callee);
                let call_args: Vec<_> = captures.iter().chain(args).collect();
                let return_ty = self.lower_type(expr.ty);
                return self.call(callee, &call_args, return_ty);
            }
            hir::ExprKind::CallableCall {
                callee,
                function_type,
                args,
            } => {
                let function_type = self.lower_function_type_id(*function_type);
                let mut call_args = Vec::with_capacity(args.len() + 1);
                call_args.push(self.lower_expr(callee));
                call_args.extend(args.iter().map(|arg| self.lower_expr(arg)));
                smir::ExprKind::Call(smir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Closure { function_type },
                        callee: mir::Callee::Closure(function_type),
                    },
                    args: call_args,
                    return_ty: self.lower_type(expr.ty),
                })
            }
            hir::ExprKind::Binary { op, lhs, rhs } => {
                return self.lower_binary(*op, lhs, rhs, expr.span);
            }
            hir::ExprKind::Unary { op, operand } => {
                let operand = Box::new(self.lower_expr(operand));
                let op = match op {
                    hir::UnOp::Neg => mir::UnOp::IntNeg,
                    hir::UnOp::Not => mir::UnOp::BoolNot,
                };
                smir::ExprKind::Unary { op, operand }
            }
            // The Option nodes (hir-lower's `?.` / `?:` / `!!`
            // desugars) become generic enum operations on core's
            // `Option` enum (DESIGN 3.3).
            hir::ExprKind::SomeWrap(operand) => {
                let (some, _) = self.option_variants;
                smir::ExprKind::VariantConstruct {
                    variant: some,
                    fields: vec![self.lower_expr(operand)],
                }
            }
            hir::ExprKind::NoneLiteral => {
                let (_, none) = self.option_variants;
                smir::ExprKind::VariantConstruct {
                    variant: none,
                    fields: Vec::new(),
                }
            }
            hir::ExprKind::IsSome(operand) => {
                let (some, _) = self.option_variants;
                let operand = self.lower_expr(operand);
                smir::ExprKind::Binary {
                    op: mir::BinOp::IntEq,
                    lhs: Box::new(smir::Expr::new(
                        mir::Type::Int,
                        smir::ExprKind::EnumTag(Box::new(operand)),
                    )),
                    rhs: Box::new(smir::Expr::int(i64::from(some))),
                }
            }
            hir::ExprKind::Unwrap {
                operand,
                trap_on_none,
            } => {
                let (some, _) = self.option_variants;
                if *trap_on_none {
                    return self.trapping_unwrap(operand, expr.ty, expr.span, some);
                } else {
                    // The surrounding control flow already guarantees
                    // `Some` (`?.` / `?:` desugars, the equality
                    // expansion).
                    smir::ExprKind::EnumField {
                        operand: Box::new(self.lower_expr(operand)),
                        variant: some,
                        index: 0,
                    }
                }
            }
        };
        smir::Expr::new(ty, kind)
    }

    fn ensure_callback_bridge(
        &mut self,
        source: mir::FunctionId,
        signature: mir::FunctionTypeId,
        span: Span,
    ) -> mir::CallbackBridgeId {
        if let Some(callback) = self.callback_by_target.get(&(source, signature)) {
            return *callback;
        }

        let callback_index = self.callback_bridges.len();
        let signature_def = self.shell.function_types[signature].clone();
        let source_name = self.functions[source].name.clone();
        let mut locals = Arena::new();
        let mut params = Vec::new();

        let result_storage = if signature_def.return_type == mir::Type::Unit {
            None
        } else {
            let ty = mir::Type::Ptr(Box::new(signature_def.return_type.clone()));
            let local = locals.alloc(mir::Local {
                name: "$result".to_string(),
                ty: ty.clone(),
                mutable: false,
            });
            params.push(mir::Param {
                name: "$result".to_string(),
                ty,
                local,
            });
            Some(local)
        };

        let mut args = Vec::with_capacity(signature_def.parameter_types.len());
        for (index, parameter_type) in signature_def.parameter_types.iter().enumerate() {
            let name = format!("$arg{index}");
            let pointer_type = mir::Type::Ptr(Box::new(parameter_type.clone()));
            let local = locals.alloc(mir::Local {
                name: name.clone(),
                ty: pointer_type.clone(),
                mutable: false,
            });
            params.push(mir::Param {
                name,
                ty: pointer_type.clone(),
                local,
            });
            args.push(mir::Expr::new(
                parameter_type.clone(),
                mir::ExprKind::PtrLoad {
                    pointer: Box::new(mir::Expr::local(local, pointer_type)),
                    pointee: Box::new(parameter_type.clone()),
                    offset: None,
                },
            ));
        }

        let call = mir::Call {
            target: mir::CallTarget {
                kind: mir::CallKind::Direct,
                callee: mir::Callee::User(source),
            },
            args,
        };
        let mut statements = Vec::new();
        if let Some(result_storage) = result_storage {
            let result = locals.alloc(mir::Local {
                name: "$value".to_string(),
                ty: signature_def.return_type.clone(),
                mutable: false,
            });
            statements.push(mir::Statement {
                kind: mir::StatementKind::Call(mir::CallEffect::Value {
                    destination: result,
                    call,
                }),
                span,
            });
            statements.push(mir::Statement {
                kind: mir::StatementKind::Expr(mir::Expr::new(
                    mir::Type::Unit,
                    mir::ExprKind::PtrStore {
                        pointer: Box::new(mir::Expr::local(
                            result_storage,
                            mir::Type::Ptr(Box::new(signature_def.return_type.clone())),
                        )),
                        pointee: Box::new(signature_def.return_type.clone()),
                        offset: None,
                        value: Box::new(mir::Expr::local(
                            result,
                            signature_def.return_type.clone(),
                        )),
                    },
                )),
                span,
            });
        } else {
            statements.push(mir::Statement {
                kind: mir::StatementKind::Call(mir::CallEffect::Unit(call)),
                span,
            });
        }

        let mut blocks = Arena::new();
        let entry = blocks.alloc(mir::BasicBlock {
            name: "entry".to_string(),
            statements,
            terminator: mir::Terminator::Return { value: None },
            unwind: None,
        });
        let bridge_function = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::NoGc,
            name: format!("callback bridge for {source_name}"),
            symbol: format!("scoop_callback_bridge_{callback_index}"),
            params,
            return_ty: mir::Type::Unit,
            body: mir::Body {
                locals,
                blocks,
                entry,
            },
        });
        self.top_level.push(bridge_function);
        let callback = self.callback_bridges.alloc(mir::CallbackBridge {
            source,
            signature,
            bridge_function,
        });
        self.callback_by_target
            .insert((source, signature), callback);
        callback
    }

    fn ensure_foreign_callback_bridge(
        &mut self,
        registration_id: hir::ForeignCallbackRegistrationId,
        span: Span,
    ) -> mir::ForeignCallbackBridgeId {
        if let Some(&bridge) = self.foreign_callback_by_registration.get(&registration_id) {
            return bridge;
        }

        let registration = self.module.foreign_callback_registrations[registration_id].clone();
        let native_signature = self.lower_function_type_id(registration.native_function_type);
        let managed_signature = self.lower_function_type_id(registration.managed_function_type);
        let callback = self.struct_map[&registration.callback];
        let signature = self.shell.function_types[managed_signature].clone();
        debug_assert!(!signature.is_suspend);

        let mut locals = Arena::new();
        let closure_ty = mir::Type::Function(managed_signature);
        let closure = locals.alloc(mir::Local {
            name: "$closure".to_string(),
            ty: closure_ty.clone(),
            mutable: false,
        });
        let result_pointer_ty = mir::Type::Ptr(Box::new(signature.return_type.clone()));
        let result_storage = locals.alloc(mir::Local {
            name: "$result".to_string(),
            ty: result_pointer_ty.clone(),
            mutable: false,
        });
        let opaque_pointer = mir::Type::Ptr(Box::new(mir::Type::Unit));
        let arguments_pointer_ty = mir::Type::Ptr(Box::new(opaque_pointer.clone()));
        let argument_storage = locals.alloc(mir::Local {
            name: "$arguments".to_string(),
            ty: arguments_pointer_ty.clone(),
            mutable: false,
        });
        let throwable =
            mir::Type::Class(self.class_map[&self.module.exception_core.throwable.class()]);
        let exception_pointer_ty = mir::Type::Ptr(Box::new(throwable.clone()));
        let exception_out = locals.alloc(mir::Local {
            name: "$exception".to_string(),
            ty: exception_pointer_ty.clone(),
            mutable: false,
        });

        let params = vec![
            mir::Param {
                name: "$closure".to_string(),
                ty: closure_ty.clone(),
                local: closure,
            },
            mir::Param {
                name: "$result".to_string(),
                ty: result_pointer_ty.clone(),
                local: result_storage,
            },
            mir::Param {
                name: "$arguments".to_string(),
                ty: arguments_pointer_ty.clone(),
                local: argument_storage,
            },
            mir::Param {
                name: "$exception".to_string(),
                ty: exception_pointer_ty.clone(),
                local: exception_out,
            },
        ];

        let mut call_args = vec![mir::Expr::local(closure, closure_ty.clone())];
        for (index, parameter_ty) in signature.parameter_types.iter().enumerate() {
            let raw = mir::Expr::new(
                opaque_pointer.clone(),
                mir::ExprKind::PtrLoad {
                    pointer: Box::new(mir::Expr::local(
                        argument_storage,
                        arguments_pointer_ty.clone(),
                    )),
                    pointee: Box::new(opaque_pointer.clone()),
                    offset: Some(Box::new(mir::Expr::int(index as i64))),
                },
            );
            let parameter_pointer = mir::Type::Ptr(Box::new(parameter_ty.clone()));
            call_args.push(mir::Expr::new(
                parameter_ty.clone(),
                mir::ExprKind::PtrLoad {
                    pointer: Box::new(mir::Expr::new(
                        parameter_pointer,
                        mir::ExprKind::PtrCast {
                            operand: Box::new(raw),
                            pointee: Box::new(parameter_ty.clone()),
                        },
                    )),
                    pointee: Box::new(parameter_ty.clone()),
                    offset: None,
                },
            ));
        }
        let call = mir::Call {
            target: mir::CallTarget {
                kind: mir::CallKind::Closure {
                    function_type: managed_signature,
                },
                callee: mir::Callee::Closure(managed_signature),
            },
            args: call_args,
        };

        let exception = locals.alloc(mir::Local {
            name: "$caught".to_string(),
            ty: throwable.clone(),
            mutable: false,
        });
        let statement = |kind| mir::Statement { kind, span };
        let mut blocks = Arena::new();
        let catch = blocks.alloc(mir::BasicBlock {
            name: "callback.failure".to_string(),
            statements: vec![
                statement(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
                    cleanup: false,
                })),
                statement(mir::StatementKind::Eh(mir::EhStatement::BeginCatch)),
                statement(mir::StatementKind::Call(mir::CallEffect::Value {
                    destination: exception,
                    call: mir::Call {
                        target: mir::CallTarget {
                            kind: mir::CallKind::Direct,
                            callee: mir::Callee::Runtime(mir::RuntimeFn::MaterializeException),
                        },
                        args: vec![mir::Expr::caught_exception()],
                    },
                })),
                statement(mir::StatementKind::Eh(mir::EhStatement::EndCatch)),
                statement(mir::StatementKind::Expr(mir::Expr::new(
                    mir::Type::Unit,
                    mir::ExprKind::PtrStore {
                        pointer: Box::new(mir::Expr::local(
                            exception_out,
                            exception_pointer_ty.clone(),
                        )),
                        pointee: Box::new(throwable.clone()),
                        offset: None,
                        value: Box::new(mir::Expr::local(exception, throwable.clone())),
                    },
                ))),
            ],
            terminator: mir::Terminator::Return {
                value: Some(mir::Expr::new(
                    mir::Type::UInt,
                    mir::ExprKind::IntLiteral(1),
                )),
            },
            unwind: None,
        });

        let mut success_statements = Vec::new();
        if signature.return_type == mir::Type::Unit {
            success_statements.push(statement(mir::StatementKind::Call(mir::CallEffect::Unit(
                call,
            ))));
        } else {
            let value = locals.alloc(mir::Local {
                name: "$value".to_string(),
                ty: signature.return_type.clone(),
                mutable: false,
            });
            success_statements.push(statement(mir::StatementKind::Call(
                mir::CallEffect::Value {
                    destination: value,
                    call,
                },
            )));
            success_statements.push(statement(mir::StatementKind::Expr(mir::Expr::new(
                mir::Type::Unit,
                mir::ExprKind::PtrStore {
                    pointer: Box::new(mir::Expr::local(result_storage, result_pointer_ty.clone())),
                    pointee: Box::new(signature.return_type.clone()),
                    offset: None,
                    value: Box::new(mir::Expr::local(value, signature.return_type.clone())),
                },
            ))));
        }
        let entry = blocks.alloc(mir::BasicBlock {
            name: "entry".to_string(),
            statements: success_statements,
            terminator: mir::Terminator::Return {
                value: Some(mir::Expr::new(
                    mir::Type::UInt,
                    mir::ExprKind::IntLiteral(0),
                )),
            },
            unwind: Some(catch),
        });
        let adapter_index = self.foreign_callback_adapters.len();
        let function = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: format!("foreign callback adapter {adapter_index}"),
            symbol: format!("scoop_foreign_callback_adapter_{adapter_index}"),
            params,
            return_ty: mir::Type::UInt,
            body: mir::Body {
                locals,
                blocks,
                entry,
            },
        });
        self.top_level.push(function);
        let adapter = self
            .foreign_callback_adapters
            .alloc(mir::ForeignCallbackAdapter {
                function,
                managed_signature,
            });
        let bridge = self
            .foreign_callback_bridges
            .alloc(mir::ForeignCallbackBridge {
                adapter,
                callback,
                native_signature,
                context_index: registration.context_index,
                mode: match registration.mode {
                    hir::ForeignCallbackMode::Reusable => mir::ForeignCallbackMode::Reusable,
                    hir::ForeignCallbackMode::OneShot => mir::ForeignCallbackMode::OneShot,
                },
            });
        self.foreign_callback_by_registration
            .insert(registration_id, bridge);
        bridge
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
    ) -> smir::Expr {
        let option_ty = self.lower_type(operand.ty);
        let payload_ty = self.lower_type(result_ty);
        let value = self.lower_expr(operand);
        let slot = self.new_hidden("opt", option_ty.clone(), false);
        let result = self.new_hidden("uw", payload_ty.clone(), false);
        let throw = self.throw_builtin(self.module.exception_core.unwrap_exception, span);
        self.prelude.push(smir::StatementKind::ValDecl {
            local: slot,
            init: value,
        });
        self.prelude.push(smir::StatementKind::If {
            cond: smir::Expr::new(
                mir::Type::Boolean,
                smir::ExprKind::Binary {
                    op: mir::BinOp::IntEq,
                    lhs: Box::new(smir::Expr::new(
                        mir::Type::Int,
                        smir::ExprKind::EnumTag(Box::new(smir::Expr::local(
                            slot,
                            option_ty.clone(),
                        ))),
                    )),
                    rhs: Box::new(smir::Expr::int(i64::from(some))),
                },
            ),
            then_body: vec![smir::Statement {
                kind: smir::StatementKind::ValDecl {
                    local: result,
                    init: smir::Expr::new(
                        payload_ty.clone(),
                        smir::ExprKind::EnumField {
                            operand: Box::new(smir::Expr::local(slot, option_ty)),
                            variant: some,
                            index: 0,
                        },
                    ),
                },
                span,
            }],
            else_body: Some(vec![throw]),
        });
        smir::Expr::local(result, payload_ty)
    }

    fn lower_call(
        &mut self,
        callable: hir::Callable,
        args: &[hir::Expr],
        result_ty: hir::TypeId,
    ) -> smir::Expr {
        let function = self.module.callable_function(callable);
        if let Some(protocol) = self
            .module
            .coroutine_protocol_for_function(function)
            .copied()
        {
            if function == protocol.start_coroutine {
                return self.lower_coroutine_start(protocol, args);
            }
            return self.lower_coroutine_suspend(protocol, args);
        }
        // `@Intrinsic` primitive functions (scoop.core, M7 DESIGN
        // section 2): handled up front — generic intrinsics (the M9
        // GC facilities) take this path too, before the generic-callee
        // arm below would reject their missing function-map entry.
        if let hir::FunctionKind::Intrinsic(intrinsic) = &self.module.functions[function].kind {
            return self.lower_intrinsic_call(intrinsic.kind, args, result_ty);
        }
        let callee = self.lower_user_callee(callable);
        let return_ty = self.lower_type(result_ty);
        self.call(callee, &args.iter().collect::<Vec<_>>(), return_ty)
    }

    fn lower_user_callee(&mut self, callable: hir::Callable) -> mir::Callee {
        let function = self.module.callable_function(callable);
        match &self.module.functions[function].kind {
            hir::FunctionKind::User(_) => self.instances.get(function).map_or_else(
                || mir::Callee::User(self.function_map[&function]),
                mir::Callee::Monomorphized,
            ),
            hir::FunctionKind::Extern(extern_id) => mir::Callee::Extern(self.extern_map[extern_id]),
            hir::FunctionKind::Intrinsic(_) => unreachable!("handled above"),
        }
    }

    fn lower_coroutine_start(
        &mut self,
        protocol: hir::CoroutineProtocol,
        args: &[hir::Expr],
    ) -> smir::Expr {
        let [task, completion] = args else {
            unreachable!("hir-lower validates startCoroutine's two parameters")
        };
        let result = self.lower_type(protocol.result_type);
        let task = self.lower_expr(task);
        let completion = self.lower_expr(completion);
        let task_interface = self.interfaces.mir_id(protocol.suspend_task);
        let continuation_interface = self.interfaces.mir_id(protocol.continuation);
        let (_, step_ty) = self
            .coroutines
            .step_for(&result, self.structs, self.enums, self.shell);
        let run = self.instances.get(protocol.suspend_task_run).unwrap();
        let resume = self.instances.get(protocol.continuation_resume).unwrap();
        let failure = self
            .instances
            .get(protocol.continuation_resume_with_exception)
            .unwrap();
        let throwable =
            mir::Type::Class(self.class_map[&self.module.exception_core.throwable.class()]);
        let helper = self.coroutines.start_helper(
            &result,
            task_interface,
            continuation_interface,
            run,
            resume,
            failure,
            &step_ty,
            throwable,
            self.functions,
            self.top_level,
            self.shell,
        );
        self.prelude.push(smir::StatementKind::Expr(smir::Expr::new(
            mir::Type::Unit,
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee: mir::Callee::User(helper),
                },
                args: vec![task, completion],
                return_ty: mir::Type::Unit,
            }),
        )));
        smir::Expr::unit()
    }

    fn lower_coroutine_suspend(
        &mut self,
        protocol: hir::CoroutineProtocol,
        args: &[hir::Expr],
    ) -> smir::Expr {
        let [registration] = args else {
            unreachable!("hir-lower validates suspendCoroutine's one parameter")
        };
        let result = self.lower_type(protocol.result_type);
        let registration_interface = self.interfaces.mir_id(protocol.suspend_registration);
        let register = self
            .instances
            .get(protocol.suspend_registration_register)
            .unwrap();
        smir::Expr::new(
            result.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Interface {
                        interface: registration_interface,
                        slot: 0,
                    },
                    callee: mir::Callee::CoroutineSuspend { register },
                },
                args: vec![self.lower_expr(registration)],
                return_ty: result,
            }),
        )
    }

    /// An `@Intrinsic` call: the typed intrinsic kind maps directly onto the
    /// runtime function (`print` / `println` themselves are ordinary
    /// overloaded core functions and take the `User` path in
    /// `lower_call`). Raw intrinsic names do not reach this stage.
    ///
    /// The M9 GC facilities (milestone9 DESIGN section 1, runtime spec
    /// 3.4) marshal between the raw machine word the runtime functions
    /// speak (`u64` addresses / handle values) and the Scoop-level
    /// handle aggregates: `pin` / `getGcHandle` wrap the word into the
    /// handle struct, `unpin` / `releaseGcHandle` unwrap it.
    fn lower_intrinsic_call(
        &mut self,
        kind: hir::IntrinsicFunctionKind,
        args: &[hir::Expr],
        result_ty: hir::TypeId,
    ) -> smir::Expr {
        let function = match kind {
            hir::IntrinsicFunctionKind::IntToString => mir::RuntimeFn::IntToString,
            hir::IntrinsicFunctionKind::BoolToString => mir::RuntimeFn::BoolToString,
            hir::IntrinsicFunctionKind::GcPinRaw => mir::RuntimeFn::Pin,
            hir::IntrinsicFunctionKind::GcUnpinRaw => mir::RuntimeFn::Unpin,
            hir::IntrinsicFunctionKind::GcGetHandleRaw => mir::RuntimeFn::GetHandle,
            hir::IntrinsicFunctionKind::GcReleaseHandleRaw => mir::RuntimeFn::ReleaseHandle,
            hir::IntrinsicFunctionKind::GcCollect => mir::RuntimeFn::GcCollect,
            hir::IntrinsicFunctionKind::GcStats => mir::RuntimeFn::GcStats,
            hir::IntrinsicFunctionKind::CoroutineStart
            | hir::IntrinsicFunctionKind::CoroutineSuspend => {
                unreachable!("coroutine intrinsics are lowered through the typed protocol")
            }
            hir::IntrinsicFunctionKind::Pointer(_)
            | hir::IntrinsicFunctionKind::ForeignCallbackRegister
            | hir::IntrinsicFunctionKind::ForeignCallbackRetain
            | hir::IntrinsicFunctionKind::ForeignCallbackRelease
            | hir::IntrinsicFunctionKind::ForeignCallbackState
            | hir::IntrinsicFunctionKind::ForeignCallbackFailure => {
                unreachable!("HIR expands this intrinsic before MIR")
            }
        };
        let callee = mir::Callee::Runtime(function);
        let return_ty = self.lower_type(result_ty);
        self.call(callee, &args.iter().collect::<Vec<_>>(), return_ty)
    }

    fn call(
        &mut self,
        callee: mir::Callee,
        args: &[&hir::Expr],
        return_ty: mir::Type,
    ) -> smir::Expr {
        smir::Expr::new(
            return_ty.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee,
                },
                args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                return_ty,
            }),
        )
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
        callable: hir::Callable,
        args: &[hir::Expr],
        result_ty: hir::TypeId,
    ) -> smir::Expr {
        let module = self.module;
        let function = module.callable_function(callable);
        let f = &module.functions[function];
        let callee = self.instances.get(function).map_or_else(
            || mir::Callee::User(self.function_map[&function]),
            mir::Callee::Monomorphized,
        );
        // The receiver's static type decides the dispatch kind.
        enum Receiver {
            Class(hir::ClassId),
            Interface(hir::TypeId),
            Value,
        }
        let receiver_kind = match &module.types[receiver.ty].kind {
            hir::TypeKind::Class(class) => Receiver::Class(*class),
            hir::TypeKind::Interface(..) => Receiver::Interface(receiver.ty),
            hir::TypeKind::Any => unreachable!("Any has no methods"),
            _ => Receiver::Value,
        };
        let generic_static_method = is_generic_method(f);
        let kind = if generic_static_method {
            // Generic member functions never participate in virtual
            // dispatch. Methods parameterized only by a generic interface
            // host are different: they still dispatch through that concrete
            // interface application's itable.
            mir::CallKind::Direct
        } else {
            match receiver_kind {
                Receiver::Class(_)
                    if f.method
                        .is_some_and(|method| method.modifier == hir::MethodModifier::Final) =>
                {
                    mir::CallKind::Direct
                }
                Receiver::Class(class) => {
                    let key = self.signature_key(f);
                    match self.method_slots[&self.class_map[&class]].get(&key) {
                        Some(&slot) => mir::CallKind::Virtual { slot },
                        None => mir::CallKind::Direct,
                    }
                }
                Receiver::Interface(interface_ty) => {
                    let mir::Type::Interface(interface) = self.lower_type(interface_ty) else {
                        unreachable!()
                    };
                    let (iface, _) = self.interfaces.source(interface);
                    let key = self.signature_key(f);
                    let mut slot = None;
                    for (index, sig) in module.interfaces[iface].methods.iter().enumerate() {
                        if self.sig_key(sig) == key {
                            slot = Some(index as u32);
                            break;
                        }
                    }
                    mir::CallKind::Interface {
                        interface,
                        slot: slot
                            .expect("hir-lower resolves interface calls to interface methods"),
                    }
                }
                Receiver::Value => mir::CallKind::Direct,
            }
        };
        let mut call_args = Vec::with_capacity(args.len() + 1);
        call_args.push(self.lower_expr(receiver));
        call_args.extend(args.iter().map(|arg| self.lower_expr(arg)));
        let return_ty = self.lower_type(result_ty);
        smir::Expr::new(
            return_ty.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget { kind, callee },
                args: call_args,
                return_ty,
            }),
        )
    }

    /// The callee's dispatch signature key (`name(<param encoding>)`,
    /// receiver excluded) — must agree with
    /// `Lowerer::fn_signature_key`, which keys the vtable slots.
    fn signature_key(&mut self, function: &hir::Function) -> String {
        let skip = usize::from(function.method.is_some());
        self.key_parts(short_name(&function.name), &function.params[skip..])
    }

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
        let declared: Vec<hir::TypeId> = match payload {
            mir::Type::Struct(mir_id) => {
                let hir_id = self.hir_struct(*mir_id);
                self.module.structs[hir_id].interfaces.clone()
            }
            mir::Type::Enum(mir_id, _) => {
                let hir_id = self.enums.hir_ids[mir_id];
                self.module.enums[hir_id].interfaces.clone()
            }
            // Tuples and primitives implement no interfaces.
            _ => Vec::new(),
        };
        let types = Types {
            module: self.module,
            struct_map: self.struct_map,
            class_map: self.class_map,
        };
        let mut covered = Vec::new();
        for interface_ty in declared {
            let lowered = types.lower(
                interface_ty,
                self.enums,
                self.structs,
                self.interfaces,
                self.shell,
            );
            let mir::Type::Interface(interface) = lowered else {
                unreachable!()
            };
            covered.push(interface);
        }
        if let Some(target) = target {
            if matches!(self.module.types[target].kind, hir::TypeKind::Interface(..)) {
                let mir::Type::Interface(interface) = self.lower_type(target) else {
                    unreachable!()
                };
                covered.push(interface);
            }
        }
        for iface in covered {
            let interfaces = &mut self.classes[class_id].interfaces;
            if !interfaces.contains(&iface) {
                interfaces.push(iface);
            }
        }
    }

    /// The HIR declaration behind a plain or instantiated MIR struct.
    fn hir_struct(&self, mir_id: mir::StructId) -> hir::StructId {
        if let Some((hir_id, _)) = self.structs.instances.get(&mir_id) {
            return *hir_id;
        }
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
        } else if let mir::Type::Function(function_type) = check_ty
            && !self.function_bridge_targets.contains(function_type)
        {
            self.function_bridge_targets.push(*function_type);
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
    ) -> smir::Expr {
        let target_hir = if optional {
            let hir::TypeKind::Enum(option) = self.module.types[expr_ty].kind else {
                unreachable!("an `as?` result is an Option<T>")
            };
            let (some, _) = self.module.enums[option]
                .option_variants
                .expect("an `as?` result is core's Option<T>");
            self.module.enums[option].variants[some.into_raw() as usize].fields[0].ty
        } else {
            expr_ty
        };
        let target = self.lower_type(target_hir);
        self.register_check(&target);
        let operand_ty = self.lower_type(operand.ty);
        let value = self.lower_expr(operand);
        let slot = self.new_hidden("cast", operand_ty.clone(), false);
        self.prelude.push(smir::StatementKind::ValDecl {
            local: slot,
            init: value,
        });
        let cond = match &target {
            mir::Type::Any => smir::Expr::bool(true),
            _ => smir::Expr::new(
                mir::Type::Boolean,
                smir::ExprKind::IsInstance {
                    operand: Box::new(smir::Expr::local(slot, operand_ty.clone())),
                    check_ty: Box::new(target.clone()),
                },
            ),
        };
        if !optional {
            let throw = self.throw_builtin(self.module.exception_core.class_cast_exception, span);
            self.prelude.push(smir::StatementKind::If {
                cond: smir::Expr::new(
                    mir::Type::Boolean,
                    smir::ExprKind::Unary {
                        op: mir::UnOp::BoolNot,
                        operand: Box::new(cond),
                    },
                ),
                then_body: vec![throw],
                else_body: None,
            });
            // A checked function view needs an exact target-ABI closure;
            // its invoke dispatches through the source closure's bridge
            // table. Value targets are unboxed by the outer HIR node.
            return match target {
                mir::Type::Function(function_type) => self.adapt_checked_function_value(
                    smir::Expr::local(slot, operand_ty),
                    function_type,
                    span,
                ),
                target @ (mir::Type::String
                | mir::Type::Class(_)
                | mir::Type::Interface(_)
                | mir::Type::Any) => smir::Expr::new(
                    target.clone(),
                    smir::ExprKind::Retype {
                        operand: Box::new(smir::Expr::local(slot, operand_ty)),
                        ty: Box::new(target),
                    },
                ),
                // hir-lower wraps a checked value-type cast in an outer
                // `Unbox`. Preserve the checked boxed/reference operand here;
                // the outer node is the sole operation that produces the
                // target value type.
                _ => smir::Expr::local(slot, operand_ty),
            };
        }
        let unboxed = match &target {
            mir::Type::Function(function_type) => self.adapt_checked_function_value(
                smir::Expr::local(slot, operand_ty.clone()),
                *function_type,
                span,
            ),
            mir::Type::Class(_) | mir::Type::Interface(_) | mir::Type::Any => smir::Expr::new(
                target.clone(),
                smir::ExprKind::Retype {
                    operand: Box::new(smir::Expr::local(slot, operand_ty.clone())),
                    ty: Box::new(target.clone()),
                },
            ),
            // Only `as?` unwraps here: hir-lower wraps a value-typed
            // `as` in a hir-level `Unbox(Cast)` node, so the payload
            // extraction for `as` happens when that outer `Unbox` is
            // lowered — adding another one here would double-unwrap.
            _ => smir::Expr::new(
                target.clone(),
                smir::ExprKind::Unbox(Box::new(smir::Expr::local(slot, operand_ty))),
            ),
        };
        let option_ty = self.lower_type(expr_ty);
        let (some, none) = self.option_variants;
        let result = self.new_hidden("cast", option_ty.clone(), true);
        let some_value = smir::Expr::new(
            option_ty.clone(),
            smir::ExprKind::VariantConstruct {
                variant: some,
                fields: vec![unboxed],
            },
        );
        let none_value = smir::Expr::new(
            option_ty.clone(),
            smir::ExprKind::VariantConstruct {
                variant: none,
                fields: Vec::new(),
            },
        );
        self.prelude.push(smir::StatementKind::If {
            cond,
            then_body: vec![smir::Statement {
                kind: smir::StatementKind::Assign {
                    local: result,
                    value: some_value,
                },
                span,
            }],
            else_body: Some(vec![smir::Statement {
                kind: smir::StatementKind::Assign {
                    local: result,
                    value: none_value,
                },
                span,
            }]),
        });
        smir::Expr::local(result, option_ty)
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
    ) -> smir::Expr {
        use mir::BinOp::*;
        match op {
            // String `+` is runtime concatenation (DESIGN 2.3); hir-lower
            // type checking makes both operands String here.
            hir::BinOp::Add if matches!(self.module.types[lhs.ty].kind, hir::TypeKind::String) => {
                self.call(
                    mir::Callee::Runtime(mir::RuntimeFn::StringConcat),
                    &[lhs, rhs],
                    mir::Type::String,
                )
            }
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
                self.prelude.push(smir::StatementKind::ValDecl {
                    local: lhs_slot,
                    init: lhs,
                });
                let rhs = self.lower_expr(rhs);
                self.prelude.push(smir::StatementKind::ValDecl {
                    local: rhs_slot,
                    init: rhs,
                });
                let throw =
                    self.throw_builtin(self.module.exception_core.arithmetic_exception, span);
                self.prelude.push(smir::StatementKind::If {
                    cond: smir::Expr::new(
                        mir::Type::Boolean,
                        smir::ExprKind::Binary {
                            op: mir::BinOp::IntEq,
                            lhs: Box::new(smir::Expr::local(rhs_slot, mir::Type::Int)),
                            rhs: Box::new(smir::Expr::int(0)),
                        },
                    ),
                    then_body: vec![throw],
                    else_body: None,
                });
                smir::Expr::new(
                    mir::Type::Int,
                    smir::ExprKind::Binary {
                        op: IntDiv,
                        lhs: Box::new(smir::Expr::local(lhs_slot, mir::Type::Int)),
                        rhs: Box::new(smir::Expr::local(rhs_slot, mir::Type::Int)),
                    },
                )
            }
            hir::BinOp::Lt => self.primitive(IntLt, lhs, rhs),
            hir::BinOp::Le => self.primitive(IntLe, lhs, rhs),
            hir::BinOp::Gt => self.primitive(IntGt, lhs, rhs),
            hir::BinOp::Ge => self.primitive(IntGe, lhs, rhs),
            hir::BinOp::Eq | hir::BinOp::Ne => {
                unreachable!("HIR resolves == and != to exact method calls")
            }
            // `===` / `!==`: reference identity — the primitive
            // comparison on the two pointers.
            hir::BinOp::RefEq => self.primitive(IntEq, lhs, rhs),
            hir::BinOp::RefNe => self.primitive(IntNe, lhs, rhs),
            // Short-circuit operators stay in the private construction
            // tree until CFG normalization emits their branch edges.
            hir::BinOp::And => self.short_circuit(smir::LogicOp::And, lhs, rhs),
            hir::BinOp::Or => self.short_circuit(smir::LogicOp::Or, lhs, rhs),
        }
    }

    fn primitive(&mut self, op: mir::BinOp, lhs: &hir::Expr, rhs: &hir::Expr) -> smir::Expr {
        let ty = match op {
            mir::BinOp::IntLt
            | mir::BinOp::IntLe
            | mir::BinOp::IntGt
            | mir::BinOp::IntGe
            | mir::BinOp::IntEq
            | mir::BinOp::IntNe
            | mir::BinOp::BoolEq
            | mir::BinOp::BoolNe => mir::Type::Boolean,
            mir::BinOp::IntAdd | mir::BinOp::IntSub | mir::BinOp::IntMul | mir::BinOp::IntDiv => {
                self.lower_type(lhs.ty)
            }
        };
        let lhs = Box::new(self.lower_expr(lhs));
        let rhs = Box::new(self.lower_expr(rhs));
        smir::Expr::new(ty, smir::ExprKind::Binary { op, lhs, rhs })
    }

    fn short_circuit(&mut self, op: smir::LogicOp, lhs: &hir::Expr, rhs: &hir::Expr) -> smir::Expr {
        let lhs = self.lower_expr(lhs);
        let rhs = self.lower_expr(rhs);
        logic(op, lhs, rhs)
    }

    /// Produce a pattern subject's nested value. Variant field extractions are
    /// guarded by the decision sequence that proved the active tag.
    fn accessed(&mut self, root: mir::LocalId, path: &[Access]) -> smir::Expr {
        let mut current_ty = self.locals[root].ty.clone();
        let mut lowered = smir::Expr::local(root, current_ty.clone());
        for access in path {
            let (next_ty, kind) = match access {
                Access::Field(index) => {
                    let next_ty = match &current_ty {
                        mir::Type::Tuple(elements) => elements[*index as usize].clone(),
                        mir::Type::Struct(struct_id) => self.structs.defs[*struct_id]
                            .declared_fields()[*index as usize]
                            .ty
                            .clone(),
                        _ => unreachable!("tuple/struct patterns only access aggregate fields"),
                    };
                    let kind = smir::ExprKind::FieldAccess {
                        receiver: Box::new(lowered),
                        index: *index,
                    };
                    (next_ty, kind)
                }
                Access::EnumField { variant, index } => {
                    let mir::Type::Enum(enum_id, _) = current_ty else {
                        unreachable!("variant pattern field access has an enum receiver")
                    };
                    let next_ty = self.enums.defs[enum_id].variants[*variant as usize].fields
                        [*index as usize]
                        .ty
                        .clone();
                    let kind = smir::ExprKind::EnumField {
                        operand: Box::new(lowered),
                        variant: *variant,
                        index: *index,
                    };
                    (next_ty, kind)
                }
            };
            current_ty = next_ty.clone();
            lowered = smir::Expr::new(next_ty, kind);
        }
        lowered
    }
}

/// Combine two conditions with `&&`; CFG normalization expands the short circuit.
fn and(lhs: smir::Expr, rhs: smir::Expr) -> smir::Expr {
    logic(smir::LogicOp::And, lhs, rhs)
}

fn logic(op: smir::LogicOp, lhs: smir::Expr, rhs: smir::Expr) -> smir::Expr {
    smir::Expr::new(
        mir::Type::Boolean,
        smir::ExprKind::ShortCircuit {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
    )
}

/// `Some(statements)` unless empty (an absent else branch).
fn non_empty(statements: Vec<smir::Statement>) -> Option<Vec<smir::Statement>> {
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
    use scoop_hir as hir;

    fn lower(module: &hir::Module) -> mir::Module {
        let concrete = scoop_hir_lower::concretize_export(module);
        super::lower(&concrete)
    }

    fn dump(module: &mir::Module) -> String {
        mir::dump(module)
            .lines()
            .filter(|line| {
                !(line.contains("class $") && line.contains("ExceptionProtocol"))
                    && !line.contains("class $ThrowableProtocol")
            })
            .map(|line| format!("{line}\n"))
            .collect()
    }

    fn visible_class_count(module: &mir::Module) -> usize {
        module
            .classes
            .iter()
            .filter(|(_, class)| {
                !class.name.ends_with("Protocol")
                    && matches!(
                        class.representation,
                        mir::ClassRepresentation::Declared { .. }
                    )
            })
            .count()
    }

    fn boxed_class<'a>(module: &'a mir::Module, name: &str) -> &'a mir::ClassDef {
        module
            .classes
            .iter()
            .map(|(_, class)| class)
            .find(|class| class.name == name)
            .unwrap_or_else(|| panic!("missing boxed class `{name}`"))
    }

    const SPAN: Span = Span { start: 0, end: 0 };

    fn type_param(name: impl Into<String>) -> hir::TypeParamDecl {
        hir::TypeParamDecl {
            id: hir::TypeParamId::from_raw(0),
            name: name.into(),
            variance: hir::Variance::Invariant,
            bounds: hir::TypeParamBounds::Unconstrained,
            span: SPAN,
        }
    }

    fn entry_statements(body: &mir::Body) -> &[mir::Statement] {
        &body.blocks[body.entry].statements
    }

    fn statement_call(statement: &mir::Statement) -> (&mir::Call, Option<mir::LocalId>) {
        let mir::StatementKind::Call(effect) = &statement.kind else {
            panic!("expected an explicit call effect")
        };
        match effect {
            mir::CallEffect::Unit(call) => (call, None),
            mir::CallEffect::Value { destination, call } => (call, Some(*destination)),
        }
    }

    fn block_named<'a>(body: &'a mir::Body, prefix: &str) -> &'a mir::BasicBlock {
        body.blocks
            .iter()
            .map(|(_, block)| block)
            .find(|block| block.name.starts_with(prefix))
            .unwrap_or_else(|| panic!("missing MIR block `{prefix}`"))
    }

    /// HIR module shell as hir-lower produces it: well-known types,
    /// core's managed `write` extern, two conversion intrinsics, and the ordinary
    /// `print` / `println` overloads (M7), plus core's `Option` enum
    /// allocated first.
    enum CanonicalTypePlan {
        Existing(hir::TypeId),
        Allocate,
    }

    struct Harness {
        types: Arena<hir::Type>,
        functions: Arena<hir::Function>,
        extern_functions: Arena<hir::ExternFunction>,
        generic_functions: Arena<hir::GenericFunction>,
        method_applications: Arena<hir::MethodApplication>,
        method_applications_by_key:
            HashMap<(hir::FunctionId, hir::MethodOwnerApplication), hir::MethodApplicationId>,
        generic_methods: Arena<hir::GenericMethod>,
        generic_method_applications: Arena<hir::GenericMethodApplication>,
        structs: Arena<hir::StructDecl>,
        struct_applications: Arena<hir::StructApplication>,
        struct_applications_by_key:
            HashMap<(hir::StructId, Vec<hir::TypeId>), hir::StructApplicationId>,
        enums: Arena<hir::EnumDecl>,
        enum_applications: Arena<hir::EnumApplication>,
        enum_applications_by_key: HashMap<(hir::EnumId, Vec<hir::TypeId>), hir::EnumApplicationId>,
        classes: Arena<hir::ClassDecl>,
        class_applications: Arena<hir::ClassApplication>,
        class_applications_by_key:
            HashMap<(hir::ClassId, Vec<hir::TypeId>), hir::ClassApplicationId>,
        interfaces: Arena<hir::InterfaceDecl>,
        interface_applications: Arena<hir::InterfaceApplication>,
        interface_methods: Arena<hir::InterfaceMethod>,
        interface_applications_by_key:
            HashMap<(hir::InterfaceId, Vec<hir::TypeId>), hir::InterfaceApplicationId>,
        top_level: Vec<hir::FunctionId>,
        unit: hir::TypeId,
        int: hir::TypeId,
        boolean: hir::TypeId,
        string: hir::TypeId,
        option_enum: hir::EnumId,
        write: Option<hir::FunctionId>,
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
        instantiations: Arena<hir::ResolvedGenericFunction>,
        uint: Option<hir::TypeId>,
        gc_core: Option<GcCore>,
        intrinsic_array: Option<hir::ClassId>,
        intrinsic_mutable_array: Option<hir::ClassId>,
    }

    /// core's GC facilities (M9), as `Harness::gc_core` declares them.
    #[derive(Clone, Copy)]
    struct GcCore {
        pinned_ptr: hir::StructId,
        gc_handle: hir::StructId,
        pin_raw: hir::FunctionId,
        unpin_raw: hir::FunctionId,
        get_handle_raw: hir::FunctionId,
        release_handle_raw: hir::FunctionId,
        gc_collect: hir::FunctionId,
        gc_stats: hir::FunctionId,
    }

    impl Harness {
        fn new() -> Self {
            let mut types = Arena::new();
            let unit = types.alloc(hir::Type::Unit);
            let int = types.alloc(hir::Type::Int);
            let boolean = types.alloc(hir::Type::Boolean);
            let string = types.alloc(hir::Type::String);
            let mut functions = Arena::new();
            // scoop.core's managed output extern and conversion intrinsics:
            // `@Extern(name = "scoop_rt_write", abi = "scoop") fun write(...)`,
            // `@Intrinsic("rt_int_to_string") fun intToString(...)`,
            // `@Intrinsic("rt_bool_to_string") fun boolToString(...)`.
            let extern_functions = Arena::new();
            let int_to_string = functions.alloc(hir::Function {
                name: "intToString".to_string(),
                genericity: hir::FunctionGenericity::Plain,
                is_suspend: false,
                params: Vec::new(),
                return_ty: string,
                attributes: hir::FunctionAttributes::default(),
                kind: hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
                    kind: hir::IntrinsicFunctionKind::IntToString,
                    provider: hir::IntrinsicProviderId::from_raw(0),
                }),
                method: None,
                span: SPAN,
            });
            let bool_to_string = functions.alloc(hir::Function {
                name: "boolToString".to_string(),
                genericity: hir::FunctionGenericity::Plain,
                is_suspend: false,
                params: Vec::new(),
                return_ty: string,
                attributes: hir::FunctionAttributes::default(),
                kind: hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
                    kind: hir::IntrinsicFunctionKind::BoolToString,
                    provider: hir::IntrinsicProviderId::from_raw(0),
                }),
                method: None,
                span: SPAN,
            });
            // scoop.core's `enum Option<T> { Some(T), None }`.
            let t = types.alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
            let mut enums = Arena::new();
            let mut enum_applications = Arena::new();
            let option_self_application = hir::EnumApplicationId::from_raw(0.into());
            let option_enum = enums.alloc(hir::EnumDecl {
                name: "Option".to_string(),
                self_application: option_self_application,
                type_params: vec![type_param("T")],
                no_gc: false,
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
                interface_implementations: Vec::new(),
                methods: Vec::new(),
                derived_equality: None,
                span: SPAN,
            });
            let option_self_type = hir::TypeId::from_raw((types.len() as u32).into());
            let actual_option_self_application = enum_applications.alloc(hir::EnumApplication {
                template: option_enum,
                arguments: vec![t],
                canonical_type: option_self_type,
            });
            assert_eq!(actual_option_self_application, option_self_application);
            let actual_option_self_type =
                types.alloc(hir::Type::Enum(actual_option_self_application));
            assert_eq!(actual_option_self_type, option_self_type);
            let mut enum_applications_by_key = HashMap::new();
            enum_applications_by_key.insert((option_enum, vec![t]), actual_option_self_application);
            Harness {
                types,
                functions,
                extern_functions,
                generic_functions: Arena::new(),
                method_applications: Arena::new(),
                method_applications_by_key: HashMap::new(),
                generic_methods: Arena::new(),
                generic_method_applications: Arena::new(),
                structs: Arena::new(),
                struct_applications: Arena::new(),
                struct_applications_by_key: HashMap::new(),
                enums,
                enum_applications,
                enum_applications_by_key,
                classes: Arena::new(),
                class_applications: Arena::new(),
                class_applications_by_key: HashMap::new(),
                interfaces: Arena::new(),
                interface_applications: Arena::new(),
                interface_methods: Arena::new(),
                interface_applications_by_key: HashMap::new(),
                top_level: vec![int_to_string, bool_to_string],
                unit,
                int,
                boolean,
                string,
                option_enum,
                write: None,
                int_to_string,
                bool_to_string,
                print_string: None,
                print_int: None,
                print_boolean: None,
                println_string: None,
                println_int: None,
                println_boolean: None,
                instantiations: Arena::new(),
                uint: None,
                gc_core: None,
                intrinsic_array: None,
                intrinsic_mutable_array: None,
            }
        }

        /// Adds core's managed `write` extern on first use so tests unrelated
        /// to output keep their MIR dumps focused on the feature under test.
        fn write(&mut self) -> hir::FunctionId {
            if let Some(id) = self.write {
                return id;
            }
            let extern_id = self.extern_functions.alloc(hir::ExternFunction {
                source_name: "write".to_string(),
                native_symbol: "scoop_rt_write".to_string(),
                library: String::new(),
                abi: hir::ExternAbi::Scoop,
                calling_convention: hir::CallingConvention::Cdecl,
                gc_effect: hir::GcEffect::Managed,
                safety: hir::Safety::Safe,
                params: vec![self.string],
                return_type: self.unit,
            });
            let id = self.functions.alloc(hir::Function {
                name: "write".to_string(),
                genericity: hir::FunctionGenericity::Plain,
                is_suspend: false,
                params: Vec::new(),
                return_ty: self.unit,
                attributes: hir::FunctionAttributes::default(),
                kind: hir::FunctionKind::Extern(extern_id),
                method: None,
                span: SPAN,
            });
            self.top_level.push(id);
            self.write = Some(id);
            id
        }

        /// core's `fun print(message: String) = write(message)`,
        /// created on first use (tests that never print keep core's
        /// overloads out of their MIR dumps).
        fn print_string(&mut self) -> hir::FunctionId {
            if let Some(id) = self.print_string {
                return id;
            }
            let (unit, string) = (self.unit, self.string);
            let write = self.write();
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
            let (write, int_to_string) = (self.write(), self.int_to_string);
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
            let (write, bool_to_string) = (self.write(), self.bool_to_string);
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
            let write = self.write();
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
            let application = self.enum_application(self.option_enum, vec![inner]);
            self.enum_applications[application].canonical_type
        }

        fn any(&mut self) -> hir::TypeId {
            self.types.alloc(hir::Type::Any)
        }

        fn class_ty(&mut self, id: hir::ClassId) -> hir::TypeId {
            let application = self.class_application(id, Vec::new());
            self.class_applications[application].canonical_type
        }

        fn interface_ty(&mut self, id: hir::InterfaceId) -> hir::TypeId {
            self.interface_app(id, Vec::new())
        }

        fn interface_app(
            &mut self,
            id: hir::InterfaceId,
            arguments: Vec<hir::TypeId>,
        ) -> hir::TypeId {
            assert_eq!(self.interfaces[id].type_params.len(), arguments.len());
            let application = self.interface_application(id, arguments);
            self.interface_applications[application].canonical_type
        }

        fn struct_ty(&mut self, id: hir::StructId) -> hir::TypeId {
            self.struct_app(id, Vec::new())
        }

        fn enum_ty(&mut self, id: hir::EnumId) -> hir::TypeId {
            assert!(self.enums[id].type_params.is_empty());
            let application = self.enum_application(id, Vec::new());
            self.enum_applications[application].canonical_type
        }

        fn struct_application_of(&self, ty: hir::TypeId) -> hir::StructApplicationId {
            let hir::Type::Struct(application) = self.types[ty] else {
                panic!("expected a struct application type")
            };
            application
        }

        fn enum_application_of(&self, ty: hir::TypeId) -> hir::EnumApplicationId {
            let hir::Type::Enum(application) = self.types[ty] else {
                panic!("expected an enum application type")
            };
            application
        }

        fn class_application_of(&self, ty: hir::TypeId) -> hir::ClassApplicationId {
            let hir::Type::Class(application) = self.types[ty] else {
                panic!("expected a class application type")
            };
            application
        }

        fn struct_application(
            &mut self,
            template: hir::StructId,
            arguments: Vec<hir::TypeId>,
        ) -> hir::StructApplicationId {
            let key = (template, arguments);
            if let Some(application) = self.struct_applications_by_key.get(&key) {
                return *application;
            }
            let canonical_type = hir::TypeId::from_raw((self.types.len() as u32).into());
            let application = self.struct_applications.alloc(hir::StructApplication {
                template,
                arguments: key.1.clone(),
                canonical_type,
                representation: hir::StructApplicationRepresentation::Declared,
            });
            let actual_type = self.types.alloc(hir::Type::Struct(application));
            assert_eq!(actual_type, canonical_type);
            self.struct_applications_by_key.insert(key, application);
            application
        }

        fn enum_application(
            &mut self,
            template: hir::EnumId,
            arguments: Vec<hir::TypeId>,
        ) -> hir::EnumApplicationId {
            let key = (template, arguments);
            if let Some(application) = self.enum_applications_by_key.get(&key) {
                return *application;
            }
            let canonical_type = hir::TypeId::from_raw((self.types.len() as u32).into());
            let application = self.enum_applications.alloc(hir::EnumApplication {
                template,
                arguments: key.1.clone(),
                canonical_type,
            });
            let actual_type = self.types.alloc(hir::Type::Enum(application));
            assert_eq!(actual_type, canonical_type);
            self.enum_applications_by_key.insert(key, application);
            application
        }

        fn declare_enum(
            &mut self,
            name: &str,
            type_params: Vec<hir::TypeParamDecl>,
            self_arguments: Vec<hir::TypeId>,
            variants: Vec<hir::Variant>,
        ) -> hir::EnumId {
            assert_eq!(type_params.len(), self_arguments.len());
            let self_application =
                hir::EnumApplicationId::from_raw((self.enum_applications.len() as u32).into());
            let enumeration = self.enums.alloc(hir::EnumDecl {
                name: name.to_string(),
                self_application,
                type_params,
                no_gc: false,
                variants,
                interfaces: Vec::new(),
                interface_implementations: Vec::new(),
                methods: Vec::new(),
                derived_equality: None,
                span: SPAN,
            });
            let actual = self.enum_application(enumeration, self_arguments);
            assert_eq!(actual, self_application);
            enumeration
        }

        fn class_application(
            &mut self,
            template: hir::ClassId,
            arguments: Vec<hir::TypeId>,
        ) -> hir::ClassApplicationId {
            let key = (template, arguments);
            if let Some(application) = self.class_applications_by_key.get(&key) {
                return *application;
            }
            let canonical_type = hir::TypeId::from_raw((self.types.len() as u32).into());
            let representation = match self.classes[template].representation {
                hir::ClassRepresentation::Declared(_) => {
                    hir::ClassApplicationRepresentation::Declared
                }
                hir::ClassRepresentation::Intrinsic(declaration) => {
                    hir::ClassApplicationRepresentation::Intrinsic(
                        declaration.kind.application(&key.1),
                    )
                }
            };
            let application = self.class_applications.alloc(hir::ClassApplication {
                template,
                arguments: key.1.clone(),
                canonical_type,
                representation,
            });
            let actual_type = self.types.alloc(hir::Type::Class(application));
            assert_eq!(actual_type, canonical_type);
            self.class_applications_by_key.insert(key, application);
            application
        }

        fn interface_application(
            &mut self,
            template: hir::InterfaceId,
            arguments: Vec<hir::TypeId>,
        ) -> hir::InterfaceApplicationId {
            let key = (template, arguments);
            if let Some(application) = self.interface_applications_by_key.get(&key) {
                return *application;
            }
            let canonical_type = hir::TypeId::from_raw((self.types.len() as u32).into());
            let application = self
                .interface_applications
                .alloc(hir::InterfaceApplication {
                    template,
                    arguments: key.1.clone(),
                    canonical_type,
                });
            let actual_type = self.types.alloc(hir::Type::Interface(application));
            assert_eq!(actual_type, canonical_type);
            self.interface_applications_by_key.insert(key, application);
            application
        }

        fn declare_interface(
            &mut self,
            name: &str,
            type_params: Vec<hir::TypeParamDecl>,
            self_arguments: Vec<hir::TypeId>,
            methods: Vec<hir::MethodSig>,
        ) -> hir::InterfaceId {
            assert_eq!(type_params.len(), self_arguments.len());
            let self_application = hir::InterfaceApplicationId::from_raw(
                (self.interface_applications.len() as u32).into(),
            );
            let interface = self.interfaces.alloc(hir::InterfaceDecl {
                name: name.to_string(),
                self_application,
                type_params,
                parents: Vec::new(),
                methods: Vec::new(),
                span: SPAN,
            });
            let actual = self.interface_application(interface, self_arguments);
            assert_eq!(actual, self_application);
            for method in methods {
                self.add_interface_method_signature(interface, method);
            }
            interface
        }

        fn add_interface_method_signature(
            &mut self,
            interface: hir::InterfaceId,
            method: hir::MethodSig,
        ) {
            assert!(method.type_params.is_empty());
            let declaration = self.interfaces[interface].clone();
            let owner = self.interface_applications[declaration.self_application].canonical_type;
            let mut locals = Arena::new();
            let this = locals.alloc(local("this", owner));
            let mut params = vec![param("this", owner, this)];
            for source in method.params {
                let local = locals.alloc(local(&source.name, source.ty));
                params.push(param(&source.name, source.ty, local));
            }
            let function = self.functions.alloc(hir::Function {
                name: format!("{}.{}", declaration.name, method.name),
                genericity: hir::FunctionGenericity::Plain,
                is_suspend: method.is_suspend,
                params,
                return_ty: method.return_ty,
                attributes: method.attributes,
                kind: hir::FunctionKind::User(hir::Body {
                    locals,
                    statements: Vec::new(),
                }),
                method: Some(hir::Method {
                    owner,
                    modifier: hir::MethodModifier::Abstract,
                    operator: None,
                }),
                span: method.span,
            });
            if !declaration.type_params.is_empty() {
                self.functions[function].genericity =
                    hir::FunctionGenericity::OwnerParameterizedMethod {
                        owner_parameters: declaration.type_params,
                        no_gc_type_params: Vec::new(),
                    };
            }
            let member = self.interface_methods.alloc(hir::InterfaceMethod {
                owner: interface,
                function,
            });
            self.interfaces[interface].methods.push(member);
        }

        fn interface(&mut self, name: &str, methods: &[&str]) -> hir::InterfaceId {
            let unit = self.unit;
            let methods = methods
                .iter()
                .map(|name| hir::MethodSig {
                    name: name.to_string(),
                    is_suspend: false,
                    attributes: hir::FunctionAttributes::default(),
                    type_params: Vec::new(),
                    params: Vec::new(),
                    return_ty: unit,
                    span: SPAN,
                })
                .collect();
            self.declare_interface(name, Vec::new(), Vec::new(), methods)
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
            let interfaces: Vec<_> = interfaces
                .iter()
                .map(|&interface| self.interface_ty(interface))
                .collect();
            let base = base.map(|(base, arguments)| (self.class_ty(base), arguments));
            self.declare_class(name, modifier, constructor, base, interfaces)
        }

        fn declare_class(
            &mut self,
            name: &str,
            modifier: hir::ClassModifier,
            constructor: &[(&str, hir::TypeId)],
            base_class: Option<(hir::TypeId, Vec<hir::Expr>)>,
            interfaces: Vec<hir::TypeId>,
        ) -> hir::ClassId {
            let interface_implementations = self.interface_implementation_shells(&interfaces);
            let self_application =
                hir::ClassApplicationId::from_raw((self.class_applications.len() as u32).into());
            let class = self.classes.alloc(hir::ClassDecl {
                modifier,
                name: name.to_string(),
                self_application,
                type_params: Vec::new(),
                representation: hir::ClassRepresentation::Declared(
                    constructor
                        .iter()
                        .enumerate()
                        .map(|(index, (name, ty))| hir::ConstructorField {
                            parameter: hir::ConstructorParamId::from_raw(index as u32),
                            name: name.to_string(),
                            ty: *ty,
                            mutable: false,
                        })
                        .collect(),
                ),
                base_class,
                interfaces,
                interface_implementations,
                methods: Vec::new(),
                span: SPAN,
            });
            let actual = self.class_application(class, Vec::new());
            assert_eq!(actual, self_application);
            class
        }

        /// A concrete zero-argument exception shell used by tests that
        /// exercise compiler-generated exception edges.
        fn exception(&mut self, name: &str) -> hir::ClassId {
            self.class(name, hir::ClassModifier::Final, &[], None, &[])
        }

        fn exception_target(&mut self, name: &str, include: bool) -> hir::CompilerException {
            let existing = self
                .classes
                .iter()
                .find_map(|(id, declaration)| (declaration.name == name).then_some(id));
            let class = if let Some(existing) = existing {
                existing
            } else if include {
                self.exception(name)
            } else {
                self.class(
                    &format!("${name}Protocol"),
                    hir::ClassModifier::Abstract,
                    &[],
                    None,
                    &[],
                )
            };
            hir::CompilerException {
                constructor: hir::ZeroArgClassConstructor { class },
            }
        }

        fn test_exception_core(&mut self, include: bool) -> hir::CompilerExceptionCore {
            hir::CompilerExceptionCore {
                throwable: self.exception_target("Throwable", include),
                unwrap_exception: self.exception_target("UnwrapException", include),
                class_cast_exception: self.exception_target("ClassCastException", include),
                arithmetic_exception: self.exception_target("ArithmeticException", include),
                index_out_of_bounds_exception: self
                    .exception_target("IndexOutOfBoundsException", include),
                illegal_state_exception: self.exception_target("IllegalStateException", include),
            }
        }

        /// A member function (kept out of `top_level`, as hir-lower
        /// does); member metadata carries the receiver type. The name is
        /// qualified `Owner.method`, as hir-lower names members.
        fn method_fn(
            &mut self,
            name: &str,
            method_of: hir::TypeId,
            params: Vec<hir::Param>,
            return_ty: hir::TypeId,
            body: hir::Body,
        ) -> hir::FunctionId {
            let genericity = match self.types[method_of] {
                hir::Type::Class(application) => {
                    let parameters = self.classes[self.class_applications[application].template]
                        .type_params
                        .clone();
                    if parameters.is_empty() {
                        hir::FunctionGenericity::Plain
                    } else {
                        hir::FunctionGenericity::OwnerParameterizedMethod {
                            owner_parameters: parameters,
                            no_gc_type_params: Vec::new(),
                        }
                    }
                }
                hir::Type::Struct(application) => {
                    let parameters = self.structs[self.struct_applications[application].template]
                        .type_params
                        .clone();
                    if parameters.is_empty() {
                        hir::FunctionGenericity::Plain
                    } else {
                        hir::FunctionGenericity::OwnerParameterizedMethod {
                            owner_parameters: parameters,
                            no_gc_type_params: Vec::new(),
                        }
                    }
                }
                hir::Type::Enum(application) => {
                    let parameters = self.enums[self.enum_applications[application].template]
                        .type_params
                        .clone();
                    if parameters.is_empty() {
                        hir::FunctionGenericity::Plain
                    } else {
                        hir::FunctionGenericity::OwnerParameterizedMethod {
                            owner_parameters: parameters,
                            no_gc_type_params: Vec::new(),
                        }
                    }
                }
                hir::Type::Interface(application) => {
                    let parameters = self.interfaces
                        [self.interface_applications[application].template]
                        .type_params
                        .clone();
                    if parameters.is_empty() {
                        hir::FunctionGenericity::Plain
                    } else {
                        hir::FunctionGenericity::OwnerParameterizedMethod {
                            owner_parameters: parameters,
                            no_gc_type_params: Vec::new(),
                        }
                    }
                }
                hir::Type::Any => hir::FunctionGenericity::Plain,
                _ => panic!("test harness methods have nominal owners"),
            };
            let function = self.functions.alloc(hir::Function {
                name: name.to_string(),
                genericity,
                is_suspend: false,
                params,
                return_ty,
                attributes: hir::FunctionAttributes::default(),
                kind: hir::FunctionKind::User(body),
                method: Some(hir::Method {
                    owner: method_of,
                    modifier: hir::MethodModifier::Open,
                    operator: None,
                }),
                span: SPAN,
            });
            match self.types[method_of] {
                hir::Type::Class(application) => self.classes
                    [self.class_applications[application].template]
                    .methods
                    .push(function),
                hir::Type::Struct(application) => self.structs
                    [self.struct_applications[application].template]
                    .methods
                    .push(function),
                hir::Type::Enum(application) => self.enums
                    [self.enum_applications[application].template]
                    .methods
                    .push(function),
                hir::Type::Interface(_) | hir::Type::Any => {}
                _ => unreachable!(),
            }
            function
        }

        fn method_application(&mut self, function: hir::FunctionId) -> hir::MethodApplicationId {
            let owner_ty = self.functions[function]
                .method
                .expect("test harness method has metadata")
                .owner;
            let owner = match self.types[owner_ty] {
                hir::Type::Class(application) => hir::MethodOwnerApplication::Class(application),
                hir::Type::Struct(application) => hir::MethodOwnerApplication::Struct(application),
                hir::Type::Enum(application) => hir::MethodOwnerApplication::Enum(application),
                hir::Type::Interface(application) => {
                    hir::MethodOwnerApplication::Interface(application)
                }
                hir::Type::Any => panic!("Any has no methods"),
                _ => panic!("test harness methods have nominal owners"),
            };
            let key = (function, owner);
            if let Some(&application) = self.method_applications_by_key.get(&key) {
                return application;
            }
            let application = self
                .method_applications
                .alloc(hir::MethodApplication { function, owner });
            self.method_applications_by_key.insert(key, application);
            application
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
            self.declare_struct(name, Vec::new(), Vec::new(), fields, interfaces)
        }

        fn declare_struct(
            &mut self,
            name: &str,
            type_params: Vec<hir::TypeParamDecl>,
            self_arguments: Vec<hir::TypeId>,
            fields: &[(&str, hir::TypeId)],
            interfaces: &[hir::InterfaceId],
        ) -> hir::StructId {
            assert_eq!(type_params.len(), self_arguments.len());
            let interfaces: Vec<_> = interfaces
                .iter()
                .map(|&interface| self.interface_ty(interface))
                .collect();
            let interface_implementations = self.interface_implementation_shells(&interfaces);
            let self_application =
                hir::StructApplicationId::from_raw((self.struct_applications.len() as u32).into());
            let strukt = self.structs.alloc(hir::StructDecl {
                name: name.to_string(),
                self_application,
                type_params,
                attributes: hir::StructAttributes::default(),
                representation: hir::StructRepresentation::Declared(
                    fields
                        .iter()
                        .map(|(name, ty)| hir::Field {
                            name: name.to_string(),
                            ty: *ty,
                        })
                        .collect(),
                ),
                interfaces,
                interface_implementations,
                methods: Vec::new(),
                derived_equality: None,
                span: SPAN,
            });
            let actual = self.struct_application(strukt, self_arguments);
            assert_eq!(actual, self_application);
            strukt
        }

        fn declare_fixed_intrinsic_struct(
            &mut self,
            name: &str,
            kind: hir::IntrinsicTypeKind,
            canonical_type: hir::TypeId,
        ) -> hir::StructId {
            let self_application =
                hir::StructApplicationId::from_raw((self.struct_applications.len() as u32).into());
            let declaration = hir::IntrinsicTypeDeclaration {
                kind,
                provider: hir::IntrinsicProviderId::from_raw(0),
            };
            let strukt = self.structs.alloc(hir::StructDecl {
                name: name.to_string(),
                self_application,
                type_params: Vec::new(),
                attributes: hir::StructAttributes::default(),
                representation: hir::StructRepresentation::Intrinsic(declaration),
                interfaces: Vec::new(),
                interface_implementations: Vec::new(),
                methods: Vec::new(),
                derived_equality: None,
                span: SPAN,
            });
            let representation = kind.application(&[]);
            let actual = self.struct_applications.alloc(hir::StructApplication {
                template: strukt,
                arguments: Vec::new(),
                canonical_type,
                representation: hir::StructApplicationRepresentation::Intrinsic(representation),
            });
            assert_eq!(actual, self_application);
            self.struct_applications_by_key
                .insert((strukt, Vec::new()), actual);
            strukt
        }

        fn declare_intrinsic_class(
            &mut self,
            name: &str,
            kind: hir::IntrinsicTypeKind,
            type_params: Vec<hir::TypeParamDecl>,
            self_arguments: Vec<hir::TypeId>,
            canonical_type_plan: CanonicalTypePlan,
        ) -> hir::ClassId {
            let self_application =
                hir::ClassApplicationId::from_raw((self.class_applications.len() as u32).into());
            let declaration = hir::IntrinsicTypeDeclaration {
                kind,
                provider: hir::IntrinsicProviderId::from_raw(0),
            };
            let class = self.classes.alloc(hir::ClassDecl {
                modifier: hir::ClassModifier::Final,
                name: name.to_string(),
                self_application,
                type_params,
                representation: hir::ClassRepresentation::Intrinsic(declaration),
                base_class: None,
                interfaces: Vec::new(),
                interface_implementations: Vec::new(),
                methods: Vec::new(),
                span: SPAN,
            });
            let representation = kind.application(&self_arguments);
            let (canonical_type, allocate_canonical_type) = match canonical_type_plan {
                CanonicalTypePlan::Existing(canonical_type) => (canonical_type, false),
                CanonicalTypePlan::Allocate => (
                    hir::TypeId::from_raw((self.types.len() as u32).into()),
                    true,
                ),
            };
            let actual = self.class_applications.alloc(hir::ClassApplication {
                template: class,
                arguments: self_arguments.clone(),
                canonical_type,
                representation: hir::ClassApplicationRepresentation::Intrinsic(representation),
            });
            assert_eq!(actual, self_application);
            if allocate_canonical_type {
                let allocated = self.types.alloc(hir::Type::Class(actual));
                assert_eq!(allocated, canonical_type);
            }
            self.class_applications_by_key
                .insert((class, self_arguments), actual);
            class
        }

        fn interface_implementation_shells(
            &self,
            interfaces: &[hir::TypeId],
        ) -> Vec<hir::InterfaceImplementation> {
            interfaces
                .iter()
                .map(|&interface| {
                    let hir::Type::Interface(application) = self.types[interface] else {
                        panic!("test harness interface lists are fully applied")
                    };
                    let template = self.interface_applications[application].template;
                    hir::InterfaceImplementation {
                        interface: application,
                        methods: self.interfaces[template]
                            .methods
                            .iter()
                            .map(|&member| hir::InterfaceMethodImplementation {
                                member,
                                target: hir::InterfaceImplementationTarget::Subclass,
                            })
                            .collect(),
                    }
                })
                .collect()
        }

        /// The `UInt` well-known type (M9, spec 11.2), allocated on
        /// first use.
        fn uint(&mut self) -> hir::TypeId {
            if let Some(ty) = self.uint {
                return ty;
            }
            let ty = self.types.alloc(hir::Type::UInt);
            self.uint = Some(ty);
            ty
        }

        /// Intern a generic struct application type.
        fn struct_app(&mut self, struct_id: hir::StructId, args: Vec<hir::TypeId>) -> hir::TypeId {
            assert_eq!(self.structs[struct_id].type_params.len(), args.len());
            let application = self.struct_application(struct_id, args);
            self.struct_applications[application].canonical_type
        }

        /// core's GC facilities (M12): `PinnedPtr<T>` / `GcHandle<T>`
        /// and the six low-level runtime intrinsics, created on first use.
        fn gc_core(&mut self) -> GcCore {
            if let Some(core) = self.gc_core {
                return core;
            }
            let uint = self.uint();
            let unit = self.unit;
            let t = self
                .types
                .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
            let pinned_ptr = self.declare_struct(
                "PinnedPtr",
                vec![type_param("T")],
                vec![t],
                &[("raw", uint)],
                &[],
            );
            let gc_handle = self.declare_struct(
                "GcHandle",
                vec![type_param("T")],
                vec![t],
                &[("raw", uint)],
                &[],
            );
            let mut dummy_locals = Arena::new();
            let mut intrinsic = |name: &str,
                                 intrinsic: &str,
                                 type_params: Vec<String>,
                                 params: Vec<(&str, hir::TypeId)>,
                                 return_ty: hir::TypeId| {
                let generic = !type_params.is_empty();
                let type_params = type_params.into_iter().map(type_param).collect();
                let id = self.functions.alloc(hir::Function {
                    name: name.to_string(),
                    genericity: hir::FunctionGenericity::Plain,
                    is_suspend: false,
                    params: params
                        .into_iter()
                        .map(|(name, ty)| hir::Param {
                            name: name.to_string(),
                            ty,
                            local: dummy_locals.alloc(hir::Local {
                                binding: hir::BindingId::from_raw(dummy_locals.len() as u32),
                                name: name.to_string(),
                                ty,
                                mutable: false,
                            }),
                        })
                        .collect(),
                    return_ty,
                    attributes: hir::FunctionAttributes::default(),
                    kind: hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
                        kind: hir::intrinsic_spec(intrinsic)
                            .expect("test intrinsic is registered")
                            .kind,
                        provider: hir::IntrinsicProviderId::from_raw(0),
                    }),
                    method: None,
                    span: SPAN,
                });
                if generic {
                    self.register_generic(id, type_params);
                }
                self.top_level.push(id);
                id
            };
            let type_params = vec!["T".to_string()];
            let pin_raw = intrinsic(
                "_pin",
                "gc_pin_raw",
                type_params.clone(),
                vec![("v", t)],
                uint,
            );
            let unpin_raw = intrinsic(
                "_unpin",
                "gc_unpin_raw",
                type_params.clone(),
                vec![("raw", uint)],
                t,
            );
            let get_handle_raw = intrinsic(
                "_getGcHandle",
                "gc_get_handle_raw",
                type_params.clone(),
                vec![("v", t)],
                uint,
            );
            let release_handle_raw = intrinsic(
                "_releaseGcHandle",
                "gc_release_handle_raw",
                type_params,
                vec![("raw", uint)],
                t,
            );
            let gc_collect = intrinsic("gcCollect", "rt_gc_collect", vec![], vec![], unit);
            let gc_stats = intrinsic("gcStats", "rt_gc_stats", vec![], vec![], uint);
            let core = GcCore {
                pinned_ptr,
                gc_handle,
                pin_raw,
                unpin_raw,
                get_handle_raw,
                release_handle_raw,
                gc_collect,
                gc_stats,
            };
            self.gc_core = Some(core);
            core
        }

        fn tuple(&mut self, elements: &[hir::TypeId]) -> hir::TypeId {
            self.types.alloc(hir::Type::Tuple(elements.to_vec()))
        }

        fn intrinsic_array_class(&mut self, kind: hir::IntrinsicTypeKind) -> hir::ClassId {
            let existing = match kind {
                hir::IntrinsicTypeKind::Array => self.intrinsic_array,
                hir::IntrinsicTypeKind::MutableArray => self.intrinsic_mutable_array,
                _ => unreachable!("array helper accepts only intrinsic array families"),
            };
            if let Some(class) = existing {
                return class;
            }
            let parameter = self
                .types
                .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
            let class = self.declare_intrinsic_class(
                kind.source_name(),
                kind,
                vec![type_param("T")],
                vec![parameter],
                CanonicalTypePlan::Allocate,
            );
            match kind {
                hir::IntrinsicTypeKind::Array => self.intrinsic_array = Some(class),
                hir::IntrinsicTypeKind::MutableArray => self.intrinsic_mutable_array = Some(class),
                _ => unreachable!("array helper accepts only intrinsic array families"),
            }
            class
        }

        fn array(&mut self, element: hir::TypeId) -> hir::TypeId {
            let class = self.intrinsic_array_class(hir::IntrinsicTypeKind::Array);
            let application = self.class_application(class, vec![element]);
            self.class_applications[application].canonical_type
        }

        fn mutable_array(&mut self, element: hir::TypeId) -> hir::TypeId {
            let class = self.intrinsic_array_class(hir::IntrinsicTypeKind::MutableArray);
            let application = self.class_application(class, vec![element]);
            self.class_applications[application].canonical_type
        }

        fn user_fn(&mut self, name: &str, body: hir::Body) -> hir::FunctionId {
            let unit = self.unit;
            self.user_fn_full(name, Vec::new(), Vec::new(), unit, body)
        }

        fn register_generic(
            &mut self,
            function: hir::FunctionId,
            parameters: Vec<hir::TypeParamDecl>,
        ) -> hir::GenericFunctionId {
            if let hir::FunctionGenericity::Generic {
                definition,
                parameters: existing,
            } = &self.functions[function].genericity
            {
                assert_eq!(existing, &parameters);
                return *definition;
            }
            let generic = self.generic_functions.alloc(hir::GenericFunction {
                function,
                no_gc_type_params: Vec::new(),
            });
            self.functions[function].genericity = hir::FunctionGenericity::Generic {
                definition: generic,
                parameters,
            };
            generic
        }

        fn user_fn_full(
            &mut self,
            name: &str,
            type_params: Vec<String>,
            params: Vec<hir::Param>,
            return_ty: hir::TypeId,
            body: hir::Body,
        ) -> hir::FunctionId {
            let generic = !type_params.is_empty();
            let type_params = type_params.into_iter().map(type_param).collect();
            let id = self.functions.alloc(hir::Function {
                name: name.to_string(),
                genericity: hir::FunctionGenericity::Plain,
                is_suspend: false,
                params,
                return_ty,
                attributes: hir::FunctionAttributes::default(),
                kind: hir::FunctionKind::User(body),
                method: None,
                span: SPAN,
            });
            if generic {
                self.register_generic(id, type_params);
            }
            self.top_level.push(id);
            id
        }

        fn instantiate(
            &mut self,
            function: hir::FunctionId,
            type_args: Vec<hir::TypeId>,
        ) -> hir::ResolvedGenericFunctionId {
            let Some(generic) = self.functions[function].generic_definition() else {
                panic!("generic test function must be registered")
            };
            if let Some((id, _)) = self.instantiations.iter().find(|(_, resolved)| {
                resolved.generic == generic && resolved.type_args == type_args
            }) {
                return id;
            }
            self.instantiations
                .alloc(hir::ResolvedGenericFunction { generic, type_args })
        }

        fn test_coroutine_core(&mut self, throwable: hir::ClassId) -> hir::CoroutineCore {
            let throwable_ty = self.class_ty(throwable);
            let t = self
                .types
                .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
            let type_param = || hir::TypeParamDecl {
                id: hir::TypeParamId::from_raw(0),
                name: "T".to_string(),
                variance: hir::Variance::Invariant,
                bounds: hir::TypeParamBounds::Unconstrained,
                span: SPAN,
            };
            let continuation =
                self.declare_interface("Continuation", vec![type_param()], vec![t], Vec::new());
            let continuation_ty = self.interface_applications
                [self.interfaces[continuation].self_application]
                .canonical_type;
            let mut resume_locals = Arena::new();
            let resume_value = resume_locals.alloc(local("value", t));
            let continuation_resume = self.functions.alloc(hir::Function {
                name: "Continuation.resume".to_string(),
                genericity: hir::FunctionGenericity::Plain,
                is_suspend: false,
                params: vec![param("value", t, resume_value)],
                return_ty: self.unit,
                attributes: hir::FunctionAttributes::default(),
                kind: hir::FunctionKind::User(hir::Body {
                    locals: resume_locals,
                    statements: Vec::new(),
                }),
                method: Some(hir::Method {
                    owner: continuation_ty,
                    modifier: hir::MethodModifier::Abstract,
                    operator: None,
                }),
                span: SPAN,
            });
            let mut failure_locals = Arena::new();
            let failure = failure_locals.alloc(local("exception", throwable_ty));
            let continuation_resume_with_exception = self.functions.alloc(hir::Function {
                name: "Continuation.resumeWithException".to_string(),
                genericity: hir::FunctionGenericity::Plain,
                is_suspend: false,
                params: vec![param("exception", throwable_ty, failure)],
                return_ty: self.unit,
                attributes: hir::FunctionAttributes::default(),
                kind: hir::FunctionKind::User(hir::Body {
                    locals: failure_locals,
                    statements: Vec::new(),
                }),
                method: Some(hir::Method {
                    owner: continuation_ty,
                    modifier: hir::MethodModifier::Abstract,
                    operator: None,
                }),
                span: SPAN,
            });
            for method in [
                hir::MethodSig {
                    name: "resume".to_string(),
                    is_suspend: false,
                    attributes: hir::FunctionAttributes::default(),
                    type_params: Vec::new(),
                    params: vec![param("value", t, resume_value)],
                    return_ty: self.unit,
                    span: SPAN,
                },
                hir::MethodSig {
                    name: "resumeWithException".to_string(),
                    is_suspend: false,
                    attributes: hir::FunctionAttributes::default(),
                    type_params: Vec::new(),
                    params: vec![param("exception", throwable_ty, failure)],
                    return_ty: self.unit,
                    span: SPAN,
                },
            ] {
                self.add_interface_method_signature(continuation, method);
            }

            let suspend_task = self.declare_interface(
                "SuspendTask",
                vec![type_param()],
                vec![t],
                vec![hir::MethodSig {
                    name: "run".to_string(),
                    is_suspend: true,
                    attributes: hir::FunctionAttributes::default(),
                    type_params: Vec::new(),
                    params: Vec::new(),
                    return_ty: t,
                    span: SPAN,
                }],
            );
            let suspend_task_ty = self.interface_applications
                [self.interfaces[suspend_task].self_application]
                .canonical_type;
            let suspend_task_run = self.functions.alloc(hir::Function {
                name: "SuspendTask.run".to_string(),
                genericity: hir::FunctionGenericity::Plain,
                is_suspend: true,
                params: Vec::new(),
                return_ty: t,
                attributes: hir::FunctionAttributes::default(),
                kind: hir::FunctionKind::User(hir::Body {
                    locals: Arena::new(),
                    statements: Vec::new(),
                }),
                method: Some(hir::Method {
                    owner: suspend_task_ty,
                    modifier: hir::MethodModifier::Abstract,
                    operator: None,
                }),
                span: SPAN,
            });

            let suspend_registration = self.declare_interface(
                "SuspendRegistration",
                vec![type_param()],
                vec![t],
                vec![hir::MethodSig {
                    name: "register".to_string(),
                    is_suspend: false,
                    attributes: hir::FunctionAttributes::default(),
                    type_params: Vec::new(),
                    params: Vec::new(),
                    return_ty: self.unit,
                    span: SPAN,
                }],
            );
            let suspend_registration_ty = self.interface_applications
                [self.interfaces[suspend_registration].self_application]
                .canonical_type;
            let suspend_registration_register = self.functions.alloc(hir::Function {
                name: "SuspendRegistration.register".to_string(),
                genericity: hir::FunctionGenericity::Plain,
                is_suspend: false,
                params: Vec::new(),
                return_ty: self.unit,
                attributes: hir::FunctionAttributes::default(),
                kind: hir::FunctionKind::User(hir::Body {
                    locals: Arena::new(),
                    statements: Vec::new(),
                }),
                method: Some(hir::Method {
                    owner: suspend_registration_ty,
                    modifier: hir::MethodModifier::Abstract,
                    operator: None,
                }),
                span: SPAN,
            });

            for function in [
                continuation_resume,
                continuation_resume_with_exception,
                suspend_task_run,
                suspend_registration_register,
            ] {
                self.functions[function].genericity =
                    hir::FunctionGenericity::OwnerParameterizedMethod {
                        owner_parameters: vec![type_param()],
                        no_gc_type_params: Vec::new(),
                    };
            }
            let start_coroutine = self.functions.alloc(hir::Function {
                name: "startCoroutine".to_string(),
                genericity: hir::FunctionGenericity::Plain,
                is_suspend: false,
                params: Vec::new(),
                return_ty: self.unit,
                attributes: hir::FunctionAttributes::default(),
                kind: hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
                    kind: hir::IntrinsicFunctionKind::CoroutineStart,
                    provider: hir::IntrinsicProviderId::from_raw(0),
                }),
                method: None,
                span: SPAN,
            });
            let suspend_coroutine = self.functions.alloc(hir::Function {
                name: "suspendCoroutine".to_string(),
                genericity: hir::FunctionGenericity::Plain,
                is_suspend: true,
                params: Vec::new(),
                return_ty: t,
                attributes: hir::FunctionAttributes::default(),
                kind: hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
                    kind: hir::IntrinsicFunctionKind::CoroutineSuspend,
                    provider: hir::IntrinsicProviderId::from_raw(0),
                }),
                method: None,
                span: SPAN,
            });
            for function in [start_coroutine, suspend_coroutine] {
                self.register_generic(function, vec![type_param()]);
                self.top_level.push(function);
            }
            hir::CoroutineCore {
                continuation,
                continuation_resume,
                continuation_resume_with_exception,
                suspend_task,
                suspend_task_run,
                suspend_registration,
                suspend_registration_register,
                start_coroutine,
                suspend_coroutine,
            }
        }

        fn finish(self, entry: hir::FunctionId) -> hir::Module {
            self.finish_with_coroutine_core(entry, false)
        }

        fn finish_coroutines(self, entry: hir::FunctionId) -> hir::Module {
            self.finish_with_coroutine_core(entry, true)
        }

        fn finish_with_coroutine_core(
            mut self,
            entry: hir::FunctionId,
            include_exceptions: bool,
        ) -> hir::Module {
            let exception_core = self.test_exception_core(include_exceptions);
            let coroutine_core = self.test_coroutine_core(exception_core.throwable.class());
            let t = self
                .types
                .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
            let ptr = self.declare_struct("Ptr", vec![type_param("T")], vec![t], &[], &[]);
            let fun_ptr = self.declare_struct("FunPtr", vec![type_param("F")], vec![t], &[], &[]);
            let pinned_ptr =
                self.declare_struct("PinnedPtr", vec![type_param("T")], vec![t], &[], &[]);
            let gc_handle =
                self.declare_struct("GcHandle", vec![type_param("T")], vec![t], &[], &[]);
            let ffi_core = hir::FfiCore {
                ptr,
                fun_ptr,
                pinned_ptr,
                gc_handle,
                ptr_to_uint: entry,
                ptr_cast: entry,
                ptr_load: entry,
                ptr_load_offset: entry,
                ptr_store: entry,
                ptr_store_offset: entry,
                ptr_plus: entry,
                ptr_minus: entry,
                address_of: entry,
                size_of: entry,
                align_of: entry,
                gc_pin_raw: entry,
                gc_unpin_raw: entry,
                gc_get_handle_raw: entry,
                gc_release_handle_raw: entry,
            };
            let unit_variants = |variants: &[&str]| {
                variants
                    .iter()
                    .map(|variant| hir::Variant {
                        name: (*variant).to_string(),
                        fields: Vec::new(),
                        defaults: Vec::new(),
                    })
                    .collect()
            };
            let callback_mode = self.declare_enum(
                "ForeignCallbackMode",
                Vec::new(),
                Vec::new(),
                unit_variants(&["Reusable", "OneShot"]),
            );
            let callback_state = self.declare_enum(
                "ForeignCallbackState",
                Vec::new(),
                Vec::new(),
                unit_variants(&["Registered", "Active", "Completed", "Failed"]),
            );
            let uint = self.uint();
            let intrinsic_int =
                self.declare_fixed_intrinsic_struct("Int", hir::IntrinsicTypeKind::Int, self.int);
            let intrinsic_uint =
                self.declare_fixed_intrinsic_struct("UInt", hir::IntrinsicTypeKind::UInt, uint);
            let intrinsic_boolean = self.declare_fixed_intrinsic_struct(
                "Boolean",
                hir::IntrinsicTypeKind::Boolean,
                self.boolean,
            );
            let intrinsic_string = self.declare_intrinsic_class(
                "String",
                hir::IntrinsicTypeKind::String,
                Vec::new(),
                Vec::new(),
                CanonicalTypePlan::Existing(self.string),
            );
            let intrinsic_array = self.intrinsic_array_class(hir::IntrinsicTypeKind::Array);
            let intrinsic_mutable_array =
                self.intrinsic_array_class(hir::IntrinsicTypeKind::MutableArray);
            hir::Module {
                types: self.types,
                function_types: Arena::new(),
                lambdas: Arena::new(),
                anonymous_functions: Arena::new(),
                local_functions: Arena::new(),
                callable_references: Arena::new(),
                bound_callable_refs: Arena::new(),
                function_coercions: Arena::new(),
                foreign_callback_registrations: Arena::new(),
                functions: self.functions,
                extern_functions: self.extern_functions,
                globals: Arena::new(),
                generic_functions: self.generic_functions,
                method_applications: self.method_applications,
                generic_methods: self.generic_methods,
                generic_method_applications: self.generic_method_applications,
                derived_equality_applications: Arena::new(),
                structs: self.structs,
                struct_applications: self.struct_applications,
                enums: self.enums,
                enum_applications: self.enum_applications,
                classes: self.classes,
                class_applications: self.class_applications,
                interfaces: self.interfaces,
                interface_applications: self.interface_applications,
                interface_methods: self.interface_methods,
                top_level: self.top_level,
                unit: self.unit,
                int: self.int,
                boolean: self.boolean,
                string: self.string,
                option_enum: self.option_enum,
                exception_core,
                coroutine_core,
                ffi_core,
                foreign_callback_core: hir::ForeignCallbackCore {
                    callback: ptr,
                    mode: callback_mode,
                    state: callback_state,
                    register: entry,
                    retain: entry,
                    release: entry,
                    query_state: entry,
                    failure: entry,
                },
                intrinsic_type_core: hir::IntrinsicTypeCore {
                    int: intrinsic_int,
                    uint: intrinsic_uint,
                    boolean: intrinsic_boolean,
                    string: intrinsic_string,
                    array: intrinsic_array,
                    mutable_array: intrinsic_mutable_array,
                },
                entry,
                instantiations: self.instantiations,
            }
        }
    }

    fn local(name: &str, ty: hir::TypeId) -> hir::Local {
        hir::Local {
            binding: hir::BindingId::from_raw(0),
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
                callee: hir::Callable::Function(function),
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
                callee: hir::Callable::Function(function),
                args,
            },
            ty,
        )
    }

    fn struct_init(h: &Harness, ty: hir::TypeId, args: Vec<hir::Expr>) -> hir::Expr {
        let hir::Type::Struct(application) = h.types[ty] else {
            panic!("struct construction requires a struct application type")
        };
        expr(hir::ExprKind::StructInit { application, args }, ty)
    }

    fn module_interface_application(
        module: &mut hir::Module,
        template: hir::InterfaceId,
        arguments: Vec<hir::TypeId>,
    ) -> hir::TypeId {
        let canonical_type = hir::TypeId::from_raw((module.types.len() as u32).into());
        let application = module
            .interface_applications
            .alloc(hir::InterfaceApplication {
                template,
                arguments,
                canonical_type,
            });
        let actual_type = module.types.alloc(hir::Type::Interface(application));
        assert_eq!(actual_type, canonical_type);
        canonical_type
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
    fn suspend_leaf_uses_typed_hidden_abi_and_completed_step() {
        let mut h = Harness::new();
        let leaf = h.user_fn_full(
            "leaf",
            Vec::new(),
            Vec::new(),
            h.int,
            hir::Body {
                locals: Arena::new(),
                statements: vec![stmt(hir::StatementKind::Return {
                    value: Some(int_lit(&h, 42)),
                })],
            },
        );
        h.functions[leaf].is_suspend = true;
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            },
        );

        let module = lower(&h.finish_coroutines(main));
        let (_, coroutine) = module
            .meta
            .coroutine_functions
            .iter()
            .next()
            .expect("one transformed suspend function");
        let function = &module.functions[coroutine.function];
        assert_eq!(function.symbol, "scoop.leaf$suspend");
        assert_eq!(function.params.len(), 1);
        let mir::Type::Interface(continuation) = function.params[0].ty else {
            panic!("hidden completion must be a concrete Continuation<Int>")
        };
        assert_eq!(module.interfaces[continuation].name, "Continuation$I");

        let step = &module.meta.coroutine_steps[coroutine.step];
        assert_eq!(step.result, mir::Type::Int);
        let step_def = &module.enums[step.enum_id];
        assert!(step_def.gc_free);
        assert!(step_def.variants.iter().all(|variant| variant.gc_free));
        assert_eq!(
            function.return_ty,
            mir::Type::Enum(step.enum_id, Vec::new())
        );
        assert_eq!(step_def.variants[0].name, "Completed");
        let mir::Terminator::Return { value: Some(value) } =
            &function.body.blocks[function.body.entry].terminator
        else {
            panic!("leaf returns a completed step")
        };
        assert!(matches!(
            &value.kind,
            mir::ExprKind::VariantConstruct {
                variant: 0,
                fields,
                ..
            } if matches!(fields.as_slice(), [field] if matches!(field.kind, mir::ExprKind::IntLiteral(42)))
        ));
    }

    #[test]
    fn suspend_call_generates_a_liveness_based_frame_and_resume_point() {
        let mut h = Harness::new();
        let leaf = h.user_fn_full(
            "leaf",
            Vec::new(),
            Vec::new(),
            h.int,
            hir::Body {
                locals: Arena::new(),
                statements: vec![stmt(hir::StatementKind::Return {
                    value: Some(int_lit(&h, 41)),
                })],
            },
        );
        h.functions[leaf].is_suspend = true;
        let mut locals = Arena::new();
        let value = locals.alloc(local("value", h.int));
        let caller = h.user_fn_full(
            "caller",
            Vec::new(),
            Vec::new(),
            h.int,
            hir::Body {
                locals,
                statements: vec![
                    val_decl(value, call_typed(leaf, Vec::new(), h.int)),
                    stmt(hir::StatementKind::Return {
                        value: Some(binary(
                            hir::BinOp::Add,
                            local_ref(value, h.int),
                            int_lit(&h, 1),
                            h.int,
                        )),
                    }),
                ],
            },
        );
        h.functions[caller].is_suspend = true;
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            },
        );

        let module = lower(&h.finish_coroutines(main));
        let (_, caller) = module
            .meta
            .coroutine_functions
            .iter()
            .find(|(_, coroutine)| module.functions[coroutine.function].name == "caller")
            .expect("caller coroutine metadata");
        let mir::CoroutineLowering::StateMachine {
            frame,
            driver,
            resume_points,
        } = &caller.lowering
        else {
            panic!("a suspend call requires a state machine")
        };
        assert_eq!(resume_points.len(), 1);
        let frame = &module.meta.coroutine_frames[*frame];
        let fields = module.classes[frame.class].declared_fields();
        assert_eq!(fields[0].name, "state");
        assert_eq!(fields[1].name, "completion");
        assert_eq!(
            fields
                .iter()
                .filter(|field| field.name == "local$value")
                .count(),
            1
        );
        assert!(
            module.functions[*driver]
                .body
                .blocks
                .iter()
                .any(|(_, block)| block.name == "coroutine.resume.1")
        );
        let int_slot = module
            .enums
            .iter()
            .find_map(|(_, definition)| {
                (definition.name == "CoroutineSlot$I").then_some(definition)
            })
            .expect("live Int local uses a concrete coroutine slot");
        assert!(int_slot.gc_free);
        assert!(int_slot.variants.iter().all(|variant| variant.gc_free));
        let throwable = module
            .classes
            .iter()
            .find_map(|(id, definition)| (definition.name == "Throwable").then_some(id))
            .expect("Throwable class");
        let throwable_slot = module
            .enums
            .iter()
            .find_map(|(_, definition)| {
                (definition.name.starts_with("CoroutineSlot$")
                    && definition.variants.get(1).is_some_and(|variant| {
                        variant.fields.len() == 1
                            && variant.fields[0].ty == mir::Type::Class(throwable)
                    }))
                .then_some(definition)
            })
            .expect("the failure latch uses a concrete Throwable slot");
        assert!(!throwable_slot.gc_free);
        assert!(throwable_slot.variants[0].gc_free);
        assert!(!throwable_slot.variants[1].gc_free);
        let point = &module.meta.coroutine_resume_points[resume_points[0]];
        assert_eq!(point.result, mir::Type::Int);
        assert_eq!(point.state, 1);
        assert_eq!(module.classes[point.adapter].interfaces.len(), 1);
    }

    #[test]
    fn start_coroutine_resumes_only_an_immediately_completed_task() {
        let mut h = Harness::new();
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            },
        );
        let mut hir_module = h.finish_coroutines(main);
        let result = hir_module.int;
        let suspend_task = hir_module.coroutine_core.suspend_task;
        let continuation = hir_module.coroutine_core.continuation;
        let task_ty = module_interface_application(&mut hir_module, suspend_task, vec![result]);
        let completion_ty =
            module_interface_application(&mut hir_module, continuation, vec![result]);
        let mut locals = Arena::new();
        let task = locals.alloc(local("task", task_ty));
        let completion = locals.alloc(local("completion", completion_ty));
        let Some(start_generic) =
            hir_module.functions[hir_module.coroutine_core.start_coroutine].generic_definition()
        else {
            panic!("startCoroutine is generic")
        };
        let start = hir_module
            .instantiations
            .alloc(hir::ResolvedGenericFunction {
                generic: start_generic,
                type_args: vec![result],
            });
        let launcher = hir_module.functions.alloc(hir::Function {
            name: "launcher".to_string(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            params: vec![
                param("task", task_ty, task),
                param("completion", completion_ty, completion),
            ],
            return_ty: hir_module.unit,
            attributes: hir::FunctionAttributes::default(),
            kind: hir::FunctionKind::User(hir::Body {
                locals,
                statements: vec![expr_stmt(expr(
                    hir::ExprKind::Call {
                        callee: hir::Callable::Generic(start),
                        args: vec![
                            local_ref(task, task_ty),
                            local_ref(completion, completion_ty),
                        ],
                    },
                    hir_module.unit,
                ))],
            }),
            method: None,
            span: SPAN,
        });
        hir_module.top_level.push(launcher);

        let module = lower(&hir_module);
        let launcher = module
            .functions
            .iter()
            .find_map(|(_, function)| (function.name == "launcher").then_some(function))
            .expect("launcher is lowered");
        let launcher_entry = &launcher.body.blocks[launcher.body.entry];
        let (start, destination) = statement_call(&launcher_entry.statements[0]);
        assert!(destination.is_none(), "startCoroutine returns Unit");
        let mir::Callee::User(helper) = start.target.callee else {
            panic!("startCoroutine lowers to its concrete guarded helper")
        };
        let helper = &module.functions[helper];
        let entry = &helper.body.blocks[helper.body.entry];
        let (run, step_local) = statement_call(&entry.statements[0]);
        let mir::CallKind::Interface {
            interface: task_interface,
            slot: 0,
        } = run.target.kind
        else {
            panic!("startCoroutine must invoke SuspendTask<T>.run through interface dispatch")
        };
        assert_eq!(module.interfaces[task_interface].name, "SuspendTask$I");
        assert_eq!(run.args.len(), 2, "run receives task and hidden completion");
        let step_local = step_local.expect("run returns a CoroutineStep<T>");
        let mir::Terminator::Branch {
            then_block: completed,
            else_block: suspended,
            ..
        } = entry.terminator
        else {
            panic!("startCoroutine must distinguish Completed from Suspended")
        };

        let completed = &helper.body.blocks[completed];
        let (resume, destination) = statement_call(&completed.statements[0]);
        assert!(destination.is_none(), "Continuation.resume returns Unit");
        let mir::CallKind::Interface {
            interface: continuation_interface,
            slot: 0,
        } = resume.target.kind
        else {
            panic!("completed task must resume its outer continuation")
        };
        assert_eq!(
            module.interfaces[continuation_interface].name,
            "Continuation$I"
        );
        assert!(matches!(resume.args.as_slice(), [completion, field]
            if matches!(completion.kind, mir::ExprKind::Local(_))
                && matches!(&field.kind, mir::ExprKind::EnumField {
                    operand,
                    variant: 0,
                    index: 0
                } if matches!(operand.kind, mir::ExprKind::Local(local) if local == step_local))));

        let suspended = &helper.body.blocks[suspended];
        assert!(suspended.statements.is_empty());
        assert!(matches!(
            suspended.terminator,
            mir::Terminator::Return { value: None }
        ));
        let catch_pad = entry
            .unwind
            .expect("task body exceptions enter the guarded helper pad");
        assert!(
            helper.body.blocks[catch_pad]
                .statements
                .iter()
                .any(|statement| {
                    matches!(
                        &statement.kind,
                        mir::StatementKind::Call(mir::CallEffect::Value { call, .. })
                            if call.target.callee
                                == mir::Callee::Runtime(mir::RuntimeFn::MaterializeException)
                    )
                })
        );
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
  extern ef0 write @scoop_rt_write(String) -> Unit <abi=scoop managed>
  fun print @scoop.print(message: String) -> Unit
    bb0 entry
      call extern0 @scoop_rt_write direct
        Type String
        Local message
      return
  fun println @scoop.println(message: String) -> Unit
    bb0 entry
      call extern0 @scoop_rt_write direct
        Type String
        Local message
      call extern0 @scoop_rt_write direct
        Type String
        StringConst @scoop.str.0
      return
  fun helper @scoop.helper() -> Unit
    bb0 entry
      call @scoop.print direct
        Type String
        StringConst @scoop.str.1
      return
  fun main @scoop_main() -> Unit
    bb0 entry
      call @scoop.println direct
        Type String
        StringConst @scoop.str.2
      call @scoop.helper direct
      return
  str @scoop.str.0 \"\\n\"
  str @scoop.str.1 \"!\"
  str @scoop.str.2 \"hello, world\"
  entry @scoop_main
";
        assert_eq!(dump(&module), expected);
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
                    callee: hir::Callable::Function(println),
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
    fn typed_intrinsic_kinds_map_to_runtime_functions() {
        // core's `print` / `println` overloads are ordinary user
        // functions (their forwarding is locked by the golden dumps);
        // only the two conversion intrinsics map onto runtime
        // functions, by validated intrinsic kind.
        let mut h = Harness::new();
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: vec![
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
        let shims: Vec<mir::RuntimeFn> = entry_statements(body)
            .iter()
            .map(|statement| {
                let (call, _) = statement_call(statement);
                let mir::Callee::Runtime(function) = call.target.callee else {
                    panic!("expected a runtime callee")
                };
                function
            })
            .collect();
        assert_eq!(
            shims,
            [mir::RuntimeFn::IntToString, mir::RuntimeFn::BoolToString,]
        );
    }

    // ---- M9: GC intrinsics and generic structs ----

    /// The source-level bodies of `pin` / `unpin` / handle operations after
    /// inlining their ordinary wrappers: raw runtime call plus explicit
    /// handle construction or field extraction.
    fn gc_shapes() -> (Harness, hir::FunctionId) {
        let mut h = Harness::new();
        let gc = h.gc_core();
        let (string, uint) = (h.string, h.uint());
        let pinned_ptr_s = h.struct_app(gc.pinned_ptr, vec![string]);
        let gc_handle_s = h.struct_app(gc.gc_handle, vec![string]);
        let pinned_ptr_s_application = h.struct_application_of(pinned_ptr_s);
        let gc_handle_s_application = h.struct_application_of(gc_handle_s);
        let pin_raw = h.instantiate(gc.pin_raw, vec![string]);
        let unpin_raw = h.instantiate(gc.unpin_raw, vec![string]);
        let get_handle_raw = h.instantiate(gc.get_handle_raw, vec![string]);
        let release_handle_raw = h.instantiate(gc.release_handle_raw, vec![string]);
        let mut locals = Arena::new();
        let s = locals.alloc(local("s", string));
        let pin_word = locals.alloc(local("pinWord", uint));
        let ph = locals.alloc(local("ph", pinned_ptr_s));
        let r = locals.alloc(local("r", string));
        let handle_word = locals.alloc(local("handleWord", uint));
        let gh = locals.alloc(local("gh", gc_handle_s));
        let r2 = locals.alloc(local("r2", string));
        let n = locals.alloc(local("n", uint));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![
                    val_decl(
                        pin_word,
                        generic_call(pin_raw, vec![local_ref(s, string)], uint),
                    ),
                    val_decl(
                        ph,
                        struct_init(&h, pinned_ptr_s, vec![local_ref(pin_word, uint)]),
                    ),
                    val_decl(
                        r,
                        generic_call(
                            unpin_raw,
                            vec![expr(
                                hir::ExprKind::FieldAccess {
                                    receiver: Box::new(local_ref(ph, pinned_ptr_s)),
                                    field: hir::FieldRef::StructField {
                                        application: pinned_ptr_s_application,
                                        index: 0,
                                    },
                                },
                                uint,
                            )],
                            string,
                        ),
                    ),
                    val_decl(
                        handle_word,
                        generic_call(get_handle_raw, vec![local_ref(s, string)], uint),
                    ),
                    val_decl(
                        gh,
                        struct_init(&h, gc_handle_s, vec![local_ref(handle_word, uint)]),
                    ),
                    val_decl(
                        r2,
                        generic_call(
                            release_handle_raw,
                            vec![expr(
                                hir::ExprKind::FieldAccess {
                                    receiver: Box::new(local_ref(gh, gc_handle_s)),
                                    field: hir::FieldRef::StructField {
                                        application: gc_handle_s_application,
                                        index: 0,
                                    },
                                },
                                uint,
                            )],
                            string,
                        ),
                    ),
                    expr_stmt(call(&h, gc.gc_collect, vec![])),
                    val_decl(n, call_typed(gc.gc_stats, vec![], uint)),
                ],
            },
        );
        (h, main)
    }

    #[test]
    fn gc_wrappers_use_explicit_raw_word_marshalling() {
        let (h, main) = gc_shapes();
        let module = lower(&h.finish(main));
        let body = &module.functions[module.entry].body;

        // `_pin(s)` produces the raw word used by `PinnedPtr(raw)`.
        let (call, pin_result) = statement_call(&entry_statements(body)[0]);
        assert!(matches!(
            call.target.callee,
            mir::Callee::Runtime(mir::RuntimeFn::Pin)
        ));
        assert!(
            matches!(call.args.as_slice(), [arg] if matches!(arg.kind, mir::ExprKind::Local(_)))
        );
        let pin_result = pin_result.expect("_pin returns a raw word");
        let mir::StatementKind::ValDecl { init, .. } = &entry_statements(body)[1].kind else {
            panic!("PinnedPtr construction is a val decl")
        };
        let mir::ExprKind::StructInit { struct_id, args } = &init.kind else {
            panic!("the raw result is wrapped into PinnedPtr")
        };
        assert_eq!(module.structs[*struct_id].name, "PinnedPtr$S");
        assert_eq!(
            module.structs[*struct_id].declared_fields()[0].ty,
            mir::Type::UInt
        );
        assert!(matches!(args.as_slice(), [arg]
            if matches!(arg.kind, mir::ExprKind::Local(local) if local == pin_result)));

        // `_unpin(ph.raw)` directly initializes the source result local.
        let (call, hidden) = statement_call(&entry_statements(body)[2]);
        let hidden = hidden.expect("_unpin produces the typed result");
        assert!(matches!(
            call.target.callee,
            mir::Callee::Runtime(mir::RuntimeFn::Unpin)
        ));
        let [arg] = call.args.as_slice() else {
            panic!("unpin takes one argument")
        };
        let mir::ExprKind::FieldAccess { index: 0, .. } = arg.kind else {
            panic!("unpin's argument is the handle's raw field")
        };
        assert_eq!(body.locals[hidden].name, "r");

        // `getGcHandle` / `releaseGcHandle` share the wrap / unwrap
        // shapes with their own runtime symbols and handle struct.
        let (call, handle_result) = statement_call(&entry_statements(body)[3]);
        assert!(matches!(
            call.target.callee,
            mir::Callee::Runtime(mir::RuntimeFn::GetHandle)
        ));
        let handle_result = handle_result.expect("getGcHandle returns a raw word");
        let mir::StatementKind::ValDecl { init, .. } = &entry_statements(body)[4].kind else {
            panic!("getGcHandle's statement is a val decl")
        };
        let mir::ExprKind::StructInit { struct_id, args } = &init.kind else {
            panic!("getGcHandle's result is wrapped into the handle struct")
        };
        assert_eq!(module.structs[*struct_id].name, "GcHandle$S");
        assert!(matches!(args.as_slice(), [arg]
            if matches!(arg.kind, mir::ExprKind::Local(local) if local == handle_result)));
        let (call, _) = statement_call(&entry_statements(body)[5]);
        assert!(matches!(
            call.target.callee,
            mir::Callee::Runtime(mir::RuntimeFn::ReleaseHandle)
        ));
        let [arg] = call.args.as_slice() else {
            panic!("releaseGcHandle takes one argument")
        };
        let mir::ExprKind::FieldAccess { index: 0, .. } = arg.kind else {
            panic!("releaseGcHandle's argument is the handle's raw field")
        };

        // `gcCollect()` is a plain void runtime call; `gcStats()`
        // yields the raw word (`UInt`).
        let (call, destination) = statement_call(&entry_statements(body)[6]);
        assert!(destination.is_none());
        assert!(matches!(
            call.target.callee,
            mir::Callee::Runtime(mir::RuntimeFn::GcCollect)
        ));
        let (call, destination) = statement_call(&entry_statements(body)[7]);
        assert!(matches!(
            call.target.callee,
            mir::Callee::Runtime(mir::RuntimeFn::GcStats)
        ));
        assert_eq!(
            destination,
            Some(
                *body
                    .locals
                    .iter()
                    .find(|(_, local)| local.name == "n")
                    .map(|(id, _)| id)
                    .as_ref()
                    .expect("n local")
            )
        );
    }

    #[test]
    fn generic_structs_instantiate_per_argument_list() {
        let mut h = Harness::new();
        let gc = h.gc_core();
        let (string, uint) = (h.string, h.uint());
        // Two applications of one generic struct, one of them twice
        // (dedup), plus a struct whose field mentions its type
        // parameter (the general substitution path).
        let pinned_ptr_v = h.struct_app(gc.pinned_ptr, vec![uint]);
        let pinned_ptr_s = h.struct_app(gc.pinned_ptr, vec![string]);
        let t = h
            .types
            .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
        let box2 = h.declare_struct("Box2", vec![type_param("T")], vec![t], &[("x", t)], &[]);
        let box2_s = h.struct_app(box2, vec![string]);
        let mut locals = Arena::new();
        let a = locals.alloc(local("a", pinned_ptr_v));
        let b = locals.alloc(local("b", pinned_ptr_s));
        let c = locals.alloc(local("c", pinned_ptr_s));
        let d = locals.alloc(local("d", box2_s));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![
                    val_decl(a, struct_init(&h, pinned_ptr_v, vec![int_lit(&h, 1)])),
                    val_decl(b, struct_init(&h, pinned_ptr_s, vec![int_lit(&h, 2)])),
                    val_decl(c, struct_init(&h, pinned_ptr_s, vec![int_lit(&h, 3)])),
                    val_decl(d, struct_init(&h, box2_s, vec![str_lit(&h, "x")])),
                ],
            },
        );
        let module = lower(&h.finish(main));

        // One instance per (struct, args): `PinnedPtr$V` once,
        // `PinnedPtr$S` once despite two uses, `Box2$S` once — named
        // like the enum instances. Generic definitions themselves do
        // not survive into MIR: MIR contains no generic types.
        let defs = |name: &str| {
            module
                .structs
                .iter()
                .filter(|(_, def)| def.name == name)
                .map(|(_, def)| def)
                .collect::<Vec<_>>()
        };
        assert!(defs("PinnedPtr").is_empty());
        assert!(defs("Box2").is_empty());
        assert_eq!(defs("PinnedPtr$V").len(), 1);
        assert_eq!(
            defs("PinnedPtr$V")[0].declared_fields()[0].ty,
            mir::Type::UInt
        );
        assert!(defs("PinnedPtr$V")[0].gc_free);
        assert_eq!(defs("PinnedPtr$S").len(), 1);
        assert!(defs("PinnedPtr$S")[0].gc_free);
        assert_eq!(defs("Box2$S").len(), 1);
        // Field substitution: `Box2<String>`'s `x` is `String`.
        assert_eq!(defs("Box2$S")[0].declared_fields()[0].ty, mir::Type::String);
        assert!(!defs("Box2$S")[0].gc_free);

        // Locals and StructInits resolve to the instances.
        let body = &module.functions[module.entry].body;
        let instance_of = |local: mir::LocalId| {
            let mir::Type::Struct(id) = &body.locals[local].ty else {
                panic!("a struct local")
            };
            module.structs[*id].name.as_str()
        };
        let mir::StatementKind::ValDecl { local: la, .. } = entry_statements(body)[0].kind else {
            panic!()
        };
        let mir::StatementKind::ValDecl { local: lb, .. } = entry_statements(body)[1].kind else {
            panic!()
        };
        let mir::StatementKind::ValDecl { local: lc, .. } = entry_statements(body)[2].kind else {
            panic!()
        };
        let mir::StatementKind::ValDecl { local: ld, .. } = entry_statements(body)[3].kind else {
            panic!()
        };
        assert_eq!(instance_of(la), "PinnedPtr$V");
        assert_eq!(instance_of(lb), "PinnedPtr$S");
        assert_eq!(instance_of(lc), "PinnedPtr$S");
        assert_eq!(instance_of(ld), "Box2$S");
        let mir::StatementKind::ValDecl { init, .. } = &entry_statements(body)[0].kind else {
            panic!()
        };
        let mir::ExprKind::StructInit { struct_id, .. } = init.kind else {
            panic!("a struct construction")
        };
        assert_eq!(module.structs[struct_id].name, "PinnedPtr$V");
    }

    #[test]
    fn generic_interface_applications_get_distinct_mir_identities() {
        let mut h = Harness::new();
        let t = h
            .types
            .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
        let interface = h.declare_interface(
            "Channel",
            vec![hir::TypeParamDecl {
                id: hir::TypeParamId::from_raw(0),
                name: "T".to_string(),
                variance: hir::Variance::Out,
                bounds: hir::TypeParamBounds::Unconstrained,
                span: SPAN,
            }],
            vec![t],
            Vec::new(),
        );
        let (int, string) = (h.int, h.string);
        let int_channel = h.interface_app(interface, vec![int]);
        let string_channel = h.interface_app(interface, vec![string]);
        h.declare_class(
            "Ints",
            hir::ClassModifier::Final,
            &[],
            None,
            vec![int_channel],
        );
        h.declare_class(
            "Strings",
            hir::ClassModifier::Final,
            &[],
            None,
            vec![string_channel],
        );
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            },
        );
        let module = lower(&h.finish(main));

        let names: Vec<_> = module
            .interfaces
            .iter()
            .map(|(_, interface)| interface.name.as_str())
            .collect();
        assert_eq!(names, ["Channel$I", "Channel$S"]);
        let class_interfaces: Vec<_> = module
            .classes
            .iter()
            .filter(|(_, class)| class.name == "Ints" || class.name == "Strings")
            .map(|(_, class)| class.interfaces[0])
            .collect();
        assert_ne!(class_interfaces[0], class_interfaces[1]);
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
        let callees: Vec<mir::FunctionId> = entry_statements(body)
            .iter()
            .map(|statement| {
                let (call, _) = statement_call(statement);
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

    fn instance_id(
        module: &mir::Module,
        function: mir::FunctionId,
    ) -> mir::MonomorphizedFunctionId {
        module
            .meta
            .instances
            .iter()
            .find_map(|(id, instance)| (instance.function == function).then_some(id))
            .expect("function must have monomorphization metadata")
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
        let generic_int = h.instantiate(generic, vec![int]);
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: vec![expr_stmt(generic_call(
                    generic_int,
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
        assert_eq!(doc_def.vtable.len(), 2);
        assert_eq!(slot_fn(&module, &doc_def.vtable[0]), "scoop.Doc.describe.I");
        assert_eq!(slot_fn(&module, &doc_def.vtable[1]), "scoop.Doc.describe.S");
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

        // A: one slot per overload.
        let a_def = &module.classes[class_index(0)];
        assert_eq!(a_def.vtable.len(), 2);
        assert_eq!(slot_fn(&module, &a_def.vtable[0]), "scoop.A.s.I");
        assert_eq!(slot_fn(&module, &a_def.vtable[1]), "scoop.A.s.S");
        // B: the `s(Int)` override replaces slot 0 in place; the
        // inherited `s(String)` keeps slot 1. (`B.s` is a unique name
        // in the module, so it keeps the plain symbol.)
        let b_def = &module.classes[class_index(1)];
        assert_eq!(b_def.vtable.len(), 2);
        assert_eq!(slot_fn(&module, &b_def.vtable[0]), "scoop.B.s");
        assert_eq!(slot_fn(&module, &b_def.vtable[1]), "scoop.A.s.S");
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
        let s_int = h.method_application(s_int);
        let s_string = h.method_application(s_string);
        let method_call =
            |receiver: hir::Expr, application: hir::MethodApplicationId, arg: hir::Expr| {
                expr(
                    hir::ExprKind::MethodCall {
                        receiver: Box::new(receiver),
                        callee: hir::MethodCallee::Callable(hir::Callable::Method(application)),
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
            let (call, _) = statement_call(&entry_statements(body)[index]);
            &call.target.kind
        };
        assert!(matches!(call_kind(0), mir::CallKind::Virtual { slot: 0 }));
        assert!(matches!(call_kind(1), mir::CallKind::Virtual { slot: 1 }));
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
        let methods = methods
            .iter()
            .map(|(name, ty)| {
                let v = locals.alloc(local("v", *ty));
                hir::MethodSig {
                    name: name.to_string(),
                    is_suspend: false,
                    attributes: hir::FunctionAttributes::default(),
                    type_params: Vec::new(),
                    params: vec![param("v", *ty, v)],
                    return_ty: unit,
                    span: SPAN,
                }
            })
            .collect();
        h.declare_interface(name, Vec::new(), Vec::new(), methods)
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
        let m_int = h.method_application(m_int);
        let m_string = h.method_application(m_string);
        let method_call =
            |receiver: hir::Expr, application: hir::MethodApplicationId, arg: hir::Expr| {
                expr(
                    hir::ExprKind::MethodCall {
                        receiver: Box::new(receiver),
                        callee: hir::MethodCallee::Callable(hir::Callable::Method(application)),
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
            let (call, _) = statement_call(&entry_statements(body)[index]);
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
        let s_ty = h.struct_ty(s);
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
                        hir::ExprKind::Box(Box::new(struct_init(&h, s_ty, vec![int_lit(&h, 1)]))),
                        multi_ty,
                    ),
                )],
            },
        );
        let module = lower(&h.finish(main));

        let boxed = boxed_class(&module, "box$D1_SX");
        assert_eq!(boxed.itables.len(), 1);
        let record = &boxed.itables[0];
        assert_eq!(record.slots.len(), 2);
        assert_eq!(
            slot_fn(&module, &record.slots[0]),
            "scoop.thunk.D1_SX.Multi.m.I"
        );
        assert_eq!(
            slot_fn(&module, &record.slots[1]),
            "scoop.thunk.D1_SX.Multi.m.S"
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
            let (call, _) = statement_call(&entry_statements(&thunk.body)[0]);
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
        let (call, destination) = statement_call(&entry_statements(body)[0]);
        assert_eq!(
            call.target.callee,
            mir::Callee::Runtime(mir::RuntimeFn::StringConcat)
        );
        let destination = destination.expect("String concatenation returns String");
        assert_eq!(body.locals[destination].name, "s");
        assert!(matches!(call.args.as_slice(), [left, right]
            if matches!(left.kind, mir::ExprKind::StringConst(_))
                && matches!(right.kind, mir::ExprKind::StringConst(_))));
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
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements,
            },
        );
        let module = lower(&h.finish(main));

        let expected: Vec<mir::BinOp> = int_cases.iter().map(|(_, mir_op)| *mir_op).collect();
        let body = &module.functions[module.entry].body;
        let ops: Vec<mir::BinOp> = entry_statements(body)
            .iter()
            .map(|statement| {
                let mir::StatementKind::Expr(expr) = &statement.kind else {
                    panic!("expected an expression statement")
                };
                let mir::ExprKind::Binary { op, .. } = &expr.kind else {
                    panic!("expected a binary expression")
                };
                *op
            })
            .collect();
        assert_eq!(ops, expected);
    }

    #[test]
    fn short_circuit_rhs_calls_stay_on_rhs_edges() {
        let mut h = Harness::new();
        let boolean = h.boolean;
        let rhs = h.user_fn_full(
            "rhs",
            Vec::new(),
            Vec::new(),
            boolean,
            hir::Body {
                locals: Arena::new(),
                statements: vec![stmt(hir::StatementKind::Return {
                    value: Some(bool_lit(&h, true)),
                })],
            },
        );
        let mut locals = Arena::new();
        let and_result = locals.alloc(local("and_result", boolean));
        let or_result = locals.alloc(local("or_result", boolean));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![
                    val_decl(
                        and_result,
                        binary(
                            hir::BinOp::And,
                            bool_lit(&h, false),
                            call_typed(rhs, Vec::new(), boolean),
                            boolean,
                        ),
                    ),
                    val_decl(
                        or_result,
                        binary(
                            hir::BinOp::Or,
                            bool_lit(&h, true),
                            call_typed(rhs, Vec::new(), boolean),
                            boolean,
                        ),
                    ),
                ],
            },
        );
        let module = lower(&h.finish(main));
        let body = &module.functions[module.entry].body;

        let rhs_blocks: Vec<_> = body
            .blocks
            .iter()
            .map(|(_, block)| block)
            .filter(|block| block.name.starts_with("logic.rhs"))
            .collect();
        assert_eq!(rhs_blocks.len(), 2);
        for block in rhs_blocks {
            let (call, destination) = statement_call(&block.statements[0]);
            assert_eq!(call.target.callee, mir::Callee::User(module.top_level[0]));
            assert!(destination.is_some());
        }
        assert!(body.blocks.iter().all(|(_, block)| {
            block.name.starts_with("logic.rhs")
                || block
                    .statements
                    .iter()
                    .all(|statement| !matches!(statement.kind, mir::StatementKind::Call(_)))
        }));

        let mir::Terminator::Branch {
            then_block,
            else_block,
            ..
        } = body.blocks[body.entry].terminator
        else {
            panic!("`&&` must branch to its RHS or short-circuit block")
        };
        assert!(body.blocks[then_block].name.starts_with("logic.rhs"));
        assert!(body.blocks[else_block].name.starts_with("logic.short"));
    }

    #[test]
    fn nested_calls_are_normalized_left_to_right() {
        let mut h = Harness::new();
        let int = h.int;
        let first_result = int_lit(&h, 1);
        let first = h.user_fn_full(
            "first",
            Vec::new(),
            Vec::new(),
            int,
            hir::Body {
                locals: Arena::new(),
                statements: vec![stmt(hir::StatementKind::Return {
                    value: Some(first_result),
                })],
            },
        );
        let second_result = int_lit(&h, 2);
        let second = h.user_fn_full(
            "second",
            Vec::new(),
            Vec::new(),
            int,
            hir::Body {
                locals: Arena::new(),
                statements: vec![stmt(hir::StatementKind::Return {
                    value: Some(second_result),
                })],
            },
        );
        let mut outer_locals = Arena::new();
        let a = outer_locals.alloc(local("a", int));
        let b = outer_locals.alloc(local("b", int));
        let outer = h.user_fn_full(
            "outer",
            Vec::new(),
            vec![param("a", int, a), param("b", int, b)],
            int,
            hir::Body {
                locals: outer_locals,
                statements: vec![stmt(hir::StatementKind::Return {
                    value: Some(local_ref(a, int)),
                })],
            },
        );
        let mut locals = Arena::new();
        let result = locals.alloc(local("result", int));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![val_decl(
                    result,
                    call_typed(
                        outer,
                        vec![
                            call_typed(first, Vec::new(), int),
                            call_typed(second, Vec::new(), int),
                        ],
                        int,
                    ),
                )],
            },
        );
        let module = lower(&h.finish(main));
        let body = &module.functions[module.entry].body;
        let statements = entry_statements(body);
        assert_eq!(statements.len(), 3);

        let (first_call, first_destination) = statement_call(&statements[0]);
        let first_destination = first_destination.expect("first returns Int");
        assert_eq!(
            first_call.target.callee,
            mir::Callee::User(module.top_level[0])
        );
        let (second_call, second_destination) = statement_call(&statements[1]);
        let second_destination = second_destination.expect("second returns Int");
        assert_eq!(
            second_call.target.callee,
            mir::Callee::User(module.top_level[1])
        );
        let (outer_call, outer_destination) = statement_call(&statements[2]);
        assert_eq!(
            outer_call.target.callee,
            mir::Callee::User(module.top_level[2])
        );
        assert!(matches!(outer_call.args.as_slice(), [first, second]
            if matches!(first.kind, mir::ExprKind::Local(local) if local == first_destination)
                && matches!(second.kind, mir::ExprKind::Local(local) if local == second_destination)));
        let outer_destination = outer_destination.expect("outer returns Int");
        assert_eq!(body.locals[outer_destination].name, "result");
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
  class ArithmeticException vtable=0 itables=0
  fun main @scoop_main() -> Unit
    bb0 entry
      val $div.1: Int
        Type Int
        IntLiteral 10
      val $div.2: Int
        Type Int
        IntLiteral 2
      branch bb1 bb2
        Type Boolean
        Binary IntEq
          Type Int
          Local $div.2
          Type Int
          IntLiteral 0
    bb1 if.then.1
      call $call.1: ArithmeticException = @scoop.ctor.ArithmeticException direct
      throw
        Type ArithmeticException
        Local $call.1
    bb2 if.merge.2
      val q: Int
        Type Int
        Binary IntDiv
          Type Int
          Local $div.1
          Type Int
          Local $div.2
      return
  fun ctor.ArithmeticException @scoop.ctor.ArithmeticException() -> ArithmeticException
    bb0 entry
      return
        Type ArithmeticException
        ClassInit ArithmeticException
  entry @scoop_main
";
        assert_eq!(dump(&module), expected);
    }

    #[test]
    fn try_and_throw_become_explicit_cfg() {
        // try { throw MyError() } catch (e: MyError) { 1 } finally { 2 }
        // — MIR keeps the structured form (DESIGN 3.3); the
        // control-flow expansion is LIR's job.
        let mut h = Harness::new();
        let my_error = h.exception("MyError");
        let error_ty = h.class_ty(my_error);
        let error_application = h.class_application_of(error_ty);
        let mut locals = Arena::new();
        let e = locals.alloc(local("e", error_ty));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![stmt(hir::StatementKind::Try(hir::Try {
                    body: vec![stmt(hir::StatementKind::Throw(expr(
                        hir::ExprKind::ClassInit {
                            application: error_application,
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
  class MyError vtable=0 itables=0
  fun main @scoop_main() -> Unit
    bb0 entry
      goto bb8
    bb1 try.unwind.1
      landing_pad cleanup=false
      goto bb2
    bb2 try.dispatch.2
      begin_catch
      branch bb9 bb10
        Type Boolean
        IsInstance MyError
          Type Any
          CaughtException
    bb3 try.handler_pad.3
      landing_pad cleanup=true
      goto bb4
    bb4 try.handler_cleanup.4
      end_catch
      Type Int
      IntLiteral 2
      resume
    bb5 try.exit_pad.5
      landing_pad cleanup=true
      goto bb6
    bb6 try.exit_cleanup.6
      end_catch
      resume
    bb7 try.end.7
      return
    bb8 try.body.8 unwind bb1
      call $call.1: MyError = @scoop.ctor.MyError direct
      throw unwind bb1
        Type MyError
        Local $call.1
    bb9 try.catch.9
      val e: MyError
        Type MyError
        Retype MyError
          Type Any
          CaughtException
      goto bb11
    bb10 try.next.10 unwind bb5
      Type Int
      IntLiteral 2
      rethrow unwind bb5
    bb11 scope.11 unwind bb3
      Type Int
      IntLiteral 1
      goto bb12
    bb12 scope.12
      end_catch
      Type Int
      IntLiteral 2
      goto bb7
  fun ctor.MyError @scoop.ctor.MyError() -> MyError
    bb0 entry
      return
        Type MyError
        ClassInit MyError
  entry @scoop_main
";
        assert_eq!(dump(&module), expected);
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
        let ops: Vec<mir::UnOp> = entry_statements(body)
            .iter()
            .map(|statement| {
                let mir::StatementKind::Expr(expr) = &statement.kind else {
                    panic!("expected an expression statement")
                };
                let mir::ExprKind::Unary { op, .. } = &expr.kind else {
                    panic!("expected a unary expression")
                };
                *op
            })
            .collect();
        assert_eq!(ops, [mir::UnOp::IntNeg, mir::UnOp::BoolNot]);
    }

    #[test]
    fn field_access_uses_zero_based_indices() {
        let mut h = Harness::new();
        let point = h.strukt("Point", &[("x", h.int), ("y", h.int)]);
        let point_ty = h.struct_ty(point);
        let point_application = h.struct_application_of(point_ty);
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
                                    application: point_application,
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
        for statement in entry_statements(body) {
            let mir::StatementKind::ValDecl { init, .. } = &statement.kind else {
                panic!("expected a val declaration")
            };
            assert!(matches!(
                init.kind,
                mir::ExprKind::FieldAccess { index: 1, .. }
            ));
        }
    }

    #[test]
    fn control_flow_becomes_cfg() {
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
        assert!(matches!(
            body.blocks[body.entry].terminator,
            mir::Terminator::Branch { .. }
        ));
        assert_eq!(block_named(body, "if.then").statements.len(), 1);
        assert_eq!(block_named(body, "if.else").statements.len(), 1);
        assert!(matches!(
            block_named(body, "while.cond").terminator,
            mir::Terminator::Branch { .. }
        ));
        assert!(block_named(body, "while.body").statements.is_empty());
    }

    fn generic_call(
        resolved: hir::ResolvedGenericFunctionId,
        args: Vec<hir::Expr>,
        ty: hir::TypeId,
    ) -> hir::Expr {
        expr(
            hir::ExprKind::Call {
                callee: hir::Callable::Generic(resolved),
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
        let t = h
            .types
            .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
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
            &add_fn.body.blocks[add_fn.body.entry].terminator,
            mir::Terminator::Return {
                value: Some(value)
            } if matches!(value.kind, mir::ExprKind::Binary { op: mir::BinOp::IntAdd, .. })
        ));
    }

    #[test]
    fn monomorphizes_generic_functions() {
        let mut h = Harness::new();
        let identity = identity_fn(&mut h, "identity");
        let (int, string) = (h.int, h.string);
        let identity_int = h.instantiate(identity, vec![int]);
        let identity_string = h.instantiate(identity, vec![string]);
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: vec![
                    expr_stmt(generic_call(identity_int, vec![int_lit(&h, 41)], int)),
                    expr_stmt(generic_call(
                        identity_string,
                        vec![str_lit(&h, "hi")],
                        string,
                    )),
                ],
            },
        );
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
            &int_instance.body.blocks[int_instance.body.entry].terminator,
            mir::Terminator::Return {
                value: Some(value)
            } if matches!(value.kind, mir::ExprKind::Local(local) if local == x)
        ));
        assert_eq!(string_instance.params[0].ty, mir::Type::String);
        assert_eq!(string_instance.return_ty, mir::Type::String);

        // MIR gives every materialized body its own typed identity and
        // records symbol -> generic source provenance in the meta.
        assert_eq!(module.meta.instances.len(), 2);
        let int_meta = &module.meta.instances[instance_id(&module, module.top_level[1])];
        assert_eq!(int_meta.symbol, "scoop.identity$I");
        assert_eq!(int_meta.source, "identity");
        assert_eq!(int_meta.type_args, vec![mir::Type::Int]);

        // The calls in main resolve to the two instances.
        let main_fn = &module.functions[module.entry];
        for (statement, instance) in entry_statements(&main_fn.body)
            .iter()
            .zip([module.top_level[1], module.top_level[2]])
        {
            let (call, _) = statement_call(statement);
            assert_eq!(
                call.target.callee,
                mir::Callee::Monomorphized(instance_id(&module, instance))
            );
        }
    }

    #[test]
    fn duplicate_requests_produce_one_instance() {
        let mut h = Harness::new();
        let identity = identity_fn(&mut h, "identity");
        let int = h.int;
        let identity_int = h.instantiate(identity, vec![int]);
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: vec![
                    expr_stmt(generic_call(identity_int, vec![int_lit(&h, 1)], int)),
                    expr_stmt(generic_call(identity_int, vec![int_lit(&h, 2)], int)),
                ],
            },
        );
        // HIR dedups its list, but be robust: the same request listed
        // twice, plus two calls with the same type arguments.
        assert_eq!(h.instantiate(identity, vec![int]), identity_int);
        let module = lower(&h.finish(main));

        assert_eq!(module.top_level.len(), 2);
        let instance = module.top_level[1];
        let main_fn = &module.functions[module.entry];
        for statement in entry_statements(&main_fn.body) {
            let (call, _) = statement_call(statement);
            assert_eq!(
                call.target.callee,
                mir::Callee::Monomorphized(instance_id(&module, instance))
            );
        }
    }

    #[test]
    fn nested_generic_calls_extend_the_worklist() {
        let mut h = Harness::new();
        // fun <T> inner(x: T): T { return x }
        let inner = identity_fn(&mut h, "inner");
        // fun <T> forward(x: T): T { return inner(x) }
        let t = h
            .types
            .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
        let mut locals = Arena::new();
        let x = locals.alloc(local("x", t));
        let inner_t = h.instantiate(inner, vec![t]);
        let forward = h.user_fn_full(
            "forward",
            vec!["T".to_string()],
            vec![param("x", t, x)],
            t,
            hir::Body {
                locals,
                statements: vec![stmt(hir::StatementKind::Return {
                    value: Some(generic_call(inner_t, vec![local_ref(x, t)], t)),
                })],
            },
        );
        let int = h.int;
        let forward_int = h.instantiate(forward, vec![int]);
        let main = h.user_fn(
            "main",
            hir::Body {
                locals: Arena::new(),
                statements: vec![expr_stmt(generic_call(
                    forward_int,
                    vec![int_lit(&h, 1)],
                    int,
                ))],
            },
        );
        // The nested request is parameterized in export HIR's list;
        // local-concrete HIR resolves it while materializing forward$I.
        let module = lower(&h.finish(main));

        // main, forward$I, then inner$I (discovered via the worklist).
        assert_eq!(module.top_level.len(), 3);
        let forward_i = &module.functions[module.top_level[1]];
        let inner_i = &module.functions[module.top_level[2]];
        assert_eq!(forward_i.symbol, "scoop.forward$I");
        assert_eq!(inner_i.symbol, "scoop.inner$I");
        let (call, destination) = statement_call(&entry_statements(&forward_i.body)[0]);
        let destination = destination.expect("inner$I returns Int");
        assert_eq!(
            call.target.callee,
            mir::Callee::Monomorphized(instance_id(&module, module.top_level[2]))
        );
        assert!(matches!(
            &forward_i.body.blocks[forward_i.body.entry].terminator,
            mir::Terminator::Return {
                value: Some(value)
            } if matches!(value.kind, mir::ExprKind::Local(local) if local == destination)
        ));
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
        // An enum argument encodes the category, length-delimited
        // instance name, and complete argument list (`mir::encode_type`).
        assert_eq!(symbols, ["scoop.f$E8_Option$IAIX", "scoop.f$TI_SX"]);
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
        let color_variants = ["Red", "Green", "Blue"]
            .iter()
            .map(|name| hir::Variant {
                name: name.to_string(),
                fields: Vec::new(),
                defaults: Vec::new(),
            })
            .collect();
        let color = h.declare_enum("Color", Vec::new(), Vec::new(), color_variants);
        let color_ty = h.enum_ty(color);
        let s = h.strukt("S", &[("x", int)]);
        let s_ty = h.struct_ty(s);
        let option_int = h.option(int);
        let option_string = h.option(string);
        let option_s = h.option(s_ty);
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
        let mut locals3 = Arena::new();
        locals3.alloc(local("s", option_s));
        let _f3 = h.user_fn(
            "f3",
            hir::Body {
                locals: locals3,
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
        // duplicate Option<Int> request was deduplicated by enum identity.
        let names: Vec<&str> = module
            .enums
            .iter()
            .map(|(_, def)| def.name.as_str())
            .collect();
        assert_eq!(names, ["Option$I", "Color", "Option$S", "Option$D1_SX"]);

        // The variant field types are substituted with the instance's
        // type arguments.
        let option_int_def = &module.enums[la_arena::Idx::from_raw(0.into())];
        assert_eq!(option_int_def.variants[0].name, "Some");
        assert_eq!(option_int_def.variants[0].fields[0].ty, mir::Type::Int);
        assert!(option_int_def.gc_free);
        assert!(
            option_int_def
                .variants
                .iter()
                .all(|variant| variant.gc_free)
        );
        let option_string_def = &module.enums[la_arena::Idx::from_raw(2.into())];
        assert_eq!(
            option_string_def.variants[0].fields[0].ty,
            mir::Type::String
        );
        let option_s_def = &module.enums[la_arena::Idx::from_raw(3.into())];
        assert_eq!(
            option_s_def.variants[0].fields[0].ty,
            mir::Type::Struct(la_arena::Idx::from_raw(0.into()))
        );
        assert_ne!(option_string_def.name, option_s_def.name);
        assert!(!option_string_def.gc_free);
        assert!(!option_string_def.variants[0].gc_free);
        assert!(option_string_def.variants[1].gc_free);
        // Color's variants are all unit variants.
        let color_def = &module.enums[la_arena::Idx::from_raw(1.into())];
        assert!(color_def.gc_free);
        assert_eq!(color_def.variants.len(), 3);
        assert!(
            color_def
                .variants
                .iter()
                .all(|variant| variant.fields.is_empty() && variant.gc_free)
        );
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
    bb0 entry
      val o: Option$I<Int>
        Type Option$I<Int>
        VariantConstruct Option$I<Int> v0
          Type Int
          IntLiteral 41
      val n: Option$I<Int>
        Type Option$I<Int>
        VariantConstruct Option$I<Int> v1
      val b: Boolean
        Type Boolean
        Binary IntEq
          Type Int
          EnumTag
            Type Option$I<Int>
            Local o
          Type Int
          IntLiteral 0
      val y: Int
        Type Int
        EnumField v0 f0
          Type Option$I<Int>
          Local o
      return
  entry @scoop_main
";
        assert_eq!(dump(&module), expected);
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
  class UnwrapException vtable=0 itables=0
  fun main @scoop_main() -> Unit
    bb0 entry
      val o: Option$I<Int>
        Type Option$I<Int>
        VariantConstruct Option$I<Int> v0
          Type Int
          IntLiteral 1
      val $opt.1: Option$I<Int>
        Type Option$I<Int>
        Local o
      branch bb1 bb2
        Type Boolean
        Binary IntEq
          Type Int
          EnumTag
            Type Option$I<Int>
            Local $opt.1
          Type Int
          IntLiteral 0
    bb1 if.then.1
      val $uw.2: Int
        Type Int
        EnumField v0 f0
          Type Option$I<Int>
          Local $opt.1
      goto bb3
    bb2 if.else.2
      call $call.1: UnwrapException = @scoop.ctor.UnwrapException direct
      throw
        Type UnwrapException
        Local $call.1
    bb3 if.merge.3
      val y: Int
        Type Int
        Local $uw.2
      return
  fun ctor.UnwrapException @scoop.ctor.UnwrapException() -> UnwrapException
    bb0 entry
      return
        Type UnwrapException
        ClassInit UnwrapException
  entry @scoop_main
";
        assert_eq!(dump(&module), expected);
    }

    /// `val a: Option<Int> = None; val b = Some(1); val r = a <op> b`
    /// — the shared shell of the enum equality tests.
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
        let option_int = h.option(int);
        let option_application = h.enum_application_of(option_int);
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
                                    application: option_application,
                                    variant: 0,
                                    fields: vec![(0, hir::Pattern::Binding { local: x })],
                                },
                                None,
                                vec![expr_stmt(call(&h, print_int, vec![local_ref(x, int)]))],
                            ),
                            arm(
                                hir::Pattern::Variant {
                                    application: option_application,
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
  extern ef0 write @scoop_rt_write(String) -> Unit <abi=scoop managed>
  enum Option$I
    Some(_1: Int)
    None()
  fun print @scoop.print(message: Int) -> Unit
    bb0 entry
      call $call.1: String = @scoop_rt_int_to_string direct
        Type Int
        Local message
      call extern0 @scoop_rt_write direct
        Type String
        Local $call.1
      return
  fun println @scoop.println(message: String) -> Unit
    bb0 entry
      call extern0 @scoop_rt_write direct
        Type String
        Local message
      call extern0 @scoop_rt_write direct
        Type String
        StringConst @scoop.str.0
      return
  fun main @scoop_main() -> Unit
    bb0 entry
      val o: Option$I<Int>
        Type Option$I<Int>
        VariantConstruct Option$I<Int> v0
          Type Int
          IntLiteral 1
      val $when.1: Option$I<Int>
        Type Option$I<Int>
        Local o
      branch bb1 bb2
        Type Boolean
        Binary IntEq
          Type Int
          EnumTag
            Type Option$I<Int>
            Local $when.1
          Type Int
          IntLiteral 0
    bb1 if.then.1
      val x: Int
        Type Int
        EnumField v0 f0
          Type Option$I<Int>
          Local $when.1
      call @scoop.print direct
        Type Int
        Local x
      goto bb3
    bb2 if.else.2
      branch bb4 bb5
        Type Boolean
        Binary IntEq
          Type Int
          EnumTag
            Type Option$I<Int>
            Local $when.1
          Type Int
          IntLiteral 1
    bb3 if.merge.3
      return
    bb4 if.then.4
      call @scoop.println direct
        Type String
        StringConst @scoop.str.1
      goto bb5
    bb5 if.merge.5
      goto bb3
  str @scoop.str.0 \"\\n\"
  str @scoop.str.1 \"none\"
  entry @scoop_main
";
        assert_eq!(dump(&module), expected);
    }

    #[test]
    fn a_failed_guard_falls_through_to_the_next_arm() {
        // when (o) { Some(x) if (x > 0) -> print(x); else -> println("neg") }
        let mut h = Harness::new();
        let print_int = h.print_int();
        let println_string = h.println_string();
        let int = h.int;
        let option_int = h.option(int);
        let option_application = h.enum_application_of(option_int);
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
                            application: option_application,
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
  extern ef0 write @scoop_rt_write(String) -> Unit <abi=scoop managed>
  enum Option$I
    Some(_1: Int)
    None()
  fun print @scoop.print(message: Int) -> Unit
    bb0 entry
      call $call.1: String = @scoop_rt_int_to_string direct
        Type Int
        Local message
      call extern0 @scoop_rt_write direct
        Type String
        Local $call.1
      return
  fun println @scoop.println(message: String) -> Unit
    bb0 entry
      call extern0 @scoop_rt_write direct
        Type String
        Local message
      call extern0 @scoop_rt_write direct
        Type String
        StringConst @scoop.str.0
      return
  fun main @scoop_main() -> Unit
    bb0 entry
      val $when.1: Option$I<Int>
        Type Option$I<Int>
        Local o
      branch bb1 bb2
        Type Boolean
        Binary IntEq
          Type Int
          EnumTag
            Type Option$I<Int>
            Local $when.1
          Type Int
          IntLiteral 0
    bb1 if.then.1
      val x: Int
        Type Int
        EnumField v0 f0
          Type Option$I<Int>
          Local $when.1
      branch bb4 bb5
        Type Boolean
        Binary IntGt
          Type Int
          Local x
          Type Int
          IntLiteral 0
    bb2 if.else.2
      call @scoop.println direct
        Type String
        StringConst @scoop.str.2
      goto bb3
    bb3 if.merge.3
      return
    bb4 if.then.4
      call @scoop.print direct
        Type Int
        Local x
      goto bb6
    bb5 if.else.5
      call @scoop.println direct
        Type String
        StringConst @scoop.str.1
      goto bb6
    bb6 if.merge.6
      goto bb3
  str @scoop.str.0 \"\\n\"
  str @scoop.str.1 \"neg\"
  str @scoop.str.2 \"neg\"
  entry @scoop_main
";
        assert_eq!(dump(&module), expected);
    }

    #[test]
    fn literal_patterns_match_by_equality() {
        // when (n) { 1 -> println("one"); else -> println("other") }
        let mut h = Harness::new();
        let println = h.println_string();
        let int = h.int;
        let boolean = h.boolean;
        let mut equals_locals = Arena::new();
        let left = equals_locals.alloc(local("left", int));
        let right = equals_locals.alloc(local("right", int));
        let equals = h.user_fn_full(
            "Int.equals",
            Vec::new(),
            vec![param("left", int, left), param("right", int, right)],
            boolean,
            hir::Body {
                locals: equals_locals,
                statements: vec![hir::Statement {
                    kind: hir::StatementKind::Return {
                        value: Some(bool_lit(&h, true)),
                    },
                    span: SPAN,
                }],
            },
        );
        let mut locals = Arena::new();
        let n = locals.alloc(local("n", int));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![when_stmt(
                    local_ref(n, int),
                    vec![arm(
                        hir::Pattern::Literal {
                            value: int_lit(&h, 1),
                            equals: hir::Callable::Function(equals),
                            subject_ty: int,
                        },
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
        let (call, result) = entry_statements(body)
            .iter()
            .find_map(|statement| {
                matches!(statement.kind, mir::StatementKind::Call(_))
                    .then(|| statement_call(statement))
            })
            .expect("literal pattern calls its HIR-selected equality target");
        assert!(matches!(call.target.kind, mir::CallKind::Direct));
        assert!(matches!(call.args.as_slice(), [scrutinee, literal]
            if matches!(scrutinee.kind, mir::ExprKind::Local(_))
                && matches!(literal.kind, mir::ExprKind::IntLiteral(1))));
        let result = result.expect("equals returns Boolean");
        let mir::Terminator::Branch { cond, .. } = &body.blocks[body.entry].terminator else {
            panic!("literal equality result controls the pattern branch")
        };
        assert!(matches!(cond.kind, mir::ExprKind::Local(local) if local == result));
    }

    #[test]
    fn destructuring_val_declarations_extract_bindings() {
        // val (a, b) = (1, "x"); val Point { x, .. } = p
        let mut h = Harness::new();
        let (int, string) = (h.int, h.string);
        let point = h.strukt("Point", &[("x", int), ("y", int)]);
        let point_ty = h.struct_ty(point);
        let point_application = h.struct_application_of(point_ty);
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
                        struct_init(&h, point_ty, vec![int_lit(&h, 3), int_lit(&h, 4)]),
                    ),
                    stmt(hir::StatementKind::ValDecl {
                        pattern: hir::Pattern::Struct {
                            application: point_application,
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
    bb0 entry
      val $bind.1: (Int, String)
        Type (Int, String)
        TupleLiteral
          Type Int
          IntLiteral 1
          Type String
          StringConst @scoop.str.0
      val a: Int
        Type Int
        FieldAccess 0
          Type (Int, String)
          Local $bind.1
      val b: String
        Type String
        FieldAccess 1
          Type (Int, String)
          Local $bind.1
      val p: Point
        Type Point
        StructInit Point
          Type Int
          IntLiteral 3
          Type Int
          IntLiteral 4
      val $bind.2: Point
        Type Point
        Local p
      val x: Int
        Type Int
        FieldAccess 0
          Type Point
          Local $bind.2
      return
  str @scoop.str.0 \"x\"
  entry @scoop_main
";
        assert_eq!(dump(&module), expected);
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
  class IndexOutOfBoundsException vtable=0 itables=0
  fun main @scoop_main() -> Unit
    bb0 entry
      val a: Array<Int>
        Type Array<Int>
        ArrayLiteral Array$I
          Type Int
          IntLiteral 1
          Type Int
          IntLiteral 2
          Type Int
          IntLiteral 3
      val $arr.1: Array<Int>
        Type Array<Int>
        Local a
      val $idx.2: Int
        Type Int
        IntLiteral 0
      branch bb2 bb1
        Type Boolean
        Binary IntLt
          Type Int
          Local $idx.2
          Type Int
          IntLiteral 0
    bb1 logic.rhs.1
      assign $logic.1
        Type Boolean
        Binary IntGe
          Type Int
          Local $idx.2
          Type Int
          ArrayLen Array$I
            Type Array<Int>
            Local $arr.1
      goto bb3
    bb2 logic.short.2
      assign $logic.1
        Type Boolean
        BoolLiteral true
      goto bb3
    bb3 logic.merge.3
      branch bb4 bb5
        Type Boolean
        Local $logic.1
    bb4 if.then.4
      call $call.2: IndexOutOfBoundsException = @scoop.ctor.IndexOutOfBoundsException direct
      throw
        Type IndexOutOfBoundsException
        Local $call.2
    bb5 if.merge.5
      val x: Int
        Type Int
        ArrayGet Array$I
          Type Array<Int>
          Local $arr.1
          Type Int
          Local $idx.2
      val n: Int
        Type Int
        ArrayLen Array$I
          Type Array<Int>
          Local a
      val m: MutableArray<Int>
        Type MutableArray<Int>
        ArrayClone Array$I -> MutableArray$I
          Type Array<Int>
          Local a
      val $arr.3: MutableArray<Int>
        Type MutableArray<Int>
        Local m
      val $idx.4: Int
        Type Int
        IntLiteral 0
      branch bb7 bb6
        Type Boolean
        Binary IntLt
          Type Int
          Local $idx.4
          Type Int
          IntLiteral 0
    bb6 logic.rhs.6
      assign $logic.3
        Type Boolean
        Binary IntGe
          Type Int
          Local $idx.4
          Type Int
          ArrayLen MutableArray$I
            Type MutableArray<Int>
            Local $arr.3
      goto bb8
    bb7 logic.short.7
      assign $logic.3
        Type Boolean
        BoolLiteral true
      goto bb8
    bb8 logic.merge.8
      branch bb9 bb10
        Type Boolean
        Local $logic.3
    bb9 if.then.9
      call $call.4: IndexOutOfBoundsException = @scoop.ctor.IndexOutOfBoundsException direct
      throw
        Type IndexOutOfBoundsException
        Local $call.4
    bb10 if.merge.10
      array_set MutableArray$I
        Type MutableArray<Int>
        Local $arr.3
        Type Int
        Local $idx.4
        Type Int
        IntLiteral 40
      return
  fun ctor.IndexOutOfBoundsException @scoop.ctor.IndexOutOfBoundsException() -> IndexOutOfBoundsException
    bb0 entry
      return
        Type IndexOutOfBoundsException
        ClassInit IndexOutOfBoundsException
  entry @scoop_main
";
        assert_eq!(dump(&module), expected);
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
            mir::array_type(&module, &array_instance.params[0].ty),
            Some((mir::ArrayKind::Immutable, &mir::Type::Int))
        );
        let mutable_instance = &module.functions[module.top_level[2]];
        assert_eq!(
            mir::array_type(&module, &mutable_instance.return_ty),
            Some((mir::ArrayKind::Mutable, &mir::Type::Int))
        );
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

    #[test]
    fn no_gc_effect_is_preserved_in_mir() {
        let mut h = Harness::new();
        let main = empty_main(&mut h);
        h.functions[main].attributes.gc_effect = hir::GcEffect::NoGc;
        let module = lower(&h.finish(main));
        assert_eq!(
            module.functions[module.entry].gc_effect,
            mir::GcEffect::NoGc
        );
        assert!(mir::dump(&module).contains("-> Unit <no-gc>"));
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
        let derived_application = h.class_application_of(derived_ty);
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
                                application: derived_application,
                                index: 1,
                            },
                        },
                        string,
                    ),
                )],
            },
        );
        let module = lower(&h.finish(main));

        assert_eq!(visible_class_count(&module), 2);
        let base_def = &module.classes[class_index(0)];
        let derived_def = &module.classes[class_index(1)];
        let field_names = |def: &mir::ClassDef| {
            def.declared_fields()
                .iter()
                .map(|field| field.name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(field_names(base_def), ["a"]);
        // The base prefix comes first; HIR's `ClassField` indices
        // follow the same flattened order.
        assert_eq!(field_names(derived_def), ["a", "b"]);
        assert_eq!(derived_def.declared_fields()[1].ty, mir::Type::String);
        assert_eq!(derived_def.base_class(), Some(class_index(0)));
        assert_eq!(derived_def.modifier, mir::ClassModifier::Final);
        assert_eq!(base_def.modifier, mir::ClassModifier::Open);

        // The field access keeps its 0-based index into the flattened
        // layout.
        let body = &module.functions[module.entry].body;
        let mir::StatementKind::ValDecl { init, .. } = &entry_statements(body)[0].kind else {
            panic!("expected a val declaration")
        };
        assert!(matches!(
            init.kind,
            mir::ExprKind::FieldAccess { index: 1, .. }
        ));
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
        // Ordinary member functions are the whole vtable; Any does not
        // reserve compiler-owned slots.
        assert_eq!(
            vtable_symbols(&module.classes[class_index(0)]),
            ["scoop.Base.m1", "scoop.Base.m2"]
        );
        // The base prefix is preserved; the override replaces slot 1
        // in place; the new method appends at slot 2.
        assert_eq!(
            vtable_symbols(&module.classes[class_index(1)]),
            ["scoop.Base.m1", "scoop.Derived.m2", "scoop.Derived.m3"]
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
        let s_ty = h.struct_ty(s);
        let s_describe = empty_method(&mut h, "S", "describe", s_ty);
        let class_describe = h.method_application(class_describe);
        let iface_label = h.method_application(iface_label);
        let s_describe = h.method_application(s_describe);

        let unit = h.unit;
        let mut locals = Arena::new();
        let c = locals.alloc(local("c", class_ty));
        let i = locals.alloc(local("i", iface_ty));
        let sv = locals.alloc(local("sv", s_ty));
        let method_call = |receiver: hir::Expr, application: hir::MethodApplicationId| {
            expr(
                hir::ExprKind::MethodCall {
                    receiver: Box::new(receiver),
                    callee: hir::MethodCallee::Callable(hir::Callable::Method(application)),
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
            let (call, _) = statement_call(&entry_statements(body)[index]);
            // The receiver becomes argument 0 (`this`).
            assert!(!call.args.is_empty());
            &call.target.kind
        };
        // Class receiver: virtual through its ordinary vtable.
        assert!(matches!(call_kind(0), mir::CallKind::Virtual { slot: 0 }));
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
    fn final_methods_are_direct_while_final_overrides_keep_the_base_slot() {
        let mut h = Harness::new();
        let base = h.class("Base", hir::ClassModifier::Open, &[], None, &[]);
        let base_ty = h.class_ty(base);
        let base_open = empty_method(&mut h, "Base", "openMethod", base_ty);
        let base_final = empty_method(&mut h, "Base", "finalMethod", base_ty);
        h.functions[base_final]
            .method
            .as_mut()
            .expect("method")
            .modifier = hir::MethodModifier::Final;

        let derived = h.class(
            "Derived",
            hir::ClassModifier::Final,
            &[],
            Some((base, vec![])),
            &[],
        );
        let derived_ty = h.class_ty(derived);
        let derived_override = empty_method(&mut h, "Derived", "openMethod", derived_ty);
        h.functions[derived_override]
            .method
            .as_mut()
            .expect("method")
            .modifier = hir::MethodModifier::Final;
        let base_open = h.method_application(base_open);
        let base_final = h.method_application(base_final);
        let derived_override = h.method_application(derived_override);

        let unit = h.unit;
        let mut locals = Arena::new();
        let as_base = locals.alloc(local("asBase", base_ty));
        let as_derived = locals.alloc(local("asDerived", derived_ty));
        let call = |receiver: hir::Expr, application: hir::MethodApplicationId| {
            expr(
                hir::ExprKind::MethodCall {
                    receiver: Box::new(receiver),
                    callee: hir::MethodCallee::Callable(hir::Callable::Method(application)),
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
                    expr_stmt(call(local_ref(as_base, base_ty), base_open)),
                    expr_stmt(call(local_ref(as_base, base_ty), base_final)),
                    expr_stmt(call(local_ref(as_derived, derived_ty), derived_override)),
                ],
            },
        );
        let module = lower(&h.finish(main));

        let base_vtable = &module.classes[class_index(0)].vtable;
        assert_eq!(base_vtable.len(), 1);
        assert_eq!(slot_fn(&module, &base_vtable[0]), "scoop.Base.openMethod");
        let derived_vtable = &module.classes[class_index(1)].vtable;
        assert_eq!(derived_vtable.len(), 1);
        assert_eq!(
            slot_fn(&module, &derived_vtable[0]),
            "scoop.Derived.openMethod"
        );

        let body = &module.functions[module.entry].body;
        let kind = |index: usize| {
            let (call, _) = statement_call(&entry_statements(body)[index]);
            &call.target.kind
        };
        assert!(matches!(kind(0), mir::CallKind::Virtual { slot: 0 }));
        assert!(matches!(kind(1), mir::CallKind::Direct));
        assert!(matches!(kind(2), mir::CallKind::Direct));
    }

    #[test]
    fn boxing_only_materializes_the_payload_class() {
        let mut h = Harness::new();
        let int = h.int;
        let s = h.strukt("S", &[("x", int)]);
        let s_ty = h.struct_ty(s);
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
                        hir::ExprKind::Box(Box::new(struct_init(&h, s_ty, vec![int_lit(&h, 1)]))),
                        any,
                    ),
                )],
            },
        );
        let module = lower(&h.finish(main));

        let boxed = boxed_class(&module, "box$D1_SX");
        assert_eq!(boxed.declared_fields().len(), 1);
        assert_eq!(boxed.declared_fields()[0].name, "value");
        assert_eq!(
            boxed.declared_fields()[0].ty,
            mir::Type::Struct(la_arena::Idx::from_raw(0.into()))
        );
        assert!(boxed.vtable.is_empty());
        assert!(boxed.itables.is_empty());
        assert_eq!(module.meta.boxed_types.len(), 1);
        let boxed_meta = &module.meta.boxed_types[0];
        assert_eq!(
            boxed_meta.payload,
            mir::Type::Struct(la_arena::Idx::from_raw(0.into()))
        );
        assert_eq!(module.classes[boxed_meta.class].name, "box$D1_SX");
        assert!(module.functions.iter().all(|(_, function)| {
            !function.symbol.starts_with("scoop.eq.")
                && !function.symbol.starts_with("scoop.tostring.")
        }));
    }

    #[test]
    fn boxed_interface_implementations_dispatch_through_adjust_thunks() {
        let mut h = Harness::new();
        let int = h.int;
        let iface = h.interface("Describable", &["describe"]);
        let iface_ty = h.interface_ty(iface);
        let s = h.strukt_with("S", &[("x", int)], &[iface]);
        let s_ty = h.struct_ty(s);
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
                        hir::ExprKind::Box(Box::new(struct_init(&h, s_ty, vec![int_lit(&h, 1)]))),
                        iface_ty,
                    ),
                )],
            },
        );
        let module = lower(&h.finish(main));

        let boxed = boxed_class(&module, "box$D1_SX");
        assert_eq!(boxed.interfaces.len(), 1);
        assert_eq!(boxed.itables.len(), 1);
        let record = &boxed.itables[0];
        assert_eq!(record.interface, boxed.interfaces[0]);
        assert_eq!(record.slots.len(), 1);
        let thunk_symbol = slot_fn(&module, &record.slots[0]);
        assert_eq!(thunk_symbol, "scoop.thunk.D1_SX.Describable.describe");

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
        let (call, _) = statement_call(&entry_statements(&thunk.body)[0]);
        assert!(matches!(call.target.kind, mir::CallKind::Direct));
        let mir::Callee::User(impl_id) = call.target.callee else {
            panic!("the thunk calls a user function")
        };
        assert_eq!(module.functions[impl_id].symbol, "scoop.S.describe");
        assert_eq!(call.args.len(), 1);
        assert!(matches!(&call.args[0].kind, mir::ExprKind::Unbox(operand)
            if matches!(operand.kind, mir::ExprKind::Local(local) if local == thunk.params[0].local)));
    }

    #[test]
    fn is_instance_and_casts_lower_to_runtime_checks() {
        let mut h = Harness::new();
        h.exception("ClassCastException");
        let (int, boolean) = (h.int, h.boolean);
        let s = h.strukt("S", &[("x", int)]);
        let s_ty = h.struct_ty(s);
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
        // payload class. Capabilities are not synthesized from boxing.
        let expected = "\
Module
  struct S (x: Int)
  enum Option$D1_SX
    Some(_1: S)
    None()
  class ClassCastException vtable=0 itables=0
  class box$D1_SX vtable=0 itables=0
  fun main @scoop_main() -> Unit
    bb0 entry
      val is_s: Boolean
        Type Boolean
        IsInstance S
          Type Any
          Local a
      val $cast.1: Any
        Type Any
        Local a
      branch bb1 bb2
        Type Boolean
        Unary BoolNot
          Type Boolean
          IsInstance S
            Type Any
            Local $cast.1
    bb1 if.then.1
      call $call.1: ClassCastException = @scoop.ctor.ClassCastException direct
      throw
        Type ClassCastException
        Local $call.1
    bb2 if.merge.2
      val $ub.2: S
        Type S
        Unbox
          Type Any
          Local $cast.1
      val s2: S
        Type S
        Local $ub.2
      val $cast.3: Any
        Type Any
        Local a
      branch bb3 bb4
        Type Boolean
        IsInstance S
          Type Any
          Local $cast.3
    bb3 if.then.3
      assign $cast.4
        Type Option$D1_SX<S>
        VariantConstruct Option$D1_SX<S> v0
          Type S
          Unbox
            Type Any
            Local $cast.3
      goto bb5
    bb4 if.else.4
      assign $cast.4
        Type Option$D1_SX<S>
        VariantConstruct Option$D1_SX<S> v1
      goto bb5
    bb5 if.merge.5
      val maybe: Option$D1_SX<S>
        Type Option$D1_SX<S>
        Local $cast.4
      return
  fun ctor.ClassCastException @scoop.ctor.ClassCastException() -> ClassCastException
    bb0 entry
      return
        Type ClassCastException
        ClassInit ClassCastException
  entry @scoop_main
";
        assert_eq!(dump(&module), expected);
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
        let point_application = h.class_application_of(point_ty);
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
                            application: point_application,
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
  class Root vtable=0 itables=0
  class Base vtable=0 itables=0
  class Point vtable=0 itables=0
  fun main @scoop_main() -> Unit
    bb0 entry
      call p: Point = @scoop.ctor.Point direct
        Type Int
        IntLiteral 1
      return
  fun ctor.Root @scoop.ctor.Root(label: String) -> Root
    bb0 entry
      return
        Type Root
        ClassInit Root
          Type String
          Local label
  fun ctor.Base @scoop.ctor.Base(name: String) -> Base
    bb0 entry
      return
        Type Base
        ClassInit Base
          Type String
          StringConst @scoop.str.0
          Type String
          Local name
  fun ctor.Point @scoop.ctor.Point(x: Int) -> Point
    bb0 entry
      return
        Type Point
        ClassInit Point
          Type String
          StringConst @scoop.str.2
          Type String
          StringConst @scoop.str.1
          Type Int
          Local x
  str @scoop.str.0 \"root\"
  str @scoop.str.1 \"point\"
  str @scoop.str.2 \"root\"
  entry @scoop_main
";
        assert_eq!(dump(&module), expected);
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
        let c_application = h.class_application_of(c_ty);
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
                            application: c_application,
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
        } = &entry_statements(body)[0].kind
        else {
            panic!("a class property assignment must lower to FieldSet")
        };
        assert!(matches!(object.kind, mir::ExprKind::Local(_)));
        assert!(matches!(value.kind, mir::ExprKind::IntLiteral(3)));
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
        let s_ty = h.struct_ty(s);
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
                        hir::ExprKind::Box(Box::new(struct_init(&h, s_ty, vec![int_lit(&h, 1)]))),
                        any,
                    ),
                )],
            },
        );
        let module = lower(&h.finish(main));

        let boxed = boxed_class(&module, "box$D1_SX");
        assert_eq!(boxed.interfaces.len(), 1);
        assert_eq!(boxed.itables.len(), 1);
        let record = &boxed.itables[0];
        assert_eq!(record.interface, boxed.interfaces[0]);
        assert_eq!(
            slot_fn(&module, &record.slots[0]),
            "scoop.thunk.D1_SX.Describable.describe"
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
            let mir::StatementKind::ValDecl { init, .. } = &entry_statements(body)[index].kind
            else {
                panic!("expected a val declaration")
            };
            let mir::ExprKind::Binary { op, .. } = &init.kind else {
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
        let id = h.method_fn(
            "Base.id",
            base_ty,
            vec![param("this", base_ty, this)],
            int,
            hir::Body {
                locals,
                statements: Vec::new(),
            },
        );
        h.functions[id].method.as_mut().expect("a method").modifier = hir::MethodModifier::Abstract;
        let main = empty_main(&mut h);
        let module = lower(&h.finish(main));

        // The abstract method is emitted (the abstract class's vtable
        // slot references it) and traps like a pure-virtual stub.
        let base_def = &module.classes[class_index(0)];
        assert_eq!(slot_fn(&module, &base_def.vtable[0]), "scoop.Base.id");
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
        assert!(matches!(
            stub.body.blocks[stub.body.entry].terminator,
            mir::Terminator::Trap { .. }
        ));
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
        assert_eq!(slot_fn(&module, &doc_def.vtable[0]), "scoop.Doc.describe");
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
        let s_ty = h.struct_ty(s);
        let s_application = h.struct_application_of(s_ty);
        let any = h.any();
        let mut locals = Arena::new();
        let a = locals.alloc(local("a", any));
        let print_call = expr(
            hir::ExprKind::Call {
                callee: hir::Callable::Function(println_int),
                args: vec![expr(
                    hir::ExprKind::FieldAccess {
                        receiver: Box::new(expr(
                            hir::ExprKind::Unbox(Box::new(local_ref(a, any))),
                            s_ty,
                        )),
                        field: hir::FieldRef::StructField {
                            application: s_application,
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
        let mir::Terminator::Branch { then_block, .. } = body.blocks[body.entry].terminator else {
            panic!("expected a conditional branch")
        };
        let then_body = &body.blocks[then_block].statements;
        let mir::StatementKind::ValDecl { local: ub, init } = &then_body[0].kind else {
            panic!("the unbox must be a val declaration")
        };
        let mir::ExprKind::Unbox(_) = init.kind else {
            panic!("the unbox must be bound to a typed hidden local")
        };
        assert_eq!(
            body.locals[*ub].ty,
            mir::Type::Struct(la_arena::Idx::from_raw(0.into()))
        );
        let (call, _) = statement_call(&then_body[1]);
        assert!(
            matches!(&call.args[0].kind, mir::ExprKind::FieldAccess { receiver, .. }
            if matches!(receiver.kind, mir::ExprKind::Local(local) if local == *ub))
        );
    }
}
