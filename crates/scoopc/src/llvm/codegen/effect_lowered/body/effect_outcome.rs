//! Effect-outcome construction: builds the EffectOutcome value returned by every effectful body and the corresponding effect-signal constants used by the propagating path.

use super::*;

impl<'cg, 'a, 'ctx> CallableEmitter<'cg, 'a, 'ctx> {
    pub(super) fn zero_transport_parts(&self) -> ValueTransportParts<'ctx> {
        ValueTransportParts {
            word: self.codegen.context.i64_type().const_zero(),
            gc_ref: self.codegen.llvm_gc_i8_ptr_type().const_null(),
        }
    }

    pub(super) fn effect_signal_constants_for_case(
        &mut self,
        case_layout: &StepCaseLayout<'ctx>,
    ) -> Result<(IntValue<'ctx>, IntValue<'ctx>), LlvmEmitError> {
        let effect_family = case_layout.concrete_op_key().effect_family();
        let op_tag = self.codegen.context.i32_type().const_int(
            u64::from(self.codegen.effect_op_tag(effect_family.effect_fqn())),
            false,
        );
        let effect_instance_key = if effect_family.effect_fqn() == "scoop.core.Raise"
            && effect_family.type_args().len() == 1
            && self.source_ty_is_runtime_error(effect_family.type_args()[0])
        {
            EFFECT_INSTANCE_KEY_RAISE_RUNTIME_ERROR
        } else {
            let mapped_effect_args = effect_family
                .type_args()
                .iter()
                .map(|ty| {
                    self.codegen
                        .equivalent_codegen_type_id(self.source_types, *ty)
                        .ok_or_else(|| {
                            frontend_error(format!(
                                "step schema s{} case c{} effect family type arg t{} 缺少 codegen 等价类型",
                                self.abi_step_schema.as_u32(),
                                case_layout.case_tag().as_u32(),
                                ty.as_u32(),
                            ))
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let effect_ty = self.codegen
                .types
                .iter_ids()
                .find(|type_id| {
                    matches!(
                        self.codegen.types.kind(*type_id),
                        TypeKind::Ref(RefTypeKind::Nominal(nominal))
                            if nominal.fqn == effect_family.effect_fqn()
                                && nominal.args.as_slice() == mapped_effect_args.as_slice()
                    )
                })
                .ok_or_else(|| {
                    frontend_error(format!(
                        "step schema s{} case c{} 缺少 effect family `{}` 的 codegen nominal type",
                        self.abi_step_schema.as_u32(),
                        case_layout.case_tag().as_u32(),
                        effect_family.effect_fqn(),
                    ))
                })?;
            self.codegen.effect_instance_key(effect_ty).ok_or_else(|| {
                frontend_error(format!(
                    "step schema s{} case c{} 缺少可发布的 effect_instance_key",
                    self.abi_step_schema.as_u32(),
                    case_layout.case_tag().as_u32()
                ))
            })?
        };
        Ok((
            op_tag,
            self.codegen
                .context
                .i32_type()
                .const_int(u64::from(effect_instance_key), false),
        ))
    }

    pub(super) fn emit_effect_outcome_return_to_ptr(
        &mut self,
        outcome_ptr: PointerValue<'ctx>,
        outcome: StructValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        self.codegen.builder.build_store(outcome_ptr, outcome)?;
        self.codegen.builder.build_return(None)?;
        Ok(())
    }

    pub(super) fn emit_effect_outcome_return(
        &mut self,
        outcome: StructValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        self.sync_frame_slots_from_locals()?;
        let outcome_ptr = self.current_effect_outcome_ptr()?;
        self.emit_effect_outcome_return_to_ptr(outcome_ptr, outcome)
    }

    pub(super) fn build_complete_effect_outcome_from_payload_source(
        &mut self,
        payload_source: &LateLoweredCompletionPayloadSource,
    ) -> Result<StructValue<'ctx>, LlvmEmitError> {
        let payload_ty = self.step_layout.complete_variant().payload_source_ty();
        let payload = self.lower_completion_payload_as(payload_source, payload_ty)?;
        let payload = self.complete_payload_or_default(self.step_layout, payload)?;
        let complete =
            self.encode_effect_transport_parts(payload_ty, payload, "effect_outcome_complete")?;
        let zero_signal = self.codegen.build_effect_signal(
            self.codegen.context.i32_type().const_zero(),
            self.codegen.context.i32_type().const_zero(),
            self.zero_transport_parts(),
            self.codegen.llvm_gc_i8_ptr_type().const_null(),
        )?;
        self.codegen
            .build_effect_outcome(EffectOutcomeTag::Complete, complete, zero_signal)
    }

    pub(super) fn build_propagating_effect_outcome_for_case(
        &mut self,
        case_tag: CaseTag,
        payload: Option<BasicValueEnum<'ctx>>,
        payload_ty: TypeId,
        resume_token: PointerValue<'ctx>,
    ) -> Result<StructValue<'ctx>, LlvmEmitError> {
        let case_layout = self.step_layout.case_layout(case_tag).ok_or_else(|| {
            frontend_error(format!(
                "callable `{}` step schema s{} 缺少 outward case c{}",
                self.callable.root_fqn(),
                self.abi_step_schema.as_u32(),
                case_tag.as_u32()
            ))
        })?;
        let (op_tag, effect_instance_key) = self.effect_signal_constants_for_case(case_layout)?;
        let payload_transport =
            self.encode_effect_transport_parts(payload_ty, payload, "effect_outcome_payload")?;
        let signal = self.codegen.build_effect_signal(
            op_tag,
            effect_instance_key,
            payload_transport,
            resume_token,
        )?;
        self.codegen.build_effect_outcome(
            EffectOutcomeTag::Propagate,
            self.zero_transport_parts(),
            signal,
        )
    }

    pub(super) fn build_step_from_effect_outcome(
        &mut self,
        step_layout: &StepLayout<'ctx>,
        outcome_ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        let function = self.function;
        let complete_bb = self
            .codegen
            .context
            .append_basic_block(function, &format!("{name}_complete"));
        let dispatch_bb = self
            .codegen
            .context
            .append_basic_block(function, &format!("{name}_dispatch"));
        let done_bb = self
            .codegen
            .context
            .append_basic_block(function, &format!("{name}_done"));
        let unmatched_bb = self
            .codegen
            .context
            .append_basic_block(function, &format!("{name}_unmatched"));
        let is_propagating = self.codegen.effect_outcome_is_propagating(
            self.mir_fun.span,
            outcome_ptr,
            &format!("{name}_outcome"),
        )?;
        self.codegen
            .builder
            .build_conditional_branch(is_propagating, dispatch_bb, complete_bb)?;
        let mut incoming_steps = Vec::<(BasicValueEnum<'ctx>, BasicBlock<'ctx>)>::new();

        self.codegen.builder.position_at_end(complete_bb);
        let complete_transport = self.codegen.effect_outcome_complete_transport(
            self.mir_fun.span,
            outcome_ptr,
            &format!("{name}_complete_transport"),
        )?;
        let complete_payload = self.decode_effect_transport_parts(
            step_layout.complete_variant().payload_source_ty(),
            complete_transport,
            &format!("{name}_complete_payload"),
        )?;
        let complete_step = self
            .codegen
            .build_step_complete(step_layout, complete_payload)?;
        self.codegen.builder.build_unconditional_branch(done_bb)?;
        let complete_end = self.codegen.builder.get_insert_block().ok_or_else(|| {
            frontend_error(format!("`{name}` complete path 缺少 insert block"))
        })?;
        incoming_steps.push((complete_step, complete_end));

        self.codegen.builder.position_at_end(dispatch_bb);
        let signal_op_tag = self.codegen.effect_outcome_signal_op_tag(
            self.mir_fun.span,
            outcome_ptr,
            &format!("{name}_signal"),
        )?;
        let signal_effect_instance_key = self.codegen.effect_outcome_signal_effect_instance_key(
            self.mir_fun.span,
            outcome_ptr,
            &format!("{name}_signal"),
        )?;
        let signal_payload = self.codegen.effect_outcome_payload_transport(
            self.mir_fun.span,
            outcome_ptr,
            &format!("{name}_signal_payload"),
        )?;
        let signal_resume_token = self.codegen.effect_outcome_resume_token(
            self.mir_fun.span,
            outcome_ptr,
            &format!("{name}_signal_resume_token"),
        )?;
        let first_check = self
            .codegen
            .context
            .append_basic_block(function, &format!("{name}_check0"));
        self.codegen
            .builder
            .build_unconditional_branch(first_check)?;
        let mut check_bb = first_check;
        let mut case_blocks = Vec::new();
        for (index, case_layout) in step_layout.cases().values().enumerate() {
            let next_bb = self
                .codegen
                .context
                .append_basic_block(function, &format!("{name}_check{}", index + 1));
            let hit_bb = self.codegen.context.append_basic_block(
                function,
                &format!("{name}_case{}", case_layout.case_tag().as_u32()),
            );
            self.codegen.builder.position_at_end(check_bb);
            let (expected_op_tag, expected_effect_instance_key) =
                self.effect_signal_constants_for_case(case_layout)?;
            let op_match = self.codegen.builder.build_int_compare(
                inkwell::IntPredicate::EQ,
                signal_op_tag,
                expected_op_tag,
                &format!("{name}_op_match"),
            )?;
            let effect_instance_match = self.codegen.builder.build_int_compare(
                inkwell::IntPredicate::EQ,
                signal_effect_instance_key,
                expected_effect_instance_key,
                &format!("{name}_effect_instance_match"),
            )?;
            let both_match = self.codegen.builder.build_and(
                op_match,
                effect_instance_match,
                &format!("{name}_case_match"),
            )?;
            self.codegen
                .builder
                .build_conditional_branch(both_match, hit_bb, next_bb)?;
            case_blocks.push((
                case_layout.case_tag(),
                case_layout.payload_tuple_ty(),
                hit_bb,
            ));
            check_bb = next_bb;
        }

        for (case_tag, payload_ty, hit_bb) in case_blocks {
            self.codegen.builder.position_at_end(hit_bb);
            let case_layout = step_layout.case_layout(case_tag).ok_or_else(|| {
                frontend_error(format!(
                    "step schema 缺少 case c{}",
                    case_tag.as_u32()
                ))
            })?;
            let payload = self.decode_effect_transport_parts(
                payload_ty,
                signal_payload,
                &format!("{name}_case{}_payload", case_tag.as_u32()),
            )?;
            let step = self.codegen.build_step_case(
                step_layout,
                case_layout,
                payload,
                signal_resume_token,
            )?;
            self.codegen.builder.build_unconditional_branch(done_bb)?;
            let end_bb = self.codegen.builder.get_insert_block().ok_or_else(|| {
                frontend_error(format!(
                    "`{name}` case c{} path 缺少 insert block",
                    case_tag.as_u32()
                ))
            })?;
            incoming_steps.push((step, end_bb));
        }

        self.codegen.builder.position_at_end(check_bb);
        self.codegen
            .builder
            .build_unconditional_branch(unmatched_bb)?;

        self.codegen.builder.position_at_end(unmatched_bb);
        self.codegen.builder.build_unreachable()?;

        self.codegen.builder.position_at_end(done_bb);
        let step_phi = self
            .codegen
            .builder
            .build_phi(step_layout.llvm_ty(), &format!("{name}_phi"))?;
        for (step, block) in incoming_steps {
            step_phi.add_incoming(&[(&step, block)]);
        }
        Ok(step_phi.as_basic_value())
    }
}
