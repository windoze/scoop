//! MIR operand-type and callable-value FQN resolution helpers.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn mir_local_type_id(
        &self,
        body: &mir_source::Body,
        local: mir_source::LocalId,
    ) -> Option<TypeId> {
        body.locals
            .get(local.as_u32() as usize)
            .map(|local| local.ty)
    }

    pub(in crate::llvm::codegen) fn mir_operand_type_id(
        &self,
        body: &mir_source::Body,
        operand: &mir_source::Operand,
    ) -> Option<TypeId> {
        match operand {
            mir_source::Operand::Local(local) => self.mir_local_type_id(body, *local),
            mir_source::Operand::Const(value) => Some(match value {
                mir_source::ConstValue::Bool(_) => self.builtins.bool_,
                mir_source::ConstValue::Char => self.builtins.char_,
                mir_source::ConstValue::Unit => self.builtins.unit,
                mir_source::ConstValue::Int | mir_source::ConstValue::SynthInt(_) => {
                    self.builtins.int
                }
                mir_source::ConstValue::Float64 => self.builtins.float64,
                mir_source::ConstValue::Float32 => self.builtins.float32,
                mir_source::ConstValue::String | mir_source::ConstValue::SynthString(_) => {
                    self.builtins.string
                }
            }),
        }
    }

    pub(in crate::llvm::codegen) fn lir_local_type_id(
        &self,
        body: &LirExecutableBody,
        local: crate::effect_lowered::mir_source::LocalId,
    ) -> Option<TypeId> {
        body.locals()
            .get(local.as_u32() as usize)
            .map(LirLocalDecl::ty)
    }

    pub(in crate::llvm::codegen) fn lir_operand_type_id(
        &self,
        body: &LirExecutableBody,
        operand: &LirOperand,
    ) -> Option<TypeId> {
        match operand {
            LirOperand::Local(local) => self.lir_local_type_id(body, *local),
            LirOperand::Const(value) => Some(match value {
                mir_source::ConstValue::Bool(_) => self.builtins.bool_,
                mir_source::ConstValue::Char => self.builtins.char_,
                mir_source::ConstValue::Unit => self.builtins.unit,
                mir_source::ConstValue::Int | mir_source::ConstValue::SynthInt(_) => {
                    self.builtins.int
                }
                mir_source::ConstValue::Float64 => self.builtins.float64,
                mir_source::ConstValue::Float32 => self.builtins.float32,
                mir_source::ConstValue::String | mir_source::ConstValue::SynthString(_) => {
                    self.builtins.string
                }
            }),
        }
    }

    pub(in crate::llvm::codegen) fn mir_operand_function_type(
        &self,
        body: &mir_source::Body,
        mir_types: &TypeStore,
        operand: &mir_source::Operand,
    ) -> Option<crate::ty::FunctionType> {
        let ty = self.mir_operand_type_id(body, operand)?;
        match mir_types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Function(fun_ty)) => {
                self.equivalent_codegen_function_type(mir_types, fun_ty)
            }
            _ => None,
        }
    }

    pub(in crate::llvm::codegen) fn lir_operand_function_type(
        &self,
        body: &LirExecutableBody,
        source_types: &TypeStore,
        operand: &LirOperand,
    ) -> Option<crate::ty::FunctionType> {
        let ty = self.lir_operand_type_id(body, operand)?;
        match source_types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Function(fun_ty)) => {
                self.equivalent_codegen_function_type(source_types, fun_ty)
            }
            _ => None,
        }
    }

    pub(in crate::llvm::codegen) fn lir_operand_funptr_function_type(
        &self,
        body: &LirExecutableBody,
        source_types: &TypeStore,
        operand: &LirOperand,
    ) -> Option<crate::ty::FunctionType> {
        let ty = self.lir_operand_type_id(body, operand)?;
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = source_types.kind(ty) else {
            return None;
        };
        if nominal.fqn != "scoop.unsafe.FunPtr" || nominal.args.len() != 1 {
            return None;
        }
        let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = source_types.kind(nominal.args[0])
        else {
            return None;
        };
        self.equivalent_codegen_function_type(source_types, fun_ty)
    }

    pub(in crate::llvm::codegen) fn mir_callable_fqn_may_outward_effect(
        &self,
        callable_fqn: &str,
    ) -> Option<bool> {
        if let Some((_, callable_types, callable_fun)) = self.lir_source_callable(callable_fqn) {
            return Some(
                mir_source::summarize_pass_rewritten_fun(callable_fun, callable_types, None)
                    .may_outward_effect,
            );
        }
        self.published_codegen_callable_signature(callable_fqn)
            .map(|_| {
                self.direct_call_abi_identity(callable_fqn)
                    .uses_effect_bridge_abi()
            })
    }

    pub(in crate::llvm::codegen) fn mir_fun_value_callee_fqn(
        &self,
        body: &mir_source::Body,
        mir_types: &TypeStore,
        operand: &mir_source::Operand,
    ) -> Option<String> {
        let mut visiting = HashSet::new();
        self.mir_callable_value_fqn_for_operand(body, mir_types, operand, &mut visiting)
    }

    pub(in crate::llvm::codegen) fn lir_fun_value_callee_key(
        &self,
        body: &LirExecutableBody,
        operand: &LirOperand,
    ) -> Option<String> {
        let mut visiting = HashSet::new();
        self.lir_callable_value_key_for_operand(body, operand, &mut visiting)
    }

    fn lir_callable_value_key_for_operand(
        &self,
        body: &LirExecutableBody,
        operand: &LirOperand,
        visiting: &mut HashSet<crate::effect_lowered::mir_source::LocalId>,
    ) -> Option<String> {
        let LirOperand::Local(local) = operand else {
            return None;
        };
        self.lir_callable_value_key_for_local(body, *local, visiting)
    }

    fn lir_callable_value_key_for_local(
        &self,
        body: &LirExecutableBody,
        local: crate::effect_lowered::mir_source::LocalId,
        visiting: &mut HashSet<crate::effect_lowered::mir_source::LocalId>,
    ) -> Option<String> {
        if !visiting.insert(local) {
            return None;
        }

        let mut matched: Option<String> = None;
        for state in body.states().states() {
            for stmt in state.body().statements() {
                let LirStatementKind::Assign { target, value } = &stmt.kind else {
                    continue;
                };
                if *target != local {
                    continue;
                }
                let candidate = self.lir_callable_value_key_for_rvalue(body, value, visiting)?;
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

    fn lir_callable_value_key_for_rvalue(
        &self,
        body: &LirExecutableBody,
        value: &LirRvalue,
        visiting: &mut HashSet<crate::effect_lowered::mir_source::LocalId>,
    ) -> Option<String> {
        match value {
            LirRvalue::Use(operand) | LirRvalue::Transport { value: operand, .. } => {
                self.lir_callable_value_key_for_operand(body, operand, visiting)
            }
            LirRvalue::TopLevelRef(top) => match &top.target {
                LirTopLevelRefTarget::Global(root) => Some(root.as_str().to_string()),
                LirTopLevelRefTarget::Callable(id) => self
                    .published_late_lowered_program()
                    .and_then(|program| program.callable_by_id(*id))
                    .map(|callable| callable.root_fqn().to_string()),
            },
            LirRvalue::MakeClosure { fn_ptr, .. } => self
                .published_late_lowered_program()
                .and_then(|program| program.callable_by_id(*fn_ptr))
                .map(|callable| callable.root_fqn().to_string()),
            LirRvalue::MemberAccess { member, .. } => match &member.resolved {
                LirMemberTarget::Fun { callable } | LirMemberTarget::ExtensionFun { callable } => {
                    self.published_late_lowered_program()
                        .and_then(|program| program.callable_by_id(*callable))
                        .map(|callable| callable.root_fqn().to_string())
                }
                LirMemberTarget::Value { .. } | LirMemberTarget::ExtensionValue { .. } => None,
            },
            LirRvalue::Call { .. }
            | LirRvalue::TypeCheck { .. }
            | LirRvalue::Cast { .. }
            | LirRvalue::SizeOf { .. }
            | LirRvalue::KindOf { .. }
            | LirRvalue::AlignOf { .. }
            | LirRvalue::DescOf { .. }
            | LirRvalue::TypeMetadataLiteral(_)
            | LirRvalue::EnumVariant { .. }
            | LirRvalue::ClassCtor { .. }
            | LirRvalue::MakeTuple { .. }
            | LirRvalue::StructLit { .. }
            | LirRvalue::InterpolatedString { .. }
            | LirRvalue::TupleGet { .. }
            | LirRvalue::PatternMatch { .. }
            | LirRvalue::PatternExtract { .. }
            | LirRvalue::PerformResult { .. } => None,
        }
    }

    pub(in crate::llvm::codegen) fn mir_callable_value_fqn_for_operand(
        &self,
        body: &mir_source::Body,
        mir_types: &TypeStore,
        operand: &mir_source::Operand,
        visiting: &mut HashSet<mir_source::LocalId>,
    ) -> Option<String> {
        let mir_source::Operand::Local(local) = operand else {
            return None;
        };
        self.mir_callable_value_fqn_for_local(body, mir_types, *local, visiting)
    }

    pub(in crate::llvm::codegen) fn mir_callable_value_fqn_for_local(
        &self,
        body: &mir_source::Body,
        mir_types: &TypeStore,
        local: mir_source::LocalId,
        visiting: &mut HashSet<mir_source::LocalId>,
    ) -> Option<String> {
        if !visiting.insert(local) {
            return None;
        }

        let mut matched: Option<String> = None;
        for block in &body.blocks {
            for stmt in &block.stmts {
                let mir_source::StatementKind::Assign { target, value } = &stmt.kind else {
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
        body: &mir_source::Body,
        mir_types: &TypeStore,
        value: &mir_source::Rvalue,
        visiting: &mut HashSet<mir_source::LocalId>,
    ) -> Option<String> {
        match value {
            mir_source::Rvalue::Use(operand)
            | mir_source::Rvalue::Transport { value: operand, .. } => {
                self.mir_callable_value_fqn_for_operand(body, mir_types, operand, visiting)
            }
            mir_source::Rvalue::TopLevelRef(mir_source::TopLevelRef { fqn, .. }) => {
                Some(fqn.clone())
            }
            mir_source::Rvalue::MakeClosure { fn_ptr, .. } => Some(fn_ptr.clone()),
            mir_source::Rvalue::MemberAccess { member, .. } => match member.resolved.as_ref()? {
                mir_source::MemberTarget::Fun { fqn }
                | mir_source::MemberTarget::ExtensionFun { fqn } => Some(fqn.clone()),
                mir_source::MemberTarget::Value { .. }
                | mir_source::MemberTarget::ExtensionValue { .. } => None,
            },
            mir_source::Rvalue::Call {
                kind: mir_source::CallKind::Direct { callee_fqn, .. },
                args,
                ..
            } => self.mir_callable_value_fqn_from_direct_call(
                body, mir_types, callee_fqn, args, visiting,
            ),
            mir_source::Rvalue::UnresolvedName { .. }
            | mir_source::Rvalue::TypeCheck { .. }
            | mir_source::Rvalue::Cast { .. }
            | mir_source::Rvalue::SizeOf { .. }
            | mir_source::Rvalue::KindOf { .. }
            | mir_source::Rvalue::AlignOf { .. }
            | mir_source::Rvalue::DescOf { .. }
            | mir_source::Rvalue::TypeMetadataLiteral(_)
            | mir_source::Rvalue::EnumVariant { .. }
            | mir_source::Rvalue::ClassCtor { .. }
            | mir_source::Rvalue::Call { .. }
            | mir_source::Rvalue::MakeTuple { .. }
            | mir_source::Rvalue::StructLit { .. }
            | mir_source::Rvalue::InterpolatedString { .. }
            | mir_source::Rvalue::TupleGet { .. }
            | mir_source::Rvalue::PatternMatch { .. }
            | mir_source::Rvalue::PatternExtract { .. }
            | mir_source::Rvalue::PerformResult { .. }
            | mir_source::Rvalue::Todo(_) => None,
        }
    }

    pub(in crate::llvm::codegen) fn mir_callable_value_fqn_from_direct_call(
        &self,
        body: &mir_source::Body,
        mir_types: &TypeStore,
        callee_fqn: &str,
        args: &[mir_source::CallArg],
        visiting: &mut HashSet<mir_source::LocalId>,
    ) -> Option<String> {
        let (_, callee_types, callable_fun) = self.lir_source_callable(callee_fqn)?;
        let summary = mir_source::summarize_pass_rewritten_fun(callable_fun, callee_types, None);
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
        body: &mir_source::Body,
        mir_types: &TypeStore,
        result: &mir_source::ResultProvenance,
        params: &[mir_source::Param],
        args: &[mir_source::CallArg],
        visiting: &mut HashSet<mir_source::LocalId>,
    ) -> Option<String> {
        match result {
            mir_source::ResultProvenance::DirectFunction(fqn)
            | mir_source::ResultProvenance::KnownClosure(fqn) => Some(fqn.clone()),
            mir_source::ResultProvenance::Param(index) => self
                .mir_callable_value_fqn_from_param_result(
                    body, mir_types, *index, params, args, visiting,
                ),
            mir_source::ResultProvenance::Join(sources) if sources.len() == 1 => self
                .mir_callable_value_fqn_from_result_source(
                    body,
                    mir_types,
                    &sources[0],
                    params,
                    args,
                    visiting,
                ),
            mir_source::ResultProvenance::Unit
            | mir_source::ResultProvenance::TopLevelValue(_)
            | mir_source::ResultProvenance::PerformResult(_)
            | mir_source::ResultProvenance::Join(_)
            | mir_source::ResultProvenance::Unknown => None,
        }
    }

    pub(in crate::llvm::codegen) fn mir_callable_value_fqn_from_result_source(
        &self,
        body: &mir_source::Body,
        mir_types: &TypeStore,
        source: &mir_source::ResultProvenanceSource,
        params: &[mir_source::Param],
        args: &[mir_source::CallArg],
        visiting: &mut HashSet<mir_source::LocalId>,
    ) -> Option<String> {
        match source {
            mir_source::ResultProvenanceSource::DirectFunction(fqn)
            | mir_source::ResultProvenanceSource::KnownClosure(fqn) => Some(fqn.clone()),
            mir_source::ResultProvenanceSource::Param(index) => self
                .mir_callable_value_fqn_from_param_result(
                    body, mir_types, *index, params, args, visiting,
                ),
            mir_source::ResultProvenanceSource::TopLevelValue(_)
            | mir_source::ResultProvenanceSource::PerformResult(_) => None,
        }
    }

    pub(in crate::llvm::codegen) fn mir_callable_value_fqn_from_param_result(
        &self,
        body: &mir_source::Body,
        mir_types: &TypeStore,
        index: usize,
        params: &[mir_source::Param],
        args: &[mir_source::CallArg],
        visiting: &mut HashSet<mir_source::LocalId>,
    ) -> Option<String> {
        let bound_args = bind_mir_call_args_to_params(params, args)?;
        let operand = bound_args.get(index)?;
        self.mir_callable_value_fqn_for_operand(body, mir_types, operand, visiting)
    }

    pub(in crate::llvm::codegen) fn mir_operand_funptr_function_type(
        &self,
        body: &mir_source::Body,
        mir_types: &TypeStore,
        operand: &mir_source::Operand,
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
                let TypeKind::Value(ValueTypeKind::Tuple(elements)) =
                    self.types.kind(tuple_ty.inner())
                else {
                    return None;
                };
                let elements = elements.clone();
                let mut out = Vec::with_capacity(elements.len());
                for elem_ty in elements {
                    let cg = self.try_cg_ty_of_type_id(elem_ty)?;
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
        contract: &mir_source::ClosureEnvTransportMetadata,
    ) -> Result<Vec<CgTy>, LlvmEmitError> {
        let contract_env_cg = self.cg_ty_of_mir_type(mir_types, contract.env_ty).unwrap_or_else(|| {
            panic!("mir_closure_env_capture_element_cg_tys_from_contract: TypeStore equivalence verifier accepted unsupported closure env contract codegen type")
        });
        if contract_env_cg != env_cg {
            panic!(
                "mir_closure_env_capture_element_cg_tys_from_contract: MIR verifier accepted closure env contract type drift"
            )
        }

        let capture_field_cgs = self
            .mir_closure_env_capture_element_cg_tys(env_cg)
            .unwrap_or_else(|| {
                panic!("mir_closure_env_capture_element_cg_tys_from_contract: MIR verifier accepted non-tuple closure env")
            });
        if capture_field_cgs.len() != contract.captures.len() {
            panic!(
                "mir_closure_env_capture_element_cg_tys_from_contract: MIR verifier accepted closure env capture arity drift"
            )
        }

        let env_transport = mir_source::ValueTransportMetadata {
            source_ty: contract.env_ty,
            kind: mir_source::MirTransportKind::ClosureEnv,
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
            _ => panic!(
                "mir_closure_env_capture_element_cg_tys_from_contract: MIR verifier accepted non-tuple closure env contract"
            ),
        };

        for (index, capture) in contract.captures.iter().enumerate() {
            let env_element_ty =
                env_element_tys
                    .get(index)
                    .copied()
                    .unwrap_or_else(|| {
                        panic!("mir_closure_env_capture_element_cg_tys_from_contract: MIR verifier accepted missing closure env capture element")
                    });
            if mir_types.display(capture.transport.source_ty).to_string()
                != mir_types.display(env_element_ty).to_string()
            {
                panic!(
                    "mir_closure_env_capture_element_cg_tys_from_contract: MIR verifier accepted closure env capture type drift"
                )
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

    pub(in crate::llvm::codegen) fn mir_local_slot(
        &self,
        _span: crate::span::Span,
        slots: &[MirLocalSlot<'ctx>],
        local: mir_source::LocalId,
    ) -> Result<MirLocalSlot<'ctx>, LlvmEmitError> {
        slots
            .get(local.as_u32() as usize)
            .copied()
            .ok_or_else(|| std::panic::panic_any("MIR local must have an allocated slot"))
    }

    pub(in crate::llvm::codegen) fn load_mir_local(
        &mut self,
        span: crate::span::Span,
        slot: MirLocalSlot<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match slot.cg_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                let local_ptr = self.local_ptr_for_use(
                    span,
                    CgLocal {
                        hir_ty: None,
                        call_may_suspend: false,
                        ty: slot.cg_ty,
                        ptr: slot.ptr,
                        frame_backing_ptr: None,
                        mutable: false,
                    },
                    "pass_mir_load_slot",
                )?;
                let llvm_ty = self.llvm_basic_type_of(span, slot.cg_ty)?;
                let loaded = self
                    .builder
                    .build_load(llvm_ty, local_ptr, "pass_mir_load")?;
                self.cg_value_from_loaded(span, slot.cg_ty, loaded)
            }
        }
    }
}
