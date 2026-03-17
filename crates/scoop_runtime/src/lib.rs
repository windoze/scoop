//! Scoop 早期运行时（占位）。
//!
//! 说明：
//! - 当前 crate 的主要作用是把 `runtime/c` 的代码作为静态库编译进构建产物，
//!   供后续 `scoop build` 链接用户程序使用。
//! - 未来当 Scoop 具备足够的 `@NoGC/@Unsafe/FFI/线程` 能力时，本 crate 将
//!   逐步被 Scoop 自己实现的 GC/runtime 取代。

// 目前不暴露任何 Rust API。

