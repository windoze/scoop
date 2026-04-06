// Scoop GC backend selection (v0).
//
// 目标（TODO T1405a）：
// - 为 GC 引入“编译期可选择”的 backend 开关；
// - 允许在不改变 runtime ABI allowlist 的前提下，把不同 GC 实现放在不同编译单元中；
// - 默认选择 baseline backend，保持现有行为不变。
//
// 说明：
// - backend 选择通过宏 `SCOOP_GC_BACKEND` 控制；
// - 当未显式定义该宏时，默认使用 baseline；
// - 值必须为下列常量之一，否则编译时报错。

#pragma once

#define SCOOP_GC_BACKEND_BASELINE 1
#define SCOOP_GC_BACKEND_MINIMAL 2
#define SCOOP_GC_BACKEND_IMMIX 3
#define SCOOP_GC_BACKEND_HOSTED 4

#ifndef SCOOP_GC_BACKEND
#define SCOOP_GC_BACKEND SCOOP_GC_BACKEND_BASELINE
#endif

#if (SCOOP_GC_BACKEND != SCOOP_GC_BACKEND_BASELINE) && \
    (SCOOP_GC_BACKEND != SCOOP_GC_BACKEND_MINIMAL) &&  \
    (SCOOP_GC_BACKEND != SCOOP_GC_BACKEND_IMMIX) &&    \
    (SCOOP_GC_BACKEND != SCOOP_GC_BACKEND_HOSTED)
#error "unsupported SCOOP_GC_BACKEND value"
#endif

// --- capability matrix（TODO T1405b） ---
//
// 目标：
// - 把“backend 能力”固化为编译期可检查的宏，供 C 侧 `#if` 分支与 Rust 测试 gating 使用；
// - 让“选择了某 backend 但测试/实现假设不成立”的情况更早、更清晰地暴露出来。
//
// 说明：
// - 所有 capability 宏均为 0/1；
// - 这里只定义能力“形状”，不在 T1405b 内实现 moving/roots update 等能力（留给后续任务）。
//
// 名词约定：
// - STW：stop-the-world。当前 v0 实现为“协作式 STW”（线程需要进入 `scoop_gc_safepoint` 才能暂停）。
// - roots：根集槽位（stackmap spill slots / native_roots slots / handles / pinned 等）。

#if SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_BASELINE

// baseline：协作式 STW + 多线程 roots 枚举（通过线程注册表 + shadow stack 链表扫描）。
#define SCOOP_GC_CAP_STW 1
#define SCOOP_GC_CAP_MULTI_THREAD_ROOTS_ENUM 1

// v0：非移动 mark-sweep，不更新 roots。
#define SCOOP_GC_CAP_MOVING 0
#define SCOOP_GC_CAP_PRECISE_ROOTS_UPDATE 0

// GC-FIX Phase B2：roots 由 stackmap + native_roots + handles/pinned 等机制提供。
#define SCOOP_GC_CAP_STACKMAP_ROOTS 1
#define SCOOP_GC_CAP_NATIVE_ROOTS 1

#elif SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_MINIMAL

// minimal：单线程/无 STW。若检测到多线程参与注册，collect 退化为 no-op（宁可泄漏也不错误回收）。
#define SCOOP_GC_CAP_STW 0
#define SCOOP_GC_CAP_MULTI_THREAD_ROOTS_ENUM 0

// v0：非移动，不更新 roots。
#define SCOOP_GC_CAP_MOVING 0
#define SCOOP_GC_CAP_PRECISE_ROOTS_UPDATE 0

// minimal backend 无 STW/park，也不维护 native_roots slots；roots 仅来自 handles/pinned。
#define SCOOP_GC_CAP_STACKMAP_ROOTS 0
#define SCOOP_GC_CAP_NATIVE_ROOTS 0

#elif SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_IMMIX

// Immix v0：协作式 STW、moving/compaction。
// - stop-the-world 为协作式：线程需要进入 `scoop_gc_safepoint()` 才会暂停。
#define SCOOP_GC_CAP_STW 1
#define SCOOP_GC_CAP_MULTI_THREAD_ROOTS_ENUM 1

#define SCOOP_GC_CAP_MOVING 1
#define SCOOP_GC_CAP_PRECISE_ROOTS_UPDATE 1

#define SCOOP_GC_CAP_STACKMAP_ROOTS 1
#define SCOOP_GC_CAP_NATIVE_ROOTS 1

#elif SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_HOSTED

// hosted/adapter v0：单线程、无 STW。
//
// 说明：
// - 该 backend 的设计目标是“尽量不依赖 OS 线程/同步原语”，以便在受限环境（例如 WASM/embedded）
//   里可以替换/裁剪为 hosted allocator/collector 或 host GC adapter；
// - v0 仍复用 shadow stack roots，使其可单独回归验证；未来若对接 WASM GC，可把 roots 形态
//   升级为 host-managed references，并将该 capability 改为 0（由后续任务细化）。
#define SCOOP_GC_CAP_STW 0
#define SCOOP_GC_CAP_MULTI_THREAD_ROOTS_ENUM 0

#define SCOOP_GC_CAP_MOVING 0
#define SCOOP_GC_CAP_PRECISE_ROOTS_UPDATE 0

// hosted backend v0 无 STW/park，也不维护 native_roots slots；roots 仅来自 handles/pinned。
#define SCOOP_GC_CAP_STACKMAP_ROOTS 0
#define SCOOP_GC_CAP_NATIVE_ROOTS 0

#endif
