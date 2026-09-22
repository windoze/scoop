use super::*;

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
    pub(super) continuation_shells: Vec<mir::CoroutineContinuationShell>,
    pub(super) start_helpers: Vec<mir::CoroutineStart>,
    /// Transient structured call order used only while identifying suspend sites.
    pub(super) pre_coroutine_call_sites: HashMap<mir::FunctionId, Vec<cfg::CallSite>>,
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
        exact_types
            .get(lowered)
            .expect("coroutine value types originate in local-concrete HIR")
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

    pub(super) fn slot_for(
        &mut self,
        exact_types: &SourceExactTypeRegistry,
        value: &mir::Type,
        structs: &StructRegistry,
        enums: &mut EnumRegistry,
        shell: &mut mir::Module,
    ) -> (mir::CoroutineSlotId, mir::Type) {
        let source = Self::source_type(exact_types, value);
        let exact = source.identity_record();
        let nominal_group = source.nominal_specialization();
        if let Some((_, id)) = self
            .slots_by_value
            .iter()
            .find(|(found, _)| *found == exact.id())
        {
            let slot = &self.slots[*id];
            return (*id, mir::Type::Enum(slot.enum_id(), Vec::new()));
        }
        let identity = mir::CoroutineSlotIdentity::new(exact, nominal_group)
            .expect("local-concrete exact types have one coroutine-slot root");
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
        self.slots_by_value.push((exact.id(), id));
        (id, mir::Type::Enum(enum_id, Vec::new()))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn continuation_shells(
        &mut self,
        exact_types: &SourceExactTypeRegistry,
        result: &mir::Type,
        success_signature: hir::ExactCallableSignature,
        failure_signature: hir::ExactCallableSignature,
        continuation: mir::InterfaceId,
        throwable: mir::Type,
        functions: &mut Arena<mir::Function>,
        shell: &mir::Module,
    ) -> (mir::FunctionId, mir::FunctionId) {
        let source = Self::source_type(exact_types, result);
        let exact = source.identity_record();
        let nominal_group = source.nominal_specialization();
        if let Some(found) = self
            .continuation_shells
            .iter()
            .find(|found| found.identity().result_record().id() == exact.id())
        {
            return (found.success(), found.failure());
        }
        let result_name = mir::type_name(shell, result);
        let mut resume_locals = Arena::new();
        let receiver = resume_locals.alloc(mir::Local {
            name: "this".to_string(),
            ty: mir::Type::Interface(continuation),
            mutable: false,
        });
        let value = resume_locals.alloc(mir::Local {
            name: "value".to_string(),
            ty: result.clone(),
            mutable: false,
        });
        let resume = functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: format!("Continuation.resume<{result_name}>"),
            params: vec![
                mir::Param {
                    name: "this".to_string(),
                    ty: mir::Type::Interface(continuation),
                    local: receiver,
                },
                mir::Param {
                    name: "value".to_string(),
                    ty: result.clone(),
                    local: value,
                },
            ],
            return_ty: mir::Type::Unit,
            body: mir::Body::unreachable(resume_locals),
        });
        let mut failure_locals = Arena::new();
        let receiver = failure_locals.alloc(mir::Local {
            name: "this".to_string(),
            ty: mir::Type::Interface(continuation),
            mutable: false,
        });
        let exception = failure_locals.alloc(mir::Local {
            name: "exception".to_string(),
            ty: throwable.clone(),
            mutable: false,
        });
        let failure = functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: format!("Continuation.resumeWithException<{result_name}>"),
            params: vec![
                mir::Param {
                    name: "this".to_string(),
                    ty: mir::Type::Interface(continuation),
                    local: receiver,
                },
                mir::Param {
                    name: "exception".to_string(),
                    ty: throwable,
                    local: exception,
                },
            ],
            return_ty: mir::Type::Unit,
            body: mir::Body::unreachable(failure_locals),
        });
        let identity = mir::ContinuationShellIdentity::new(
            exact,
            nominal_group,
            success_signature,
            failure_signature,
        )
        .expect("local-concrete coroutine protocols have one continuation-shell identity");
        let metadata = mir::CoroutineContinuationShell::checked(
            functions,
            result.clone(),
            resume,
            failure,
            identity,
        )
        .expect("generated continuation shells have the exact dispatch signatures");
        self.continuation_shells.push(metadata);
        (resume, failure)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn start_helper(
        &mut self,
        exact_types: &SourceExactTypeRegistry,
        result: &mir::Type,
        task_interface: mir::InterfaceId,
        continuation_interface: mir::InterfaceId,
        run: mir::MonomorphizedFunctionId,
        resume: mir::MonomorphizedFunctionId,
        resume_with_exception: mir::MonomorphizedFunctionId,
        step_ty: &mir::Type,
        throwable: mir::Type,
        functions: &mut Arena<mir::Function>,
        top_level: &mut Vec<mir::FunctionId>,
        shell: &mir::Module,
    ) -> mir::FunctionId {
        let source = Self::source_type(exact_types, result);
        let exact = source.identity_record();
        let nominal_group = source.nominal_specialization();
        let step_id = self
            .steps_by_result
            .iter()
            .find(|(found, _)| *found == exact.id())
            .map(|(_, id)| *id)
            .expect("start helpers are created after their CoroutineStep metadata");
        let step_metadata = &self.steps[step_id];
        assert_eq!(
            step_ty,
            &mir::Type::Enum(step_metadata.enum_id(), Vec::new()),
            "the start helper result and CoroutineStep type must identify the same metadata",
        );
        assert_eq!(
            self.step_metadata_for_type(step_ty).result(),
            result,
            "the start helper CoroutineStep metadata must carry its result type",
        );
        if let Some(found) = self
            .start_helpers
            .iter()
            .find(|found| found.identity().result_record().id() == exact.id())
        {
            return found.function();
        }
        let completed_variant = step_metadata.completed();
        let completed_payload = step_metadata.completed_payload();

        let mut locals = Arena::new();
        let task = locals.alloc(mir::Local {
            name: "task".to_string(),
            ty: mir::Type::Interface(task_interface),
            mutable: false,
        });
        let completion = locals.alloc(mir::Local {
            name: "completion".to_string(),
            ty: mir::Type::Interface(continuation_interface),
            mutable: false,
        });
        let step = locals.alloc(mir::Local {
            name: "$step".to_string(),
            ty: step_ty.clone(),
            mutable: false,
        });
        let exception = locals.alloc(mir::Local {
            name: "$exception".to_string(),
            ty: throwable.clone(),
            mutable: false,
        });
        let span = mir::SourceSpan::new(0, 0).expect("synthetic span is ordered");
        let statement = |kind| mir::Statement { kind, span };
        let mut blocks = Arena::new();
        let suspended = blocks.alloc(mir::BasicBlock {
            name: "suspended".to_string(),
            statements: Vec::new(),
            terminator: mir::Terminator::Return { value: None },
            unwind: None,
        });
        let completed = blocks.alloc(mir::BasicBlock {
            name: "completed".to_string(),
            statements: vec![statement(mir::StatementKind::Call(mir::CallEffect::Unit(
                mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Interface {
                            interface: continuation_interface,
                            slot: 0,
                        },
                        callee: mir::Callee::Monomorphized(resume),
                    },
                    args: vec![
                        mir::Expr::local(completion, mir::Type::Interface(continuation_interface)),
                        mir::Expr::new(
                            result.clone(),
                            mir::ExprKind::EnumField {
                                operand: Box::new(mir::Expr::local(step, step_ty.clone())),
                                variant: completed_payload.variant().variant_index(),
                                index: completed_payload.field_index(),
                            },
                        ),
                    ],
                    pending: mir::CoroutinePendingContext::Root,
                },
            )))],
            terminator: mir::Terminator::Return { value: None },
            unwind: None,
        });
        let failed = blocks.alloc(mir::BasicBlock {
            name: "failed".to_string(),
            statements: vec![statement(mir::StatementKind::Call(mir::CallEffect::Unit(
                mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Interface {
                            interface: continuation_interface,
                            slot: 1,
                        },
                        callee: mir::Callee::Monomorphized(resume_with_exception),
                    },
                    args: vec![
                        mir::Expr::local(completion, mir::Type::Interface(continuation_interface)),
                        mir::Expr::local(exception, throwable.clone()),
                    ],
                    pending: mir::CoroutinePendingContext::Root,
                },
            )))],
            terminator: mir::Terminator::Return { value: None },
            unwind: None,
        });
        let catch_pad = blocks.alloc(mir::BasicBlock {
            name: "body_failure".to_string(),
            statements: vec![
                statement(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
                    cleanup: false,
                })),
                statement(mir::StatementKind::Eh(mir::EhStatement::BeginCatch)),
                statement(mir::StatementKind::Call(mir::CallEffect::Value {
                    destination: exception,
                    call: mir::Call {
                        target: mir::CallTarget {
                            kind: mir::CallKind::Direct,
                            callee: mir::Callee::Runtime(mir::RuntimeFn::MaterializeException),
                        },
                        args: vec![mir::Expr::caught_exception()],
                        pending: mir::CoroutinePendingContext::Root,
                    },
                })),
                statement(mir::StatementKind::Eh(mir::EhStatement::EndCatch)),
            ],
            terminator: mir::Terminator::Goto(failed),
            unwind: None,
        });
        let entry = blocks.alloc(mir::BasicBlock {
            name: "entry".to_string(),
            statements: vec![statement(mir::StatementKind::Call(
                mir::CallEffect::Value {
                    destination: step,
                    call: mir::Call {
                        target: mir::CallTarget {
                            kind: mir::CallKind::Interface {
                                interface: task_interface,
                                slot: 0,
                            },
                            callee: mir::Callee::Monomorphized(run),
                        },
                        args: vec![
                            mir::Expr::local(task, mir::Type::Interface(task_interface)),
                            mir::Expr::local(
                                completion,
                                mir::Type::Interface(continuation_interface),
                            ),
                        ],
                        pending: mir::CoroutinePendingContext::Root,
                    },
                },
            ))],
            terminator: mir::Terminator::Branch {
                cond: mir::Expr::machine_eq(
                    mir::Expr::enum_tag(mir::Expr::local(step, step_ty.clone())),
                    mir::MachineScalarValue::EnumTag(completed_variant.variant_index()),
                ),
                then_block: completed,
                else_block: suspended,
            },
            unwind: Some(catch_pad),
        });
        let result_name = mir::type_name(shell, result);
        let function = functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: format!("startCoroutine<{result_name}>"),
            params: vec![
                mir::Param {
                    name: "task".to_string(),
                    ty: mir::Type::Interface(task_interface),
                    local: task,
                },
                mir::Param {
                    name: "completion".to_string(),
                    ty: mir::Type::Interface(continuation_interface),
                    local: completion,
                },
            ],
            return_ty: mir::Type::Unit,
            body: mir::Body {
                locals,
                blocks,
                entry,
                loop_header_polls: Vec::new(),
            },
        });
        top_level.push(function);
        let signature = hir::ExactCallableSignature::new(
            hir::Effect::Ordinary,
            None,
            [task_interface, continuation_interface]
                .map(|interface| {
                    exact_types
                        .get(&mir::Type::Interface(interface))
                        .expect("a concrete coroutine protocol interface has one exact identity")
                        .identity_record()
                        .id()
                })
                .into(),
            exact_types
                .get(&mir::Type::Unit)
                .expect("the core Unit type has one exact identity")
                .identity_record()
                .id(),
        );
        let identity = mir::CoroutineStartIdentity::new(exact, nominal_group, signature)
            .expect("local-concrete coroutine protocols have one coroutine-start identity");
        let metadata = mir::CoroutineStart::checked(functions, result.clone(), function, identity)
            .expect("generated coroutine start helper has the exact erased signature");
        self.start_helpers.push(metadata);
        function
    }
}
