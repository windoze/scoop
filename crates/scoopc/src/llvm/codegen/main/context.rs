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

    pub(in crate::llvm::codegen) fn current_top_level_fun_call_binding(
        &self,
        span: crate::span::Span,
    ) -> Result<Option<&ast::TopLevelFunCallBinding>, LlvmEmitError> {
        let call_site = self.current_call_site(span)?;
        Ok(self.top_level_fun_call_sites.get(&call_site))
    }

    pub(in crate::llvm::codegen) fn concrete_top_level_fun_call_fqn(
        &self,
        span: crate::span::Span,
        fallback_fqn: &str,
    ) -> Result<String, LlvmEmitError> {
        fn callable_dispatch_base_fqn(fqn: &str) -> &str {
            let base = fqn.rsplit_once("::<").map(|(base, _)| base).unwrap_or(fqn);
            base.split_once("$overload$")
                .map(|(base, _)| base)
                .unwrap_or(base)
        }

        fn callable_fqn_specificity(fqn: &str) -> u8 {
            u8::from(fqn.contains("$overload$")) + u8::from(fqn.contains("::<"))
        }

        fn type_contains_param(types: &TypeStore, ty: TypeId) -> bool {
            let mut stack = vec![ty];
            while let Some(id) = stack.pop() {
                match types.kind(id) {
                    TypeKind::Param(_) => return true,
                    TypeKind::StarProjection(star) => stack.push(star.read_ty),
                    TypeKind::Ref(RefTypeKind::Nominal(nominal))
                    | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                        stack.extend(nominal.args.iter().copied());
                        if let Some(eff) = &nominal.eff {
                            stack.extend(eff.terms.iter().copied());
                        }
                    }
                    TypeKind::Ref(RefTypeKind::Function(fun)) => {
                        if let Some(receiver) = fun.receiver {
                            stack.push(receiver);
                        }
                        stack.extend(fun.params.iter().copied());
                        stack.push(fun.return_ty);
                        stack.extend(fun.effects.terms.iter().copied());
                    }
                    TypeKind::Ref(RefTypeKind::Union(union)) => {
                        stack.extend(union.variants.iter().copied());
                    }
                    TypeKind::Value(ValueTypeKind::Option(inner)) => stack.push(*inner),
                    TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                        stack.extend(elements.iter().copied());
                    }
                    TypeKind::Ref(RefTypeKind::Any | RefTypeKind::String)
                    | TypeKind::Value(
                        ValueTypeKind::Unit
                        | ValueTypeKind::Nothing
                        | ValueTypeKind::Bool
                        | ValueTypeKind::Char
                        | ValueTypeKind::Float64
                        | ValueTypeKind::Float32
                        | ValueTypeKind::Int
                        | ValueTypeKind::UInt
                        | ValueTypeKind::IntN(_)
                        | ValueTypeKind::UIntN(_),
                    ) => {}
                }
            }
            false
        }

        let Some(binding) = self.current_top_level_fun_call_binding(span)? else {
            return Ok(fallback_fqn.to_string());
        };
        let binding_fqn = if binding.type_args.is_empty() {
            binding.fqn.clone()
        } else {
            let args = binding
                .type_args
                .iter()
                .map(|ty| self.types.display(*ty).to_string())
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}::<{}>", binding.fqn, args)
        };
        let binding_contains_unresolved_params = binding
            .type_args
            .iter()
            .any(|&ty| type_contains_param(self.types, ty))
            || binding.eff_args.iter().any(|row| {
                row.terms
                    .iter()
                    .any(|&ty| type_contains_param(self.types, ty))
            });
        if binding_contains_unresolved_params && callable_fqn_specificity(fallback_fqn) > 0 {
            return Ok(fallback_fqn.to_string());
        }
        let fallback_base = callable_dispatch_base_fqn(fallback_fqn);
        let binding_base = callable_dispatch_base_fqn(&binding_fqn);
        if binding_base == fallback_base
            && callable_fqn_specificity(binding_fqn.as_str())
                < callable_fqn_specificity(fallback_fqn)
        {
            return Ok(fallback_fqn.to_string());
        }
        // materialized MIR 已经可以把 where-bound/interface dispatch 等 source-level binding
        // 具体化为不同 base 的 direct callee；此时必须信任 MIR 中的 fallback target，不能再按
        // 原始 source-span side table 回退到抽象接口/trait base FQN。
        if binding_base != fallback_base {
            return Ok(fallback_fqn.to_string());
        }
        Ok(binding_fqn)
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
        self.top_level_vars
            .get(fqn)
            .map(|var| var.ty)
            .or_else(|| {
                self.materialized_extern_global_root(fqn)
                    .map(|global| global.ty)
            })
            .or_else(|| self.extern_globals.get(fqn).map(|global| global.ty))
            .or_else(|| self.top_level_consts.get(fqn).map(|value| value.ty))
            .or_else(|| {
                self.top_level_immutable_values
                    .get(fqn)
                    .map(|value| value.ty)
            })
    }

    pub(in crate::llvm::codegen) fn materialized_extern_global_root(
        &self,
        fqn: &str,
    ) -> Option<&crate::mir::ExternGlobalRoot> {
        self.materialized_pass_view()?
            .materialized()
            .file
            .items
            .iter()
            .find_map(|item| match item {
                crate::mir::Item::ExternGlobal(root) if root.fqn == fqn => Some(root),
                _ => None,
            })
    }

    pub(in crate::llvm::codegen) fn has_extern_global_contract(&self, fqn: &str) -> bool {
        self.materialized_extern_global_root(fqn).is_some() || self.extern_globals.contains_key(fqn)
    }

    pub(in crate::llvm::codegen) fn source_slice_at(
        &self,
        source_id: SourceId,
        span: crate::span::Span,
    ) -> Result<&str, LlvmEmitError> {
        let bound = self.source_map.bind_span(source_id, span).map_err(|_| {
            LlvmEmitError::UnsupportedMainBody {
                kind: "source-backed literal span",
                at: span.into(),
            }
        })?;
        self.source_map
            .slice(bound)
            .map_err(|_| LlvmEmitError::UnsupportedMainBody {
                kind: "source-backed literal slice",
                at: span.into(),
            })
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
        Ok(parse_int_literal(self.current_source_slice(span)?))
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
        let Ok(text) = self.current_source_slice(span) else {
            return Ok(None);
        };
        let source = self.current_source()?;
        let Some((negative, body)) = source_text_int_literal_body(text) else {
            return Ok(None);
        };
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
