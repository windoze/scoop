//! `scoop2_codegen_llvm`：Scoop 新 pipeline 的 LLVM 后端。
//!
//! 消费 `scoop2_lir::LirProgram`（自包含），产出 LLVM module（IR 文本 / object 文件）。
//! codegen 遍历 LIR 结构时是机械翻译：所有布局/ABI/分发决策已在 LIR 完成。
//!
//! GC 方案：explicit root frame（TLS 链表 + slot 镜像），不使用 statepoint/stackmap。
//!
//! 详见 `NEW-LLVM-CODEGEN.md`。

#![cfg_attr(not(feature = "llvm"), allow(unused_imports))]

pub mod error;
pub mod target;

#[cfg(feature = "llvm")]
pub mod body;
#[cfg(feature = "llvm")]
pub mod context;
#[cfg(feature = "llvm")]
pub mod emit;
#[cfg(feature = "llvm")]
pub mod gc;
#[cfg(feature = "llvm")]
pub mod globals;
#[cfg(feature = "llvm")]
pub mod intrinsics;
#[cfg(feature = "llvm")]
pub mod runtime_abi;
#[cfg(feature = "llvm")]
pub mod types;

pub use error::{CodegenError, CodegenResult};

#[cfg(feature = "llvm")]
pub use emit::{EmitOptions, EmittedModule, emit_object_to_file, emit_program};
