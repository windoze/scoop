//! Physical registration counts from the statepoints LLVM actually lowered.
use super::*;
use inkwell::llvm_sys::core::{LLVMGetFirstUse, LLVMGetNextUse, LLVMGetUser, LLVMIsACallInst};

#[derive(Debug)]
pub(crate) struct LoweredSafepoints {
    pub(super) roots: BTreeMap<u64, usize>,
}

impl LoweredSafepoints {
    pub(crate) fn site_count(&self) -> usize {
        self.roots.len()
    }

    pub(crate) fn root_count(&self, id: u64) -> Option<usize> {
        self.roots.get(&id).copied()
    }
}

pub(super) fn collect(module: &LlvmModule<'_>) -> LoweredSafepoints {
    let mut roots = BTreeMap::new();
    for function in module.get_functions() {
        for block in function.get_basic_blocks() {
            for instruction in block.get_instructions() {
                if !matches!(
                    instruction.get_opcode(),
                    InstructionOpcode::Call | InstructionOpcode::Invoke
                ) {
                    continue;
                }
                // SAFETY: this instruction is a CallBase in LLVM's successful output.
                let call = unsafe { CallSiteValue::new(instruction.as_value_ref()) };
                if call.get_called_fn_value().is_none_or(|callee| {
                    !callee
                        .get_name()
                        .to_bytes()
                        .starts_with(b"llvm.experimental.gc.statepoint")
                }) {
                    continue;
                }
                let id = unsafe {
                    LLVMConstIntGetZExtValue(LLVMGetOperand(instruction.as_value_ref(), 0))
                };
                let live = call
                    .get_operand_bundles()
                    .find(|bundle| bundle.get_tag() == Ok("gc-live"))
                    .map(|bundle| bundle.get_args().collect::<Vec<_>>())
                    .unwrap_or_default();
                let mut derived = BTreeSet::new();
                // Scoop invokes and native transitions use explicit roots and
                // zero-live statepoints. Ordinary relocating calls use token users.
                let mut usage = unsafe { LLVMGetFirstUse(instruction.as_value_ref()) };
                while !usage.is_null() {
                    let user = unsafe { LLVMGetUser(usage) };
                    if !unsafe { LLVMIsACallInst(user) }.is_null() {
                        let relocation = unsafe { CallSiteValue::new(user) };
                        if relocation.get_called_fn_value().is_some_and(|callee| {
                            callee
                                .get_name()
                                .to_bytes()
                                .starts_with(b"llvm.experimental.gc.relocate")
                        }) {
                            let index = unsafe { LLVMConstIntGetZExtValue(LLVMGetOperand(user, 2)) }
                                as usize;
                            derived.insert(live[index].as_value_ref() as usize);
                        }
                    }
                    usage = unsafe { LLVMGetNextUse(usage) };
                }
                roots.insert(id, derived.len());
            }
        }
    }
    LoweredSafepoints { roots }
}
