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
                    function.locals[id].ty(),
                )?;
                if function.locals[id].storage().is_zst() {
                    ty.const_zero()
                } else {
                    self.builder
                        .build_load(ty, self.local_pointer(id)?, &function.locals[id].name)
                        .map_err(|e| {
                            CodegenError(format!("load %{}: {e}", function.locals[id].name))
                        })?
                }
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
                    let logical_index = index as usize;
                    let argument = self
                        .function
                        .signature
                        .arguments()
                        .get(logical_index)
                        .ok_or_else(|| CodegenError(format!("param {index} out of range")))?;
                    let location = self
                        .function
                        .signature
                        .argument_location(logical_index)
                        .expect("an existing ABI argument has a location");
                    match (argument, location) {
                        (
                            scoop_lir::AbiArgument::ElidedZst(value),
                            scoop_lir::AbiArgumentLocation::Elided,
                        ) => basic_ty(
                            context,
                            self.structs,
                            self.enums,
                            self.managed_address_space,
                            value.storage_type(),
                        )?
                        .const_zero(),
                        (
                            scoop_lir::AbiArgument::Direct(_),
                            scoop_lir::AbiArgumentLocation::Parameter(physical_index),
                        ) => {
                            let physical_index = u32::try_from(physical_index).map_err(|_| {
                                CodegenError(format!(
                                    "physical parameter for logical param {index} exceeds u32::MAX"
                                ))
                            })?;
                            self.llvm_function
                                .get_nth_param(physical_index)
                                .ok_or_else(|| {
                                    CodegenError(format!(
                                        "physical parameter for logical param {index} is out of range"
                                    ))
                                })?
                        }
                        (
                            scoop_lir::AbiArgument::Indirect(value),
                            scoop_lir::AbiArgumentLocation::Parameter(physical_index),
                        ) => {
                            let physical_index = u32::try_from(physical_index).map_err(|_| {
                                CodegenError(format!(
                                    "physical parameter for logical param {index} exceeds u32::MAX"
                                ))
                            })?;
                            let storage = self
                                .llvm_function
                                .get_nth_param(physical_index)
                                .ok_or_else(|| {
                                    CodegenError(format!(
                                        "physical parameter for logical param {index} is out of range"
                                    ))
                                })?;
                            let BasicValueEnum::PointerValue(storage) = storage else {
                                return Err(CodegenError(format!(
                                    "indirect logical param {index} does not use pointer storage"
                                )));
                            };
                            let ty = basic_ty(
                                context,
                                self.structs,
                                self.enums,
                                self.managed_address_space,
                                value.storage_type(),
                            )?;
                            self.builder
                                .build_load(ty, storage, "indirect_param")
                                .map_err(|error| {
                                    CodegenError(format!(
                                        "load indirect logical param {index}: {error}"
                                    ))
                                })?
                        }
                        _ => {
                            return Err(CodegenError(format!(
                                "logical param {index} has an inconsistent Scoop ABI location"
                            )));
                        }
                    }
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
            Value::IntegerConst(value) => integer_ty(context, value.kind().width())
                .const_int(value.raw_bits(), false)
                .into(),
            Value::MachineScalar(value) => {
                context.i64_type().const_int(value.raw_bits(), false).into()
            }
            Value::BoolConst(value) => context.bool_type().const_int(value as u64, false).into(),
            Value::NullPointer(kind) => pointer_ty(context, self.managed_address_space, kind)
                .const_null()
                .into(),
            Value::TypeDescriptor(reference) => type_descriptor_global(
                reference,
                TypeDescriptorGlobals {
                    local: self.type_tds,
                    external: self.external_type_tds,
                },
            )?
            .as_pointer_value()
            .into(),
            Value::RootScan(id) => self.root_scans[arena_index(id)].into(),
            Value::ContextKeyCell(key) => self.runtime_scans.context_cell(key)?.into(),
            Value::Global(id) => self.globals[arena_index(id)]
                .expect("ordinary globals are emitted")
                .as_pointer_value()
                .into(),
            Value::InitializationUnit(id) => self.initialization_units[arena_index(id)]
                .as_pointer_value()
                .into(),
            Value::CArgumentStorage(_) => {
                return Err(CodegenError(
                    "C argument storage address escaped its native-safe call operand".to_string(),
                ));
            }
        })
    }

    /// Materialize a typed call operand. Exact C argument storage is an
    /// address-only operand and therefore bypasses the ordinary implicit
    /// local load; every other operand follows the regular/statepoint path.
    pub(in crate::function) fn typed_call_argument_value(
        &self,
        value: Value,
        live: Option<&MaterializedStatepointLive<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        match value {
            Value::CArgumentStorage(storage) => Ok(self.local_pointer(storage.local())?.into()),
            _ => match live {
                Some(live) => self.statepoint_value(value, live),
                None => self.value(value),
            },
        }
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
            Value::IntegerConst(_)
            | Value::MachineScalar(_)
            | Value::BoolConst(_)
            | Value::NullPointer(_)
            | Value::TypeDescriptor(_)
            | Value::RootScan(_)
            | Value::ContextKeyCell(_)
            | Value::Global(_)
            | Value::InitializationUnit(_)
            | Value::CArgumentStorage(_) => None,
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
