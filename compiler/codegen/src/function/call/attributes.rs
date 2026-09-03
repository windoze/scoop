use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(in crate::function) fn apply_call_protocol(
        &self,
        call: inkwell::values::CallSiteValue<'ctx>,
        destination: scoop_lir::CallDestination,
        protocol: &CallProtocol<'_>,
    ) {
        if matches!(protocol, CallProtocol::NoGc) {
            call.add_attribute(
                AttributeLoc::Function,
                self.context.create_string_attribute("gc-leaf-function", ""),
            );
        }
        if matches!(
            protocol,
            CallProtocol::NativeSafe { .. } | CallProtocol::NativeBorrowed { .. }
        ) {
            self.apply_nounwind(call);
        }
        if let Some(safepoint) = protocol.safepoint() {
            self.apply_safepoint_id(call, safepoint);
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
