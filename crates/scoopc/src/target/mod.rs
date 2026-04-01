//! 目标平台信息（早期：capability gating / 平台选择器输入）。
//!
//! 说明：
//! - 目前编译器绝大多数流程仍只支持 host target（见 TODO T08xx/平台抽象路线）；
//! - 但 typecheck/stdlib/sysroot 的设计需要从第一天就能表达“平台能力差异”，
//!   避免出现“能通过解析/类型检查，但链接/运行必炸”的长期技术债；
//! - 因此这里先引入一个轻量的 `TargetPlatform`，用于：
//!   - delegated properties 等“由编译器插入 runtime 原语”的语义 gate（T1326c）
//!   - Cone.toml 平台选择器（spec §13.9；当前 driver 仍默认 host）
//!
//! 注意：这里的 `TargetPlatform` 与 sysroot 的 `scoop.core.Platform`（`getPlatform()` 返回值）
//! 概念上相关但不等价：前者用于编译器内部策略与 gate；后者是语言层可见的反射 API。

/// 编译目标平台（早期：用一个稳定字符串 id 表示）。
///
/// 命名风格：
/// - 对齐 spec 中 Cone 平台选择器示例：`linux-x64` / `macos-arm64` / `windows-x64`。
/// - 其它平台（例如 wasm/embedded）目前只用于 capability gating 与测试回归，不做更严格校验。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetPlatform {
    id: String,
}

impl TargetPlatform {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    /// 返回平台 id（例如 `macos-arm64`）。
    pub fn id(&self) -> &str {
        &self.id
    }

    /// host 平台 id（v0：来自 Cargo cfg；与 Cone selector 的 host id 保持一致）。
    pub fn host() -> Self {
        Self::new(host_target_platform_id())
    }

    /// 当前阶段是否认为该平台支持“线程 + 互斥锁”。
    ///
    /// 说明（early stage）：
    /// - 这里是一个保守的 gate：只有明确的 desktop/server host 平台返回 true；
    /// - wasm/embedded 等平台的细化策略会在 platform/backends 任务中补齐（T14xx/T15xx）。
    pub fn supports_threads(&self) -> bool {
        let id = self.id.as_str();
        id.starts_with("linux-") || id.starts_with("macos-") || id.starts_with("windows-")
    }

    /// 是否支持 `scoop.sync.Mutex` 这类同步原语的 runtime 落点。
    ///
    /// 备注：当前阶段 mutex 仅作为“线程能力”的一部分进行 gate。
    pub fn supports_sync_mutex(&self) -> bool {
        self.supports_threads()
    }
}

impl Default for TargetPlatform {
    fn default() -> Self {
        Self::host()
    }
}

/// 生成当前“host 目标平台 id”。
///
/// 说明：
/// - 该 id 使用 spec 中的 `linux-x64` / `macos-arm64` / `windows-x64` 命名风格；
/// - v0 阶段只支持 host target：交叉编译/target triple 选择留给后续任务。
fn host_target_platform_id() -> String {
    // 说明：不要使用 `CARGO_CFG_TARGET_*`：
    // - 这些环境变量通常只在 build script（build.rs）环境中可用；
    // - 在普通 crate 编译单元里，`option_env!()` 往往拿不到，从而退化成 `unknown-unknown`。
    //
    // v0 阶段只支持 host target，因此直接使用 Rust 的编译期常量即可。
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let arch = match arch {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        "i686" => "x86",
        other => other,
    };

    format!("{os}-{arch}")
}
