//! LLVM StackMap（`.llvm_stackmaps` / `__llvm_stackmaps`）的最小解析器。
//!
//! 说明：
//! - 该文件位于 LLVM backend namespace 下，便于后端相关代码引用；
//! - 真实实现位于 `scoopc_codegen_llvm::stackmap`（不依赖 LLVM/inkwell），这里仅做 re-export，
//!   以保持现有引用路径稳定（例如单测中 `super::stackmap::StackMapHeader`）。

#[allow(unused_imports)]
pub use crate::stackmap::{StackMapHeader, StackMapParseError};
