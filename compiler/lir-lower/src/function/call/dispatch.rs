use super::*;

impl FunctionLowerer<'_> {
    pub(super) fn load_dispatch_destination(
        &mut self,
        kind: &mir::CallKind,
        receiver: lir::Value,
        effect: mir::GcEffect,
    ) -> StorageResult<LoweredCallDestination> {
        let (table, kind, slot) = match kind {
            mir::CallKind::Virtual { slot } => {
                let descriptor = self.load_at_offset(
                    receiver,
                    self.context.object_type_descriptor_offset(),
                    lir::METADATA_PTR,
                );
                let table = self.load_at_offset(
                    lir::Value::Temp(descriptor),
                    self.context.type_descriptor_vtable_offset(),
                    lir::METADATA_PTR,
                );
                (lir::Value::Temp(table), lir::DispatchKind::Virtual, *slot)
            }
            mir::CallKind::Interface { slot, .. } => {
                let table = self.interface_component(receiver, 1);
                (table, lir::DispatchKind::Interface, *slot)
            }
            _ => unreachable!("only virtual and interface calls load a dispatch table"),
        };
        Ok(self.dispatch_destination(table, kind, slot, effect))
    }
}
