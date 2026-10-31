//! Emit a data-only partition with LLVM's normal target assembler.

use crate::CodegenError;
use inkwell::memory_buffer::MemoryBuffer;
use inkwell::module::Module;
use inkwell::targets::TargetMachine;
use llvm_sys::prelude::{LLVMMemoryBufferRef, LLVMModuleRef};
use llvm_sys::target_machine::LLVMTargetMachineRef;
use std::path::Path;

unsafe extern "C" {
    fn scoop_emit_metadata_object(
        target: LLVMTargetMachineRef,
        module: LLVMModuleRef,
    ) -> LLVMMemoryBufferRef;
}

pub(super) fn write(
    machine: &TargetMachine,
    module: &Module<'_>,
    path: &Path,
) -> Result<(), CodegenError> {
    // SAFETY: both LLVM values remain live for this synchronous call. Only the
    // non-callable partition enters here; machine functions use the full pipeline.
    let buffer = unsafe { scoop_emit_metadata_object(machine.as_mut_ptr(), module.as_mut_ptr()) };
    if buffer.is_null() {
        return Err(CodegenError(format!(
            "LLVM cannot emit a metadata object for {}",
            machine.get_triple().as_str().to_string_lossy()
        )));
    }
    // SAFETY: the shim returns a newly owned LLVM memory buffer on success.
    let buffer = unsafe { MemoryBuffer::new(buffer) };
    std::fs::write(path, buffer.as_slice())
        .map_err(|error| CodegenError(format!("failed to write {}: {error}", path.display())))
}

#[cfg(test)]
mod tests;
