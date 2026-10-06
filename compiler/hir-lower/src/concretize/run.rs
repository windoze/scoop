use super::*;

impl Concretizer<'_> {
    pub(super) fn run(self) -> Result<concrete::Module, Vec<scoop_ast::Diagnostic>> {
        self.run_with(|_| ()).map(|(module, ())| module)
    }

    fn run_with<Extra>(
        mut self,
        finish: impl FnOnce(&mut Self) -> Extra,
    ) -> Result<(concrete::Module, Extra), Vec<scoop_ast::Diagnostic>> {
        let prepared = self.prepare_source();
        let roots = self.shared_result_types();
        self.coroutine_results
            .extend(self.owned_result_types(&roots));
        self.finish(prepared, finish)
    }

    pub(super) fn finish<Extra>(
        mut self,
        prepared: prepare::PreparedSource,
        finish: impl FnOnce(&mut Self) -> Extra,
    ) -> Result<(concrete::Module, Extra), Vec<scoop_ast::Diagnostic>> {
        let prepare::PreparedSource {
            unit,
            boolean,
            string,
            native_callback_signatures,
            core_protocols,
        } = prepared;
        let coroutine_protocols = self.build_coroutine_protocols();
        if !self.initialization_requests.is_empty()
            || self.source.cone
                == scoop_identity::CoreBuiltinNominal::Any
                    .declaration_key()
                    .origin()
        {
            self.intern_type(concrete::TypeKind::Any, false);
        }
        let extra = finish(&mut self);
        if !self.type_condition_errors.is_empty() {
            return Err(self.type_condition_errors);
        }
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
        self.finish_initialization_units(&exact_type_identities);
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
        let release_hooks = release::finish_release_hooks(
            self.release_hook_slots,
            &self.classes,
            &exact_type_identities,
        );
        let local_value_identities =
            concrete::LocalValueIdentities::from_callables(concrete::LocalValueIdentityInputs {
                source_files: &self.source.source_files,
                source_contexts: &self.source.source_context_identities,
                callable_applications: &identities.callable_applications,
                functions: &functions,
                lambdas: &self.lambdas,
                anonymous_functions: &self.anonymous_functions,
                lexical_local_values: &identities.lexical_local_values,
                callable_references: &callable_references,
                class_constructors: &class_constructors,
                struct_constructors: &struct_constructors,
                release_hooks: &release_hooks,
            })
            .expect("validated concretization produces a total local-value identity relation");

        let module = concrete::Module {
            cone: self.source.cone,
            types: self.types,
            tuple_interface_implementations: self.tuple_interface_implementations,
            exact_type_identities,
            local_value_identities,
            dispatch_slot_identities,
            callable_applications: identities.callable_applications,
            generated_callable_identities: identities.generated_callable_identities,
            callback_applications: identities.callback_applications,
            function_types: self.function_types,
            native_callback_signatures,
            coroutine_protocols,
            lambdas: self.lambdas,
            anonymous_functions: self.anonymous_functions,
            local_functions: self.local_functions,
            callable_references,
            imported_derived_equalities: self.imported_derived_equalities,
            imported_dependency_callables: self.imported_dependency_callables,
            function_coercions: self.function_coercions,
            foreign_callback_registrations,
            functions,
            extern_functions: self.extern_functions,
            globals: self.globals,
            generic_delegate_specializations: self.generic_delegate_specializations,
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
            release_hooks,
            class_constructors,
            struct_constructors,
            interfaces: self.interfaces,
            top_level: self.emitted_functions,
            unit,
            boolean,
            string,
            core_protocols,
        };
        Ok((module, extra))
    }

    pub(super) fn drain_pending_callables(&mut self) {
        loop {
            if let Some(unit) = self.pending_initializations.pop_front() {
                self.require_initialization_dependencies(unit);
            } else if let Some((key, id, site)) = self.pending_functions.pop_front() {
                let previous = self.instantiation_site;
                let previous_use = self.type_use_site;
                self.instantiation_site = site;
                self.type_use_site = site;
                let function = self.lower_function(&key);
                self.instantiation_site = previous;
                self.type_use_site = previous_use;
                let slot = id.into_raw().into_u32() as usize;
                assert!(self.function_slots[slot].replace(function).is_none());
            } else if let Some((constructor, site)) = self.pending_constructors.pop_front() {
                let previous = self.instantiation_site;
                let previous_use = self.type_use_site;
                self.instantiation_site = site;
                self.type_use_site = site;
                self.lower_pending_constructor(constructor);
                self.instantiation_site = previous;
                self.type_use_site = previous_use;
            } else {
                break;
            }
        }
    }
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
