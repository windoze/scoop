//! Runtime-error effect machinery: nominal/effect lookup, raise variant emission, composite transport boxing.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn effect_nominal(&self, ty: TypeId) -> Option<&NominalType> {
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.types.kind(ty) else {
            return None;
        };
        if !matches!(
            self.nominal_kinds.get(&nominal.fqn),
            Some(ast::TypeKind::Effect)
        ) {
            return None;
        }
        Some(nominal)
    }

    pub(in crate::llvm::codegen) fn is_runtime_error_type(&self, ty: TypeId) -> bool {
        match self.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
            | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                nominal.fqn == "scoop.core.RuntimeError"
            }
            _ => false,
        }
    }

    pub(in crate::llvm::codegen) fn is_raise_runtime_error_effect(
        &self,
        effect_ty: TypeId,
    ) -> bool {
        let Some(nominal) = self.effect_nominal(effect_ty) else {
            return false;
        };
        nominal.fqn == "scoop.core.Raise"
            && nominal.args.len() == 1
            && self.is_runtime_error_type(nominal.args[0])
    }

    pub(in crate::llvm::codegen) fn known_effect_instance_types_for_fqn(
        &self,
        effect_fqn: &str,
    ) -> Vec<TypeId> {
        let mut ids = self
            .known_effect_instances_by_effect_fqn
            .get(effect_fqn)
            .cloned()
            .unwrap_or_default();

        ids.extend(self.types.iter_ids().filter(|type_id| {
            let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.types.kind(*type_id) else {
                return false;
            };
            nominal.fqn == effect_fqn
                && matches!(
                    self.nominal_kinds.get(&nominal.fqn),
                    Some(ast::TypeKind::Effect)
                )
        }));

        ids.sort_by(|lhs, rhs| {
            let lhs_display = self.types.display(*lhs).to_string();
            let rhs_display = self.types.display(*rhs).to_string();
            lhs_display.cmp(&rhs_display).then_with(|| lhs.cmp(rhs))
        });
        ids.dedup();
        ids
    }

    pub(in crate::llvm::codegen) fn effect_instance_key(&self, effect_ty: TypeId) -> Option<u32> {
        if self.is_raise_runtime_error_effect(effect_ty) {
            return Some(EFFECT_INSTANCE_KEY_RAISE_RUNTIME_ERROR);
        }

        let nominal = self.effect_nominal(effect_ty)?;
        self.known_effect_instance_types_for_fqn(&nominal.fqn)
            .iter()
            .position(|candidate| *candidate == effect_ty)
            .and_then(|index| u32::try_from(index).ok())
    }

    #[allow(dead_code)]
    pub(in crate::llvm::codegen) fn effect_instance_key_for_family(
        &self,
        family: &crate::effect_facts::EffectFamilyKey,
    ) -> Option<u32> {
        if family.effect_fqn() == "scoop.core.Raise"
            && family.type_args().len() == 1
            && self.is_runtime_error_type(family.type_args()[0])
        {
            return Some(EFFECT_INSTANCE_KEY_RAISE_RUNTIME_ERROR);
        }

        self.known_effect_instance_types_for_fqn(family.effect_fqn())
            .iter()
            .position(|candidate| {
                self.effect_nominal(*candidate)
                    .is_some_and(|nominal| nominal.args.as_slice() == family.type_args())
            })
            .and_then(|index| u32::try_from(index).ok())
    }

    pub(in crate::llvm::codegen) fn raise_runtime_error_effect_ty(&self) -> Option<TypeId> {
        self.known_effect_instance_types_for_fqn("scoop.core.Raise")
            .into_iter()
            .find(|type_id| self.is_raise_runtime_error_effect(*type_id))
    }

    pub(in crate::llvm::codegen) fn box_composite_effect_transport_value(
        &mut self,
        at: crate::span::Span,
        source_ty: TypeId,
        source: CgValue<'ctx>,
        label: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let deferred_source =
            self.defer_gc_sensitive_cg_value(at, &format!("{label}_source"), source)?;
        let box_obj_ty = self.mir_value_box_object_type(at, source_ty, source.ty)?;
        let obj_size_bytes = self.target_data.get_store_size(&box_obj_ty);
        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);
        let box_desc =
            self.get_or_create_mir_value_box_type_desc_global(at, source_ty, box_obj_ty)?;
        let box_desc_i8 = self.builder.build_pointer_cast(
            box_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            &format!("{label}_desc_i8"),
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            at,
            rt_alloc,
            &[box_desc_i8.into(), size_v.into()],
            &format!("rt_alloc_{label}"),
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect transport value box return value",
                at: at.into(),
            })?;
        let BasicValueEnum::PointerValue(obj_i8) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect transport value box return type",
                at: at.into(),
            });
        };

        let obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let obj_ptr =
            self.builder
                .build_pointer_cast(obj_i8, obj_ptr_ty, &format!("{label}_obj_ptr"))?;
        let deferred_obj = self.defer_gc_ref_pointer(at, &format!("{label}_obj_root"), obj_ptr)?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            at,
            &format!("{label}_obj_reload"),
            &deferred_obj,
        )?;
        let payload_gep = self.builder.build_struct_gep(
            box_obj_ty,
            obj_ptr,
            1,
            &format!("{label}_payload_gep"),
        )?;
        let payload = self.materialize_deferred_cg_value(
            at,
            &format!("{label}_source_reload"),
            deferred_source,
        )?;
        let _ = self.store_local_value(at, payload_gep, source.ty, payload)?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            at,
            &format!("{label}_return"),
            &deferred_obj,
        )?;
        let gc_ref = self.builder.build_pointer_cast(
            obj_ptr,
            self.llvm_gc_i8_ptr_type(),
            &format!("{label}_gc_ref"),
        )?;
        self.clear_deferred_cg_value_root_homes(
            at,
            &format!("{label}_obj_root_drop"),
            &deferred_obj,
        )?;
        Ok(gc_ref)
    }

    pub(in crate::llvm::codegen) fn emit_raise_runtime_error_variant(
        &mut self,
        span: crate::span::Span,
        variant_name: &str,
    ) -> Result<(), LlvmEmitError> {
        let outcome_ptr = self.function_cx.current_effect_outcome_ptr.ok_or_else(|| {
            LlvmEmitError::Frontend {
                message: format!(
                    "direct runtime-error raise `{variant_name}` 缺少当前 explicit EffectOutcome 槽位；该路径应由 published late-lowered/local-effect-control handoff 接管"
                ),
            }
        })?;
        let raise_runtime_error_effect = self.raise_runtime_error_effect_ty().ok_or_else(|| {
            LlvmEmitError::Frontend {
                message: "缺少 Raise<RuntimeError> effect type；HIR/MIR runtime-error lowering contract 未闭合"
                    .to_string(),
            }
        })?;
        let effect_instance_key = self
            .effect_instance_key(raise_runtime_error_effect)
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: "Raise<RuntimeError> effect instance key 未发布到 active codegen contract"
                    .to_string(),
            })?;
        let variant_fqn = format!("scoop.core.RuntimeError.{variant_name}");
        let payload_value = self
            .try_codegen_qualified_enum_unit_variant_value(span, &variant_fqn)?
            .unwrap_or_else(|| {
                panic!("emit_raise_runtime_error_variant: verifier accepted missing RuntimeError unit variant `{variant_name}`")
            });
        let CgTy::Enum(payload_ty) = payload_value.ty else {
            panic!(
                "emit_raise_runtime_error_variant: TypeStore equivalence verifier accepted non-enum RuntimeError payload type"
            );
        };
        let payload_gc_ref = self.box_composite_effect_transport_value(
            span,
            payload_ty,
            payload_value,
            "raise_runtime_error_payload",
        )?;
        let zero_transport = effect_outcome::ValueTransportParts {
            word: self.context.i64_type().const_zero(),
            gc_ref: self.llvm_gc_i8_ptr_type().const_null(),
        };
        let raise_op_tag = self.effect_op_tag("scoop.core.Raise.raise");
        let signal = self.build_effect_signal(
            self.context
                .i32_type()
                .const_int(u64::from(raise_op_tag), false),
            self.context
                .i32_type()
                .const_int(u64::from(effect_instance_key), false),
            effect_outcome::ValueTransportParts {
                word: self.context.i64_type().const_zero(),
                gc_ref: payload_gc_ref,
            },
            self.llvm_gc_i8_ptr_type().const_null(),
        )?;
        let outcome = self.build_effect_outcome(
            effect_outcome::EffectOutcomeTag::Propagate,
            zero_transport,
            signal,
        )?;
        self.builder.build_store(outcome_ptr, outcome)?;
        Ok(())
    }
}
