//! Instruction boundaries come from the linked LLVM X86 disassembler.

use std::ffi::CStr;

use inkwell::llvm_sys::disassembler::{
    LLVMCreateDisasm, LLVMDisasmContextRef, LLVMDisasmDispose, LLVMDisasmInstruction,
};
use inkwell::targets::Target;

use crate::CodegenError;

use super::{FunctionRelocation, TextSection};

pub(super) struct Instruction {
    pub(super) start: u64,
    pub(super) end: u64,
    pub(super) call: bool,
    saves_frame: bool,
    sets_frame: bool,
}

struct Disassembler(LLVMDisasmContextRef);

impl Disassembler {
    fn new() -> Result<Self, CodegenError> {
        Target::initialize_x86(&Default::default());
        // SAFETY: the triple is NUL-terminated; no callbacks or user data are used.
        let raw = unsafe {
            LLVMCreateDisasm(
                c"x86_64-unknown-linux-gnu".as_ptr(),
                std::ptr::null_mut(),
                0,
                None,
                None,
            )
        };
        if raw.is_null() {
            return Err(CodegenError("LLVM X86 disassembler is unavailable".into()));
        }
        Ok(Self(raw))
    }
}

impl Drop for Disassembler {
    fn drop(&mut self) {
        // SAFETY: this wrapper owns the non-null context returned by LLVM.
        unsafe { LLVMDisasmDispose(self.0) };
    }
}

pub(super) fn instructions(bytes: &[u8], address: u64) -> Result<Vec<Instruction>, CodegenError> {
    let decoder = Disassembler::new()?;
    let mut offset = 0;
    let mut result = Vec::new();
    while offset < bytes.len() {
        let start = address
            .checked_add(offset as u64)
            .ok_or_else(|| CodegenError("amd64 instruction address overflows".into()))?;
        let mut output = [0i8; 256];
        // LLVM's C API predates const-correctness; the implementation only reads
        // the input buffer. The output buffer remains live throughout the call.
        let size = unsafe {
            LLVMDisasmInstruction(
                decoder.0,
                bytes[offset..].as_ptr().cast_mut(),
                (bytes.len() - offset) as u64,
                start,
                output.as_mut_ptr(),
                output.len(),
            )
        };
        if size == 0 || size > bytes.len() - offset {
            return Err(CodegenError(format!(
                "invalid or truncated amd64 instruction at {start:#x}"
            )));
        }
        // SAFETY: LLVM writes a NUL-terminated assembly string on success.
        let assembly = unsafe { CStr::from_ptr(output.as_ptr()) }.to_string_lossy();
        let mnemonic = assembly.split_whitespace().next().unwrap_or("");
        let instruction = &bytes[offset..offset + size];
        let end = start
            .checked_add(size as u64)
            .ok_or_else(|| CodegenError("amd64 instruction end overflows".into()))?;
        result.push(Instruction {
            start,
            end,
            call: matches!(mnemonic, "callq" | "call"),
            saves_frame: instruction == [0x55],
            sets_frame: instruction == [0x48, 0x89, 0xe5],
        });
        offset += size;
    }
    Ok(result)
}

pub(super) fn validate_safepoint(
    text: &TextSection,
    function: &FunctionRelocation,
    instruction_offset: u32,
    safepoint: u64,
    check_frame: bool,
) -> Result<u64, CodegenError> {
    let return_pc = function
        .address
        .checked_add(u64::from(instruction_offset))
        .ok_or_else(|| CodegenError(format!("SafepointId {safepoint} return PC overflows")))?;
    let bytes = text.range(function.address..return_pc)?;
    let decoded = instructions(bytes, function.address)?;
    let call = decoded.last().filter(|instruction| instruction.call && instruction.end == return_pc).ok_or_else(|| {
        CodegenError(format!(
            "SafepointId {safepoint} return PC {return_pc:#x} does not immediately follow an amd64 call"
        ))
    })?;
    if check_frame {
        let mut saved = false;
        let mut established = false;
        for instruction in &decoded {
            saved |= instruction.saves_frame;
            established |= saved && instruction.sets_frame;
            if instruction.call {
                break;
            }
        }
        if !established {
            return Err(CodegenError(format!(
                "managed amd64 function `{}` does not establish an RBP frame chain before its first call",
                function.symbol
            )));
        }
    }
    Ok(call.start)
}
