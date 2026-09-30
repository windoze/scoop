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
mod enums;
mod functions;
mod imported_constructors;
mod imported_methods;
mod imported_nominals;
mod initialization;
mod initializing_fields;
mod interfaces;
mod nominals;
mod objects;
mod protocols;
mod requests;
mod run;
mod runtime_exceptions;
mod types;
mod variants;

use automatic::AutomaticNominalRoots;
use callback_slots::{PendingForeignCallbackRegistration, finish_foreign_callback_slots};
use closures::{CallableReferenceSource, PendingCallableReference, finish_callable_references};
use constructor_slots::{
    PendingClassConstructor, PendingStructConstructor, finish_class_constructor_slots,
    finish_struct_constructor_slots,
};
use functions::PendingFunction;
use initialization::{InitializationKey, InitializationRequest};

fn remap_idx<S, T>(id: la_arena::Idx<S>) -> la_arena::Idx<T> {
    la_arena::Idx::from_raw(id.into_raw())
}

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
    types: Arena<concrete::Type>,
    type_by_kind: HashMap<concrete::TypeKind, concrete::TypeId>,
    function_types: Arena<concrete::FunctionType>,
    function_type_by_signature:
        HashMap<(bool, Vec<concrete::TypeId>, concrete::TypeId), concrete::FunctionTypeId>,
    structs: Arena<concrete::StructDef>,
    struct_by_key: HashMap<(export::StructId, Vec<concrete::TypeId>), concrete::StructId>,
    imported_structs: HashMap<(export::SourceNominalId, Vec<concrete::TypeId>), concrete::StructId>,
    imported_classes: HashMap<(export::SourceNominalId, Vec<concrete::TypeId>), concrete::ClassId>,
    imported_interface_families: HashMap<export::SourceNominalId, concrete::InterfaceFamilyId>,
    imported_interfaces:
        HashMap<(export::SourceNominalId, Vec<concrete::TypeId>), concrete::InterfaceId>,
    struct_type: HashMap<concrete::StructId, concrete::TypeId>,
    struct_source: HashMap<concrete::StructId, export::StructId>,
    enums: Arena<concrete::EnumDef>,
    enum_by_key: HashMap<(export::SourceNominalId, Vec<concrete::TypeId>), concrete::EnumId>,
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
    object_type_map: HashMap<export::ObjectTypeId, concrete::ObjectTypeId>,
    singleton_value_map: HashMap<export::SingletonValueId, concrete::SingletonValueId>,
    singleton_root_map:
        HashMap<export::SingletonPublishedRootId, concrete::SingletonPublishedRootId>,
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
}
