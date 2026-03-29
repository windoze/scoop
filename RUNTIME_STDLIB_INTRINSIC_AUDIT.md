# runtime/std 的 intrinsic 需求审计（T1017）

> 生成时间：2026-03-30  
> 目的：为后续 `std` 设计与实现（T1316/T1317）与“纯 Scoop 补齐 Kotlin runtime gap”（T1315）提供**底层 primitive 分层结论**；并作为 T1018（新增 intrinsic/backends）的 gate 输入。

## 1. 范围与原则

### 1.1 范围

本文关注的是：当我们推进 **runtime/std** 时，哪些能力能用现有语言机制“纯 Scoop”完成，哪些必须落到运行时/平台库，哪些确实需要新增编译器 intrinsic（或后端 hook）。

覆盖的典型需求：

- `std`：collections / iterators / text / algorithms / io / fs / time / process / sync/thread 等
- “pure Scoop runtime gap”：对齐 Kotlin 风格语言在无 JVM 依赖下常用的 runtime helper（见 `KOTLIN_RUNTIME_GAP_AUDIT.md`）

不覆盖（或仅作为背景）：

- comptime/reflection intrinsics（已在 T12xx 系列落地，属于“现有机制”）
- 具体 `std` API 的命名与模块边界（属于 T1316）

### 1.2 原则（gate 规则）

1. **默认结论应是“无新增 intrinsic”**：若某能力可通过“runtime lib + 普通函数调用 + 既有 sysroot 声明/类型系统”实现，则不新增 intrinsic。
2. **intrinsic 只解决“无法表达”问题**：仅当满足以下任意条件时才可进入 `needs_new_intrinsic`：
   - 需要编译器/后端生成“非函数调用可表达”的操作（例如取址、栈上分配、内建原子指令、特殊 ABI 入口等）
   - 需要编译器在类型系统/可见性上强制门禁，且无法通过普通库 API 约束（例如必须是 `@NoGC`、必须是 `@Unsafe`、必须是 GC-free 类型等）
3. **能用 runtime lib 就不要用 intrinsic**：特别是 OS/平台能力（时间、文件、线程、随机源），优先走 `needs_runtime_lib`。

### 1.3 仓库现状基线（2026-03-30）

“现有机制”包括但不限于：

- sysroot 已有 intrinsics：
  - 反射/平台：`nameOf/sizeOf/alignOf/fieldsOf/variantsOf/superTypesOf/annotationsOf/paramsOf/getPlatform`
  - unsafe：`Ptr<T>`、`ptrToUIntPtr/uintPtrToPtr`
  - effect runtime：`__scoop_effect_*`（slot/active flag 等）
- 早期 C runtime：
  - `scoop_alloc`（可变大小分配，返回对象头指针）
  - mark-sweep GC v0、shadow stack roots、type descriptor（bitmap/trace_fn/release_fn）、pin/unpin
  - `once` 原语与最小线程辅助（用于 fixtures）

> 备注：本文的 `needs_new_intrinsic` 指 **新增** intrinsic；上述“已有 intrinsics”被视为可直接使用的基础能力。

---

## 2. 分层清单（T1017 验收输出）

### 2.1 `pure_scoop_ok`

这些能力可用纯 Scoop 库完成（允许依赖已存在的 sysroot 声明与既有运行时：GC/effect/print 等），不需要新增 intrinsic：

- **算法层**：`map/filter/fold/reduce/chunked/windowed/sort` 等通用算法（在既定集合基元之上）
- **迭代协议与适配器**：`Iterator<T>`/`Iterable<T>`、lazy adapters（只要函数值调用链路可用）
- **ranges/progressions**：`..`/`until`/`downTo`/`step` 的库实现与 `for` 迭代协议对接（语法糖/降级属于前端任务，但不需要 intrinsic）
- **hashing/PRNG（纯库部分）**：xxHash/wyhash/PCG/xorshift 等（seed 来源可下沉到 runtime lib）
- **字符串算法（在 String API 之上）**：`startsWith/endsWith/indexOf/split/join` 的库实现（具体编码与切片约束由 String 表示决定）
- **Kotlin 风格 runtime helper**：scope functions、delegates 的更完整库实现（在既有语义/调用链路可表达的前提下）

### 2.2 `needs_runtime_lib`

这些能力不应新增 intrinsic；应通过运行时/平台库（C runtime 或未来 Scoop runtime）提供，再由上层 Scoop 代码封装：

- **String 表示与 Unicode 相关能力**
  - 选择编码（例如 UTF-8）与切片规则（byte-index vs scalar-index vs grapheme）
  - 与 OS/FFI 的编码边界（stdin/stdout、文件名、环境变量）
- **`Array<T>` / `MutableArray<T>` 的底层存储与搬移**
  - 可通过 runtime 提供“分配/扩容/搬移/填充/拷贝”的最小 API（例如 `memmove`/`memcpy`/buffer grow）
  - 对 GC 指针的正确追踪应依赖 type descriptor（bitmap 或 trace_fn），属于 runtime 的职责；编译器侧只需保证 ABI/元数据一致
  - 结论：这类需求更像“runtime ABI + codegen 能力”，不是 intrinsic gate 的理由
- **时间/随机/IO/FS/进程/环境**
  - `now()`/monotonic clock、熵源、stdin/stdout、文件/路径、进程/子进程等都属于平台能力
  - 建议在 T1316 的 `platform layer` 中统一抽象，避免编译器内建
- **并发原语（优先 runtime lib）**
  - thread/mutex/cond/channel 等：优先 runtime lib
  - atomics：优先用 C11 原子或平台原子库在 runtime 中实现，再由 Scoop 封装（性能优化后置，不作为 gate）
- **identity hash / 对象地址相关能力（若 std 需要）**
  - 需要与 GC/对象头配合，属于 runtime 语义；可通过 runtime 提供“稳定且可审计”的 API

### 2.3 `needs_new_intrinsic`

（空）

结论：以当前阶段的 `std v1` 与 “Kotlin runtime gap 的纯 Scoop 补齐”目标为边界，没有发现**必须**新增编译器 intrinsic 的 blocker。  
所有先前“疑似需要 intrinsic”的候选项（数组 buffer、原子、identity hash 等）都可以通过 runtime lib + ABI 合约方式落地，并保持 gate 结论为“无新增 intrinsic”。

---

## 3. 对后续 TODO 的落点建议（非验收项，但用于避免漂移）

- `T1018`：当前结论为“无新增 intrinsic”，因此 **不应**为了推进 `std` 主线而主动开启 T1018；保留该任务用于未来确实出现“无法表达”的 case。
- `T1315/T1316/T1317`：
  - 以“runtime lib 提供底层 primitive，Scoop 提供抽象与算法”作为默认路线；
  - 若实现过程中出现“只能通过编译器内建才能表达”的点，必须回流到 T1017 更新审计结论，再考虑进入 T1018。

> 推荐维护策略：当 `std` 的实现第一次出现“需要新增 intrinsic”的真实例子时，把该例子写进本文的 2.3，并附上最小可回归 fixture（指向 future T1018 的验收物）。

