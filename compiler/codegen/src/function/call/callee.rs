use super::*;

#[derive(Clone, Copy)]
pub(super) enum PreparedCallee<'ctx> {
    Direct(inkwell::values::FunctionValue<'ctx>),
    Dispatch {
        table: Value,
        slot: scoop_lir::DispatchSlotId,
    },
}

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn prepare_callee(
        &self,
        destination: scoop_lir::CallDestination,
        fn_ty: inkwell::types::FunctionType<'ctx>,
        signature: &scoop_lir::ScoopAbiSignature,
        apply_scoop_abi_attributes: bool,
    ) -> Result<PreparedCallee<'ctx>, CodegenError> {
        let symbol = match destination {
            scoop_lir::CallDestination::Local(id) => {
                let symbol = self
                    .functions
                    .get(id.into_u32() as usize)
                    .ok_or_else(|| {
                        CodegenError(format!("invalid local function id {}", id.into_u32()))
                    })?
                    .symbol();
                let function = self.llvm.get_function(symbol).ok_or_else(|| {
                    CodegenError(format!(
                        "typed local target `{symbol}` was not declared in the module pass"
                    ))
                })?;
                if function.get_type() != fn_ty {
                    return Err(CodegenError(format!(
                        "typed target `{symbol}` disagrees with its existing declaration"
                    )));
                }
                return Ok(PreparedCallee::Direct(function));
            }
            scoop_lir::CallDestination::Runtime(function) => function.symbol(),
            scoop_lir::CallDestination::External(id) => {
                let declaration = &self.external_callables[id];
                let symbol = declaration.expected_symbol().symbol();
                let function = self.llvm.get_function(symbol.as_str()).ok_or_else(|| {
                    CodegenError(format!(
                        "typed external target `{symbol}` was not declared in the module pass"
                    ))
                })?;
                if function.get_type() != fn_ty {
                    return Err(CodegenError(format!(
                        "typed target `{symbol}` disagrees with its existing declaration"
                    )));
                }
                return Ok(PreparedCallee::Direct(function));
            }
            scoop_lir::CallDestination::Extern(id) => match &self.extern_functions[id].kind {
                ExternFunctionKind::C {
                    call_plan: scoop_lir::CAbiCallPlan::StorageBridge { entry: bridge, .. },
                    ..
                } => bridge.symbol(),
                ExternFunctionKind::C {
                    call_plan: scoop_lir::CAbiCallPlan::Direct(_),
                    ..
                }
                | ExternFunctionKind::Scoop { .. } => &self.extern_functions[id].native_symbol,
            },
            scoop_lir::CallDestination::Dispatch { table, slot } => {
                return Ok(PreparedCallee::Dispatch { table, slot });
            }
        };
        if apply_scoop_abi_attributes {
            return abi::declare_or_get(
                self.context,
                self.llvm,
                self.structs,
                self.enums,
                self.profile,
                symbol,
                signature,
            )
            .map(PreparedCallee::Direct);
        }
        let function = if let Some(function) = self.llvm.get_function(symbol) {
            if function.get_type() != fn_ty {
                return Err(CodegenError(format!(
                    "typed target `{symbol}` disagrees with its existing declaration"
                )));
            }
            function
        } else {
            self.llvm.add_function(symbol, fn_ty, None)
        };
        self.apply_c_abi_attributes(destination, |location, attribute| {
            function.add_attribute(location, attribute)
        })?;
        Ok(PreparedCallee::Direct(function))
    }

    pub(super) fn prepared_callee_pointer(
        &self,
        callee: PreparedCallee<'ctx>,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        match callee {
            PreparedCallee::Direct(function) => Ok(function.as_global_value().as_pointer_value()),
            PreparedCallee::Dispatch { table, slot } => self.dispatch_function_pointer(table, slot),
        }
    }
}
