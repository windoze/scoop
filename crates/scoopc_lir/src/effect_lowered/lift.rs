use std::collections::HashMap;

use scoopc_ids::LirCallableId;

use crate::effect_facts::ConcreteOpKey;
use crate::mir::{self, BasicBlockId, Body, FunDecl, LocalId, Operand, Rvalue, StatementKind};
use crate::ty::TypeId;

use super::EffectLoweringError;
use super::instruction::{
    LirCallAbiHandoffMetadata, LirCallArg, LirCallKind, LirCallTransportMetadata,
    LirCallableHeader, LirCastOp, LirClassCtorCallMetadata, LirDispatchKey, LirDispatchMetadata,
    LirExecutableBody, LirExecutableBodyFlavor, LirExecutableState,
    LirGcIntrinsicTransportMetadata, LirInterpolatedStringPart, LirInterpolatedStringPartKind,
    LirLocalDecl, LirMemberAccessMetadata, LirMemberKey, LirMemberTarget, LirOperand, LirPattern,
    LirRuntimeCastFailure, LirRuntimeCastMetadata, LirRuntimeCastResult, LirRuntimeNominalKind,
    LirRuntimePatternTypeTestKind, LirRuntimePatternTypeTestMetadata, LirRuntimeTypeDescriptorKey,
    LirRuntimeTypeDescriptorKind, LirRuntimeTypeParameterizedMatch, LirRuntimeTypeTestMetadata,
    LirRvalue, LirStateBody, LirStateMachineBody, LirStatement, LirStatementKind,
    LirStructLitField, LirTopLevelRef, LirTopLevelRefTarget, LirTypeCheckOp,
    LirTypeMetadataLiteral,
};
use super::ir::{LateLoweredCompletionPayloadSource, LateLoweredOperandSource};
use super::ir::{LateLoweredStateGraph, LateLoweredStateRole, LateLoweredStateTerminator, StateId};

pub(crate) struct LirLiftContext<'a> {
    root_fqn: &'a str,
    callable_ids: &'a HashMap<String, LirCallableId>,
    concrete_ops: &'a HashMap<String, ConcreteOpKey>,
}

impl<'a> LirLiftContext<'a> {
    pub(crate) fn new(
        root_fqn: &'a str,
        callable_ids: &'a HashMap<String, LirCallableId>,
        concrete_ops: &'a HashMap<String, ConcreteOpKey>,
    ) -> Self {
        Self {
            root_fqn,
            callable_ids,
            concrete_ops,
        }
    }

    pub(crate) fn lift_plain_body(
        &self,
        fun: &FunDecl,
        body: &Body,
        flavor: LirExecutableBodyFlavor,
    ) -> Result<LirExecutableBody, EffectLoweringError> {
        let complete_state = StateId::new(body.blocks.len() as u32);
        let mut states = Vec::with_capacity(body.blocks.len() + 1);
        for (block_index, block) in body.blocks.iter().enumerate() {
            let state_id = StateId::new(block_index as u32);
            let role = if body.start.as_u32() == block_index as u32 {
                LateLoweredStateRole::Entry
            } else if block.is_cleanup {
                LateLoweredStateRole::Cleanup
            } else {
                LateLoweredStateRole::Segment
            };
            let statements = self.lift_statement_range(
                body,
                BasicBlockId::from_raw(block_index as u32),
                0,
                block.stmts.len() as u32,
            )?;
            let terminator =
                self.lift_plain_terminator(body, &block.terminator, complete_state, fun.return_ty)?;
            states.push(LirExecutableState::new(
                state_id,
                role,
                LirStateBody::new(statements, terminator),
            ));
        }
        states.push(LirExecutableState::new(
            complete_state,
            LateLoweredStateRole::Complete,
            LirStateBody::new(Vec::new(), LateLoweredStateTerminator::Unreachable),
        ));
        let locals = body
            .locals
            .iter()
            .enumerate()
            .map(|(index, local)| LirLocalDecl::from_source(LocalId::from_raw(index as u32), local))
            .collect();
        Ok(LirExecutableBody::new(
            flavor,
            LirCallableHeader::from_source(fun),
            locals,
            LirStateMachineBody::new(
                StateId::new(body.start.as_u32()),
                complete_state,
                None,
                None,
                states,
            ),
        ))
    }

    pub(crate) fn lift_control_body(
        &self,
        fun: &FunDecl,
        body: &Body,
        flavor: LirExecutableBodyFlavor,
        state_graph: &LateLoweredStateGraph,
    ) -> LirExecutableBody {
        let states = state_graph
            .states()
            .iter()
            .map(|state| {
                LirExecutableState::new(state.state_id(), state.role(), state.body().clone())
            })
            .collect();
        let locals = body
            .locals
            .iter()
            .enumerate()
            .map(|(index, local)| LirLocalDecl::from_source(LocalId::from_raw(index as u32), local))
            .collect();
        LirExecutableBody::new(
            flavor,
            LirCallableHeader::from_source(fun),
            locals,
            LirStateMachineBody::new(
                state_graph.entry_state(),
                state_graph.complete_state(),
                state_graph.cleanup_state(),
                state_graph.drop_state(),
                states,
            ),
        )
    }

    pub(crate) fn lift_statement_range(
        &self,
        body: &Body,
        block_id: BasicBlockId,
        start_statement_index: u32,
        end_statement_index: u32,
    ) -> Result<Vec<LirStatement>, EffectLoweringError> {
        let block = body.blocks.get(block_id.as_u32() as usize).ok_or_else(|| {
            self.invalid_lift(format!("missing MIR block bb{}", block_id.as_u32()))
        })?;
        let start = start_statement_index as usize;
        let end = end_statement_index as usize;
        if start > end || end > block.stmts.len() {
            return Err(self.invalid_lift(format!(
                "statement range [{}..{}) is outside bb{} with {} statements",
                start_statement_index,
                end_statement_index,
                block_id.as_u32(),
                block.stmts.len()
            )));
        }
        block.stmts[start..end]
            .iter()
            .map(|stmt| self.lift_statement(stmt))
            .collect()
    }

    pub(crate) fn lift_statement(
        &self,
        stmt: &mir::Statement,
    ) -> Result<LirStatement, EffectLoweringError> {
        let kind = match &stmt.kind {
            StatementKind::Nop => LirStatementKind::Nop,
            StatementKind::Assign { target, value } => LirStatementKind::Assign {
                target: *target,
                value: self.lift_rvalue(value)?,
            },
            StatementKind::StoreMember {
                receiver,
                member,
                value,
                value_ty,
                continuation_route,
            } => LirStatementKind::StoreMember {
                receiver: lift_operand(receiver),
                member: self.lift_member_access(member)?,
                value: lift_operand(value),
                value_ty: *value_ty,
                continuation_route: continuation_route.clone(),
            },
            StatementKind::StoreTopLevelVar {
                fqn,
                value,
                value_ty,
            } => LirStatementKind::StoreGlobal {
                root: scoopc_lir_facts::LirGlobalRootKey::new(fqn.clone()),
                value: lift_operand(value),
                value_ty: *value_ty,
            },
            StatementKind::Todo(reason) => {
                return Err(
                    self.invalid_lift(format!("MIR Todo statement reached LIR lift: {reason}"))
                );
            }
        };
        Ok(LirStatement {
            span: stmt.span,
            kind,
        })
    }

    pub(crate) fn lift_rvalue(&self, value: &Rvalue) -> Result<LirRvalue, EffectLoweringError> {
        match value {
            Rvalue::Use(operand) => Ok(LirRvalue::Use(lift_operand(operand))),
            Rvalue::Transport { value, transport } => Ok(LirRvalue::Transport {
                value: lift_operand(value),
                transport: transport.clone(),
            }),
            Rvalue::TopLevelRef(top) => Ok(LirRvalue::TopLevelRef(LirTopLevelRef {
                target: self.lift_top_level_ref_target(&top.fqn),
                site_id: top.site_id,
                hidden_effects: top.hidden_effects.clone(),
                stable_template_key: top.stable_template_key.clone(),
                stable_instance_key: top.stable_instance_key.clone(),
                generic_type_args: top.generic_type_args.clone(),
                generic_eff_args: top.generic_eff_args.clone(),
            })),
            Rvalue::UnresolvedName { name } => {
                Err(self.invalid_lift(format!("unresolved MIR name `{name}` reached LIR lift")))
            }
            Rvalue::TypeCheck {
                value,
                op,
                test_ty,
                metadata,
            } => Ok(LirRvalue::TypeCheck {
                value: lift_operand(value),
                op: lift_type_check_op(*op),
                test_ty: *test_ty,
                metadata: lift_runtime_type_test_metadata(metadata),
            }),
            Rvalue::Cast {
                value,
                op,
                target_ty,
                metadata,
            } => Ok(LirRvalue::Cast {
                value: lift_operand(value),
                op: lift_cast_op(*op),
                target_ty: *target_ty,
                metadata: lift_runtime_cast_metadata(metadata),
            }),
            Rvalue::MemberAccess {
                site_id,
                receiver,
                member,
            } => Ok(LirRvalue::MemberAccess {
                site_id: *site_id,
                receiver: lift_operand(receiver),
                member: self.lift_member_access(member)?,
            }),
            Rvalue::EnumVariant {
                enum_ty,
                variant_name,
                args,
                payload,
            } => Ok(LirRvalue::EnumVariant {
                enum_ty: *enum_ty,
                variant_name: variant_name.clone(),
                args: lift_call_args(args),
                payload: payload.clone(),
            }),
            Rvalue::ClassCtor {
                site_id,
                class_fqn,
                ctor,
                args,
                hidden_effects,
            } => Ok(LirRvalue::ClassCtor {
                site_id: *site_id,
                class: scoopc_lir_facts::LirNominalLayoutKey::new(class_fqn.clone()),
                ctor: LirClassCtorCallMetadata {
                    target_init_class: scoopc_lir_facts::LirNominalLayoutKey::new(
                        ctor.target_init_class_fqn.clone(),
                    ),
                    selected_ctor_span: ctor.selected_ctor_span,
                    ordered_param_count: ctor.ordered_param_count,
                },
                args: lift_call_args(args),
                hidden_effects: hidden_effects.clone(),
            }),
            Rvalue::Call {
                site_id,
                kind,
                args,
                transport,
            } => Ok(LirRvalue::Call {
                site_id: *site_id,
                kind: self.lift_call_kind(kind)?,
                args: lift_call_args(args),
                transport: self.lift_call_transport(transport)?,
            }),
            Rvalue::MakeTuple {
                elements,
                transport,
            } => Ok(LirRvalue::MakeTuple {
                elements: elements.iter().map(lift_operand).collect(),
                transport: transport.clone(),
            }),
            Rvalue::StructLit { fields, transport } => Ok(LirRvalue::StructLit {
                fields: fields
                    .iter()
                    .map(|field| LirStructLitField {
                        span: field.span,
                        name: field.name.clone(),
                        value: lift_operand(&field.value),
                    })
                    .collect(),
                transport: transport.clone(),
            }),
            Rvalue::SizeOf { site_id, value_ty } => Ok(LirRvalue::SizeOf {
                site_id: *site_id,
                value_ty: *value_ty,
            }),
            Rvalue::KindOf { site_id, value_ty } => Ok(LirRvalue::KindOf {
                site_id: *site_id,
                value_ty: *value_ty,
            }),
            Rvalue::AlignOf { site_id, value_ty } => Ok(LirRvalue::AlignOf {
                site_id: *site_id,
                value_ty: *value_ty,
            }),
            Rvalue::DescOf { site_id, value_ty } => Ok(LirRvalue::DescOf {
                site_id: *site_id,
                value_ty: *value_ty,
            }),
            Rvalue::TypeMetadataLiteral(metadata) => {
                Ok(LirRvalue::TypeMetadataLiteral(LirTypeMetadataLiteral {
                    source_ty: metadata.source_ty,
                    source_nominal: metadata
                        .source_fqn
                        .as_ref()
                        .map(|fqn| scoopc_lir_facts::LirNominalLayoutKey::new(fqn.clone())),
                    kind: metadata.kind,
                }))
            }
            Rvalue::InterpolatedString { raw, parts } => Ok(LirRvalue::InterpolatedString {
                raw: *raw,
                parts: parts
                    .iter()
                    .map(|part| match part {
                        mir::InterpolatedStringPart::Text { span } => LirInterpolatedStringPart {
                            span: *span,
                            kind: LirInterpolatedStringPartKind::Text,
                        },
                        mir::InterpolatedStringPart::Expr { span, value, ty } => {
                            LirInterpolatedStringPart {
                                span: *span,
                                kind: LirInterpolatedStringPartKind::Expr {
                                    value: lift_operand(value),
                                    ty: *ty,
                                },
                            }
                        }
                    })
                    .collect(),
            }),
            Rvalue::TupleGet { tuple, index } => Ok(LirRvalue::TupleGet {
                tuple: lift_operand(tuple),
                index: *index,
            }),
            Rvalue::PatternMatch { subject, pattern } => Ok(LirRvalue::PatternMatch {
                subject: lift_operand(subject),
                pattern: lift_pattern(pattern),
            }),
            Rvalue::PatternExtract { subject, path } => Ok(LirRvalue::PatternExtract {
                subject: lift_operand(subject),
                path: path.clone(),
            }),
            Rvalue::MakeClosure {
                env,
                fn_ptr,
                env_contract,
            } => Ok(LirRvalue::MakeClosure {
                env: lift_operand(env),
                fn_ptr: self.callable_id_for_root(fn_ptr)?,
                env_contract: env_contract.clone(),
            }),
            Rvalue::PerformResult { op_fqn, effect_ty } => Ok(LirRvalue::PerformResult {
                op: self.concrete_op_key(op_fqn)?,
                effect_ty: *effect_ty,
            }),
            Rvalue::Todo(reason) => {
                Err(self.invalid_lift(format!("MIR Todo rvalue reached LIR lift: {reason}")))
            }
        }
    }

    fn lift_plain_terminator(
        &self,
        body: &Body,
        terminator: &mir::Terminator,
        complete_state: StateId,
        complete_ty: TypeId,
    ) -> Result<LateLoweredStateTerminator, EffectLoweringError> {
        if let mir::UnwindAction::Todo(reason) = &terminator.unwind {
            return Err(
                self.invalid_lift(format!("MIR Todo unwind action reached LIR lift: {reason}"))
            );
        }
        match &terminator.kind {
            mir::TerminatorKind::Return { value } => Ok(LateLoweredStateTerminator::Return {
                payload_source: completion_payload_source(
                    body,
                    value,
                    terminator.span,
                    complete_ty,
                ),
                complete_state,
            }),
            mir::TerminatorKind::Goto { target } => Ok(LateLoweredStateTerminator::Goto {
                target: StateId::new(target.as_u32()),
            }),
            mir::TerminatorKind::CondBr {
                cond,
                then_target,
                else_target,
            } => {
                let Operand::Local(cond_local) = cond else {
                    return Err(self
                        .invalid_lift("MIR CondBr condition is not a local operand".to_string()));
                };
                Ok(LateLoweredStateTerminator::Branch {
                    cond_local: *cond_local,
                    then_state: StateId::new(then_target.as_u32()),
                    else_state: StateId::new(else_target.as_u32()),
                })
            }
            mir::TerminatorKind::Unreachable => Ok(LateLoweredStateTerminator::Unreachable),
            mir::TerminatorKind::ResumeUnwind => Ok(LateLoweredStateTerminator::ResumeUnwind),
            mir::TerminatorKind::Perform { .. } | mir::TerminatorKind::Handle { .. } => Err(self
                .invalid_lift(
                    "raw MIR effect/control terminator reached plain LIR body lift".to_string(),
                )),
            mir::TerminatorKind::Todo(reason) => {
                Err(self.invalid_lift(format!("MIR Todo terminator reached LIR lift: {reason}")))
            }
        }
    }

    fn lift_top_level_ref_target(&self, fqn: &str) -> LirTopLevelRefTarget {
        self.callable_ids
            .get(fqn)
            .copied()
            .map(LirTopLevelRefTarget::Callable)
            .unwrap_or_else(|| {
                LirTopLevelRefTarget::Global(scoopc_lir_facts::LirGlobalRootKey::new(fqn))
            })
    }

    fn lift_member_access(
        &self,
        member: &mir::MemberAccessMetadata,
    ) -> Result<LirMemberAccessMetadata, EffectLoweringError> {
        let resolved = member.resolved.as_ref().ok_or_else(|| {
            self.invalid_lift(format!(
                "unresolved member `{}` reached LIR lift",
                member.name
            ))
        })?;
        Ok(LirMemberAccessMetadata {
            name: member.name.clone(),
            receiver_ty: member.receiver_ty,
            resolved: self.lift_member_target(resolved)?,
            hidden_effects: member.hidden_effects.clone(),
        })
    }

    fn lift_member_target(
        &self,
        target: &mir::MemberTarget,
    ) -> Result<LirMemberTarget, EffectLoweringError> {
        match target {
            mir::MemberTarget::Value { fqn } => Ok(LirMemberTarget::Value {
                member: LirMemberKey::new(fqn.clone()),
            }),
            mir::MemberTarget::Fun { fqn } => Ok(LirMemberTarget::Fun {
                callable: self.callable_id_for_root(fqn)?,
            }),
            mir::MemberTarget::ExtensionValue { fqn } => Ok(LirMemberTarget::ExtensionValue {
                member: LirMemberKey::new(fqn.clone()),
            }),
            mir::MemberTarget::ExtensionFun { fqn } => Ok(LirMemberTarget::ExtensionFun {
                callable: self.callable_id_for_root(fqn)?,
            }),
        }
    }

    fn lift_call_kind(&self, kind: &mir::CallKind) -> Result<LirCallKind, EffectLoweringError> {
        match kind {
            mir::CallKind::Direct {
                callee_fqn,
                stable_template_key,
                stable_instance_key,
                intrinsic_entry_name,
                generic_type_args,
                generic_eff_args,
            } => Ok(LirCallKind::Direct {
                callee: self.callable_id_for_root(callee_fqn)?,
                stable_template_key: stable_template_key.clone(),
                stable_instance_key: stable_instance_key.clone(),
                intrinsic_entry_name: intrinsic_entry_name.clone(),
                generic_type_args: generic_type_args.clone(),
                generic_eff_args: generic_eff_args.clone(),
            }),
            mir::CallKind::Closure { callee, fn_ptr } => Ok(LirCallKind::Closure {
                callee: lift_operand(callee),
                fn_ptr: self.callable_id_for_root(fn_ptr)?,
            }),
            mir::CallKind::FunValue { callee } => Ok(LirCallKind::FunValue {
                callee: lift_operand(callee),
            }),
            mir::CallKind::FunPtr { callee } => Ok(LirCallKind::FunPtr {
                callee: lift_operand(callee),
            }),
            mir::CallKind::Virtual { receiver, dispatch } => Ok(LirCallKind::Virtual {
                receiver: lift_operand(receiver),
                dispatch: lift_dispatch(dispatch),
            }),
            mir::CallKind::Interface { receiver, dispatch } => Ok(LirCallKind::Interface {
                receiver: lift_operand(receiver),
                dispatch: lift_dispatch(dispatch),
            }),
            mir::CallKind::Resume {
                continuation,
                resume,
            } => Ok(LirCallKind::Resume {
                continuation: lift_operand(continuation),
                resume: resume.clone().into(),
            }),
        }
    }

    fn lift_call_transport(
        &self,
        transport: &mir::CallTransportMetadata,
    ) -> Result<LirCallTransportMetadata, EffectLoweringError> {
        Ok(LirCallTransportMetadata {
            result: transport.result.clone(),
            aggregate_return: transport.aggregate_return.clone(),
            array: transport.array.clone(),
            gc: transport
                .gc
                .as_ref()
                .map(|gc| {
                    Ok(LirGcIntrinsicTransportMetadata {
                        callee: self.callable_id_for_root(&gc.callee_fqn)?,
                        operation: gc.operation,
                        root_lifetime: gc.root_lifetime,
                        pairing: gc.pairing,
                        unsafe_required: gc.unsafe_required,
                        subject_ty: gc.subject_ty,
                        token_ty: gc.token_ty,
                        subject: gc.subject.clone(),
                    })
                })
                .transpose()?,
            abi: LirCallAbiHandoffMetadata {
                callable_abi_kind: transport.abi.callable_abi_kind,
                resolved_outward_cases: Vec::new(),
                impl_plan: transport.abi.impl_plan,
                adapter_required: transport.abi.adapter_required,
            },
        })
    }

    fn callable_id_for_root(&self, fqn: &str) -> Result<LirCallableId, EffectLoweringError> {
        self.callable_ids.get(fqn).copied().ok_or_else(|| {
            self.invalid_lift(format!(
                "callable reference `{fqn}` is not present in this LIR program"
            ))
        })
    }

    fn concrete_op_key(&self, op_fqn: &str) -> Result<ConcreteOpKey, EffectLoweringError> {
        self.concrete_ops.get(op_fqn).cloned().ok_or_else(|| {
            self.invalid_lift(format!(
                "effect op reference `{op_fqn}` is not present in StepSchema cases"
            ))
        })
    }

    fn invalid_lift(&self, detail: String) -> EffectLoweringError {
        EffectLoweringError::InvalidLirInstructionLift {
            root_fqn: self.root_fqn.to_string(),
            detail,
        }
    }
}

fn lift_operand(operand: &Operand) -> LirOperand {
    match operand {
        Operand::Local(local) => LirOperand::Local(*local),
        Operand::Const(value) => LirOperand::Const(value.clone()),
    }
}

fn lift_call_args(args: &[mir::CallArg]) -> Vec<LirCallArg> {
    args.iter()
        .map(|arg| LirCallArg {
            span: arg.span,
            name: arg.name.clone(),
            value: lift_operand(&arg.value),
        })
        .collect()
}

fn lift_type_check_op(op: impl std::fmt::Debug) -> LirTypeCheckOp {
    match format!("{op:?}").as_str() {
        "Is" => LirTypeCheckOp::Is,
        "NotIs" => LirTypeCheckOp::NotIs,
        other => panic!("unknown MIR type-check op {other}"),
    }
}

fn lift_cast_op(op: impl std::fmt::Debug) -> LirCastOp {
    match format!("{op:?}").as_str() {
        "As" => LirCastOp::As,
        "AsQ" => LirCastOp::AsQuestion,
        other => panic!("unknown MIR cast op {other}"),
    }
}

fn lift_runtime_nominal_kind(kind: Option<impl std::fmt::Debug>) -> Option<LirRuntimeNominalKind> {
    kind.map(|kind| match format!("{kind:?}").as_str() {
        "Class" => LirRuntimeNominalKind::Class,
        "Interface" => LirRuntimeNominalKind::Interface,
        "Struct" => LirRuntimeNominalKind::Struct,
        "Enum" => LirRuntimeNominalKind::Enum,
        "Effect" => LirRuntimeNominalKind::Effect,
        other => panic!("unknown MIR runtime nominal kind {other}"),
    })
}

fn lift_runtime_type_descriptor(
    descriptor: &mir::RuntimeTypeDescriptorKey,
) -> LirRuntimeTypeDescriptorKey {
    LirRuntimeTypeDescriptorKey {
        ty: descriptor.ty,
        kind: match &descriptor.kind {
            mir::RuntimeTypeDescriptorKind::Any => LirRuntimeTypeDescriptorKind::Any,
            mir::RuntimeTypeDescriptorKind::String => LirRuntimeTypeDescriptorKind::String,
            mir::RuntimeTypeDescriptorKind::Nominal { fqn, kind } => {
                LirRuntimeTypeDescriptorKind::Nominal {
                    nominal: scoopc_lir_facts::LirNominalLayoutKey::new(fqn.clone()),
                    kind: lift_runtime_nominal_kind(*kind),
                }
            }
            mir::RuntimeTypeDescriptorKind::Function => LirRuntimeTypeDescriptorKind::Function,
            mir::RuntimeTypeDescriptorKind::Option => LirRuntimeTypeDescriptorKind::Option,
            mir::RuntimeTypeDescriptorKind::Tuple => LirRuntimeTypeDescriptorKind::Tuple,
            mir::RuntimeTypeDescriptorKind::Value => LirRuntimeTypeDescriptorKind::Value,
            mir::RuntimeTypeDescriptorKind::TypeParam => LirRuntimeTypeDescriptorKind::TypeParam,
            mir::RuntimeTypeDescriptorKind::StarProjection => {
                LirRuntimeTypeDescriptorKind::StarProjection
            }
            mir::RuntimeTypeDescriptorKind::Union => LirRuntimeTypeDescriptorKind::Union,
        },
    }
}

fn lift_runtime_parameterized(
    parameterized: &mir::RuntimeTypeParameterizedMatch,
) -> LirRuntimeTypeParameterizedMatch {
    match parameterized {
        mir::RuntimeTypeParameterizedMatch::None => LirRuntimeTypeParameterizedMatch::None,
        mir::RuntimeTypeParameterizedMatch::Nominal {
            type_args,
            effect_arg,
        } => LirRuntimeTypeParameterizedMatch::Nominal {
            type_args: type_args.clone(),
            effect_arg: effect_arg.clone(),
        },
        mir::RuntimeTypeParameterizedMatch::Function {
            receiver,
            params,
            return_ty,
            effects,
            effects_closed,
        } => LirRuntimeTypeParameterizedMatch::Function {
            receiver: *receiver,
            params: params.clone(),
            return_ty: *return_ty,
            effects: effects.clone(),
            effects_closed: *effects_closed,
        },
        mir::RuntimeTypeParameterizedMatch::Option { payload_ty } => {
            LirRuntimeTypeParameterizedMatch::Option {
                payload_ty: *payload_ty,
            }
        }
        mir::RuntimeTypeParameterizedMatch::Tuple { element_tys } => {
            LirRuntimeTypeParameterizedMatch::Tuple {
                element_tys: element_tys.clone(),
            }
        }
        mir::RuntimeTypeParameterizedMatch::Union { variants } => {
            LirRuntimeTypeParameterizedMatch::Union {
                variants: variants.clone(),
            }
        }
        mir::RuntimeTypeParameterizedMatch::StarProjection { read_ty } => {
            LirRuntimeTypeParameterizedMatch::StarProjection { read_ty: *read_ty }
        }
    }
}

fn lift_runtime_type_test_metadata(
    metadata: &mir::RuntimeTypeTestMetadata,
) -> LirRuntimeTypeTestMetadata {
    LirRuntimeTypeTestMetadata {
        source_ty: metadata.source_ty,
        target_ty: metadata.target_ty,
        descriptor: lift_runtime_type_descriptor(&metadata.descriptor),
        static_fold: metadata.static_fold,
        parameterized: lift_runtime_parameterized(&metadata.parameterized),
    }
}

fn lift_runtime_cast_metadata(metadata: &mir::RuntimeCastMetadata) -> LirRuntimeCastMetadata {
    LirRuntimeCastMetadata {
        test: lift_runtime_type_test_metadata(&metadata.test),
        failure: match &metadata.failure {
            mir::RuntimeCastFailure::Panic { message } => LirRuntimeCastFailure::Panic {
                message: message.clone(),
            },
            mir::RuntimeCastFailure::ReturnNone => LirRuntimeCastFailure::ReturnNone,
        },
        result: match metadata.result {
            mir::RuntimeCastResult::Target { ty } => LirRuntimeCastResult::Target { ty },
            mir::RuntimeCastResult::Option { option_ty, some_ty } => {
                LirRuntimeCastResult::Option { option_ty, some_ty }
            }
        },
    }
}

fn lift_runtime_pattern_metadata(
    metadata: &mir::RuntimePatternTypeTestMetadata,
) -> LirRuntimePatternTypeTestMetadata {
    LirRuntimePatternTypeTestMetadata {
        subject_ty: metadata.subject_ty,
        target_ty: metadata.target_ty,
        descriptor: lift_runtime_type_descriptor(&metadata.descriptor),
        match_kind: match metadata.match_kind {
            mir::RuntimePatternTypeTestKind::StaticValue => {
                LirRuntimePatternTypeTestKind::StaticValue
            }
            mir::RuntimePatternTypeTestKind::RuntimeRef => {
                LirRuntimePatternTypeTestKind::RuntimeRef
            }
            mir::RuntimePatternTypeTestKind::RuntimeClass => {
                LirRuntimePatternTypeTestKind::RuntimeClass
            }
            mir::RuntimePatternTypeTestKind::RuntimeInterface => {
                LirRuntimePatternTypeTestKind::RuntimeInterface
            }
            mir::RuntimePatternTypeTestKind::RuntimeNominal => {
                LirRuntimePatternTypeTestKind::RuntimeNominal
            }
            mir::RuntimePatternTypeTestKind::RuntimeParameterized => {
                LirRuntimePatternTypeTestKind::RuntimeParameterized
            }
        },
        static_fold: metadata.static_fold,
        parameterized: lift_runtime_parameterized(&metadata.parameterized),
    }
}

fn lift_pattern(pattern: &mir::Pattern) -> LirPattern {
    match pattern {
        mir::Pattern::Else => LirPattern::Else,
        mir::Pattern::Or { pats } => LirPattern::Or {
            pats: pats.iter().map(lift_pattern).collect(),
        },
        mir::Pattern::Wildcard => LirPattern::Wildcard,
        mir::Pattern::Rest => LirPattern::Rest,
        mir::Pattern::Is { ty, metadata } => LirPattern::Is {
            ty: *ty,
            metadata: lift_runtime_pattern_metadata(metadata),
        },
        mir::Pattern::Bind { name, ty } => LirPattern::Bind {
            name: name.clone(),
            ty: *ty,
        },
        mir::Pattern::Tuple { elements } => LirPattern::Tuple {
            elements: elements.iter().map(lift_pattern).collect(),
        },
        mir::Pattern::Variant { name, args } => LirPattern::Variant {
            name: name.clone(),
            args: args.iter().map(lift_pattern).collect(),
        },
        mir::Pattern::IntLit { raw } => LirPattern::IntLit { raw: raw.clone() },
        mir::Pattern::CharLit { value } => LirPattern::CharLit { value: *value },
        mir::Pattern::StringLit { value } => LirPattern::StringLit {
            value: value.clone(),
        },
        mir::Pattern::BoolLit { value } => LirPattern::BoolLit { value: *value },
    }
}

fn lift_dispatch(dispatch: &mir::DispatchMetadata) -> LirDispatchMetadata {
    LirDispatchMetadata {
        dispatch: LirDispatchKey::new(format!("{}::{}", dispatch.owner_fqn, dispatch.member_fqn)),
        owner: scoopc_lir_facts::LirNominalLayoutKey::new(dispatch.owner_fqn.clone()),
        member_name: dispatch.member_name.clone(),
        member: LirMemberKey::new(dispatch.member_fqn.clone()),
        member_decl_span: dispatch.member_decl_span,
        receiver_ty: dispatch.receiver_ty,
        stable_candidate_keys: dispatch.stable_candidate_keys.clone(),
        stable_template_key: dispatch.stable_template_key.clone(),
        generic_type_args: dispatch.generic_type_args.clone(),
        generic_eff_args: dispatch.generic_eff_args.clone(),
    }
}

fn completion_payload_source(
    body: &Body,
    value: &Option<Operand>,
    span: crate::span::Span,
    complete_ty: TypeId,
) -> LateLoweredCompletionPayloadSource {
    let Some(value) = value else {
        return LateLoweredCompletionPayloadSource::unit(complete_ty);
    };
    match value {
        Operand::Local(local) => {
            let source_ty = body
                .locals
                .get(local.as_u32() as usize)
                .map(|decl| decl.ty)
                .unwrap_or(complete_ty);
            LateLoweredCompletionPayloadSource::operand(LateLoweredOperandSource::new_local(
                *local,
                source_ty,
                Some(span),
            ))
        }
        Operand::Const(value) => LateLoweredCompletionPayloadSource::operand(
            LateLoweredOperandSource::new_const(value.clone(), complete_ty, Some(span)),
        ),
    }
}
