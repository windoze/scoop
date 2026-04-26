# Scoop stdlib 完整性清单（T1801）

> 生成时间：2026-04-09
> 基于：`KOTLIN_RUNTIME_GAP_AUDIT.md`（T1314）的能力矩阵
> 目的：以"能力项"为粒度，对照当前仓库（sysroot/stdlib/runtime/c）的实现覆盖度，产出 DONE/TODO/Blockers 清单，并为 T1802（拆分任务）提供输入。
> 状态修订：2026-04-26 已按 `T5000e3b` / `T5000e3bR` / `T5000e3c` 回写平台与程序边界收口结果；`scoop.process`、`scoop.env`、`scoop.time`、`scoop.io`、`scoop.fs`、`scoop.path`、`scoop.channels`、`scoop.net` 与 `stdlib/test.scoop` 已从当前 shipped sysroot 移除，等待重设计后再重新进入清单。可执行 argv/退出码 contract 已并入程序边界 `main`，不再作为独立 sysroot surface 维护。

## 约定

- **状态**：
  - `DONE`：声明 + 实现 + 至少 1 个 run-pass fixture
  - `PARTIAL`：有声明/实现但覆盖不完整（缺泛型版本、缺 fixture、或为 stub）
  - `DECL-ONLY`：sysroot 有声明但无可执行实现或无 fixture
  - `TODO`：尚未提供任何落点
- **分类**（沿用 `KOTLIN_RUNTIME_GAP_AUDIT.md` §1.2）：
  - `pure_scoop_ok`：可用纯 Scoop 实现（依赖已有 sysroot/runtime）
  - `needs_runtime_lib`：需要 runtime/平台库支持
  - `needs_new_intrinsic`：需要新 intrinsic（当前结论：空集，见 `RUNTIME_STDLIB_INTRINSIC_AUDIT.md`）
- **优先级**（沿用审计文档）：P0 > P1 > P2

---

## 1. Core types & primitives

| 能力项 | 优先级 | 分类 | 状态 | 实现位置 | Fixtures | 备注 |
|---|:---:|:---:|:---:|---|---|---|
| `Any` / `Unit` / `Nothing` | P0 | pure_scoop_ok | DONE | `sysroot/core.scoop` | 多个 run-pass | `Nothing` 已有 `CgTy::Never` codegen（T1612） |
| `Bool` / `Int` / `String` | P0 | pure_scoop_ok | DONE | `sysroot/core.scoop` | 多个 run-pass | 编译器内建布局 |
| Fixed-width integers (`Int8`..`UInt64`) | P1 | pure_scoop_ok | DONE | `sysroot/core.scoop` | 部分 run-pass | 声明完整；codegen 已支持 |
| `Option<T>` (nullable `T?`) | P0 | pure_scoop_ok | DONE | `sysroot/core.scoop` | 多个 run-pass | `Some`/`None` + pattern match |
| `RuntimeError` + `Raise<E>` | P0 | pure_scoop_ok | DONE | `sysroot/core.scoop` | `try_catch_raise_runtime_error_basic` 等 | 效果系统核心 |
| `Continuation<T>` | P0 | pure_scoop_ok | DONE | `sysroot/core.scoop` | 多个 T17xx fixtures | 多 perform/GC/多线程已验证 |
| `Task<T>` / `TaskStep<T>` / `Async` effect | P1 | pure_scoop_ok | DONE | `sysroot/core.scoop` + `sysroot/task.scoop` + generic continuation/atomic/thread runtime | `async_await_minimal_int_basic`、`async_await_string_basic`、`task_step_manual_basic` | 公开 surface 已收口为 `Task<T>` / `TaskStep<T>` / `step()` / `Async.await`；内部为 ordinary Scoop state + atomic claim-bit 的 single-driver 合同；task-only C ABI 已删除 |
| `Platform` / `getPlatform()` | P2 | pure_scoop_ok | DONE | `sysroot/core.scoop` | comptime fixtures | intrinsic |
| Compile-time metadata (`TypeMeta` 等) | P2 | pure_scoop_ok | DONE | `sysroot/core.scoop` | 多个 comptime fixtures | `fieldsOf`/`variantsOf` 等 intrinsic |
| Annotations (`@TailRec` 等) | P2 | pure_scoop_ok | DONE | `sysroot/core.scoop` | typecheck fixtures | 内建注解集合 |

---

## 2. Properties / Delegates

| 能力项 | 优先级 | 分类 | 状态 | 实现位置 | Fixtures | 备注 |
|---|:---:|:---:|:---:|---|---|---|
| `lazy` (thread-safe modes) | P0 | pure_scoop_ok | DONE | `sysroot/delegates.scoop` + 编译器 lowering | `delegated_property_lazy_*`（5 个） | 3 种 thread-safety mode |
| `observable` / `vetoable` | P0 | pure_scoop_ok | DONE | `sysroot/delegates.scoop` + 编译器 lowering | `delegated_property_observable_vetoable_*`（3 个） | 并发安全（per-property Mutex） |
| `map`-backed delegate | P1 | pure_scoop_ok | DONE | `sysroot/delegates.scoop` + `sysroot/collections.scoop` | `delegated_property_map_backed_basic` | 最小落点 |
| `ReadOnlyProperty` / `ReadWriteProperty` | P1 | pure_scoop_ok | DONE | `sysroot/delegates.scoop` | typecheck fixtures | 接口声明 |

---

## 3. Collections

| 能力项 | 优先级 | 分类 | 状态 | 实现位置 | Fixtures | 备注/缺口 |
|---|:---:|:---:|:---:|---|---|---|
| `Array<T>` — `size`/`get` | P0 | needs_runtime_lib | DONE | `sysroot/core.scoop` + `runtime/c/scoop_array.c` | `array_mutable_array_min_primitive_basic` 等 | 只读数组 |
| `MutableArray<T>` — `size`/`get`/`set` | P0 | needs_runtime_lib | DONE | `sysroot/core.scoop` + `runtime/c/scoop_array.c` | `mutable_array_ops_basic` 等 | 可变数组 |
| `MutableArray<Int>` — `push`/`pop`/`insert`/`removeAt`/`splice` | P0 | pure_scoop_ok | DONE | `stdlib/mutable_array.scoop` | `mutable_array_ops_basic` | **仅 Int**；泛型版本缺失 |
| `MutableArray<Int>.toArray()` | P1 | pure_scoop_ok | DONE | `stdlib/mutable_array.scoop` | `mutable_array_ops_basic` | 仅 Int |
| `List<T>` / `MutableList<T>` typealias | P0 | pure_scoop_ok | DONE | `sysroot/core.scoop` | `list_and_mutable_list_basic` | 别名到 Array/MutableArray |
| `MutableList<Int>.add()` | P0 | pure_scoop_ok | DONE | `stdlib/mutable_list.scoop` | `list_and_mutable_list_basic` | 仅 Int |
| `Array<Int>.forEach/map/filter/fold` | P0 | pure_scoop_ok | DONE | `stdlib/array_iter.scoop` | `stdlib_iter_algorithms_basic` | 仅 Int；effect-polymorphic |
| `MutableArray<Int>.forEach/map/filter/fold` | P0 | pure_scoop_ok | DONE | `stdlib/mutable_array_iter.scoop` | `stdlib_iter_algorithms_basic` | 仅 Int；effect-polymorphic |
| `Set` / `MutableSet`（Int-only） | P1 | pure_scoop_ok | DONE | `stdlib/collections_set.scoop` | `stdlib_set_map_basic`, `stdlib_hash_set_map_basic` | typealias surface 保留；`MutableSet` 内部为开放寻址哈希表，`Set` 导出为顺序视图 |
| `Map` / `MutableMap`（Int→Int only） | P1 | pure_scoop_ok | DONE | `stdlib/collections_map.scoop` | `stdlib_set_map_basic`, `stdlib_hash_set_map_basic` | typealias surface 保留；`MutableMap` 内部为开放寻址哈希表，`MapView` 导出为 flat kv 顺序视图 |
| `Iterator<T>` / `Iterable<T>` protocol | P0 | pure_scoop_ok | PARTIAL | `sysroot/collections.scoop` | — | 声明完整；`for-in` lowering 尚未全面打通（T1508） |
| `IntIterable.toArray()` | P1 | pure_scoop_ok | PARTIAL | `stdlib/collections_iter.scoop` | — | **stub**：返回空数组；依赖 member call 链路 |
| `sort` / `reduce` / `zip` / `flatten` / `chunked` / `windowed` | P1 | pure_scoop_ok | TODO | — | — | 纯 Scoop 可实现；依赖泛型或至少 Int 专用落点 |
| `Sequence<T>` / lazy adapters | P1 | pure_scoop_ok | TODO | — | — | 可结合代数效果设计 |
| 泛型版本 collections API（`<T>` 版 forEach/map 等） | P0 | pure_scoop_ok | TODO | — | — | 依赖泛型单态化/跨文件 codegen 完善 |

### 3.1 Collections 缺口总结

1. **所有集合操作仅 `Int` 专用** — 泛型版本依赖 codegen/monomorph 更完善后补齐
2. **Set/Map 已迁移到基于 `Hashable.hash` 的开放寻址实现** — 仍仅 Int/Int→Int 专用，泛型版本后置
3. **Iterator/Iterable 协议声明存在但 for-in 链路未全面打通** — `IntIterable.toArray()` 为 stub
4. **缺少 sort/reduce/zip/flatten/chunked/windowed/Sequence** — 纯 Scoop 可实现

---

## 4. Ranges / Progressions

| 能力项 | 优先级 | 分类 | 状态 | 实现位置 | Fixtures | 备注/缺口 |
|---|:---:|:---:|:---:|---|---|---|
| `IntProgression` struct | P0 | pure_scoop_ok | DONE | `sysroot/core.scoop` | `kotlin_ranges_progressions_basic` | `first/last/step/increasing` |
| `Int.rangeTo(endInclusive, step)` | P0 | pure_scoop_ok | DONE | `stdlib/prelude.scoop` | `kotlin_ranges_progressions_basic`, `stdlib_ranges_enhanced_basic` | step 显式传入 |
| `Int.downTo(endInclusive, step)` | P0 | pure_scoop_ok | DONE | `stdlib/prelude.scoop` | `kotlin_ranges_progressions_basic` | step 显式传入 |
| `Int.until(endExclusive)` | P1 | pure_scoop_ok | DONE | `stdlib/prelude.scoop` | `stdlib_ranges_enhanced_basic` | exclusive end；默认 step 由 helper 派生 |
| `IntProgression.forEach(action)` | P0 | pure_scoop_ok | DONE | `stdlib/prelude.scoop` | `kotlin_ranges_progressions_basic`, `stdlib_ranges_enhanced_basic` | effect-polymorphic |
| `..` operator syntax sugar | P1 | pure_scoop_ok | DONE | `parser/expr.rs`, `typecheck/expr/ops.rs`, `hir/lower/expr.rs` | `stdlib_ranges_enhanced_basic` | lowering 为现有 `rangeTo(start, end, step)` 调用 |
| `for (x in range)` direct integration | P1 | pure_scoop_ok | DONE | `typecheck/expr/stmt.rs`, `hir/lower/stmt.rs` | `for_in_int_progression_basic`, `stdlib_ranges_enhanced_basic` | `IntProgression` 直接走专用 lowering |

---

## 5. Text (String)

| 能力项 | 优先级 | 分类 | 状态 | 实现位置 | Fixtures | 备注/缺口 |
|---|:---:|:---:|:---:|---|---|---|
| String 类型（内建） | P0 | needs_runtime_lib | DONE | `sysroot/core.scoop` + `runtime/c` | 多个 run-pass | UTF-8 编码；编译器管理布局 |
| `print`/`println`（String/Int） | P0 | needs_runtime_lib | DONE | `sysroot/core.scoop` + `runtime/c` | 多个 run-pass | |
| String concatenation（`+`） | P0 | needs_runtime_lib | DONE | 编译器内建 | 多个 run-pass | |
| String equality（`==`） | P0 | needs_runtime_lib | DONE | 编译器内建 | 多个 run-pass | |
| `String.length` / `String.size()` | P0 | needs_runtime_lib | TODO | — | — | 需 runtime 提供 UTF-8 长度 |
| `substring(start, end)` | P0 | needs_runtime_lib | TODO | — | — | 需 runtime 切片支持 |
| `startsWith` / `endsWith` | P0 | needs_runtime_lib | TODO | — | — | 可在 runtime 中实现 |
| `indexOf` / `contains` | P0 | needs_runtime_lib | TODO | — | — | 字符串搜索 |
| `split(delimiter)` | P0 | needs_runtime_lib | TODO | — | — | 返回 Array<String> |
| `trim` / `trimStart` / `trimEnd` | P1 | needs_runtime_lib | TODO | — | — | |
| `toUpperCase` / `toLowerCase` | P2 | needs_runtime_lib | TODO | — | — | Unicode 依赖较重 |
| `charAt(index)` / indexing | P1 | needs_runtime_lib | TODO | — | — | 需明确 byte vs scalar index |
| `trimIndent` | P1 | pure_scoop_ok | PARTIAL | 编译器内建 | `string_trim_indent_basic` | 字面量级别的 trimIndent |
| `String.toInt()` / `Int.toString()` | P0 | needs_runtime_lib | TODO | — | — | 数值↔文本转换 |

### 5.1 Text 缺口总结

**Text 是当前最大的 P0 缺口**。除内建的 `+`/`==`/`print` 外，几乎所有字符串操作（长度、子串、搜索、分割）都缺失。这直接限制了 fixtures 的可写性和语言可用性。优先补齐路径：runtime/c 提供底层 API → sysroot 声明 → stdlib 封装 → fixtures。

---

## 6. Text formatting

| 能力项 | 优先级 | 分类 | 状态 | 实现位置 | Fixtures | 备注/缺口 |
|---|:---:|:---:|:---:|---|---|---|
| `StringBuilder` | P1 | pure_scoop_ok / needs_runtime_lib | TODO | — | — | 可先用可变字节数组；后续 runtime 高效拼接 |
| `joinToString` | P1 | pure_scoop_ok | TODO | — | — | 依赖 collections + String 操作 |
| String interpolation (`"$var"` / `"${expr}"`) | P1 | pure_scoop_ok | TODO | — | — | 需前端语法糖 + toString 支持 |

---

## 7. Math

| 能力项 | 优先级 | 分类 | 状态 | 实现位置 | Fixtures | 备注/缺口 |
|---|:---:|:---:|:---:|---|---|---|
| `abs` / `min` / `max` (Int) | P1 | pure_scoop_ok | TODO | — | — | 纯 Scoop 可实现 |
| Trigonometric / `sqrt` / `pow` | P2 | needs_runtime_lib | TODO | — | — | 需链接 libm |
| Floating-point type (`Float`/`Double`) | P2 | needs_runtime_lib | TODO | — | — | 需 codegen 支持浮点 |

---

## 8. Hashing

| 能力项 | 优先级 | 分类 | 状态 | 实现位置 | Fixtures | 备注/缺口 |
|---|:---:|:---:|:---:|---|---|---|
| `Hashable` interface | P1 | pure_scoop_ok | PARTIAL | `sysroot/core.scoop` | — | 声明存在；默认 `hash() -> 0`（占位） |
| Int/String hash 实现 | P1 | pure_scoop_ok / needs_runtime_lib | DONE | `crates/scoopc/src/llvm/codegen/mod.rs`, `runtime/c/scoop_runtime.c` | `stdlib_hash_basic` | Int 走 inline bit-mixing；String 走 runtime FNV-1a |
| Hash-based Set/Map | P1 | pure_scoop_ok | DONE | `stdlib/collections_set.scoop`, `stdlib/collections_map.scoop` | `stdlib_hash_set_map_basic`, `stdlib_set_map_basic` | 开放寻址 + linear probing；保留 typealias surface 与只读导出视图 |

---

## 9. Random

| 能力项 | 优先级 | 分类 | 状态 | 实现位置 | Fixtures | 备注/缺口 |
|---|:---:|:---:|:---:|---|---|---|
| PRNG（xorshift/PCG） | P1 | pure_scoop_ok | TODO | — | — | 纯 Scoop 算法 |
| `Random` class / `nextInt` | P1 | pure_scoop_ok | TODO | — | — | 上层 API |
| Default seed（时间/熵） | P1 | needs_runtime_lib | TODO | — | — | 依赖 time 或 OS 熵源 |

---

## 10. Time

| 能力项 | 优先级 | 分类 | 状态 | 实现位置 | Fixtures | 备注/缺口 |
|---|:---:|:---:|:---:|---|---|---|
| `nowUnixMillis()` | P1 | needs_runtime_lib | TODO | — | — | 旧 `sysroot/time.scoop` 已在 `T5000e3b` 移除；需按新 platform/time 设计重做 |
| `Duration` 值类型 | P1 | pure_scoop_ok | TODO | — | — | 可在 stdlib 实现 |
| `Instant` / monotonic clock | P2 | needs_runtime_lib | TODO | — | — | 需先重建 time surface，再补 runtime 平台 API |

---

## 11. IO (stdin/stdout/stderr)

| 能力项 | 优先级 | 分类 | 状态 | 实现位置 | Fixtures | 备注/缺口 |
|---|:---:|:---:|:---:|---|---|---|
| `stdoutWriteString` / `stdoutWriteLine` | P0 | needs_runtime_lib | TODO | — | — | 旧 `sysroot/io.scoop` 已在 `T5000e3b` 移除；当前仅保留 `scoop.core.print/println` 最小可观察输出 |
| `stderrWriteString` / `stderrWriteLine` | P1 | needs_runtime_lib | TODO | — | — | 同上，需按新 I/O surface 重做 |
| `stdinReadLine()` | P1 | needs_runtime_lib | TODO | — | — | 同上，旧 fixture 已删除 |
| Buffered / binary IO | P2 | needs_runtime_lib | TODO | — | — | 后续扩展 |

---

## 12. File system

| 能力项 | 优先级 | 分类 | 状态 | 实现位置 | Fixtures | 备注/缺口 |
|---|:---:|:---:|:---:|---|---|---|
| `readAllText` / `writeAllText` | P1 | needs_runtime_lib | TODO | — | — | 旧 `sysroot/fs.scoop` 已在 `T5000e3b` 移除；需按新 filesystem surface 重做 |
| Directory operations (mkdir/readdir) | P2 | needs_runtime_lib | TODO | — | — | |
| Binary file read/write | P2 | needs_runtime_lib | TODO | — | — | |

---

## 13. Program Boundary / Env / Path

| 能力项 | 优先级 | 分类 | 状态 | 实现位置 | Fixtures | 备注/缺口 |
|---|:---:|:---:|:---:|---|---|---|
| Executable `main` argv / exit-code contract | P1 | needs_runtime_lib | DONE | 程序边界 `main` + `runtime/c` | `std_process_args_exit_basic`、`entry_main_args_int_exit_basic` | `T5000e3c` 已移除 `scoop.process`；当前只接受 `main(): Unit/Int / Pure!` 与 `main(args: Array<String>): Unit/Int / Pure!` 四种形状。`args` 直接承载完整 native argv（含 `argv[0]`）；正常返回 `Unit` 映射退出码 `0`，正常返回 `Int` 映射为进程退出码 |
| `env.getOrNull(key)` | P1 | needs_runtime_lib | TODO | — | — | 旧 `sysroot/env.scoop` 已在 `T5000e3b` 移除；需按新 env surface 重做 |
| `path.normalize/join/basename/dirname` | P1 | needs_runtime_lib | TODO | — | — | 旧 `sysroot/path.scoop` 已在 `T5000e3b` 移除；需按新 path surface 重做 |
| Subprocess / exec | P2 | needs_runtime_lib | TODO | — | — | |

---

## 14. Concurrency / Threading

| 能力项 | 优先级 | 分类 | 状态 | 实现位置 | Fixtures | 备注/缺口 |
|---|:---:|:---:|:---:|---|---|---|
| `threadSpawn` / `Thread.join` | P1 | needs_runtime_lib | DONE | `sysroot/thread.scoop` + `runtime/c` | `std_thread_basic` + 多个跨线程 fixtures | |
| `yield` / `sleepMillis` / `currentId` | P1 | needs_runtime_lib | DONE | `sysroot/thread.scoop` + `runtime/c` | `std_thread_basic` | |
| `Mutex` (create/lock/unlock/destroy) | P1 | needs_runtime_lib | DONE | `sysroot/sync.scoop` + `runtime/c` | `std_sync_basic` | |
| `CondVar` (create/wait/notify) | P1 | needs_runtime_lib | DONE | `sysroot/sync.scoop` + `runtime/c` | `std_sync_basic` | |
| `Once` (create/isDone/run) | P1 | needs_runtime_lib | DONE | `sysroot/sync.scoop` + `runtime/c` | `std_sync_basic` | |
| `Channel<T>` (create/send/recv/close) | P1 | needs_runtime_lib | TODO | — | — | 旧 `sysroot/channels.scoop` 已在 `T5000e3b` 移除；需按新并发/通信设计重做 |
| Atomics (`__AtomicInt` load/store/CAS) | P2 | needs_runtime_lib | DONE | `sysroot/unsafe.scoop` + 编译器 | 部分 run-pass | SeqCst only；更多 ordering 后置 |

---

## 15. Task (async core)

| 能力项 | 优先级 | 分类 | 状态 | 实现位置 | Fixtures | 备注/缺口 |
|---|:---:|:---:|:---:|---|---|---|
| Lazy `Task<T>` create/join core | P1 | needs_runtime_lib | DONE | `sysroot/core.scoop` + `sysroot/task.scoop` + generic continuation/atomic/thread runtime | `async_fun_task_runtime_basic` / `async_await_string_basic` | 仅保留内部 `__task_*` helper；无公开 executor surface；single-driver task 以私有 claim-bit 做顺序 handoff；task-only C ABI 已删除 |
| `Task.step()` / `TaskStep<T>` | P1 | pure_scoop_ok | DONE | `sysroot/core.scoop` + `sysroot/task.scoop` + generic continuation/atomic runtime | `task_step_manual_basic` | 公开 manual-drive surface 已收口；`Task.step()` 走 ordinary Scoop 定义，不再依赖 `scoop_task_poll`；claim 冲突/`Running` 观察直接 trap，`Pending` 只表示真实未就绪 |
| Structured concurrency / `spawn` 最终语义 | P2 | pure_scoop_ok | TODO | — | — | 当前 `spawn/join` 仍是过渡期 helper，最终模型待 `T4009c` |

---

## 16. Net

| 能力项 | 优先级 | 分类 | 状态 | 实现位置 | Fixtures | 备注/缺口 |
|---|:---:|:---:|:---:|---|---|---|
| `TcpStream` / `TcpListener` 声明 | P2 | needs_runtime_lib | TODO | — | — | 旧 `sysroot/net.scoop` 已在 `T5000e3b` 移除；需按新 net surface 重做 |
| `tcpConnect` / `tcpListen` / `accept` / `close` | P2 | needs_runtime_lib | TODO | — | — | 需先重建设计，再落 runtime socket backend |
| `writeUtf8` / `readUtf8` | P2 | needs_runtime_lib | TODO | — | — | 同上 |

---

## 17. Unsafe / Pointers

| 能力项 | 优先级 | 分类 | 状态 | 实现位置 | Fixtures | 备注/缺口 |
|---|:---:|:---:|:---:|---|---|---|
| `Ptr<T>` — load/store/plus/minus/cast | P1 | needs_new_intrinsic (已有) | DONE | `sysroot/unsafe.scoop` | 部分 run-pass | intrinsic |
| `FunPtr<F>` — invoke | P1 | needs_new_intrinsic (已有) | DONE | `sysroot/unsafe.scoop` | 部分 run-pass | intrinsic |
| `addressOf` / `stackAlloc` | P1 | needs_new_intrinsic (已有) | DONE | `sysroot/unsafe.scoop` | 部分 run-pass | intrinsic |
| GC handles (`pin`/`unpin`/`handleNew`) | P2 | needs_new_intrinsic (已有) | DONE | `sysroot/core.scoop` | 部分 run-pass | intrinsic |

---

## 18. Scope functions

| 能力项 | 优先级 | 分类 | 状态 | 实现位置 | Fixtures | 备注/缺口 |
|---|:---:|:---:|:---:|---|---|---|
| `Int.let/run/also/apply` | P0 | pure_scoop_ok | DONE | `stdlib/prelude.scoop` | `kotlin_scope_functions_basic` | 仅 Int |
| 泛型 `<T>.let/run/also/apply` | P1 | pure_scoop_ok | TODO | — | — | 依赖泛型 codegen |

---

## 19. Preconditions

| 能力项 | 优先级 | 分类 | 状态 | 实现位置 | Fixtures | 备注/缺口 |
|---|:---:|:---:|:---:|---|---|---|
| `require(condition)` / `check(condition)` | P0 | pure_scoop_ok | DONE | `stdlib/prelude.scoop` | `kotlin_require_check_basic` | |
| `requireLazy` / `checkLazy`（lazy message） | P0 | pure_scoop_ok | DONE | `stdlib/prelude.scoop` | `kotlin_require_check_lazy_message_basic` | |

---

## 20. Test utilities

| 能力项 | 优先级 | 分类 | 状态 | 实现位置 | Fixtures | 备注/缺口 |
|---|:---:|:---:|:---:|---|---|---|
| `assertTrue` / `assertFalse` | P0 | pure_scoop_ok | TODO | — | — | `stdlib/test.scoop` 已在 `T5000e3b` 移除；当前 fixture 改用局部 helper 或 `require(...)` |
| `assertEqInt` | P0 | pure_scoop_ok | TODO | — | — | 同上，后续需按新测试 support 设计重做 |
| `assertSomeInt` / `assertNoneInt` | P1 | pure_scoop_ok | TODO | — | — | 同上 |
| `assertEqString` | P1 | pure_scoop_ok | TODO | — | — | 需测试 support 重做 + String comparison |
| `assertEqBool` | P1 | pure_scoop_ok | TODO | — | — | 需测试 support 重做 |

---

## 21. Reflection

| 能力项 | 优先级 | 分类 | 状态 | 实现位置 | Fixtures | 备注/缺口 |
|---|:---:|:---:|:---:|---|---|---|
| `nameOf<T>()` / `sizeOf<T>()` / `alignOf<T>()` | P2 | pure_scoop_ok (intrinsic) | DONE | `sysroot/core.scoop` | comptime fixtures | |
| `fieldsOf<T>()` / `variantsOf<T>()` | P2 | pure_scoop_ok (intrinsic) | DONE | `sysroot/core.scoop` | comptime fixtures | |
| `annotationsOf<T>()` / `paramsOf(fn)` | P2 | pure_scoop_ok (intrinsic) | DONE | `sysroot/core.scoop` | comptime fixtures | |

---

## Summary: P0/P1 缺口与建议优先级

### 最高优先级缺口（P0，直接影响语言可用性和 fixture 可写性）

1. **Text 基础**：`String.length`/`substring`/`startsWith`/`split`/`indexOf`/`contains`/`String.toInt`/`Int.toString`
   - 分类：`needs_runtime_lib`
   - 路径：runtime/c 新增 `scoop_string_*` API → sysroot 声明 → stdlib 封装 → fixtures
   - 依赖：无

2. **泛型 collections 操作**：`Array<T>`/`MutableArray<T>` 的 forEach/map/filter/fold 的 `<T>` 版本
   - 分类：`pure_scoop_ok`
   - 依赖：泛型单态化 / 跨文件 codegen 完善（编译器侧）

### 高优先级缺口（P1，提升可用性但不阻塞核心链路）

3. **Text 格式化**：`StringBuilder`/`joinToString`
   - 分类：`pure_scoop_ok` / `needs_runtime_lib`
   - 依赖：Text 基础（P0-1）

4. **Math 基础**：`abs`/`min`/`max`（Int）
   - 分类：`pure_scoop_ok`
   - 依赖：无

5. **Hashing 落地**：Int/String 的真实 hash + hash-based Set/Map
   - 分类：`pure_scoop_ok` / `needs_runtime_lib`
   - 依赖：Text 基础（P0-1，用于 String hash）

6. **Collections 算法**：`sort`/`reduce`/`zip`/`flatten`
   - 分类：`pure_scoop_ok`
   - 依赖：基础 collections（已 DONE）

7. **Ranges 增强**：`..` syntax / `until` / `for (x in range)` integration
   - 分类：`pure_scoop_ok`
   - 依赖：前端语法糖 + for-in 链路（T1508）

8. **Duration / time surface 重建设计**
   - 分类：`pure_scoop_ok`
   - 依赖：新的 time surface（`T5000e3b` 后当前未提供）

9. **Random / PRNG**
   - 分类：`pure_scoop_ok`
   - 依赖：新的 time 或 OS entropy surface（当前未提供）

10. **Test utilities 重做**：`assertTrue`/`assertFalse`/`assertEqInt`/`assertSomeInt`/`assertNoneInt`
    - 分类：`pure_scoop_ok`
    - 依赖：新的测试 support 设计；`assertEqString` 还额外依赖 String comparison

### Intrinsic 结论

与 `RUNTIME_STDLIB_INTRINSIC_AUDIT.md` 一致：**当前不需要新增编译器 intrinsic**。所有缺口均可通过 `pure_scoop_ok` 或 `needs_runtime_lib` 路径补齐。
