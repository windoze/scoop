use super::*;

pub(super) enum LocalAllocation<'ctx> {
    LogicalZst,
    ZstToken(PointerValue<'ctx>),
    NonZero(PointerValue<'ctx>),
}

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn allocate_locals(&mut self) -> Result<(), CodegenError> {
        for (_, local) in self.function.locals.iter() {
            let (ty, alignment, token) = match local.storage() {
                scoop_lir::LocalStorage::LogicalZst(_) => {
                    self.allocas.push(LocalAllocation::LogicalZst);
                    continue;
                }
                scoop_lir::LocalStorage::AddressableZst(place) => {
                    let scoop_lir::LocalPlaceLifetime::FunctionActivation = place.lifetime();
                    (
                        self.context.i8_type().into(),
                        place.value().representation().layout().alignment().get(),
                        true,
                    )
                }
                scoop_lir::LocalStorage::NonZero(value) => (
                    basic_ty(
                        self.context,
                        self.structs,
                        self.enums,
                        self.managed_address_space,
                        value.storage_type(),
                    )?,
                    value.layout().alignment().get(),
                    false,
                ),
            };
            let alignment = u32::try_from(alignment).map_err(|_| {
                CodegenError(format!(
                    "local %{} alignment exceeds LLVM limits",
                    local.name
                ))
            })?;
            let pointer = self
                .builder
                .build_alloca(ty, &local.name)
                .map_err(|error| CodegenError(format!("alloca %{}: {error}", local.name)))?;
            pointer
                .as_instruction_value()
                .expect("alloca is an instruction")
                .set_alignment(alignment)
                .map_err(|error| CodegenError(format!("align local %{}: {error}", local.name)))?;
            self.allocas.push(if token {
                LocalAllocation::ZstToken(pointer)
            } else {
                LocalAllocation::NonZero(pointer)
            });
        }
        Ok(())
    }

    pub(super) fn local_pointer(
        &self,
        local: scoop_lir::LocalId,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        match self.allocas[arena_index(local)] {
            LocalAllocation::ZstToken(pointer) | LocalAllocation::NonZero(pointer) => Ok(pointer),
            LocalAllocation::LogicalZst => Err(CodegenError(format!(
                "function @{} cannot take the address of logical ZST local %{} without a place token",
                self.function.symbol(),
                self.function.locals[local].name,
            ))),
        }
    }
}
