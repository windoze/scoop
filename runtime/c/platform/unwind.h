// Scoop C runtime platform unwind layer (v0).
//
// 目标（TODO T1411a）：
// - 把 unwind/ABI 细节收敛到 `runtime/c/platform/*`；
// - core/runtime 与未来的 GC stack walking（T1505）通过一个最小 API 复用该能力；
// - API 必须保持“内部链接”（static），避免污染 runtime ABI 导出符号集合（见 T1401）。
//
// 当前阶段：
// - 仅实现 current-thread backtrace（采样 instruction pointers）；
// - remote unwind / 从 ucontext 开始的逐帧 unwind 后置到 T1411b/T1505；
// - Windows backend 先占位返回 0（上层需通过 capability/诊断处理）。

#pragma once

#include <stdint.h>

// 用于 platform backend 内部实现：避免把“平台能力函数”当作未使用的静态函数触发编译警告。
#if defined(__clang__) || defined(__GNUC__)
#define SCOOP_UNWIND_UNUSED __attribute__((unused))
#else
#define SCOOP_UNWIND_UNUSED
#endif

// 捕获当前线程的 backtrace（instruction pointers）。
//
// 约定：
// - 返回值为实际写入的帧数（可能为 0 表示不支持或失败）；
// - `skip_frames` 用于跳过最顶端的 N 帧（用于隐藏 runtime wrapper 自身）；
// - 该 API 只承诺“尽力而为”：不同优化级别/链接方式可能影响帧数与地址分布。
static uint32_t scoop_platform_unwind_capture_ips(uintptr_t *out_ips,
                                                  uint32_t out_cap,
                                                  uint32_t skip_frames);

// --- backend selection ---
//
// 注意：这里通过 include 选择 backend，并在 backend 文件中提供上述 static 函数定义。
// 这样可避免新增任何全局导出符号，从而与 runtime ABI allowlist 检查（T1401）保持兼容。
#if defined(_WIN32)
#include "unwind_win32.c"
#else
#include "unwind_posix.c"
#endif

