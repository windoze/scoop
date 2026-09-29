use super::*;
use crate::dataflow::{LiveValue, instruction_defs, instruction_uses, terminator_uses};

fn local_allocation_order(function: &Function) -> Vec<scoop_lir::LocalId> {
    let mut order = Vec::with_capacity(function.locals.len());
    let mut seen = HashSet::with_capacity(function.locals.len());
    let mut visit = |value| {
        if let LiveValue::Local(local) = value
            && seen.insert(local)
        {
            order.push(local);
        }
    };
    for (_, block) in function.blocks.iter() {
        for instruction in &block.instructions {
            for value in instruction_uses(instruction, function)
                .into_iter()
                .filter_map(LiveValue::from_value)
                .chain(instruction_defs(instruction))
            {
                visit(value);
            }
        }
        terminator_uses(&block.terminator, |value| {
            if let Some(value) = LiveValue::from_value(value) {
                visit(value);
            }
        });
    }
    for (local, _) in function.locals.iter() {
        visit(LiveValue::Local(local));
    }
    order
}

pub(super) enum LocalAllocation<'ctx> {
    LogicalZst,
    ZstToken(PointerValue<'ctx>),
    NonZero(PointerValue<'ctx>),
}

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn allocate_locals(&mut self) -> Result<(), CodegenError> {
        // Arena allocation can differ when a template is imported. Preserve
        // the body's use/def order so equivalent bodies get the same slots.
        for id in local_allocation_order(self.function) {
            let local = &self.function.locals[id];
            let (ty, alignment, token) = match local.storage() {
                scoop_lir::LocalStorage::LogicalZst(_) => {
                    self.allocas.insert(id, LocalAllocation::LogicalZst);
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
            self.allocas.insert(
                id,
                if token {
                    LocalAllocation::ZstToken(pointer)
                } else {
                    LocalAllocation::NonZero(pointer)
                },
            );
        }
        Ok(())
    }

    pub(super) fn local_pointer(
        &self,
        local: scoop_lir::LocalId,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        match self.allocas[&local] {
            LocalAllocation::ZstToken(pointer) | LocalAllocation::NonZero(pointer) => Ok(pointer),
            LocalAllocation::LogicalZst => Err(CodegenError(format!(
                "function @{} cannot take the address of logical ZST local %{} without a place token",
                self.function.symbol(),
                self.function.locals[local].name,
            ))),
        }
    }
}
