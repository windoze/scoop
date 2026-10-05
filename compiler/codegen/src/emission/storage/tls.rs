//! Mach-O descriptors/templates and ELF TLS storage use different extents.

use super::*;

pub(super) fn boundaries(
    llvm: &LlvmModule<'_>,
    definition: &scoop_lir::DefinitionSymbolPlanV1,
    symbol: &str,
    size: u64,
    target: scoop_lir::LirTargetProfile,
) -> Result<(), CodegenError> {
    let format = target.native_object_format();
    let normalization = target.contract().native_symbol_normalization();
    let native_symbol = normalization.compiler_generated_object_symbol(symbol);
    let mut assembly = String::new();
    for boundary in definition.atom_boundaries() {
        let (owner, extent) = match boundary.atom_role() {
            scoop_lir::DefinitionAtomRole::Primary => (
                native_symbol.clone(),
                match format {
                    scoop_lir::NativeObjectFormat::MachO64 => 24,
                    scoop_lir::NativeObjectFormat::Elf64 => size,
                },
            ),
            scoop_lir::DefinitionAtomRole::AddressTakenConstant
                if format == scoop_lir::NativeObjectFormat::MachO64 =>
            {
                (format!("_{symbol}$tlv$init"), size)
            }
            role => return Err(CodegenError(format!("unexpected TLS atom role: {role:?}"))),
        };
        for (request, offset) in [(boundary.start(), 0), (boundary.end(), extent)] {
            let name = normalization.compiler_generated_object_symbol(request.symbol().as_str());
            assembly.push_str(&format!(".globl {name}\n{name} = {owner} + {offset}\n"));
            if format == scoop_lir::NativeObjectFormat::Elf64 {
                assembly.push_str(&format!(
                    ".hidden {name}\n.type {name},@tls_object\n.size {name},0\n"
                ));
            }
        }
    }
    unsafe {
        llvm_sys::core::LLVMAppendModuleInlineAsm(
            llvm.as_mut_ptr(),
            assembly.as_ptr().cast(),
            assembly.len(),
        );
    }
    Ok(())
}
