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
            Instruction::ULongToPtr { out, value } => {
                let input_ty = function.value_ty(self.globals_arena, *value);
                let output_ty = &function.temps[*out].ty;
                if input_ty != LirType::I64 || *output_ty != scoop_lir::RAW_PTR {
                    return Err(CodegenError(format!(
                        "ulong_to_ptr @{} requires ULong/i64 -> ptr<raw>, got {} -> {}",
                        function.symbol(),
                        input_ty.dump(),
                        output_ty.dump()
                    )));
                }
                let value = self.value(*value)?.into_int_value();
                let result = builder
                    .build_int_to_ptr(value, ptr_ty(context), "raw_ptr")
                    .map_err(|e| CodegenError(format!("inttoptr @{}: {e}", function.symbol())))?;
                self.temps.insert(*out, result.into());
            }
            Instruction::PtrToULong { out, value } => {
                let input_ty = function.value_ty(self.globals_arena, *value);
                let output_ty = &function.temps[*out].ty;
                if input_ty != scoop_lir::RAW_PTR || *output_ty != LirType::I64 {
                    return Err(CodegenError(format!(
                        "ptr_to_ulong @{} requires ptr<raw> -> ULong/i64, got {} -> {}",
                        function.symbol(),
                        input_ty.dump(),
                        output_ty.dump()
                    )));
                }
                let value = self.value(*value)?.into_pointer_value();
                if value.get_type().get_address_space() == self.managed_address_space.inkwell() {
                    return Err(CodegenError(format!(
                        "managed pointer cannot be lowered by PtrToULong in @{}",
                        function.symbol()
                    )));
                }
                let result = builder
                    .build_ptr_to_int(value, context.i64_type(), "raw_uint")
                    .map_err(|e| CodegenError(format!("ptrtoint @{}: {e}", function.symbol())))?;
                self.temps.insert(*out, result.into());
            }
            Instruction::LocalAddress { out, local } => {
                let local_ty = function.locals[*local].ty();
                if function.temps[*out].ty != scoop_lir::RAW_PTR
                    || validation::contains_machine_scalar(self.structs, self.enums, local_ty)
                {
                    return Err(CodegenError(format!(
                        "local_address @{} cannot expose {} as a raw pointer",
                        function.symbol(),
                        local_ty.dump()
                    )));
                }
                self.temps.insert(*out, self.local_pointer(*local)?.into());
            }
            Instruction::GlobalLoad { out, global } => {
                let (GlobalInit::Storage { ty: storage_ty, .. }
                | GlobalInit::RawStorage { ty: storage_ty, .. }
                | GlobalInit::ImportedStorage { ty: storage_ty, .. }) =
                    &self.globals_arena[*global].init
                else {
                    return Err(CodegenError(format!(
                        "global load @{} targets non-storage global `{}`",
                        function.symbol(),
                        self.globals_arena[*global].symbol()
                    )));
                };
                let out_ty = &function.temps[*out].ty;
                if out_ty != storage_ty {
                    return Err(CodegenError(format!(
                        "global load @{} has result type {}, but `{}` stores {}",
                        function.symbol(),
                        out_ty.dump(),
                        self.globals_arena[*global].symbol(),
                        storage_ty.dump()
                    )));
                }
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
                let (GlobalInit::Storage { ty: storage_ty, .. }
                | GlobalInit::RawStorage { ty: storage_ty, .. }) =
                    &self.globals_arena[*global].init
                else {
                    return Err(CodegenError(format!(
                        "global store @{} targets non-storage global `{}`",
                        function.symbol(),
                        self.globals_arena[*global].symbol()
                    )));
                };
                let value_ty = function.value_ty(self.globals_arena, *value);
                if &value_ty != storage_ty {
                    return Err(CodegenError(format!(
                        "global store @{} has value type {}, but `{}` stores {}",
                        function.symbol(),
                        value_ty.dump(),
                        self.globals_arena[*global].symbol(),
                        storage_ty.dump()
                    )));
                }
                let llvm_global =
                    self.globals[arena_index(*global)].expect("storage globals are emitted");
                builder
                    .build_store(llvm_global.as_pointer_value(), self.value(*value)?)
                    .map_err(|error| CodegenError(format!("global store: {error}")))?;
            }
            Instruction::GlobalAddress { out, global } => {
                let global_def = &self.globals_arena[*global];
                let (GlobalInit::Storage { ty, .. }
                | GlobalInit::RawStorage { ty, .. }
                | GlobalInit::ImportedStorage { ty, .. }) = &global_def.init
                else {
                    return Err(CodegenError(format!(
                        "global_address @{} targets non-storage global `{}`",
                        function.symbol(),
                        global_def.symbol()
                    )));
                };
                if function.temps[*out].ty != scoop_lir::RAW_PTR
                    || global_def.address_kind != PointerKind::Raw
                    || validation::contains_machine_scalar(self.structs, self.enums, ty)
                {
                    return Err(CodegenError(format!(
                        "global_address @{} cannot expose global `{}` of type {} as a raw pointer",
                        function.symbol(),
                        global_def.symbol(),
                        ty.dump()
                    )));
                }
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
                let storage_type = native.storage_type();
                if function.temps[*out].ty != storage_type {
                    return Err(CodegenError(format!(
                        "native global load @{} has result type {}, but `{}` stores {}",
                        function.symbol(),
                        function.temps[*out].ty.dump(),
                        native.source_name,
                        storage_type.dump()
                    )));
                }
                let ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &storage_type,
                )?;
                let slot = self.entry_alloca(ty, "native_global_result")?;
                let symbol = self.native_global_bridges.gets[native.access.get()]
                    .identity
                    .symbol();
                let callee = self.native_global_bridge(symbol);
                let transition = self.publish_native_roots(
                    roots.as_slice(),
                    None,
                    NativeTransitionKind::Safe,
                    self.safepoint_id(*safepoint),
                )?;
                self.emit_native_global_call(callee, slot)?;
                self.finish_native_transition(
                    transition,
                    NativeTransitionKind::Safe,
                    roots.as_slice(),
                )?;
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
                let storage_type = native.storage_type();
                let value_ty = function.value_ty(self.globals_arena, *value);
                if value_ty != storage_type {
                    return Err(CodegenError(format!(
                        "native global store @{} has value type {}, but `{}` stores {}",
                        function.symbol(),
                        value_ty.dump(),
                        native.source_name,
                        storage_type.dump()
                    )));
                }
                let ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &storage_type,
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
                let symbol = self.native_global_bridges.sets[set].identity.symbol();
                let callee = self.native_global_bridge(symbol);
                let transition = self.publish_native_roots(
                    roots.as_slice(),
                    None,
                    NativeTransitionKind::Safe,
                    self.safepoint_id(*safepoint),
                )?;
                self.emit_native_global_call(callee, slot)?;
                self.finish_native_transition(
                    transition,
                    NativeTransitionKind::Safe,
                    roots.as_slice(),
                )?;
            }
            Instruction::NativeGlobalAddress {
                out,
                global,
                safepoint,
                roots,
            } => {
                let native = &self.native_globals[*global];
                let storage_type = native.storage_type();
                if function.temps[*out].ty != scoop_lir::RAW_PTR
                    || validation::contains_machine_scalar(self.structs, self.enums, &storage_type)
                {
                    return Err(CodegenError(format!(
                        "native_global_address @{} cannot expose `{}` of type {}",
                        function.symbol(),
                        native.source_name,
                        storage_type.dump()
                    )));
                }
                let ty: BasicTypeEnum = ptr_ty(context).into();
                let slot = self.entry_alloca(ty, "native_global_address")?;
                let symbol = self.native_global_bridges.addresses[native.access.address()]
                    .identity
                    .symbol();
                let callee = self.native_global_bridge(symbol);
                let transition = self.publish_native_roots(
                    roots.as_slice(),
                    None,
                    NativeTransitionKind::Safe,
                    self.safepoint_id(*safepoint),
                )?;
                self.emit_native_global_call(callee, slot)?;
                self.finish_native_transition(
                    transition,
                    NativeTransitionKind::Safe,
                    roots.as_slice(),
                )?;
                let value = builder
                    .build_load(ty, slot, "native_global_pointer")
                    .map_err(|error| CodegenError(format!("native global pointer: {error}")))?;
                self.temps.insert(*out, value);
            }
            Instruction::Store { local, value: v } => {
                let value_ty = function.value_ty(self.globals_arena, *v);
                if &value_ty != function.locals[*local].ty() {
                    return Err(CodegenError(format!(
                        "store @{} has value type {}, but %{} stores {}",
                        function.symbol(),
                        value_ty.dump(),
                        function.locals[*local].name,
                        function.locals[*local].ty().dump()
                    )));
                }
                let operand = self.value(*v)?;
                if function.locals[*local].storage().is_zst() {
                    return Ok(());
                }
                builder
                    .build_store(self.local_pointer(*local)?, operand)
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
