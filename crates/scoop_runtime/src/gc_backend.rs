//! GC backend 能力矩阵（编译期可检查）。
//!
//! 设计目标（TODO T1405b）：
//! - 把“当前选择的 GC backend”与其 capability 固化为稳定的 Rust 常量；
//! - 供运行时集成测试做 gating/断言，避免在不支持的 backend 上出现“静默不一致”；
//! - 作为后续 Immix/adapter backend 的统一扩展点（capability 只增不删，语义向后兼容）。
//!
//! 说明：
//! - 当前 capability 仅覆盖最小集合（STW/多线程 roots/移动/roots 更新/shadow stack roots）。
//! - capability 与 `runtime/c/scoop_gc_backend.h` 保持同名/同语义；但这里不从 C 侧导入，
//!   而是基于 Cargo features（`gc-baseline`/`gc-minimal`/`gc-immix`）固化为 compile-time 常量。

// 这些 feature 是互斥的：选择多个会导致 build.rs 选择 backend 时语义不明确。
#[cfg(all(feature = "gc-baseline", feature = "gc-minimal"))]
compile_error!(
    "features `gc-baseline` and `gc-minimal` are mutually exclusive; use `--no-default-features` when selecting `gc-minimal`"
);
#[cfg(all(feature = "gc-baseline", feature = "gc-immix"))]
compile_error!(
    "features `gc-baseline` and `gc-immix` are mutually exclusive; use `--no-default-features` when selecting `gc-immix`"
);
#[cfg(all(feature = "gc-minimal", feature = "gc-immix"))]
compile_error!("features `gc-minimal` and `gc-immix` are mutually exclusive");

/// 编译期选择的 GC backend（与 C 侧 `SCOOP_GC_BACKEND_*` 一一对应）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcBackend {
    /// baseline：协作式 stop-the-world mark-sweep（目前默认）。
    Baseline,
    /// minimal：单线程、无 STW 的最小 backend（用于验证 backend 选择机制）。
    Minimal,
    /// immix：Immix GC（v0：协作式 STW、moving/compaction；性能优化逐步落地）。
    Immix,
}

/// GC backend 的能力集合（capability matrix）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GcCapabilities {
    /// 是否支持 stop-the-world（当前为“协作式 STW”）。
    pub stw: bool,
    /// 是否支持在多线程场景下枚举“所有已注册线程”的 roots。
    pub multi_thread_roots_enum: bool,
    /// 是否为 moving/compaction GC（需要转发指针 + 引用修复）。
    pub moving: bool,
    /// 是否支持“精确 roots 更新”（moving GC 需要把 roots 槽位改写为新地址）。
    pub precise_roots_update: bool,
    /// roots 是否来源于 shadow stack（编译器插桩 push/pop 的 roots frame 链表）。
    pub shadow_stack_roots: bool,
}

/// 当前选择的 GC backend。
pub const GC_BACKEND: GcBackend = resolve_gc_backend();

/// 当前 backend 的能力矩阵。
pub const GC_CAPABILITIES: GcCapabilities = match GC_BACKEND {
    GcBackend::Baseline => GcCapabilities {
        stw: true,
        multi_thread_roots_enum: true,
        moving: false,
        precise_roots_update: false,
        shadow_stack_roots: true,
    },
    GcBackend::Minimal => GcCapabilities {
        stw: false,
        multi_thread_roots_enum: false,
        moving: false,
        precise_roots_update: false,
        shadow_stack_roots: true,
    },
    GcBackend::Immix => GcCapabilities {
        stw: true,
        multi_thread_roots_enum: true,
        moving: true,
        precise_roots_update: true,
        shadow_stack_roots: true,
    },
};

/// 解析编译期选择的 backend。
///
/// 备注：
/// - `build.rs` 会把 C 侧的 `SCOOP_GC_BACKEND` 宏设置为同一选择；
/// - 当未显式启用任何 backend feature 时，build.rs 与 C 侧都会默认回退到 baseline。
const fn resolve_gc_backend() -> GcBackend {
    if cfg!(feature = "gc-immix") {
        return GcBackend::Immix;
    }
    if cfg!(feature = "gc-minimal") {
        return GcBackend::Minimal;
    }
    GcBackend::Baseline
}
