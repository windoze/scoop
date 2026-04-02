# sysroot/std 平台 API surface 清单（T1404）

> 目的：把 Scoop “平台能力”在 **sysroot/stdlib（Scoop 侧）** 与 **runtime/c（C 侧）** 之间的边界固化为一份可审计清单，确保：
>
> - Scoop 侧不暴露 `errno/FILE*/HANDLE/pthread_t` 等 OS 概念与平台差异；
> - 平台差异只存在于 `runtime/c/platform/*` backends；
> - 平台“不支持”的语义以 **capability gating** + **`Option/Result/Int` 形状**表达，避免 ad-hoc `cfg(target_*)` 分叉。

相关文件（source of truth）：

- Scoop 声明面（sysroot）：`sysroot/*.scoop`
- runtime ABI allowlist（对外导出符号）：`runtime/c/scoop_runtime_api.h`
- runtime 平台层（内部 static API，隔离 OS 调用）：`runtime/c/platform/platform.h`

> 备注：本文只覆盖 “平台 API” 模块（env/time/fs/path/io/process/thread/sync/channels/net）。其它 sysroot（core/unsafe/task/delegates/collections…）不在本文范围内。

---

## 1. 总览表（模块 → sysroot → runtime）

| 模块 | sysroot 声明 | runtime ABI 符号（C） | 平台 backend 支持现状（v0） |
|---|---|---|---|
| `scoop.env` | `sysroot/env.scoop` | `scoop_env_get` | POSIX ✅；Windows（`platform_win32.c`）占位：恒 `None` |
| `scoop.time` | `sysroot/time.scoop` | `scoop_time_now_unix_millis` | POSIX ✅；Windows 占位：恒返回 0 |
| `scoop.io` | `sysroot/io.scoop` | `scoop_io_stdout_write_string` / `scoop_io_stdout_write_line` / `scoop_io_stderr_write_string` / `scoop_io_stderr_write_line` / `scoop_io_stdin_read_line_utf8` | POSIX ✅；Windows 占位：写入失败（无输出）/ 读取失败（`None`） |
| `scoop.fs` | `sysroot/fs.scoop` | `scoop_fs_read_all_text_utf8` / `scoop_fs_write_all_text_utf8` | 依赖 C stdio（`fopen/fread/fwrite`）：host 平台预期可用；WASM/embedded 等需后续 capability gating |
| `scoop.path` | `sysroot/path.scoop` | `scoop_path_normalize` / `scoop_path_join` / `scoop_path_basename` / `scoop_path_dirname` | 纯字符串处理：host 平台可用（分隔符策略以 runtime 约定为准） |
| `scoop.process` | `sysroot/process.scoop` | `scoop_process_exit` / `scoop_process_args_array`（初始化：`scoop_process_init`） | host 平台 ✅（依赖 libc/CRT） |
| `scoop.thread` | `sysroot/thread.scoop` | `scoop_thread_spawn` / `scoop_thread_join` / `scoop_thread_yield` / `scoop_thread_sleep_millis` / `scoop_thread_current_id` | POSIX ✅；Windows 占位：语义未实现（留待后续） |
| `scoop.sync` | `sysroot/sync.scoop` | `scoop_sync_mutex_*` / `scoop_sync_condvar_*` / `scoop_sync_once_*` | POSIX ✅；Windows 占位：语义未实现（留待后续） |
| `scoop.channels` | `sysroot/channels.scoop` | `scoop_channels_channel_create` / `scoop_channels_send_u64` / `scoop_channels_recv_u64` / `scoop_channels_close` | POSIX ✅；Windows 占位：语义未实现（留待后续） |
| `scoop.net` | `sysroot/net.scoop` | （暂无；未进入 runtime ABI allowlist） | 未实现：仅固定 API 形状（typecheck 级别）；需后续 backends + capability gating |

---

## 2. 逐模块明细（API surface ↔ runtime 符号 ↔ 不支持语义）

### 2.1 `scoop.env`（环境变量）

sysroot：`sysroot/env.scoop`

- Scoop API：
  - `fun getOrNull(key: String): String?`
- runtime 符号：
  - `scoop_env_get(key: *ScoopString) -> *ScoopString`（返回 NULL 表示 `None`）
- 平台差异隔离点：
  - runtime 通过 `scoop_platform_env_getenv`（`platform.h`）调用后端；
  - POSIX backend 使用 `getenv`；Windows backend 目前恒失败（返回 NULL）。

### 2.2 `scoop.time`（时间）

sysroot：`sysroot/time.scoop`

- Scoop API：
  - `fun nowUnixMillis(): Int`
- runtime 符号：
  - `scoop_time_now_unix_millis() -> i64`（失败返回 0）
- 平台差异隔离点：
  - runtime 通过 `scoop_platform_time_now_unix_millis` 调用后端；
  - POSIX backend 使用 `gettimeofday`；Windows backend 占位返回失败。

### 2.3 `scoop.io`（stdin/stdout/stderr）

sysroot：`sysroot/io.scoop`

- Scoop API：
  - `fun stdoutWriteString(value: String): Unit`
  - `fun stdoutWriteLine(value: String): Unit`
  - `fun stderrWriteString(value: String): Unit`
  - `fun stderrWriteLine(value: String): Unit`
  - `fun stdinReadLine(): String?`
- runtime 符号：
  - `scoop_io_stdout_write_string`
  - `scoop_io_stdout_write_line`
  - `scoop_io_stderr_write_string`
  - `scoop_io_stderr_write_line`
  - `scoop_io_stdin_read_line_utf8`（NULL 表示 `None`）
- 平台差异隔离点：
  - runtime 通过 `scoop_platform_io_*` 写入/读取；
  - POSIX backend 使用 `read/write`；Windows backend 占位失败。
- 相关（非 `scoop.io` 模块，但同属 I/O 表面）：
  - `scoop.core.print/println` 对应 runtime `scoop_print/scoop_println`（同样不暴露 `FILE*` 等 OS 概念）。

### 2.4 `scoop.fs`（文件）

sysroot：`sysroot/fs.scoop`

- Scoop API：
  - `fun readAllText(path: String): String?`
  - `fun writeAllText(path: String, content: String): Int`（0 表示成功）
- runtime 符号：
  - `scoop_fs_read_all_text_utf8`（NULL 表示 `None`）
  - `scoop_fs_write_all_text_utf8`（0 表示成功；非 0 失败）
- 平台差异与“不支持”语义：
  - 当前实现依赖 C stdio（`fopen/fread/fwrite`），属于 host/desktop 路线；
  - 对 WASM/embedded 等环境：应通过后续 platform/backends 或 capability gating 给出稳定“不支持”行为（例如统一返回 `None`/非 0）。

### 2.5 `scoop.path`（路径字符串处理）

sysroot：`sysroot/path.scoop`

- Scoop API：
  - `fun normalize(path: String): String`
  - `fun join(base: String, child: String): String`
  - `fun basename(path: String): String`
  - `fun dirname(path: String): String`
- runtime 符号：
  - `scoop_path_normalize`
  - `scoop_path_join`
  - `scoop_path_basename`
  - `scoop_path_dirname`
- 平台差异隔离点：
  - 目前以 runtime 约定的分隔符策略为准（host 优先）。

### 2.6 `scoop.process`（进程）

sysroot：`sysroot/process.scoop`

- Scoop API：
  - `fun exit(code: Int): Unit`
  - `fun args(): Array<String>`（不含 argv[0]）
- runtime 符号：
  - `scoop_process_exit`
  - `scoop_process_args_array`（配合 `scoop_process_init` 在 runtime init 时捕获 argv）
- 平台差异隔离点：
  - 早期实现依赖 host libc/CRT；不向 Scoop 暴露 `argv` 指针/宽字符等 OS 细节。

### 2.7 `scoop.thread`（线程）

sysroot：`sysroot/thread.scoop`

- Scoop API：
  - `class Thread`（opaque handle）
  - `fun threadSpawn(block: () -> Unit): Thread`
  - `fun Thread.join(): Unit`
  - `fun yield(): Unit`
  - `fun sleepMillis(ms: Int): Unit`
  - `fun currentId(): Int`（0 表示不支持/失败）
- runtime 符号：
  - `scoop_thread_spawn`
  - `scoop_thread_join`
  - `scoop_thread_yield`
  - `scoop_thread_sleep_millis`
  - `scoop_thread_current_id`
- 平台差异隔离点：
  - runtime 内部通过 `scoop_platform_thread_*`（`platform.h`）对接后端；
  - POSIX backend 使用 pthread；Windows backend 目前为占位。

### 2.8 `scoop.sync`（同步原语）

sysroot：`sysroot/sync.scoop`

- Scoop API：
  - `class Mutex` / `class CondVar` / `class Once`（均为 opaque handle）
  - `mutexCreate/lock/unlock/destroy`
  - `condVarCreate/wait/notifyOne/notifyAll/destroy`
  - `onceCreate/isDone/run`
- runtime 符号：
  - `scoop_sync_mutex_create/lock/unlock/destroy`
  - `scoop_sync_condvar_create/wait/notify_one/notify_all/destroy`
  - `scoop_sync_once_create/is_done/run`
- 平台差异隔离点：
  - runtime 内部通过 `scoop_platform_sync_*` + `scoop_platform_thread_self/equal`（`platform.h`）对接后端；
  - POSIX backend 使用 pthread；Windows backend 目前为占位。

### 2.9 `scoop.channels`（channel）

sysroot：`sysroot/channels.scoop`

- Scoop API：
  - `class Channel<T>`（opaque handle）
  - `fun <T> channelCreate(): Channel<T>`
  - `fun <T> Channel<T>.send(value: T): Bool`
  - `fun <T> Channel<T>.recv(): T?`
  - `fun <T> Channel<T>.close(): Unit`
- runtime 符号（v0：以 word/u64 承载 payload，配合 monomorphization）：
  - `scoop_channels_channel_create`
  - `scoop_channels_send_u64`
  - `scoop_channels_recv_u64`
  - `scoop_channels_close`
- 平台差异隔离点：
  - runtime 内部使用 `scoop_platform_sync_*` 对接锁/条件变量；
  - POSIX backend ✅；Windows backend 目前为占位。

### 2.10 `scoop.net`（网络）

sysroot：`sysroot/net.scoop`

- Scoop API（仅固定声明面 + 失败语义；尚未落地）：
  - `class TcpStream` / `class TcpListener`（opaque handle）
  - `fun tcpIsSupported(): Bool`
  - `fun tcpConnect(host: String, port: Int): TcpStream?`
  - `fun tcpListen(host: String, port: Int): TcpListener?`
  - `fun TcpListener.accept(): TcpStream?`
  - `fun TcpListener.close(): Unit`
  - `fun TcpStream.writeUtf8(text: String): Int`
  - `fun TcpStream.readUtf8(maxBytes: Int): String?`
  - `fun TcpStream.close(): Unit`
- runtime 符号：
  - 当前无（未加入 `runtime/c/scoop_runtime_api.h` allowlist）。
- 后续落点建议：
  - 先实现 `tcpIsSupported` 的 capability gating（target/platform/backends 统一策略）；
  - 再按 “runtime backends 隔离 OS 差异” 的路线逐步实现 socket/DNS。

