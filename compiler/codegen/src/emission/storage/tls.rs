//! Mach-O TLS descriptors and initial images are distinct physical atoms.

use super::*;

pub(super) fn boundaries(
    llvm: &LlvmModule<'_>,
    definition: &scoop_lir::DefinitionSymbolPlanV1,
    symbol: &str,
    size: u64,
) -> Result<(), CodegenError> {
    let mut assembly = String::new();
    for boundary in definition.atom_boundaries() {
        let (owner, extent) = match boundary.atom_role() {
            scoop_lir::DefinitionAtomRole::Primary => (format!("_{symbol}"), 24),
            scoop_lir::DefinitionAtomRole::AddressTakenConstant => {
                (format!("_{symbol}$tlv$init"), size)
            }
            role => return Err(CodegenError(format!("unexpected TLS atom role: {role:?}"))),
        };
        for (request, offset) in [(boundary.start(), 0), (boundary.end(), extent)] {
            let name = request.symbol();
            assembly.push_str(&format!(".globl _{name}\n_{name} = {owner} + {offset}\n"));
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
