use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    /// Materialize an operand as an LLVM value: locals are loaded from
    /// their stack slot, temps are SSA values, globals are addressed by
    /// pointer.
    pub(in crate::function) fn value(
        &self,
        value: Value,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let context = self.context;
        let function = self.function;
        Ok(match value {
            Value::Local(id) => {
                let ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &function.locals[id].ty,
                )?;
                self.builder
                    .build_load(ty, self.allocas[arena_index(id)], &function.locals[id].name)
                    .map_err(|e| CodegenError(format!("load %{}: {e}", function.locals[id].name)))?
            }
            Value::Param(index) => {
                let source = scoop_lir::CallerRootSource::Param(index);
                if let Some(storage) = self.root_storage.get(&source) {
                    self.builder
                        .build_load(storage.ty, storage.pointer, "root_param")
                        .map_err(|error| {
                            CodegenError(format!("load rooted param {index}: {error}"))
                        })?
                } else {
                    self.llvm_function
                        .get_nth_param(index + self.param_offset)
                        .ok_or_else(|| CodegenError(format!("param {index} out of range")))?
                }
            }
            Value::Temp(id) => {
                let source = scoop_lir::CallerRootSource::Temp(id);
                if let Some(storage) = self.root_storage.get(&source) {
                    self.builder
                        .build_load(storage.ty, storage.pointer, "root_temp")
                        .map_err(|error| {
                            CodegenError(format!(
                                "load rooted temp t{}: {error}",
                                id.into_raw().into_u32()
                            ))
                        })?
                } else {
                    *self.temps.get(&id).ok_or_else(|| {
                        CodegenError(format!(
                            "temp t{} used before definition",
                            id.into_raw().into_u32()
                        ))
                    })?
                }
            }
            Value::IntConst(value) => context.i64_type().const_int(value as u64, true).into(),
            Value::BoolConst(value) => context.bool_type().const_int(value as u64, false).into(),
            Value::NullPointer(kind) => pointer_ty(context, self.managed_address_space, kind)
                .const_null()
                .into(),
            Value::TypeDescriptor(reference) => {
                type_descriptor_global(reference, self.type_tds, self.external_type_tds)?
                    .as_pointer_value()
                    .into()
            }
            Value::RootScan(id) => self.root_scans[arena_index(id)].into(),
            Value::Global(id) => self.globals[arena_index(id)]
                .expect("ordinary globals are emitted")
                .as_pointer_value()
                .into(),
        })
    }

    pub(in crate::function) fn statepoint_value(
        &self,
        value: Value,
        live: &MaterializedStatepointLive<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let source = match value {
            Value::Param(index) => Some(scoop_lir::CallerRootSource::Param(index)),
            Value::Local(id) => Some(scoop_lir::CallerRootSource::Local(id)),
            Value::Temp(id) => Some(scoop_lir::CallerRootSource::Temp(id)),
            Value::IntConst(_)
            | Value::BoolConst(_)
            | Value::NullPointer(_)
            | Value::TypeDescriptor(_)
            | Value::RootScan(_)
            | Value::Global(_) => None,
        };
        source
            .and_then(|source| live.arguments.get(&source).copied())
            .map_or_else(|| self.value(value), Ok)
    }

    pub(in crate::function) fn sync_root_temp(&self, temp: TempId) -> Result<(), CodegenError> {
        let source = scoop_lir::CallerRootSource::Temp(temp);
        let Some(storage) = self.root_storage.get(&source) else {
            return Ok(());
        };
        let value = self.temps.get(&temp).ok_or_else(|| {
            CodegenError(format!(
                "rooted temp t{} has no emitted definition",
                temp.into_raw().into_u32()
            ))
        })?;
        self.builder
            .build_store(storage.pointer, *value)
            .map_err(|error| {
                CodegenError(format!(
                    "store rooted temp t{}: {error}",
                    temp.into_raw().into_u32()
                ))
            })?;
        Ok(())
    }
}
