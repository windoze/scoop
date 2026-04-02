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

#ifndef SCOOP_GC_BACKEND
#define SCOOP_GC_BACKEND SCOOP_GC_BACKEND_BASELINE
#endif

#if (SCOOP_GC_BACKEND != SCOOP_GC_BACKEND_BASELINE) && \
    (SCOOP_GC_BACKEND != SCOOP_GC_BACKEND_MINIMAL) &&  \
    (SCOOP_GC_BACKEND != SCOOP_GC_BACKEND_IMMIX)
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
// - roots：根集槽位（shadow stack/stackmap spill slots 等）。v0 只有 shadow stack。

#if SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_BASELINE

// baseline：协作式 STW + 多线程 roots 枚举（通过线程注册表 + shadow stack 链表扫描）。
#define SCOOP_GC_CAP_STW 1
#define SCOOP_GC_CAP_MULTI_THREAD_ROOTS_ENUM 1

// v0：非移动 mark-sweep，不更新 roots。
#define SCOOP_GC_CAP_MOVING 0
#define SCOOP_GC_CAP_PRECISE_ROOTS_UPDATE 0

// v0：roots 来源为 shadow stack（精确枚举）。
#define SCOOP_GC_CAP_SHADOW_STACK_ROOTS 1

#elif SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_MINIMAL

// minimal：单线程/无 STW。若检测到多线程参与注册，collect 退化为 no-op（宁可泄漏也不错误回收）。
#define SCOOP_GC_CAP_STW 0
#define SCOOP_GC_CAP_MULTI_THREAD_ROOTS_ENUM 0

// v0：非移动，不更新 roots。
#define SCOOP_GC_CAP_MOVING 0
#define SCOOP_GC_CAP_PRECISE_ROOTS_UPDATE 0

// v0：仍使用 shadow stack roots（单线程下精确枚举）。
#define SCOOP_GC_CAP_SHADOW_STACK_ROOTS 1

#elif SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_IMMIX

// Immix v0：协作式 STW、moving/compaction。
// - stop-the-world 为协作式：线程需要进入 `scoop_gc_safepoint()` 才会暂停；
// - roots 来源为 shadow stack，因此可在 compaction 时执行“精确 roots 更新”。
#define SCOOP_GC_CAP_STW 1
#define SCOOP_GC_CAP_MULTI_THREAD_ROOTS_ENUM 1

#define SCOOP_GC_CAP_MOVING 1
#define SCOOP_GC_CAP_PRECISE_ROOTS_UPDATE 1

#define SCOOP_GC_CAP_SHADOW_STACK_ROOTS 1

#endif
