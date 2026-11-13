//! LLVM instrumentation mechanics; logical MIR positions belong to the producer.

use std::ffi::{CStr, c_char, c_void};

use inkwell::builder::Builder;
use inkwell::intrinsics::Intrinsic;
use inkwell::llvm_sys::core::LLVMDisposeMessage;
use inkwell::llvm_sys::prelude::LLVMModuleRef;
use inkwell::llvm_sys::target_machine::LLVMTargetMachineRef;
use inkwell::module::{Linkage, Module};
use inkwell::targets::TargetMachine;
use inkwell::values::{FunctionValue, GlobalValue, IntValue};

use crate::CodegenError;
pub use scoop_profile::IndexedRecord;

/// One physical record for one emitted LIR callable.
pub fn record(function: &scoop_lir::Function) -> Option<scoop_profile::Record> {
    let counters = function
        .blocks
        .iter()
        .flat_map(|(_, block)| block.instructions.iter())
        .filter_map(|instruction| match instruction {
            scoop_lir::Instruction::ProfileCounter(counter) => Some(counter.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    (!counters.is_empty()).then(|| scoop_profile::Record::new(function.symbol().into(), counters))
}

/// Save the same physical layouts used by instruction emission. This is an
/// ordinary compilation result, independent of debug/symbol observations.
pub fn profile_map(module: &scoop_lir::Module, target: &str) -> scoop_profile::ProfileMap {
    scoop_profile::ProfileMap {
        schema: scoop_profile::SCHEMA,
        target: target.into(),
        bodies: module.meta.profile_bodies.clone(),
        records: module.callable_bodies().filter_map(record).collect(),
    }
}

unsafe extern "C" {
    fn scoop_lower_profile_counters(module: LLVMModuleRef, target: LLVMTargetMachineRef);
    fn scoop_read_indexed_profile(
        data: *const c_char,
        size: usize,
        context: *mut c_void,
        sink: unsafe extern "C" fn(*mut c_void, *const c_char, usize, u64, *const u64, usize),
    ) -> *mut c_char;
}

/// One physical counter layout, owned by the function containing its increments.
pub struct LlvmProfileRecord<'ctx> {
    name: GlobalValue<'ctx>,
    intrinsic: FunctionValue<'ctx>,
    hash: IntValue<'ctx>,
    count: IntValue<'ctx>,
    counters: u32,
}

impl<'ctx> LlvmProfileRecord<'ctx> {
    pub fn new(
        module: &Module<'ctx>,
        function: FunctionValue<'ctx>,
        hash: u64,
        counters: u32,
    ) -> Result<Self, CodegenError> {
        if counters == 0 {
            return Err(CodegenError(
                "a profile record needs at least one counter".into(),
            ));
        }
        let context = module.get_context();
        let function_name = function.get_name();
        let bytes = function_name.to_bytes();
        let value = context.const_string(bytes, false);
        let global_name = format!("__profn_{}", function_name.to_string_lossy());
        let name = module.add_global(value.get_type(), None, &global_name);
        name.set_linkage(Linkage::Private);
        name.set_constant(true);
        name.set_initializer(&value);
        let intrinsic = Intrinsic::find("llvm.instrprof.increment")
            .and_then(|intrinsic| intrinsic.get_declaration(module, &[]))
            .expect("LLVM 22.1 provides the non-overloaded instrprof increment");
        Ok(Self {
            name,
            intrinsic,
            hash: context.i64_type().const_int(hash, false),
            count: context.i32_type().const_int(u64::from(counters), false),
            counters,
        })
    }

    pub fn increment(&self, builder: &Builder<'ctx>, index: u32) -> Result<(), CodegenError> {
        if index >= self.counters {
            return Err(CodegenError(
                "profile counter index exceeds its layout".into(),
            ));
        }
        builder
            .build_call(
                self.intrinsic,
                &[
                    self.name.as_pointer_value().into(),
                    self.hash.into(),
                    self.count.into(),
                    self.count
                        .get_type()
                        .const_int(u64::from(index), false)
                        .into(),
                ],
                "",
            )
            .map_err(|error| CodegenError(format!("profile increment: {error}")))?;
        Ok(())
    }
}

/// Lower increments to atomic 64-bit updates before normal LLVM optimization.
pub fn lower_counters(module: &Module<'_>, machine: &TargetMachine) {
    // Both handles remain owned by their callers for the entire pass invocation.
    unsafe { scoop_lower_profile_counters(module.as_mut_ptr(), machine.as_mut_ptr()) };
}

/// LLVM initially separates counters from an ODR function for its general
/// inliner. A production callable module has one final physical record; after
/// optimization its ELF contributions must follow the selected callable group.
pub(crate) fn associate_elf_callable_record(
    module: &Module<'_>,
    function_name: &str,
) -> Result<(), CodegenError> {
    let function = module.get_function(function_name).ok_or_else(|| {
        CodegenError(format!(
            "profiled ELF callable `{function_name}` is missing"
        ))
    })?;
    let Some(group) = function.as_global_value().get_comdat() else {
        return Ok(());
    };
    for global in module.get_globals() {
        if global.get_section().is_some_and(|section| {
            matches!(
                section.to_bytes(),
                b"__llvm_prf_cnts" | b"__llvm_prf_data" | b"__llvm_prf_names"
            )
        }) {
            global.set_comdat(group);
        }
    }
    Ok(())
}

/// Decode an indexed profile once, preserving all counters including zeros.
pub fn decode_indexed_profile(bytes: &[u8]) -> Result<Vec<IndexedRecord>, CodegenError> {
    let mut records = Vec::<IndexedRecord>::new();
    // LLVM borrows bytes and invokes the sink synchronously. The sink copies
    // each record before the LLVM iterator reuses its storage.
    let error = unsafe {
        scoop_read_indexed_profile(
            bytes.as_ptr().cast(),
            bytes.len(),
            (&raw mut records).cast(),
            collect_record,
        )
    };
    if error.is_null() {
        Ok(records)
    } else {
        let message = unsafe { CStr::from_ptr(error) }
            .to_string_lossy()
            .into_owned();
        unsafe { LLVMDisposeMessage(error) };
        Err(CodegenError(format!(
            "indexed instrumentation profile: {message}"
        )))
    }
}

unsafe extern "C" fn collect_record(
    context: *mut c_void,
    name: *const c_char,
    name_len: usize,
    hash: u64,
    counts: *const u64,
    count_len: usize,
) {
    // LLVM owns the callback buffers and guarantees the supplied extents.
    let records = unsafe { &mut *context.cast::<Vec<IndexedRecord>>() };
    let name = if name_len == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(name.cast(), name_len) }.to_vec()
    };
    let counts = if count_len == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(counts, count_len) }.to_vec()
    };
    records.push(IndexedRecord { name, hash, counts });
}

#[cfg(test)]
#[path = "profiling/tests.rs"]
mod tests;
