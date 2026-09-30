//! Checked compiler roles are explicit concrete requests.

use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_defined_core_protocols(
        &mut self,
        protocols: &export::DefinedCoreProtocols,
    ) -> concrete::ConcreteCoreProtocols {
        // Protocol outputs request their checked subjects through the same queue
        // as source uses; they do not depend on automatic declaration roots.
        for exception in [
            protocols.exceptions.throwable,
            protocols.exceptions.unwrap_exception,
            protocols.exceptions.class_cast_exception,
            protocols.exceptions.arithmetic_exception,
            protocols.exceptions.index_out_of_bounds_exception,
            protocols.exceptions.illegal_state_exception,
        ] {
            let application = self.source.classes[exception.class()].self_application;
            let class = self.lower_class_application(application, &[]);
            self.request_class_constructor(exception.callable(), class);
        }
        self.request_function(
            protocols.exceptions.initialization_cycle_thrower,
            Vec::new(),
        );
        let coroutine_protocols = self.build_coroutine_protocols(protocols.coroutines);
        self.drain_pending_callables();

        let source_callback_core = protocols.foreign_callbacks;
        let callback_reusable =
            self.lower_applied_enum_variant_ref(source_callback_core.modes.reusable(), &[]);
        let callback_one_shot =
            self.lower_applied_enum_variant_ref(source_callback_core.modes.one_shot(), &[]);
        let callback_modes = concrete::ForeignCallbackModes::checked(
            &self.enums,
            callback_reusable,
            callback_one_shot,
        )
        .expect("the validated foreign callback mode protocol survives concretization");
        let callback_registered =
            self.lower_applied_enum_variant_ref(source_callback_core.states.registered(), &[]);
        let callback_active =
            self.lower_applied_enum_variant_ref(source_callback_core.states.active(), &[]);
        let callback_completed =
            self.lower_applied_enum_variant_ref(source_callback_core.states.completed(), &[]);
        let callback_failed =
            self.lower_applied_enum_variant_ref(source_callback_core.states.failed(), &[]);
        let callback_states = concrete::ForeignCallbackStates::checked(
            &self.enums,
            callback_registered,
            callback_active,
            callback_completed,
            callback_failed,
        )
        .expect("the validated foreign callback state protocol survives concretization");
        let callback_failure_some = self.lower_applied_enum_variant_field_ref(
            source_callback_core.failure_result.some_payload(),
            &[],
        );
        let callback_failure_none =
            self.lower_applied_enum_variant_ref(source_callback_core.failure_result.none(), &[]);
        let callback_failure_option = concrete::OptionCore::checked(
            &self.enums,
            callback_failure_some,
            callback_failure_none,
        )
        .expect("the validated foreign callback failure protocol survives concretization");
        let callback_throwable = self.class_by_key[&(
            self.source.nominal_identities[protocols.exceptions.throwable.class()].declaration_id(),
            Vec::new(),
        )];
        let callback_failure_result = concrete::ForeignCallbackFailureResult::checked(
            &self.enums,
            &self.types,
            callback_failure_option,
            callback_throwable,
        )
        .expect("foreign callback failure remains the exact Option<Throwable> specialization");

        let fundamental_types = concrete::IntrinsicTypeCore {
            integers: export::IntegerTypeCore::new(export::IntegerKind::ALL.map(|kind| {
                let declaration = protocols.fundamental_types.integers.owner(kind);
                let origin = self.source.nominal_identities[declaration].declaration_id();
                self.struct_by_key[&(origin, Vec::new())]
            }))
            .expect("validated integer owners remain distinct after concretization"),
            boolean: self.struct_by_key[&(
                self.source.nominal_identities[protocols.fundamental_types.boolean]
                    .declaration_id(),
                Vec::new(),
            )],
            string: self.class_by_key[&(
                self.source.nominal_identities[protocols.fundamental_types.string].declaration_id(),
                Vec::new(),
            )],
        };

        let lower_exception = |exception: export::CompilerException| concrete::CompilerException {
            constructor: {
                let class = self.class_by_key[&(
                    self.source.nominal_identities[exception.class()].declaration_id(),
                    Vec::new(),
                )];
                concrete::ZeroArgClassConstructor {
                    class,
                    callable: self.class_constructor_by_key[&(
                        constructor_work::ClassConstructorSource::Local(exception.callable()),
                        class,
                    )],
                }
            },
        };
        let source_exception_core = protocols.exceptions;
        let source_option_core = protocols.option;
        let option = self
            .enums
            .iter()
            .filter(|(enumeration, _)| {
                self.enum_source[enumeration] == source_option_core.enumeration()
            })
            .map(|(enumeration, _)| {
                let some = concrete::EnumVariantRef::checked(
                    &self.enums,
                    enumeration,
                    concrete::VariantId::from_raw(
                        source_option_core.some_payload().variant().local_index(),
                    ),
                )
                .expect("a concrete Option specialization retains its Some variant");
                let some_payload = concrete::EnumVariantFieldRef::checked(
                    &self.enums,
                    some,
                    source_option_core.some_payload().local_index(),
                )
                .expect("a concrete Option specialization retains its Some payload identity");
                let none = concrete::EnumVariantRef::checked(
                    &self.enums,
                    enumeration,
                    concrete::VariantId::from_raw(source_option_core.none().local_index()),
                )
                .expect("a concrete Option specialization retains its None variant");
                concrete::OptionCore::checked(&self.enums, some_payload, none)
                    .expect("the validated Option shape survives concretization")
            })
            .collect();

        concrete::ConcreteCoreProtocols::Defined(Box::new(concrete::DefinedConcreteCoreProtocols {
            option,
            exceptions: concrete::CompilerExceptionCore {
                throwable: lower_exception(source_exception_core.throwable),
                unwrap_exception: lower_exception(source_exception_core.unwrap_exception),
                class_cast_exception: lower_exception(source_exception_core.class_cast_exception),
                arithmetic_exception: lower_exception(source_exception_core.arithmetic_exception),
                index_out_of_bounds_exception: lower_exception(
                    source_exception_core.index_out_of_bounds_exception,
                ),
                illegal_state_exception: lower_exception(
                    source_exception_core.illegal_state_exception,
                ),
                initialization_cycle_thrower: self.function_by_key[&self.function_key(
                    FunctionSource::Local(source_exception_core.initialization_cycle_thrower),
                    None,
                    Vec::new(),
                )],
            },
            coroutines: coroutine_protocols,
            foreign_callbacks: concrete::ForeignCallbackCore {
                modes: callback_modes,
                states: callback_states,
                failure_result: callback_failure_result,
            },
            fundamental_types,
        }))
    }

    fn build_coroutine_protocols(
        &mut self,
        core: export::CoroutineCore,
    ) -> Vec<concrete::CoroutineProtocol> {
        let mut protocols = Vec::new();
        loop {
            self.drain_pending_callables();
            let mut results = Vec::new();
            for (index, function) in self.function_slots.iter().enumerate() {
                let Some(function) = function else {
                    continue;
                };
                if function.is_suspend
                    && matches!(
                        function.kind,
                        concrete::FunctionKind::User(_) | concrete::FunctionKind::Abstract { .. }
                    )
                {
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
                    results.extend(self.function_key_arguments(&self.function_keys[index]));
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
                        Vec::new(),
                    ),
                    continuation_resume_with_exception: self.request_method(
                        core.continuation_resume_with_exception,
                        concrete::MethodOwner::Interface(continuation),
                        Vec::new(),
                    ),
                    suspend_task_run: self.request_method(
                        core.suspend_task_run,
                        concrete::MethodOwner::Interface(suspend_task),
                        Vec::new(),
                    ),
                    suspend_registration_register: self.request_method(
                        core.suspend_registration_register,
                        concrete::MethodOwner::Interface(suspend_registration),
                        Vec::new(),
                    ),
                }
            }));
        }
        protocols
    }
}
