use super::*;

impl FunctionLowerer<'_> {
    pub(super) fn load_dispatch_destination(
        &mut self,
        kind: &mir::CallKind,
        receiver: lir::Value,
        effect: mir::GcEffect,
    ) -> StorageResult<LoweredCallDestination> {
        let descriptor = self.load_at_offset(
            receiver,
            self.context.object_type_descriptor_offset(),
            lir::METADATA_PTR,
        );
        let (table, kind, slot) = match kind {
            mir::CallKind::Virtual { slot } => {
                let table = self.load_at_offset(
                    lir::Value::Temp(descriptor),
                    self.context.type_descriptor_vtable_offset(),
                    lir::METADATA_PTR,
                );
                (lir::Value::Temp(table), lir::DispatchKind::Virtual, *slot)
            }
            mir::CallKind::Interface { interface, slot } => {
                let interface = self.td_ref(&mir::Type::Interface(*interface));
                let table = self.emit_plain_call(
                    LoweredCallDestination::no_gc_runtime(lir::NoGcRuntimeFunction::ITableLookup),
                    vec![lir::METADATA_PTR, lir::METADATA_PTR],
                    lir::METADATA_PTR,
                    vec![lir::Value::Temp(descriptor), interface],
                )?;
                (table, lir::DispatchKind::Interface, *slot)
            }
            _ => unreachable!("only virtual and interface calls load a dispatch table"),
        };
        Ok(self.dispatch_destination(table, kind, slot, effect))
    }
}
