//! 编译期执行（comptime / const）相关基础设施。
//!
//! 当前阶段（TODO T1202a）只落地“最小值模型 + 纯表达式求值器”，用于：
//! - 为后续 `const fun` 解释器与 `comptime { ... }` 执行提供可复用的底座；
//! - 在不依赖 LLVM 后端的前端阶段完成常量求值与错误诊断。
//!
//! 非目标（留给后续子任务 T1202b/T1202c）：
//! - `const fun` 的调用/栈帧/局部变量；
//! - 控制流（`if/when`）、effects、循环等复杂语义。

mod eval;
mod value;

pub use eval::{ConstEvalCtx, ConstEvalError, eval_const_expr};
pub use value::{ConstEnum, ConstInt, ConstIntTy, ConstStruct, ConstValue};

#[cfg(test)]
mod tests;
