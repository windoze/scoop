use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_address_instruction(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let function = self.function;
        match instruction {
            Instruction::IntToPtr { out, value } => {
                let value = self.value(*value)?.into_int_value();
                let result = builder
                    .build_int_to_ptr(value, ptr_ty(context), "raw_ptr")
                    .map_err(|e| CodegenError(format!("inttoptr @{}: {e}", function.symbol)))?;
                self.temps.insert(*out, result.into());
            }
            Instruction::PtrToInt { out, value } => {
                let value = self.value(*value)?.into_pointer_value();
                if value.get_type().get_address_space() == self.managed_address_space.inkwell() {
                    return Err(CodegenError(format!(
                        "managed pointer cannot be lowered by PtrToInt in @{}",
                        function.symbol
                    )));
                }
                let result = builder
                    .build_ptr_to_int(value, context.i64_type(), "raw_uint")
                    .map_err(|e| CodegenError(format!("ptrtoint @{}: {e}", function.symbol)))?;
                self.temps.insert(*out, result.into());
            }
            Instruction::RawLoad {
                out,
                pointer,
                align,
            } => {
                let pointer = self.value(*pointer)?.into_pointer_value();
                let ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &function.temps[*out].ty,
                )?;
                let value = builder
                    .build_load(ty, pointer, "raw_load")
                    .map_err(|e| CodegenError(format!("raw load @{}: {e}", function.symbol)))?;
                value
                    .as_instruction_value()
                    .expect("a non-constant load is an instruction")
                    .set_alignment(*align as u32)
                    .map_err(|e| {
                        CodegenError(format!("raw load alignment @{}: {e}", function.symbol))
                    })?;
                self.temps.insert(*out, value);
            }
            Instruction::RawStore {
                pointer,
                value,
                align,
            } => {
                let pointer = self.value(*pointer)?.into_pointer_value();
                let store = builder
                    .build_store(pointer, self.value(*value)?)
                    .map_err(|e| CodegenError(format!("raw store @{}: {e}", function.symbol)))?;
                store.set_alignment(*align as u32).map_err(|e| {
                    CodegenError(format!("raw store alignment @{}: {e}", function.symbol))
                })?;
            }
            Instruction::PtrOffset {
                out,
                pointer,
                bytes,
            } => {
                let pointer = self.value(*pointer)?.into_pointer_value();
                let bytes = self.value(*bytes)?.into_int_value();
                // SAFETY: source semantics make raw pointer arithmetic unsafe;
                // validity of the resulting address remains the caller's duty.
                let result = unsafe {
                    builder.build_gep(context.i8_type(), pointer, &[bytes], "raw_offset")
                }
                .map_err(|e| CodegenError(format!("raw gep @{}: {e}", function.symbol)))?;
                self.temps.insert(*out, result.into());
            }
            Instruction::LocalAddress { out, local } => {
                self.temps
                    .insert(*out, self.allocas[arena_index(*local)].into());
            }
            Instruction::GlobalLoad { out, global } => {
                let llvm_global =
                    self.globals[arena_index(*global)].expect("storage globals are emitted");
                let ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &function.temps[*out].ty,
                )?;
                let value = builder
                    .build_load(ty, llvm_global.as_pointer_value(), "global_load")
                    .map_err(|error| CodegenError(format!("global load: {error}")))?;
                self.temps.insert(*out, value);
            }
            Instruction::GlobalStore { global, value } => {
                let llvm_global =
                    self.globals[arena_index(*global)].expect("storage globals are emitted");
                builder
                    .build_store(llvm_global.as_pointer_value(), self.value(*value)?)
                    .map_err(|error| CodegenError(format!("global store: {error}")))?;
            }
            Instruction::GlobalAddress { out, global } => {
                let llvm_global =
                    self.globals[arena_index(*global)].expect("storage globals are emitted");
                self.temps
                    .insert(*out, llvm_global.as_pointer_value().into());
            }
            Instruction::NativeGlobalLoad {
                out,
                global,
                safepoint,
                roots,
            } => {
                let native = &self.native_globals[*global];
                let ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &native.ty,
                )?;
                let slot = self.entry_alloca(ty, "native_global_result")?;
                let symbol = &self.native_global_bridges.gets[native.access.get()].symbol;
                let callee = self.native_global_bridge(symbol);
                let transition = self.publish_native_roots(
                    roots.as_slice(),
                    None,
                    NativeTransitionKind::Safe,
                    *safepoint,
                )?;
                self.emit_native_global_call(callee, slot)?;
                self.finish_native_transition(transition, NativeTransitionKind::Safe)?;
                let value = builder
                    .build_load(ty, slot, "native_global_value")
                    .map_err(|error| CodegenError(format!("native global load: {error}")))?;
                self.temps.insert(*out, value);
            }
            Instruction::NativeGlobalStore {
                global,
                value,
                safepoint,
                roots,
            } => {
                let native = &self.native_globals[*global];
                let ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &native.ty,
                )?;
                let slot = self.entry_alloca(ty, "native_global_argument")?;
                builder
                    .build_store(slot, self.value(*value)?)
                    .map_err(|error| CodegenError(format!("native global spill: {error}")))?;
                let scoop_lir::NativeGlobalAccess::Mutable { set, .. } = native.access else {
                    return Err(CodegenError(format!(
                        "native global store targets readonly `{}`",
                        native.source_name
                    )));
                };
                let symbol = &self.native_global_bridges.sets[set].symbol;
                let callee = self.native_global_bridge(symbol);
                let transition = self.publish_native_roots(
                    roots.as_slice(),
                    None,
                    NativeTransitionKind::Safe,
                    *safepoint,
                )?;
                self.emit_native_global_call(callee, slot)?;
                self.finish_native_transition(transition, NativeTransitionKind::Safe)?;
            }
            Instruction::NativeGlobalAddress {
                out,
                global,
                safepoint,
                roots,
            } => {
                let native = &self.native_globals[*global];
                let ty: BasicTypeEnum = ptr_ty(context).into();
                let slot = self.entry_alloca(ty, "native_global_address")?;
                let symbol = &self.native_global_bridges.addresses[native.access.address()].symbol;
                let callee = self.native_global_bridge(symbol);
                let transition = self.publish_native_roots(
                    roots.as_slice(),
                    None,
                    NativeTransitionKind::Safe,
                    *safepoint,
                )?;
                self.emit_native_global_call(callee, slot)?;
                self.finish_native_transition(transition, NativeTransitionKind::Safe)?;
                let value = builder
                    .build_load(ty, slot, "native_global_pointer")
                    .map_err(|error| CodegenError(format!("native global pointer: {error}")))?;
                self.temps.insert(*out, value);
            }
            Instruction::Store { local, value: v } => {
                let operand = self.value(*v)?;
                builder
                    .build_store(self.allocas[arena_index(*local)], operand)
                    .map_err(|e| {
                        CodegenError(format!("store %{}: {e}", function.locals[*local].name))
                    })?;
            }
            _ => {
                unreachable!("instruction dispatcher routes only address and storage instructions")
            }
        }
        Ok(())
    }
}
