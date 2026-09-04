//! Construction of the local, fully concrete HIR graph.
//!
//! Export-side HIR is the checked template/declaration graph.  This pass owns
//! the fixed-point instantiation closure and emits a distinct id domain for
//! MIR.  MIR never receives the export graph.

use std::collections::{HashMap, HashSet, VecDeque};

use la_arena::Arena;
use scoop_hir as export;
use scoop_hir::concrete;

mod body;
mod callables;
mod classes;
mod closures;
mod functions;
mod nominals;
mod types;

pub(crate) fn lower(module: &export::Module) -> concrete::Module {
    Concretizer::new(module).run()
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum FunctionKey {
    Free {
        source: export::FunctionId,
        arguments: Vec<concrete::TypeId>,
    },
    Method {
        source: export::FunctionId,
        owner: concrete::MethodOwner,
        specialization: MethodRequest,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum MethodRequest {
    Plain,
    Generic {
        definition: export::GenericMethodId,
        method_arguments: export::NonEmptyVec<concrete::TypeId>,
    },
}

#[derive(Debug, Clone)]
enum ConcreteApplicationRepresentation {
    Declared,
    Intrinsic(concrete::IntrinsicTypeRepresentation),
}

impl FunctionKey {
    fn source(&self) -> export::FunctionId {
        match *self {
            Self::Free { source, .. } | Self::Method { source, .. } => source,
        }
    }
}

struct Concretizer<'a> {
    source: &'a export::Module,
    types: Arena<concrete::Type>,
    type_by_kind: HashMap<concrete::TypeKind, concrete::TypeId>,
    function_types: Arena<concrete::FunctionType>,
    function_type_by_signature:
        HashMap<(bool, Vec<concrete::TypeId>, concrete::TypeId), concrete::FunctionTypeId>,
    structs: Arena<concrete::StructDef>,
    struct_by_key: HashMap<(export::StructId, Vec<concrete::TypeId>), concrete::StructId>,
    struct_type: HashMap<concrete::StructId, concrete::TypeId>,
    struct_source: HashMap<concrete::StructId, export::StructId>,
    enums: Arena<concrete::EnumDef>,
    enum_by_key: HashMap<(export::EnumId, Vec<concrete::TypeId>), concrete::EnumId>,
    enum_type: HashMap<concrete::EnumId, concrete::TypeId>,
    enum_source: HashMap<concrete::EnumId, export::EnumId>,
    interfaces: Arena<concrete::InterfaceDef>,
    interface_by_key: HashMap<(export::InterfaceId, Vec<concrete::TypeId>), concrete::InterfaceId>,
    interface_type: HashMap<concrete::InterfaceId, concrete::TypeId>,
    interface_slot_by_source:
        HashMap<(concrete::InterfaceId, export::InterfaceMethodId), concrete::InterfaceMethodSlot>,
    virtual_method_by_source: HashMap<export::VirtualMethodId, concrete::VirtualMethodId>,
    classes: Arena<concrete::ClassDef>,
    class_by_key: HashMap<(export::ClassId, Vec<concrete::TypeId>), concrete::ClassId>,
    class_type: HashMap<concrete::ClassId, concrete::TypeId>,
    class_source: HashMap<concrete::ClassId, export::ClassId>,
    class_constructor_slots: Vec<Option<concrete::ClassConstructor>>,
    class_constructor_by_key:
        HashMap<(export::ClassConstructorId, concrete::ClassId), concrete::ClassConstructorId>,
    struct_constructor_slots: Vec<Option<concrete::StructConstructor>>,
    struct_constructor_by_key:
        HashMap<(export::StructConstructorId, concrete::StructId), concrete::StructConstructorId>,
    extern_functions: Arena<concrete::ExternFunction>,
    extern_map: HashMap<export::ExternFunctionId, concrete::ExternFunctionId>,
    globals: Arena<concrete::Global>,
    global_map: HashMap<export::GlobalId, concrete::GlobalId>,
    initialization_units: Arena<concrete::InitializationUnit>,
    initialization_failure_roots: Arena<concrete::InitializationFailureRoot>,
    function_slots: Vec<Option<concrete::Function>>,
    function_by_key: HashMap<FunctionKey, concrete::FunctionId>,
    /// Concrete ordinary bodies supplied by typed derived-equality
    /// applications before their function key enters the emission queue.
    derived_bodies: HashMap<FunctionKey, (concrete::Body, Vec<concrete::LocalId>)>,
    structural_derived_functions:
        HashMap<(export::DerivedEqualityApplicationId, concrete::TypeId), concrete::FunctionId>,
    pending_functions: VecDeque<(FunctionKey, concrete::FunctionId)>,
    emitted_functions: Vec<concrete::FunctionId>,
    overloaded_generic_names: HashSet<String>,
    lambdas: Arena<concrete::Lambda>,
    lambda_by_key: HashMap<(export::LambdaId, Vec<concrete::TypeId>), concrete::LambdaId>,
    anonymous_functions: Arena<concrete::AnonymousFunction>,
    anonymous_by_key: HashMap<
        (export::AnonymousFunctionId, Vec<concrete::TypeId>),
        concrete::AnonymousFunctionId,
    >,
    local_functions: Arena<concrete::LocalFunction>,
    local_by_key:
        HashMap<(export::LocalFunctionId, Vec<concrete::TypeId>), concrete::LocalFunctionId>,
    callable_references: Arena<concrete::CallableReference>,
    reference_by_key: HashMap<
        (export::CallableReferenceId, Vec<concrete::TypeId>),
        concrete::CallableReferenceId,
    >,
    function_coercions: Arena<concrete::FunctionCoercion>,
    coercion_by_key:
        HashMap<(export::FunctionCoercionId, Vec<concrete::TypeId>), concrete::FunctionCoercionId>,
    foreign_callback_registrations: Arena<concrete::ForeignCallbackRegistration>,
    foreign_callback_by_key: HashMap<
        (export::ForeignCallbackRegistrationId, Vec<concrete::TypeId>),
        concrete::ForeignCallbackRegistrationId,
    >,
}

impl<'a> Concretizer<'a> {
    fn new(source: &'a export::Module) -> Self {
        Self {
            source,
            types: Arena::new(),
            type_by_kind: HashMap::new(),
            function_types: Arena::new(),
            function_type_by_signature: HashMap::new(),
            structs: Arena::new(),
            struct_by_key: HashMap::new(),
            struct_type: HashMap::new(),
            struct_source: HashMap::new(),
            enums: Arena::new(),
            enum_by_key: HashMap::new(),
            enum_type: HashMap::new(),
            enum_source: HashMap::new(),
            interfaces: Arena::new(),
            interface_by_key: HashMap::new(),
            interface_type: HashMap::new(),
            interface_slot_by_source: HashMap::new(),
            virtual_method_by_source: HashMap::new(),
            classes: Arena::new(),
            class_by_key: HashMap::new(),
            class_type: HashMap::new(),
            class_source: HashMap::new(),
            class_constructor_slots: Vec::new(),
            class_constructor_by_key: HashMap::new(),
            struct_constructor_slots: Vec::new(),
            struct_constructor_by_key: HashMap::new(),
            extern_functions: Arena::new(),
            extern_map: HashMap::new(),
            globals: Arena::new(),
            global_map: HashMap::new(),
            initialization_units: Arena::new(),
            initialization_failure_roots: Arena::new(),
            function_slots: Vec::new(),
            function_by_key: HashMap::new(),
            derived_bodies: HashMap::new(),
            structural_derived_functions: HashMap::new(),
            pending_functions: VecDeque::new(),
            emitted_functions: Vec::new(),
            overloaded_generic_names: overloaded_generic_names(source),
            lambdas: Arena::new(),
            lambda_by_key: HashMap::new(),
            anonymous_functions: Arena::new(),
            anonymous_by_key: HashMap::new(),
            local_functions: Arena::new(),
            local_by_key: HashMap::new(),
            callable_references: Arena::new(),
            reference_by_key: HashMap::new(),
            function_coercions: Arena::new(),
            coercion_by_key: HashMap::new(),
            foreign_callback_registrations: Arena::new(),
            foreign_callback_by_key: HashMap::new(),
        }
    }

    fn run(mut self) -> concrete::Module {
        let unit = self.lower_type(self.source.unit, &[]);
        let int = self.lower_type(self.source.int, &[]);
        let boolean = self.lower_type(self.source.boolean, &[]);
        let string = self.lower_type(self.source.string, &[]);

        self.lower_extern_functions();
        self.lower_globals();

        // Non-generic aggregate declarations and source functions are local
        // concrete entities even when no expression happens to mention them.
        for (_, declaration) in self.source.structs.iter() {
            if declaration.type_params.is_empty() {
                self.lower_struct_application(declaration.self_application, &[]);
            }
        }
        for (id, declaration) in self.source.enums.iter() {
            if declaration.type_params.is_empty() {
                self.ensure_enum(id, Vec::new());
            }
        }
        for (id, declaration) in self.source.interfaces.iter() {
            if declaration.type_params.is_empty() {
                self.ensure_interface(id, Vec::new());
            }
        }
        for (_, declaration) in self.source.classes.iter() {
            if declaration.type_params.is_empty() {
                self.lower_class_application(declaration.self_application, &[]);
            }
        }
        for (id, function) in self.source.functions.iter() {
            if function.method.is_none()
                && function.type_param_count() == 0
                && self.is_emittable_source_function(id)
            {
                self.request_function(id, Vec::new());
            }
        }
        for (_, request) in self.source.instantiations.iter() {
            let function = self.source.generic_functions[request.generic].function;
            if !self.is_emittable_source_function(function)
                || request
                    .type_args
                    .iter()
                    .any(|argument| export_type_has_param(self.source, *argument))
            {
                continue;
            }
            let arguments = request
                .type_args
                .iter()
                .map(|argument| self.lower_type(*argument, &[]))
                .collect();
            self.request_function(function, arguments);
        }
        self.drain_pending_functions();
        let coroutine_protocols = self.build_coroutine_protocols();
        self.drain_pending_functions();
        let callback_mode = self.ensure_enum(self.source.foreign_callback_core.mode, Vec::new());
        let callback_state = self.ensure_enum(self.source.foreign_callback_core.state, Vec::new());

        for (source_id, source) in self.source.initialization_failure_roots.iter() {
            let id = self
                .initialization_failure_roots
                .alloc(concrete::InitializationFailureRoot {
                    unit: concrete::InitializationUnitId::from_raw(source.unit.into_raw()),
                });
            assert_eq!(source_id.into_raw(), id.into_raw());
        }
        for (source_id, source) in self.source.initialization_units.iter() {
            let kind = match source.kind {
                export::InitializationUnitKind::EagerTopLevel { storage, .. } => {
                    concrete::InitializationUnitKind::EagerTopLevel {
                        storage: self.global_map[&storage],
                    }
                }
            };
            let function = |source| {
                self.function_by_key[&FunctionKey::Free {
                    source,
                    arguments: Vec::new(),
                }]
            };
            let id = self
                .initialization_units
                .alloc(concrete::InitializationUnit {
                    stable_key: source.stable_key.clone(),
                    kind,
                    initializer: function(source.initializer),
                    ensure: function(source.ensure),
                    failure_root: concrete::InitializationFailureRootId::from_raw(
                        source.failure_root.into_raw(),
                    ),
                    dependencies: source
                        .dependencies
                        .iter()
                        .map(|dependency| concrete::InitializationDependency {
                            unit: concrete::InitializationUnitId::from_raw(
                                dependency.unit.into_raw(),
                            ),
                        })
                        .collect(),
                });
            assert_eq!(source_id.into_raw(), id.into_raw());
        }

        let intrinsic_type_core = concrete::IntrinsicTypeCore {
            int: self.struct_by_key[&(self.source.intrinsic_type_core.int, Vec::new())],
            uint: self.struct_by_key[&(self.source.intrinsic_type_core.uint, Vec::new())],
            boolean: self.struct_by_key[&(self.source.intrinsic_type_core.boolean, Vec::new())],
            string: self.class_by_key[&(self.source.intrinsic_type_core.string, Vec::new())],
        };

        let functions = arena_from_complete_slots(self.function_slots, "concrete function");
        let class_constructors =
            arena_from_complete_slots(self.class_constructor_slots, "concrete class constructor");
        let struct_constructors =
            arena_from_complete_slots(self.struct_constructor_slots, "concrete struct constructor");
        let entry = self.function_by_key[&FunctionKey::Free {
            source: self.source.entry,
            arguments: Vec::new(),
        }];
        let lower_exception = |exception: export::CompilerException| concrete::CompilerException {
            constructor: {
                let class = self.class_by_key[&(exception.class(), Vec::new())];
                concrete::ZeroArgClassConstructor {
                    class,
                    callable: self.class_constructor_by_key[&(exception.callable(), class)],
                }
            },
        };
        let source_exception_core = self.source.exception_core;
        let message_constructor = source_exception_core.illegal_state_message_constructor;
        let message_class = self.class_by_key[&(message_constructor.class, Vec::new())];
        let option = &self.source.enums[self.source.option_enum];
        let option_variant = |name: &str| {
            concrete::VariantId::from_raw(
                option
                    .variants
                    .iter()
                    .position(|variant| variant.name == name)
                    .expect("hir-lower validates the core Option variants") as u32,
            )
        };

        concrete::Module {
            types: self.types,
            function_types: self.function_types,
            lambdas: self.lambdas,
            anonymous_functions: self.anonymous_functions,
            local_functions: self.local_functions,
            callable_references: self.callable_references,
            function_coercions: self.function_coercions,
            foreign_callback_registrations: self.foreign_callback_registrations,
            functions,
            extern_functions: self.extern_functions,
            globals: self.globals,
            initialization_units: self.initialization_units,
            initialization_failure_roots: self.initialization_failure_roots,
            structs: self.structs,
            enums: self.enums,
            classes: self.classes,
            class_constructors,
            struct_constructors,
            interfaces: self.interfaces,
            top_level: self.emitted_functions,
            unit,
            int,
            boolean,
            string,
            option_variants: (option_variant("Some"), option_variant("None")),
            exception_core: concrete::CompilerExceptionCore {
                throwable: lower_exception(source_exception_core.throwable),
                unwrap_exception: lower_exception(source_exception_core.unwrap_exception),
                class_cast_exception: lower_exception(source_exception_core.class_cast_exception),
                arithmetic_exception: lower_exception(source_exception_core.arithmetic_exception),
                index_out_of_bounds_exception: lower_exception(
                    source_exception_core.index_out_of_bounds_exception,
                ),
                illegal_state_exception: lower_exception(
                    source_exception_core.illegal_state_exception,
                ),
                illegal_state_message_constructor: concrete::MessageClassConstructor {
                    class: message_class,
                    callable: self.class_constructor_by_key
                        [&(message_constructor.constructor, message_class)],
                },
            },
            coroutine_protocols,
            foreign_callback_core: concrete::ForeignCallbackCore {
                mode: callback_mode,
                state: callback_state,
            },
            intrinsic_type_core,
            entry,
        }
    }

    fn drain_pending_functions(&mut self) {
        while let Some((key, id)) = self.pending_functions.pop_front() {
            let function = self.lower_function(&key);
            let slot = id.into_raw().into_u32() as usize;
            assert!(self.function_slots[slot].replace(function).is_none());
        }
    }

    fn build_coroutine_protocols(&mut self) -> Vec<concrete::CoroutineProtocol> {
        let core = self.source.coroutine_core;
        let mut protocols = Vec::new();
        loop {
            self.drain_pending_functions();
            let mut results = Vec::new();
            for function in self.function_slots.iter().flatten() {
                if function.is_suspend && matches!(function.kind, concrete::FunctionKind::User(_)) {
                    results.push(function.return_ty);
                }
                if matches!(
                    function.kind,
                    concrete::FunctionKind::Intrinsic(intrinsic)
                        if matches!(
                            intrinsic.kind,
                            concrete::IntrinsicFunctionKind::CoroutineStart
                                | concrete::IntrinsicFunctionKind::CoroutineSuspend
                        )
                ) {
                    results.extend(self.concrete_function_arguments(function));
                }
            }
            // Suspend function-value variance bridges are synthesized by MIR
            // and use the target function type's result. Include those
            // concrete results in HIR's closed coroutine protocol set too.
            results.extend(
                self.function_types.iter().filter_map(|(_, function)| {
                    function.is_suspend.then_some(function.return_type)
                }),
            );
            results.sort_by_key(|id| id.into_raw().into_u32());
            results.dedup();
            results.retain(|result| {
                !protocols
                    .iter()
                    .any(|protocol: &concrete::CoroutineProtocol| protocol.result_type == *result)
            });
            if results.is_empty() {
                break;
            }
            protocols.extend(results.into_iter().map(|result_type| {
                let continuation = self.ensure_interface(core.continuation, vec![result_type]);
                let suspend_task = self.ensure_interface(core.suspend_task, vec![result_type]);
                let suspend_registration =
                    self.ensure_interface(core.suspend_registration, vec![result_type]);
                concrete::CoroutineProtocol {
                    result_type,
                    continuation,
                    suspend_task,
                    suspend_registration,
                    start_coroutine: self.request_function(core.start_coroutine, vec![result_type]),
                    suspend_coroutine: self
                        .request_function(core.suspend_coroutine, vec![result_type]),
                    continuation_resume: self.request_method(
                        core.continuation_resume,
                        concrete::MethodOwner::Interface(continuation),
                        MethodRequest::Plain,
                    ),
                    continuation_resume_with_exception: self.request_method(
                        core.continuation_resume_with_exception,
                        concrete::MethodOwner::Interface(continuation),
                        MethodRequest::Plain,
                    ),
                    suspend_task_run: self.request_method(
                        core.suspend_task_run,
                        concrete::MethodOwner::Interface(suspend_task),
                        MethodRequest::Plain,
                    ),
                    suspend_registration_register: self.request_method(
                        core.suspend_registration_register,
                        concrete::MethodOwner::Interface(suspend_registration),
                        MethodRequest::Plain,
                    ),
                }
            }));
        }
        protocols
    }
}

fn remap_idx<S, T>(id: la_arena::Idx<S>) -> la_arena::Idx<T> {
    la_arena::Idx::from_raw(id.into_raw())
}

fn arena_from_complete_slots<T>(slots: Vec<Option<T>>, what: &str) -> Arena<T> {
    let mut arena = Arena::new();
    for (index, slot) in slots.into_iter().enumerate() {
        let value = slot.unwrap_or_else(|| panic!("missing {what} at index {index}"));
        let id = arena.alloc(value);
        assert_eq!(id.into_raw().into_u32() as usize, index);
    }
    arena
}

fn overloaded_generic_names(module: &export::Module) -> HashSet<String> {
    let mut counts = HashMap::<String, usize>::new();
    for (_, function) in module.functions.iter() {
        if !matches!(function.genericity, export::FunctionGenericity::Plain) {
            *counts.entry(function.name.clone()).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .filter_map(|(name, count)| (count > 1).then_some(name))
        .collect()
}

fn export_type_has_param(module: &export::Module, ty: export::TypeId) -> bool {
    match &module.types[ty] {
        export::Type::Param(_) => true,
        export::Type::Ptr(element) => export_type_has_param(module, *element),
        export::Type::Tuple(elements) => elements
            .iter()
            .any(|element| export_type_has_param(module, *element)),
        export::Type::Function(function) | export::Type::FunPtr(function) => {
            let function = &module.function_types[*function];
            function
                .parameter_types
                .iter()
                .any(|parameter| export_type_has_param(module, *parameter))
                || export_type_has_param(module, function.return_type)
        }
        export::Type::Class(application) => module.class_applications[*application]
            .arguments
            .iter()
            .any(|argument| export_type_has_param(module, *argument)),
        export::Type::Struct(application) => module.struct_applications[*application]
            .arguments
            .iter()
            .any(|argument| export_type_has_param(module, *argument)),
        export::Type::Interface(application) => module.interface_applications[*application]
            .arguments
            .iter()
            .any(|argument| export_type_has_param(module, *argument)),
        export::Type::Enum(application) => module.enum_applications[*application]
            .arguments
            .iter()
            .any(|argument| export_type_has_param(module, *argument)),
        _ => false,
    }
}
