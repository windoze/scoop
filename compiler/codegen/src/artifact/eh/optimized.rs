//! Retain the typed EH actions of invokes that survive ordinary LLVM passes.

use super::*;
use inkwell::basic_block::BasicBlock;
use inkwell::llvm_sys::core::{
    LLVMConstIntGetZExtValue, LLVMGetNumClauses, LLVMGetOperand, LLVMGetUnwindDest,
    LLVMIsAConstantInt, LLVMIsCleanup, LLVMSetPersonalityFn,
};
use inkwell::module::Module as LlvmModule;
use inkwell::values::{AsValueRef, CallSiteValue, InstructionOpcode};

impl ExpectedEh {
    pub(crate) fn finalize(&self, module: &LlvmModule<'_>) -> Result<Self, CodegenError> {
        let mut functions = BTreeMap::new();
        for function in module.get_functions() {
            let symbol = function
                .get_name()
                .to_str()
                .map_err(|error| CodegenError(format!("EH function name: {error}")))?;
            let mut available = self.function(symbol).cloned().unwrap_or_default();
            let mut actual = ExpectedEhFunction::default();
            for block in function.get_basic_blocks() {
                for instruction in block
                    .get_instructions()
                    .filter(|instruction| instruction.get_opcode() == InstructionOpcode::Invoke)
                {
                    let raw = instruction.as_value_ref();
                    // SAFETY: only Invoke instructions reach these CallBase APIs.
                    let call = unsafe { CallSiteValue::new(raw) };
                    let safepoint = if call.get_called_fn_value().is_some_and(|callee| {
                        callee
                            .get_name()
                            .to_bytes()
                            .starts_with(b"llvm.experimental.gc.statepoint")
                    }) {
                        let id = unsafe { LLVMGetOperand(raw, 0) };
                        if unsafe { LLVMIsAConstantInt(id) }.is_null() {
                            return Err(CodegenError(
                                "managed invoke has a nonconstant site id".into(),
                            ));
                        }
                        Some(unsafe { LLVMConstIntGetZExtValue(id) })
                    } else {
                        None
                    };
                    let unwind = unsafe { BasicBlock::new(LLVMGetUnwindDest(raw)) }
                        .ok_or_else(|| CodegenError("invoke has no unwind destination".into()))?;
                    let pad = unwind
                        .get_instructions()
                        .find(|instruction| instruction.get_opcode() != InstructionOpcode::Phi)
                        .filter(|instruction| {
                            instruction.get_opcode() == InstructionOpcode::LandingPad
                        })
                        .ok_or_else(|| {
                            CodegenError("optimized invoke does not reach a landing pad".into())
                        })?;
                    let action = if unsafe { LLVMGetNumClauses(pad.as_value_ref()) } != 0 {
                        EhActionKind::CatchAll
                    } else if unsafe { LLVMIsCleanup(pad.as_value_ref()) } != 0 {
                        EhActionKind::Cleanup
                    } else {
                        return Err(CodegenError(
                            "optimized landing pad has no catch or cleanup action".into(),
                        ));
                    };
                    let position = available.invokes.iter().position(|invoke| invoke.safepoint == safepoint && invoke.action == action)
                        .ok_or_else(|| CodegenError(format!("optimized invoke in {symbol} has an unexpected EH action or site {safepoint:?}")))?;
                    actual.invokes.push(available.invokes.remove(position));
                }
            }
            if !actual.invokes.is_empty() {
                functions.insert(symbol.to_owned(), actual);
            } else if function.get_personality_function().is_some() {
                // Unreachable-block removal has removed every local unwind edge and pad.
                // LLVM otherwise emits an empty LSDA for the unused personality.
                unsafe { LLVMSetPersonalityFn(function.as_value_ref(), std::ptr::null_mut()) };
            }
        }
        Ok(Self { functions })
    }
}
