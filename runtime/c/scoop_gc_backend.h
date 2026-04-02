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

#ifndef SCOOP_GC_BACKEND
#define SCOOP_GC_BACKEND SCOOP_GC_BACKEND_BASELINE
#endif

#if (SCOOP_GC_BACKEND != SCOOP_GC_BACKEND_BASELINE) && \
    (SCOOP_GC_BACKEND != SCOOP_GC_BACKEND_MINIMAL)
#error "unsupported SCOOP_GC_BACKEND value"
#endif

// capability（供后续任务做编译期检查/测试 gating 使用）
#if SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_BASELINE
#define SCOOP_GC_CAP_STW 1
#else
#define SCOOP_GC_CAP_STW 0
#endif

