use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::function) fn emit_typed_invoke(
        &mut self,
        destination: scoop_lir::CallDestination,
        fn_type: inkwell::types::FunctionType<'ctx>,
        call_args: &[BasicValueEnum<'ctx>],
        result: &TypedCallResult<'_>,
        signature: &scoop_lir::ScoopAbiSignature,
        apply_scoop_abi_attributes: bool,
        protocol: &CallProtocol<'_>,
        callee: Option<inkwell::values::FunctionValue<'ctx>>,
        normal: scoop_lir::BlockId,
        unwind: scoop_lir::BlockId,
    ) -> Result<(), CodegenError> {
        let roots = match protocol {
            CallProtocol::ManagedInvoke { roots, .. } => roots.as_slice(),
            CallProtocol::NoGc => &[],
            CallProtocol::Managed { .. }
            | CallProtocol::ReleaseNativeLeaf
            | CallProtocol::NativeSafe { .. }
            | CallProtocol::NativeBorrowed { .. } => {
                return Err(CodegenError(format!(
                    "non-invoke protocol reached invoke emission in @{}",
                    self.function.symbol()
                )));
            }
        };
        let frame = self.publish_compiler_roots(roots)?;
        let cleanup = self.context.append_basic_block(
            self.llvm_function,
            &format!("invoke.normal.cleanup.{}", self.compiler_invoke_index - 1),
        );
        let actual_callee = match destination {
            scoop_lir::CallDestination::Dispatch { table, slot } => {
                self.dispatch_function_pointer(table, slot)?
            }
            _ => callee
                .expect("direct destination has a typed callee")
                .as_global_value()
                .as_pointer_value(),
        };
        let result_type = match result {
            TypedCallResult::Direct { value, .. } => Some(basic_ty(
                self.context,
                self.structs,
                self.enums,
                self.managed_address_space,
                value.storage_type(),
            )?),
            TypedCallResult::Void
            | TypedCallResult::ElidedZst { .. }
            | TypedCallResult::Indirect { .. } => None,
        };
        let direct_value = match protocol {
            CallProtocol::ManagedInvoke { safepoint, .. } => statepoint::build_managed_invoke(
                self.context,
                self.llvm,
                self.builder,
                statepoint::ManagedInvoke {
                    callee: actual_callee,
                    callee_type: fn_type,
                    call_args,
                    safepoint: *safepoint,
                    normal: cleanup,
                    unwind: self.llvm_blocks[arena_index(unwind)],
                    result_type,
                    abi_signature: apply_scoop_abi_attributes.then_some(signature),
                    structs: self.structs,
                    enums: self.enums,
                    managed_address_space: self.managed_address_space,
                },
            )?,
            CallProtocol::NoGc => {
                let call = match destination {
                    scoop_lir::CallDestination::Dispatch { .. } => self
                        .builder
                        .build_indirect_invoke(
                            fn_type,
                            actual_callee,
                            call_args,
                            cleanup,
                            self.llvm_blocks[arena_index(unwind)],
                            "typed_invoke",
                        )
                        .map_err(|error| CodegenError(format!("typed dispatch invoke: {error}")))?,
                    _ => self
                        .builder
                        .build_invoke(
                            callee.expect("direct destination has a typed callee"),
                            call_args,
                            cleanup,
                            self.llvm_blocks[arena_index(unwind)],
                            "typed_invoke",
                        )
                        .map_err(|error| CodegenError(format!("typed direct invoke: {error}")))?,
                };
                // Preserve each protected call through machine tail merging.
                call.add_attribute(
                    AttributeLoc::Function,
                    self.context
                        .create_enum_attribute(Attribute::get_named_enum_kind_id("nomerge"), 0),
                );
                if apply_scoop_abi_attributes {
                    abi::apply_call_attributes(
                        self.context,
                        self.structs,
                        self.enums,
                        self.managed_address_space,
                        call,
                        signature,
                    )?;
                }
                self.apply_call_protocol(call, destination, protocol);
                self.builder.position_at_end(cleanup);
                match result {
                    TypedCallResult::Direct { .. } => call.try_as_basic_value().basic(),
                    TypedCallResult::Void
                    | TypedCallResult::ElidedZst { .. }
                    | TypedCallResult::Indirect { .. } => None,
                }
            }
            _ => unreachable!("protocol was checked above"),
        };

        let normal_sources = roots
            .iter()
            .filter(|root| root.normal_live)
            .map(|root| root.root.source)
            .collect::<Vec<_>>();
        self.validate_compiler_root_sources(normal_sources.iter().copied())?;
        let reloaded_roots = self.reload_published_roots(normal_sources)?;
        self.pop_compiler_roots(frame)?;
        self.restore_reloaded_roots(reloaded_roots)?;
        match result {
            TypedCallResult::ElidedZst { out, value } => {
                let ty = basic_ty(
                    self.context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    value.storage_type(),
                )?;
                self.temps.insert(*out, ty.const_zero());
            }
            TypedCallResult::Direct { out, .. } => {
                let value = direct_value.ok_or_else(|| {
                    CodegenError(format!(
                        "typed invoke @{} produced no direct result",
                        self.function.symbol()
                    ))
                })?;
                self.temps.insert(*out, value);
                self.sync_root_temp(*out)?;
            }
            TypedCallResult::Void | TypedCallResult::Indirect { .. } => {}
        }
        self.builder
            .build_unconditional_branch(self.llvm_blocks[arena_index(normal)])
            .map_err(|error| CodegenError(format!("leave invoke cleanup: {error}")))?;
        Ok(())
    }
}
