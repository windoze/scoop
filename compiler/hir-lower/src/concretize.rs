//! Construction of the local, fully concrete HIR graph.
//!
//! Export-side HIR is the checked template/declaration graph.  This pass owns
//! the fixed-point instantiation closure and emits a distinct id domain for
//! MIR.  MIR never receives the export graph.

use std::collections::{HashMap, VecDeque};

use la_arena::Arena;
use scoop_hir as export;
use scoop_hir::concrete;

mod automatic;
mod body;
mod callable_identities;
mod callables;
mod callback_slots;
mod classes;
mod closures;
mod constructor_slots;
mod constructor_work;
mod functions;
mod imported_constructors;
mod imported_functions;
mod imported_nominals;
mod initialization;
mod initializing_fields;
mod interfaces;
mod nominals;
mod objects;
mod protocols;
mod runtime_exceptions;
mod types;

use automatic::AutomaticNominalRoots;
use callback_slots::{PendingForeignCallbackRegistration, finish_foreign_callback_slots};
use closures::{PendingCallableReference, finish_callable_references};
use constructor_slots::{
    PendingClassConstructor, PendingStructConstructor, finish_class_constructor_slots,
    finish_struct_constructor_slots,
};
use functions::PendingFunction;
use initialization::InitializationRequest;

pub(crate) fn lower(module: &export::Module) -> concrete::Module {
    Concretizer::new(module)
        .expect("validated declaration roots have a complete materialization closure")
        .run()
}

pub(crate) fn lower_output(
    output: &export::ExportHirOutput,
    requirements: &export::PublicNominalShapeRequirementsV1,
) -> Result<export::LocalConcreteHirOutput, Vec<scoop_ast::Diagnostic>> {
    let module = output.module();
    let concretizer = Concretizer::new(module).map_err(|error| {
        vec![scoop_ast::Diagnostic::at(
            scoop_ast::Span::new(0, 0),
            format!("failed to project automatic nominal roots: {error}"),
        )]
    })?;
    let (module, output_kind) = match output.output_kind() {
        export::ConeOutputKind::Library => {
            (concretizer.run(), export::LocalConeOutputKind::Library)
        }
        export::ConeOutputKind::Executable { local_entry } => {
            let (module, entry) =
                concretizer.run_with_entry(local_entry.local_function().function());
            let entry = export::ConcreteExecutableEntry::try_new(&module, local_entry, entry)
                .expect("concretization preserves the validated executable entry");
            (
                module,
                export::LocalConeOutputKind::Executable {
                    local_entry: Box::new(entry),
                },
            )
        }
    };
    runtime_exceptions::check_runtime_layout(&module)?;
    let materialization = export::LocalShapeSupportPlan::try_new(&module, requirements)
        .expect("validated public shape roots survive concretization");
    Ok(
        export::LocalConcreteHirOutput::try_new(module, output_kind, materialization)
            .expect("concretization produces a structurally valid closed output"),
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum FunctionKey {
    Imported {
        source: export::ImportedGenericCallableTemplateId,
        arguments: Vec<concrete::TypeId>,
    },
    ImportedMethod {
        source: export::ImportedGenericCallableTemplateId,
        owner: concrete::MethodOwner,
        method_arguments: Vec<concrete::TypeId>,
    },
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
    fn source(&self) -> FunctionSource {
        match *self {
            Self::Free { source, .. } | Self::Method { source, .. } => {
                FunctionSource::Local(source)
            }
            Self::Imported { source, .. } | Self::ImportedMethod { source, .. } => {
                FunctionSource::Imported(source)
            }
        }
    }
}

#[derive(Clone, Copy)]
enum FunctionSource {
    Local(export::FunctionId),
    Imported(export::ImportedGenericCallableTemplateId),
}

struct Concretizer<'a> {
    source: &'a export::Module,
    automatic: AutomaticNominalRoots,
    core: &'a export::CoreProtocols,
    types: Arena<concrete::Type>,
    type_by_kind: HashMap<concrete::TypeKind, concrete::TypeId>,
    function_types: Arena<concrete::FunctionType>,
    function_type_by_signature:
        HashMap<(bool, Vec<concrete::TypeId>, concrete::TypeId), concrete::FunctionTypeId>,
    structs: Arena<concrete::StructDef>,
    struct_by_key: HashMap<(export::StructId, Vec<concrete::TypeId>), concrete::StructId>,
    imported_structs: HashMap<(export::SourceNominalId, Vec<concrete::TypeId>), concrete::StructId>,
    imported_enums: HashMap<(export::SourceNominalId, Vec<concrete::TypeId>), concrete::EnumId>,
    imported_classes: HashMap<(export::SourceNominalId, Vec<concrete::TypeId>), concrete::ClassId>,
    imported_interface_families: HashMap<export::SourceNominalId, concrete::InterfaceFamilyId>,
    imported_interfaces:
        HashMap<(export::SourceNominalId, Vec<concrete::TypeId>), concrete::InterfaceId>,
    struct_type: HashMap<concrete::StructId, concrete::TypeId>,
    struct_source: HashMap<concrete::StructId, export::StructId>,
    enums: Arena<concrete::EnumDef>,
    enum_by_key: HashMap<(export::EnumId, Vec<concrete::TypeId>), concrete::EnumId>,
    enum_type: HashMap<concrete::EnumId, concrete::TypeId>,
    enum_source: HashMap<concrete::EnumId, export::EnumId>,
    interfaces: Arena<concrete::InterfaceDef>,
    interface_by_key: HashMap<(export::InterfaceId, Vec<concrete::TypeId>), concrete::InterfaceId>,
    interface_type: HashMap<concrete::InterfaceId, concrete::TypeId>,
    interface_slot_by_source: HashMap<
        (
            concrete::InterfaceId,
            scoop_identity::PersistentDispatchSlotId,
        ),
        concrete::InterfaceMethodSlot,
    >,
    virtual_method_by_source: HashMap<export::VirtualMethodId, concrete::VirtualMethodId>,
    classes: Arena<concrete::ClassDef>,
    class_by_key: HashMap<(export::ClassId, Vec<concrete::TypeId>), concrete::ClassId>,
    class_type: HashMap<concrete::ClassId, concrete::TypeId>,
    class_source: HashMap<concrete::ClassId, export::ClassId>,
    object_by_backing_class: HashMap<export::ClassId, export::ObjectId>,
    class_constructor_slots: Vec<Option<PendingClassConstructor>>,
    class_constructor_keys: Vec<(constructor_work::ClassConstructorSource, concrete::ClassId)>,
    class_constructor_by_key: HashMap<
        (constructor_work::ClassConstructorSource, concrete::ClassId),
        concrete::ClassConstructorId,
    >,
    struct_constructor_slots: Vec<Option<PendingStructConstructor>>,
    struct_constructor_keys: Vec<(
        constructor_work::StructConstructorSource,
        concrete::StructId,
    )>,
    struct_constructor_by_key: HashMap<
        (
            constructor_work::StructConstructorSource,
            concrete::StructId,
        ),
        concrete::StructConstructorId,
    >,
    extern_functions: Arena<concrete::ExternFunction>,
    extern_map: HashMap<export::ExternFunctionId, concrete::ExternFunctionId>,
    globals: Arena<concrete::Global>,
    global_map: HashMap<export::GlobalId, concrete::GlobalId>,
    initialization_units: Arena<concrete::InitializationUnit>,
    initialization_map: HashMap<export::InitializationUnitId, concrete::InitializationUnitId>,
    initialization_requests: Vec<InitializationRequest>,
    pending_initializations: VecDeque<export::InitializationUnitId>,
    initialization_function_units: HashMap<export::FunctionId, export::InitializationUnitId>,
    initialization_failure_roots: Arena<concrete::InitializationFailureRoot>,
    objects: Arena<concrete::ObjectDecl>,
    object_types: Arena<concrete::ObjectType>,
    object_type_map: HashMap<export::ObjectTypeId, concrete::ObjectTypeId>,
    singleton_value_map: HashMap<export::SingletonValueId, concrete::SingletonValueId>,
    singleton_root_map:
        HashMap<export::SingletonPublishedRootId, concrete::SingletonPublishedRootId>,
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
    pending_constructors: VecDeque<constructor_work::ConstructorWork>,
    emitted_functions: Vec<concrete::FunctionId>,
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
    imported_dependency_callables: Arena<concrete::ImportedDependencyCallableUse>,
    imported_dependency_callable_map:
        HashMap<export::ImportedDependencyCallableUseId, concrete::ImportedDependencyCallableUseId>,
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

    fn new(source: &'a export::Module) -> Result<Self, export::PublicNominalShapeProjectionError> {
        let automatic = AutomaticNominalRoots::new(source)?;
        let object_by_backing_class = source
            .objects
            .iter()
            .map(|(object, declaration)| (declaration.backing_class, object))
            .collect();
        let mut imported_dependency_callables = Arena::new();
        let imported_dependency_callable_map = source
            .imported_dependency_callables
            .iter()
            .map(|(source_id, source)| {
                let target = imported_dependency_callables.alloc(
                    concrete::ImportedDependencyCallableUse::from_export(*source),
                );
                (source_id, target)
            })
            .collect();
        Ok(Self {
            source,
            automatic,
            core: &source.core_protocols,
            types: Arena::new(),
            type_by_kind: HashMap::new(),
            function_types: Arena::new(),
            function_type_by_signature: HashMap::new(),
            structs: Arena::new(),
            struct_by_key: HashMap::new(),
            imported_structs: HashMap::new(),
            imported_enums: HashMap::new(),
            imported_classes: HashMap::new(),
            imported_interfaces: HashMap::new(),
            imported_interface_families: HashMap::new(),
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
            initialization_map: HashMap::new(),
            initialization_requests: Vec::new(),
            pending_initializations: VecDeque::new(),
            initialization_function_units: source
                .initialization_units
                .iter()
                .flat_map(|(id, unit)| [(unit.initializer, id), (unit.ensure, id)])
                .collect(),
            initialization_failure_roots: Arena::new(),
            objects: Arena::new(),
            object_types: Arena::new(),
            object_type_map: HashMap::new(),
            singleton_value_map: HashMap::new(),
            singleton_root_map: HashMap::new(),
            companion_relations: Arena::new(),
            singleton_values: Arena::new(),
            singleton_published_roots: Arena::new(),
            function_slots: Vec::new(),
            function_keys: Vec::new(),
            function_by_key: HashMap::new(),
            derived_bodies: HashMap::new(),
            structural_derived_functions: HashMap::new(),
            pending_functions: VecDeque::new(),
            pending_constructors: VecDeque::new(),
            emitted_functions: Vec::new(),
            lambdas: Arena::new(),
            lambda_by_key: HashMap::new(),
            anonymous_functions: Arena::new(),
            anonymous_by_key: HashMap::new(),
            local_functions: Arena::new(),
            local_by_key: HashMap::new(),
            callable_reference_slots: Vec::new(),
            reference_by_key: HashMap::new(),
            imported_dependency_callables,
            imported_dependency_callable_map,
            function_coercions: Arena::new(),
            coercion_by_key: HashMap::new(),
            foreign_callback_slots: Vec::new(),
            foreign_callback_by_key: HashMap::new(),
            next_loop_identity: 0,
        })
    }

    fn run(self) -> concrete::Module {
        self.run_with(|_| ()).0
    }

    fn run_with_entry(self, entry: export::FunctionId) -> (concrete::Module, concrete::FunctionId) {
        self.run_with(|concretizer| {
            concretizer.intern_type(concrete::TypeKind::Any, false);
            concretizer.function_by_key[&FunctionKey::Free {
                source: entry,
                arguments: Vec::new(),
            }]
        })
    }

    fn run_with<Extra>(
        mut self,
        finish: impl FnOnce(&mut Self) -> Extra,
    ) -> (concrete::Module, Extra) {
        let unit = self.lower_type(self.source.unit, &[]);
        match self.core {
            export::CoreProtocols::Defined(protocols) => {
                for kind in export::IntegerKind::ALL {
                    let owner = protocols.fundamental_types.integers.owner(kind);
                    let source_type = self.source.struct_applications
                        [self.source.structs[owner].self_application]
                        .canonical_type;
                    self.lower_type(source_type, &[]);
                }
            }
            export::CoreProtocols::Imported(_) => {
                let integer_types = self
                    .source
                    .types
                    .iter()
                    .filter_map(|(id, ty)| matches!(ty, export::Type::Integer(_)).then_some(id))
                    .collect::<Vec<_>>();
                for ty in integer_types {
                    self.lower_type(ty, &[]);
                }
            }
        }
        let boolean = self.lower_type(self.source.boolean, &[]);
        let string = self.lower_type(self.source.string, &[]);

        self.lower_extern_functions();
        self.lower_globals();

        for (id, declaration) in self.source.structs.iter() {
            if declaration.type_params.is_empty()
                && self.automatic_nominal(&self.source.nominal_identities[id])
            {
                self.lower_struct_application(declaration.self_application, &[]);
            }
        }
        for (id, declaration) in self.source.enums.iter() {
            if declaration.type_params.is_empty()
                && self.automatic_nominal(&self.source.nominal_identities[id])
            {
                self.ensure_enum(id, Vec::new());
            }
        }
        for (id, declaration) in self.source.interfaces.iter() {
            if declaration.type_params.is_empty()
                && self.automatic_nominal(&self.source.nominal_identities[id])
            {
                self.ensure_interface(id, Vec::new());
            }
        }
        for (id, declaration) in self.source.classes.iter() {
            if declaration.type_params.is_empty() && self.automatic_class(id) {
                self.lower_class_application(declaration.self_application, &[]);
            }
        }
        let lexical_functions = self
            .source
            .local_functions
            .iter()
            .map(|(_, local)| local.function)
            .chain(
                self.source
                    .lambdas
                    .iter()
                    .map(|(_, lambda)| lambda.function),
            )
            .chain(
                self.source
                    .anonymous_functions
                    .iter()
                    .map(|(_, anonymous)| anonymous.function),
            )
            .collect::<std::collections::HashSet<_>>();
        for (id, function) in self.source.functions.iter() {
            if !lexical_functions.contains(&id)
                && function.method.is_none()
                && function.type_param_count() == 0
                && self.is_emittable_source_function(id)
                && self.initialization_helper_is_required(id)
            {
                self.request_function(id, Vec::new());
            }
        }
        self.drain_pending_callables();

        let core_protocols = match self.core {
            export::CoreProtocols::Defined(protocols) => {
                self.lower_defined_core_protocols(protocols)
            }
            export::CoreProtocols::Imported(protocols) => {
                concrete::ConcreteCoreProtocols::Imported(protocols.clone())
            }
        };
        self.finish_initialization_units();
        if !self.initialization_units.is_empty()
            || self.source.cone
                == scoop_identity::CoreBuiltinNominal::Any
                    .declaration_key()
                    .origin()
        {
            self.intern_type(concrete::TypeKind::Any, false);
        }
        let extra = finish(&mut self);
        let core_types = match &core_protocols {
            concrete::ConcreteCoreProtocols::Defined(protocols) => {
                concrete::ConcreteCoreTypeIdentityAuthority::Defined(&protocols.fundamental_types)
            }
            concrete::ConcreteCoreProtocols::Imported(protocols) => {
                concrete::ConcreteCoreTypeIdentityAuthority::Imported(protocols.fundamental_types())
            }
        };
        let exact_type_identities =
            concrete::ExactTypeIdentities::from_types(concrete::ExactTypeIdentityInputs {
                types: &self.types,
                function_types: &self.function_types,
                structs: &self.structs,
                enums: &self.enums,
                classes: &self.classes,
                interfaces: &self.interfaces,
                objects: &self.objects,
                core_types,
            })
            .expect("validated concretization produces a total exact-type identity relation");
        let dispatch_slot_identities = self.build_dispatch_slot_identities();
        let identities = self.build_callable_identities(&exact_type_identities);
        let callable_references = finish_callable_references(
            self.callable_reference_slots,
            identities.callable_reference_identities,
        );
        let functions =
            finish_function_slots(self.function_slots, identities.function_materializations);
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
                source_files: &self.source.source_files,
                source_contexts: &self.source.source_context_identities,
                callable_applications: &identities.callable_applications,
                functions: &functions,
                lambdas: &self.lambdas,
                anonymous_functions: &self.anonymous_functions,
                default_local_values: &identities.default_local_values,
                callable_references: &callable_references,
                class_constructors: &class_constructors,
                struct_constructors: &struct_constructors,
            })
            .expect("validated concretization produces a total local-value identity relation");

        let module = concrete::Module {
            cone: self.source.cone,
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
            imported_dependency_callables: self.imported_dependency_callables,
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
            core_protocols,
        };
        (module, extra)
    }

    fn drain_pending_callables(&mut self) {
        loop {
            if let Some(unit) = self.pending_initializations.pop_front() {
                self.require_initialization_dependencies(unit);
            } else if let Some((key, id)) = self.pending_functions.pop_front() {
                let function = self.lower_function(&key);
                let slot = id.into_raw().into_u32() as usize;
                assert!(self.function_slots[slot].replace(function).is_none());
            } else if let Some(constructor) = self.pending_constructors.pop_front() {
                self.lower_pending_constructor(constructor);
            } else {
                break;
            }
        }
    }
}

fn remap_idx<S, T>(id: la_arena::Idx<S>) -> la_arena::Idx<T> {
    la_arena::Idx::from_raw(id.into_raw())
}

fn finish_function_slots(
    slots: Vec<Option<PendingFunction>>,
    materializations: Vec<concrete::CallableMaterialization>,
) -> Arena<concrete::Function> {
    assert_eq!(slots.len(), materializations.len());
    let mut arena = Arena::new();
    for (index, (slot, materialization)) in slots.into_iter().zip(materializations).enumerate() {
        let pending = slot.unwrap_or_else(|| panic!("missing concrete function at index {index}"));
        let id = arena.alloc(pending.finish(materialization));
        assert_eq!(id.into_raw().into_u32() as usize, index);
    }
    arena
}
