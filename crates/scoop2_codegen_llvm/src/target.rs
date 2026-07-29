//! 目标平台信息：host target triple + data layout。
//!
//! 当前阶段使用 host triple（与旧 codegen 一致）。`TargetInfo` 在 `context.rs` 中用于
//! 配置 inkwell `TargetMachine` / `TargetData`。

/// 目标平台信息。
#[derive(Clone, Debug)]
pub struct TargetInfo {
    /// LLVM target triple（如 `arm64-apple-darwin`）。
    pub triple: String,
    /// CPU 名（如 `apple-m1` / `generic`）。
    pub cpu: String,
}

impl TargetInfo {
    /// 取当前 host 的目标信息。
    pub fn host() -> Self {
        // inkwell 在 `feature = "llvm"` 下提供 `TargetMachine::get_default_triple`；
        // 这里仅做字符串收集，避免在未启用 llvm feature 时引用 inkwell。
        let triple = host_triple();
        let cpu = "generic".to_string();
        TargetInfo { triple, cpu }
    }
}

/// 返回 host 的 LLVM target triple。
#[cfg(feature = "llvm")]
fn host_triple() -> String {
    use inkwell::targets::TargetMachine;
    TargetMachine::get_default_triple()
        .as_str()
        .to_string_lossy()
        .into_owned()
}

#[cfg(not(feature = "llvm"))]
fn host_triple() -> String {
    // 无 LLVM 时的回退（仅用于非 llvm 构建的占位，不会被实际使用）。
    std::env::consts::ARCH.to_string()
}
