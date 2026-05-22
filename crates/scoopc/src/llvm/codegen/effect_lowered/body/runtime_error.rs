//! Runtime-error boundary lowering: emits the per-case runtime-error payload, the local terminal action that abandons the current callable, and the materialization of fatal payloads that escape to the runtime.

use super::*;

impl<'cg, 'a, 'ctx> CallableEmitter<'cg, 'a, 'ctx> {
    pub(super) fn load_gc_object_type_desc(
        &mut self,
        obj: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let header_ty = self.codegen.llvm_gc_object_header_type();
        let header_ptr_ty = self.codegen.llvm_ptr_type(self.codegen.gc_address_space());
        let header_ptr =
            self.codegen
                .builder
                .build_pointer_cast(obj, header_ptr_ty, &format!("{name}_hdr"))?;
        let type_desc_ptr = self.codegen.builder.build_struct_gep(
            header_ty,
            header_ptr,
            1,
            &format!("{name}_gep"),
        )?;
        Ok(self
            .codegen
            .builder
            .build_load(self.codegen.llvm_i8_ptr_type(), type_desc_ptr, name)?
            .into_pointer_value())
    }

    pub(super) fn lower_runtime_error_boundary(
        &mut self,
        boundary: &LateLoweredBoundary,
        lowering: &crate::effect_lowered::ir::LateLoweredRuntimeErrorBoundaryLowering,
    ) -> Result<(), LlvmEmitError> {
        let payload =
            self.lower_runtime_error_boundary_payload(lowering.emitted_step().payload_tuple_ty())?;
        self.emit_or_consume_outward_case(
            boundary,
            lowering.emitted_step().case_tag(),
            payload,
            lowering.emitted_step().payload_tuple_ty(),
            None,
            None,
        )
    }

    pub(super) fn lower_runtime_error_boundary_payload(
        &mut self,
        payload_ty: TypeId,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        let layout = self.abi.source_value_layout(payload_ty)?;
        if layout.abi().is_elided() {
            return Ok(None);
        }
        let value =
            self.runtime_error_unit_variant_payload(payload_ty, "ContinuationAlreadyResumed")?;
        match layout.kind() {
            SourceAbiLayoutKind::Scalar => Ok(value.value),
            SourceAbiLayoutKind::Tuple => Err(frontend_error(format!(
                "runtime-error boundary payload t{} 需要 scalar ABI，当前 tuple ABI 尚未发布 payload field contract",
                payload_ty.as_u32()
            ))),
        }
    }

    pub(super) fn runtime_error_unit_variant_payload(
        &mut self,
        payload_ty: TypeId,
        variant_name: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !self.source_ty_is_runtime_error(payload_ty) {
            return Err(frontend_error(format!(
                "runtime-error payload t{} 不是 scoop.core.RuntimeError",
                payload_ty.as_u32()
            )));
        }
        let enum_layout = self
            .codegen
            .enum_layouts
            .get("scoop.core.RuntimeError")
            .unwrap_or_else(|| {
                panic!("runtime_error_unit_variant_payload: verifier accepted missing RuntimeError enum layout")
            });
        let variant = enum_layout
            .variants
            .iter()
            .find(|variant| variant.name == variant_name)
            .unwrap_or_else(|| {
                panic!("runtime_error_unit_variant_payload: verifier accepted missing RuntimeError variant `{variant_name}`")
            });
        if !variant.fields.is_empty() {
            panic!(
                "runtime_error_unit_variant_payload: verifier accepted RuntimeError payload variant fields"
            );
        }
        let abi = self.abi.source_value_layout(payload_ty)?.abi();
        let raw = match abi.llvm_ty() {
            BasicTypeEnum::IntType(int_ty) => int_ty.const_int(variant.tag, false).into(),
            BasicTypeEnum::StructType(struct_ty) => {
                let Some(BasicTypeEnum::IntType(tag_ty)) = struct_ty.get_field_type_at_index(0)
                else {
                    return Err(frontend_error(
                        "RuntimeError tagged-union payload 缺少整数 tag field".to_string(),
                    ));
                };
                let mut aggregate = struct_ty.get_undef();
                aggregate = self
                    .codegen
                    .builder
                    .build_insert_value(
                        aggregate,
                        tag_ty.const_int(variant.tag, false),
                        0,
                        "runtime_error_tag",
                    )?
                    .into_struct_value();
                for field_index in 1..struct_ty.count_fields() {
                    let Some(field_ty) = struct_ty.get_field_type_at_index(field_index) else {
                        return Err(frontend_error(format!(
                            "RuntimeError tagged-union payload 缺少 field {}",
                            field_index
                        )));
                    };
                    aggregate = self
                        .codegen
                        .builder
                        .build_insert_value(
                            aggregate,
                            field_ty.const_zero(),
                            field_index,
                            "runtime_error_payload_zero",
                        )?
                        .into_struct_value();
                }
                aggregate.into()
            }
            other => {
                return Err(frontend_error(format!(
                    "RuntimeError payload ABI 不是 int/struct：{:?}",
                    other
                )));
            }
        };
        let payload_ty = self
            .codegen
            .equivalent_codegen_mono_type_id(self.source_types, payload_ty)
            .unwrap_or_else(|| {
                panic!(
                    "runtime_error_unit_variant_payload: RuntimeError payload t{} has no codegen TypeStore equivalent",
                    payload_ty.as_u32()
                )
            });
        Ok(CgValue {
            ty: CgTy::Enum(payload_ty),
            value: Some(raw),
        })
    }

    pub(super) fn emit_local_runtime_error_terminal(
        &mut self,
        runtime: &LocalRuntimeErrorRuntime,
        payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        let payload = payload.ok_or_else(|| {
            frontend_error(format!(
                "LocalRuntimeError st{} call site {} case c{} 需要 materialized payload t{}，但 lowering 产出了 elided payload",
                runtime.target_state.as_u32(),
                runtime.site_id.as_u32(),
                runtime.input_case_tag.as_u32(),
                runtime.payload_tuple_ty.as_u32()
            ))
        })?;
        let callee = self
            .codegen
            .module
            .get_function(&runtime.runtime_symbol)
            .unwrap_or_else(|| self.codegen.declare_runtime_error_fatal());
        if callee.count_params() as usize != runtime.runtime_param_count {
            return Err(frontend_error(format!(
                "LocalRuntimeError runtime entry `{}` 参数数量漂移：module={} contract={}",
                runtime.runtime_symbol,
                callee.count_params(),
                runtime.runtime_param_count
            )));
        }
        let payload = self.materialize_runtime_error_fatal_payload(payload)?;
        self.codegen
            .builder
            .build_call(callee, &[payload.into()], "local_runtime_error_fatal")?;
        self.codegen.builder.build_unreachable()?;
        Ok(())
    }

    pub(super) fn materialize_runtime_error_fatal_payload(
        &mut self,
        payload: BasicValueEnum<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let runtime_payload_ty = self.codegen.llvm_gc_i8_ptr_type();
        if let BasicValueEnum::PointerValue(ptr) = payload {
            return self
                .codegen
                .cast_ptr(ptr, runtime_payload_ty, "runtime_error_payload_ptr");
        }

        let slot = self
            .codegen
            .builder
            .build_alloca(payload.get_type(), "runtime_error_payload_obj")?;
        self.codegen.builder.build_store(slot, payload)?;
        self.codegen
            .cast_ptr(slot, runtime_payload_ty, "runtime_error_payload_ptr")
    }
}
