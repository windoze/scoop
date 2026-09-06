use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_callback_instruction(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let function = self.function;
        match instruction {
            Instruction::FunctionAddress { out, symbol } => {
                if function.temps[*out].ty != scoop_lir::CODE_PTR {
                    return Err(CodegenError(format!(
                        "function_address @{} must produce ptr<code>",
                        function.symbol
                    )));
                }
                let function_value = self.llvm.get_function(symbol).ok_or_else(|| {
                    CodegenError(format!(
                        "function_address @{}: unknown function @{}",
                        function.symbol, symbol
                    ))
                })?;
                self.temps.insert(
                    *out,
                    function_value.as_global_value().as_pointer_value().into(),
                );
            }
            Instruction::ForeignCallbackRegister {
                out,
                bridge,
                closure,
            } => {
                let bridge = &self.foreign_callback_bridges[*bridge];
                let family = &self.foreign_callback_families[bridge.family];
                let closure_ty = function.value_ty(self.globals_arena, *closure);
                let out_ty = &function.temps[*out].ty;
                if closure_ty != scoop_lir::MANAGED_PTR
                    || out_ty != &LirType::Struct(family.callback)
                {
                    return Err(CodegenError(format!(
                        "foreign callback registration @{} requires a managed closure and its exact nominal callback result, got closure {} and result {}",
                        function.symbol,
                        closure_ty.dump(),
                        out_ty.dump()
                    )));
                }
                let closure = self.value(*closure)?.into_pointer_value();
                let adapter = self
                    .llvm
                    .get_function(&bridge.adapter_symbol)
                    .ok_or_else(|| {
                        CodegenError(format!(
                            "foreign callback adapter @{} was not emitted",
                            bridge.adapter_symbol
                        ))
                    })?
                    .as_global_value()
                    .as_pointer_value();
                let signature = self
                    .llvm
                    .get_global(&bridge.signature_symbol)
                    .expect("foreign callback signature descriptor is declared")
                    .as_pointer_value();
                let register = self.gc_leaf_fn(
                    "scoop_runtime_callback_register",
                    ptr_ty(context).fn_type(
                        &[
                            managed_ptr_ty(context, self.managed_address_space).into(),
                            ptr_ty(context).into(),
                            ptr_ty(context).into(),
                            context.i32_type().into(),
                        ],
                        false,
                    ),
                );
                let mode = family
                    .modes
                    .runtime_code(bridge.mode)
                    .expect("validated callback bridge mode belongs to its family");
                let callback_context = builder
                    .build_call(
                        register,
                        &[
                            closure.into(),
                            adapter.into(),
                            signature.into(),
                            context.i32_type().const_int(u64::from(mode), false).into(),
                        ],
                        "foreign_callback_context",
                    )
                    .map_err(|error| {
                        CodegenError(format!("foreign callback registration: {error}"))
                    })?
                    .try_as_basic_value()
                    .basic()
                    .expect("callback registration returns a context")
                    .into_pointer_value();
                let trampoline = self
                    .llvm
                    .get_function(&bridge.trampoline_symbol)
                    .expect("foreign callback trampoline is declared")
                    .as_global_value()
                    .as_pointer_value();
                let ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &function.temps[*out].ty,
                )?
                .into_struct_type();
                let value = builder
                    .build_insert_value(ty.get_undef(), trampoline, 0, "callback_function")
                    .and_then(|value| {
                        builder.build_insert_value(
                            value.into_struct_value(),
                            callback_context,
                            1,
                            "callback_value",
                        )
                    })
                    .map_err(|error| {
                        CodegenError(format!("construct foreign callback value: {error}"))
                    })?;
                self.temps.insert(*out, value.into_struct_value().into());
            }
            Instruction::ForeignCallbackOperation(operation) => {
                let family = &self.foreign_callback_families[operation.family()];
                let callback = self.value(operation.callback())?.into_struct_value();
                let function_pointer = builder
                    .build_extract_value(callback, 0, "callback_function")
                    .map_err(|error| CodegenError(format!("extract callback function: {error}")))?
                    .into_pointer_value();
                let callback_context = builder
                    .build_extract_value(callback, 1, "callback_context")
                    .map_err(|error| CodegenError(format!("extract callback context: {error}")))?
                    .into_pointer_value();
                match *operation {
                    scoop_lir::ForeignCallbackOperation::Retain { out, .. } => {
                        let retain = self.gc_leaf_fn(
                            "scoop_runtime_callback_retain",
                            ptr_ty(context).fn_type(&[ptr_ty(context).into()], false),
                        );
                        let retained = builder
                            .build_call(retain, &[callback_context.into()], "retained_context")
                            .map_err(|error| {
                                CodegenError(format!("retain foreign callback: {error}"))
                            })?
                            .try_as_basic_value()
                            .basic()
                            .expect("retain returns a context")
                            .into_pointer_value();
                        let ty = basic_ty(
                            context,
                            self.structs,
                            self.enums,
                            self.managed_address_space,
                            &function.temps[out].ty,
                        )?
                        .into_struct_type();
                        let value = builder
                            .build_insert_value(
                                ty.get_undef(),
                                function_pointer,
                                0,
                                "callback_function",
                            )
                            .and_then(|value| {
                                builder.build_insert_value(
                                    value.into_struct_value(),
                                    retained,
                                    1,
                                    "retained_callback",
                                )
                            })
                            .map_err(|error| {
                                CodegenError(format!("construct retained callback: {error}"))
                            })?;
                        self.temps.insert(out, value.into_struct_value().into());
                    }
                    scoop_lir::ForeignCallbackOperation::Release { .. } => {
                        let release = self.gc_leaf_fn(
                            "scoop_runtime_callback_release",
                            context
                                .void_type()
                                .fn_type(&[ptr_ty(context).into()], false),
                        );
                        builder
                            .build_call(release, &[callback_context.into()], "")
                            .map_err(|error| {
                                CodegenError(format!("release foreign callback: {error}"))
                            })?;
                    }
                    scoop_lir::ForeignCallbackOperation::Failure { out, .. } => {
                        let failure = self.gc_leaf_fn(
                            "scoop_runtime_callback_failure",
                            managed_ptr_ty(context, self.managed_address_space)
                                .fn_type(&[ptr_ty(context).into()], false),
                        );
                        let value = builder
                            .build_call(failure, &[callback_context.into()], "callback_failure")
                            .map_err(|error| {
                                CodegenError(format!("query callback failure: {error}"))
                            })?
                            .try_as_basic_value()
                            .basic()
                            .expect("failure query returns a nullable managed reference");
                        self.temps.insert(out, value);
                    }
                    scoop_lir::ForeignCallbackOperation::State { out, .. } => {
                        let state = self.gc_leaf_fn(
                            "scoop_runtime_callback_state",
                            context.i32_type().fn_type(&[ptr_ty(context).into()], false),
                        );
                        let state = builder
                            .build_call(state, &[callback_context.into()], "callback_state")
                            .map_err(|error| {
                                CodegenError(format!("query callback state: {error}"))
                            })?
                            .try_as_basic_value()
                            .basic()
                            .expect("state query returns a tag")
                            .into_int_value();
                        let valid = builder
                            .build_int_compare(
                                IntPredicate::ULE,
                                state,
                                context.i32_type().const_int(3, false),
                                "callback_state_valid",
                            )
                            .map_err(|error| {
                                CodegenError(format!("validate callback state: {error}"))
                            })?;
                        let valid_block =
                            context.append_basic_block(self.llvm_function, "callback_state_valid");
                        let invalid_block = context
                            .append_basic_block(self.llvm_function, "callback_state_invalid");
                        builder
                            .build_conditional_branch(valid, valid_block, invalid_block)
                            .map_err(|error| {
                                CodegenError(format!("branch on callback state: {error}"))
                            })?;
                        builder.position_at_end(invalid_block);
                        let trap = self.llvm.get_function("llvm.trap").unwrap_or_else(|| {
                            self.llvm.add_function(
                                "llvm.trap",
                                context.void_type().fn_type(&[], false),
                                None,
                            )
                        });
                        builder
                            .build_call(trap, &[], "callback_state_trap")
                            .map_err(|error| {
                                CodegenError(format!("trap invalid callback state: {error}"))
                            })?;
                        builder.build_unreachable().map_err(|error| {
                            CodegenError(format!("trap invalid callback state: {error}"))
                        })?;
                        builder.position_at_end(valid_block);
                        let state_tag = |wire_code: u64,
                                         variant: scoop_lir::LirVariantRef,
                                         fallback: inkwell::values::IntValue<'ctx>|
                         -> Result<_, CodegenError> {
                            let matches = builder
                                .build_int_compare(
                                    IntPredicate::EQ,
                                    state,
                                    context.i32_type().const_int(wire_code, false),
                                    "callback_state_case",
                                )
                                .map_err(|error| {
                                    CodegenError(format!("decode callback state: {error}"))
                                })?;
                            builder
                                .build_select(
                                    matches,
                                    context
                                        .i64_type()
                                        .const_int(u64::from(variant.index()), false),
                                    fallback,
                                    "callback_state_tag",
                                )
                                .map(|value| value.into_int_value())
                                .map_err(|error| {
                                    CodegenError(format!("decode callback state: {error}"))
                                })
                        };
                        let states = family.states;
                        let state = state_tag(
                            2,
                            states.completed(),
                            context
                                .i64_type()
                                .const_int(u64::from(states.failed().index()), false),
                        )?;
                        let state = state_tag(1, states.active(), state)?;
                        let state = state_tag(0, states.registered(), state)?;
                        let enum_id = states.definition();
                        let EnumRepr::Tagged { size, align, .. } = &self.enums[enum_id].repr else {
                            unreachable!("validated callback state enum uses a tagged layout")
                        };
                        let ty = tagged_ty(
                            context,
                            self.managed_address_space,
                            *size,
                            *align,
                            &self.enums[enum_id].scan,
                        )?;
                        let slot = self.entry_alloca(ty.into(), "callback_state_value")?;
                        builder
                            .build_store(slot, ty.const_zero())
                            .map_err(|error| {
                                CodegenError(format!("zero callback state: {error}"))
                            })?;
                        let tag = self.tag_ptr(slot, ty)?;
                        builder.build_store(tag, state).map_err(|error| {
                            CodegenError(format!("construct callback state: {error}"))
                        })?;
                        let value = builder
                            .build_load(ty, slot, "callback_state_value")
                            .map_err(|error| {
                                CodegenError(format!("load callback state: {error}"))
                            })?;
                        self.temps.insert(out, value);
                    }
                }
            }
            _ => unreachable!("instruction dispatcher routes only callback instructions"),
        }
        Ok(())
    }
}
