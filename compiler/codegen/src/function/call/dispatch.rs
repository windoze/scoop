use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(in crate::function) fn emit_call_site(
        &mut self,
        site: &scoop_lir::CallSite,
    ) -> Result<(), CodegenError> {
        let targets = &self.function.call_targets;
        match site {
            scoop_lir::CallSite::Managed(site) => self.emit_typed_call(
                targets.typed_call_view(
                    &site.call,
                    &targets.managed_targets,
                    scoop_lir::ManagedCallDestination::view,
                ),
                CallProtocol::Managed {
                    safepoint: self.safepoint_id(site.safepoint),
                    live: &site.live,
                },
                None,
            ),
            scoop_lir::CallSite::NoGc(site) => self.emit_typed_call(
                targets.typed_call_view(
                    &site.call,
                    &targets.no_gc_targets,
                    scoop_lir::NoGcCallDestination::view,
                ),
                CallProtocol::NoGc,
                None,
            ),
            scoop_lir::CallSite::NativeSafe(site) => self.emit_typed_call(
                targets.typed_call_view(
                    &site.call,
                    &targets.native_safe_targets,
                    scoop_lir::NativeSafeCallDestination::view,
                ),
                CallProtocol::NativeSafe {
                    safepoint: self.safepoint_id(site.safepoint),
                    roots: &site.roots,
                },
                None,
            ),
            scoop_lir::CallSite::NativeBorrowed(site) => {
                let call = site.call.view(targets);
                self.emit_typed_call(
                    call.call,
                    CallProtocol::NativeBorrowed {
                        safepoint: self.safepoint_id(site.safepoint),
                        roots: &site.roots,
                        result: call.result,
                    },
                    None,
                )
            }
        }
    }

    pub(in crate::function) fn emit_invoke_site(
        &mut self,
        site: &scoop_lir::InvokeSite,
    ) -> Result<(), CodegenError> {
        let targets = &self.function.call_targets;
        match site {
            scoop_lir::InvokeSite::Managed(site) => self.emit_typed_call(
                targets.typed_call_view(
                    &site.call,
                    &targets.managed_targets,
                    scoop_lir::ManagedCallDestination::view,
                ),
                CallProtocol::ManagedInvoke {
                    safepoint: self.safepoint_id(site.safepoint),
                    roots: &site.roots,
                },
                Some((site.normal, site.unwind)),
            ),
            scoop_lir::InvokeSite::NoGc(site) => self.emit_typed_call(
                targets.typed_call_view(
                    &site.call,
                    &targets.no_gc_targets,
                    scoop_lir::NoGcCallDestination::view,
                ),
                CallProtocol::NoGc,
                Some((site.normal, site.unwind)),
            ),
        }
    }

    pub(in crate::function) fn typed_callee(
        &self,
        destination: scoop_lir::CallDestination,
        fn_ty: inkwell::types::FunctionType<'ctx>,
        signature: &scoop_lir::ScoopAbiSignature,
        apply_scoop_abi_attributes: bool,
    ) -> Result<inkwell::values::FunctionValue<'ctx>, CodegenError> {
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
                return Ok(function);
            }
            scoop_lir::CallDestination::Runtime(function) => function.symbol(),
            scoop_lir::CallDestination::CoreExternal(id) => {
                let declaration = &self.core_external_callables[id];
                let symbol = declaration.expected_symbol().symbol();
                let function = self.llvm.get_function(symbol.as_str()).ok_or_else(|| {
                    CodegenError(format!(
                        "typed core external target `{symbol}` was not declared in the module pass"
                    ))
                })?;
                if function.get_type() != fn_ty {
                    return Err(CodegenError(format!(
                        "typed target `{symbol}` disagrees with its existing declaration"
                    )));
                }
                return Ok(function);
            }
            scoop_lir::CallDestination::Extern(id) => match &self.extern_functions[id].kind {
                ExternFunctionKind::C { bridge, .. } => bridge.symbol(),
                ExternFunctionKind::Scoop { .. } => &self.extern_functions[id].native_symbol,
            },
            scoop_lir::CallDestination::Dispatch { .. } => {
                unreachable!("dispatch destinations have no direct callee")
            }
        };
        if apply_scoop_abi_attributes {
            return abi::declare_or_get(
                self.context,
                self.llvm,
                self.structs,
                self.enums,
                self.managed_address_space,
                symbol,
                signature,
            );
        }
        if let Some(function) = self.llvm.get_function(symbol) {
            if function.get_type() != fn_ty {
                return Err(CodegenError(format!(
                    "typed target `{symbol}` disagrees with its existing declaration"
                )));
            }
            Ok(function)
        } else {
            Ok(self.llvm.add_function(symbol, fn_ty, None))
        }
    }

    pub(in crate::function) fn dispatch_function_pointer(
        &self,
        table: Value,
        slot: scoop_lir::DispatchSlotId,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        let slot = *self
            .function
            .call_targets
            .dispatch_slots
            .get(slot)
            .ok_or_else(|| {
                CodegenError(format!(
                    "typed dispatch @{} refers to an invalid slot declaration",
                    self.function.symbol()
                ))
            })?;
        let table_ty = self.function.value_ty(self.globals_arena, table);
        let expected_table_ty = if slot.kind == scoop_lir::DispatchKind::Closure {
            scoop_lir::MANAGED_PTR
        } else {
            scoop_lir::METADATA_PTR
        };
        if table_ty != expected_table_ty {
            return Err(CodegenError(format!(
                "typed {:?} dispatch @{} requires table {}, got {}",
                slot.kind,
                self.function.symbol(),
                expected_table_ty.dump(),
                table_ty.dump()
            )));
        }
        let table = self.value(table)?.into_pointer_value();
        // SAFETY: the typed dispatch slot is assigned by lir-lower from the
        // complete vtable/itable/closure layout.
        let slot_pointer = unsafe {
            self.builder.build_gep(
                ptr_ty(self.context),
                table,
                &[self.context.i32_type().const_int(slot.index.into(), false)],
                "dispatch_slot",
            )
        }
        .map_err(|error| CodegenError(format!("typed dispatch slot: {error}")))?;
        self.builder
            .build_load(ptr_ty(self.context), slot_pointer, "dispatch_function")
            .map(BasicValueEnum::into_pointer_value)
            .map_err(|error| CodegenError(format!("typed dispatch load: {error}")))
    }
}
