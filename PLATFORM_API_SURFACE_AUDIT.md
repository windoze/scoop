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

> 备注：本文只覆盖“当前仍保留并承诺维护”的平台相关 sysroot 模块。自 2026-04-26 的 `T5000e3b` 起，早期试验性的 `scoop.env`、`scoop.time`、`scoop.io`、`scoop.fs`、`scoop.path`、`scoop.channels`、`scoop.net` 已从 sysroot 移除并等待重设计；本文不再把它们列为现行 surface，以免误导为“仍受支持但实现不完整”。其它 sysroot（core/unsafe/task/delegates/collections…）不在本文范围内。

---

## 1. 总览表（模块 → sysroot → runtime）

| 模块 | sysroot 声明 | runtime ABI 符号（C） | 平台 backend 支持现状（v0） |
|---|---|---|---|
| `scoop.process` | `sysroot/process.scoop` | `scoop_process_exit` / `scoop_process_args_array`（初始化：`scoop_process_init`） | host 平台 ✅（依赖 libc/CRT） |
| `scoop.thread` | `sysroot/thread.scoop` | `scoop_thread_spawn` / `scoop_thread_join` / `scoop_thread_yield` / `scoop_thread_sleep_millis` / `scoop_thread_current_id` | POSIX ✅；Windows 占位：语义未实现（留待后续） |
| `scoop.sync` | `sysroot/sync.scoop` | `scoop_sync_mutex_*` / `scoop_sync_condvar_*` / `scoop_sync_once_*` | POSIX ✅；Windows 占位：语义未实现（留待后续） |

---

## 2. 逐模块明细（API surface ↔ runtime 符号 ↔ 不支持语义）

### 2.1 `scoop.process`（进程，过渡 surface）

sysroot：`sysroot/process.scoop`

- Scoop API：
  - `fun exit(code: Int): Unit`
  - `fun args(): Array<String>`（不含 argv[0]）
- runtime 符号：
  - `scoop_process_exit`
  - `scoop_process_args_array`（配合 `scoop_process_init` 在 runtime init 时捕获 argv）
- 平台差异隔离点：
  - 早期实现依赖 host libc/CRT；不向 Scoop 暴露 `argv` 指针/宽字符等 OS 细节。
- 维护说明：
  - 这是当前仍保留的临时程序边界 surface；
  - 下一步 `T5000e3c` 会把 argv 直接并入扩展后的 `main` 程序边界，并连同 `process.scoop` 一起删除；届时仅允许 `fun main(): Unit / Pure!`、`fun main(): Int / Pure!`、`fun main(args: Array<String>): Unit / Pure!`、`fun main(args: Array<String>): Int / Pure!` 四种形状，其中 `Unit` 正常返回默认退出码为 `0`，`Int` 正常返回值直接作为退出码。

### 2.2 `scoop.thread`（线程）

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

### 2.3 `scoop.sync`（同步原语）

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
