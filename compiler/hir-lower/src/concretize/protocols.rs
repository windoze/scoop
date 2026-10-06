//! Checked compiler roles are explicit concrete requests.

use super::*;
mod callbacks;

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
            protocols.exceptions.illegal_argument_exception,
        ] {
            let application = self.source.classes[exception.class()].self_application;
            let class = self.lower_class_application(application, &[]);
            self.request_class_constructor(exception.callable(), class);
        }
        self.request_function(
            protocols.exceptions.initialization_cycle_thrower,
            Vec::new(),
        );
        let missing_source = protocols.exceptions.missing_context_constructor;
        let missing_owner = self.source.class_constructors[missing_source].owner;
        let missing_class =
            self.lower_class_application(self.source.classes[missing_owner].self_application, &[]);
        let missing_constructor = self.request_class_constructor(missing_source, missing_class);
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
            unit: self.struct_by_key[&(
                self.source.nominal_identities[protocols.fundamental_types.unit].declaration_id(),
                Vec::new(),
            )],
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
                        self.class_constructor_origin(export::ClassConstructorDefinition::Local(
                            exception.callable(),
                        ))
                        .0,
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
                missing_context_constructor: missing_constructor,
                throwable: lower_exception(source_exception_core.throwable),
                unwrap_exception: lower_exception(source_exception_core.unwrap_exception),
                class_cast_exception: lower_exception(source_exception_core.class_cast_exception),
                arithmetic_exception: lower_exception(source_exception_core.arithmetic_exception),
                index_out_of_bounds_exception: lower_exception(
                    source_exception_core.index_out_of_bounds_exception,
                ),
                illegal_argument_exception: lower_exception(
                    source_exception_core.illegal_argument_exception,
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
            foreign_callbacks: concrete::ForeignCallbackCore {
                modes: callback_modes,
                states: callback_states,
                failure_result: callback_failure_result,
            },
            fundamental_types,
        }))
    }
}
