//! Function body / suspend lowering context: take/restore body cx, scoped suspend lowering, source/span/literal helpers.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    /// Return the active LLVM insertion block, or panic with a named compiler invariant.
    pub(in crate::llvm::codegen) fn expect_insert_block(
        &self,
        context: &str,
    ) -> inkwell::basic_block::BasicBlock<'ctx> {
        self.builder.get_insert_block().unwrap_or_else(|| {
            panic!("expect_insert_block: missing LLVM insert block while {context}")
        })
    }

    /// Return the parent function for a block, or panic with a named compiler invariant.
    pub(in crate::llvm::codegen) fn expect_parent_function(
        &self,
        block: inkwell::basic_block::BasicBlock<'ctx>,
        context: &str,
    ) -> FunctionValue<'ctx> {
        block.get_parent().unwrap_or_else(|| {
            panic!("expect_parent_function: block has no parent function while {context}")
        })
    }

    /// Return the active function implied by the insertion block.
    pub(in crate::llvm::codegen) fn expect_current_function(
        &self,
        context: &str,
    ) -> FunctionValue<'ctx> {
        let insert_block = self.expect_insert_block(context);
        self.expect_parent_function(insert_block, context)
    }

    /// Return an instruction's parent block, or panic with a named compiler invariant.
    pub(in crate::llvm::codegen) fn expect_instruction_parent_block(
        &self,
        instruction: Option<inkwell::values::InstructionValue<'ctx>>,
        context: &str,
    ) -> inkwell::basic_block::BasicBlock<'ctx> {
        instruction.and_then(|inst| inst.get_parent()).unwrap_or_else(|| {
            panic!("expect_instruction_parent_block: instruction has no parent block while {context}")
        })
    }

    /// Return an instruction's parent function, or panic with a named compiler invariant.
    pub(in crate::llvm::codegen) fn expect_instruction_parent_function(
        &self,
        instruction: Option<inkwell::values::InstructionValue<'ctx>>,
        context: &str,
    ) -> FunctionValue<'ctx> {
        let block = self.expect_instruction_parent_block(instruction, context);
        self.expect_parent_function(block, context)
    }

    /// Return a function's entry block, or panic with a named compiler invariant.
    pub(in crate::llvm::codegen) fn expect_entry_block(
        &self,
        function: FunctionValue<'ctx>,
        context: &str,
    ) -> inkwell::basic_block::BasicBlock<'ctx> {
        function.get_first_basic_block().unwrap_or_else(|| {
            panic!("expect_entry_block: function has no entry block while {context}")
        })
    }

    /// Return a call's basic value result, or panic with a named compiler invariant.
    pub(in crate::llvm::codegen) fn expect_basic_value(
        &self,
        call: CallSiteValue<'ctx>,
        context: &str,
    ) -> BasicValueEnum<'ctx> {
        call.try_as_basic_value().basic().unwrap_or_else(|| {
            panic!("expect_basic_value: call did not produce a basic value while {context}")
        })
    }

    /// Return a pointer value, or panic with a named compiler invariant.
    pub(in crate::llvm::codegen) fn expect_pointer_value(
        &self,
        value: BasicValueEnum<'ctx>,
        context: &str,
    ) -> PointerValue<'ctx> {
        match value {
            BasicValueEnum::PointerValue(ptr) => ptr,
            _ => panic!("expect_pointer_value: value was not a pointer while {context}"),
        }
    }

    /// Return an integer value, or panic with a named compiler invariant.
    pub(in crate::llvm::codegen) fn expect_int_value(
        &self,
        value: BasicValueEnum<'ctx>,
        context: &str,
    ) -> IntValue<'ctx> {
        match value {
            BasicValueEnum::IntValue(value) => value,
            _ => panic!("expect_int_value: value was not an integer while {context}"),
        }
    }

    /// Return a float value, or panic with a named compiler invariant.
    pub(in crate::llvm::codegen) fn expect_float_value(
        &self,
        value: BasicValueEnum<'ctx>,
        context: &str,
    ) -> FloatValue<'ctx> {
        match value {
            BasicValueEnum::FloatValue(value) => value,
            _ => panic!("expect_float_value: value was not a float while {context}"),
        }
    }

    /// Return a struct value, or panic with a named compiler invariant.
    pub(in crate::llvm::codegen) fn expect_struct_value(
        &self,
        value: BasicValueEnum<'ctx>,
        context: &str,
    ) -> inkwell::values::StructValue<'ctx> {
        match value {
            BasicValueEnum::StructValue(value) => value,
            _ => panic!("expect_struct_value: value was not a struct while {context}"),
        }
    }

    /// Return a bool CgValue payload, or panic with a named compiler invariant.
    pub(in crate::llvm::codegen) fn expect_cg_bool(
        &self,
        value: CgValue<'ctx>,
        context: &str,
    ) -> IntValue<'ctx> {
        value.as_bool().unwrap_or_else(|| {
            panic!("expect_cg_bool: value was not a bool payload while {context}")
        })
    }

    /// Return an integer CgValue payload, or panic with a named compiler invariant.
    pub(in crate::llvm::codegen) fn expect_cg_int(
        &self,
        value: CgValue<'ctx>,
        context: &str,
    ) -> (IntValue<'ctx>, IntTy) {
        value.as_int().unwrap_or_else(|| {
            panic!("expect_cg_int: value was not an integer payload while {context}")
        })
    }

    /// Return a float CgValue payload, or panic with a named compiler invariant.
    pub(in crate::llvm::codegen) fn expect_cg_float(
        &self,
        value: CgValue<'ctx>,
        context: &str,
    ) -> (FloatValue<'ctx>, CgTy) {
        value.as_float().unwrap_or_else(|| {
            panic!("expect_cg_float: value was not a float payload while {context}")
        })
    }

    /// Return a pointer CgValue payload, or panic with a named compiler invariant.
    pub(in crate::llvm::codegen) fn expect_cg_pointer(
        &self,
        value: CgValue<'ctx>,
        context: &str,
    ) -> PointerValue<'ctx> {
        let raw = value.value.unwrap_or_else(|| {
            panic!("expect_cg_pointer: value did not publish a payload while {context}")
        });
        self.expect_pointer_value(raw, context)
    }

    /// Return any published CgValue payload, or panic with a named compiler invariant.
    pub(in crate::llvm::codegen) fn expect_cg_value(
        &self,
        value: CgValue<'ctx>,
        context: &str,
    ) -> BasicValueEnum<'ctx> {
        value.value.unwrap_or_else(|| {
            panic!("expect_cg_value: value did not publish a payload while {context}")
        })
    }

    pub(in crate::llvm::codegen) fn take_function_body_cx(
        &mut self,
    ) -> FunctionBodyCodegenCx<'ctx> {
        std::mem::take(&mut self.function_cx)
    }

    pub(in crate::llvm::codegen) fn restore_function_body_cx(
        &mut self,
        function_cx: FunctionBodyCodegenCx<'ctx>,
    ) {
        self.function_cx = function_cx;
    }

    pub(in crate::llvm::codegen) fn take_suspend_site_explicit_effect_outcome(
        &mut self,
        site_id: u32,
    ) -> Option<PointerValue<'ctx>> {
        self.effect_cx
            .suspend_site_effect_outcomes
            .explicit_outcomes
            .remove(&site_id)
    }

    /// 在某段 lowering 内临时安装 ordinary callee suspend/replay 状态。
    pub(in crate::llvm::codegen) fn with_callee_suspend_lowering<T, F>(
        &mut self,
        current_suspend_plan: Option<CalleeSuspendPlan>,
        current_resume_entry_fn: Option<FunctionValue<'ctx>>,
        f: F,
    ) -> Result<T, LlvmEmitError>
    where
        F: FnOnce(&mut Self) -> Result<T, LlvmEmitError>,
    {
        let saved_callee_suspend = std::mem::take(&mut self.effect_cx.callee_suspend);
        self.effect_cx.callee_suspend = CalleeSuspendLoweringCodegenCx {
            current_suspend_plan,
            current_resume_entry_fn,
        };
        let result = f(self);
        self.effect_cx.callee_suspend = saved_callee_suspend;
        result
    }

    pub(in crate::llvm::codegen) fn with_active_suspend_site_any_effect_outcome_capture<T, F>(
        &mut self,
        site_id: u32,
        f: F,
    ) -> Result<T, LlvmEmitError>
    where
        F: FnOnce(&mut Self) -> Result<T, LlvmEmitError>,
    {
        let saved_capture = self.effect_cx.suspend_site_effect_outcomes.active_capture;
        self.effect_cx.suspend_site_effect_outcomes.active_capture =
            Some(ActiveSuspendSiteEffectOutcomeCapture {
                site_id,
                call_span: crate::span::Span::new(0, 0),
                capture_any: true,
            });
        self.effect_cx
            .suspend_site_effect_outcomes
            .explicit_outcomes
            .remove(&site_id);
        let result = f(self);
        self.effect_cx.suspend_site_effect_outcomes.active_capture = saved_capture;
        result
    }

    pub(in crate::llvm::codegen) fn with_ordinary_effect_propagation_suppressed<T, F>(
        &mut self,
        f: F,
    ) -> Result<T, LlvmEmitError>
    where
        F: FnOnce(&mut Self) -> Result<T, LlvmEmitError>,
    {
        let saved_return_ty = self.function_cx.current_fun_return_ty.take();
        let result = f(self);
        self.function_cx.current_fun_return_ty = saved_return_ty;
        result
    }

    pub(in crate::llvm::codegen) fn current_local_effect_escape_target(
        &self,
    ) -> Option<inkwell::basic_block::BasicBlock<'ctx>> {
        self.function_cx.local_effect_escape_targets.last().copied()
    }

    pub(in crate::llvm::codegen) fn with_local_effect_escape_target<T, F>(
        &mut self,
        target: inkwell::basic_block::BasicBlock<'ctx>,
        f: F,
    ) -> Result<T, LlvmEmitError>
    where
        F: FnOnce(&mut Self) -> Result<T, LlvmEmitError>,
    {
        self.function_cx.local_effect_escape_targets.push(target);
        let result = f(self);
        let _ = self.function_cx.local_effect_escape_targets.pop();
        result
    }

    pub(in crate::llvm::codegen) fn when_pat_binding_hir_ty(
        &self,
        span: crate::span::Span,
    ) -> Result<Option<TypeId>, LlvmEmitError> {
        let source = self.current_source()?;
        Ok(self
            .when_pat_binding_tys
            .get(&hir::WhenPatBindingSite {
                source_path: source.path().to_path_buf(),
                decl_span: span,
            })
            .copied())
    }

    pub(in crate::llvm::codegen) fn current_source(&self) -> Result<&SourceFile, LlvmEmitError> {
        Ok(self
            .source_map
            .source(self.current_source_id)
            .expect("current source id must be registered before LLVM codegen"))
    }

    pub(in crate::llvm::codegen) fn current_call_site(
        &self,
        span: crate::span::Span,
    ) -> Result<hir::CallSite, LlvmEmitError> {
        let source = self.current_source()?;
        Ok(hir::CallSite::new(source.path().to_path_buf(), span))
    }

    pub(in crate::llvm::codegen) fn published_intrinsic_call_contract(
        &self,
        span: crate::span::Span,
    ) -> Result<Option<&crate::llvm::LlvmIntrinsicCallContract>, LlvmEmitError> {
        let source = self.current_source()?;
        let key = crate::llvm::LlvmSourceCallKey::new(source.path().to_path_buf(), span);
        if let Some(contract) = self.intrinsic_call_contracts.get(&key) {
            return Ok(Some(contract));
        }
        let mut matches = self
            .intrinsic_call_contracts
            .iter()
            .filter(|(key, _)| key.span == span)
            .map(|(_, contract)| contract);
        let Some(contract) = matches.next() else {
            return Ok(None);
        };
        if matches.next().is_some() {
            return Ok(None);
        }
        Ok(Some(contract))
    }

    pub(in crate::llvm::codegen) fn published_instantiated_call_fqn(
        &self,
        span: crate::span::Span,
    ) -> Result<Option<String>, LlvmEmitError> {
        let Some(contract) = self.published_intrinsic_call_contract(span)? else {
            return Ok(None);
        };
        if contract.type_args.is_empty() {
            return Ok(Some(contract.function_fqn.clone()));
        }
        let args = contract
            .type_args
            .iter()
            .map(|ty| self.types.display(*ty).to_string())
            .collect::<Vec<_>>()
            .join(", ");
        Ok(Some(format!("{}::<{}>", contract.function_fqn, args)))
    }

    pub(in crate::llvm::codegen) fn source_id_for_path(
        &self,
        path: &Path,
        _at: crate::span::Span,
    ) -> Result<SourceId, LlvmEmitError> {
        Ok(self
            .source_map
            .source_id_of_path(path)
            .expect("source path must be registered before LLVM codegen"))
    }

    pub(in crate::llvm::codegen) fn top_level_value_ty(&self, fqn: &str) -> Option<TypeId> {
        self.lir_global_root(fqn).and_then(|root| root.ty)
    }

    pub(in crate::llvm::codegen) fn lir_global_root(
        &self,
        fqn: &str,
    ) -> Option<&LirGlobalRootFacts> {
        self.published_lir_facts
            .global_init
            .roots
            .get(&LirGlobalRootKey::new(fqn))
    }

    pub(in crate::llvm::codegen) fn lir_global_root_has_kind(
        &self,
        fqn: &str,
        kind: LirGlobalRootKind,
    ) -> bool {
        self.lir_global_root(fqn)
            .is_some_and(|root| root.kind == kind)
    }

    pub(in crate::llvm::codegen) fn expect_lir_global_root_kind(
        &self,
        fqn: &str,
        kind: LirGlobalRootKind,
        context: &str,
    ) -> &LirGlobalRootFacts {
        let root = self
            .lir_global_root(fqn)
            .unwrap_or_else(|| panic!("{context}: LIR facts are missing global root `{fqn}`"));
        if root.kind != kind {
            panic!(
                "{context}: LIR facts classify global root `{fqn}` as `{}`, expected `{}`",
                root.kind.stable_name(),
                kind.stable_name()
            );
        }
        root
    }

    pub(in crate::llvm::codegen) fn lir_global_root_ty(
        &self,
        root: &LirGlobalRootFacts,
        context: &str,
    ) -> TypeId {
        root.ty.unwrap_or_else(|| {
            panic!(
                "{context}: LIR facts global root `{}` is missing type",
                root.root.as_str()
            )
        })
    }

    pub(in crate::llvm::codegen) fn lir_global_storage_policy_as_hir(
        &self,
        root: &LirGlobalRootFacts,
        context: &str,
    ) -> hir::TopLevelVarStorage {
        match root.storage.unwrap_or_else(|| {
            panic!(
                "{context}: LIR facts global root `{}` is missing storage policy",
                root.root.as_str()
            )
        }) {
            LirGlobalStoragePolicy::Global => hir::TopLevelVarStorage::Global,
            LirGlobalStoragePolicy::ThreadLocal => hir::TopLevelVarStorage::ThreadLocal,
        }
    }

    pub(in crate::llvm::codegen) fn has_extern_global_contract(&self, fqn: &str) -> bool {
        self.lir_global_root_has_kind(fqn, LirGlobalRootKind::ExternGlobal)
    }

    pub(in crate::llvm::codegen) fn source_slice_at(
        &self,
        source_id: SourceId,
        span: crate::span::Span,
    ) -> Result<&str, LlvmEmitError> {
        let bound = self
            .source_map
            .bind_span(source_id, span)
            .unwrap_or_else(|_| {
                panic!("source_slice_at: parser/typecheck accepted a span outside its source")
            });
        let slice = self.source_map.slice(bound).unwrap_or_else(|_| {
            panic!("source_slice_at: parser/typecheck accepted an unsliceable source span")
        });
        Ok(slice)
    }

    pub(in crate::llvm::codegen) fn current_source_slice(
        &self,
        span: crate::span::Span,
    ) -> Result<&str, LlvmEmitError> {
        self.source_slice_at(self.current_source_id, span)
    }

    pub(in crate::llvm::codegen) fn parse_current_int_literal(
        &self,
        span: crate::span::Span,
    ) -> Result<u128, LlvmEmitError> {
        let source = self.current_source()?;
        let text = self.current_source_slice(span)?;
        parse_int_literal_checked(text).map_err(|err| {
            LlvmEmitError::invalid_literal(source, span, "integer literal", err.reason(), text)
        })
    }

    pub(in crate::llvm::codegen) fn int_literal_bits_for_ty(
        &self,
        span: crate::span::Span,
        int_ty: IntTy,
    ) -> Result<u64, LlvmEmitError> {
        let source = self.current_source()?;
        let text = self.current_source_slice(span)?;
        let raw = self.parse_current_int_literal(span)?;
        let bits = checked_positive_int_literal_bits(raw, int_ty).ok_or_else(|| {
            LlvmEmitError::invalid_literal(
                source,
                span,
                "integer literal",
                "超出目标整数类型可表示范围",
                text,
            )
        })?;
        Ok(bits as u64)
    }

    pub(in crate::llvm::codegen) fn int_literal_bits_from_text_for_ty(
        &self,
        span: crate::span::Span,
        text: &str,
        int_ty: IntTy,
    ) -> Result<u64, LlvmEmitError> {
        let source = self.current_source()?;
        let raw = parse_int_literal_checked(text).map_err(|err| {
            LlvmEmitError::invalid_literal(source, span, "integer literal", err.reason(), text)
        })?;
        let bits = checked_positive_int_literal_bits(raw, int_ty).ok_or_else(|| {
            LlvmEmitError::invalid_literal(
                source,
                span,
                "integer literal",
                "超出目标整数类型可表示范围",
                text,
            )
        })?;
        Ok(bits as u64)
    }

    pub(in crate::llvm::codegen) fn negated_int_literal_bits_for_ty(
        &self,
        span: crate::span::Span,
        literal_span: crate::span::Span,
        int_ty: IntTy,
    ) -> Result<u64, LlvmEmitError> {
        let source = self.current_source()?;
        let text = self.current_source_slice(span)?;
        let raw = self.parse_current_int_literal(literal_span)?;
        let bits = checked_negated_int_literal_bits(raw, int_ty).ok_or_else(|| {
            LlvmEmitError::invalid_literal(
                source,
                span,
                "integer literal",
                "超出目标整数类型可表示范围",
                text,
            )
        })?;
        Ok(bits as u64)
    }

    pub(in crate::llvm::codegen) fn int_literal_bits_from_source_span_if_present(
        &self,
        span: crate::span::Span,
        int_ty: IntTy,
    ) -> Result<Option<u64>, LlvmEmitError> {
        let mut candidates = Vec::new();
        if let Ok(bound) = self.source_map.bind_span(self.current_source_id, span)
            && let Ok(text) = self.source_map.slice(bound)
            && let Some((negative, body)) = source_text_int_literal_body(text)
        {
            candidates.push((self.current_source_id, text, negative, body));
        }
        if candidates.is_empty() {
            for source in self.source_map.sources() {
                let Some(source_id) = self.source_map.source_id_of_path(source.path()) else {
                    continue;
                };
                if source_id == self.current_source_id {
                    continue;
                }
                let Ok(bound) = self.source_map.bind_span(source_id, span) else {
                    continue;
                };
                let Ok(text) = self.source_map.slice(bound) else {
                    continue;
                };
                let Some((negative, body)) = source_text_int_literal_body(text) else {
                    continue;
                };
                candidates.push((source_id, text, negative, body));
            }
        }
        let Some((source_id, text, negative, body)) = candidates.into_iter().next() else {
            return Ok(None);
        };
        let source = self
            .source_map
            .source(source_id)
            .expect("candidate source id must exist");
        let raw = parse_int_literal_checked(body).map_err(|err| {
            LlvmEmitError::invalid_literal(source, span, "integer literal", err.reason(), text)
        })?;
        let bits = if negative {
            checked_negated_int_literal_bits(raw, int_ty)
        } else {
            checked_positive_int_literal_bits(raw, int_ty)
        }
        .ok_or_else(|| {
            LlvmEmitError::invalid_literal(
                source,
                span,
                "integer literal",
                "超出目标整数类型可表示范围",
                text,
            )
        })?;
        Ok(Some(bits as u64))
    }

    pub(in crate::llvm::codegen) fn parse_current_string_literal_bytes(
        &self,
        span: crate::span::Span,
    ) -> Result<Vec<u8>, LlvmEmitError> {
        let text = self.current_source_slice(span)?;
        let source = self.current_source()?;
        parse_string_literal_bytes(text).map_err(|err| {
            LlvmEmitError::invalid_literal(
                source,
                span,
                "string literal",
                string_literal_parse_reason(err),
                text,
            )
        })
    }

    /// 获取 effect operation 的稳定 op_tag（T1608）。
    ///
    /// 规则：
    /// - `scoop.core.Raise.raise` → 1（固定；与 runtime 约定兼容）。
    /// - 其余 effect op：首次出现时分配递增编号（从 2 开始），后续查表复用。
    /// - 同一编译单元内 tag 稳定（相同 FQN 总是得到相同 tag）。
    pub(in crate::llvm::codegen) fn effect_op_tag(&mut self, fqn: &str) -> u32 {
        let mut state = self.effect_op_tags.borrow_mut();
        if let Some(&tag) = state.map.get(fqn) {
            return tag;
        }
        let tag = state.next;
        state.next = state.next.saturating_add(1);
        state.map.insert(fqn.to_string(), tag);
        tag
    }
}
