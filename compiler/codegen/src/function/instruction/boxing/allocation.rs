//! Allocate a known box using the ordinary managed allocation paths.

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
        let instance_layout = payload.instance_layout();
        let allocation = self.managed_alloc_value(
            descriptor,
            self.context
                .i64_type()
                .const_int(instance_layout.size().get(), false),
            Some(AllocationLayout::boxed(instance_layout)),
            safepoint,
            live,
            None,
        )?;
        let object = allocation.object;
        if let BoxPayload::NonZero(place) = payload {
            let payload = place.descriptor();
            let layout = payload.value().layout();
            let alignment = layout.alignment().get() as u32;
            // The collecting path has updated the source's canonical root storage.
            let source = self.local_pointer(place.local())?;
            let destination = self.byte_gep(object, payload.payload_offset(), "box.payload")?;
            let size = self
                .context
                .i64_type()
                .const_int(layout.size().get(), false);
            if instance_layout.size().get() > 128 {
                self.copy_large_box_payload(destination, source, size)?;
            } else {
                self.builder
                    .build_memcpy(destination, alignment, source, alignment, size)
                    .map_err(box_error)?;
            }
            self.allocated_value_barrier(
                destination,
                payload.value().storage_type(),
                allocation.young,
            )?;
        }
        Ok(object)
    }

    fn copy_large_box_payload(
        &self,
        destination: PointerValue<'ctx>,
        source: PointerValue<'ctx>,
        size: IntValue<'ctx>,
    ) -> Result<(), CodegenError> {
        let pointer = ptr_ty(self.context);
        let destination = self
            .builder
            .build_address_space_cast(destination, pointer, "box.copy_destination")
            .map_err(box_error)?;
        let memcpy = self.runtime_fn(
            "memcpy",
            pointer.fn_type(
                &[pointer.into(), pointer.into(), size.get_type().into()],
                false,
            ),
        );
        let call = self
            .builder
            .build_call(
                memcpy,
                &[destination.into(), source.into(), size.into()],
                "box.copy",
            )
            .map_err(box_error)?;
        // Keep large copies shared without changing an ordinary extern's attributes.
        call.add_attribute(
            AttributeLoc::Function,
            self.context.create_string_attribute("gc-leaf-function", ""),
        );
        call.add_attribute(
            AttributeLoc::Function,
            self.context
                .create_enum_attribute(Attribute::get_named_enum_kind_id("nobuiltin"), 0),
        );
        self.apply_nounwind(call);
        Ok(())
    }
}
