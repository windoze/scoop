//! Reuse complete LIR allocation facts across physical object partitions.

use super::*;

#[derive(Clone, Copy)]
pub(in crate::function) struct AllocationLayout {
    pub(super) alignment: u64,
    pub(super) nursery_eligible: bool,
}

impl AllocationLayout {
    pub(in crate::function) fn boxed(layout: scoop_lir::AbiNonZeroLayout) -> Self {
        Self {
            alignment: layout.alignment().get(),
            nursery_eligible: true,
        }
    }

    pub(in crate::function) fn string() -> Self {
        Self {
            alignment: 8,
            nursery_eligible: true,
        }
    }

    pub(in crate::function) fn array(layout: &scoop_lir::ArrayLayoutV1) -> Self {
        Self {
            alignment: layout.instance().instance_alignment(),
            nursery_eligible: true,
        }
    }
}

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn known_allocation_layout(&self, descriptor: Value) -> Option<AllocationLayout> {
        let Value::TypeDescriptor(scoop_lir::TypeDescriptorRef::Local(id)) = descriptor else {
            return None;
        };
        let descriptor = &self.type_descriptors[id];
        Some(AllocationLayout {
            alignment: descriptor.instance_shape.instance_alignment(),
            nursery_eligible: descriptor.release_policy.hook().is_none(),
        })
    }

    pub(super) fn allocation_parameters(
        &self,
        descriptor: PointerValue<'ctx>,
        layout: Option<AllocationLayout>,
    ) -> Result<(IntValue<'ctx>, IntValue<'ctx>), CodegenError> {
        if let Some(layout) = layout {
            return Ok((
                self.context.i64_type().const_int(layout.alignment, false),
                self.context
                    .bool_type()
                    .const_int(u64::from(layout.nursery_eligible), false),
            ));
        }
        let metadata = crate::runtime_metadata_v1::RuntimeMetadataV1Types::new(self.context);
        let shape = self
            .builder
            .build_struct_gep(
                metadata.type_descriptor(),
                descriptor,
                1,
                "allocation_shape",
            )
            .map_err(layout_error)?;
        let alignment_slot = self
            .builder
            .build_struct_gep(
                metadata.type_instance_shape(),
                shape,
                3,
                "allocation_alignment_ptr",
            )
            .map_err(layout_error)?;
        let alignment = self
            .builder
            .build_load(
                self.context.i64_type(),
                alignment_slot,
                "allocation_alignment",
            )
            .map_err(layout_error)?
            .into_int_value();
        let hook_slot = self
            .builder
            .build_struct_gep(
                metadata.type_descriptor(),
                descriptor,
                11,
                "allocation_hook_ptr",
            )
            .map_err(layout_error)?;
        let hook = self
            .builder
            .build_load(ptr_ty(self.context), hook_slot, "allocation_hook")
            .map_err(layout_error)?
            .into_pointer_value();
        let nursery_eligible = self
            .builder
            .build_is_null(hook, "allocation_nursery_eligible")
            .map_err(layout_error)?;
        Ok((alignment, nursery_eligible))
    }
}

fn layout_error(error: impl std::fmt::Display) -> CodegenError {
    CodegenError(format!("allocation layout: {error}"))
}
