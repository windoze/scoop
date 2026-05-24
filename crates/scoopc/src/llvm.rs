//! LLVM backend facade for the `scoopc` umbrella crate.
//!
//! The standalone backend implementation lives in `scoopc_codegen_llvm`.
//! Single-file driver helpers live in `scoopc::pipeline`; they are re-exported
//! here only to preserve the historical `scoopc::llvm::*` compatibility path.

pub use crate::pipeline::{
    emit_minimal_main_asm_to_file, emit_minimal_main_asm_to_file_with_opt_level,
    emit_minimal_main_ir, emit_minimal_main_ir_to_file, emit_minimal_main_obj_to_file,
    emit_minimal_main_obj_to_file_with_opt_level,
};
pub use scoopc_codegen_llvm::llvm::*;
