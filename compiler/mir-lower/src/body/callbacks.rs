use super::*;

mod adapter;
mod protocol;

impl BodyLowerer<'_> {
    pub(super) fn ensure_foreign_callback_family(
        &mut self,
        callback: mir::StructId,
    ) -> mir::ForeignCallbackFamilyId {
        if let Some(&family) = self.foreign_callback_family_by_callback.get(&callback) {
            return family;
        }

        let types = Types {
            module: self.module,
            struct_map: self.struct_map,
            class_map: self.class_map,
        };
        let core = self.callback_protocol();
        for enumeration in [
            core.modes.enumeration(),
            core.states.enumeration(),
            core.failure_result.enumeration(),
        ] {
            let lowered = types.lower(
                self.module.enums[enumeration].canonical_type,
                self.source_exact_types,
                self.enums,
                self.structs,
                self.interfaces,
                self.shell,
            );
            assert!(
                matches!(lowered, mir::Type::Enum(id, _) if self.enums.hir_ids[&id] == enumeration),
                "the canonical callback protocol type retains its physical enum"
            );
        }
        let reusable = self.enums.lower_variant_ref(core.modes.reusable());
        let one_shot = self.enums.lower_variant_ref(core.modes.one_shot());
        let modes = mir::ForeignCallbackModes::checked(&self.enums.defs, reusable, one_shot)
            .expect("the concrete callback mode protocol maps to checked MIR refs");
        let registered = self.enums.lower_variant_ref(core.states.registered());
        let active = self.enums.lower_variant_ref(core.states.active());
        let completed = self.enums.lower_variant_ref(core.states.completed());
        let failed = self.enums.lower_variant_ref(core.states.failed());
        let states = mir::ForeignCallbackStates::checked(
            &self.enums.defs,
            registered,
            active,
            completed,
            failed,
        )
        .expect("the concrete callback state protocol maps to checked MIR refs");
        let failure_some = self
            .enums
            .lower_variant_field_ref(core.failure_result.some_payload());
        let failure_none = self.enums.lower_variant_ref(core.failure_result.none());
        let failure_option = mir::OptionCore::checked(&self.enums.defs, failure_some, failure_none)
            .expect("the concrete callback failure protocol maps to checked MIR refs");
        let failure_result = mir::ForeignCallbackFailureResult::checked(
            &self.enums.defs,
            failure_option,
            self.class_map[&core.failure_result.throwable()],
        )
        .expect("callback failure remains the exact MIR Option<Throwable> specialization");
        let family = self
            .foreign_callback_families
            .alloc(mir::ForeignCallbackFamily {
                callback,
                modes,
                states,
                failure_result,
            });
        self.foreign_callback_family_by_callback
            .insert(callback, family);
        family
    }

    pub(super) fn ensure_foreign_callback_bridge(
        &mut self,
        registration_id: hir::ForeignCallbackRegistrationId,
        span: Span,
    ) -> mir::ForeignCallbackBridgeId {
        let registration = self.module.foreign_callback_registrations[registration_id].clone();
        if let Some(&bridge) = self
            .foreign_callback_by_application
            .get(&registration.application)
        {
            return bridge;
        }
        let span = source_span(span);
        let native_signature = self.lower_function_type_id(registration.native_function_type);
        let managed_signature = self.lower_function_type_id(registration.managed_function_type);
        let exact_managed_signature =
            exact_callback_signature(self.module, registration.managed_function_type);
        let callback = self.struct_map[&registration.callback];
        let family = self.ensure_foreign_callback_family(callback);
        let callback_mode = registration.mode;
        let modes = self.foreign_callback_families[family].modes;
        let mode = match callback_mode {
            hir::CallbackMode::Reusable => modes.reusable(),
            hir::CallbackMode::OneShot => modes.one_shot(),
        };
        let signature = self.shell.function_types[managed_signature].clone();
        debug_assert!(!signature.is_suspend);

        let adapter_index = self.foreign_callback_adapters.len();
        let throwable = mir::Type::Class(
            self.foreign_callback_families[family]
                .failure_result
                .throwable(),
        );
        let function = self.functions.alloc(adapter::build(
            crate::context::core_provider(self.module),
            managed_signature,
            &signature,
            throwable,
            adapter_index,
            span,
        ));
        self.top_level.push(function);
        let generated = hir::PersistentGeneratedCallableId::from_key(
            &hir::GeneratedCallableKey::ForeignCallbackManagedAdapter {
                application: registration.application,
            },
        )
        .expect("a callback adapter generated-callable identity is hashable");
        let odr_member =
            callback_adapter_odr_member(self.module, registration.application, generated);
        let adapter = self.foreign_callback_adapters.alloc(
            mir::ForeignCallbackAdapter::checked(
                function,
                managed_signature,
                &signature,
                registration.application,
                &exact_managed_signature,
                odr_member,
            )
            .expect("a callback adapter generated-callable identity is hashable"),
        );
        let application_identity = self
            .module
            .callback_applications
            .get(registration.application)
            .expect("a concrete callback registration has a persistent application record")
            .clone();
        let application_record = mir::CallbackApplicationRecord::new(
            registration.application,
            self.foreign_callback_adapters[adapter].signature_subject(),
            exact_managed_signature,
            mir::ForeignCallbackStorageAbi::ClosureContextResultRootsThrowableToStatus,
            callback_mode,
        );
        let bridge = self
            .foreign_callback_bridges
            .alloc(mir::ForeignCallbackBridge {
                application_identity,
                application_record,
                adapter,
                family,
                native_signature,
                context_index: registration.context_index,
                mode,
            });
        self.foreign_callback_by_application
            .insert(registration.application, bridge);
        bridge
    }
}

fn callback_adapter_odr_member(
    module: &hir::Module,
    application: hir::PersistentCallbackApplicationId,
    generated: hir::PersistentGeneratedCallableId,
) -> Option<hir::OdrMemberRecord> {
    let application = module
        .callback_applications
        .get(application)
        .expect("a concrete callback registration has a persistent application record");
    let group = materialization_context_odr_group(module, application.key().context())?;
    let key = hir::OdrMemberKey::new(
        group,
        hir::OdrMemberRole::CallableBody,
        hir::OdrMemberDiscriminator::GeneratedCallable(generated),
    )
    .expect("a callback adapter is a callable ODR member");
    Some(
        hir::CborIdentityRecord::from_key(key)
            .expect("a callback adapter ODR member identity is hashable"),
    )
}

pub(super) fn materialization_context_odr_group(
    module: &hir::Module,
    context: hir::CallableMaterializationContext,
) -> Option<hir::OdrGroupId> {
    match context {
        hir::CallableMaterializationContext::NoSubstitution => None,
        hir::CallableMaterializationContext::Application(application) => Some(
            module
                .callable_applications
                .odr(application)
                .expect("a materialization references a concrete callable application")
                .group(),
        ),
        hir::CallableMaterializationContext::InitializationApplication(unit) => {
            Some(crate::initialization_odr_group(module, unit))
        }
    }
}

pub(super) fn exact_callback_signature(
    module: &hir::Module,
    signature: hir::FunctionTypeId,
) -> hir::ExactCallableSignature {
    let signature = &module.function_types[signature];
    assert!(!signature.is_suspend, "a managed callback cannot suspend");
    hir::ExactCallableSignature::new(
        hir::Effect::Ordinary,
        None,
        signature
            .parameter_types
            .iter()
            .map(|parameter| module.exact_type_identities[*parameter].id())
            .collect(),
        module.exact_type_identities[signature.return_type].id(),
    )
}
