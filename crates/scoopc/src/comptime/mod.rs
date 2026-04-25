//! 编译期执行（comptime / const）相关基础设施。
//!
//! 当前阶段（TODO T1202a/T1202b/T1202c/T1203）落地：
//! - 最小值模型（ConstValue）
//! - 纯表达式求值（字面量/一元/二元/aggregate）
//! - `const fun` 的最小解释器入口（现已接到 compilation-unit resolve/typecheck 主线，支持跨文件 non-generic 顶层调用）
//! - `const val` initializer 的常量折叠（用于 fixtures 回归）
//! - `comptime { ... }` / `comptime if` 的最小语句级执行（仅在 const 解释器求值路径内）
//! - 为后续 `const fun` 解释器与 `comptime { ... }` 执行提供可复用的底座；
//! - 在不依赖 LLVM 后端的前端阶段完成常量求值与错误诊断。
//!
//! 非目标（留给后续子任务 T1204+）：
//! - `comptime for` 的执行与展开；
//! - 更通用的控制流（`if/when`）与循环（`while/for`）等复杂语义；
//! - 更完整的 `const fun` 静态约束（例如禁止闭包捕获）。

mod eval;
mod interpreter;
mod value;

pub use eval::{ConstEvalCtx, ConstEvalError, eval_const_expr};
pub use interpreter::{
    ConstBinding, ConstEvalOptions, eval_const_bindings_in_compilation_unit,
    eval_const_bindings_in_file, trim_package_level_comptime_ifs,
};
pub use value::{
    ConstEnum, ConstFloat, ConstFloatTy, ConstInt, ConstIntTy, ConstStruct, ConstValue,
};

#[cfg(test)]
mod tests;
