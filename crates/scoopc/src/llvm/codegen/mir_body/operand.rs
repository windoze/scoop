//! MIR operand-type and callable-value FQN resolution helpers.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn mir_local_type_id(
        &self,
        body: &crate::mir::Body,
        local: crate::mir::LocalId,
    ) -> Option<TypeId> {
        body.locals
            .get(local.as_u32() as usize)
            .map(|local| local.ty)
    }

    pub(in crate::llvm::codegen) fn mir_operand_type_id(
        &self,
        body: &crate::mir::Body,
        operand: &crate::mir::Operand,
    ) -> Option<TypeId> {
        match operand {
            crate::mir::Operand::Local(local) => self.mir_local_type_id(body, *local),
            crate::mir::Operand::Const(value) => Some(match value {
                crate::mir::ConstValue::Bool(_) => self.builtins.bool_,
                crate::mir::ConstValue::Char => self.builtins.char_,
                crate::mir::ConstValue::Unit => self.builtins.unit,
                crate::mir::ConstValue::Int | crate::mir::ConstValue::SynthInt(_) => {
                    self.builtins.int
                }
                crate::mir::ConstValue::Float64 => self.builtins.float64,
                crate::mir::ConstValue::Float32 => self.builtins.float32,
                crate::mir::ConstValue::String => self.builtins.string,
            }),
        }
    }

    pub(in crate::llvm::codegen) fn mir_operand_function_type(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        operand: &crate::mir::Operand,
    ) -> Option<crate::ty::FunctionType> {
        let ty = self.mir_operand_type_id(body, operand)?;
        match mir_types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Function(fun_ty)) => {
                self.equivalent_codegen_function_type(mir_types, fun_ty)
            }
            _ => None,
        }
    }

    pub(in crate::llvm::codegen) fn mir_callable_fqn_may_outward_effect(
        &self,
        callable_fqn: &str,
    ) -> Option<bool> {
        if let Some(callable) = self
            .published_late_lowered_program()
            .and_then(|program| program.callable(callable_fqn))
        {
            return Some(callable.effect_step_abi().is_some());
        }
        if let Some((callable_types, callable_fun)) = self.materialized_mir_callable(callable_fqn) {
            return Some(
                crate::mir::summarize_pass_rewritten_fun(callable_fun, callable_types, None)
                    .may_outward_effect,
            );
        }
        self.hir_fun_for_callable_fqn(callable_fqn)
            .map(|fun| self.known_fun_body_may_outward_effect(callable_fqn, fun.ty))
    }

    pub(in crate::llvm::codegen) fn mir_fun_value_callee_fqn(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        operand: &crate::mir::Operand,
    ) -> Option<String> {
        let mut visiting = HashSet::new();
        self.mir_callable_value_fqn_for_operand(body, mir_types, operand, &mut visiting)
    }

    pub(in crate::llvm::codegen) fn mir_callable_value_fqn_for_operand(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        operand: &crate::mir::Operand,
        visiting: &mut HashSet<crate::mir::LocalId>,
    ) -> Option<String> {
        let crate::mir::Operand::Local(local) = operand else {
            return None;
        };
        self.mir_callable_value_fqn_for_local(body, mir_types, *local, visiting)
    }

    pub(in crate::llvm::codegen) fn mir_callable_value_fqn_for_local(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        local: crate::mir::LocalId,
        visiting: &mut HashSet<crate::mir::LocalId>,
    ) -> Option<String> {
        if !visiting.insert(local) {
            return None;
        }

        let mut matched: Option<String> = None;
        for block in &body.blocks {
            for stmt in &block.stmts {
                let crate::mir::StatementKind::Assign { target, value } = &stmt.kind else {
                    continue;
                };
                if *target != local {
                    continue;
                }
                let candidate =
                    self.mir_callable_value_fqn_for_rvalue(body, mir_types, value, visiting)?;
                match &matched {
                    Some(existing) if existing != &candidate => {
                        visiting.remove(&local);
                        return None;
                    }
                    Some(_) => {}
                    None => matched = Some(candidate),
                }
            }
        }

        visiting.remove(&local);
        matched
    }

    pub(in crate::llvm::codegen) fn mir_callable_value_fqn_for_rvalue(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        value: &crate::mir::Rvalue,
        visiting: &mut HashSet<crate::mir::LocalId>,
    ) -> Option<String> {
        match value {
            crate::mir::Rvalue::Use(operand)
            | crate::mir::Rvalue::Transport { value: operand, .. } => {
                self.mir_callable_value_fqn_for_operand(body, mir_types, operand, visiting)
            }
            crate::mir::Rvalue::TopLevelRef(crate::mir::TopLevelRef { fqn, .. }) => {
                Some(fqn.clone())
            }
            crate::mir::Rvalue::MakeClosure { fn_ptr, .. } => Some(fn_ptr.clone()),
            crate::mir::Rvalue::MemberAccess { member, .. } => match member.resolved.as_ref()? {
                crate::mir::MemberTarget::Fun { fqn }
                | crate::mir::MemberTarget::ExtensionFun { fqn } => Some(fqn.clone()),
                crate::mir::MemberTarget::Value { .. }
                | crate::mir::MemberTarget::ExtensionValue { .. } => None,
            },
            crate::mir::Rvalue::Call {
                kind: crate::mir::CallKind::Direct { callee_fqn },
                args,
                ..
            } => self.mir_callable_value_fqn_from_direct_call(
                body, mir_types, callee_fqn, args, visiting,
            ),
            crate::mir::Rvalue::UnresolvedName { .. }
            | crate::mir::Rvalue::Unary { .. }
            | crate::mir::Rvalue::Binary { .. }
            | crate::mir::Rvalue::TypeCheck { .. }
            | crate::mir::Rvalue::Cast { .. }
            | crate::mir::Rvalue::SizeOf { .. }
            | crate::mir::Rvalue::KindOf { .. }
            | crate::mir::Rvalue::AlignOf { .. }
            | crate::mir::Rvalue::DescOf { .. }
            | crate::mir::Rvalue::TypeMetadataLiteral(_)
            | crate::mir::Rvalue::EnumVariant { .. }
            | crate::mir::Rvalue::ClassCtor { .. }
            | crate::mir::Rvalue::Call { .. }
            | crate::mir::Rvalue::MakeTuple { .. }
            | crate::mir::Rvalue::StructLit { .. }
            | crate::mir::Rvalue::InterpolatedString { .. }
            | crate::mir::Rvalue::TupleGet { .. }
            | crate::mir::Rvalue::CaptureBoxNew { .. }
            | crate::mir::Rvalue::CaptureBoxGet { .. }
            | crate::mir::Rvalue::CaptureBoxSet { .. }
            | crate::mir::Rvalue::PatternMatch { .. }
            | crate::mir::Rvalue::PatternExtract { .. }
            | crate::mir::Rvalue::PerformResult { .. }
            | crate::mir::Rvalue::Todo(_) => None,
        }
    }

    pub(in crate::llvm::codegen) fn mir_callable_value_fqn_from_direct_call(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        callee_fqn: &str,
        args: &[crate::mir::CallArg],
        visiting: &mut HashSet<crate::mir::LocalId>,
    ) -> Option<String> {
        let (callee_types, callable_fun) = self.materialized_mir_callable(callee_fqn)?;
        let summary = crate::mir::summarize_pass_rewritten_fun(callable_fun, callee_types, None);
        self.mir_callable_value_fqn_from_result(
            body,
            mir_types,
            &summary.result_provenance,
            &callable_fun.params,
            args,
            visiting,
        )
    }

    pub(in crate::llvm::codegen) fn mir_callable_value_fqn_from_result(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        result: &crate::mir::ResultProvenance,
        params: &[crate::mir::Param],
        args: &[crate::mir::CallArg],
        visiting: &mut HashSet<crate::mir::LocalId>,
    ) -> Option<String> {
        match result {
            crate::mir::ResultProvenance::DirectFunction(fqn)
            | crate::mir::ResultProvenance::KnownClosure(fqn) => Some(fqn.clone()),
            crate::mir::ResultProvenance::Param(index) => self
                .mir_callable_value_fqn_from_param_result(
                    body, mir_types, *index, params, args, visiting,
                ),
            crate::mir::ResultProvenance::Join(sources) if sources.len() == 1 => self
                .mir_callable_value_fqn_from_result_source(
                    body,
                    mir_types,
                    &sources[0],
                    params,
                    args,
                    visiting,
                ),
            crate::mir::ResultProvenance::Unit
            | crate::mir::ResultProvenance::TopLevelValue(_)
            | crate::mir::ResultProvenance::PerformResult(_)
            | crate::mir::ResultProvenance::Join(_)
            | crate::mir::ResultProvenance::Unknown => None,
        }
    }

    pub(in crate::llvm::codegen) fn mir_callable_value_fqn_from_result_source(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        source: &crate::mir::ResultProvenanceSource,
        params: &[crate::mir::Param],
        args: &[crate::mir::CallArg],
        visiting: &mut HashSet<crate::mir::LocalId>,
    ) -> Option<String> {
        match source {
            crate::mir::ResultProvenanceSource::DirectFunction(fqn)
            | crate::mir::ResultProvenanceSource::KnownClosure(fqn) => Some(fqn.clone()),
            crate::mir::ResultProvenanceSource::Param(index) => self
                .mir_callable_value_fqn_from_param_result(
                    body, mir_types, *index, params, args, visiting,
                ),
            crate::mir::ResultProvenanceSource::TopLevelValue(_)
            | crate::mir::ResultProvenanceSource::PerformResult(_) => None,
        }
    }

    pub(in crate::llvm::codegen) fn mir_callable_value_fqn_from_param_result(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        index: usize,
        params: &[crate::mir::Param],
        args: &[crate::mir::CallArg],
        visiting: &mut HashSet<crate::mir::LocalId>,
    ) -> Option<String> {
        let bound_args = bind_mir_call_args_to_params(params, args)?;
        let operand = bound_args.get(index)?;
        self.mir_callable_value_fqn_for_operand(body, mir_types, operand, visiting)
    }

    pub(in crate::llvm::codegen) fn mir_operand_funptr_function_type(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        operand: &crate::mir::Operand,
    ) -> Option<crate::ty::FunctionType> {
        let ty = self.mir_operand_type_id(body, operand)?;
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = mir_types.kind(ty) else {
            return None;
        };
        if nominal.fqn != "scoop.unsafe.FunPtr" || nominal.args.len() != 1 {
            return None;
        }
        let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = mir_types.kind(nominal.args[0]) else {
            return None;
        };
        self.equivalent_codegen_function_type(mir_types, fun_ty)
    }

    pub(in crate::llvm::codegen) fn mir_closure_env_capture_element_cg_tys(
        &self,
        env_cg: CgTy,
    ) -> Option<Vec<CgTy>> {
        match env_cg {
            CgTy::Unit => Some(Vec::new()),
            CgTy::Tuple(tuple_ty) => {
                let tuple_types = self.codegen_type_store_for_type_id(tuple_ty)?;
                let TypeKind::Value(ValueTypeKind::Tuple(elements)) = tuple_types.kind(tuple_ty)
                else {
                    return None;
                };
                let elements = elements.clone();
                let mut out = Vec::with_capacity(elements.len());
                for elem_ty in elements {
                    let cg = if std::ptr::eq(tuple_types, self.types) {
                        self.cg_ty_of(elem_ty)
                    } else {
                        self.cg_ty_of_mir_type(tuple_types, elem_ty)
                    }?;
                    if !Self::mir_closure_env_capture_cg_is_supported(cg) {
                        return None;
                    }
                    out.push(cg);
                }
                Some(out)
            }
            _ => None,
        }
    }

    pub(in crate::llvm::codegen) fn mir_closure_env_capture_cg_is_supported(cg_ty: CgTy) -> bool {
        matches!(
            cg_ty,
            CgTy::Unit
                | CgTy::Bool
                | CgTy::Float64
                | CgTy::Float32
                | CgTy::Int(_)
                | CgTy::String
                | CgTy::Ref
                | CgTy::Tuple(_)
                | CgTy::Struct(_)
                | CgTy::Enum(_)
        )
    }

    pub(in crate::llvm::codegen) fn mir_closure_env_capture_element_cg_tys_from_contract(
        &mut self,
        span: crate::span::Span,
        body_fqn: &str,
        mir_types: &TypeStore,
        env_cg: CgTy,
        contract: &crate::mir::ClosureEnvTransportMetadata,
    ) -> Result<Vec<CgTy>, LlvmEmitError> {
        let contract_env_cg = self.cg_ty_of_mir_type(mir_types, contract.env_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure env contract codegen type",
                at: span.into(),
            },
        )?;
        if contract_env_cg != env_cg {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure env contract mismatch",
                at: span.into(),
            });
        }

        let capture_field_cgs = self.mir_closure_env_capture_element_cg_tys(env_cg).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure env shape",
                at: span.into(),
            },
        )?;
        if capture_field_cgs.len() != contract.captures.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure env capture schema arity",
                at: span.into(),
            });
        }

        let env_transport = crate::mir::ValueTransportMetadata {
            source_ty: contract.env_ty,
            kind: crate::mir::MirTransportKind::ClosureEnv,
            requirements: self
                .composite_transport_requirements_for_type(mir_types, contract.env_ty),
            boxing: None,
        };
        self.get_or_create_value_composite_transport_descriptor_global(
            body_fqn,
            span,
            mir_types,
            &env_transport,
        )?;

        let env_element_tys = match mir_types.kind(contract.env_ty) {
            TypeKind::Value(ValueTypeKind::Unit) => &[][..],
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => elements.as_slice(),
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR closure env contract shape",
                    at: span.into(),
                });
            }
        };

        for (index, capture) in contract.captures.iter().enumerate() {
            let env_element_ty =
                env_element_tys
                    .get(index)
                    .copied()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR closure env capture schema element",
                        at: capture.decl_span.into(),
                    })?;
            if mir_types.display(capture.transport.source_ty).to_string()
                != mir_types.display(env_element_ty).to_string()
            {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR closure env capture schema type",
                    at: capture.decl_span.into(),
                });
            }
            if capture.mutable
                && (capture.transport.kind != crate::mir::MirTransportKind::CaptureBox
                    || self
                        .mir_capture_box_inner_type_id(mir_types, capture.transport.source_ty)
                        .is_none())
            {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR closure mutable capture box contract",
                    at: capture.decl_span.into(),
                });
            }
            self.get_or_create_value_composite_transport_descriptor_global(
                body_fqn,
                capture.decl_span,
                mir_types,
                &capture.transport,
            )?;
        }

        Ok(capture_field_cgs)
    }
}
