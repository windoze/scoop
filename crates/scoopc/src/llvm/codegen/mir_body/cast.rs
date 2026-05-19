//! MIR type-check / cast / runtime-ref cast lowering.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_type_check(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Operand,
        op: ast::TypeCheckOp,
        test_ty: TypeId,
        metadata: &crate::mir::RuntimeTypeTestMetadata,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if metadata.target_ty != test_ty || metadata.descriptor.ty != test_ty {
            panic!(
                "codegen_mir_type_check: MIR verifier accepted runtime type-check metadata drift"
            );
        }
        let is_ok =
            self.codegen_mir_runtime_type_test_is_ok(span, value, metadata, mir_types, slots)?;
        let out = match op {
            ast::TypeCheckOp::Is => is_ok,
            ast::TypeCheckOp::NotIs => self.builder.build_not(is_ok, "mir_typecheck_not")?,
        };
        Ok(CgValue::bool(out))
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_cast(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Operand,
        op: ast::CastOp,
        target_ty: TypeId,
        metadata: &crate::mir::RuntimeCastMetadata,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if metadata.test.target_ty != target_ty || metadata.test.descriptor.ty != target_ty {
            panic!("codegen_mir_cast: MIR verifier accepted runtime cast metadata drift");
        }
        match op {
            ast::CastOp::As => self.codegen_mir_cast_as(
                span, value, target_ty, metadata, mir_types, slots, target_cg,
            ),
            ast::CastOp::AsQ => self.codegen_mir_cast_asq(
                span, value, target_ty, metadata, mir_types, slots, target_cg,
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_cast_as(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Operand,
        target_ty: TypeId,
        metadata: &crate::mir::RuntimeCastMetadata,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let crate::mir::RuntimeCastFailure::Raise { error_fqn, .. } = &metadata.failure else {
            panic!("codegen_mir_cast_as: MIR verifier accepted invalid `as` failure contract");
        };
        if error_fqn != "scoop.core.RuntimeError.ClassCastFailed" {
            panic!(
                "codegen_mir_cast_as: MIR verifier accepted non-ClassCastFailed runtime error contract"
            );
        }
        let crate::mir::RuntimeCastResult::Target { ty } = &metadata.result else {
            panic!("codegen_mir_cast_as: MIR verifier accepted invalid `as` result contract");
        };
        if *ty != target_ty {
            panic!("codegen_mir_cast_as: MIR verifier accepted invalid `as` target contract");
        }

        let target_codegen_ty = self
            .equivalent_runtime_ref_codegen_type_id(mir_types, target_ty)
            .unwrap_or_else(|| {
                panic!("codegen_mir_cast_as: TypeStore equivalence verifier accepted unsupported `as` target codegen type")
            });
        let expected_cg = self.cg_ty_of(target_codegen_ty).unwrap_or_else(|| {
            panic!("codegen_mir_cast_as: MIR verifier accepted unsupported `as` target type")
        });
        let result_cg = if target_cg == CgTy::Never {
            expected_cg
        } else {
            target_cg
        };
        if expected_cg != result_cg || !matches!(result_cg, CgTy::Ref | CgTy::String) {
            return Err(frontend_error(format!(
                "MIR `as` target runtime type mismatch: target_ty={}, expected_cg={expected_cg:?}, result_cg={target_cg:?}",
                mir_types.display(target_ty)
            )));
        }

        let (obj_ptr, _) = self.codegen_mir_runtime_ref_operand(span, value, slots)?;
        if metadata.test.static_fold == crate::mir::RuntimeTypeStaticFold::AlwaysTrue {
            let target_ptr_ty = self.runtime_cast_target_ptr_type(span, result_cg)?;
            let casted_ptr =
                self.builder
                    .build_pointer_cast(obj_ptr, target_ptr_ty, "mir_cast_verified_ptr")?;
            return Ok(CgValue {
                ty: result_cg,
                value: Some(casted_ptr.into()),
            });
        }
        let is_ok = self.codegen_mir_runtime_type_test_is_ok(
            span,
            value,
            &metadata.test,
            mir_types,
            slots,
        )?;
        self.codegen_checked_runtime_ref_cast(span, obj_ptr, target_codegen_ty, result_cg, is_ok)
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_cast_asq(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Operand,
        target_ty: TypeId,
        metadata: &crate::mir::RuntimeCastMetadata,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !matches!(metadata.failure, crate::mir::RuntimeCastFailure::ReturnNone) {
            panic!("codegen_mir_cast_asq: MIR verifier accepted invalid `as?` failure contract");
        }
        let crate::mir::RuntimeCastResult::Option { option_ty, some_ty } = &metadata.result else {
            panic!("codegen_mir_cast_asq: MIR verifier accepted invalid `as?` result contract");
        };
        if *some_ty != target_ty {
            panic!("codegen_mir_cast_asq: MIR verifier accepted invalid `as?` target contract");
        }

        let target_codegen_ty = self
            .equivalent_runtime_ref_codegen_type_id(mir_types, target_ty)
            .unwrap_or_else(|| {
                panic!("codegen_mir_cast_asq: TypeStore equivalence verifier accepted unsupported `as?` target codegen type")
            });
        let target_value_cg = self.cg_ty_of(target_codegen_ty).unwrap_or_else(|| {
            panic!("codegen_mir_cast_asq: MIR verifier accepted unsupported `as?` target type")
        });
        if !matches!(target_value_cg, CgTy::Ref | CgTy::String) {
            panic!(
                "codegen_mir_cast_asq: MIR verifier accepted unsupported `as?` runtime target type"
            );
        }
        let option_codegen_ty = self
            .equivalent_codegen_type_id(mir_types, *option_ty)
            .unwrap_or_else(|| {
                panic!("codegen_mir_cast_asq: TypeStore equivalence verifier accepted unsupported `as?` option codegen type")
            });
        if target_cg != CgTy::Enum(option_codegen_ty) {
            panic!("codegen_mir_cast_asq: MIR verifier accepted invalid `as?` result type");
        }

        let (obj_ptr, _) = self.codegen_mir_runtime_ref_operand(span, value, slots)?;
        let is_ok = self.codegen_mir_runtime_type_test_is_ok(
            span,
            value,
            &metadata.test,
            mir_types,
            slots,
        )?;
        self.codegen_checked_runtime_ref_cast_option(
            span,
            obj_ptr,
            target_codegen_ty,
            target_value_cg,
            option_codegen_ty,
            is_ok,
        )
    }

    pub(in crate::llvm::codegen) fn codegen_mir_runtime_type_test_is_ok(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Operand,
        metadata: &crate::mir::RuntimeTypeTestMetadata,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<inkwell::values::IntValue<'ctx>, LlvmEmitError> {
        match metadata.static_fold {
            crate::mir::RuntimeTypeStaticFold::AlwaysTrue => {
                return Ok(self.context.bool_type().const_int(1, false));
            }
            crate::mir::RuntimeTypeStaticFold::AlwaysFalse => {
                return Ok(self.context.bool_type().const_int(0, false));
            }
            crate::mir::RuntimeTypeStaticFold::Dynamic => {}
        }

        if !self.runtime_type_descriptor_is_codegen_supported(mir_types, metadata) {
            panic!(
                "codegen_mir_runtime_type_test_is_ok: MIR verifier accepted unsupported runtime type descriptor"
            );
        }
        let target_ty = self
            .equivalent_runtime_ref_codegen_type_id(mir_types, metadata.target_ty)
            .unwrap_or_else(|| panic!("codegen_mir_runtime_type_test_is_ok: TypeStore equivalence verifier accepted unsupported runtime type target"));
        let (obj_ptr, _) = self.codegen_mir_runtime_ref_operand(span, value, slots)?;
        self.codegen_ref_is_instance_of(span, obj_ptr, target_ty)
    }

    pub(in crate::llvm::codegen) fn codegen_mir_runtime_ref_operand(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Operand,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<(PointerValue<'ctx>, CgValue<'ctx>), LlvmEmitError> {
        let value = self.codegen_mir_operand(span, value, slots)?;
        let value = match value.ty {
            CgTy::Ref => value,
            CgTy::String => self.coerce_value(span, value, CgTy::Ref)?,
            _ => {
                panic!(
                    "codegen_mir_runtime_ref_operand: MIR verifier accepted runtime type operand with non-reference codegen type"
                );
            }
        };
        let Some(BasicValueEnum::PointerValue(obj_ptr)) = value.value else {
            panic!(
                "codegen_mir_runtime_ref_operand: MIR verifier accepted valueless runtime type operand"
            );
        };
        Ok((obj_ptr, value))
    }

    pub(in crate::llvm::codegen) fn runtime_cast_target_ptr_type(
        &self,
        _span: crate::span::Span,
        target_cg: CgTy,
    ) -> Result<inkwell::types::PointerType<'ctx>, LlvmEmitError> {
        match target_cg {
            CgTy::Ref => Ok(self.llvm_gc_i8_ptr_type()),
            CgTy::String => Ok(self.llvm_scoop_string_ptr_type()),
            _ => panic!(
                "runtime_cast_target_ptr_type: MIR verifier accepted non-runtime-ref cast target type"
            ),
        }
    }

    pub(in crate::llvm::codegen) fn codegen_checked_runtime_ref_cast(
        &mut self,
        span: crate::span::Span,
        obj_ptr: PointerValue<'ctx>,
        _target_ty: TypeId,
        target_cg: CgTy,
        is_ok: inkwell::values::IntValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let target_ptr_ty = self.runtime_cast_target_ptr_type(span, target_cg)?;
        let func = self.expect_current_function("MIR checked cast branch blocks");

        let ok_bb = self.context.append_basic_block(func, "mir_cast_ok");
        let fail_bb = self.context.append_basic_block(func, "mir_cast_fail");
        let merge_bb = self.context.append_basic_block(func, "mir_cast_merge");
        self.builder
            .build_conditional_branch(is_ok, ok_bb, fail_bb)?;

        self.builder.position_at_end(ok_bb);
        let casted_ptr = self
            .builder
            .build_pointer_cast(obj_ptr, target_ptr_ty, "mir_cast_ptr")?;
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(fail_bb);
        self.emit_raise_runtime_error_variant(span, "ClassCastFailed")?;
        let fail_incoming = if self.ordinary_effect_propagation_enabled() {
            self.emit_ordinary_non_resuming_effect_exit(span, "mir_cast_raise_effect")?;
            self.builder.build_unreachable()?;
            None
        } else {
            let dead_bb = self.expect_insert_block("MIR checked cast failure continuation");
            let default_ptr = target_ptr_ty.const_null();
            self.builder.build_unconditional_branch(merge_bb)?;
            Some((default_ptr, dead_bb))
        };

        self.builder.position_at_end(merge_bb);
        let phi = self.builder.build_phi(target_ptr_ty, "mir_cast_value")?;
        if let Some((default_ptr, dead_bb)) = fail_incoming {
            phi.add_incoming(&[(&casted_ptr, ok_bb), (&default_ptr, dead_bb)]);
        } else {
            phi.add_incoming(&[(&casted_ptr, ok_bb)]);
        }
        Ok(CgValue {
            ty: target_cg,
            value: Some(phi.as_basic_value().into_pointer_value().into()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_checked_runtime_ref_cast_option(
        &mut self,
        span: crate::span::Span,
        obj_ptr: PointerValue<'ctx>,
        _target_ty: TypeId,
        target_cg: CgTy,
        option_ty: TypeId,
        is_ok: inkwell::values::IntValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let target_ptr_ty = self.runtime_cast_target_ptr_type(span, target_cg)?;
        let func = self.expect_current_function("MIR nullable cast branch blocks");

        let ok_bb = self.context.append_basic_block(func, "mir_asq_ok");
        let fail_bb = self.context.append_basic_block(func, "mir_asq_fail");
        let merge_bb = self.context.append_basic_block(func, "mir_asq_merge");
        self.builder
            .build_conditional_branch(is_ok, ok_bb, fail_bb)?;

        self.builder.position_at_end(ok_bb);
        let casted_ptr =
            self.builder
                .build_pointer_cast(obj_ptr, target_ptr_ty, "mir_asq_cast_ptr")?;
        let casted = CgValue {
            ty: target_cg,
            value: Some(casted_ptr.into()),
        };
        let payload = self.coerce_enum_payload(span, casted, target_cg)?;
        let some_v = self.build_enum_value(span, option_ty, 0, payload)?;
        let some_raw = some_v.value.unwrap_or_else(|| {
            panic!(
                "codegen_checked_runtime_ref_cast_option: verified Option Some produced no value"
            )
        });
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(fail_bb);
        let none_v = self.build_enum_value(span, option_ty, 1, CgEnumPayload::default())?;
        let none_raw = none_v.value.unwrap_or_else(|| {
            panic!(
                "codegen_checked_runtime_ref_cast_option: verified Option None produced no value"
            )
        });
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(merge_bb);
        let llvm_option_ty = self.llvm_enum_value_type(span, option_ty)?;
        let phi = self.builder.build_phi(llvm_option_ty, "mir_asq_value")?;
        phi.add_incoming(&[(&some_raw, ok_bb), (&none_raw, fail_bb)]);
        Ok(CgValue {
            ty: CgTy::Enum(option_ty),
            value: Some(phi.as_basic_value()),
        })
    }
}
