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
struct FunctionKey {
    source: export::FunctionId,
    arguments: Vec<concrete::TypeId>,
}

#[derive(Debug, Clone, Copy)]
enum ValueOwner {
    Struct(export::StructId),
    Enum(export::EnumId),
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
    enums: Arena<concrete::EnumDef>,
    enum_by_key: HashMap<(export::EnumId, Vec<concrete::TypeId>), concrete::EnumId>,
    enum_type: HashMap<concrete::EnumId, concrete::TypeId>,
    interfaces: Arena<concrete::InterfaceDef>,
    interface_by_key: HashMap<(export::InterfaceId, Vec<concrete::TypeId>), concrete::InterfaceId>,
    interface_type: HashMap<concrete::InterfaceId, concrete::TypeId>,
    classes: Arena<concrete::ClassDef>,
    class_map: HashMap<export::ClassId, concrete::ClassId>,
    class_type: HashMap<concrete::ClassId, concrete::TypeId>,
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
        let mut result = Self {
            source,
            types: Arena::new(),
            type_by_kind: HashMap::new(),
            function_types: Arena::new(),
            function_type_by_value: HashMap::new(),
            structs: Arena::new(),
            struct_by_key: HashMap::new(),
            struct_type: HashMap::new(),
            enums: Arena::new(),
            enum_by_key: HashMap::new(),
            enum_type: HashMap::new(),
            interfaces: Arena::new(),
            interface_by_key: HashMap::new(),
            interface_type: HashMap::new(),
            classes: Arena::new(),
            class_map: HashMap::new(),
            class_type: HashMap::new(),
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
        };
        result.allocate_class_shells();
        result
    }

    fn run(mut self) -> concrete::Module {
        let unit = self.lower_type(self.source.unit, &[]);
        let int = self.lower_type(self.source.int, &[]);
        let boolean = self.lower_type(self.source.boolean, &[]);
        let string = self.lower_type(self.source.string, &[]);

        self.fill_classes();
        self.lower_extern_functions();
        self.lower_globals();

        // Non-generic aggregate declarations and source functions are local
        // concrete entities even when no expression happens to mention them.
        for (id, declaration) in self.source.structs.iter() {
            if declaration.type_params.is_empty() {
                self.ensure_struct(id, Vec::new());
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
        for (id, function) in self.source.functions.iter() {
            if function.type_params.is_empty() && self.is_emittable_source_function(id) {
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

        let functions = arena_from_complete_slots(self.function_slots, "concrete function");
        let entry = self.function_by_key[&FunctionKey {
            source: self.source.entry,
            arguments: Vec::new(),
        }];
        let throwable = self.class_map[&self.source.coroutine_core.throwable];
        let illegal_state_exception =
            self.class_map[&self.source.coroutine_core.illegal_state_exception];
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
                if matches!(function.kind, concrete::FunctionKind::Intrinsic(ref name) if name == "coroutine_start" || name == "coroutine_suspend")
                {
                    results.extend(function.type_arguments.iter().copied());
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
            protocols.extend(
                results
                    .into_iter()
                    .map(|result_type| concrete::CoroutineProtocol {
                        result_type,
                        continuation: self.ensure_interface(core.continuation, vec![result_type]),
                        suspend_task: self.ensure_interface(core.suspend_task, vec![result_type]),
                        suspend_registration: self
                            .ensure_interface(core.suspend_registration, vec![result_type]),
                        start_coroutine: self
                            .request_function(core.start_coroutine, vec![result_type]),
                        suspend_coroutine: self
                            .request_function(core.suspend_coroutine, vec![result_type]),
                        continuation_resume: self
                            .request_function(core.continuation_resume, vec![result_type]),
                        continuation_resume_with_exception: self.request_function(
                            core.continuation_resume_with_exception,
                            vec![result_type],
                        ),
                        suspend_task_run: self
                            .request_function(core.suspend_task_run, vec![result_type]),
                        suspend_registration_register: self.request_function(
                            core.suspend_registration_register,
                            vec![result_type],
                        ),
                    }),
            );
        }
        protocols
    }

    fn allocate_class_shells(&mut self) {
        for (source_id, source) in self.source.classes.iter() {
            let id = self.classes.alloc(concrete::ClassDef {
                modifier: source.modifier,
                name: source.name.clone(),
                constructor: Vec::new(),
                base_class: None,
                interfaces: Vec::new(),
                span: source.span,
            });
            self.class_map.insert(source_id, id);
        }
    }

    fn fill_classes(&mut self) {
        for (source_id, source) in self.source.classes.iter() {
            let source = source.clone();
            let id = self.class_map[&source_id];
            let constructor = source
                .constructor
                .iter()
                .map(|field| concrete::Field {
                    name: field.name.clone(),
                    ty: self.lower_type(field.ty, &[]),
                })
                .collect();
            let interfaces = source
                .interfaces
                .iter()
                .map(|ty| self.lower_type(*ty, &[]))
                .collect();
            let base_class = source.base_class.map(|(base, args)| {
                let base = self.class_map[&base];
                let args = args
                    .iter()
                    .map(|argument| self.lower_expr(argument, &[], &[]))
                    .collect();
                (base, args)
            });
            let target = &mut self.classes[id];
            target.constructor = constructor;
            target.interfaces = interfaces;
            target.base_class = base_class;
        }
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
            export::ConstantValue::Struct { struct_id, fields } => {
                let struct_id = self.ensure_struct(*struct_id, Vec::new());
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
            export::Type::Struct(id, arguments) => {
                if id == self.source.ffi_core.fun_ptr {
                    let [function] = arguments.as_slice() else {
                        panic!("validated deferred FunPtr has one argument")
                    };
                    let function = self.lower_type(*function, substitution);
                    let concrete::TypeKind::Function(function) = self.types[function].kind else {
                        panic!("deferred FunPtr resolves to a concrete function type")
                    };
                    return self.intern_type(concrete::TypeKind::FunPtr(function), true);
                }
                let arguments = arguments
                    .iter()
                    .map(|argument| self.lower_type(*argument, substitution))
                    .collect();
                let id = self.ensure_struct(id, arguments);
                self.struct_type[&id]
            }
            export::Type::Class(id) => {
                let id = self.class_map[&id];
                if let Some(&ty) = self.class_type.get(&id) {
                    ty
                } else {
                    let ty = self.intern_type(concrete::TypeKind::Class(id), false);
                    self.class_type.insert(id, ty);
                    ty
                }
            }
            export::Type::Interface(id, arguments) => {
                let arguments = arguments
                    .iter()
                    .map(|argument| self.lower_type(*argument, substitution))
                    .collect();
                let id = self.ensure_interface(id, arguments);
                self.interface_type[&id]
            }
            export::Type::Any => self.intern_type(concrete::TypeKind::Any, false),
            export::Type::Array(element) => {
                let element = self.lower_type(element, substitution);
                self.intern_type(concrete::TypeKind::Array(element), false)
            }
            export::Type::MutableArray(element) => {
                let element = self.lower_type(element, substitution);
                self.intern_type(concrete::TypeKind::MutableArray(element), false)
            }
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
            export::Type::Enum(id, arguments) => {
                let arguments = arguments
                    .iter()
                    .map(|argument| self.lower_type(*argument, substitution))
                    .collect();
                let id = self.ensure_enum(id, arguments);
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
    ) -> concrete::StructId {
        let key = (source_id, arguments.clone());
        if let Some(&id) = self.struct_by_key.get(&key) {
            return id;
        }
        let source = self.source.structs[source_id].clone();
        assert_eq!(source.type_params.len(), arguments.len());
        let name = self.instance_name(&source.name, &arguments);
        let id = self.structs.alloc(concrete::StructDef {
            name,
            type_arguments: arguments.clone(),
            gc_free: false,
            attributes: source.attributes,
            fields: Vec::new(),
            interfaces: Vec::new(),
            span: source.span,
        });
        self.struct_by_key.insert(key, id);
        let ty = self.intern_type(concrete::TypeKind::Struct(id), false);
        self.struct_type.insert(id, ty);
        let fields: Vec<_> = source
            .fields
            .iter()
            .map(|field| concrete::Field {
                name: field.name.clone(),
                ty: self.lower_type(field.ty, &arguments),
            })
            .collect();
        let interfaces = source
            .interfaces
            .iter()
            .map(|interface| self.lower_type(*interface, &arguments))
            .collect();
        let gc_free = fields.iter().all(|field| self.types[field.ty].gc_free);
        assert!(
            !source.attributes.no_gc || gc_free,
            "HIR diagnoses an invalid @NoGC struct specialization"
        );
        self.structs[id].fields = fields;
        self.structs[id].interfaces = interfaces;
        self.structs[id].gc_free = gc_free;
        self.types[ty].gc_free = gc_free;
        self.request_concrete_value_methods(ValueOwner::Struct(source_id), &arguments);
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
            span: source.span,
        });
        self.enum_by_key.insert(key, id);
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
            .interfaces
            .iter()
            .map(|interface| self.lower_type(*interface, &arguments))
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
        self.request_concrete_value_methods(ValueOwner::Enum(source_id), &arguments);
        id
    }

    /// Materialize every non-generic method of a fully specialized value
    /// type. These methods may be reached only through an interface thunk,
    /// so no ordinary call expression necessarily requests them.
    fn request_concrete_value_methods(
        &mut self,
        owner: ValueOwner,
        arguments: &[concrete::TypeId],
    ) {
        let methods = self
            .source
            .functions
            .iter()
            .filter_map(|(id, function)| {
                let method = function.method?;
                if method.owner_type_param_count as usize != function.type_params.len() {
                    return None;
                }
                let owner_matches = match (&self.source.types[method.owner], owner) {
                    (export::Type::Struct(id, _), ValueOwner::Struct(expected)) => *id == expected,
                    (export::Type::Enum(id, _), ValueOwner::Enum(expected)) => *id == expected,
                    _ => false,
                };
                owner_matches.then_some(id)
            })
            .collect::<Vec<_>>();
        for method in methods {
            self.request_function(method, arguments.to_vec());
        }
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
        let methods = source
            .methods
            .iter()
            .filter(|method| method.type_params.is_empty())
            .map(|method| concrete::MethodSig {
                name: method.name.clone(),
                is_suspend: method.is_suspend,
                attributes: method.attributes,
                params: method
                    .params
                    .iter()
                    .map(|param| concrete::Param {
                        name: param.name.clone(),
                        ty: self.lower_type(param.ty, &arguments),
                        local: remap_idx(param.local),
                    })
                    .collect(),
                return_ty: self.lower_type(method.return_ty, &arguments),
                span: method.span,
            })
            .collect();
        self.interfaces[id].methods = methods;
        id
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
            concrete::TypeKind::Class(id) => self.classes[*id].name.clone(),
            concrete::TypeKind::Interface(id) => self.interfaces[*id].name.clone(),
            concrete::TypeKind::Any => "Any".to_string(),
            concrete::TypeKind::Array(element) => format!("A{}X", self.encode_type(*element)),
            concrete::TypeKind::MutableArray(element) => {
                format!("M{}X", self.encode_type(*element))
            }
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
        let key = FunctionKey { source, arguments };
        if let Some(&id) = self.function_by_key.get(&key) {
            return id;
        }
        assert_eq!(
            self.source.functions[source].type_params.len(),
            key.arguments.len()
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
        let source = self.source.functions[key.source].clone();
        let (kind, local_map) = match &source.kind {
            export::FunctionKind::User(body) => {
                let (body, local_map) = self.lower_body(body, &key.arguments);
                (concrete::FunctionKind::User(body), local_map)
            }
            export::FunctionKind::Intrinsic(name) => {
                (concrete::FunctionKind::Intrinsic(name.clone()), Vec::new())
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
                ty: self.lower_type(param.ty, &key.arguments),
                local: local_map
                    .get(param.local.into_raw().into_u32() as usize)
                    .copied()
                    .unwrap_or_else(|| remap_idx(param.local)),
            })
            .collect();
        let return_ty = self.lower_type(source.return_ty, &key.arguments);
        let method = source.method.map(|method| concrete::Method {
            owner: self.lower_type(method.owner, &key.arguments),
            modifier: method.modifier,
        });
        let generic = self
            .source
            .generic_functions
            .iter()
            .find_map(|(id, generic)| (generic.function == key.source).then_some(id));
        let generic_discriminator = generic.and_then(|generic| {
            self.overloaded_generic_names
                .contains(&source.name)
                .then(|| generic.into_raw().into_u32())
        });
        concrete::Function {
            name: source.name,
            type_arguments: key.arguments.clone(),
            generic_discriminator,
            is_suspend: source.is_suspend,
            params,
            return_ty,
            attributes: source.attributes,
            kind,
            method,
            span: source.span,
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
                let field = self.lower_field_ref(*field, receiver.ty);
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
                enum_id,
                variant,
                fields,
            } => {
                let concrete::TypeKind::Enum(concrete_enum) = self.types[expected].kind else {
                    panic!("a resolved variant pattern has a concrete enum subject")
                };
                let source_enum = self
                    .enum_by_key
                    .iter()
                    .find_map(|((source, _), id)| (*id == concrete_enum).then_some(*source));
                assert_eq!(source_enum, Some(*enum_id));
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
            export::Pattern::Struct { struct_id, fields } => {
                let concrete::TypeKind::Struct(concrete_struct) = self.types[expected].kind else {
                    panic!("a resolved struct pattern has a concrete struct subject")
                };
                let source_struct = self
                    .struct_by_key
                    .iter()
                    .find_map(|((source, _), id)| (*id == concrete_struct).then_some(*source));
                assert_eq!(source_struct, Some(*struct_id));
                let definition = self.structs[concrete_struct].fields.clone();
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
            export::ExprKind::StructInit { struct_id, args } => {
                let concrete::TypeKind::Struct(id) = self.types[ty].kind else {
                    panic!("a resolved struct initializer has a concrete struct type")
                };
                assert!(
                    self.struct_by_key
                        .keys()
                        .any(|(source, _)| source == struct_id)
                );
                concrete::ExprKind::StructInit {
                    struct_id: id,
                    args: args
                        .iter()
                        .map(|argument| self.lower_expr(argument, substitution, locals))
                        .collect(),
                }
            }
            export::ExprKind::ClassInit { class_id, args } => concrete::ExprKind::ClassInit {
                class_id: self.class_map[class_id],
                args: args
                    .iter()
                    .map(|argument| self.lower_expr(argument, substitution, locals))
                    .collect(),
            },
            export::ExprKind::VariantConstruct {
                enum_id,
                variant,
                args,
                ..
            } => {
                let concrete::TypeKind::Enum(id) = self.types[ty].kind else {
                    panic!("a resolved variant constructor has a concrete enum type")
                };
                assert!(self.enum_by_key.keys().any(|(source, _)| source == enum_id));
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
                let field = self.lower_field_ref(*field, receiver.ty);
                concrete::ExprKind::FieldAccess {
                    receiver: Box::new(receiver),
                    field,
                }
            }
            export::ExprKind::MethodCall {
                receiver,
                callee,
                args,
            } => concrete::ExprKind::MethodCall {
                receiver: Box::new(self.lower_expr(receiver, substitution, locals)),
                callee: self.lower_callable(*callee, substitution),
                args: args
                    .iter()
                    .map(|argument| self.lower_expr(argument, substitution, locals))
                    .collect(),
            },
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

    fn lower_callable_with_arguments(
        &mut self,
        source: export::Callable,
        substitution: &[concrete::TypeId],
    ) -> (concrete::Callable, Vec<concrete::TypeId>) {
        let (function, arguments) = match source {
            export::Callable::Function(function) => {
                assert!(
                    self.source.functions[function].type_params.is_empty(),
                    "a parameterized callable uses a resolved generic identity"
                );
                (function, Vec::new())
            }
            export::Callable::Generic(resolved) => {
                let resolved = self.source.instantiations[resolved].clone();
                let function = self.source.generic_functions[resolved.generic].function;
                let arguments = resolved
                    .type_args
                    .iter()
                    .map(|argument| self.lower_type(*argument, substitution))
                    .collect();
                (function, arguments)
            }
        };
        let concrete = self.request_function(function, arguments.clone());
        (concrete::Callable::Function(concrete), arguments)
    }

    fn lower_place(&self, source: export::Place, locals: &[concrete::LocalId]) -> concrete::Place {
        match source {
            export::Place::Local(local) => concrete::Place::Local(self.lower_local(local, locals)),
            export::Place::Global(global) => concrete::Place::Global(self.global_map[&global]),
        }
    }

    fn lower_field_ref(
        &self,
        source: export::FieldRef,
        receiver: concrete::TypeId,
    ) -> concrete::FieldRef {
        match source {
            export::FieldRef::StructField { struct_id, index } => {
                let concrete::TypeKind::Struct(concrete_id) = self.types[receiver].kind else {
                    panic!("a struct field has a concrete struct receiver")
                };
                assert!(
                    self.struct_by_key
                        .keys()
                        .any(|(source, _)| *source == struct_id)
                );
                concrete::FieldRef::StructField {
                    struct_id: concrete_id,
                    index,
                }
            }
            export::FieldRef::TupleIndex(index) => concrete::FieldRef::TupleIndex(index),
            export::FieldRef::ClassField { class_id, index } => concrete::FieldRef::ClassField {
                class_id: self.class_map[&class_id],
                index,
            },
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
                concrete::CallableReferenceTarget::BoundMember {
                    receiver: Box::new(self.lower_expr(&receiver, substitution, locals)),
                    callee: self.lower_callable(callee, substitution),
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
    for (_, generic) in module.generic_functions.iter() {
        let function = &module.functions[generic.function];
        *counts.entry(function.name.clone()).or_default() += 1;
    }
    counts
        .into_iter()
        .filter_map(|(name, count)| (count > 1).then_some(name))
        .collect()
}

fn export_type_has_param(module: &export::Module, ty: export::TypeId) -> bool {
    match &module.types[ty] {
        export::Type::Param(_) => true,
        export::Type::Array(element)
        | export::Type::MutableArray(element)
        | export::Type::Ptr(element) => export_type_has_param(module, *element),
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
        export::Type::Struct(_, arguments)
        | export::Type::Interface(_, arguments)
        | export::Type::Enum(_, arguments) => arguments
            .iter()
            .any(|argument| export_type_has_param(module, *argument)),
        _ => false,
    }
}
