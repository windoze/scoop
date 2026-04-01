# Scoop `std` 分层设计（T1316）

> 生成时间：2026-03-30  
> 目的：为后续 `std` 设计与实现（T1317+）提供**分层边界**、**稳定性策略**与**平台能力矩阵**，并把 “sysroot / stdlib / runtime / platform backends” 的职责说清楚。

## 1. 名词与边界（先把“谁负责什么”说清）

### 1.1 sysroot（编译器内建声明源）

当前仓库中的 `sysroot/*.scoop` 是编译器默认注入的“声明源”：

- 只承诺**名字可见**与**类型可检查**；
- 允许只给出 **API 表面**，实现可能在 runtime 或 stdlib；
- sysroot 中包含的少量 `intrinsic`（例如反射/平台查询）属于**编译器识别的 primitive**，其演进需要非常谨慎（见 `RUNTIME_STDLIB_INTRINSIC_AUDIT.md`）。

### 1.2 stdlib（随编译器发布的 Scoop 源码库）

当前仓库中的 `stdlib/*.scoop`（目前只有 `stdlib/prelude.scoop`）属于 **pure-scoop 标准库实现的早期落点**：

- 由 driver（`scoop build/run`）在编译时注入到编译单元；
- 以 “库代码” 方式提供高层语义（例如 Kotlin-like helpers），原则上不引入新的 intrinsic；
- 允许与 sysroot “同名 API” 的声明协作：sysroot 提供声明，stdlib 提供实现。

### 1.3 runtime lib（本地/平台库，当前主要是 C runtime）

运行时库（例如当前的 `runtime/c/` + `crates/scoop_runtime/`）负责：

- GC、分配器、roots 扫描（当前实现包含 shadow stack；目标是 LLVM stackmap/statepoint 精确根集）、effect runtime 等**运行期机制**；
- 需要 OS/ABI 的能力（时间、文件、线程、网络等）的**平台胶水**（长期由 C runtime 的 platform/backends 层提供；不把 OS 类型泄漏到 Scoop 侧）。

> 原则：能落到 runtime lib 的不要变成 intrinsic（参见 `RUNTIME_STDLIB_INTRINSIC_AUDIT.md` 的 gate 结论）。

### 1.4 platform backends（编译期可选的一组 runtime 实现）

platform backend 指“一套可被编译期选择的运行时实现组合”，例如：

- desktop/server：POSIX/Windows runtime backend（完整 OS 能力）
- embedded：裁剪 runtime backend（可能没有线程/文件系统）
- wasm：hosted/adapter backend（例如 WASM GC adapter、WASI 接口）

该概念与 TODO `T1406/T1409` 直接相关：GC backend 与平台 backend 需要组合、可替换。

---

## 2. 设计目标（对标 Rust，但不照搬 API）

### 2.1 对标点

- **分层对标 Rust**：至少区分 `core / alloc / std / platform` 四层；
- **语义优先**：不要求 API 名称与 Rust 一致，但要做到能力同量级、可比较；
- **避免隐性扩 intrinsic**：`std v1` 以 “sysroot 声明 + stdlib 实现 + runtime lib 支撑” 为主线。

### 2.2 Scoop 特有约束（0.1 阶段需要显式写出来）

- GC 与 effect runtime 是语言语义的一部分：即使 `std` 做分层，也必须把这些依赖显式化；
- 编译期反射与 comptime 能力已经在 sysroot 有 intrinsics：`std` 的设计需要把 “编译期数据结构” 与 “运行期集合” 区分开；
- 多平台：`std` 需要从第一天就给出 capability matrix，否则 “看起来能 import，但跑不起来” 会造成长期技术债。

---

## 3. 推荐分层（core / alloc / std / platform）

这里的 “层” 是**依赖方向**，不是文件目录：

### 3.1 `core`（无平台假设的语言核心库）

**定位**：只依赖语言本身与最小 runtime（effect/GC 的存在性），不依赖 OS/文件/线程/网络等平台能力。  
**典型内容**：

- `scoop.core`：标量类型、`Any/Unit/Nothing`、最小异常/错误载体、基础操作符与语义胶水
- `scoop.unsafe`：`Ptr<T>` 等（仍属 core，但门禁更严格）
- 反射/编译期 API 的“表面”：`TypeMeta/*Meta` 与 `nameOf/sizeOf/...`
- `print/println` 这类 I/O：在 0.1 可视为 core 的最小可观察接口，但实现来自 runtime

### 3.2 `alloc`（分配/GC 抽象与“会分配的基础类型”）

**定位**：把 “需要堆/分配器/GC 的部分” 与 core 分开，便于 embedded/wasm 裁剪。  
**典型内容**：

- `String` 的表示与基本操作（底层编码/切片规则由 runtime/platform 参与定义）
- `Array<T>` / `MutableArray<T>` 这类集合底座（上层算法纯 Scoop，底层 buffer 操作走 runtime lib）
- 与 GC/allocator 的稳定抽象边界（与 `T1406` 对齐）

> 备注：当前 sysroot 把 `String/Array` 放在 `scoop.core`，属于早期工程折中；`std` 设计上仍建议把它们归为 `alloc` 层概念，以便 capability matrix 可表达。

### 3.3 `std`（面向用户的标准库：可移植语义 + 按平台裁剪）

**定位**：建立稳定、可组合的高层 API；其中部分子模块依赖 platform backend。  
**典型内容**：

- collections / iterators / algorithms
- text utilities（在 `String` API 之上）
- time / random / hashing
- io / fs / path / process / env
- sync / thread / channels / task adapters
- net / testing support

### 3.4 `platform`（平台适配层：由 runtime/backends 提供）

**定位**：把 “同名 std API 在不同平台的实现差异” 下沉到 platform layer，并给出清晰的可用性与降级策略。  
**典型形态**：

- `scoop.platform.posix.*` / `scoop.platform.windows.*`：本地 OS 封装
- `scoop.platform.wasi.*`：WASI 封装
- `scoop.platform.browser.*`：浏览器 host 封装（可能只提供 subset）
- `scoop.platform.embedded.*`：无 OS/裁剪环境的最小实现或明确不可用

---

## 4. 推荐模块树（供后续任务落地时遵循）

这里给出一棵“面向用户 import 的模块树”。实现可以分散在 sysroot/stdlib/runtime，但 **API 命名与依赖方向**应保持一致。

### 4.1 core / alloc（底座）

- `scoop.core`
  - 内建标量与 root types、`RuntimeError` 等最小错误载体
  - effect/反射相关的 sysroot 声明面
- `scoop.unsafe`
  - `Ptr<T>`、原始指针转换、FFI 辅助
- `scoop.alloc`（建议新增包；或在 0.1 先以 `scoop.core` 子集过渡）
  - allocator/GC 抽象边界（与 `T1406` 对齐）
  - `String` / `Array<T>` / `MutableArray<T>` 的 alloc 语义层定义（表面）

### 4.2 std（用户层）

- `scoop.collections`
  - `Iterable/Iterator`（迭代协议推荐形状：`Iterator.next(): Option<T>`，避免 `hasNext()` + 内部缓存）
  - `Array/MutableArray` 的高层算法扩展（map/filter/fold/…）
  - `List/MutableList/Set/Map/MutableMap`（T1317）
- `scoop.text`
  - 字符串算法（substring/indexOf/split/joinToString/format 等）
- `scoop.hash`
  - `Hashable`、hash 算法与约束（T1317 需要）
- `scoop.random`
  - PRNG（纯库）+ runtime seed（平台）
- `scoop.time`
  - `Duration/Instant` + platform clock
- `scoop.io`
  - `stdin/stdout`、buffered I/O、编码边界
- `scoop.fs` / `scoop.path`
  - 文件与路径
- `scoop.process` / `scoop.env`
  - 进程与环境变量
- `scoop.sync` / `scoop.thread` / `scoop.channels`
  - 线程、同步与通信（能力受 platform backend 约束）
- `scoop.net`
  - socket / dns / http adapters（逐步推进）
- `scoop.test`
  - 测试与断言辅助、golden helpers（仅 std 测试环境）

> 说明：当前 sysroot 中已存在 `scoop.collections.Map` 的最小声明（为 delegated property 服务）。后续 `std` 落地时应避免破坏该路径；可以通过扩展/补齐接口的方式演进。

---

## 5. 稳定性与版本策略（0.1 阶段的“可进化约束”）

### 5.1 稳定性分级（建议）

0.1 阶段建议把 std API 分为三类：

1. **Stable**：默认可用、向后兼容（修 bug 不算破坏）
2. **Experimental**：允许破坏性变更（需要显式标注，例如 `@Experimental` / `@RequiresOptIn` 风格）
3. **Platform**：平台相关 API（即使稳定，也必须声明 capability matrix 与失败模式）

### 5.2 版本绑定（编译器 ↔ sysroot ↔ stdlib）

建议把以下三者视为“同版本发布单元”：

- `scoopc`（编译器）
- `sysroot/*.scoop`（声明源）
- `stdlib/*.scoop`（随编译器发布的源码库）

这样可以避免 “stdlib 调用了 sysroot 中不存在的声明” 或 “sysroot 声明变更但 stdlib 未同步” 的漂移。

---

## 6. 平台能力矩阵（capability matrix）

下表按 **层/模块** 给出在不同目标平台上的可用性，以及其依赖的 runtime/platform backends。

图例：

- ✅：完整可用（或目标平台上可提供等价语义）
- ⚠️：部分可用（subset/降级/需要 feature）
- ❌：不可用（明确不支持）

| 模块/层 | desktop / server | embedded | wasm（WASI） | wasm（browser） | 关键依赖（runtime/platform） |
|---|:---:|:---:|:---:|:---:|---|
| `core`（`scoop.core`/反射表面/最小 I/O） | ✅ | ✅ | ✅ | ✅ | effect runtime；`print/println` 走平台输出 |
| `alloc`（GC/allocator + `String/Array` 表面） | ✅ | ⚠️ | ✅ | ✅ | **GC backend**（T1406：baseline/embedded/adapter） |
| `collections/iterators/algorithms` | ✅ | ✅ | ✅ | ✅ | 依赖 alloc；不依赖 OS |
| `text`（String 算法） | ✅ | ⚠️ | ✅ | ✅ | String 表示/编码边界（runtime lib） |
| `time` | ✅ | ⚠️ | ✅ | ⚠️ | 平台时钟（POSIX/WinAPI/WASI/JS） |
| `random` | ✅ | ⚠️ | ✅ | ✅ | 熵源（OS/WASI/JS）；PRNG 纯库 |
| `io`（stdin/stdout） | ✅ | ⚠️ | ✅ | ⚠️ | 平台 I/O（WASI/JS 受限） |
| `fs/path` | ✅ | ❌ | ⚠️ | ❌ | 文件系统 API（WASI subset；browser 无） |
| `process/env` | ✅ | ❌ | ⚠️ | ❌ | 进程/环境（WASI subset；browser 无） |
| `thread/sync/channels` | ✅ | ⚠️ | ⚠️ | ⚠️ | 线程/原子/TLS（平台能力差异大；需显式 feature） |
| `delegates`（`lazy/observable/vetoable`） | ✅ | ⚠️ | ⚠️ | ⚠️ | 线程安全语义依赖 `sync.Mutex`；无线程平台需降级/报错（T1326c） |
| `net` | ✅ | ⚠️ | ⚠️ | ⚠️ | socket/DNS/host API（WASI/JS 形态不同） |
| `test` | ✅ | ⚠️ | ⚠️ | ⚠️ | 测试 runner 与宿主支持（通常用于 host-side） |

### 6.1 GC backend 与 std 的关系（与 T1406/T1409 的接口约束）

为了让 `alloc/std` 在不同平台可裁剪、可替换，建议把 GC/backend 需求收敛为以下“对 std 可见的能力”：

- `alloc`：分配/释放（或由 GC 接管）、roots 扫描、pin/unpin、（可选）finalizer/release hook
- `time`/`thread`：是否支持线程、TLS、原子（影响 GC 的 STW/并发安全边界）
- `wasm`：是否为 hosted/adapter backend（例如 WASM GC、或线性内存 + host 回调）

> 上述是 **std 的需求接口**；具体如何实现（C runtime / Scoop runtime / adapter）由 `T1406/T1409` 决定。

### 6.2 delegated properties 的平台策略（lazy/observable/vetoable）

背景：delegated properties 的部分语义会被编译器 lowering 为对 runtime 原语的直接调用（例如 `Mutex.lock/unlock`）。
因此在“目标平台不具备对应能力”时，**必须在编译期给出清晰诊断**，避免出现“能编译但无法链接/运行”的隐性失败。

当前阶段（T1326c，early stage）策略：

- `lazy(LazyThreadSafetyMode.None)`：不依赖线程/互斥锁，可在所有平台落地（语义等价于单线程 lazy cache）。
- `lazy()` 默认模式、以及 `lazy(Publication/Synchronized)`：
  - 会被 lowering 为 `sync.Mutex` + lock/unlock；
  - 仅在 **desktop/server（host pthread/WinAPI 等）** 可用；
  - 在无线程/无 mutex 平台（embedded / wasm 默认）将报错，并提示改用 `LazyThreadSafetyMode.None` 或按平台分发。
- `observable/vetoable`：
  - 当前实现（T1326b）依赖 per-property `Mutex` 保证并发可见性与避免 data race；
  - 因此同样要求目标平台具备线程/互斥锁能力；无 mutex 平台将报错（未来可考虑单线程降级实现）。

---

## 7. 对后续任务的落点提示（不作为验收，但避免漂移）

- `T1317`（std v1：collections/iterators/text/algorithms）应严格遵循 “alloc 底座 + std 算法” 的分层：上层 API/算法尽量 pure-scoop；底层 buffer/编码/平台能力走 runtime lib。
- 若 `T1317` 实现过程中出现 “无法表达，必须新增 intrinsic” 的真实 blocker，必须先回流更新 `RUNTIME_STDLIB_INTRINSIC_AUDIT.md`（T1017 gate），再考虑开启 `T1018`。
