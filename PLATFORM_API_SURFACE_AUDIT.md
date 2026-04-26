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

> 备注：本文只覆盖“当前仍保留并承诺维护”的平台相关 sysroot 模块。自 2026-04-26 的 `T5000e3b` / `T5000e3c` 起，早期试验性的 `scoop.env`、`scoop.time`、`scoop.io`、`scoop.fs`、`scoop.path`、`scoop.channels`、`scoop.net` 与过渡期 `scoop.process` 已从 sysroot 移除并等待重设计；本文不再把它们列为现行 surface，以免误导为“仍受支持但实现不完整”。其它 sysroot（core/unsafe/task/delegates/collections…）不在本文范围内。

---

## 1. 总览表（模块 → sysroot → runtime）

| 模块 | sysroot 声明 | runtime ABI 符号（C） | 平台 backend 支持现状（v0） |
|---|---|---|---|
| `scoop.thread` | `sysroot/thread.scoop` | `scoop_thread_spawn` / `scoop_thread_join` / `scoop_thread_yield` / `scoop_thread_sleep_millis` / `scoop_thread_current_id` | POSIX ✅；Windows 占位：语义未实现（留待后续） |
| `scoop.sync` | `sysroot/sync.scoop` | `scoop_sync_mutex_*` / `scoop_sync_condvar_*` / `scoop_sync_once_*` | POSIX ✅；Windows 占位：语义未实现（留待后续） |

补充说明（非 sysroot surface）：

- 可执行入口的 argv/退出码 contract 已在 `T5000e3c` 并入程序边界 `main`，不再通过 `scoop.process` 暴露。
- 当前工具链只接受四种 executable `main` 形状：`fun main(): Unit / Pure!`、`fun main(): Int / Pure!`、`fun main(args: Array<String>): Unit / Pure!`、`fun main(args: Array<String>): Int / Pure!`。
- 对 `main(args)`，runtime 会通过内部 helper `scoop_entry_argv_array` 把完整 native argv（含 `argv[0]`）直接传入；这是程序边界 contract，不是稳定的 sysroot 模块 API。

---

## 2. 逐模块明细（API surface ↔ runtime 符号 ↔ 不支持语义）

### 2.1 `scoop.thread`（线程）

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

### 2.2 `scoop.sync`（同步原语）

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
