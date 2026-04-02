// Scoop C runtime platform layer (v0).
//
// 目标（TODO T1402）：
// - 把 runtime 内部对 OS API 的直接调用收敛到 platform/backends；
// - core/runtime 代码只调用 platform API；
// - 本文件中的平台函数必须保持“内部链接”（static），避免污染 runtime ABI 导出符号集合。
//
// 说明：
// - 目前只覆盖 env/time/io 三条路径（对齐 T1318a/T1318e 的最低需求）。
// - 未来会把 thread/sync/channels/task/net 等扩展到 platform 层（TODO T1403）。

#pragma once

#include <stddef.h>
#include <stdint.h>

// --- platform API（internal linkage）---

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

// --- backend selection ---
//
// 注意：这里通过 include 选择 backend，并在 backend 文件中提供上述 static 函数定义。
// 这样可避免新增任何全局导出符号，从而与 runtime ABI allowlist 检查（T1401）保持兼容。
#if defined(_WIN32)
#include "platform_win32.c"
#else
#include "platform_posix.c"
#endif

