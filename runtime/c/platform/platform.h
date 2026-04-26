// Scoop C runtime platform layer (v0).
//
// 目标（TODO T1402）：
// - 把 runtime 内部对 OS API 的直接调用收敛到 platform/backends；
// - core/runtime 代码只调用 platform API；
// - 本文件中的平台函数必须保持“内部链接”（static），避免污染 runtime ABI 导出符号集合。
//
// 说明：
// - v0 覆盖 env/time/io（早期 runtime 内部能力）。
// - T1403a 开始补齐 sync primitives（mutex/condvar）与线程自识别（self/equal），用于把
//   `scoop_sync_*`/task executor 等从 pthread 直接调用收敛到 platform API。

#pragma once

#include <stddef.h>
#include <stdint.h>

// --- platform API（internal linkage）---

// 用于 platform backend 内部实现：避免把“平台能力函数”当作未使用的静态函数触发编译警告。
#if defined(__clang__) || defined(__GNUC__)
#define SCOOP_PLATFORM_UNUSED __attribute__((unused))
#else
#define SCOOP_PLATFORM_UNUSED
#endif

// --- sync/thread types（opaque-ish, backend-dependent）---
//
// 说明：
// - 这些类型只用于 runtime 内部实现（例如把 pthread 类型隔离在 platform 层），不属于对外 ABI。
// - Windows backend 目前仅占位；storage 大小不作为稳定 ABI 承诺。
#if defined(_WIN32)
typedef struct ScoopPlatformMutex {
  uint64_t _storage[8];
} ScoopPlatformMutex;

typedef struct ScoopPlatformCondVar {
  uint64_t _storage[8];
} ScoopPlatformCondVar;

typedef uint64_t ScoopPlatformThread;
#else
#include <pthread.h>

typedef struct ScoopPlatformMutex {
  pthread_mutex_t inner;
} ScoopPlatformMutex;

typedef struct ScoopPlatformCondVar {
  pthread_cond_t inner;
} ScoopPlatformCondVar;

typedef pthread_t ScoopPlatformThread;
#endif

// env: `getenv`（返回 libc 风格 C string；可能为 NULL）
static const char *scoop_platform_env_getenv(const char *key_cstr);

// time: 获取 Unix epoch 毫秒（UTC），成功返回 1 并写入 out；失败返回 0。
static int scoop_platform_time_now_unix_millis(int64_t *out_unix_millis);

// io: 向 stdout/stderr 写入全部字节；成功返回 1；失败返回 0。
static int scoop_platform_io_write_stdout_all(const uint8_t *buf, size_t len);
static int scoop_platform_io_write_stderr_all(const uint8_t *buf, size_t len);

// io: 向 stdout/stderr 写入单个字节；成功返回 1；失败返回 0。
static int scoop_platform_io_write_stdout_byte(uint8_t byte);
static int scoop_platform_io_write_stderr_byte(uint8_t byte);

// io: 从 stdin 读取最多 len 字节；成功返回 1 并写入 out_nread（可为 0 表示 EOF）；失败返回 0。
static int scoop_platform_io_read_stdin(uint8_t *buf, size_t len, size_t *out_nread);

// dynlib: 在“进程默认符号表（RTLD_DEFAULT 类语义）”中查找符号地址。
// - 成功返回非 NULL 指针；
// - 不支持或未找到返回 NULL；
// - 该接口用于 runtime 内部的动态链接场景（例如 once guard canonicalize）。
static void *scoop_platform_dynlib_lookup_symbol_default(const char *symbol_name_cstr);

// sync: mutex/condvar。
static int scoop_platform_sync_mutex_init(ScoopPlatformMutex *mutex);
static void scoop_platform_sync_mutex_lock(ScoopPlatformMutex *mutex);
static void scoop_platform_sync_mutex_unlock(ScoopPlatformMutex *mutex);
static void scoop_platform_sync_mutex_destroy(ScoopPlatformMutex *mutex);

static int scoop_platform_sync_condvar_init(ScoopPlatformCondVar *condvar);
static void scoop_platform_sync_condvar_wait(ScoopPlatformCondVar *condvar, ScoopPlatformMutex *mutex);
static void scoop_platform_sync_condvar_signal(ScoopPlatformCondVar *condvar);
static void scoop_platform_sync_condvar_broadcast(ScoopPlatformCondVar *condvar);
static void scoop_platform_sync_condvar_destroy(ScoopPlatformCondVar *condvar);

// thread: self/equal（用于 once 重入检测等内部逻辑）。
static ScoopPlatformThread scoop_platform_thread_self(void);
static int scoop_platform_thread_equal(ScoopPlatformThread a, ScoopPlatformThread b);

// thread: spawn/join/yield/sleep/currentId（用于 `scoop.thread` 等 runtime 内部实现）。
typedef void *(*ScoopPlatformThreadEntryFn)(void *arg);
static int scoop_platform_thread_spawn(ScoopPlatformThread *out_thread,
                                      ScoopPlatformThreadEntryFn entry,
                                      void *arg);
static int scoop_platform_thread_join(ScoopPlatformThread thread);
static void scoop_platform_thread_yield(void);
static void scoop_platform_thread_sleep_millis(int64_t ms);
static int64_t scoop_platform_thread_current_id(void);

// --- backend selection ---
//
// 注意：这里通过 include 选择 backend，并在 backend 文件中提供上述 static 函数定义。
// 这样可避免新增任何全局导出符号，从而与 runtime ABI allowlist 检查（T1401）保持兼容。
#if defined(_WIN32)
#include "platform_win32.c"
#else
#include "platform_posix.c"
#endif
