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
mod callable_identities;
mod callables;
mod callback_slots;
mod classes;
mod closures;
mod constructor_slots;
mod functions;
mod nominals;
mod types;

use callback_slots::{PendingForeignCallbackRegistration, finish_foreign_callback_slots};
use closures::{PendingCallableReference, finish_callable_references};
use constructor_slots::{
    PendingClassConstructor, PendingStructConstructor, finish_class_constructor_slots,
    finish_struct_constructor_slots,
};
use functions::PendingFunction;

pub(crate) fn lower(module: &export::Module) -> concrete::Module {
    export::validate_iteration_plans(module)
        .expect("Export HIR iteration plans must pass the complete reader boundary validator");
    Concretizer::new(module).run()
}

pub(crate) fn lower_legacy_executable(
    executable: &export::LegacyExecutableExportHir,
) -> export::LegacyExecutableLocalHir {
    let module = executable.module();
    export::validate_iteration_plans(module)
        .expect("Export HIR iteration plans must pass the complete reader boundary validator");
    let (module, entry) = Concretizer::new(module).run_with_entry(executable.entry());
    export::LegacyExecutableLocalHir::try_new(module, entry)
        .expect("concretization preserves the structurally valid legacy entry")
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
    object_by_backing_class: HashMap<export::ClassId, export::ObjectId>,
    class_constructor_slots: Vec<Option<PendingClassConstructor>>,
    class_constructor_keys: Vec<(export::ClassConstructorId, concrete::ClassId)>,
    class_constructor_by_key:
        HashMap<(export::ClassConstructorId, concrete::ClassId), concrete::ClassConstructorId>,
    struct_constructor_slots: Vec<Option<PendingStructConstructor>>,
    struct_constructor_keys: Vec<(export::StructConstructorId, concrete::StructId)>,
    struct_constructor_by_key:
        HashMap<(export::StructConstructorId, concrete::StructId), concrete::StructConstructorId>,
    extern_functions: Arena<concrete::ExternFunction>,
    extern_map: HashMap<export::ExternFunctionId, concrete::ExternFunctionId>,
    globals: Arena<concrete::Global>,
    global_map: HashMap<export::GlobalId, concrete::GlobalId>,
    initialization_units: Arena<concrete::InitializationUnit>,
    initialization_failure_roots: Arena<concrete::InitializationFailureRoot>,
    objects: Arena<concrete::ObjectDecl>,
    object_types: Arena<concrete::ObjectType>,
    companion_relations: Arena<concrete::CompanionRelation>,
    singleton_values: Arena<concrete::SingletonValue>,
    singleton_published_roots: Arena<concrete::SingletonPublishedRoot>,
    function_slots: Vec<Option<PendingFunction>>,
    function_keys: Vec<FunctionKey>,
    function_by_key: HashMap<FunctionKey, concrete::FunctionId>,
    /// Concrete ordinary bodies supplied by typed derived-equality
    /// applications before their function key enters the emission queue.
    derived_bodies: HashMap<FunctionKey, (concrete::Body, Vec<concrete::LocalId>)>,
    structural_derived_functions:
        HashMap<(export::DerivedEqualityApplicationId, concrete::TypeId), concrete::FunctionId>,
    pending_functions: VecDeque<(FunctionKey, concrete::FunctionId)>,
    emitted_functions: Vec<concrete::FunctionId>,
    overloaded_generic_link_stems: HashSet<export::CallableLinkStem>,
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
    callable_reference_slots: Vec<PendingCallableReference>,
    reference_by_key: HashMap<
        (export::CallableReferenceId, Vec<concrete::TypeId>),
        concrete::CallableReferenceId,
    >,
    function_coercions: Arena<concrete::FunctionCoercion>,
    coercion_by_key:
        HashMap<(export::FunctionCoercionId, Vec<concrete::TypeId>), concrete::FunctionCoercionId>,
    foreign_callback_slots: Vec<PendingForeignCallbackRegistration>,
    foreign_callback_by_key: HashMap<
        (export::ForeignCallbackRegistrationId, Vec<concrete::TypeId>),
        concrete::ForeignCallbackRegistrationId,
    >,
    next_loop_identity: u32,
}

impl<'a> Concretizer<'a> {
    fn lower_nominal_owner(
        &self,
        owner: Option<export::NominalOwner>,
    ) -> Option<concrete::NominalOwner> {
        owner.map(|owner| match owner {
            export::NominalOwner::Class(id) => {
                concrete::NominalOwner::Class(self.source.nominal_identities[id].clone())
            }
            export::NominalOwner::Interface(id) => {
                concrete::NominalOwner::Interface(self.source.nominal_identities[id].clone())
            }
            export::NominalOwner::Struct(id) => {
                concrete::NominalOwner::Struct(self.source.nominal_identities[id].clone())
            }
            export::NominalOwner::Enum(id) => {
                concrete::NominalOwner::Enum(self.source.nominal_identities[id].clone())
            }
            export::NominalOwner::Object(id) => {
                concrete::NominalOwner::Object(self.source.nominal_identities[id].clone())
            }
        })
    }

    fn source_nominal_name(&self, name: &str, owner: Option<export::NominalOwner>) -> String {
        let Some(owner) = owner else {
            return name.to_string();
        };
        let prefix = match owner {
            export::NominalOwner::Class(id) => {
                let declaration = &self.source.classes[id];
                self.source_nominal_name(&declaration.name, declaration.owner)
            }
            export::NominalOwner::Interface(id) => {
                let declaration = &self.source.interfaces[id];
                self.source_nominal_name(&declaration.name, declaration.owner)
            }
            export::NominalOwner::Struct(id) => {
                let declaration = &self.source.structs[id];
                self.source_nominal_name(&declaration.name, declaration.owner)
            }
            export::NominalOwner::Enum(id) => {
                let declaration = &self.source.enums[id];
                self.source_nominal_name(&declaration.name, declaration.owner)
            }
            export::NominalOwner::Object(id) => {
                let declaration = &self.source.objects[id];
                self.source_nominal_name(&declaration.name, declaration.owner)
            }
        };
        format!("{prefix}.{name}")
    }

    fn new(source: &'a export::Module) -> Self {
        let object_by_backing_class = source
            .objects
            .iter()
            .map(|(object, declaration)| (declaration.backing_class, object))
            .collect();
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
            object_by_backing_class,
            class_constructor_slots: Vec::new(),
            class_constructor_keys: Vec::new(),
            class_constructor_by_key: HashMap::new(),
            struct_constructor_slots: Vec::new(),
            struct_constructor_keys: Vec::new(),
            struct_constructor_by_key: HashMap::new(),
            extern_functions: Arena::new(),
            extern_map: HashMap::new(),
            globals: Arena::new(),
            global_map: HashMap::new(),
            initialization_units: Arena::new(),
            initialization_failure_roots: Arena::new(),
            objects: Arena::new(),
            object_types: Arena::new(),
            companion_relations: Arena::new(),
            singleton_values: Arena::new(),
            singleton_published_roots: Arena::new(),
            function_slots: Vec::new(),
            function_keys: Vec::new(),
            function_by_key: HashMap::new(),
            derived_bodies: HashMap::new(),
            structural_derived_functions: HashMap::new(),
            pending_functions: VecDeque::new(),
            emitted_functions: Vec::new(),
            overloaded_generic_link_stems: overloaded_generic_link_stems(source),
            lambdas: Arena::new(),
            lambda_by_key: HashMap::new(),
            anonymous_functions: Arena::new(),
            anonymous_by_key: HashMap::new(),
            local_functions: Arena::new(),
            local_by_key: HashMap::new(),
            callable_reference_slots: Vec::new(),
            reference_by_key: HashMap::new(),
            function_coercions: Arena::new(),
            coercion_by_key: HashMap::new(),
            foreign_callback_slots: Vec::new(),
            foreign_callback_by_key: HashMap::new(),
            next_loop_identity: 0,
        }
    }

    fn run(self) -> concrete::Module {
        self.run_with(|_| ()).0
    }

    fn run_with_entry(self, entry: export::FunctionId) -> (concrete::Module, concrete::FunctionId) {
        self.run_with(|concretizer| {
            concretizer.function_by_key[&FunctionKey::Free {
                source: entry,
                arguments: Vec::new(),
            }]
        })
    }

    fn run_with<Extra>(mut self, finish: impl FnOnce(&Self) -> Extra) -> (concrete::Module, Extra) {
        let unit = self.lower_type(self.source.unit, &[]);
        for kind in export::IntegerKind::ALL {
            let owner = self.source.intrinsic_type_core.integers.owner(kind);
            let source_type = self.source.struct_applications
                [self.source.structs[owner].self_application]
                .canonical_type;
            self.lower_type(source_type, &[]);
        }
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
        for (source_id, declaration) in self.source.objects.iter() {
            let backing_class = self.lower_class_application(
                self.source.classes[declaration.backing_class].self_application,
                &[],
            );
            let source_type = &self.source.object_types[declaration.object_type];
            let canonical_type = self.lower_type(source_type.canonical_type, &[]);
            let object_type = self.object_types.alloc(concrete::ObjectType {
                declaration: concrete::ObjectId::from_raw(source_id.into_raw()),
                representation: backing_class,
                canonical_type,
            });
            assert_eq!(declaration.object_type.into_raw(), object_type.into_raw());
            let object = self.objects.alloc(concrete::ObjectDecl {
                origin: self.source.nominal_identities[source_id].clone(),
                link_stem: declaration.link_stem.clone(),
                name: declaration.name.clone(),
                owner: self.lower_nominal_owner(declaration.owner),
                object_type,
                singleton_value: concrete::SingletonValueId::from_raw(
                    declaration.singleton_value.into_raw(),
                ),
                kind: match declaration.kind {
                    export::ObjectKind::Standalone => concrete::ObjectKind::Standalone,
                    export::ObjectKind::Companion(relation) => concrete::ObjectKind::Companion(
                        concrete::CompanionRelationId::from_raw(relation.into_raw()),
                    ),
                },
                backing_class,
                span: declaration.span,
            });
            assert_eq!(source_id.into_raw(), object.into_raw());
        }
        for (source_id, relation) in self.source.companion_relations.iter() {
            let lowered = self.companion_relations.alloc(concrete::CompanionRelation {
                host: self
                    .lower_nominal_owner(Some(relation.host))
                    .expect("a companion relation always has a nominal host"),
                object: concrete::ObjectId::from_raw(relation.object.into_raw()),
                name: match &relation.name {
                    export::CompanionName::Default => concrete::CompanionName::Default,
                    export::CompanionName::Named(name) => {
                        concrete::CompanionName::Named(name.clone())
                    }
                },
            });
            assert_eq!(source_id.into_raw(), lowered.into_raw());
        }
        for (source_id, source) in self.source.singleton_published_roots.iter() {
            let ty = self.lower_type(source.ty, &[]);
            let root = self
                .singleton_published_roots
                .alloc(concrete::SingletonPublishedRoot {
                    value: concrete::SingletonValueId::from_raw(source.value.into_raw()),
                    ty,
                    link_name: source.link_name.clone(),
                });
            assert_eq!(source_id.into_raw(), root.into_raw());
        }
        for (source_id, source) in self.source.singleton_values.iter() {
            let value = self.singleton_values.alloc(concrete::SingletonValue {
                declaration: concrete::ObjectId::from_raw(source.declaration.into_raw()),
                object_type: concrete::ObjectTypeId::from_raw(source.object_type.into_raw()),
                published_root: concrete::SingletonPublishedRootId::from_raw(
                    source.published_root.into_raw(),
                ),
                initialization: concrete::InitializationUnitId::from_raw(
                    source.initialization.into_raw(),
                ),
            });
            assert_eq!(source_id.into_raw(), value.into_raw());
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
        let source_callback_core = self.source.foreign_callback_core;
        let callback_reusable =
            self.lower_applied_enum_variant_ref(source_callback_core.modes.reusable(), &[]);
        let callback_one_shot =
            self.lower_applied_enum_variant_ref(source_callback_core.modes.one_shot(), &[]);
        let callback_modes = concrete::ForeignCallbackModes::checked(
            &self.enums,
            callback_reusable,
            callback_one_shot,
        )
        .expect("the validated foreign callback mode protocol survives concretization");
        let callback_registered =
            self.lower_applied_enum_variant_ref(source_callback_core.states.registered(), &[]);
        let callback_active =
            self.lower_applied_enum_variant_ref(source_callback_core.states.active(), &[]);
        let callback_completed =
            self.lower_applied_enum_variant_ref(source_callback_core.states.completed(), &[]);
        let callback_failed =
            self.lower_applied_enum_variant_ref(source_callback_core.states.failed(), &[]);
        let callback_states = concrete::ForeignCallbackStates::checked(
            &self.enums,
            callback_registered,
            callback_active,
            callback_completed,
            callback_failed,
        )
        .expect("the validated foreign callback state protocol survives concretization");
        let callback_failure_some = self.lower_applied_enum_variant_field_ref(
            source_callback_core.failure_result.some_payload(),
            &[],
        );
        let callback_failure_none =
            self.lower_applied_enum_variant_ref(source_callback_core.failure_result.none(), &[]);
        let callback_failure_option = concrete::OptionCore::checked(
            &self.enums,
            callback_failure_some,
            callback_failure_none,
        )
        .expect("the validated foreign callback failure protocol survives concretization");
        let callback_throwable =
            self.class_by_key[&(self.source.exception_core.throwable.class(), Vec::new())];
        let callback_failure_result = concrete::ForeignCallbackFailureResult::checked(
            &self.enums,
            &self.types,
            callback_failure_option,
            callback_throwable,
        )
        .expect("foreign callback failure remains the exact Option<Throwable> specialization");

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
                export::InitializationUnitKind::LazySingleton {
                    value,
                    published_root,
                } => concrete::InitializationUnitKind::LazySingleton {
                    value: concrete::SingletonValueId::from_raw(value.into_raw()),
                    published_root: concrete::SingletonPublishedRootId::from_raw(
                        published_root.into_raw(),
                    ),
                },
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
                    identity: self.source.initialization_unit_identities[source_id].clone(),
                    stable_key: source.stable_key.clone(),
                    display_name: source.display_name.clone(),
                    schedule: match source.schedule {
                        export::InitializationSchedule::EagerStartup => {
                            concrete::InitializationSchedule::EagerStartup
                        }
                        export::InitializationSchedule::LazyAccess => {
                            concrete::InitializationSchedule::LazyAccess
                        }
                    },
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
            integers: export::IntegerTypeCore::new(export::IntegerKind::ALL.map(|kind| {
                self.struct_by_key[&(
                    self.source.intrinsic_type_core.integers.owner(kind),
                    Vec::new(),
                )]
            }))
            .expect("validated integer owners remain distinct after concretization"),
            boolean: self.struct_by_key[&(self.source.intrinsic_type_core.boolean, Vec::new())],
            string: self.class_by_key[&(self.source.intrinsic_type_core.string, Vec::new())],
        };

        let extra = finish(&self);
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
        let source_option_core = self.source.option_core;
        let option_core = self
            .enums
            .iter()
            .filter(|(enumeration, _)| {
                self.enum_source[enumeration] == source_option_core.enumeration()
            })
            .map(|(enumeration, _)| {
                let some = concrete::EnumVariantRef::checked(
                    &self.enums,
                    enumeration,
                    concrete::VariantId::from_raw(
                        source_option_core.some_payload().variant().local_index(),
                    ),
                )
                .expect("a concrete Option specialization retains its Some variant");
                let some_payload = concrete::EnumVariantFieldRef::checked(
                    &self.enums,
                    some,
                    source_option_core.some_payload().local_index(),
                )
                .expect("a concrete Option specialization retains its Some payload identity");
                let none = concrete::EnumVariantRef::checked(
                    &self.enums,
                    enumeration,
                    concrete::VariantId::from_raw(source_option_core.none().local_index()),
                )
                .expect("a concrete Option specialization retains its None variant");
                concrete::OptionCore::checked(&self.enums, some_payload, none)
                    .expect("the validated Option shape survives concretization")
            })
            .collect();

        let exact_type_identities =
            concrete::ExactTypeIdentities::from_types(concrete::ExactTypeIdentityInputs {
                types: &self.types,
                function_types: &self.function_types,
                structs: &self.structs,
                enums: &self.enums,
                classes: &self.classes,
                interfaces: &self.interfaces,
                objects: &self.objects,
                intrinsic_core: &intrinsic_type_core,
            })
            .expect("validated concretization produces a total exact-type identity relation");
        let dispatch_slot_identities = self.build_dispatch_slot_identities();
        let identities = self.build_callable_identities(&exact_type_identities);
        let callable_references = finish_callable_references(
            self.callable_reference_slots,
            identities.callable_reference_identities,
        );
        let functions = finish_function_slots(
            self.function_slots,
            identities.function_materializations,
            identities.function_emissions,
        );
        let class_constructors = finish_class_constructor_slots(
            self.class_constructor_slots,
            identities.class_constructor_materializations,
        );
        let struct_constructors = finish_struct_constructor_slots(
            self.struct_constructor_slots,
            identities.struct_constructor_materializations,
        );
        let foreign_callback_registrations = finish_foreign_callback_slots(
            self.foreign_callback_slots,
            identities.foreign_callback_applications,
        );
        let local_value_identities =
            concrete::LocalValueIdentities::from_callables(concrete::LocalValueIdentityInputs {
                callable_applications: &identities.callable_applications,
                functions: &functions,
                lambdas: &self.lambdas,
                anonymous_functions: &self.anonymous_functions,
                local_functions: &self.local_functions,
                callable_references: &callable_references,
                class_constructors: &class_constructors,
                struct_constructors: &struct_constructors,
            })
            .expect("validated concretization produces a total local-value identity relation");

        let module = concrete::Module {
            types: self.types,
            exact_type_identities,
            local_value_identities,
            dispatch_slot_identities,
            callable_applications: identities.callable_applications,
            callback_applications: identities.callback_applications,
            function_types: self.function_types,
            lambdas: self.lambdas,
            anonymous_functions: self.anonymous_functions,
            local_functions: self.local_functions,
            callable_references,
            function_coercions: self.function_coercions,
            foreign_callback_registrations,
            functions,
            extern_functions: self.extern_functions,
            globals: self.globals,
            initialization_units: self.initialization_units,
            initialization_failure_roots: self.initialization_failure_roots,
            objects: self.objects,
            object_types: self.object_types,
            companion_relations: self.companion_relations,
            singleton_values: self.singleton_values,
            singleton_published_roots: self.singleton_published_roots,
            structs: self.structs,
            enums: self.enums,
            classes: self.classes,
            class_constructors,
            struct_constructors,
            interfaces: self.interfaces,
            top_level: self.emitted_functions,
            unit,
            boolean,
            string,
            option_core,
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
                modes: callback_modes,
                states: callback_states,
                failure_result: callback_failure_result,
            },
            intrinsic_type_core,
        };
        (module, extra)
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
            for (index, function) in self.function_slots.iter().enumerate() {
                let Some(function) = function else {
                    continue;
                };
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
                    results.extend(self.function_key_arguments(&self.function_keys[index]));
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

fn finish_function_slots(
    slots: Vec<Option<PendingFunction>>,
    materializations: Vec<concrete::CallableMaterialization>,
    emissions: Vec<concrete::FunctionEmission>,
) -> Arena<concrete::Function> {
    assert_eq!(slots.len(), materializations.len());
    assert_eq!(slots.len(), emissions.len());
    let mut arena = Arena::new();
    for (index, ((slot, materialization), emission)) in slots
        .into_iter()
        .zip(materializations)
        .zip(emissions)
        .enumerate()
    {
        let pending = slot.unwrap_or_else(|| panic!("missing concrete function at index {index}"));
        let id = arena.alloc(pending.finish(materialization, emission));
        assert_eq!(id.into_raw().into_u32() as usize, index);
    }
    arena
}

fn overloaded_generic_link_stems(module: &export::Module) -> HashSet<export::CallableLinkStem> {
    let mut counts = HashMap::<export::CallableLinkStem, usize>::new();
    for (_, function) in module.functions.iter() {
        if !matches!(function.genericity, export::FunctionGenericity::Plain) {
            *counts.entry(function.link_stem.clone()).or_default() += 1;
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
