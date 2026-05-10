//! Effect-lowered LLVM backend 的独立主题根。
//!
//! P6-T01 先在这里显式保留目录边界，并把统一 stage/emit 入口接到
//! `pipeline/llvm_codegen_stage.rs` + `llvm/emit.rs`。
//! 后续 P6-T02/P6-T04 会在该目录下继续填入 type/layout/body/gc/runtime 等细分模块，
//! 逐步把 effect lowering 从旧 `effect/` 主题中彻底拆开。

mod body;
mod layout;
mod types;
mod value;
