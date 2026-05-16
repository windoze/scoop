//! Refactor step-case build and extract helpers: encodes a Step variant from a payload and the matching extract helpers (tag, case parts, payload struct) used by the consumer side of any effectful boundary.

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(super) fn refactor_build_step_case(
        &mut self,
        step_layout: &RefactorStepLayout<'ctx>,
        case_layout: &RefactorStepCaseLayout<'ctx>,
        payload: Option<BasicValueEnum<'ctx>>,
        continuation: PointerValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        self.refactor_build_step_variant(
            step_layout,
            case_layout.variant(),
            case_layout.variant().tag_value(),
            payload,
            Some(continuation),
        )
    }

    pub(super) fn refactor_build_step_variant(
        &mut self,
        step_layout: &RefactorStepLayout<'ctx>,
        variant: &RefactorStepVariantLayout<'ctx>,
        tag: u32,
        payload: Option<BasicValueEnum<'ctx>>,
        continuation: Option<PointerValue<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        let step_ptr = self
            .builder
            .build_alloca(step_layout.llvm_ty(), "refactor_step_tmp")?;
        self.builder
            .build_store(step_ptr, step_layout.llvm_ty().const_zero())?;
        let tag_ptr = self.builder.build_struct_gep(
            step_layout.llvm_ty(),
            step_ptr,
            0,
            "refactor_step_tag_gep",
        )?;
        self.builder.build_store(
            tag_ptr,
            self.context.i32_type().const_int(u64::from(tag), false),
        )?;
        let storage_ptr = self.builder.build_struct_gep(
            step_layout.llvm_ty(),
            step_ptr,
            1,
            "refactor_step_storage_gep",
        )?;
        let payload_ptr = self.refactor_cast_ptr(
            storage_ptr,
            self.context.ptr_type(AddressSpace::default()),
            "refactor_step_payload_ptr",
        )?;
        let mut payload_value = variant.payload_ty().get_undef();
        let mut next_field = 0u32;
        if !variant.payload_is_elided() {
            let payload = payload.ok_or_else(|| {
                frontend_error(format!(
                    "refactor Step variant tag {} ({}) 需要 payload，但 lowering 未提供",
                    tag,
                    variant.payload_anchor_name()
                ))
            })?;
            let expected_payload_ty = variant
                .payload_ty()
                .get_field_type_at_index(next_field)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor Step variant tag {} ({}) 缺少 payload field#{} layout",
                        tag,
                        variant.payload_anchor_name(),
                        next_field
                    ))
                })?;
            if payload.get_type() != expected_payload_ty {
                return Err(frontend_error(format!(
                    "refactor Step variant tag {} ({}) payload field#{} type drift: expected {:?}, got {:?}",
                    tag,
                    variant.payload_anchor_name(),
                    next_field,
                    expected_payload_ty,
                    payload.get_type()
                )));
            }
            payload_value = self
                .builder
                .build_insert_value(
                    payload_value,
                    payload,
                    next_field,
                    "refactor_step_payload_insert",
                )?
                .into_struct_value();
            next_field += 1;
        }
        if let Some(continuation) = continuation {
            payload_value = self
                .builder
                .build_insert_value(
                    payload_value,
                    continuation,
                    next_field,
                    "refactor_step_cont_insert",
                )?
                .into_struct_value();
        }
        self.builder.build_store(payload_ptr, payload_value)?;
        Ok(self
            .builder
            .build_load(step_layout.llvm_ty(), step_ptr, "refactor_step")?)
    }

    pub(super) fn refactor_extract_step_tag(
        &mut self,
        _step_layout: &RefactorStepLayout<'ctx>,
        step: BasicValueEnum<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let BasicValueEnum::StructValue(step) = step else {
            return Err(frontend_error(
                "refactor Step value 不是 struct".to_string(),
            ));
        };
        Ok(self
            .builder
            .build_extract_value(step, 0, "refactor_step_tag")?
            .into_int_value())
    }

    pub(in crate::llvm::codegen::effect_lowered) fn refactor_extract_step_payload(
        &mut self,
        step_layout: &RefactorStepLayout<'ctx>,
        step: BasicValueEnum<'ctx>,
        variant: &RefactorStepVariantLayout<'ctx>,
        name: &str,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        let (payload, _) =
            self.refactor_extract_step_payload_struct(step_layout, step, variant, name)?;
        if variant.payload_is_elided() {
            return Ok(None);
        }
        Ok(Some(self.builder.build_extract_value(payload, 0, name)?))
    }

    pub(super) fn refactor_extract_step_case_parts(
        &mut self,
        step_layout: &RefactorStepLayout<'ctx>,
        step: BasicValueEnum<'ctx>,
        case_layout: &RefactorStepCaseLayout<'ctx>,
        name: &str,
    ) -> Result<(Option<BasicValueEnum<'ctx>>, PointerValue<'ctx>), LlvmEmitError> {
        let variant = case_layout.variant();
        let (payload_struct, _) =
            self.refactor_extract_step_payload_struct(step_layout, step, variant, name)?;
        let payload = if variant.payload_is_elided() {
            None
        } else {
            Some(
                self.builder
                    .build_extract_value(payload_struct, 0, &format!("{name}_payload"))?,
            )
        };
        let cont_index = if variant.payload_is_elided() { 0 } else { 1 };
        let cont = self
            .builder
            .build_extract_value(payload_struct, cont_index, &format!("{name}_cont"))?
            .into_pointer_value();
        Ok((payload, cont))
    }

    pub(super) fn refactor_extract_step_payload_struct(
        &mut self,
        step_layout: &RefactorStepLayout<'ctx>,
        step: BasicValueEnum<'ctx>,
        variant: &RefactorStepVariantLayout<'ctx>,
        name: &str,
    ) -> Result<(inkwell::values::StructValue<'ctx>, PointerValue<'ctx>), LlvmEmitError> {
        let step_ptr = self
            .builder
            .build_alloca(step_layout.llvm_ty(), &format!("{name}_step_tmp"))?;
        self.builder.build_store(step_ptr, step)?;
        let storage_ptr = self.builder.build_struct_gep(
            step_layout.llvm_ty(),
            step_ptr,
            1,
            &format!("{name}_storage_gep"),
        )?;
        let payload_ptr = self.refactor_cast_ptr(
            storage_ptr,
            self.context.ptr_type(AddressSpace::default()),
            &format!("{name}_payload_ptr"),
        )?;
        let payload = self
            .builder
            .build_load(variant.payload_ty(), payload_ptr, name)?
            .into_struct_value();
        Ok((payload, payload_ptr))
    }
}
