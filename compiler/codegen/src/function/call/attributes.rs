use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(in crate::function) fn apply_c_abi_attributes(
        &self,
        destination: scoop_lir::CallDestination,
        mut apply: impl FnMut(AttributeLoc, Attribute),
    ) {
        let scoop_lir::CallDestination::Extern(id) = destination else {
            return;
        };
        let ExternFunctionKind::C {
            call_plan: scoop_lir::CAbiCallPlan::Direct(signature),
            ..
        } = &self.extern_functions[id].kind
        else {
            return;
        };
        let mut extend = |location, extension| {
            let name = match extension {
                scoop_lir::CIntegerExtension::None => return,
                scoop_lir::CIntegerExtension::Sign => "signext",
                scoop_lir::CIntegerExtension::Zero => "zeroext",
            };
            apply(
                location,
                self.context
                    .create_enum_attribute(Attribute::get_named_enum_kind_id(name), 0),
            );
        };
        for (index, parameter) in signature.params.iter().enumerate() {
            extend(AttributeLoc::Param(index as u32), parameter.extension);
        }
        if let scoop_lir::DirectCReturn::Value(result) = &signature.result {
            extend(AttributeLoc::Return, result.extension);
        }
        // A source extern always calls its selected native symbol, including
        // names that LLVM would otherwise recognize as library builtins.
        apply(
            AttributeLoc::Function,
            self.context
                .create_enum_attribute(Attribute::get_named_enum_kind_id("nobuiltin"), 0),
        );
    }

    pub(in crate::function) fn apply_call_protocol(
        &self,
        call: inkwell::values::CallSiteValue<'ctx>,
        destination: scoop_lir::CallDestination,
        protocol: &CallProtocol<'_>,
    ) {
        if matches!(
            protocol,
            CallProtocol::NoGc | CallProtocol::NativeGcLeaf | CallProtocol::ReleaseNativeLeaf
        ) {
            call.add_attribute(
                AttributeLoc::Function,
                self.context.create_string_attribute("gc-leaf-function", ""),
            );
        }
        if matches!(
            protocol,
            CallProtocol::NativeSafe { .. }
                | CallProtocol::NativeBorrowed { .. }
                | CallProtocol::NativeGcLeaf
                | CallProtocol::ReleaseNativeLeaf
        ) {
            self.apply_nounwind(call);
        }
        if let Some(safepoint) = protocol.safepoint() {
            self.apply_safepoint_id(call, safepoint);
        }
        if matches!(
            destination,
            scoop_lir::CallDestination::Runtime(
                scoop_lir::RuntimeFunction::Managed(
                    scoop_lir::ManagedRuntimeFunction::ContextPush
                        | scoop_lir::ManagedRuntimeFunction::ContextFork
                        | scoop_lir::ManagedRuntimeFunction::ContextEnsureRoot
                ) | scoop_lir::RuntimeFunction::NoGc(
                    scoop_lir::NoGcRuntimeFunction::ContextTryGet
                        | scoop_lir::NoGcRuntimeFunction::ContextRestore
                        | scoop_lir::NoGcRuntimeFunction::ContextSnapshot
                        | scoop_lir::NoGcRuntimeFunction::ContextCurrent
                        | scoop_lir::NoGcRuntimeFunction::ContextEnter
                        | scoop_lir::NoGcRuntimeFunction::ContextLeave
                )
            )
        ) {
            self.apply_nounwind(call);
        }
        if matches!(
            destination,
            scoop_lir::CallDestination::Runtime(scoop_lir::RuntimeFunction::NoGc(
                scoop_lir::NoGcRuntimeFunction::Trap
                    | scoop_lir::NoGcRuntimeFunction::Throw
                    | scoop_lir::NoGcRuntimeFunction::Rethrow
            ))
        ) {
            call.add_attribute(
                AttributeLoc::Function,
                self.context
                    .create_enum_attribute(Attribute::get_named_enum_kind_id("noreturn"), 0),
            );
        }
    }

    pub(in crate::function) fn apply_nounwind(&self, call: inkwell::values::CallSiteValue<'ctx>) {
        call.add_attribute(
            AttributeLoc::Function,
            self.context
                .create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
        );
    }

    pub(in crate::function) fn apply_safepoint_id(
        &self,
        call: inkwell::values::CallSiteValue<'ctx>,
        safepoint: scoop_lir::SafepointId,
    ) {
        call.add_attribute(
            AttributeLoc::Function,
            self.context
                .create_string_attribute("statepoint-id", &safepoint.get().to_string()),
        );
    }
}
