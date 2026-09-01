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
mod coroutine;
mod dispatch;
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
        let mut interface_methods = module
            .interfaces
            .iter()
            .map(|(id, interface)| (id, vec![None; interface.methods.len()]))
            .collect::<HashMap<_, _>>();
        for (hir_id, function) in module.functions.iter() {
            let Some(method) = function.method else {
                continue;
            };
            // Interface methods are signature-only shells: dispatch goes
            // through the itable, so their declarations only provide the
            // complete indirect-call signature. The typed dispatch identity
            // is authoritative; MIR does not infer this role from the owner.
            let hir::MethodDispatch::Interface { interface, slot } = method.dispatch else {
                continue;
            };
            let mir_id = self.declare_interface_method(module, hir_id);
            let previous = interface_methods
                .get_mut(&interface)
                .expect("the interface method names a local interface")[slot.into_raw() as usize]
                .replace(mir_id);
            assert!(
                previous.is_none(),
                "concrete HIR emits one declaration per interface slot"
            );
        }
        for (hir_id, slots) in interface_methods {
            let mir_id = self.interfaces.mir_id(hir_id);
            let mut methods = Vec::with_capacity(slots.len());
            for (slot, function) in slots.into_iter().enumerate() {
                methods.push(
                    function
                        .unwrap_or_else(|| self.declare_interface_signature(module, hir_id, slot)),
                );
            }
            self.interfaces.defs[mir_id].methods = methods;
        }
        // Bound callable-reference invoke bodies preserve virtual/interface
        // dispatch, so closure materialization needs completed slot tables.
        // Dispatch itself only depends on declared methods, not constructors
        // or lowered source bodies.
        self.compute_dispatch(module, &class_order);
        self.declare_closures(module);
        // Constructor functions are declared from HIR's complete hidden
        // callable arena; MIR does not rediscover instantiability from class
        // modifiers or representation shape.
        let mut ctor_functions = Vec::new();
        for (constructor_id, _) in module.class_constructors.iter() {
            let id = self.declare_ctor(module, constructor_id);
            ctor_functions.push((constructor_id, id));
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
        for (constructor_id, mir_id) in ctor_functions {
            let (params, return_ty, body) = self.lower_ctor(module, constructor_id);
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
        _receiver_ty: hir::TypeId,
        callable: hir::Callable,
    ) -> mir::CallKind {
        let function = module.callable_function(callable);
        let declaration = &module.functions[function];
        let method = declaration
            .method
            .expect("a bound member reference names method metadata");
        match method.dispatch {
            hir::MethodDispatch::Direct | hir::MethodDispatch::FinalOverride(_) => {
                mir::CallKind::Direct
            }
            hir::MethodDispatch::Virtual(family) => {
                let hir::TypeKind::Class(class) = module.types[method.owner].kind else {
                    unreachable!("a virtual family belongs to a class method")
                };
                let slot = self.method_slots[&self.class_map[&class]][&family];
                mir::CallKind::Virtual { slot }
            }
            hir::MethodDispatch::Interface { interface, slot } => mir::CallKind::Interface {
                interface: self.interfaces.mir_id(interface),
                slot: slot.into_raw(),
            },
        }
    }

    /// Declare an interface method as a signature-only shell that is never
    /// emitted. Virtual interface calls name it so LIR receives the complete
    /// indirect-call parameter and return types.
    fn declare_interface_method(
        &mut self,
        module: &hir::Module,
        hir_id: hir::FunctionId,
    ) -> mir::FunctionId {
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
        id
    }

    /// Materialize an interface slot that has no callable use in this cone.
    /// Local-concrete HIR carries its complete signature on the interface
    /// definition, so MIR can still give every slot a typed function entity
    /// without waiting for a call site or reconstructing it from a name.
    fn declare_interface_signature(
        &mut self,
        module: &hir::Module,
        interface: hir::InterfaceId,
        slot: usize,
    ) -> mir::FunctionId {
        let declaration = &module.interfaces[interface];
        let method = &declaration.methods[slot];
        let owner = mir::Type::Interface(self.interfaces.mir_id(interface));
        let mut locals = Arena::new();
        let this = locals.alloc(mir::Local {
            name: "this".to_string(),
            ty: owner.clone(),
            mutable: false,
        });
        let mut params = vec![mir::Param {
            name: "this".to_string(),
            ty: owner,
            local: this,
        }];
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        params.extend(method.params.iter().map(|param| {
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
        }));
        let return_ty = types.lower(
            method.return_ty,
            &mut self.enums,
            &mut self.structs,
            &mut self.interfaces,
            &mut self.shell,
        );
        let name = format!("{}.{}", declaration.name, method.name);
        let symbol = format!(
            "scoop.$interface_signature.{}.{}",
            interface.into_raw().into_u32(),
            slot
        );
        self.functions.alloc(mir::Function {
            gc_effect: lower_gc_effect(method.attributes.gc_effect),
            name,
            symbol,
            params,
            return_ty,
            body: mir::Body::unreachable(locals),
        })
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

    /// Declare the constructor function of one class (`scoop.ctor.
    /// <Class>`): parameters are the constructor properties in
    /// declaration order; the body is filled by `lower_ctor`.
    fn declare_ctor(
        &mut self,
        module: &hir::Module,
        constructor_id: hir::ClassConstructorId,
    ) -> mir::FunctionId {
        let constructor = &module.class_constructors[constructor_id];
        let decl = &module.classes[constructor.class];
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
        self.ctors.insert(constructor_id, id);
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
        constructor_id: hir::ClassConstructorId,
    ) -> (Vec<mir::Param>, mir::Type, smir::Body) {
        let constructor = module.class_constructors[constructor_id].clone();
        let hir_id = constructor.class;
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
        assert_eq!(
            decl.declared_constructor().len(),
            constructor.params.len(),
            "the typed constructor signature covers every source parameter"
        );
        let mut params = Vec::new();
        let mut own = Vec::new();
        for (field, parameter_type) in decl
            .declared_constructor()
            .iter()
            .zip(constructor.params.iter().copied())
        {
            let ty = lowerer.lower_type(parameter_type);
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
        let return_ty = lowerer.lower_type(constructor.return_type);
        assert_eq!(
            return_ty,
            mir::Type::Class(mir_id),
            "the hidden constructor returns its owning concrete class"
        );
        let body = smir::Body {
            locals: lowerer.locals,
            statements: vec![smir::Statement {
                kind: smir::StatementKind::Return {
                    value: Some(smir::Expr::new(
                        return_ty.clone(),
                        smir::ExprKind::ClassInit {
                            class_id: mir_id,
                            args,
                        },
                    )),
                },
                span: decl.span,
            }],
        };
        (params, return_ty, body)
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

mod body;

use body::BodyLowerer;

#[cfg(test)]
mod tests;
