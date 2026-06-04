//! LIR-native effect-typed closure adapter selection and emission.

use std::collections::BTreeSet;

use inkwell::module::Linkage;
use inkwell::types::FunctionType;
use inkwell::values::{BasicMetadataValueEnum, FunctionValue, PointerValue};
use scoopc_ids::LirCallableId;

use crate::effect_lowered::mir_source as mir;
use crate::effect_lowered::{
    LirCallKind, LirExecutableBody, LirOperand, LirRvalue, LirStatementKind, LirStructLitField,
};
use crate::llvm::LlvmEmitError;
use crate::span::Span;
use crate::stable_id::canonical_record;
use crate::ty::{MonoTypeId, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::super::mir_body::MirLocalSlot;
use super::super::types::CgTy;
use super::super::{CallableCarrierKind, MainCodegen};
use super::stable_naming;
use super::types::{ProgramAbiQuery, SourceAbiLayoutKind, StepLayout};

#[derive(Clone, Copy)]
struct ClosureSurfaceLayout<'ctx> {
    llvm_ty: FunctionType<'ctx>,
    invoke_args_tuple_ty: TypeId,
    return_step_schema: crate::effect_facts::StepSchemaId,
}

type EffectFamilyMatchKey = (String, Vec<TypeId>);

fn frontend_error(message: String) -> LlvmEmitError {
    LlvmEmitError::Frontend { message }
}

fn function_type_source_args(fun_ty: &crate::ty::FunctionType) -> Vec<TypeId> {
    fun_ty
        .receiver
        .into_iter()
        .chain(fun_ty.params.iter().copied())
        .collect()
}

fn source_carrier_types(types: &TypeStore, carrier_ty: TypeId) -> Option<Vec<TypeId>> {
    match types.kind(carrier_ty) {
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => Some(elements.clone()),
        TypeKind::Value(ValueTypeKind::Unit) => Some(Vec::new()),
        _ => Some(vec![carrier_ty]),
    }
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    /// Selects an adapter fn pointer for a LIR closure value assigned to an effect-typed surface.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn maybe_build_lir_effect_typed_closure_target_fn_ptr(
        &mut self,
        span: Span,
        abi: &ProgramAbiQuery<'ctx>,
        source_types: &TypeStore,
        body: &LirExecutableBody,
        target_local: Option<crate::effect_lowered::mir_source::LocalId>,
        fn_ptr: LirCallableId,
    ) -> Result<Option<PointerValue<'ctx>>, LlvmEmitError> {
        let mut surface_tys = Vec::new();
        if let Some(target_ty) = target_local
            .and_then(|local| body.locals().get(local.as_u32() as usize))
            .map(|local| local.ty())
        {
            surface_tys.push(target_ty);
        }
        if let Some(target_local) = target_local
            && let Some(consumer_ty) = self.lir_local_function_value_consumer_surface_ty(
                abi,
                source_types,
                body,
                target_local,
            )?
            && !surface_tys.contains(&consumer_ty)
        {
            surface_tys.push(consumer_ty);
        }
        for surface_ty in surface_tys {
            if let Some(ptr) = self
                .maybe_build_lir_effect_typed_closure_target_fn_ptr_for_source_ty(
                    span,
                    abi,
                    source_types,
                    surface_ty,
                    fn_ptr,
                )?
            {
                return Ok(Some(ptr));
            }
        }
        Ok(None)
    }

    pub(in crate::llvm::codegen) fn lir_local_make_closure_source(
        &self,
        body: &LirExecutableBody,
        local: crate::effect_lowered::mir_source::LocalId,
    ) -> Option<(LirOperand, LirCallableId, mir::ClosureEnvTransportMetadata)> {
        body.states().states().iter().find_map(|state| {
            state.body().statements().iter().find_map(|stmt| {
                let LirStatementKind::Assign { target, value } = &stmt.kind else {
                    return None;
                };
                if *target != local {
                    return None;
                }
                let LirRvalue::MakeClosure {
                    env,
                    fn_ptr,
                    env_contract,
                } = value
                else {
                    return None;
                };
                Some((env.clone(), *fn_ptr, env_contract.clone()))
            })
        })
    }

    /// Installs effect-typed adapter overrides for closure objects stored in LIR struct fields.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn install_lir_effect_typed_closure_target_overrides_for_struct_fields(
        &mut self,
        span: Span,
        abi: &ProgramAbiQuery<'ctx>,
        source_types: &TypeStore,
        body: &LirExecutableBody,
        fields: &[LirStructLitField],
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<(), LlvmEmitError> {
        let CgTy::Struct(struct_ty) = target_cg else {
            return Ok(());
        };
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(struct_ty.inner())
        else {
            return Ok(());
        };
        let layout_key = self.nominal_layout_key(nominal);
        let layout = self.struct_layouts.get(&layout_key).unwrap_or_else(|| {
            panic!(
                "install_lir_effect_typed_closure_target_overrides_for_struct_fields: layout verifier accepted missing struct layout at {span:?}"
            )
        });
        let layout_fields = layout
            .fields
            .iter()
            .map(|field| (field.name.clone(), field.ty))
            .collect::<Vec<_>>();
        for (field_name, field_ty) in layout_fields {
            let Some(init) = fields.iter().find(|field| field.name == field_name) else {
                continue;
            };
            let LirOperand::Local(source_local) = init.value else {
                continue;
            };
            let Some((_env, fn_ptr, _env_contract)) =
                self.lir_local_make_closure_source(body, source_local)
            else {
                continue;
            };
            let Some(field_ty) = field_ty else {
                continue;
            };
            let Some(source_field_ty) =
                self.source_type_matching_codegen_ty(source_types, field_ty)
            else {
                continue;
            };
            let Some(adapter) = self
                .maybe_build_lir_effect_typed_closure_target_fn_ptr_for_source_ty(
                    init.span,
                    abi,
                    source_types,
                    source_field_ty,
                    fn_ptr,
                )?
            else {
                continue;
            };
            self.store_lir_closure_dynamic_entry(init.span, &init.value, slots, adapter)?;
        }
        Ok(())
    }

    fn lir_local_function_value_consumer_surface_ty(
        &self,
        abi: &ProgramAbiQuery<'ctx>,
        source_types: &TypeStore,
        body: &LirExecutableBody,
        local: crate::effect_lowered::mir_source::LocalId,
    ) -> Result<Option<TypeId>, LlvmEmitError> {
        let mut matched: Option<TypeId> = None;
        for state in body.states().states() {
            for stmt in state.body().statements() {
                let LirStatementKind::Assign { value, .. } = &stmt.kind else {
                    continue;
                };
                let Some(surface_ty) =
                    self.lir_call_arg_function_surface_ty(abi, source_types, value, local)?
                else {
                    continue;
                };
                if let Some(existing) = matched {
                    if existing != surface_ty {
                        return Err(frontend_error(format!(
                            "LIR closure local{} 被多个不兼容的 function surface 消费：t{} 与 t{}",
                            local.as_u32(),
                            existing.as_u32(),
                            surface_ty.as_u32(),
                        )));
                    }
                } else {
                    matched = Some(surface_ty);
                }
            }
        }
        Ok(matched)
    }

    fn lir_call_arg_function_surface_ty(
        &self,
        abi: &ProgramAbiQuery<'ctx>,
        source_types: &TypeStore,
        value: &LirRvalue,
        local: crate::effect_lowered::mir_source::LocalId,
    ) -> Result<Option<TypeId>, LlvmEmitError> {
        let LirRvalue::Call { kind, args, .. } = value else {
            return Ok(None);
        };
        let Some(arg_index) = args.iter().position(
            |arg| matches!(&arg.value, LirOperand::Local(candidate) if *candidate == local),
        ) else {
            return Ok(None);
        };
        let surface_ty = match kind {
            LirCallKind::Direct { callee, .. } => callee
                .local_id()
                .and_then(|id| self.published_late_lowered_program()?.callable_by_id(id))
                .and_then(|callable| {
                    if let Ok(layout) =
                        abi.callable_layout_by_version_key(callable.body_version_key())
                    {
                        source_carrier_types(
                            source_types,
                            layout.direct_entry().invoke_args_tuple_ty(),
                        )
                        .and_then(|tys| tys.get(arg_index).copied())
                    } else if let Ok(layout) =
                        abi.plain_callable_layout_by_version_key(callable.body_version_key())
                    {
                        layout.direct_entry().param_tys().get(arg_index).copied()
                    } else {
                        None
                    }
                }),
            LirCallKind::Closure { .. }
            | LirCallKind::FunValue { .. }
            | LirCallKind::FunPtr { .. }
            | LirCallKind::Virtual { .. }
            | LirCallKind::Interface { .. }
            | LirCallKind::Resume { .. } => None,
        };
        Ok(surface_ty.filter(|ty| {
            matches!(
                source_types.kind(*ty),
                TypeKind::Ref(RefTypeKind::Function(_))
            )
        }))
    }

    fn maybe_build_lir_effect_typed_closure_target_fn_ptr_for_source_ty(
        &mut self,
        span: Span,
        abi: &ProgramAbiQuery<'ctx>,
        source_types: &TypeStore,
        target_ty: TypeId,
        fn_ptr: LirCallableId,
    ) -> Result<Option<PointerValue<'ctx>>, LlvmEmitError> {
        let TypeKind::Ref(RefTypeKind::Function(surface_fun_ty)) = source_types.kind(target_ty)
        else {
            return Ok(None);
        };
        let Some(fun_ty) = self.equivalent_codegen_function_type(source_types, surface_fun_ty)
        else {
            panic!(
                "maybe_build_lir_effect_typed_closure_target_fn_ptr_for_source_ty: TypeStore equivalence verifier accepted non-codegen effect-typed surface function at {span:?}"
            );
        };
        if fun_ty.effects.is_pure() {
            return Ok(None);
        }
        let Some(layout) = self.effect_typed_closure_surface_layout(abi, source_types, &fun_ty)?
        else {
            return Ok(None);
        };
        let fn_root = self.lir_callable_root_for_closure_adapter(fn_ptr, span)?;
        if let Some(source_target) =
            abi.maybe_callable_carrier_target_layout(CallableCarrierKind::ClosureObject, &fn_root)
        {
            let source_step_schema = source_target.step_schema();
            let source_symbol_name = source_target.symbol_name().to_string();
            if source_step_schema == layout.return_step_schema {
                return Ok(None);
            }
            return self
                .build_effect_typed_effectful_closure_adapter(
                    span,
                    abi,
                    &fn_root,
                    layout,
                    source_step_schema,
                    &source_symbol_name,
                )
                .map(Some);
        }
        if abi
            .maybe_plain_callable_layout_by_root_fqn(&fn_root)?
            .is_some()
        {
            return self
                .build_effect_typed_plain_closure_adapter(span, abi, &fn_root, &fun_ty, layout)
                .map(Some);
        }
        Err(frontend_error(format!(
            "effect-typed LIR closure surface `{}` 缺少 published closure carrier target 或 plain callable layout",
            fn_root,
        )))
    }

    fn lir_callable_root_for_closure_adapter(
        &self,
        fn_ptr: LirCallableId,
        span: Span,
    ) -> Result<String, LlvmEmitError> {
        let program = self.published_late_lowered_program().ok_or_else(|| {
            frontend_error(format!(
                "LIR closure adapter at {span:?} requires a published LIR program"
            ))
        })?;
        let callable = program.callable_by_id(fn_ptr).ok_or_else(|| {
            frontend_error(format!(
                "LIR closure adapter at {span:?} references unknown callable id {fn_ptr:?}"
            ))
        })?;
        Ok(callable.root_fqn().to_string())
    }

    fn source_type_matching_codegen_ty(
        &self,
        source_types: &TypeStore,
        codegen_ty: MonoTypeId,
    ) -> Option<TypeId> {
        let display = self.types.display(codegen_ty.inner()).to_string();
        source_types
            .iter_ids()
            .find(|&ty| source_types.display(ty).to_string() == display)
    }

    fn store_lir_closure_dynamic_entry(
        &mut self,
        span: Span,
        closure_operand: &LirOperand,
        slots: &[MirLocalSlot<'ctx>],
        fn_ptr: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let closure =
            self.codegen_lir_operand_expected(span, closure_operand, slots, Some(CgTy::Ref))?;
        let closure = self.coerce_value(span, closure, CgTy::Ref)?;
        let raw_closure = self.expect_cg_pointer(
            closure,
            "store_lir_closure_dynamic_entry struct closure adapter value",
        );
        let closure_ptr = self.cast_ptr(
            raw_closure,
            self.llvm_ptr_type(self.gc_address_space()),
            "lir_struct_closure_adapter_obj",
        )?;
        let fn_gep = self.builder.build_struct_gep(
            self.llvm_closure_object_type(),
            closure_ptr,
            2,
            "lir_struct_closure_adapter_fn_gep",
        )?;
        let _ = self.builder.build_store(fn_gep, fn_ptr)?;
        Ok(())
    }

    fn effect_typed_closure_surface_layout(
        &self,
        abi: &ProgramAbiQuery<'ctx>,
        source_types: &TypeStore,
        fun_ty: &crate::ty::FunctionType,
    ) -> Result<Option<ClosureSurfaceLayout<'ctx>>, LlvmEmitError> {
        let expected_args = function_type_source_args(fun_ty);
        let expected_effect_families = self.effect_row_family_match_keys(&fun_ty.effects)?;
        let mut matches = abi.dynamic_invoke_layouts().filter_map(|layout| {
            let args = source_carrier_types(source_types, layout.invoke_args_tuple_ty())?
                .into_iter()
                .map(|ty| self.equivalent_codegen_type_id(source_types, ty))
                .collect::<Option<Vec<_>>>()?;
            if args != expected_args {
                return None;
            }
            let step_layout = abi.step_layout(layout.return_step_schema())?;
            let effect_families =
                self.step_layout_effect_family_match_keys(source_types, step_layout)?;
            if effect_families != expected_effect_families {
                return None;
            }
            let payload_ty = self.equivalent_codegen_type_id(
                source_types,
                step_layout.complete_variant().payload_source_ty(),
            )?;
            (payload_ty == fun_ty.return_ty).then_some(ClosureSurfaceLayout {
                llvm_ty: layout.llvm_ty(),
                invoke_args_tuple_ty: layout.invoke_args_tuple_ty(),
                return_step_schema: layout.return_step_schema(),
            })
        });
        let Some(first) = matches.next() else {
            return Ok(None);
        };
        let ambiguous = matches.any(|candidate| {
            candidate.return_step_schema != first.return_step_schema
                || candidate.invoke_args_tuple_ty != first.invoke_args_tuple_ty
                || candidate.llvm_ty != first.llvm_ty
        });
        if ambiguous {
            return Err(frontend_error(format!(
                "effect-typed LIR closure surface function type args={:?} effects={:?} return=t{} 匹配多个 dynamic-invoke layout",
                expected_args
                    .iter()
                    .map(|ty| ty.as_u32())
                    .collect::<Vec<_>>(),
                expected_effect_families,
                fun_ty.return_ty.as_u32(),
            )));
        }
        Ok(Some(first))
    }

    fn effect_row_family_match_keys(
        &self,
        row: &crate::ty::EffectRow,
    ) -> Result<BTreeSet<EffectFamilyMatchKey>, LlvmEmitError> {
        let mut families = BTreeSet::new();
        for effect_ty in &row.terms {
            let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.types.kind(*effect_ty) else {
                return Err(frontend_error(format!(
                    "effect-typed LIR closure adapter effect row term t{} is not a nominal effect type",
                    effect_ty.as_u32()
                )));
            };
            families.insert((nominal.fqn.clone(), nominal.args.clone()));
        }
        Ok(families)
    }

    fn step_layout_effect_family_match_keys(
        &self,
        source_types: &TypeStore,
        step_layout: &StepLayout<'ctx>,
    ) -> Option<BTreeSet<EffectFamilyMatchKey>> {
        let mut families = BTreeSet::new();
        for case in step_layout.cases().values() {
            let family = case.concrete_op_key().effect_family();
            let type_args = family
                .type_args()
                .iter()
                .map(|ty| self.equivalent_codegen_type_id(source_types, *ty))
                .collect::<Option<Vec<_>>>()?;
            families.insert((family.effect_fqn().to_string(), type_args));
        }
        Some(families)
    }

    fn build_effect_typed_plain_closure_adapter(
        &mut self,
        span: Span,
        abi: &ProgramAbiQuery<'ctx>,
        fn_ptr: &str,
        fun_ty: &crate::ty::FunctionType,
        adapter: ClosureSurfaceLayout<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let plain = abi.plain_callable_layout_by_root_fqn(fn_ptr)?;
        let return_step_layout = abi.step_layout(adapter.return_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "effect-typed LIR plain adapter `{}` 缺少 return step schema s{} layout",
                fn_ptr,
                adapter.return_step_schema.as_u32(),
            ))
        })?;
        let name = stable_naming::private_name_from_key_text(
            "plain_adapter",
            &canonical_record(
                "plain_adapter",
                [
                    plain.stable_callable_key_text().to_string(),
                    return_step_layout.stable_effect_key_text().to_string(),
                ],
            ),
        );
        if let Some(existing) = self.module.get_function(&name) {
            if existing.count_basic_blocks() == 0 {
                self.define_effect_typed_plain_closure_adapter(
                    span, abi, fn_ptr, fun_ty, adapter, existing,
                )?;
            }
            return Ok(existing.as_global_value().as_pointer_value());
        }
        let function = self.declare_compiler_private_helper_function(
            &name,
            adapter.llvm_ty,
            Linkage::Internal,
        );
        self.define_effect_typed_plain_closure_adapter(
            span, abi, fn_ptr, fun_ty, adapter, function,
        )?;
        Ok(function.as_global_value().as_pointer_value())
    }

    fn define_effect_typed_plain_closure_adapter(
        &mut self,
        span: Span,
        abi: &ProgramAbiQuery<'ctx>,
        fn_ptr: &str,
        _fun_ty: &crate::ty::FunctionType,
        adapter: ClosureSurfaceLayout<'ctx>,
        function: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let saved_block = self.builder.get_insert_block();
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);

        let plain = abi.plain_callable_layout_by_root_fqn(fn_ptr)?;
        let plain_fun = self
            .module
            .get_function(plain.direct_entry().symbol_name())
            .ok_or_else(|| {
                frontend_error(format!(
                    "effect-typed LIR plain adapter `{}` 缺少 plain entry `{}`",
                    fn_ptr,
                    plain.direct_entry().symbol_name(),
                ))
            })?;
        let step_layout = abi.step_layout(adapter.return_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "effect-typed LIR plain adapter 缺少 return step schema s{} layout",
                adapter.return_step_schema.as_u32(),
            ))
        })?;
        let complete_variant = step_layout.complete_variant();
        let complete_payload_ty = if complete_variant.payload_is_elided() {
            None
        } else {
            Some(complete_variant.payload_ty().get_field_type_at_index(0).ok_or_else(|| {
                frontend_error(format!(
                    "effect-typed LIR plain adapter Step complete payload `{}` 缺少 field#0",
                    complete_variant.payload_anchor_name(),
                ))
            })?)
        };

        let carrier = function
            .get_nth_param(0)
            .unwrap_or_else(|| {
                panic!(
                    "define_effect_typed_lir_plain_closure_adapter: closure adapter ABI accepted missing carrier param at {span:?}"
                )
            })
            .into_pointer_value();
        let closure_ptr = self.cast_ptr(
            carrier,
            self.llvm_ptr_type(self.gc_address_space()),
            "lir_adapter_closure_obj",
        )?;
        let env_gep = self.builder.build_struct_gep(
            self.llvm_closure_object_type(),
            closure_ptr,
            1,
            "lir_adapter_env_gep",
        )?;
        let env = self
            .builder
            .build_load(self.llvm_gc_i8_ptr_type(), env_gep, "lir_adapter_env")?
            .into_pointer_value();
        let explicit_args =
            self.adapter_explicit_args(span, abi, function, adapter.invoke_args_tuple_ty)?;
        let plain_arg_count_without_sret = 1 + explicit_args.len();
        let uses_hidden_sret = match (plain.direct_entry().param_count(), complete_payload_ty) {
            (count, Some(_)) if count == plain_arg_count_without_sret + 1 => true,
            (count, _) if count == plain_arg_count_without_sret => false,
            (count, _) => {
                return Err(frontend_error(format!(
                    "effect-typed LIR plain adapter `{}` plain entry param count drift: entry={} expected={} or {}",
                    fn_ptr,
                    count,
                    plain_arg_count_without_sret,
                    plain_arg_count_without_sret + 1,
                )));
            }
        };

        let mut call_args = Vec::<BasicMetadataValueEnum<'ctx>>::new();
        let sret_result_slot = if uses_hidden_sret {
            let result_ty = complete_payload_ty.unwrap_or_else(|| {
                panic!(
                    "define_effect_typed_lir_plain_closure_adapter: closure adapter ABI accepted hidden sret without Complete payload type at {span:?}"
                )
            });
            let slot = self.create_entry_alloca_raw(span, "lir_adapter_plain_sret", result_ty)?;
            call_args.push(slot.into());
            Some((slot, result_ty))
        } else {
            None
        };
        call_args.push(env.into());
        call_args.extend(explicit_args);
        let call = self
            .builder
            .build_call(plain_fun, &call_args, "lir_carrier_to_plain")?;
        if let Some((_, result_ty)) = sret_result_slot {
            self.add_sret_attribute_to_call(call, 0, result_ty);
        }
        let payload = if let Some(expected_payload_ty) = complete_payload_ty {
            Some(if let Some((result_ptr, _)) = sret_result_slot {
                if self.basic_type_contains_gc_ptrs(span, expected_payload_ty)? {
                    self.sync_storage_slot_into_explicit_frame(
                        span,
                        result_ptr,
                        expected_payload_ty,
                        "lir_adapter_plain_sret",
                    )?;
                }
                let payload = self.builder.build_load(
                    expected_payload_ty,
                    result_ptr,
                    "lir_adapter_plain_sret_payload",
                )?;
                self.clear_spill_slot_root_homes(
                    span,
                    result_ptr,
                    expected_payload_ty,
                    "lir_adapter_plain_sret",
                )?;
                payload
            } else {
                let payload = call.try_as_basic_value().basic().unwrap_or_else(|| {
                    panic!(
                        "define_effect_typed_lir_plain_closure_adapter: closure adapter ABI accepted valueless plain return at {span:?}"
                    )
                });
                if payload.get_type() != expected_payload_ty {
                    return Err(frontend_error(format!(
                        "effect-typed LIR plain adapter `{}` direct payload type drift: expected {:?}, got {:?}",
                        fn_ptr,
                        expected_payload_ty,
                        payload.get_type(),
                    )));
                }
                payload
            })
        } else {
            None
        };
        let step = self
            .build_step_complete(step_layout, payload)
            .map_err(|err| frontend_error(format!("LIR adapter_complete failed: {err}")))?;
        self.builder.build_return(Some(&step))?;

        if let Some(saved) = saved_block {
            self.builder.position_at_end(saved);
        }
        Ok(())
    }

    fn build_effect_typed_effectful_closure_adapter(
        &mut self,
        span: Span,
        abi: &ProgramAbiQuery<'ctx>,
        fn_ptr: &str,
        adapter: ClosureSurfaceLayout<'ctx>,
        source_step_schema: crate::effect_facts::StepSchemaId,
        source_symbol_name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let source_step_layout = abi.step_layout(source_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "effectful LIR closure adapter `{}` 缺少 source step schema s{} layout",
                fn_ptr,
                source_step_schema.as_u32(),
            ))
        })?;
        let return_step_layout = abi.step_layout(adapter.return_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "effectful LIR closure adapter `{}` 缺少 return step schema s{} layout",
                fn_ptr,
                adapter.return_step_schema.as_u32(),
            ))
        })?;
        let name = stable_naming::private_name_from_key_text(
            "closure_step_adapter",
            &canonical_record(
                "closure_step_adapter",
                [
                    source_step_layout.stable_effect_key_text().to_string(),
                    return_step_layout.stable_effect_key_text().to_string(),
                ],
            ),
        );
        if let Some(existing) = self.module.get_function(&name) {
            if existing.count_basic_blocks() == 0 {
                self.define_effect_typed_effectful_closure_adapter(
                    span,
                    abi,
                    fn_ptr,
                    adapter,
                    source_step_schema,
                    source_symbol_name,
                    existing,
                )?;
            }
            return Ok(existing.as_global_value().as_pointer_value());
        }
        let function = self.declare_compiler_private_helper_function(
            &name,
            adapter.llvm_ty,
            Linkage::Internal,
        );
        self.define_effect_typed_effectful_closure_adapter(
            span,
            abi,
            fn_ptr,
            adapter,
            source_step_schema,
            source_symbol_name,
            function,
        )?;
        Ok(function.as_global_value().as_pointer_value())
    }

    #[allow(clippy::too_many_arguments)]
    fn define_effect_typed_effectful_closure_adapter(
        &mut self,
        span: Span,
        abi: &ProgramAbiQuery<'ctx>,
        fn_ptr: &str,
        adapter: ClosureSurfaceLayout<'ctx>,
        source_step_schema: crate::effect_facts::StepSchemaId,
        source_symbol_name: &str,
        function: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let saved_block = self.builder.get_insert_block();
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);

        let source_fun = self
            .module
            .get_function(source_symbol_name)
            .ok_or_else(|| {
                frontend_error(format!(
                    "effect-typed LIR closure adapter `{}` 缺少 source carrier entry `{}`",
                    fn_ptr, source_symbol_name,
                ))
            })?;
        let mut call_args = vec![function
            .get_nth_param(0)
            .unwrap_or_else(|| {
                panic!(
                    "define_effect_typed_lir_effectful_closure_adapter: closure adapter ABI accepted missing carrier param at {span:?}"
                )
            })
            .into()];
        if let Some(explicit_args) = function.get_nth_param(1) {
            call_args.push(explicit_args.into());
        }
        if source_fun.count_params() as usize != call_args.len() {
            return Err(frontend_error(format!(
                "effect-typed LIR closure adapter `{}` source carrier entry `{}` param count drift: entry={} expected={}",
                fn_ptr,
                source_symbol_name,
                source_fun.count_params(),
                call_args.len(),
            )));
        }
        let call = self
            .builder
            .build_call(source_fun, &call_args, "lir_carrier_to_effectful")?;
        let step = call.try_as_basic_value().basic().unwrap_or_else(|| {
            panic!(
                "define_effect_typed_lir_effectful_closure_adapter: source carrier entry returned no Step value at {span:?}"
            )
        });
        let step = if source_step_schema == adapter.return_step_schema {
            step
        } else {
            self.project_step_to_schema(abi, step, source_step_schema, adapter.return_step_schema)?
        };
        self.builder.build_return(Some(&step))?;

        if let Some(saved) = saved_block {
            self.builder.position_at_end(saved);
        }
        Ok(())
    }

    fn adapter_explicit_args(
        &mut self,
        span: Span,
        abi: &ProgramAbiQuery<'ctx>,
        function: FunctionValue<'ctx>,
        invoke_args_tuple_ty: TypeId,
    ) -> Result<Vec<BasicMetadataValueEnum<'ctx>>, LlvmEmitError> {
        if function.get_nth_param(1).is_none() {
            return Ok(Vec::new());
        }
        let layout = abi.source_value_layout(invoke_args_tuple_ty)?;
        if layout.abi().is_elided() {
            return Ok(Vec::new());
        }
        let raw = function.get_nth_param(1).unwrap_or_else(|| {
            panic!(
                "lir_adapter_explicit_args: closure adapter ABI accepted missing args payload param at {span:?}"
            )
        });
        match layout.kind() {
            SourceAbiLayoutKind::Scalar => Ok(vec![raw.into()]),
            SourceAbiLayoutKind::Tuple => {
                let tuple = raw.into_struct_value();
                let mut args = Vec::new();
                for field in layout.fields() {
                    let Some(index) = field.abi_field_index() else {
                        continue;
                    };
                    let value = self.builder.build_extract_value(
                        tuple,
                        index,
                        &format!("lir_adapter_arg{}", field.source_index()),
                    )?;
                    args.push(value.into());
                }
                Ok(args)
            }
        }
    }
}
