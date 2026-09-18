use super::*;

impl Lowerer {
    /// Seals a successful frontend graph into persistent identities and the
    /// structurally complete Export HIR module.
    pub(super) fn finish(
        self,
        current_cone: scoop_identity::ConeIdentity,
        warnings: Vec<Diagnostic>,
        core_protocols: hir::CoreProtocols,
        completion: CoreLoweringCompletion,
    ) -> Result<(hir::Module, Vec<Diagnostic>, CoreLoweringCompletion), Vec<Diagnostic>> {
        let core_types = match &core_protocols {
            hir::CoreProtocols::Defined(protocols) => {
                hir::HirCoreTypeIdentityAuthority::Defined(&protocols.fundamental_types)
            }
            hir::CoreProtocols::Imported(protocols) => {
                hir::HirCoreTypeIdentityAuthority::Imported(protocols.fundamental_types())
            }
        };
        let public_surface = self.public_semantic_surface();
        let nominal_identities = match crate::persistent_nominals::build(&self) {
            Ok(identities) => identities,
            Err(error) => {
                let mut diagnostic = Diagnostic::at(error.span(), error.to_string());
                diagnostic.file = error.file();
                return Err(vec![diagnostic]);
            }
        };
        let type_identities = match crate::persistent_type_identities::build(
            &self,
            &nominal_identities,
            core_types,
        ) {
            Ok(identities) => identities,
            Err(error) => {
                let mut diagnostic = Diagnostic::at(error.span(), error.to_string());
                diagnostic.file = error.file();
                return Err(vec![diagnostic]);
            }
        };
        let constructor_identities = match crate::persistent_constructor_identities::build(
            &self,
            &nominal_identities,
            core_types,
        ) {
            Ok(identities) => identities,
            Err(error) => {
                let mut diagnostic = Diagnostic::at(error.span(), error.to_string());
                diagnostic.file = error.file();
                return Err(vec![diagnostic]);
            }
        };
        let property_identities =
            match crate::persistent_properties::build(&self, &nominal_identities, core_types) {
                Ok(identities) => identities,
                Err(error) => {
                    let mut diagnostic = Diagnostic::at(error.span(), error.to_string());
                    diagnostic.file = error.file();
                    return Err(vec![diagnostic]);
                }
            };
        let property_accessor_identities =
            match crate::persistent_accessors::build(&self, &property_identities) {
                Ok(identities) => identities,
                Err(error) => {
                    let mut diagnostic = Diagnostic::at(error.span(), error.to_string());
                    diagnostic.file = error.file();
                    return Err(vec![diagnostic]);
                }
            };
        let type_alias_identities = match crate::persistent_aliases::build(&self) {
            Ok(identities) => identities,
            Err(error) => {
                let mut diagnostic = Diagnostic::at(error.span(), error.to_string());
                diagnostic.file = error.file();
                return Err(vec![diagnostic]);
            }
        };
        let enum_member_identities =
            match crate::persistent_enum_members::build(&self, &nominal_identities) {
                Ok(identities) => identities,
                Err(error) => {
                    let mut diagnostic = Diagnostic::at(error.span(), error.to_string());
                    diagnostic.file = error.file();
                    return Err(vec![diagnostic]);
                }
            };
        let field_identities =
            match crate::persistent_fields::build(&self, &nominal_identities, &property_identities)
            {
                Ok(identities) => identities,
                Err(error) => {
                    let mut diagnostic = Diagnostic::at(error.span(), error.to_string());
                    diagnostic.file = error.file();
                    return Err(vec![diagnostic]);
                }
            };
        let object_value_identities =
            match crate::persistent_object_values::build(&self, &nominal_identities) {
                Ok(identities) => identities,
                Err(error) => {
                    let mut diagnostic = Diagnostic::at(error.span(), error.to_string());
                    diagnostic.file = error.file();
                    return Err(vec![diagnostic]);
                }
            };
        let initialization_unit_identities = match crate::persistent_initialization_units::build(
            &self,
            &nominal_identities,
            &property_identities,
        ) {
            Ok(identities) => identities,
            Err(error) => {
                let mut diagnostic = Diagnostic::at(error.span(), error.to_string());
                diagnostic.file = error.file();
                return Err(vec![diagnostic]);
            }
        };
        let function_identities = match crate::persistent_functions::build(
            &self,
            &nominal_identities,
            &property_identities,
            &property_accessor_identities,
            &initialization_unit_identities,
            &type_identities,
            &constructor_identities,
            &enum_member_identities,
            core_types,
        ) {
            Ok(identities) => identities,
            Err(error) => {
                let mut diagnostic = Diagnostic::at(error.span(), error.to_string());
                diagnostic.file = error.file();
                return Err(vec![diagnostic]);
            }
        };
        let callback_registration_identities = match &core_protocols {
            hir::CoreProtocols::Defined(protocols) => {
                match crate::persistent_callbacks::build(
                    &self,
                    protocols.foreign_callbacks,
                    &nominal_identities,
                    &property_accessor_identities,
                    &constructor_identities,
                    &enum_member_identities,
                    &function_identities,
                    core_types,
                ) {
                    Ok(identities) => identities,
                    Err(error) => {
                        let (file, span) = error.registration().map_or(
                            (0, Span { start: 0, end: 0 }),
                            |registration| {
                                let registration =
                                    &self.foreign_callback_registrations[registration];
                                let file = match registration.definition_root {
                                    hir::LexicalDefinitionRoot::Function(function) => {
                                        self.function_files[&function]
                                    }
                                    hir::LexicalDefinitionRoot::ClassConstructor(constructor) => {
                                        self.class_files
                                            [&self.class_constructors[constructor].owner]
                                    }
                                    hir::LexicalDefinitionRoot::StructConstructor(constructor) => {
                                        self.struct_files
                                            [&self.struct_constructors[constructor].owner]
                                    }
                                    hir::LexicalDefinitionRoot::VariantConstructor(variant) => {
                                        self.enum_files[&variant.enumeration()]
                                    }
                                };
                                (file, registration.span)
                            },
                        );
                        let mut diagnostic = Diagnostic::at(span, error.to_string());
                        diagnostic.file = file;
                        return Err(vec![diagnostic]);
                    }
                }
            }
            hir::CoreProtocols::Imported(_) => {
                match hir::HirCallbackRegistrationIdentities::for_imported_core(
                    &self.foreign_callback_registrations,
                ) {
                    Ok(identities) => identities,
                    Err(error) => {
                        let registration =
                            &self.foreign_callback_registrations[error.registration()];
                        return Err(vec![Diagnostic::at(registration.span, error.to_string())]);
                    }
                }
            }
        };
        let public_bindings = match crate::persistent_export_bindings::build(
            &self,
            &public_surface,
            &nominal_identities,
            &object_value_identities,
            &function_identities,
            &property_identities,
            &type_alias_identities,
        ) {
            Ok(bindings) => bindings,
            Err(error) => {
                let mut diagnostic = Diagnostic::at(error.span(), error.to_string());
                diagnostic.file = error.file();
                return Err(vec![diagnostic]);
            }
        };
        let export_binding_identities = public_bindings.identities;
        let public_export_bindings = public_bindings.surface;
        let dispatch_slot_identities = match crate::persistent_dispatch::build(
            &self,
            &function_identities,
            &property_accessor_identities,
        ) {
            Ok(identities) => identities,
            Err(error) => {
                let mut diagnostic = Diagnostic::at(error.span(), error.to_string());
                diagnostic.file = error.file();
                return Err(vec![diagnostic]);
            }
        };
        let source_native_contracts = match crate::persistent_native_contracts::build(
            &self,
            &nominal_identities,
            &function_identities,
            &property_identities,
            core_types,
        ) {
            Ok(contracts) => contracts,
            Err(error) => {
                let (file, span) = match error.owner() {
                    Some(hir::HirSourceNativeContractOwner::Function(function)) => (
                        self.function_files[&function],
                        self.functions[function].span,
                    ),
                    Some(hir::HirSourceNativeContractOwner::Global(global)) => {
                        let declaration = &self.globals[global];
                        (self.property_files[&declaration.property], declaration.span)
                    }
                    None => (0, Span { start: 0, end: 0 }),
                };
                let mut diagnostic = Diagnostic::at(span, error.to_string());
                diagnostic.file = file;
                return Err(vec![diagnostic]);
            }
        };
        let source_files = self
            .intrinsic_sources
            .iter()
            .map(|source| hir::SourceFileMetadata {
                provider: source.provider,
                identity: source.identity.clone(),
                name: source.name.clone(),
                source: source.source.clone(),
            })
            .collect::<Vec<_>>();
        let source_context_identities = match crate::persistent_source_contexts::build(
            &self,
            &source_files,
            &nominal_identities,
            &function_identities,
            &property_accessor_identities,
            &constructor_identities,
            &property_identities,
            &initialization_unit_identities,
        ) {
            Ok(identities) => identities,
            Err(error) => {
                return Err(vec![Diagnostic::at(
                    Span { start: 0, end: 0 },
                    error.to_string(),
                )]);
            }
        };
        let local_binding_identities = match crate::persistent_local_bindings::build(
            &self,
            &nominal_identities,
            &object_value_identities,
            &function_identities,
            &property_identities,
            &type_alias_identities,
            &enum_member_identities,
            &source_context_identities,
        ) {
            Ok(identities) => identities,
            Err(error) => {
                let mut diagnostic = Diagnostic::at(error.span(), error.to_string());
                diagnostic.file = error.file();
                return Err(vec![diagnostic]);
            }
        };
        let export_definition_origins = match crate::persistent_definition_origins::build(
            &self,
            &nominal_identities,
            &property_identities,
            &property_accessor_identities,
            &type_alias_identities,
            &enum_member_identities,
            &field_identities,
            &initialization_unit_identities,
            &constructor_identities,
            &function_identities,
            &callback_registration_identities,
            &local_binding_identities,
            &source_native_contracts,
            &source_context_identities,
        ) {
            Ok(origins) => origins,
            Err(error) => {
                let mut diagnostic = Diagnostic::at(error.span(), error.to_string());
                diagnostic.file = error.file();
                return Err(vec![diagnostic]);
            }
        };
        let module = hir::Module {
            cone: current_cone,
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
            local_binding_identities,
            dispatch_slot_identities,
            source_context_identities,
            source_native_contracts,
            export_definition_origins,
            public_surface,
            source_files,
            source_contexts: self.source_contexts,
            types: self.types,
            function_types: self.function_types,
            lambdas: self.lambdas,
            anonymous_functions: self.anonymous_functions,
            local_functions: self.local_functions,
            callable_references: self.callable_references,
            imported_core_callables: self.imported_core_callables,
            imported_core_types: self.imported_core_types,
            imported_core_values: self.imported_core_values,
            bound_callable_refs: self.bound_callable_refs,
            function_coercions: self.function_coercions,
            foreign_callback_registrations: self.foreign_callback_registrations,
            source_parameter_interfaces: self.source_parameter_interfaces,
            export_default_exprs: self.export_default_exprs,
            export_default_sources: self.export_default_sources,
            export_vararg_parameter_types: self.export_vararg_parameter_types,
            functions: self.functions,
            extern_functions: self.extern_functions,
            globals: self.globals,
            initialization_units: self.initialization_units,
            initialization_failure_roots: self.initialization_failure_roots,
            objects: self.objects,
            object_types: self.object_types,
            companion_relations: self.companion_relations,
            singleton_values: self.singleton_values,
            singleton_published_roots: self.singleton_published_roots,
            properties: self.properties,
            extension_properties: self.extension_properties,
            property_getters: self.property_getters,
            property_setters: self.property_setters,
            delegate_storages: self.delegate_storages,
            type_aliases: self.type_aliases,
            generic_functions: self.generic_functions,
            method_applications: self.method_applications,
            generic_methods: self.generic_methods,
            generic_method_applications: self.generic_method_applications,
            derived_equality_applications: self.derived_equality_applications,
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
            interface_methods: self.interface_method_entities,
            top_level: self.top_level,
            unit: self.unit,
            boolean: self.boolean,
            string: self.string,
            core_protocols,
            instantiations: self.instantiations,
        };
        Ok((module, warnings, completion))
    }
}
