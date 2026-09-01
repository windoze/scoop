//! Construction of the local, fully concrete HIR graph.
//!
//! Export-side HIR is the checked template/declaration graph.  This pass owns
//! the fixed-point instantiation closure and emits a distinct id domain for
//! MIR.  MIR never receives the export graph.

use std::collections::{HashMap, HashSet, VecDeque};

use la_arena::Arena;
use scoop_hir as export;
use scoop_hir::concrete;

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
    function_type_by_value: HashMap<concrete::FunctionType, concrete::FunctionTypeId>,
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
    classes: Arena<concrete::ClassDef>,
    class_by_key: HashMap<(export::ClassId, Vec<concrete::TypeId>), concrete::ClassId>,
    class_type: HashMap<concrete::ClassId, concrete::TypeId>,
    class_source: HashMap<concrete::ClassId, export::ClassId>,
    extern_functions: Arena<concrete::ExternFunction>,
    extern_map: HashMap<export::ExternFunctionId, concrete::ExternFunctionId>,
    globals: Arena<concrete::Global>,
    global_map: HashMap<export::GlobalId, concrete::GlobalId>,
    function_slots: Vec<Option<concrete::Function>>,
    function_by_key: HashMap<FunctionKey, concrete::FunctionId>,
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
            function_type_by_value: HashMap::new(),
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
            classes: Arena::new(),
            class_by_key: HashMap::new(),
            class_type: HashMap::new(),
            class_source: HashMap::new(),
            extern_functions: Arena::new(),
            extern_map: HashMap::new(),
            globals: Arena::new(),
            global_map: HashMap::new(),
            function_slots: Vec::new(),
            function_by_key: HashMap::new(),
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

        let intrinsic_type_core = concrete::IntrinsicTypeCore {
            int: self.struct_by_key[&(self.source.intrinsic_type_core.int, Vec::new())],
            uint: self.struct_by_key[&(self.source.intrinsic_type_core.uint, Vec::new())],
            boolean: self.struct_by_key[&(self.source.intrinsic_type_core.boolean, Vec::new())],
            string: self.class_by_key[&(self.source.intrinsic_type_core.string, Vec::new())],
        };

        let functions = arena_from_complete_slots(self.function_slots, "concrete function");
        let entry = self.function_by_key[&FunctionKey::Free {
            source: self.source.entry,
            arguments: Vec::new(),
        }];
        let throwable = self.class_by_key[&(self.source.coroutine_core.throwable, Vec::new())];
        let illegal_state_exception = self.class_by_key[&(
            self.source.coroutine_core.illegal_state_exception,
            Vec::new(),
        )];
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
            structs: self.structs,
            enums: self.enums,
            classes: self.classes,
            interfaces: self.interfaces,
            top_level: self.emitted_functions,
            unit,
            int,
            boolean,
            string,
            option_variants: (option_variant("Some"), option_variant("None")),
            exception_core: concrete::ExceptionCore {
                throwable,
                illegal_state_exception,
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

    fn ensure_class(
        &mut self,
        source_id: export::ClassId,
        arguments: Vec<concrete::TypeId>,
        application: ConcreteApplicationRepresentation,
    ) -> concrete::ClassId {
        let key = (source_id, arguments.clone());
        if let Some(&id) = self.class_by_key.get(&key) {
            return id;
        }
        let source = self.source.classes[source_id].clone();
        assert_eq!(source.type_params.len(), arguments.len());
        let representation = match (&source.representation, application) {
            (
                export::ClassRepresentation::Declared(_),
                ConcreteApplicationRepresentation::Declared,
            ) => concrete::ClassRepresentation::Declared {
                constructor: Vec::new(),
                base_class: None,
            },
            (
                export::ClassRepresentation::Intrinsic(declaration),
                ConcreteApplicationRepresentation::Intrinsic(application),
            ) => concrete::ClassRepresentation::Intrinsic {
                declaration: *declaration,
                application,
            },
            _ => unreachable!("ExportHir declaration and application representations agree"),
        };
        let id = self.classes.alloc(concrete::ClassDef {
            modifier: source.modifier,
            name: self.instance_name(&source.name, &arguments),
            type_arguments: arguments.clone(),
            representation,
            interfaces: Vec::new(),
            methods: Vec::new(),
            span: source.span,
        });
        self.class_by_key.insert(key, id);
        self.class_source.insert(id, source_id);
        let ty = match self.classes[id].representation {
            concrete::ClassRepresentation::Intrinsic {
                application: concrete::IntrinsicTypeRepresentation::String,
                ..
            } => self.intern_type(concrete::TypeKind::String, false),
            concrete::ClassRepresentation::Declared { .. }
            | concrete::ClassRepresentation::Intrinsic {
                application:
                    concrete::IntrinsicTypeRepresentation::Array { .. }
                    | concrete::IntrinsicTypeRepresentation::MutableArray { .. },
                ..
            } => self.intern_type(concrete::TypeKind::Class(id), false),
            concrete::ClassRepresentation::Intrinsic { .. } => {
                unreachable!("the registry fixes intrinsic declaration targets")
            }
        };
        self.class_type.insert(id, ty);

        let constructor = source
            .semantic_constructor()
            .iter()
            .map(|field| concrete::ConstructorField {
                parameter: concrete::ConstructorParamId::from_raw(field.parameter.into_raw()),
                name: field.name.clone(),
                ty: self.lower_type(field.ty, &arguments),
                mutable: field.mutable,
            })
            .collect();
        let interfaces = source
            .interface_implementations
            .iter()
            .map(|implementation| {
                let interface =
                    self.lower_interface_application(implementation.interface, &arguments);
                self.interface_type[&interface]
            })
            .collect();
        let base_class = source.base_class.map(|(base, args)| {
            let base = self.lower_type(base, &arguments);
            let concrete::TypeKind::Class(base) = self.types[base].kind else {
                unreachable!("class bases concretize to class identities")
            };
            let args = args
                .iter()
                .map(|argument| self.lower_expr(argument, &arguments, &[]))
                .collect();
            (base, args)
        });
        self.classes[id].interfaces = interfaces;
        match &mut self.classes[id].representation {
            concrete::ClassRepresentation::Declared {
                constructor: concrete_constructor,
                base_class: concrete_base,
            } => {
                *concrete_constructor = constructor;
                *concrete_base = base_class;
            }
            concrete::ClassRepresentation::Intrinsic { .. } => {
                debug_assert!(constructor.is_empty() && base_class.is_none());
            }
        }
        let methods =
            self.request_concrete_methods(&source.methods, concrete::MethodOwner::Class(id));
        self.classes[id].methods = methods;
        id
    }

    fn request_concrete_methods(
        &mut self,
        source_methods: &[export::FunctionId],
        owner: concrete::MethodOwner,
    ) -> Vec<concrete::FunctionId> {
        source_methods
            .iter()
            .copied()
            .filter_map(|method| {
                let function = &self.source.functions[method];
                match function.genericity {
                    export::FunctionGenericity::Plain
                    | export::FunctionGenericity::OwnerParameterizedMethod { .. } => {
                        Some(self.request_method(method, owner, MethodRequest::Plain))
                    }
                    export::FunctionGenericity::GenericMethod { .. } => None,
                    export::FunctionGenericity::Generic { .. } => {
                        unreachable!("nominal method lists do not contain generic functions")
                    }
                }
            })
            .collect()
    }

    fn lower_extern_functions(&mut self) {
        for (source_id, source) in self.source.extern_functions.iter() {
            let params = source
                .params
                .iter()
                .map(|ty| self.lower_type(*ty, &[]))
                .collect();
            let return_type = self.lower_type(source.return_type, &[]);
            let id = self.extern_functions.alloc(concrete::ExternFunction {
                source_name: source.source_name.clone(),
                native_symbol: source.native_symbol.clone(),
                library: source.library.clone(),
                abi: source.abi,
                calling_convention: source.calling_convention,
                gc_effect: source.gc_effect,
                safety: source.safety,
                params,
                return_type,
            });
            self.extern_map.insert(source_id, id);
        }
    }

    fn lower_globals(&mut self) {
        // Allocate ids first because expressions in function bodies may refer
        // to any global regardless of declaration order.
        for (source_id, source) in self.source.globals.iter() {
            let ty = self.lower_type(source.ty, &[]);
            let storage = self.lower_global_storage(&source.storage);
            let id = self.globals.alloc(concrete::Global {
                name: source.name.clone(),
                ty,
                mutable: source.mutable,
                storage,
                span: source.span,
            });
            self.global_map.insert(source_id, id);
        }
    }

    fn lower_global_storage(&mut self, storage: &export::GlobalStorage) -> concrete::GlobalStorage {
        match storage {
            export::GlobalStorage::Local {
                thread_local,
                initializer,
            } => concrete::GlobalStorage::Local {
                thread_local: *thread_local,
                initializer: self.lower_constant(initializer),
            },
            export::GlobalStorage::Extern {
                library,
                native_symbol,
                thread_local,
            } => concrete::GlobalStorage::Extern {
                library: library.clone(),
                native_symbol: native_symbol.clone(),
                thread_local: *thread_local,
            },
        }
    }

    fn lower_constant(&mut self, value: &export::ConstantValue) -> concrete::ConstantValue {
        match value {
            export::ConstantValue::Int(value) => concrete::ConstantValue::Int(*value),
            export::ConstantValue::Bool(value) => concrete::ConstantValue::Bool(*value),
            export::ConstantValue::NullPtr => concrete::ConstantValue::NullPtr,
            export::ConstantValue::NullFunPtr => concrete::ConstantValue::NullFunPtr,
            export::ConstantValue::Struct {
                application,
                fields,
            } => {
                let struct_id = self.lower_struct_application(*application, &[]);
                concrete::ConstantValue::Struct {
                    struct_id,
                    fields: fields
                        .iter()
                        .map(|field| self.lower_constant(field))
                        .collect(),
                }
            }
        }
    }

    fn lower_struct_application(
        &mut self,
        source: export::StructApplicationId,
        substitution: &[concrete::TypeId],
    ) -> concrete::StructId {
        let application = self.source.struct_applications[source].clone();
        let arguments = application
            .arguments
            .iter()
            .map(|argument| self.lower_type(*argument, substitution))
            .collect();
        let representation = match application.representation {
            export::StructApplicationRepresentation::Declared => {
                ConcreteApplicationRepresentation::Declared
            }
            export::StructApplicationRepresentation::Intrinsic(representation) => {
                ConcreteApplicationRepresentation::Intrinsic(
                    self.lower_intrinsic_type_representation(representation, substitution),
                )
            }
        };
        self.ensure_struct(application.template, arguments, representation)
    }

    fn lower_enum_application(
        &mut self,
        source: export::EnumApplicationId,
        substitution: &[concrete::TypeId],
    ) -> concrete::EnumId {
        let application = self.source.enum_applications[source].clone();
        let arguments = application
            .arguments
            .iter()
            .map(|argument| self.lower_type(*argument, substitution))
            .collect();
        self.ensure_enum(application.template, arguments)
    }

    fn lower_class_application(
        &mut self,
        source: export::ClassApplicationId,
        substitution: &[concrete::TypeId],
    ) -> concrete::ClassId {
        let application = self.source.class_applications[source].clone();
        let arguments = application
            .arguments
            .iter()
            .map(|argument| self.lower_type(*argument, substitution))
            .collect();
        let representation = match application.representation {
            export::ClassApplicationRepresentation::Declared => {
                ConcreteApplicationRepresentation::Declared
            }
            export::ClassApplicationRepresentation::Intrinsic(representation) => {
                ConcreteApplicationRepresentation::Intrinsic(
                    self.lower_intrinsic_type_representation(representation, substitution),
                )
            }
        };
        self.ensure_class(application.template, arguments, representation)
    }

    fn lower_intrinsic_type_representation(
        &mut self,
        representation: export::IntrinsicTypeRepresentation,
        substitution: &[concrete::TypeId],
    ) -> concrete::IntrinsicTypeRepresentation {
        match representation {
            export::IntrinsicTypeRepresentation::Int => concrete::IntrinsicTypeRepresentation::Int,
            export::IntrinsicTypeRepresentation::UInt => {
                concrete::IntrinsicTypeRepresentation::UInt
            }
            export::IntrinsicTypeRepresentation::Boolean => {
                concrete::IntrinsicTypeRepresentation::Boolean
            }
            export::IntrinsicTypeRepresentation::String => {
                concrete::IntrinsicTypeRepresentation::String
            }
            export::IntrinsicTypeRepresentation::Array { element } => {
                concrete::IntrinsicTypeRepresentation::Array {
                    element: self.lower_type(element, substitution),
                }
            }
            export::IntrinsicTypeRepresentation::MutableArray { element } => {
                concrete::IntrinsicTypeRepresentation::MutableArray {
                    element: self.lower_type(element, substitution),
                }
            }
        }
    }

    fn lower_interface_application(
        &mut self,
        source: export::InterfaceApplicationId,
        substitution: &[concrete::TypeId],
    ) -> concrete::InterfaceId {
        let application = self.source.interface_applications[source].clone();
        let arguments = application
            .arguments
            .iter()
            .map(|argument| self.lower_type(*argument, substitution))
            .collect();
        self.ensure_interface(application.template, arguments)
    }

    fn lower_type(
        &mut self,
        source: export::TypeId,
        substitution: &[concrete::TypeId],
    ) -> concrete::TypeId {
        match self.source.types[source].clone() {
            export::Type::Unit => self.intern_type(concrete::TypeKind::Unit, true),
            export::Type::Int => self.intern_type(concrete::TypeKind::Int, true),
            export::Type::UInt => self.intern_type(concrete::TypeKind::UInt, true),
            export::Type::Boolean => self.intern_type(concrete::TypeKind::Boolean, true),
            export::Type::String => self.intern_type(concrete::TypeKind::String, false),
            export::Type::Struct(application) => {
                let value = self.source.struct_applications[application].clone();
                if value.template == self.source.ffi_core.fun_ptr {
                    let [function] = value.arguments.as_slice() else {
                        panic!("validated deferred FunPtr has one argument")
                    };
                    let function = self.lower_type(*function, substitution);
                    let concrete::TypeKind::Function(function) = self.types[function].kind else {
                        panic!("deferred FunPtr resolves to a concrete function type")
                    };
                    return self.intern_type(concrete::TypeKind::FunPtr(function), true);
                }
                let id = self.lower_struct_application(application, substitution);
                self.struct_type[&id]
            }
            export::Type::Class(application) => {
                let id = self.lower_class_application(application, substitution);
                self.class_type[&id]
            }
            export::Type::Interface(application) => {
                let id = self.lower_interface_application(application, substitution);
                self.interface_type[&id]
            }
            export::Type::Any => self.intern_type(concrete::TypeKind::Any, false),
            export::Type::Tuple(elements) => {
                let elements: Vec<_> = elements
                    .iter()
                    .map(|element| self.lower_type(*element, substitution))
                    .collect();
                let gc_free = elements.iter().all(|element| self.types[*element].gc_free);
                self.intern_type(concrete::TypeKind::Tuple(elements), gc_free)
            }
            export::Type::Function(id) => {
                let id = self.lower_function_type(id, substitution);
                self.intern_type(concrete::TypeKind::Function(id), false)
            }
            export::Type::Ptr(pointee) => {
                let pointee = self.lower_type(pointee, substitution);
                self.intern_type(concrete::TypeKind::Ptr(pointee), true)
            }
            export::Type::FunPtr(id) => {
                let id = self.lower_function_type(id, substitution);
                self.intern_type(concrete::TypeKind::FunPtr(id), true)
            }
            export::Type::Enum(application) => {
                let id = self.lower_enum_application(application, substitution);
                self.enum_type[&id]
            }
            export::Type::Param(index) => substitution
                .get(index.into_raw() as usize)
                .copied()
                .expect("every local-concrete type parameter has a substitution"),
        }
    }

    fn intern_type(&mut self, kind: concrete::TypeKind, gc_free: bool) -> concrete::TypeId {
        if let Some(&id) = self.type_by_kind.get(&kind) {
            assert_eq!(
                self.types[id].gc_free, gc_free,
                "one concrete type identity has one GC-free classification"
            );
            return id;
        }
        let id = self.types.alloc(concrete::Type {
            kind: kind.clone(),
            gc_free,
        });
        self.type_by_kind.insert(kind, id);
        id
    }

    fn lower_function_type(
        &mut self,
        source: export::FunctionTypeId,
        substitution: &[concrete::TypeId],
    ) -> concrete::FunctionTypeId {
        let source = self.source.function_types[source].clone();
        let value = concrete::FunctionType {
            is_suspend: source.is_suspend,
            parameter_types: source
                .parameter_types
                .iter()
                .map(|ty| self.lower_type(*ty, substitution))
                .collect(),
            return_type: self.lower_type(source.return_type, substitution),
        };
        if let Some(&id) = self.function_type_by_value.get(&value) {
            return id;
        }
        let id = self.function_types.alloc(value.clone());
        self.function_type_by_value.insert(value, id);
        id
    }

    fn ensure_struct(
        &mut self,
        source_id: export::StructId,
        arguments: Vec<concrete::TypeId>,
        application: ConcreteApplicationRepresentation,
    ) -> concrete::StructId {
        let key = (source_id, arguments.clone());
        if let Some(&id) = self.struct_by_key.get(&key) {
            return id;
        }
        let source = self.source.structs[source_id].clone();
        assert_eq!(source.type_params.len(), arguments.len());
        let name = self.instance_name(&source.name, &arguments);
        let representation = match (&source.representation, application) {
            (
                export::StructRepresentation::Declared(_),
                ConcreteApplicationRepresentation::Declared,
            ) => concrete::StructRepresentation::Declared {
                attributes: source.attributes,
                fields: Vec::new(),
            },
            (
                export::StructRepresentation::Intrinsic(declaration),
                ConcreteApplicationRepresentation::Intrinsic(application),
            ) => concrete::StructRepresentation::Intrinsic {
                declaration: *declaration,
                application,
            },
            _ => unreachable!("ExportHir declaration and application representations agree"),
        };
        let id = self.structs.alloc(concrete::StructDef {
            name,
            type_arguments: arguments.clone(),
            gc_free: false,
            representation,
            interfaces: Vec::new(),
            methods: Vec::new(),
            span: source.span,
        });
        self.struct_by_key.insert(key, id);
        self.struct_source.insert(id, source_id);
        let ty = match self.structs[id].representation {
            concrete::StructRepresentation::Intrinsic {
                application: concrete::IntrinsicTypeRepresentation::Int,
                ..
            } => self.intern_type(concrete::TypeKind::Int, true),
            concrete::StructRepresentation::Intrinsic {
                application: concrete::IntrinsicTypeRepresentation::UInt,
                ..
            } => self.intern_type(concrete::TypeKind::UInt, true),
            concrete::StructRepresentation::Intrinsic {
                application: concrete::IntrinsicTypeRepresentation::Boolean,
                ..
            } => self.intern_type(concrete::TypeKind::Boolean, true),
            concrete::StructRepresentation::Declared { .. } => {
                self.intern_type(concrete::TypeKind::Struct(id), false)
            }
            concrete::StructRepresentation::Intrinsic { .. } => {
                unreachable!("the registry fixes intrinsic declaration targets")
            }
        };
        self.struct_type.insert(id, ty);
        let fields: Vec<_> = source
            .semantic_fields()
            .iter()
            .map(|field| concrete::Field {
                name: field.name.clone(),
                ty: self.lower_type(field.ty, &arguments),
            })
            .collect();
        let interfaces = source
            .interface_implementations
            .iter()
            .map(|implementation| {
                let interface =
                    self.lower_interface_application(implementation.interface, &arguments);
                self.interface_type[&interface]
            })
            .collect();
        let gc_free = fields.iter().all(|field| self.types[field.ty].gc_free);
        assert!(
            !source.attributes.no_gc || gc_free,
            "HIR diagnoses an invalid @NoGC struct specialization"
        );
        self.structs[id].interfaces = interfaces;
        self.structs[id].gc_free = gc_free;
        self.types[ty].gc_free = gc_free;
        match &mut self.structs[id].representation {
            concrete::StructRepresentation::Declared {
                fields: concrete_fields,
                ..
            } => *concrete_fields = fields,
            concrete::StructRepresentation::Intrinsic { .. } => {
                debug_assert!(fields.is_empty() && gc_free);
            }
        }
        let methods =
            self.request_concrete_methods(&source.methods, concrete::MethodOwner::Struct(id));
        self.structs[id].methods = methods;
        id
    }

    fn ensure_enum(
        &mut self,
        source_id: export::EnumId,
        arguments: Vec<concrete::TypeId>,
    ) -> concrete::EnumId {
        let key = (source_id, arguments.clone());
        if let Some(&id) = self.enum_by_key.get(&key) {
            return id;
        }
        let source = self.source.enums[source_id].clone();
        assert_eq!(source.type_params.len(), arguments.len());
        let name = self.instance_name(&source.name, &arguments);
        let id = self.enums.alloc(concrete::EnumDef {
            name,
            type_arguments: arguments.clone(),
            gc_free: false,
            variants: Vec::new(),
            option_variants: None,
            interfaces: Vec::new(),
            methods: Vec::new(),
            span: source.span,
        });
        self.enum_by_key.insert(key, id);
        self.enum_source.insert(id, source_id);
        let ty = self.intern_type(concrete::TypeKind::Enum(id), false);
        self.enum_type.insert(id, ty);
        let variants: Vec<_> = source
            .variants
            .iter()
            .map(|variant| {
                let fields: Vec<_> = variant
                    .fields
                    .iter()
                    .map(|field| concrete::Field {
                        name: field.name.clone(),
                        ty: self.lower_type(field.ty, &arguments),
                    })
                    .collect();
                let gc_free = fields.iter().all(|field| self.types[field.ty].gc_free);
                concrete::Variant {
                    name: variant.name.clone(),
                    gc_free,
                    fields,
                    defaults: Vec::new(),
                }
            })
            .collect();
        let interfaces = source
            .interface_implementations
            .iter()
            .map(|implementation| {
                let interface =
                    self.lower_interface_application(implementation.interface, &arguments);
                self.interface_type[&interface]
            })
            .collect();
        let gc_free = variants.iter().all(|variant| variant.gc_free);
        assert!(
            !source.no_gc || gc_free,
            "HIR diagnoses an invalid @NoGC enum specialization"
        );
        let option_variants = (source_id == self.source.option_enum).then(|| {
            let some = source
                .variants
                .iter()
                .position(|variant| variant.name == "Some")
                .expect("validated Option has Some");
            let none = source
                .variants
                .iter()
                .position(|variant| variant.name == "None")
                .expect("validated Option has None");
            (
                concrete::VariantId::from_raw(some as u32),
                concrete::VariantId::from_raw(none as u32),
            )
        });
        self.enums[id].variants = variants;
        self.enums[id].interfaces = interfaces;
        self.enums[id].option_variants = option_variants;
        self.enums[id].gc_free = gc_free;
        self.types[ty].gc_free = gc_free;
        let methods =
            self.request_concrete_methods(&source.methods, concrete::MethodOwner::Enum(id));
        self.enums[id].methods = methods;
        id
    }

    fn ensure_interface(
        &mut self,
        source_id: export::InterfaceId,
        arguments: Vec<concrete::TypeId>,
    ) -> concrete::InterfaceId {
        let key = (source_id, arguments.clone());
        if let Some(&id) = self.interface_by_key.get(&key) {
            return id;
        }
        let source = self.source.interfaces[source_id].clone();
        assert_eq!(source.type_params.len(), arguments.len());
        let name = self.instance_name(&source.name, &arguments);
        let id = self.interfaces.alloc(concrete::InterfaceDef {
            name,
            family: concrete::InterfaceFamilyId::from_raw(source_id.into_raw().into_u32()),
            variances: source
                .type_params
                .iter()
                .map(|parameter| parameter.variance)
                .collect(),
            type_arguments: arguments.clone(),
            methods: Vec::new(),
            span: source.span,
        });
        self.interface_by_key.insert(key, id);
        let ty = self.intern_type(concrete::TypeKind::Interface(id), false);
        self.interface_type.insert(id, ty);
        let method_instances = self.interface_method_instances(source.self_application, &arguments);
        let methods = method_instances
            .iter()
            .map(|(method, method_arguments)| {
                let function =
                    &self.source.functions[self.source.interface_methods[*method].function];
                concrete::MethodSig {
                    name: function
                        .name
                        .rsplit('.')
                        .next()
                        .expect("interface methods are qualified")
                        .to_string(),
                    is_suspend: function.is_suspend,
                    attributes: function.attributes,
                    params: function
                        .params
                        .iter()
                        .skip(1)
                        .map(|param| concrete::Param {
                            name: param.name.clone(),
                            ty: self.lower_type(param.ty, method_arguments),
                            local: remap_idx(param.local),
                        })
                        .collect(),
                    return_ty: self.lower_type(function.return_ty, method_arguments),
                    span: function.span,
                }
            })
            .collect();
        self.interfaces[id].methods = methods;
        id
    }

    fn interface_method_instances(
        &mut self,
        application: export::InterfaceApplicationId,
        substitution: &[concrete::TypeId],
    ) -> Vec<(export::InterfaceMethodId, Vec<concrete::TypeId>)> {
        let mut result = Vec::new();
        let mut seen = Vec::new();
        self.collect_interface_method_instances(application, substitution, &mut seen, &mut result);
        result
    }

    fn collect_interface_method_instances(
        &mut self,
        application: export::InterfaceApplicationId,
        substitution: &[concrete::TypeId],
        seen: &mut Vec<(export::InterfaceId, Vec<concrete::TypeId>)>,
        out: &mut Vec<(export::InterfaceMethodId, Vec<concrete::TypeId>)>,
    ) {
        let application = self.source.interface_applications[application].clone();
        let arguments = application
            .arguments
            .iter()
            .map(|argument| self.lower_type(*argument, substitution))
            .collect::<Vec<_>>();
        let key = (application.template, arguments.clone());
        if seen.contains(&key) {
            return;
        }
        seen.push(key);
        let declaration = self.source.interfaces[application.template].clone();
        out.extend(
            declaration
                .methods
                .iter()
                .map(|&member| (member, arguments.clone())),
        );
        for parent in declaration.parents {
            self.collect_interface_method_instances(parent, &arguments, seen, out);
        }
    }

    fn instance_name(&self, base: &str, arguments: &[concrete::TypeId]) -> String {
        if arguments.is_empty() {
            base.to_string()
        } else {
            let arguments = arguments
                .iter()
                .map(|argument| self.encode_type(*argument))
                .collect::<Vec<_>>()
                .join("_");
            format!("{base}${arguments}")
        }
    }

    fn encode_type(&self, ty: concrete::TypeId) -> String {
        match &self.types[ty].kind {
            concrete::TypeKind::Unit => "U".to_string(),
            concrete::TypeKind::Int => "I".to_string(),
            concrete::TypeKind::UInt => "V".to_string(),
            concrete::TypeKind::Boolean => "B".to_string(),
            concrete::TypeKind::String => "S".to_string(),
            concrete::TypeKind::Struct(id) => self.structs[*id].name.clone(),
            concrete::TypeKind::Class(id) => match &self.classes[*id].representation {
                concrete::ClassRepresentation::Intrinsic {
                    application: concrete::IntrinsicTypeRepresentation::Array { element },
                    ..
                } => format!("A{}X", self.encode_type(*element)),
                concrete::ClassRepresentation::Intrinsic {
                    application: concrete::IntrinsicTypeRepresentation::MutableArray { element },
                    ..
                } => format!("M{}X", self.encode_type(*element)),
                concrete::ClassRepresentation::Declared { .. }
                | concrete::ClassRepresentation::Intrinsic {
                    application: concrete::IntrinsicTypeRepresentation::String,
                    ..
                } => self.classes[*id].name.clone(),
                concrete::ClassRepresentation::Intrinsic { .. } => {
                    unreachable!("the intrinsic registry fixes declaration targets")
                }
            },
            concrete::TypeKind::Interface(id) => self.interfaces[*id].name.clone(),
            concrete::TypeKind::Any => "Any".to_string(),
            concrete::TypeKind::Tuple(elements) => format!(
                "T{}X",
                elements
                    .iter()
                    .map(|element| self.encode_type(*element))
                    .collect::<Vec<_>>()
                    .join("_")
            ),
            concrete::TypeKind::Function(id) => {
                let function = &self.function_types[*id];
                let kind = if function.is_suspend { "S" } else { "F" };
                let parameters = function
                    .parameter_types
                    .iter()
                    .map(|ty| self.encode_type(*ty))
                    .collect::<Vec<_>>()
                    .join("_");
                format!(
                    "{kind}{parameters}R{}X",
                    self.encode_type(function.return_type)
                )
            }
            concrete::TypeKind::Ptr(pointee) => format!("P{}X", self.encode_type(*pointee)),
            concrete::TypeKind::FunPtr(id) => {
                let function = &self.function_types[*id];
                let parameters = function
                    .parameter_types
                    .iter()
                    .map(|ty| self.encode_type(*ty))
                    .collect::<Vec<_>>()
                    .join("_");
                format!("N{parameters}R{}X", self.encode_type(function.return_type))
            }
            concrete::TypeKind::Enum(id) => format!("E{}", self.enums[*id].name),
        }
    }

    fn is_emittable_source_function(&self, id: export::FunctionId) -> bool {
        let function = &self.source.functions[id];
        if !matches!(function.kind, export::FunctionKind::User(_)) {
            return false;
        }
        let Some(method) = function.method else {
            return true;
        };
        !matches!(
            self.source.types[method.owner],
            export::Type::Interface(..) | export::Type::Any
        )
    }

    fn request_function(
        &mut self,
        source: export::FunctionId,
        arguments: Vec<concrete::TypeId>,
    ) -> concrete::FunctionId {
        assert!(
            self.source.functions[source].method.is_none(),
            "method instances require an exact concrete owner"
        );
        let key = FunctionKey::Free { source, arguments };
        self.request_function_key(key)
    }

    fn request_method(
        &mut self,
        source: export::FunctionId,
        owner: concrete::MethodOwner,
        specialization: MethodRequest,
    ) -> concrete::FunctionId {
        assert!(
            self.source.functions[source].method.is_some(),
            "method requests name a method declaration"
        );
        self.request_function_key(FunctionKey::Method {
            source,
            owner,
            specialization,
        })
    }

    fn request_function_key(&mut self, key: FunctionKey) -> concrete::FunctionId {
        if let Some(&id) = self.function_by_key.get(&key) {
            return id;
        }
        let source = key.source();
        let arguments = self.function_key_arguments(&key);
        assert_eq!(
            self.source.functions[source].type_param_count(),
            arguments.len()
        );
        let raw = self.function_slots.len() as u32;
        self.function_slots.push(None);
        let id = concrete::FunctionId::from_raw(raw.into());
        self.function_by_key.insert(key.clone(), id);
        self.pending_functions.push_back((key, id));
        if self.is_emittable_source_function(source) {
            self.emitted_functions.push(id);
        }
        id
    }

    fn lower_function(&mut self, key: &FunctionKey) -> concrete::Function {
        let source_id = key.source();
        let source = self.source.functions[source_id].clone();
        let arguments = self.function_key_arguments(key);
        let (kind, local_map) = match &source.kind {
            export::FunctionKind::User(body) => {
                let (body, local_map) = self.lower_body(body, &arguments);
                (concrete::FunctionKind::User(body), local_map)
            }
            export::FunctionKind::Intrinsic(intrinsic) => {
                (concrete::FunctionKind::Intrinsic(*intrinsic), Vec::new())
            }
            export::FunctionKind::Extern(id) => (
                concrete::FunctionKind::Extern(self.extern_map[id]),
                Vec::new(),
            ),
        };
        let params = source
            .params
            .iter()
            .map(|param| concrete::Param {
                name: param.name.clone(),
                ty: self.lower_type(param.ty, &arguments),
                local: local_map
                    .get(param.local.into_raw().into_u32() as usize)
                    .copied()
                    .unwrap_or_else(|| remap_idx(param.local)),
            })
            .collect();
        let return_ty = self.lower_type(source.return_ty, &arguments);
        let method = source.method.map(|method| concrete::Method {
            owner: self.lower_type(method.owner, &arguments),
            modifier: method.modifier,
        });
        let origin = self.function_origin(key, &source);
        concrete::Function {
            name: source.name,
            origin,
            is_suspend: source.is_suspend,
            params,
            return_ty,
            attributes: source.attributes,
            kind,
            method,
            span: source.span,
        }
    }

    fn function_key_arguments(&self, key: &FunctionKey) -> Vec<concrete::TypeId> {
        match key {
            FunctionKey::Free { arguments, .. } => arguments.clone(),
            FunctionKey::Method {
                owner,
                specialization,
                ..
            } => {
                let mut arguments = self.concrete_method_owner_arguments(*owner).to_vec();
                if let MethodRequest::Generic {
                    method_arguments, ..
                } = specialization
                {
                    arguments.extend(method_arguments.iter().copied());
                }
                arguments
            }
        }
    }

    fn concrete_method_owner_arguments(&self, owner: concrete::MethodOwner) -> &[concrete::TypeId] {
        match owner {
            concrete::MethodOwner::Class(id) => &self.classes[id].type_arguments,
            concrete::MethodOwner::Struct(id) => &self.structs[id].type_arguments,
            concrete::MethodOwner::Enum(id) => &self.enums[id].type_arguments,
            concrete::MethodOwner::Interface(id) => &self.interfaces[id].type_arguments,
            concrete::MethodOwner::Any => &[],
        }
    }

    fn concrete_function_arguments(&self, function: &concrete::Function) -> Vec<concrete::TypeId> {
        match &function.origin {
            concrete::FunctionOrigin::Free(concrete::FreeFunctionOrigin::Plain) => Vec::new(),
            concrete::FunctionOrigin::Free(concrete::FreeFunctionOrigin::Generic {
                arguments,
                ..
            }) => arguments.clone(),
            concrete::FunctionOrigin::Method(origin) => {
                let mut arguments = self.concrete_method_owner_arguments(origin.owner).to_vec();
                if let concrete::MethodSpecialization::Generic {
                    method_arguments, ..
                } = &origin.specialization
                {
                    arguments.extend(method_arguments.iter().copied());
                }
                arguments
            }
        }
    }

    fn function_origin(
        &self,
        key: &FunctionKey,
        source: &export::Function,
    ) -> concrete::FunctionOrigin {
        match key {
            FunctionKey::Free {
                source: source_id,
                arguments,
            } => match source.genericity {
                export::FunctionGenericity::Plain => {
                    concrete::FunctionOrigin::Free(concrete::FreeFunctionOrigin::Plain)
                }
                export::FunctionGenericity::Generic { definition, .. } => {
                    concrete::FunctionOrigin::Free(concrete::FreeFunctionOrigin::Generic {
                        definition,
                        arguments: arguments.clone(),
                        symbol: self.instance_symbol(&source.name, source_id.into_raw().into_u32()),
                    })
                }
                export::FunctionGenericity::OwnerParameterizedMethod { .. }
                | export::FunctionGenericity::GenericMethod { .. } => {
                    unreachable!("free function keys cannot name methods")
                }
            },
            FunctionKey::Method {
                source: source_id,
                owner,
                specialization,
            } => {
                let specialization = match specialization {
                    MethodRequest::Plain => match source.genericity {
                        export::FunctionGenericity::Plain => concrete::MethodSpecialization::Plain,
                        export::FunctionGenericity::OwnerParameterizedMethod { .. } => {
                            concrete::MethodSpecialization::OwnerParameterized {
                                symbol: self
                                    .instance_symbol(&source.name, source_id.into_raw().into_u32()),
                            }
                        }
                        export::FunctionGenericity::Generic { .. }
                        | export::FunctionGenericity::GenericMethod { .. } => {
                            unreachable!("plain method requests match plain method declarations")
                        }
                    },
                    MethodRequest::Generic {
                        definition,
                        method_arguments,
                    } => concrete::MethodSpecialization::Generic {
                        definition: *definition,
                        method_arguments: method_arguments.clone(),
                        symbol: self.instance_symbol(&source.name, source_id.into_raw().into_u32()),
                    },
                };
                concrete::FunctionOrigin::Method(concrete::MethodOrigin {
                    owner: *owner,
                    specialization,
                })
            }
        }
    }

    fn instance_symbol(&self, name: &str, discriminator: u32) -> concrete::InstanceSymbol {
        if self.overloaded_generic_names.contains(name) {
            concrete::InstanceSymbol::Overloaded { discriminator }
        } else {
            concrete::InstanceSymbol::Unique
        }
    }

    fn lower_body(
        &mut self,
        source: &export::Body,
        substitution: &[concrete::TypeId],
    ) -> (concrete::Body, Vec<concrete::LocalId>) {
        let mut locals = Arena::new();
        let mut local_map = Vec::with_capacity(source.locals.len());
        for (source_id, source_local) in source.locals.iter() {
            let id = locals.alloc(concrete::Local {
                binding: concrete::BindingId::from_raw(source_local.binding.into_raw()),
                name: source_local.name.clone(),
                ty: self.lower_type(source_local.ty, substitution),
                mutable: source_local.mutable,
            });
            assert_eq!(id.into_raw(), source_id.into_raw());
            local_map.push(id);
        }
        let statements = source
            .statements
            .iter()
            .filter_map(|statement| self.lower_statement(statement, substitution, &local_map))
            .collect();
        (concrete::Body { locals, statements }, local_map)
    }

    fn lower_statement(
        &mut self,
        source: &export::Statement,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> Option<concrete::Statement> {
        let kind = match &source.kind {
            export::StatementKind::Expr(expr) => {
                concrete::StatementKind::Expr(self.lower_expr(expr, substitution, locals))
            }
            // This marker has no runtime semantics. Concrete local-function
            // entities are requested by direct calls/references instead.
            export::StatementKind::LocalFunction(_) => return None,
            export::StatementKind::Return { value } => concrete::StatementKind::Return {
                value: value
                    .as_ref()
                    .map(|value| self.lower_expr(value, substitution, locals)),
            },
            export::StatementKind::ValDecl { pattern, init } => {
                let init = self.lower_expr(init, substitution, locals);
                let pattern = self.lower_pattern(pattern, init.ty, substitution, locals);
                concrete::StatementKind::ValDecl { pattern, init }
            }
            export::StatementKind::Assign { target, value } => concrete::StatementKind::Assign {
                target: self.lower_assign_target(target, substitution, locals),
                value: self.lower_expr(value, substitution, locals),
            },
            export::StatementKind::If {
                cond,
                then_body,
                else_body,
            } => concrete::StatementKind::If {
                cond: self.lower_expr(cond, substitution, locals),
                then_body: self.lower_statements(then_body, substitution, locals),
                else_body: else_body
                    .as_ref()
                    .map(|body| self.lower_statements(body, substitution, locals)),
            },
            export::StatementKind::While { cond, body } => concrete::StatementKind::While {
                cond: self.lower_expr(cond, substitution, locals),
                body: self.lower_statements(body, substitution, locals),
            },
            export::StatementKind::When(when) => {
                concrete::StatementKind::When(self.lower_when(when, substitution, locals))
            }
            export::StatementKind::Try(try_) => concrete::StatementKind::Try(concrete::Try {
                body: self.lower_statements(&try_.body, substitution, locals),
                catches: try_
                    .catches
                    .iter()
                    .map(|catch| concrete::CatchClause {
                        local: self.lower_local(catch.local, locals),
                        ty: self.lower_type(catch.ty, substitution),
                        body: self.lower_statements(&catch.body, substitution, locals),
                        span: catch.span,
                    })
                    .collect(),
                finally_body: try_
                    .finally_body
                    .as_ref()
                    .map(|body| self.lower_statements(body, substitution, locals)),
            }),
            export::StatementKind::Throw(expr) => {
                concrete::StatementKind::Throw(self.lower_expr(expr, substitution, locals))
            }
        };
        Some(concrete::Statement {
            kind,
            span: source.span,
        })
    }

    fn lower_statements(
        &mut self,
        source: &[export::Statement],
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> Vec<concrete::Statement> {
        source
            .iter()
            .filter_map(|statement| self.lower_statement(statement, substitution, locals))
            .collect()
    }

    fn lower_assign_target(
        &mut self,
        source: &export::AssignTarget,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::AssignTarget {
        match source {
            export::AssignTarget::Local(local) => {
                concrete::AssignTarget::Local(self.lower_local(*local, locals))
            }
            export::AssignTarget::Global(global) => {
                concrete::AssignTarget::Global(self.global_map[global])
            }
            export::AssignTarget::Index { array, index } => concrete::AssignTarget::Index {
                array: self.lower_expr(array, substitution, locals),
                index: self.lower_expr(index, substitution, locals),
            },
            export::AssignTarget::Field { receiver, field } => {
                let receiver = self.lower_expr(receiver, substitution, locals);
                let field = self.lower_field_ref(*field, substitution);
                concrete::AssignTarget::Field {
                    receiver: Box::new(receiver),
                    field,
                }
            }
        }
    }

    fn lower_when(
        &mut self,
        source: &export::When,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::When {
        let subject = self.lower_expr(&source.subject, substitution, locals);
        let arms = source
            .arms
            .iter()
            .map(|arm| concrete::WhenArm {
                pattern: self.lower_pattern(&arm.pattern, subject.ty, substitution, locals),
                guard: arm
                    .guard
                    .as_ref()
                    .map(|guard| self.lower_expr(guard, substitution, locals)),
                body: self.lower_statements(&arm.body, substitution, locals),
                span: arm.span,
            })
            .collect();
        concrete::When {
            subject,
            arms,
            else_body: source
                .else_body
                .as_ref()
                .map(|body| self.lower_statements(body, substitution, locals)),
        }
    }

    fn lower_pattern(
        &mut self,
        source: &export::Pattern,
        expected: concrete::TypeId,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::Pattern {
        match source {
            export::Pattern::Binding { local } => concrete::Pattern::Binding {
                local: self.lower_local(*local, locals),
            },
            export::Pattern::Wildcard => concrete::Pattern::Wildcard,
            export::Pattern::Literal(expr) => {
                concrete::Pattern::Literal(self.lower_expr(expr, substitution, locals))
            }
            export::Pattern::Variant {
                application,
                variant,
                fields,
            } => {
                let concrete_enum = self.lower_enum_application(*application, substitution);
                assert_eq!(
                    self.types[expected].kind,
                    concrete::TypeKind::Enum(concrete_enum),
                    "the checked pattern application must match its subject"
                );
                let variant_id = concrete::VariantId::from_raw(*variant);
                let definition = self.enums[concrete_enum].variants[*variant as usize].clone();
                let fields = fields
                    .iter()
                    .map(|(field, pattern)| {
                        (
                            *field,
                            self.lower_pattern(
                                pattern,
                                definition.fields[*field as usize].ty,
                                substitution,
                                locals,
                            ),
                        )
                    })
                    .collect();
                concrete::Pattern::Variant {
                    enum_id: concrete_enum,
                    variant: variant_id,
                    fields,
                }
            }
            export::Pattern::Tuple(patterns) => {
                let concrete::TypeKind::Tuple(elements) = self.types[expected].kind.clone() else {
                    panic!("a resolved tuple pattern has a concrete tuple subject")
                };
                concrete::Pattern::Tuple(
                    patterns
                        .iter()
                        .zip(elements)
                        .map(|(pattern, expected)| {
                            self.lower_pattern(pattern, expected, substitution, locals)
                        })
                        .collect(),
                )
            }
            export::Pattern::Struct {
                application,
                fields,
            } => {
                let concrete_struct = self.lower_struct_application(*application, substitution);
                assert_eq!(
                    self.types[expected].kind,
                    concrete::TypeKind::Struct(concrete_struct),
                    "the checked pattern application must match its subject"
                );
                let definition = self.structs[concrete_struct].declared_fields().to_vec();
                concrete::Pattern::Struct {
                    struct_id: concrete_struct,
                    fields: fields
                        .iter()
                        .map(|(field, pattern)| {
                            (
                                *field,
                                self.lower_pattern(
                                    pattern,
                                    definition[*field as usize].ty,
                                    substitution,
                                    locals,
                                ),
                            )
                        })
                        .collect(),
                }
            }
        }
    }

    fn lower_expr(
        &mut self,
        source: &export::Expr,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::Expr {
        let ty = self.lower_type(source.ty, substitution);
        let kind = match &source.kind {
            export::ExprKind::StringLiteral(value) => {
                concrete::ExprKind::StringLiteral(value.clone())
            }
            export::ExprKind::IntLiteral(value) => concrete::ExprKind::IntLiteral(*value),
            export::ExprKind::BoolLiteral(value) => concrete::ExprKind::BoolLiteral(*value),
            export::ExprKind::UnitLiteral => concrete::ExprKind::UnitLiteral,
            export::ExprKind::TupleLiteral(elements) => concrete::ExprKind::TupleLiteral(
                elements
                    .iter()
                    .map(|element| self.lower_expr(element, substitution, locals))
                    .collect(),
            ),
            export::ExprKind::StructInit { application, args } => {
                let id = self.lower_struct_application(*application, substitution);
                concrete::ExprKind::StructInit {
                    struct_id: id,
                    args: args
                        .iter()
                        .map(|argument| self.lower_expr(argument, substitution, locals))
                        .collect(),
                }
            }
            export::ExprKind::ClassInit { application, args } => {
                let concrete_id = self.lower_class_application(*application, substitution);
                concrete::ExprKind::ClassInit {
                    class_id: concrete_id,
                    args: args
                        .iter()
                        .map(|argument| self.lower_expr(argument, substitution, locals))
                        .collect(),
                }
            }
            export::ExprKind::ConstructorParam(parameter) => concrete::ExprKind::ConstructorParam(
                concrete::ConstructorParamId::from_raw(parameter.into_raw()),
            ),
            export::ExprKind::VariantConstruct {
                application,
                variant,
                args,
            } => {
                let id = self.lower_enum_application(*application, substitution);
                concrete::ExprKind::VariantConstruct {
                    enum_id: id,
                    variant: concrete::VariantId::from_raw(*variant),
                    args: args
                        .iter()
                        .map(|argument| self.lower_expr(argument, substitution, locals))
                        .collect(),
                }
            }
            export::ExprKind::Local(local) => {
                concrete::ExprKind::Local(self.lower_local(*local, locals))
            }
            export::ExprKind::GlobalRead(global) => {
                concrete::ExprKind::GlobalRead(self.global_map[global])
            }
            export::ExprKind::Capture(binding) => {
                concrete::ExprKind::Capture(concrete::BindingId::from_raw(binding.into_raw()))
            }
            export::ExprKind::Lambda(id) => {
                concrete::ExprKind::Lambda(self.ensure_lambda(*id, substitution, locals))
            }
            export::ExprKind::AnonymousFunction(id) => concrete::ExprKind::AnonymousFunction(
                self.ensure_anonymous(*id, substitution, locals),
            ),
            export::ExprKind::CallableReference(id) => concrete::ExprKind::CallableReference(
                self.ensure_reference(*id, substitution, locals),
            ),
            export::ExprKind::FunctionCoercion {
                source,
                coercion,
                target_type,
            } => concrete::ExprKind::FunctionCoercion {
                source: Box::new(self.lower_expr(source, substitution, locals)),
                coercion: self.ensure_coercion(*coercion, substitution),
                target_type: self.lower_function_type(*target_type, substitution),
            },
            export::ExprKind::PtrFromUInt(value) => concrete::ExprKind::PtrFromUInt(Box::new(
                self.lower_expr(value, substitution, locals),
            )),
            export::ExprKind::PtrToUInt(value) => concrete::ExprKind::PtrToUInt(Box::new(
                self.lower_expr(value, substitution, locals),
            )),
            export::ExprKind::PtrCast(value) => {
                concrete::ExprKind::PtrCast(Box::new(self.lower_expr(value, substitution, locals)))
            }
            export::ExprKind::PtrLoad { pointer, offset } => concrete::ExprKind::PtrLoad {
                pointer: Box::new(self.lower_expr(pointer, substitution, locals)),
                offset: offset
                    .as_ref()
                    .map(|offset| Box::new(self.lower_expr(offset, substitution, locals))),
            },
            export::ExprKind::PtrStore {
                pointer,
                offset,
                value,
            } => concrete::ExprKind::PtrStore {
                pointer: Box::new(self.lower_expr(pointer, substitution, locals)),
                offset: offset
                    .as_ref()
                    .map(|offset| Box::new(self.lower_expr(offset, substitution, locals))),
                value: Box::new(self.lower_expr(value, substitution, locals)),
            },
            export::ExprKind::PtrOffset {
                pointer,
                offset,
                subtract,
            } => concrete::ExprKind::PtrOffset {
                pointer: Box::new(self.lower_expr(pointer, substitution, locals)),
                offset: Box::new(self.lower_expr(offset, substitution, locals)),
                subtract: *subtract,
            },
            export::ExprKind::AddressOf(place) => {
                concrete::ExprKind::AddressOf(self.lower_place(*place, locals))
            }
            export::ExprKind::SizeOf(size) => {
                concrete::ExprKind::SizeOf(self.lower_type(*size, substitution))
            }
            export::ExprKind::AlignOf(align) => {
                concrete::ExprKind::AlignOf(self.lower_type(*align, substitution))
            }
            export::ExprKind::FunPtrNull => concrete::ExprKind::FunPtrNull,
            export::ExprKind::FunctionAddress(function) => {
                concrete::ExprKind::FunctionAddress(self.request_function(*function, Vec::new()))
            }
            export::ExprKind::ForeignCallbackRegister {
                registration,
                closure,
            } => {
                let concrete::TypeKind::Struct(callback) = self.types[ty].kind else {
                    panic!("foreign callback registration has a concrete callback struct type")
                };
                let registration =
                    self.ensure_foreign_callback(*registration, substitution, callback);
                concrete::ExprKind::ForeignCallbackRegister {
                    registration,
                    closure: Box::new(self.lower_expr(closure, substitution, locals)),
                }
            }
            export::ExprKind::ForeignCallbackOperation {
                operation,
                callback,
            } => concrete::ExprKind::ForeignCallbackOperation {
                operation: match operation {
                    export::ForeignCallbackOperation::Retain => {
                        concrete::ForeignCallbackOperation::Retain
                    }
                    export::ForeignCallbackOperation::Release => {
                        concrete::ForeignCallbackOperation::Release
                    }
                    export::ForeignCallbackOperation::State => {
                        concrete::ForeignCallbackOperation::State
                    }
                    export::ForeignCallbackOperation::Failure => {
                        concrete::ForeignCallbackOperation::Failure
                    }
                },
                callback: Box::new(self.lower_expr(callback, substitution, locals)),
            },
            export::ExprKind::FieldAccess { receiver, field } => {
                let receiver = self.lower_expr(receiver, substitution, locals);
                let field = self.lower_field_ref(*field, substitution);
                concrete::ExprKind::FieldAccess {
                    receiver: Box::new(receiver),
                    field,
                }
            }
            export::ExprKind::MethodCall {
                receiver,
                callee,
                args,
            } => {
                let mut receiver = self.lower_expr(receiver, substitution, locals);
                let callee = match callee {
                    export::MethodCallee::Callable(callable) => {
                        self.lower_callable(*callable, substitution)
                    }
                    export::MethodCallee::Bound(bound) => {
                        let (callee, interface) =
                            self.resolve_bound_callee(*bound, receiver.ty, substitution);
                        if let Some(interface) = interface {
                            receiver = self.adapt_receiver_to_interface(receiver, interface);
                        }
                        callee
                    }
                };
                concrete::ExprKind::MethodCall {
                    receiver: Box::new(receiver),
                    callee,
                    args: args
                        .iter()
                        .map(|argument| self.lower_expr(argument, substitution, locals))
                        .collect(),
                }
            }
            export::ExprKind::Box(value) => {
                concrete::ExprKind::Box(Box::new(self.lower_expr(value, substitution, locals)))
            }
            export::ExprKind::Unbox(value) => {
                concrete::ExprKind::Unbox(Box::new(self.lower_expr(value, substitution, locals)))
            }
            export::ExprKind::IsInstance { operand, check_ty } => concrete::ExprKind::IsInstance {
                operand: Box::new(self.lower_expr(operand, substitution, locals)),
                check_ty: self.lower_type(*check_ty, substitution),
            },
            export::ExprKind::Cast { operand, optional } => concrete::ExprKind::Cast {
                operand: Box::new(self.lower_expr(operand, substitution, locals)),
                optional: *optional,
            },
            export::ExprKind::ArrayLiteral(elements) => concrete::ExprKind::ArrayLiteral(
                elements
                    .iter()
                    .map(|element| self.lower_expr(element, substitution, locals))
                    .collect(),
            ),
            export::ExprKind::Index { receiver, index } => concrete::ExprKind::Index {
                receiver: Box::new(self.lower_expr(receiver, substitution, locals)),
                index: Box::new(self.lower_expr(index, substitution, locals)),
            },
            export::ExprKind::ArrayLen(array) => {
                concrete::ExprKind::ArrayLen(Box::new(self.lower_expr(array, substitution, locals)))
            }
            export::ExprKind::ArrayClone(array) => concrete::ExprKind::ArrayClone(Box::new(
                self.lower_expr(array, substitution, locals),
            )),
            export::ExprKind::Call { callee, args } => concrete::ExprKind::Call {
                callee: self.lower_callable(*callee, substitution),
                args: args
                    .iter()
                    .map(|argument| self.lower_expr(argument, substitution, locals))
                    .collect(),
            },
            export::ExprKind::LocalFunctionCall {
                local_function,
                callee,
                captures,
                args,
            } => {
                let (callee, function_arguments) =
                    self.lower_callable_with_arguments(*callee, substitution);
                concrete::ExprKind::LocalFunctionCall {
                    local_function: self.ensure_local_function(
                        *local_function,
                        &function_arguments,
                        locals,
                    ),
                    callee,
                    captures: captures
                        .iter()
                        .map(|capture| self.lower_expr(capture, substitution, locals))
                        .collect(),
                    args: args
                        .iter()
                        .map(|argument| self.lower_expr(argument, substitution, locals))
                        .collect(),
                }
            }
            export::ExprKind::CallableCall {
                callee,
                function_type,
                args,
            } => concrete::ExprKind::CallableCall {
                callee: Box::new(self.lower_expr(callee, substitution, locals)),
                function_type: self.lower_function_type(*function_type, substitution),
                args: args
                    .iter()
                    .map(|argument| self.lower_expr(argument, substitution, locals))
                    .collect(),
            },
            export::ExprKind::Binary { op, lhs, rhs } => concrete::ExprKind::Binary {
                op: *op,
                lhs: Box::new(self.lower_expr(lhs, substitution, locals)),
                rhs: Box::new(self.lower_expr(rhs, substitution, locals)),
            },
            export::ExprKind::Unary { op, operand } => concrete::ExprKind::Unary {
                op: *op,
                operand: Box::new(self.lower_expr(operand, substitution, locals)),
            },
            export::ExprKind::SomeWrap(value) => {
                concrete::ExprKind::SomeWrap(Box::new(self.lower_expr(value, substitution, locals)))
            }
            export::ExprKind::NoneLiteral => concrete::ExprKind::NoneLiteral,
            export::ExprKind::IsSome(value) => {
                concrete::ExprKind::IsSome(Box::new(self.lower_expr(value, substitution, locals)))
            }
            export::ExprKind::Unwrap {
                operand,
                trap_on_none,
            } => concrete::ExprKind::Unwrap {
                operand: Box::new(self.lower_expr(operand, substitution, locals)),
                trap_on_none: *trap_on_none,
            },
        };
        concrete::Expr {
            kind,
            ty,
            span: source.span,
        }
    }

    fn lower_callable(
        &mut self,
        source: export::Callable,
        substitution: &[concrete::TypeId],
    ) -> concrete::Callable {
        self.lower_callable_with_arguments(source, substitution).0
    }

    /// Resolve a template-only bound member from the explicit conformance map
    /// generated by export HIR. The returned interface is present only when
    /// dispatch must use the bound interface directly (an interface receiver
    /// or an abstract subclass-provided implementation).
    fn resolve_bound_callee(
        &mut self,
        source: export::BoundCallableRefId,
        receiver: concrete::TypeId,
        substitution: &[concrete::TypeId],
    ) -> (concrete::Callable, Option<concrete::InterfaceId>) {
        let bound = self.source.bound_callable_refs[source].clone();
        let required_interface = self.lower_interface_application(bound.bound, substitution);
        match self.types[receiver].kind.clone() {
            concrete::TypeKind::Struct(id) => {
                let source = self.struct_source[&id];
                let arguments = self.structs[id].type_arguments.clone();
                let conformances = self.source.structs[source]
                    .interface_implementations
                    .clone();
                self.resolve_nominal_bound_target(
                    &conformances,
                    &arguments,
                    bound.member,
                    required_interface,
                )
            }
            concrete::TypeKind::Enum(id) => {
                let source = self.enum_source[&id];
                let arguments = self.enums[id].type_arguments.clone();
                let conformances = self.source.enums[source].interface_implementations.clone();
                self.resolve_nominal_bound_target(
                    &conformances,
                    &arguments,
                    bound.member,
                    required_interface,
                )
            }
            concrete::TypeKind::Class(id) => {
                let source = self.class_source[&id];
                let arguments = self.classes[id].type_arguments.clone();
                let conformances = self.source.classes[source]
                    .interface_implementations
                    .clone();
                self.resolve_nominal_bound_target(
                    &conformances,
                    &arguments,
                    bound.member,
                    required_interface,
                )
            }
            concrete::TypeKind::Interface(_) => {
                let application = self.source.interface_applications[bound.bound].clone();
                let arguments = application
                    .arguments
                    .iter()
                    .map(|argument| self.lower_type(*argument, substitution))
                    .collect();
                let function = self.source.interface_methods[bound.member].function;
                (
                    concrete::Callable::Function(self.request_function(function, arguments)),
                    Some(required_interface),
                )
            }
            kind => panic!(
                "bound receiver reached concretization without an explicit conformance: {kind:?}"
            ),
        }
    }

    fn resolve_nominal_bound_target(
        &mut self,
        conformances: &[export::InterfaceImplementation],
        owner_arguments: &[concrete::TypeId],
        member: export::InterfaceMethodId,
        required_interface: concrete::InterfaceId,
    ) -> (concrete::Callable, Option<concrete::InterfaceId>) {
        for conformance in conformances {
            let interface =
                self.lower_interface_application(conformance.interface, owner_arguments);
            if interface != required_interface {
                continue;
            }
            let implementation = conformance
                .methods
                .iter()
                .find(|implementation| implementation.member == member)
                .unwrap_or_else(|| {
                    panic!("export HIR conformance omits a required interface method")
                });
            return match &implementation.target {
                export::InterfaceImplementationTarget::Method(application) => (
                    self.lower_method_application(*application, owner_arguments),
                    None,
                ),
                export::InterfaceImplementationTarget::Subclass => {
                    let function = self.source.interface_methods[member].function;
                    (
                        concrete::Callable::Function(self.request_method(
                            function,
                            concrete::MethodOwner::Interface(required_interface),
                            MethodRequest::Plain,
                        )),
                        Some(required_interface),
                    )
                }
            };
        }
        panic!("export HIR has no explicit conformance for a validated bound call")
    }

    fn adapt_receiver_to_interface(
        &mut self,
        receiver: concrete::Expr,
        interface: concrete::InterfaceId,
    ) -> concrete::Expr {
        let target = self.interface_type[&interface];
        if receiver.ty == target {
            return receiver;
        }
        let span = receiver.span;
        let value = matches!(
            self.types[receiver.ty].kind,
            concrete::TypeKind::Unit
                | concrete::TypeKind::Int
                | concrete::TypeKind::UInt
                | concrete::TypeKind::Boolean
                | concrete::TypeKind::Struct(_)
                | concrete::TypeKind::Enum(_)
                | concrete::TypeKind::Tuple(_)
                | concrete::TypeKind::Ptr(_)
                | concrete::TypeKind::FunPtr(_)
        );
        if value {
            concrete::Expr {
                kind: concrete::ExprKind::Box(Box::new(receiver)),
                ty: target,
                span,
            }
        } else {
            concrete::Expr {
                kind: receiver.kind,
                ty: target,
                span,
            }
        }
    }

    fn lower_callable_with_arguments(
        &mut self,
        source: export::Callable,
        substitution: &[concrete::TypeId],
    ) -> (concrete::Callable, Vec<concrete::TypeId>) {
        match source {
            export::Callable::Function(function) => {
                assert!(
                    self.source.functions[function].type_param_count() == 0,
                    "a parameterized callable uses a resolved generic identity"
                );
                (
                    concrete::Callable::Function(self.request_function(function, Vec::new())),
                    Vec::new(),
                )
            }
            export::Callable::Generic(resolved) => {
                let resolved = self.source.instantiations[resolved].clone();
                let function = self.source.generic_functions[resolved.generic].function;
                let arguments: Vec<_> = resolved
                    .type_args
                    .iter()
                    .map(|argument| self.lower_type(*argument, substitution))
                    .collect();
                (
                    concrete::Callable::Function(
                        self.request_function(function, arguments.clone()),
                    ),
                    arguments,
                )
            }
            export::Callable::Method(application) => {
                self.lower_method_application_with_arguments(application, substitution)
            }
            export::Callable::GenericMethod(application) => {
                let application = self.source.generic_method_applications[application].clone();
                let owner = self.lower_generic_method_owner(application.owner, substitution);
                let method_arguments = export::NonEmptyVec::from_vec(
                    application
                        .method_arguments
                        .iter()
                        .map(|argument| self.lower_type(*argument, substitution))
                        .collect(),
                )
                .expect("export generic method applications are non-empty");
                let mut arguments = self.concrete_method_owner_arguments(owner).to_vec();
                arguments.extend(method_arguments.iter().copied());
                let function = self.source.generic_methods[application.method].function;
                let concrete = self.request_method(
                    function,
                    owner,
                    MethodRequest::Generic {
                        definition: application.method,
                        method_arguments,
                    },
                );
                (concrete::Callable::Function(concrete), arguments)
            }
        }
    }

    fn lower_method_application(
        &mut self,
        source: export::MethodApplicationId,
        substitution: &[concrete::TypeId],
    ) -> concrete::Callable {
        self.lower_method_application_with_arguments(source, substitution)
            .0
    }

    fn lower_method_application_with_arguments(
        &mut self,
        source: export::MethodApplicationId,
        substitution: &[concrete::TypeId],
    ) -> (concrete::Callable, Vec<concrete::TypeId>) {
        let application = self.source.method_applications[source].clone();
        let owner = self.lower_method_owner(application.owner, substitution);
        let arguments = self.concrete_method_owner_arguments(owner).to_vec();
        let function = self.request_method(application.function, owner, MethodRequest::Plain);
        (concrete::Callable::Function(function), arguments)
    }

    fn lower_method_owner(
        &mut self,
        owner: export::MethodOwnerApplication,
        substitution: &[concrete::TypeId],
    ) -> concrete::MethodOwner {
        match owner {
            export::MethodOwnerApplication::Class(id) => {
                concrete::MethodOwner::Class(self.lower_class_application(id, substitution))
            }
            export::MethodOwnerApplication::Struct(id) => {
                concrete::MethodOwner::Struct(self.lower_struct_application(id, substitution))
            }
            export::MethodOwnerApplication::Enum(id) => {
                concrete::MethodOwner::Enum(self.lower_enum_application(id, substitution))
            }
            export::MethodOwnerApplication::Interface(id) => {
                concrete::MethodOwner::Interface(self.lower_interface_application(id, substitution))
            }
            export::MethodOwnerApplication::Any => concrete::MethodOwner::Any,
        }
    }

    fn lower_generic_method_owner(
        &mut self,
        owner: export::GenericMethodOwner,
        substitution: &[concrete::TypeId],
    ) -> concrete::MethodOwner {
        match owner {
            export::GenericMethodOwner::Class(id) => {
                concrete::MethodOwner::Class(self.lower_class_application(id, substitution))
            }
            export::GenericMethodOwner::Struct(id) => {
                concrete::MethodOwner::Struct(self.lower_struct_application(id, substitution))
            }
            export::GenericMethodOwner::Enum(id) => {
                concrete::MethodOwner::Enum(self.lower_enum_application(id, substitution))
            }
        }
    }

    fn lower_place(&self, source: export::Place, locals: &[concrete::LocalId]) -> concrete::Place {
        match source {
            export::Place::Local(local) => concrete::Place::Local(self.lower_local(local, locals)),
            export::Place::Global(global) => concrete::Place::Global(self.global_map[&global]),
        }
    }

    fn lower_field_ref(
        &mut self,
        source: export::FieldRef,
        substitution: &[concrete::TypeId],
    ) -> concrete::FieldRef {
        match source {
            export::FieldRef::StructField { application, index } => {
                let concrete_id = self.lower_struct_application(application, substitution);
                concrete::FieldRef::StructField {
                    struct_id: concrete_id,
                    index,
                }
            }
            export::FieldRef::TupleIndex(index) => concrete::FieldRef::TupleIndex(index),
            export::FieldRef::ClassField { application, index } => {
                let concrete_id = self.lower_class_application(application, substitution);
                concrete::FieldRef::ClassField {
                    class_id: concrete_id,
                    index,
                }
            }
        }
    }

    fn lower_local(
        &self,
        source: export::LocalId,
        locals: &[concrete::LocalId],
    ) -> concrete::LocalId {
        locals[source.into_raw().into_u32() as usize]
    }

    fn lower_capture(
        &mut self,
        source: &export::Capture,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::Capture {
        concrete::Capture {
            binding: concrete::BindingId::from_raw(source.binding.into_raw()),
            name: source.name.clone(),
            ty: self.lower_type(source.ty, substitution),
            first_use_span: source.first_use_span,
            source: self.lower_expr(&source.source, substitution, locals),
        }
    }

    fn ensure_lambda(
        &mut self,
        source_id: export::LambdaId,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::LambdaId {
        let key = (source_id, substitution.to_vec());
        if let Some(&id) = self.lambda_by_key.get(&key) {
            return id;
        }
        let source = self.source.lambdas[source_id].clone();
        let value = concrete::Lambda {
            function: self.request_function(source.function, substitution.to_vec()),
            function_type: self.lower_function_type(source.function_type, substitution),
            captures: source
                .captures
                .iter()
                .map(|capture| self.lower_capture(capture, substitution, locals))
                .collect(),
            span: source.span,
        };
        let id = self.lambdas.alloc(value);
        self.lambda_by_key.insert(key, id);
        id
    }

    fn ensure_anonymous(
        &mut self,
        source_id: export::AnonymousFunctionId,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::AnonymousFunctionId {
        let key = (source_id, substitution.to_vec());
        if let Some(&id) = self.anonymous_by_key.get(&key) {
            return id;
        }
        let source = self.source.anonymous_functions[source_id].clone();
        let value = concrete::AnonymousFunction {
            function: self.request_function(source.function, substitution.to_vec()),
            function_type: self.lower_function_type(source.function_type, substitution),
            captures: source
                .captures
                .iter()
                .map(|capture| self.lower_capture(capture, substitution, locals))
                .collect(),
            span: source.span,
        };
        let id = self.anonymous_functions.alloc(value);
        self.anonymous_by_key.insert(key, id);
        id
    }

    fn ensure_local_function(
        &mut self,
        source_id: export::LocalFunctionId,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::LocalFunctionId {
        let key = (source_id, substitution.to_vec());
        if let Some(&id) = self.local_by_key.get(&key) {
            return id;
        }
        let source = self.source.local_functions[source_id].clone();
        let value = concrete::LocalFunction {
            function: self.request_function(source.function, substitution.to_vec()),
            function_type: self.lower_function_type(source.function_type, substitution),
            captures: source
                .captures
                .iter()
                .map(|capture| self.lower_capture(capture, substitution, locals))
                .collect(),
            span: source.span,
        };
        let id = self.local_functions.alloc(value);
        self.local_by_key.insert(key, id);
        id
    }

    fn ensure_reference(
        &mut self,
        source_id: export::CallableReferenceId,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::CallableReferenceId {
        let key = (source_id, substitution.to_vec());
        if let Some(&id) = self.reference_by_key.get(&key) {
            return id;
        }
        let source = self.source.callable_references[source_id].clone();
        let target = match source.target {
            export::CallableReferenceTarget::Named(callee) => {
                concrete::CallableReferenceTarget::Named(self.lower_callable(callee, substitution))
            }
            export::CallableReferenceTarget::Local {
                local_function,
                callee,
            } => {
                let (callee, arguments) = self.lower_callable_with_arguments(callee, substitution);
                concrete::CallableReferenceTarget::Local {
                    local_function: self.ensure_local_function(local_function, &arguments, locals),
                    callee,
                }
            }
            export::CallableReferenceTarget::BoundMember { receiver, callee } => {
                let mut receiver = self.lower_expr(&receiver, substitution, locals);
                let callee = match callee {
                    export::MethodCallee::Callable(callable) => {
                        self.lower_callable(callable, substitution)
                    }
                    export::MethodCallee::Bound(bound) => {
                        let (callee, interface) =
                            self.resolve_bound_callee(bound, receiver.ty, substitution);
                        if let Some(interface) = interface {
                            receiver = self.adapt_receiver_to_interface(receiver, interface);
                        }
                        callee
                    }
                };
                concrete::CallableReferenceTarget::BoundMember {
                    receiver: Box::new(receiver),
                    callee,
                }
            }
            export::CallableReferenceTarget::BoundExtension { receiver, callee } => {
                concrete::CallableReferenceTarget::BoundExtension {
                    receiver: Box::new(self.lower_expr(&receiver, substitution, locals)),
                    callee: self.lower_callable(callee, substitution),
                }
            }
        };
        let value = concrete::CallableReference {
            target,
            function_type: self.lower_function_type(source.function_type, substitution),
            captures: source
                .captures
                .iter()
                .map(|capture| self.lower_capture(capture, substitution, locals))
                .collect(),
            span: source.span,
        };
        let id = self.callable_references.alloc(value);
        self.reference_by_key.insert(key, id);
        id
    }

    fn ensure_coercion(
        &mut self,
        source_id: export::FunctionCoercionId,
        substitution: &[concrete::TypeId],
    ) -> concrete::FunctionCoercionId {
        let key = (source_id, substitution.to_vec());
        if let Some(&id) = self.coercion_by_key.get(&key) {
            return id;
        }
        let source = self.source.function_coercions[source_id].clone();
        let value = concrete::FunctionCoercion {
            source: self.lower_function_type(source.source, substitution),
            target: self.lower_function_type(source.target, substitution),
        };
        let id = self.function_coercions.alloc(value);
        self.coercion_by_key.insert(key, id);
        id
    }

    fn ensure_foreign_callback(
        &mut self,
        source_id: export::ForeignCallbackRegistrationId,
        substitution: &[concrete::TypeId],
        callback: concrete::StructId,
    ) -> concrete::ForeignCallbackRegistrationId {
        let key = (source_id, substitution.to_vec());
        if let Some(&id) = self.foreign_callback_by_key.get(&key) {
            assert_eq!(self.foreign_callback_registrations[id].callback, callback);
            return id;
        }
        let source = self.source.foreign_callback_registrations[source_id].clone();
        let native_function_type =
            self.lower_function_type(source.native_function_type, substitution);
        let managed_function_type =
            self.lower_function_type(source.managed_function_type, substitution);
        let mode = match source.mode {
            export::ForeignCallbackMode::Reusable => concrete::ForeignCallbackMode::Reusable,
            export::ForeignCallbackMode::OneShot => concrete::ForeignCallbackMode::OneShot,
        };
        let id = self
            .foreign_callback_registrations
            .alloc(concrete::ForeignCallbackRegistration {
                callback,
                native_function_type,
                managed_function_type,
                context_index: source.context_index,
                mode,
            });
        self.foreign_callback_by_key.insert(key, id);
        id
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
