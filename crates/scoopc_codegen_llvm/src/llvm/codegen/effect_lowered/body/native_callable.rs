use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(crate) fn codegen_native_callable_body_symbols(
        &mut self,
        abi: &ProgramAbiQuery<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let mut entries = self
            .native_callable_funs
            .iter()
            .map(|(fqn, callable)| (fqn.clone(), callable.clone()))
            .collect::<Vec<_>>();
        entries.sort_by(|(left, _), (right, _)| left.cmp(right));

        for (fqn, callable) in entries {
            self.codegen_native_callable_body_symbol(&fqn, &callable, abi)?;
        }

        Ok(())
    }

    fn codegen_native_callable_body_symbol(
        &mut self,
        fqn: &str,
        callable: &crate::effect_lowered::source::NativeCallableFun,
        abi: &ProgramAbiQuery<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let span = self
            .callable_sources
            .get(fqn)
            .map(|source| source.span)
            .unwrap_or_else(|| crate::span::Span::new(0, 0));
        let program = self.expect_active_lir_program("codegen_native_callable_body_symbol");
        let callable_id = program
            .physical_layout()
            .callable_symbols
            .iter()
            .find_map(|(id, facts)| {
                facts
                    .native
                    .as_ref()
                    .is_some_and(|native| native.symbol == callable.symbol)
                    .then_some(*id)
            })
            .ok_or_else(|| {
                frontend_error(format!(
                    "`@CallingConvention` native callable `{fqn}` 缺少 LIR callable symbol handle"
                ))
            })?;
        let target = scoopc_lir_facts::LirCallableRef::Local(callable_id);
        let signature = self
            .published_codegen_callable_signature_for_ref(target)
            .ok_or_else(|| {
                frontend_error(format!(
                    "`@CallingConvention` native callable `{fqn}` 缺少 LIR callable signature"
                ))
            })?;
        let plain = abi.plain_callable_layout_for_id(program, callable_id)?;
        let plain_entry = plain.direct_entry();
        if plain_entry.param_tys().len() != signature.param_tys.len() {
            return Err(frontend_error(format!(
                "`@CallingConvention` native callable `{fqn}` 的 plain ABI 参数数量漂移：layout={} lir={}",
                plain_entry.param_tys().len(),
                signature.param_tys.len()
            )));
        }

        let native_abi = self.classify_native_callable_body_symbol(
            span,
            &signature.param_tys,
            signature.return_ty,
            &callable.calling_convention,
        )?;
        let wrapper = self.declare_native_callable_body_wrapper(&callable.symbol, &native_abi)?;
        if wrapper.count_basic_blocks() > 0 {
            return Ok(());
        }

        let plain_fun = self.function(plain_entry.symbol_name())?;
        let entry = self.context.append_basic_block(wrapper, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(wrapper)?;

        let sret_result_ty = self.native_callable_plain_sret_result_ty(
            fqn,
            plain_entry,
            native_abi.return_abi.llvm_return_ty,
        )?;
        let mut call_args = Vec::<BasicMetadataValueEnum<'ctx>>::new();
        let sret_slot = if let Some(result_ty) = sret_result_ty {
            let slot =
                self.create_entry_alloca_raw(span, "native_callable_plain_sret", result_ty)?;
            call_args.push(slot.into());
            Some((slot, result_ty))
        } else {
            None
        };

        for index in 0..native_abi.param_abis.len() {
            let param = wrapper.get_nth_param(index as u32).ok_or_else(|| {
                frontend_error(format!(
                    "`@CallingConvention` wrapper `{}` 缺少参数 #{}",
                    callable.symbol, index
                ))
            })?;
            call_args.push(param.into());
        }

        let call = self.build_call_preserving_gc_local_roots(
            span,
            plain_fun,
            &call_args,
            "native_to_plain",
        )?;
        if let Some((_, result_ty)) = sret_slot {
            self.add_sret_attribute_to_call(call, 0, result_ty);
        }

        match native_abi.return_abi.llvm_return_ty {
            Some(return_ty) => {
                let value = if let Some((slot, result_ty)) = sret_slot {
                    self.builder
                        .build_load(result_ty, slot, "native_callable_sret_load")?
                } else {
                    call.try_as_basic_value().basic().ok_or_else(|| {
                        frontend_error(format!(
                            "`@CallingConvention` wrapper `{}` 的 plain call 未返回值",
                            callable.symbol
                        ))
                    })?
                };
                if value.get_type() != return_ty {
                    return Err(frontend_error(format!(
                        "`@CallingConvention` wrapper `{}` 返回类型漂移：expected {:?}, got {:?}",
                        callable.symbol,
                        return_ty,
                        value.get_type()
                    )));
                }
                self.builder.build_return(Some(&value))?;
            }
            None => {
                self.builder.build_return(None)?;
            }
        }

        self.finish_function_explicit_frame_layout(span)?;
        Ok(())
    }

    fn declare_native_callable_body_wrapper(
        &self,
        symbol: &str,
        native_abi: &NativeCallableAbi<'ctx>,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        if let Some(existing) = self.module.get_function(symbol) {
            let defined = if existing.count_basic_blocks() > 0 {
                "已有定义"
            } else {
                "已有声明"
            };
            return Err(frontend_error(format!(
                "`@CallingConvention` object symbol `{symbol}` 与既有 LLVM function 冲突（{defined}）"
            )));
        }

        let function = self.declare_exported_abi_function(symbol, native_abi.fn_ty);
        function.set_call_conventions(native_abi.call_convention);
        Ok(function)
    }

    fn native_callable_plain_sret_result_ty(
        &self,
        fqn: &str,
        plain_entry: &PlainCallableEntryLayout<'ctx>,
        native_return_ty: Option<BasicTypeEnum<'ctx>>,
    ) -> Result<Option<BasicTypeEnum<'ctx>>, LlvmEmitError> {
        let source_param_count = plain_entry.param_tys().len();
        let plain_param_count = plain_entry.param_count();
        let native_return_needs_sret = native_return_ty.is_some_and(Self::llvm_type_needs_sret);

        match (plain_param_count, native_return_needs_sret) {
            (count, true) if count == source_param_count + 1 => Ok(native_return_ty),
            (count, _) if count == source_param_count => Ok(None),
            (count, true) => Err(frontend_error(format!(
                "`@CallingConvention` native callable `{fqn}` plain entry 参数数量漂移：entry={count} expected={} 或 {}",
                source_param_count,
                source_param_count + 1,
            ))),
            (count, false) => Err(frontend_error(format!(
                "`@CallingConvention` native callable `{fqn}` plain entry 参数数量漂移：entry={count} expected={source_param_count}"
            ))),
        }
    }
}
