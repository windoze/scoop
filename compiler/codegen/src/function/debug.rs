use super::*;
use scoop_lir::{AbiArgument, AbiArgumentLocation, AbiDirectValue, DebugStorage, LocalStorage};

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn initialize_debug_variables(&self) -> Result<(), CodegenError> {
        let Some(debug) = &self.debug else {
            return Ok(());
        };
        let mut debug = debug.borrow_mut();
        if !debug.is_full() {
            return Ok(());
        }
        let block = self.entry_block;
        for binding in &self.function.debug_variables {
            let managed = match binding.storage {
                DebugStorage::Local(id) => match self.function.locals[id].storage() {
                    LocalStorage::LogicalZst(_) | LocalStorage::AddressableZst(_) => false,
                    LocalStorage::NonZero(value) => value.scan().contains_reference(),
                },
                DebugStorage::Parameter(index) => self.function.signature.arguments()
                    [index as usize]
                    .scan()
                    .contains_reference(),
            };
            debug.register(binding, managed);
            match binding.storage {
                DebugStorage::Local(id) => {
                    if self.function.locals[id].storage().is_zst() {
                        debug.value(
                            binding.storage,
                            self.context.i8_type().const_zero().into(),
                            None,
                            block,
                        );
                    } else if !managed {
                        debug.declare(binding.storage, self.local_pointer(id)?, block);
                    }
                }
                DebugStorage::Parameter(index) => {
                    let argument = &self.function.signature.arguments()[index as usize];
                    let location = self
                        .function
                        .signature
                        .argument_location(index as usize)
                        .expect("the logical argument has its ABI location");
                    match (argument, location) {
                        (AbiArgument::ElidedZst(_), AbiArgumentLocation::Elided) => {
                            debug.value(
                                binding.storage,
                                self.context.i8_type().const_zero().into(),
                                None,
                                block,
                            );
                        }
                        (
                            AbiArgument::Direct(AbiDirectValue::DirectParts(parts)),
                            AbiArgumentLocation::Parts { first, .. },
                        ) => {
                            for (index, part) in parts.parts().iter().enumerate() {
                                let value = self
                                    .llvm_function
                                    .get_nth_param((first + index) as u32)
                                    .expect("ABI fragments refer to declared physical parameters");
                                debug.value(
                                    binding.storage,
                                    value,
                                    Some((part.byte_offset(), part.extent())),
                                    block,
                                );
                            }
                        }
                        (AbiArgument::Direct(_), AbiArgumentLocation::Parameter(index)) => {
                            let value = self
                                .llvm_function
                                .get_nth_param(index as u32)
                                .expect("the ABI parameter is declared");
                            debug.value(binding.storage, value, None, block);
                        }
                        (AbiArgument::Indirect(_), AbiArgumentLocation::Parameter(index)) => {
                            let pointer = self
                                .llvm_function
                                .get_nth_param(index as u32)
                                .expect("the indirect ABI parameter is declared")
                                .into_pointer_value();
                            if managed {
                                debug.address(binding.storage, pointer, block);
                            } else {
                                debug.declare(binding.storage, pointer, block);
                            }
                        }
                        _ => unreachable!(
                            "the validated signature pairs each argument with its location"
                        ),
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn observe_debug_value(&self, storage: DebugStorage, value: BasicValueEnum<'ctx>) {
        if let Some(debug) = &self.debug {
            let debug = debug.borrow();
            if debug.observes(storage) {
                debug.value(
                    storage,
                    value,
                    None,
                    self.builder
                        .get_insert_block()
                        .expect("an actual value use has an insertion block"),
                );
            }
        }
    }

    pub(super) fn observe_debug_copy(&self, local: scoop_lir::LocalId) -> Result<(), CodegenError> {
        if let Some(debug) = &self.debug {
            let debug = debug.borrow();
            let storage = DebugStorage::Local(local);
            if debug.observes(storage) {
                debug.address(
                    storage,
                    self.local_pointer(local)?,
                    self.builder
                        .get_insert_block()
                        .expect("an actual copy has an insertion block"),
                );
            }
        }
        Ok(())
    }
}
