//! 编译期执行（comptime / const）相关基础设施。
//!
//! 当前阶段（TODO T1202a/T1202b/T1202c/T1203）落地：
//! - 最小值模型（ConstValue）
//! - 纯表达式求值（字面量/一元/二元/aggregate）与 block/`if`/assignment 等宿主驱动节点
//! - `const fun` 解释器入口（现已接到 compilation-unit resolve/typecheck 主线，支持跨文件顶层调用与 generic 实例化）
//!   - 声明级合同已固定：`const fun` 自身只能省略 effect row，或显式写 `/ Pure` / `/ Pure!`；
//!     不允许声明 `<eff ...>` effect-row 参数
//! - `const val` initializer 的常量折叠（用于 fixtures 回归）
//! - 普通 `if` / `do` / 局部 `val/var` / assignment / `while` / `for` / `break` / `continue`
//!   与 `comptime { ... }` / `comptime if` / `comptime for` 的解释执行
//! - 为后续 `const fun` 解释器与 `comptime { ... }` 执行提供可复用的底座；
//! - 在不依赖 LLVM 后端的前端阶段完成常量求值与错误诊断。
//!
//! 非目标（留给后续子任务 T1204+）：
//! - `when`、`handle/perform`、闭包/lambda 等更复杂语义；
//! - 超出上述 Pure/Pure! 合同的 effectful/effect-polymorphic `const fun` 设计；
//! - 更完整的 `const fun` 静态约束（例如禁止闭包捕获）。

mod eval;
mod interpreter;
mod value;

pub use eval::{ConstEvalCtx, ConstEvalError, eval_const_expr};
pub use interpreter::{
    ConstBinding, ConstEvalOptions, RuntimeComptimePlan, eval_const_bindings_in_compilation_unit,
    eval_const_bindings_in_file, plan_runtime_comptime_in_file, trim_package_level_comptime_ifs,
    trim_package_level_comptime_ifs_in_compilation_unit,
    trim_package_level_comptime_ifs_in_cone_info_compilation_unit,
    trim_package_level_comptime_ifs_in_indexed_compilation_unit,
};
pub use value::{
    ConstEnum, ConstFloat, ConstFloatTy, ConstInt, ConstIntTy, ConstStruct, ConstValue,
};

#[cfg(test)]
mod tests;
