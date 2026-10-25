//! Allocate a known small box using the ordinary managed allocation paths.

use super::*;
use crate::function::gc::AllocationLayout;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn allocate_box(
        &mut self,
        payload: &BoxPayload,
        descriptor: PointerValue<'ctx>,
        safepoint: scoop_lir::SafepointId,
        live: MaterializedStatepointLive<'ctx>,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        let layout = payload.instance_layout();
        let object = self.managed_alloc_value(
            descriptor,
            self.context
                .i64_type()
                .const_int(layout.size().get(), false),
            Some(AllocationLayout::boxed(layout)),
            safepoint,
            live,
            None,
        )?;
        if let BoxPayload::NonZero(place) = payload {
            let payload = place.descriptor();
            let layout = payload.value().layout();
            let alignment = layout.alignment().get() as u32;
            // The collecting path has updated the source's canonical root storage.
            let source = self.local_pointer(place.local())?;
            let destination = self.byte_gep(object, payload.payload_offset(), "box.payload")?;
            self.builder
                .build_memcpy(
                    destination,
                    alignment,
                    source,
                    alignment,
                    self.context
                        .i64_type()
                        .const_int(layout.size().get(), false),
                )
                .map_err(box_error)?;
            self.heap_value_barrier(destination, payload.value().storage_type())?;
        }
        Ok(object)
    }
}
