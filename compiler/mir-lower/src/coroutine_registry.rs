use super::*;

mod signatures;
mod start;
pub(crate) use signatures::lowered_signature;

pub(super) fn protocol_call(
    module: &hir::Module,
    instances: &InstanceRegistry,
    interfaces: &InterfaceRegistry,
    function: hir::FunctionId,
) -> mir::CallTarget {
    let method = module.functions[function]
        .receiver
        .method()
        .expect("a coroutine protocol callable has an interface receiver");
    let hir::MethodDispatch::Interface { interface, slot } = method.dispatch else {
        unreachable!("a checked coroutine protocol member uses interface dispatch")
    };
    mir::CallTarget {
        kind: mir::CallKind::Interface {
            interface: interfaces.mir_id(interface),
            slot: slot.into_raw(),
        },
        callee: mir::Callee::Monomorphized(
            instances
                .get(function)
                .expect("the protocol member was materialized"),
        ),
    }
}

pub(super) fn throwable_type(
    module: &hir::Module,
    classes: &HashMap<hir::ClassId, mir::ClassId>,
) -> mir::Type {
    let protocol = module
        .coroutine_protocols
        .first()
        .expect("coroutine lowering has a concrete protocol");
    let [_, exception] = module.functions[protocol.continuation_resume_with_exception]
        .params
        .as_slice()
    else {
        unreachable!("the checked failure protocol has receiver and exception parameters")
    };
    let hir::TypeKind::Class(class) = module.types[exception.ty].kind else {
        unreachable!("the failure protocol receives Throwable")
    };
    mir::Type::Class(classes[&class])
}

fn next_index<T>(values: &[T]) -> u32 {
    u32::try_from(values.len()).expect("compiler-synthesized enum arity fits in u32")
}

#[derive(Clone)]
pub(super) struct SuspendSource {
    pub(super) function: mir::FunctionId,
    pub(super) materialization: hir::CallableMaterialization,
    pub(super) odr_group: Option<hir::OdrGroupId>,
    pub(super) logical_signature: hir::ExactCallableSignature,
    pub(super) source_return: mir::Type,
}

#[derive(Default)]
pub(super) struct CoroutineRegistry {
    pub(super) functions: Arena<mir::CoroutineFunction>,
    pub(super) steps: Arena<mir::CoroutineStep>,
    pub(super) steps_by_result: Vec<(hir::PersistentExactTypeId, mir::CoroutineStepId)>,
    pub(super) slots: Arena<mir::CoroutineSlot>,
    pub(super) slots_by_value: Vec<(hir::PersistentExactTypeId, mir::CoroutineSlotId)>,
    pub(super) saved_values: Arena<mir::CoroutineSavedValue>,
    pub(super) failure_values: Arena<mir::CoroutineFailureValue>,
    pub(super) frames: Arena<mir::CoroutineFrame>,
    pub(super) resume_points: Arena<mir::CoroutineResumePoint>,
    pub(super) start_helpers: Vec<mir::CoroutineStart>,
    /// Transient structured call order used only while identifying suspend sites.
    pub(super) pre_coroutine_call_sites: HashMap<mir::FunctionId, Vec<cfg::CallSite>>,
}

impl Lowerer {
    pub(super) fn coroutine_slot_for(
        &mut self,
        value: &mir::Type,
    ) -> (mir::CoroutineSlotId, mir::Type) {
        let identity = if let mir::Type::Context(storage) = value {
            assert_eq!(storage.role, mir::ContextStorageRole::Mark);
            mir::CoroutineSlotIdentity::context_mark(storage.core)
        } else if let Some(source) = self.source_exact_types.get(value) {
            mir::CoroutineSlotIdentity::new(
                source.identity_record(),
                source.nominal_specialization(),
            )
        } else {
            let boxed = self
                .boxed
                .entries
                .iter()
                .find(|boxed| value == &mir::Type::Class(boxed.class))
                .expect("a generated saved receiver has its concrete box identity");
            let payload = self
                .source_exact_types
                .get(&boxed.payload)
                .expect("a box retains its source payload identity");
            mir::CoroutineSlotIdentity::boxed_value(
                payload.identity_record(),
                payload.nominal_specialization(),
            )
        }
        .expect("a saved value has one exact coroutine-slot identity");
        self.coroutines.slot_for(
            identity,
            value,
            &self.structs,
            &mut self.enums,
            &mut self.shell,
        )
    }
}

impl CoroutineRegistry {
    pub(super) fn record_call_sites(
        &mut self,
        function: mir::FunctionId,
        sites: Vec<cfg::CallSite>,
    ) {
        assert!(
            self.pre_coroutine_call_sites
                .insert(function, sites)
                .is_none(),
            "a MIR callable has one pre-coroutine structured-call order"
        );
    }

    pub(super) fn call_sites(&self, function: mir::FunctionId) -> &[cfg::CallSite] {
        self.pre_coroutine_call_sites
            .get(&function)
            .expect("a coroutine source retains its structured-call order")
    }

    fn source_type<'a>(
        exact_types: &'a SourceExactTypeRegistry,
        lowered: &mir::Type,
    ) -> &'a mir::SourceExactTypeIdentity {
        exact_types.get(lowered).unwrap_or_else(|| {
            panic!("coroutine value type {lowered:?} has no local-concrete HIR identity")
        })
    }

    pub(super) fn step_for(
        &mut self,
        exact_types: &SourceExactTypeRegistry,
        result: &mir::Type,
        structs: &StructRegistry,
        enums: &mut EnumRegistry,
        shell: &mut mir::Module,
    ) -> (mir::CoroutineStepId, mir::Type) {
        let source = Self::source_type(exact_types, result);
        let exact = source.identity_record();
        let nominal_group = source.nominal_specialization();
        if let Some((_, id)) = self
            .steps_by_result
            .iter()
            .find(|(found, _)| *found == exact.id())
        {
            let step = &self.steps[*id];
            return (*id, mir::Type::Enum(step.enum_id(), Vec::new()));
        }
        let identity = mir::CoroutineStepIdentity::new(exact, nominal_group)
            .expect("local-concrete exact types have one coroutine-step root");
        let name = format!("CoroutineStep<{}>", mir::type_name(shell, result));
        let result_gc_free = mir_type_gc_free(result, structs, enums);
        let mut variants = Vec::new();
        let completed_index = next_index(&variants);
        let mut completed_fields = Vec::new();
        let completed_payload_index = next_index(&completed_fields);
        completed_fields.push(mir::VariantField {
            identity: identity.completed_payload_record().id(),
            name: "value".to_string(),
            ty: result.clone(),
        });
        variants.push(mir::VariantDef {
            identity: identity.completed_variant_record().id(),
            name: "Completed".to_string(),
            gc_free: result_gc_free,
            fields: completed_fields,
        });
        let suspended_index = next_index(&variants);
        variants.push(mir::VariantDef {
            identity: identity.suspended_variant_record().id(),
            name: "Suspended".to_string(),
            gc_free: true,
            fields: Vec::new(),
        });
        let enum_id = enums.defs.alloc(mir::EnumDef {
            name: name.clone(),
            type_arguments: Vec::new(),
            gc_free: result_gc_free,
            variants,
        });
        let shell_id = shell.enums.alloc(mir::EnumDef {
            name,
            type_arguments: Vec::new(),
            gc_free: result_gc_free,
            variants: Vec::new(),
        });
        assert_eq!(enum_id, shell_id, "the type context mirrors enum ids");
        let completed = enums.variant_ref(enum_id, completed_index);
        let completed_payload = enums.variant_field_ref(completed, completed_payload_index);
        let suspended = enums.variant_ref(enum_id, suspended_index);
        let step = mir::CoroutineStep::checked(
            &enums.defs,
            completed_payload,
            suspended,
            result.clone(),
            identity,
        )
        .expect("synthesized CoroutineStep metadata matches its enum definition");
        let id = self.steps.alloc(step);
        self.steps_by_result.push((exact.id(), id));
        (id, mir::Type::Enum(enum_id, Vec::new()))
    }

    pub(super) fn step_type_for(
        &self,
        exact_types: &SourceExactTypeRegistry,
        result: &mir::Type,
    ) -> Option<mir::Type> {
        let exact = Self::source_type(exact_types, result)
            .identity_record()
            .id();
        self.steps_by_result
            .iter()
            .find(|(found, _)| *found == exact)
            .map(|(_, id)| mir::Type::Enum(self.steps[*id].enum_id(), Vec::new()))
    }

    pub(super) fn step_metadata_for_type(&self, step_ty: &mir::Type) -> &mir::CoroutineStep {
        let mir::Type::Enum(enum_id, arguments) = step_ty else {
            unreachable!("CoroutineStep has an enum type")
        };
        assert!(arguments.is_empty(), "CoroutineStep is already concrete");
        self.steps
            .iter()
            .find_map(|(_, step)| (step.enum_id() == *enum_id).then_some(step))
            .expect("every synthesized CoroutineStep type has typed metadata")
    }

    fn slot_for(
        &mut self,
        identity: mir::CoroutineSlotIdentity,
        value: &mir::Type,
        structs: &StructRegistry,
        enums: &mut EnumRegistry,
        shell: &mut mir::Module,
    ) -> (mir::CoroutineSlotId, mir::Type) {
        let exact = identity.value_record().id();
        if let Some((_, id)) = self
            .slots_by_value
            .iter()
            .find(|(found, _)| *found == exact)
        {
            let slot = &self.slots[*id];
            return (*id, mir::Type::Enum(slot.enum_id(), Vec::new()));
        }
        let name = format!("CoroutineSlot<{}>", mir::type_name(shell, value));
        let value_gc_free = mir_type_gc_free(value, structs, enums);
        let mut variants = Vec::new();
        let empty_index = next_index(&variants);
        variants.push(mir::VariantDef {
            identity: identity.empty_variant_record().id(),
            name: "Empty".to_string(),
            gc_free: true,
            fields: Vec::new(),
        });
        let value_index = next_index(&variants);
        let mut value_fields = Vec::new();
        let value_payload_index = next_index(&value_fields);
        value_fields.push(mir::VariantField {
            identity: identity.value_payload_record().id(),
            name: "value".to_string(),
            ty: value.clone(),
        });
        variants.push(mir::VariantDef {
            identity: identity.value_variant_record().id(),
            name: "Value".to_string(),
            gc_free: value_gc_free,
            fields: value_fields,
        });
        let enum_id = enums.defs.alloc(mir::EnumDef {
            name: name.clone(),
            type_arguments: Vec::new(),
            gc_free: value_gc_free,
            variants,
        });
        let shell_id = shell.enums.alloc(mir::EnumDef {
            name,
            type_arguments: Vec::new(),
            gc_free: value_gc_free,
            variants: Vec::new(),
        });
        assert_eq!(enum_id, shell_id, "the type context mirrors enum ids");
        let empty = enums.variant_ref(enum_id, empty_index);
        let value_variant = enums.variant_ref(enum_id, value_index);
        let value_payload = enums.variant_field_ref(value_variant, value_payload_index);
        let slot =
            mir::CoroutineSlot::checked(&enums.defs, value_payload, empty, value.clone(), identity)
                .expect("synthesized CoroutineSlot metadata matches its enum definition");
        let id = self.slots.alloc(slot);
        self.slots_by_value.push((exact, id));
        (id, mir::Type::Enum(enum_id, Vec::new()))
    }
}
