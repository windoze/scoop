//! Codegen stage: mechanically translate LIR to LLVM IR, emit
//! TypeDescriptors, expand codegen-stage intrinsics, produce `.o`.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.5 and
//! `docs/milestone1/DESIGN.md` section 2.6.

use std::path::Path;

use inkwell::context::Context;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use inkwell::values::GlobalValue;
use inkwell::{AddressSpace, OptimizationLevel};
use scoop_lir::{GlobalInit, Instruction, Module, Operand};

/// Error produced while translating LIR or emitting the object file.
#[derive(Debug)]
pub struct CodegenError(pub String);

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CodegenError {}

/// Translate `module` to LLVM IR and emit an object file at `output`
/// using the host target.
pub fn emit_object(module: &Module, output: &Path) -> Result<(), CodegenError> {
    Target::initialize_native(&InitializationConfig::default())
        .map_err(|e| CodegenError(format!("failed to initialize native target: {e}")))?;

    let string_layout = module
        .meta
        .layouts
        .iter()
        .find(|layout| layout.name == "String")
        .ok_or_else(|| CodegenError("LIR meta lacks a layout for String".to_string()))?;

    let context = Context::create();
    let llvm = context.create_module("scoop");
    let builder = context.create_builder();

    let ptr_ty = context.ptr_type(AddressSpace::default());
    let i8_ty = context.i8_type();
    let i64_ty = context.i64_type();
    let void_ty = context.void_type();

    // @scoop_td_String: { i64 type_id, i64 size, i64 align, ptr ref_offsets }
    // (runtime spec 2.2; type_id 1 is the M1-only type, ref_offsets is null
    // until the GC lands).
    let td_ty = context.struct_type(
        &[i64_ty.into(), i64_ty.into(), i64_ty.into(), ptr_ty.into()],
        false,
    );
    let string_td = llvm.add_global(td_ty, None, scoop_lir::STRING_TD_SYMBOL);
    string_td.set_constant(true);
    string_td.set_initializer(&context.const_struct(
        &[
            i64_ty.const_int(1, false).into(),
            i64_ty.const_int(string_layout.size, false).into(),
            i64_ty.const_int(string_layout.align, false).into(),
            ptr_ty.const_null().into(),
        ],
        false,
    ));

    // Globals. Indexed by GlobalId (arena iteration is in index order).
    let mut globals: Vec<GlobalValue> = Vec::with_capacity(module.globals.len());
    for (_, global) in module.globals.iter() {
        match &global.init {
            GlobalInit::StringConst(value) => {
                // { ptr td, i64 len, [N x i8] data } (runtime spec 2.4).
                let bytes = value.as_bytes();
                let ty = context.struct_type(
                    &[
                        ptr_ty.into(),
                        i64_ty.into(),
                        i8_ty.array_type(bytes.len() as u32).into(),
                    ],
                    false,
                );
                let llvm_global = llvm.add_global(ty, None, &global.symbol);
                llvm_global.set_constant(true);
                llvm_global.set_initializer(&context.const_struct(
                    &[
                        string_td.as_pointer_value().into(),
                        i64_ty.const_int(bytes.len() as u64, false).into(),
                        context.const_string(bytes, false).into(),
                    ],
                    false,
                ));
                globals.push(llvm_global);
            }
        }
    }

    // Functions: single entry block, straight-line instruction list.
    for function in &module.functions {
        let llvm_function = llvm.add_function(&function.symbol, void_ty.fn_type(&[], false), None);
        let entry = context.append_basic_block(llvm_function, "entry");
        builder.position_at_end(entry);
        for instruction in &function.body {
            match instruction {
                Instruction::Call { symbol, args } => {
                    let params: Vec<_> = args.iter().map(|_| ptr_ty.into()).collect();
                    let callee = llvm.get_function(symbol).unwrap_or_else(|| {
                        llvm.add_function(symbol, void_ty.fn_type(&params, false), None)
                    });
                    let call_args: Vec<_> = args
                        .iter()
                        .map(|arg| match arg {
                            Operand::StringGlobal(id) => globals[id.into_raw().into_u32() as usize]
                                .as_pointer_value()
                                .into(),
                        })
                        .collect();
                    builder
                        .build_call(callee, &call_args, "")
                        .map_err(|e| CodegenError(format!("call @{symbol}: {e}")))?;
                }
            }
        }
        builder
            .build_return(None)
            .map_err(|e| CodegenError(format!("ret @{}: {e}", function.symbol)))?;
    }

    llvm.verify()
        .map_err(|e| CodegenError(format!("invalid LLVM module: {e}")))?;

    let triple = TargetMachine::get_default_triple();
    let target = Target::from_triple(&triple)
        .map_err(|e| CodegenError(format!("no target for host triple: {e}")))?;
    let machine = target
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::None,
            RelocMode::Default,
            CodeModel::Default,
        )
        .ok_or_else(|| CodegenError("failed to create host target machine".to_string()))?;
    machine
        .write_to_file(&llvm, FileType::Object, output)
        .map_err(|e| CodegenError(format!("failed to write {}: {e}", output.display())))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use la_arena::Arena;
    use scoop_lir::{Function, Global, Layout, LirMeta};

    use super::*;

    /// A hello-world-shaped module: one string constant, a user function
    /// calling the runtime, and the entry calling the user function.
    fn hello_module() -> Module {
        let mut globals = Arena::default();
        let hello = globals.alloc(Global {
            symbol: "scoop.string.0".to_string(),
            init: GlobalInit::StringConst("hello, world".to_string()),
        });
        Module {
            globals,
            functions: vec![
                Function {
                    symbol: "scoop.greet".to_string(),
                    body: vec![Instruction::Call {
                        symbol: "scoop_rt_println".to_string(),
                        args: vec![Operand::StringGlobal(hello)],
                    }],
                },
                Function {
                    symbol: "scoop_main".to_string(),
                    body: vec![Instruction::Call {
                        symbol: "scoop.greet".to_string(),
                        args: vec![],
                    }],
                },
            ],
            entry_symbol: "scoop_main".to_string(),
            meta: LirMeta {
                layouts: vec![Layout {
                    name: "String".to_string(),
                    size: 16,
                    align: 8,
                    ref_field_offsets: vec![],
                }],
            },
        }
    }

    #[test]
    fn emits_non_empty_object_file() {
        let module = hello_module();
        let output =
            std::env::temp_dir().join(format!("scoop_codegen_test_{}.o", std::process::id()));
        emit_object(&module, &output).expect("emit object");
        let len = std::fs::metadata(&output)
            .expect("object file exists")
            .len();
        assert!(len > 0, "object file is empty");
        std::fs::remove_file(&output).ok();
    }
}
