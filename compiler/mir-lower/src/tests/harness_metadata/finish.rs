//! Seal the complete HIR graph assembled by a MIR unit fixture.

use super::*;
use crate::tests::harness_nominals::test_nominal_identities_without_objects;

impl Harness {
    pub(in crate::tests) fn finish(self, entry: hir::FunctionId) -> hir::ExportHirOutput {
        self.finish_with_coroutine_core(entry, false)
    }

    pub(in crate::tests) fn finish_with_initialization_core(
        self,
        entry: hir::FunctionId,
    ) -> hir::ExportHirOutput {
        self.finish(entry)
    }

    pub(in crate::tests) fn finish_coroutines(
        self,
        entry: hir::FunctionId,
    ) -> hir::ExportHirOutput {
        self.finish_with_coroutine_core(entry, true)
    }

    pub(in crate::tests) fn finish_with_coroutine_core(
        mut self,
        entry: hir::FunctionId,
        include_exceptions: bool,
    ) -> hir::ExportHirOutput {
        let exception_core = self.test_exception_core(include_exceptions);
        let coroutine_core = self.test_coroutine_core(exception_core.throwable.class());
        let t = self
            .types
            .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
        let ptr = self.declare_struct("Ptr", vec![type_param("T")], vec![t], &[], &[]);
        let fun_ptr = self.declare_struct("FunPtr", vec![type_param("F")], vec![t], &[], &[]);
        let (pinned_ptr, gc_handle) = if let Some(core) = self.gc_core {
            (core.pinned_ptr, core.gc_handle)
        } else {
            (
                self.declare_struct("PinnedPtr", vec![type_param("T")], vec![t], &[], &[]),
                self.declare_struct("GcHandle", vec![type_param("T")], vec![t], &[], &[]),
            )
        };
        let ffi_core = hir::FfiCore {
            ptr,
            fun_ptr,
            pinned_ptr,
            gc_handle,
            ptr_to_ulong: entry,
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
                    style: hir::VariantStyle::Unit,
                    fields: Vec::new(),
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
        let integer_types = self.integers;
        let integer_owners = hir::IntegerKind::ALL.map(|kind| {
            self.declare_fixed_intrinsic_struct(
                kind.canonical_name(),
                hir::IntrinsicTypeKind::Integer(kind),
                integer_types.owner(kind),
            )
        });
        let intrinsic_integers = hir::IntegerTypeCore::new(integer_owners)
            .expect("the eight integer kinds receive distinct nominal owners");
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
        let intrinsic_type_core = hir::IntrinsicTypeCore {
            integers: intrinsic_integers,
            boolean: intrinsic_boolean,
            string: intrinsic_string,
            array: intrinsic_array,
            mutable_array: intrinsic_mutable_array,
            ptr,
            fun_ptr,
        };
        let mut source_contexts = Arena::new();
        source_contexts.alloc(hir::SourceContext::new(
            scoop_identity::SourceIdentity::single_file(),
            hir::SourceContextSubject::File,
        ));
        let option_some = hir::EnumVariantRef::checked(&self.enums, self.option_enum, 0)
            .expect("test Option has Some");
        let option_some_payload = hir::EnumVariantFieldRef::checked(&self.enums, option_some, 0)
            .expect("test Option Some has its payload");
        let option_none = hir::EnumVariantRef::checked(&self.enums, self.option_enum, 1)
            .expect("test Option has None");
        let option_core =
            hir::OptionCore::checked(&self.enums, &self.types, option_some_payload, option_none)
                .expect("test Option has the core shape");
        let iteration_core = self.test_iteration_core(option_core);
        let callback_mode_application = self.enums[callback_mode].self_application;
        let callback_modes = hir::ForeignCallbackModes::checked(
            &self.enums,
            &self.enum_applications,
            hir::AppliedEnumVariantRef::checked_index(
                &self.enums,
                &self.enum_applications,
                callback_mode_application,
                0,
            )
            .expect("test callback mode has Reusable"),
            hir::AppliedEnumVariantRef::checked_index(
                &self.enums,
                &self.enum_applications,
                callback_mode_application,
                1,
            )
            .expect("test callback mode has OneShot"),
        )
        .expect("test callback mode has the core shape");
        let callback_state_application = self.enums[callback_state].self_application;
        let callback_state_ref = |index| {
            hir::AppliedEnumVariantRef::checked_index(
                &self.enums,
                &self.enum_applications,
                callback_state_application,
                index,
            )
            .expect("test callback state variant exists")
        };
        let callback_states = hir::ForeignCallbackStates::checked(
            &self.enums,
            &self.enum_applications,
            callback_state_ref(0),
            callback_state_ref(1),
            callback_state_ref(2),
            callback_state_ref(3),
        )
        .expect("test callback state has the core shape");
        let throwable = self.class_ty(exception_core.throwable.class());
        let callback_failure_application = self.enum_application(self.option_enum, vec![throwable]);
        let callback_failure_some = hir::AppliedEnumVariantRef::checked(
            &self.enums,
            &self.enum_applications,
            callback_failure_application,
            option_core.some(),
        )
        .expect("test callback failure has Some");
        let callback_failure_result = hir::ForeignCallbackFailureResult::checked(
            &self.enums,
            &self.enum_applications,
            option_core,
            throwable,
            hir::AppliedEnumVariantFieldRef::checked(
                &self.enums,
                &self.enum_applications,
                callback_failure_some,
                option_core.some_payload().local_index(),
            )
            .expect("test callback failure Some has a payload"),
            hir::AppliedEnumVariantRef::checked(
                &self.enums,
                &self.enum_applications,
                callback_failure_application,
                option_core.none(),
            )
            .expect("test callback failure has None"),
        )
        .expect("test callback failure has the core shape");
        let nominal_identities = test_nominal_identities_without_objects(
            &self.structs,
            &self.enums,
            &self.classes,
            &self.interfaces,
        );
        let property_identities = crate::tests::harness_nominals::test_property_identities(
            &self.properties,
            &Arena::new(),
            &nominal_identities,
        );
        let property_accessor_identities =
            crate::tests::harness_nominals::test_property_accessor_identities(
                &self.properties,
                &property_identities,
                &self.property_getters,
                &self.property_setters,
            );
        let type_alias_identities =
            hir::HirTypeAliasIdentities::from_declarations(&Arena::new(), Vec::new())
                .expect("the MIR test fixture has an empty type-alias arena");
        let enum_member_identities =
            hir::HirEnumMemberIdentities::from_declarations(&self.enums, &nominal_identities)
                .expect("the MIR test fixture enum members have persistent identities");
        let field_identities = hir::HirFieldIdentities::from_declarations(
            &self.structs,
            &self.classes,
            &Arena::new(),
            &self.class_fields,
            &self.properties,
            &Arena::new(),
            &nominal_identities,
            &property_identities,
        )
        .expect("the MIR test fixture fields have persistent identities");
        let object_value_identities = hir::HirObjectValueIdentities::from_declarations(
            &Arena::new(),
            &Arena::new(),
            &Arena::new(),
            &nominal_identities,
        )
        .expect("the MIR test fixture has no object values");
        let initialization_unit_identities =
            hir::HirInitializationUnitIdentities::from_declarations(
                &Arena::new(),
                &Arena::new(),
                &self.functions,
                &Arena::new(),
                &Arena::new(),
                &Arena::new(),
                &Arena::new(),
                &Arena::new(),
                &self.properties,
                &Arena::new(),
                &nominal_identities,
                &property_identities,
            )
            .expect("the MIR test fixture has no initialization units");
        let type_identity_inputs = hir::HirTypeIdentityInputs {
            types: &self.types,
            function_types: &self.function_types,
            structs: &self.structs,
            struct_applications: &self.struct_applications,
            enums: &self.enums,
            enum_applications: &self.enum_applications,
            classes: &self.classes,
            class_applications: &self.class_applications,
            interfaces: &self.interfaces,
            interface_applications: &self.interface_applications,
            objects: &Arena::new(),
            core_types: hir::HirCoreTypeIdentityAuthority::Defined(&intrinsic_type_core),
            nominal_identities: &nominal_identities,
        };
        let type_identities = hir::HirTypeIdentities::from_types(type_identity_inputs)
            .expect("the MIR test fixture types have persistent identities");
        let constructor_identities = crate::tests::harness_nominals::test_constructor_identities(
            type_identity_inputs,
            &self.struct_constructors,
            &self.class_constructors,
            &self.class_constructor_applications,
        );
        let function_identity_rows = self
            .functions
            .iter()
            .map(|(function, declaration)| {
                let accessor = self
                    .property_getters
                    .iter()
                    .find_map(|(getter, value)| {
                        matches!(
                            value.implementation,
                            hir::PropertyAccessorImplementation::Body(actual)
                                | hir::PropertyAccessorImplementation::AbstractSlot(actual)
                                if actual == function
                        )
                        .then_some(hir::HirPropertyAccessorFunction::Getter(getter))
                    })
                    .or_else(|| {
                        self.property_setters.iter().find_map(|(setter, value)| {
                            matches!(
                                value.implementation,
                                hir::PropertyAccessorImplementation::Body(actual)
                                    | hir::PropertyAccessorImplementation::AbstractSlot(actual)
                                    if actual == function
                            )
                            .then_some(hir::HirPropertyAccessorFunction::Setter(setter))
                        })
                    });
                if let Some(accessor) = accessor {
                    return hir::HirFunctionIdentity::property_accessor(accessor);
                }
                assert!(!matches!(
                    declaration.kind,
                    hir::FunctionKind::DerivedEquality
                ));
                self.test_function_identity(type_identity_inputs, function, entry)
            })
            .collect();
        let function_identities = hir::HirFunctionIdentities::checked(
            hir::HirFunctionIdentityInputs {
                functions: &self.functions,
                lambdas: &Arena::new(),
                anonymous_functions: &Arena::new(),
                local_functions: &Arena::new(),
                property_getters: &self.property_getters,
                property_setters: &self.property_setters,
                property_accessor_identities: &property_accessor_identities,
                initialization_units: &Arena::new(),
                initialization_unit_identities: &initialization_unit_identities,
                derived_equality_applications: &Arena::new(),
                structs: &self.structs,
                enums: &self.enums,
                type_identities: &type_identities,
                struct_constructors: &self.struct_constructors,
                class_constructors: &self.class_constructors,
                constructor_identities: &constructor_identities,
                enum_member_identities: &enum_member_identities,
            },
            function_identity_rows,
        )
        .expect("the MIR test fixture functions have persistent identities");
        let hir::HirSourceFunctionIdentity::Plain(entry_identity) = function_identities[entry]
            .source_identity()
            .expect("the executable entry is a source function")
        else {
            panic!("the executable entry is non-generic")
        };
        let entry_source = scoop_identity::SourceIdentity::single_file();
        let entry_context = scoop_identity::SourceContextKey::File {
            source: entry_source.clone(),
        };
        let entry_span = self.functions[entry].span;
        let entry_origin = scoop_identity::DefinitionOrigin::new(
            entry_source,
            scoop_identity::SourceSpan::new(u64::from(entry_span.start), u64::from(entry_span.end))
                .unwrap(),
            &entry_context,
        )
        .unwrap();
        let export_definition_origins = hir::HirExportDefinitionOrigins::canonicalize(vec![
            scoop_identity::DefinitionOriginRecord::new(
                scoop_identity::DefinitionOriginSubject::Function(entry_identity.id()),
                entry_origin,
            ),
        ])
        .unwrap();
        let dispatch_key =
            |function: hir::FunctionId, interface: bool| match &function_identities[function] {
                hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(record)) => {
                    if interface {
                        scoop_identity::DispatchSlotKey::interface_method(record.id())
                    } else {
                        scoop_identity::DispatchSlotKey::virtual_method(record.id())
                    }
                }
                hir::HirFunctionIdentity::PropertyAccessor(
                    hir::HirPropertyAccessorFunction::Getter(getter),
                ) => scoop_identity::DispatchSlotKey::property_getter(
                    property_accessor_identities[*getter].id(),
                ),
                hir::HirFunctionIdentity::PropertyAccessor(
                    hir::HirPropertyAccessorFunction::Setter(setter),
                ) => scoop_identity::DispatchSlotKey::property_setter(
                    property_accessor_identities[*setter].id(),
                ),
                _ => panic!("test dispatch slot owner must be a plain function or accessor"),
            };
        let mut virtual_roots = std::collections::BTreeMap::new();
        for (function, declaration) in self.functions.iter() {
            let Some(method) = declaration.method else {
                continue;
            };
            let family = match method.dispatch {
                hir::MethodDispatch::Virtual(family)
                | hir::MethodDispatch::FinalOverride(family) => family,
                hir::MethodDispatch::Direct | hir::MethodDispatch::Interface(_) => continue,
            };
            virtual_roots.entry(family).or_insert(function);
        }
        let virtual_slots = virtual_roots
            .into_iter()
            .map(|(family, root)| {
                let record =
                    scoop_identity::CborIdentityRecord::from_key(dispatch_key(root, false))
                        .unwrap();
                (
                    family,
                    hir::HirVirtualDispatchSlotIdentity::Local { root, record },
                )
            })
            .collect();
        let interface_slots = self
            .interface_methods
            .iter()
            .map(|(_, member)| {
                scoop_identity::CborIdentityRecord::from_key(dispatch_key(member.function, true))
                    .unwrap()
            })
            .collect();
        let dispatch_slot_identities = hir::HirDispatchSlotIdentities::checked(
            hir::HirDispatchSlotIdentityInputs {
                functions: &self.functions,
                function_identities: &function_identities,
                property_accessor_identities: &property_accessor_identities,
                interface_methods: &self.interface_methods,
            },
            virtual_slots,
            interface_slots,
        )
        .expect("the MIR test fixture dispatch slots have persistent identities");
        let public_surface = hir::PublicSemanticSurface::default();
        let export_binding_identities = hir::HirExportBindingIdentities::from_public_surface(
            hir::HirExportBindingIdentityInputs {
                surface: &public_surface,
                structs: &self.structs,
                enums: &self.enums,
                classes: &self.classes,
                interfaces: &self.interfaces,
                objects: &Arena::new(),
                singleton_values: &Arena::new(),
                functions: &self.functions,
                properties: &self.properties,
                type_aliases: &Arena::new(),
                nominal_identities: &nominal_identities,
                enum_member_identities: &enum_member_identities,
                object_value_identities: &object_value_identities,
                function_identities: &function_identities,
                property_identities: &property_identities,
                type_alias_identities: &type_alias_identities,
            },
        )
        .expect("the empty MIR test public surface has no export bindings");
        let public_export_bindings =
            hir::CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap();
        let source_files = vec![hir::SourceFileMetadata {
            identity: scoop_identity::SourceIdentity::single_file(),
            provider: hir::IntrinsicProviderId::from_raw(0),
            name: "<test>".to_string(),
            source: String::new(),
            canonical_record: None,
        }];
        for (id, constructor) in self.class_constructors.iter() {
            let context = source_contexts.alloc(hir::SourceContext::new(
                scoop_identity::SourceIdentity::single_file(),
                hir::SourceContextSubject::Constructor(hir::SourceContextConstructor::Class(id)),
            ));
            assert_eq!(context, constructor.evaluation_context);
        }
        let source_context_identities =
            hir::HirSourceContextIdentities::from_contexts(hir::HirSourceContextIdentityInputs {
                source_files: &source_files,
                source_contexts: &source_contexts,
                structs: &self.structs,
                enums: &self.enums,
                classes: &self.classes,
                interfaces: &self.interfaces,
                objects: &Arena::new(),
                functions: &self.functions,
                struct_constructors: &self.struct_constructors,
                class_constructors: &self.class_constructors,
                properties: &self.properties,
                initialization_units: &Arena::new(),
                singleton_values: &Arena::new(),
                lambdas: &Arena::new(),
                anonymous_functions: &Arena::new(),
                nominal_identities: &nominal_identities,
                function_identities: &function_identities,
                property_accessor_identities: &property_accessor_identities,
                constructor_identities: &constructor_identities,
                property_identities: &property_identities,
                initialization_unit_identities: &initialization_unit_identities,
            })
            .expect("the MIR test fixture source contexts have persistent identities");
        let source_native_contracts =
            hir::HirSourceNativeContracts::from_declarations(hir::HirSourceNativeContractInputs {
                functions: &self.functions,
                extern_functions: &self.extern_functions,
                globals: &Arena::new(),
                properties: &self.properties,
                function_identities: &function_identities,
                property_identities: &property_identities,
                type_inputs: type_identity_inputs,
                unit: self.unit,
            })
            .expect("the MIR test fixture externs have source-native contracts");
        let callback_registration_identities =
            hir::HirCallbackRegistrationIdentities::from_registrations(
                hir::HirCallbackRegistrationIdentityInputs {
                    registrations: &Arena::new(),
                    functions: &self.functions,
                    lambdas: &Arena::new(),
                    anonymous_functions: &Arena::new(),
                    local_functions: &Arena::new(),
                    class_constructors: &self.class_constructors,
                    struct_constructors: &self.struct_constructors,
                    function_identities: &function_identities,
                    property_accessor_identities: &property_accessor_identities,
                    constructor_identities: &constructor_identities,
                    enum_member_identities: &enum_member_identities,
                    callback_modes,
                    type_inputs: type_identity_inputs,
                    unit: self.unit,
                },
            )
            .expect("the empty MIR test callback relation is valid");
        let source_parameter_interfaces = self.test_parameter_interfaces();
        let module = hir::Module {
            cone: scoop_identity::ConeIdentity::SINGLE_FILE,
            nominal_identities,
            property_identities,
            property_accessor_identities,
            type_alias_identities,
            enum_member_identities,
            field_identities,
            object_value_identities,
            initialization_unit_identities,
            type_identities,
            constructor_identities,
            function_identities,
            callback_registration_identities,
            export_binding_identities,
            public_export_bindings,
            local_binding_identities: hir::HirLocalBindingIdentities::default(),
            dispatch_slot_identities,
            source_context_identities,
            source_native_contracts,
            export_definition_origins,
            public_surface,
            source_files,
            source_contexts,
            types: self.types,
            imported_intrinsic_types: std::collections::BTreeMap::new(),
            function_types: self.function_types,
            lambdas: Arena::new(),
            anonymous_functions: Arena::new(),
            local_functions: Arena::new(),
            imported_dependency_callables: Arena::new(),
            imported_generic_templates: Arena::new(),
            imported_constructor_templates: Arena::new(),
            imported_constructor_applications: Arena::new(),
            imported_generic_applications: Arena::new(),
            callable_references: Arena::new(),
            bound_callable_refs: Arena::new(),
            function_coercions: Arena::new(),
            foreign_callback_registrations: Arena::new(),
            source_parameter_interfaces,
            export_default_exprs: Arena::new(),
            default_local_value_scopes: Arena::new(),
            export_default_sources: Arena::new(),
            export_vararg_parameter_types: Arena::new(),
            functions: self.functions,
            extern_functions: self.extern_functions,
            globals: Arena::new(),
            initialization_units: Arena::new(),
            initialization_failure_roots: Arena::new(),
            objects: Arena::new(),
            object_types: Arena::new(),
            companion_relations: Arena::new(),
            singleton_values: Arena::new(),
            singleton_published_roots: Arena::new(),
            properties: self.properties,
            extension_properties: Arena::new(),
            property_getters: self.property_getters,
            property_setters: self.property_setters,
            delegate_storages: Arena::new(),
            type_aliases: Arena::new(),
            generic_functions: self.generic_functions,
            method_applications: self.method_applications,
            generic_methods: self.generic_methods,
            generic_method_applications: self.generic_method_applications,
            derived_equality_applications: Arena::new(),
            structs: self.structs,
            struct_constructors: self.struct_constructors,
            struct_constructor_applications: self.struct_constructor_applications,
            struct_applications: self.struct_applications,
            enums: self.enums,
            enum_applications: self.enum_applications,
            classes: self.classes,
            class_fields: self.class_fields,
            class_constructors: self.class_constructors,
            class_constructor_applications: self.class_constructor_applications,
            class_applications: self.class_applications,
            interfaces: self.interfaces,
            interface_applications: self.interface_applications,
            interface_methods: self.interface_methods,
            top_level: self.top_level,
            unit: self.unit,
            boolean: self.boolean,
            string: self.string,
            core_protocols: hir::CoreProtocols::Defined(Box::new(hir::DefinedCoreProtocols {
                option: option_core,
                iteration: iteration_core,
                exceptions: exception_core,
                coroutines: coroutine_core,
                ffi: ffi_core,
                foreign_callbacks: hir::ForeignCallbackCore {
                    callback: ptr,
                    modes: callback_modes,
                    states: callback_states,
                    failure_result: callback_failure_result,
                    register: entry,
                    retain: entry,
                    release: entry,
                    query_state: entry,
                    failure: entry,
                },
                fundamental_types: intrinsic_type_core,
                source_location: hir::SourceLocationCore {
                    location: ptr,
                    current: entry,
                },
            })),
            instantiations: self.instantiations,
        };
        let local_entry = hir::LocalExecutableEntry::try_new(&module, entry)
            .expect("the MIR test harness builds a valid executable entry");
        hir::ExportHirOutput::try_new(
            module,
            hir::ConeOutputKind::Executable {
                local_entry: Box::new(local_entry),
            },
        )
        .expect("the MIR test harness builds a valid executable output")
    }
}
