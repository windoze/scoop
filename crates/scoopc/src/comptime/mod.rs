//! 编译期执行（comptime / const）相关基础设施。
//!
//! 当前阶段（TODO T1202a/T1202b/T1202c）落地：
//! - 最小值模型（ConstValue）
//! - 纯表达式求值（字面量/一元/二元/aggregate）
//! - `const fun` 的最小解释器入口（调用 + 局部 val + return + block 最后表达式返回）
//! - `const val` initializer 的常量折叠（用于 fixtures 回归）
//! - 为后续 `const fun` 解释器与 `comptime { ... }` 执行提供可复用的底座；
//! - 在不依赖 LLVM 后端的前端阶段完成常量求值与错误诊断。
//!
//! 非目标（留给后续子任务 T1203+）：
//! - `comptime { ... }` 的执行上下文与语句级执行；
//! - 控制流（`if/when`）、effects、循环等复杂语义；
//! - 更完整的 `const fun` 静态约束（例如禁止闭包捕获）。

mod eval;
mod interpreter;
mod value;

pub use eval::{ConstEvalCtx, ConstEvalError, eval_const_expr};
pub use interpreter::{ConstBinding, ConstEvalOptions, eval_const_bindings_in_file};
pub use value::{ConstEnum, ConstInt, ConstIntTy, ConstStruct, ConstValue};

#[cfg(test)]
mod tests;
