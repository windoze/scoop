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
mod constructor_definitions;
mod constructor_slots;
mod constructor_work;
mod context;
mod coroutines;
mod enums;
mod equality;
mod functions;
mod globals;
mod initialization;
mod initializing_fields;
mod interfaces;
mod methods;
mod nominals;
mod objects;
mod output;
mod prepare;
mod protocols;
mod release;
mod requests;
mod run;
mod runtime_exceptions;
mod structs;
mod type_conditions;
mod types;
mod variants;

use automatic::AutomaticNominalRoots;
use callback_slots::{PendingForeignCallbackRegistration, finish_foreign_callback_slots};
use closures::{PendingCallableReference, finish_callable_references};
use constructor_slots::{
    PendingClassConstructor, PendingStructConstructor, finish_class_constructor_slots,
    finish_struct_constructor_slots,
};
use functions::PendingFunction;
use initialization::{InitializationKey, InitializationRequest};
use type_conditions::SourceSite;

fn remap_idx<S, T>(id: la_arena::Idx<S>) -> la_arena::Idx<T> {
    la_arena::Idx::from_raw(id.into_raw())
}

fn nominal_root_diagnostic(
    error: export::PublicNominalShapeProjectionError,
) -> Vec<scoop_ast::Diagnostic> {
    vec![scoop_ast::Diagnostic::without_span(
        scoop_ast::DiagnosticSeverity::Error,
        0,
        format!("failed to project automatic nominal roots: {error}"),
    )]
}

pub(crate) fn lower(
    module: &export::Module,
) -> Result<concrete::Module, Vec<scoop_ast::Diagnostic>> {
    let automatic = AutomaticNominalRoots::new(module).map_err(nominal_root_diagnostic)?;
    Concretizer::new(module, automatic).run()
}

pub(crate) use output::lower_output;

use requests::{FunctionKey, FunctionSource};

#[derive(Debug, Clone)]
enum ConcreteApplicationRepresentation {
    Declared,
    Intrinsic(concrete::IntrinsicTypeRepresentation),
}

struct Concretizer<'a> {
    source: &'a export::Module,
    automatic: AutomaticNominalRoots,
    core: &'a export::CoreProtocols,
    coroutine_results: std::collections::BTreeSet<concrete::TypeId>,
    shared_types: std::collections::BTreeSet<concrete::TypeId>,
    types: Arena<concrete::Type>,
    type_by_kind: HashMap<concrete::TypeKind, concrete::TypeId>,
    function_types: Arena<concrete::FunctionType>,
    function_type_by_signature:
        HashMap<(bool, Vec<concrete::TypeId>, concrete::TypeId), concrete::FunctionTypeId>,
    structs: Arena<concrete::StructDef>,
    struct_by_key: HashMap<(export::SourceNominalId, Vec<concrete::TypeId>), concrete::StructId>,
    struct_type: HashMap<concrete::StructId, concrete::TypeId>,
    struct_source: HashMap<concrete::StructId, export::StructId>,
    enums: Arena<concrete::EnumDef>,
    enum_by_key: HashMap<(export::SourceNominalId, Vec<concrete::TypeId>), concrete::EnumId>,
    enum_type: HashMap<concrete::EnumId, concrete::TypeId>,
    enum_source: HashMap<concrete::EnumId, export::EnumId>,
    interfaces: Arena<concrete::InterfaceDef>,
    interface_by_key:
        HashMap<(export::SourceNominalId, Vec<concrete::TypeId>), concrete::InterfaceId>,
    interface_families: HashMap<export::SourceNominalId, concrete::InterfaceFamilyId>,
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
    release_hook_slots: Vec<Option<release::PendingReleaseHook>>,
    class_by_key: HashMap<(export::SourceNominalId, Vec<concrete::TypeId>), concrete::ClassId>,
    class_type: HashMap<concrete::ClassId, concrete::TypeId>,
    class_source: HashMap<concrete::ClassId, export::ClassId>,
    object_by_backing_class: HashMap<export::ClassId, export::ObjectId>,
    class_constructor_slots: Vec<Option<PendingClassConstructor>>,
    class_constructor_definitions: Vec<(export::ClassConstructorDefinition, concrete::ClassId)>,
    class_constructor_by_key: HashMap<
        (export::DefaultClassConstructorIdV1, concrete::ClassId),
        concrete::ClassConstructorId,
    >,
    struct_constructor_slots: Vec<Option<PendingStructConstructor>>,
    struct_constructor_definitions: Vec<(export::StructConstructorDefinition, concrete::StructId)>,
    struct_constructor_by_key: HashMap<
        (scoop_identity::PersistentConstructorId, concrete::StructId),
        concrete::StructConstructorId,
    >,
    extern_functions: Arena<concrete::ExternFunction>,
    extern_map: HashMap<export::ExternFunctionId, concrete::ExternFunctionId>,
    globals: Arena<concrete::Global>,
    global_map: HashMap<export::GlobalId, concrete::GlobalId>,
    generic_delegate_specializations: Arena<concrete::GenericDelegateStorageSpecialization>,
    generic_delegate_by_key: HashMap<
        (
            scoop_identity::PersistentExtensionPropertyId,
            Vec<concrete::TypeId>,
        ),
        concrete::GenericDelegateStorageSpecializationId,
    >,
    initialization_units: Arena<concrete::InitializationUnit>,
    initialization_map: HashMap<InitializationKey, concrete::InitializationUnitId>,
    initialization_requests: Vec<InitializationRequest>,
    pending_initializations: VecDeque<InitializationKey>,
    initialization_function_units: HashMap<export::FunctionId, export::InitializationUnitId>,
    initialization_failure_roots: Arena<concrete::InitializationFailureRoot>,
    objects: Arena<concrete::ObjectDecl>,
    object_types: Arena<concrete::ObjectType>,
    object_type_map: HashMap<concrete::ClassId, concrete::ObjectTypeId>,
    singleton_value_map: HashMap<concrete::ObjectTypeId, concrete::SingletonValueId>,
    singleton_root_map: HashMap<concrete::SingletonValueId, concrete::SingletonPublishedRootId>,
    companion_relations: Arena<concrete::CompanionRelation>,
    singleton_values: Arena<concrete::SingletonValue>,
    singleton_published_roots: Arena<concrete::SingletonPublishedRoot>,
    function_slots: Vec<Option<PendingFunction>>,
    function_keys: Vec<FunctionKey>,
    function_sources: Vec<FunctionSource>,
    function_by_key: HashMap<FunctionKey, concrete::FunctionId>,
    /// Concrete ordinary bodies supplied by typed derived-equality
    /// applications before their function key enters the emission queue.
    derived_bodies: HashMap<FunctionKey, (concrete::Body, Vec<concrete::LocalId>)>,
    derived_functions: HashMap<concrete::TypeId, concrete::FunctionId>,
    type_use_site: Option<SourceSite>,
    evaluation_context: Option<export::SourceContextId>,
    instantiation_site: Option<SourceSite>,
    type_condition_errors: Vec<scoop_ast::Diagnostic>,
    checked_context_requirements:
        std::collections::HashSet<(export::ContextRequirementOwner, Vec<concrete::TypeId>)>,
    pending_functions: VecDeque<(FunctionKey, concrete::FunctionId, Option<SourceSite>)>,
    pending_constructors: VecDeque<(constructor_work::ConstructorWork, Option<SourceSite>)>,
    emitted_functions: Vec<concrete::FunctionId>,
    lambdas: Arena<concrete::Lambda>,
    anonymous_functions: Arena<concrete::AnonymousFunction>,
    local_functions: Arena<concrete::LocalFunction>,
    local_by_function: HashMap<concrete::FunctionId, concrete::LocalFunctionId>,
    callable_reference_slots: Vec<PendingCallableReference>,
    imported_derived_equalities: Arena<concrete::ImportedDerivedEqualityUse>,
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

    fn new(source: &'a export::Module, automatic: AutomaticNominalRoots) -> Self {
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
        let interface_families = source
            .interfaces
            .iter()
            .map(|(id, _)| {
                (
                    source.nominal_identities[id].declaration_id(),
                    concrete::InterfaceFamilyId::from_raw(id.into_raw().into_u32()),
                )
            })
            .collect();
        Self {
            source,
            automatic,
            core: &source.core_protocols,
            coroutine_results: std::collections::BTreeSet::new(),
            shared_types: std::collections::BTreeSet::new(),
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
            interface_families,
            interface_type: HashMap::new(),
            interface_slot_by_source: HashMap::new(),
            virtual_method_by_source: HashMap::new(),
            classes: Arena::new(),
            release_hook_slots: Vec::new(),
            class_by_key: HashMap::new(),
            class_type: HashMap::new(),
            class_source: HashMap::new(),
            object_by_backing_class,
            class_constructor_slots: Vec::new(),
            class_constructor_definitions: Vec::new(),
            class_constructor_by_key: HashMap::new(),
            struct_constructor_slots: Vec::new(),
            struct_constructor_definitions: Vec::new(),
            struct_constructor_by_key: HashMap::new(),
            extern_functions: Arena::new(),
            extern_map: HashMap::new(),
            globals: Arena::new(),
            global_map: HashMap::new(),
            generic_delegate_specializations: Arena::new(),
            generic_delegate_by_key: HashMap::new(),
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
            function_sources: Vec::new(),
            function_by_key: HashMap::new(),
            derived_bodies: HashMap::new(),
            derived_functions: HashMap::new(),
            type_use_site: None,
            evaluation_context: None,
            instantiation_site: None,
            type_condition_errors: Vec::new(),
            checked_context_requirements: Default::default(),
            pending_functions: VecDeque::new(),
            pending_constructors: VecDeque::new(),
            emitted_functions: Vec::new(),
            lambdas: Arena::new(),
            anonymous_functions: Arena::new(),
            local_functions: Arena::new(),
            local_by_function: HashMap::new(),
            callable_reference_slots: Vec::new(),
            imported_derived_equalities: Arena::new(),
            imported_dependency_callables,
            imported_dependency_callable_map,
            function_coercions: Arena::new(),
            coercion_by_key: HashMap::new(),
            foreign_callback_slots: Vec::new(),
            foreign_callback_by_key: HashMap::new(),
            next_loop_identity: 0,
        }
    }
}
