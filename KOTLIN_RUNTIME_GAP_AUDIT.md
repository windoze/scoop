# Kotlin runtime / Scoop core runtime gap 审计（T1314）

> 生成时间：2026-03-30  
> 目的：为后续 `std` 设计与实现（T1315/T1316/T1317）提供**能力矩阵**与“是否需要 intrinsic”的早期结论，并为 T1017/T1018（intrinsic gate）提供输入。

## 1. 范围与原则

### 1.1 本文讨论的 “Kotlin runtime”

这里的 Kotlin runtime 指 **Kotlin/JVM 标准库中与 JVM 绑定无关、但在语义上对 Kotlin 风格语言通用且有高用户价值** 的那部分能力，例如：

- collections / iterators / sequences
- ranges / progressions
- text（基础字符串算法与格式化）
- math / random / time（不依赖 JVM 的功能部分）
- properties（delegated properties 等）

明确排除：

- `kotlin.jvm.*`、`java.*` 相关（JVM 平台绑定）
- 依赖 JVM 反射模型/类加载器的部分（若需要，走 Scoop 自己的静态反射/编译期反射路线）

### 1.2 分类口径

为了后续任务可执行与可验证，本文将每个能力项归入三类之一：

1. **pure_scoop_ok**：可用纯 Scoop 库实现（允许依赖已存在的 sysroot 声明与既有运行时：GC/effect/print 等）；不要求新增编译器 intrinsic。
2. **needs_runtime_lib**：需要补齐运行时/平台库（例如 C runtime 或未来 Scoop runtime），但不要求新增编译器 intrinsic；编译器侧只需能链接并调用。
3. **needs_new_intrinsic**：需要新增编译器可识别的 primitive（intrinsic）或后端 hook（例如原子、线程/栈切换、低层内存布局/拷贝、平台 ABI 专用入口等）。  
   **注意**：任何该类结论都必须在 T1017 中再次审计确认，本文只给出“疑似/候选”项，避免在 std 任务里隐性扩 intrinsic。

### 1.3 当前 Scoop baseline（用于对比）

以仓库现状（2026-03-30）为基线：

- sysroot：`sysroot/core.scoop`、`sysroot/delegates.scoop`、`sysroot/collections.scoop`、`sysroot/unsafe.scoop`
- 运行时（早期 C runtime）：GC（mark-sweep v0）、shadow stack roots、effect TLS/perform slot、`print/println` 等
- 语言侧已落地：value types（struct/enum/tuple）、`when`、class init 顺序、delegated properties 的最小可执行语义、部分编译期反射 metadata（`TypeMeta/FieldMeta/...`）

因此，本审计的重点不是“有没有 Kotlin 一样的 API 名称”，而是：

- Scoop 要补齐哪些**通用能力**（能力矩阵）
- 哪些能力可在纯 Scoop 层完成，哪些必须落到 runtime
- 哪些能力可能真的需要新 intrinsic（并交由 T1017 gate）

---

## 2. 能力矩阵（capability matrix）

优先级约定：

- **P0**：能显著提升语言可用性/示例可写性，且与现有语言特性强耦合（fixture 价值高）
- **P1**：常用但可后置，不会阻塞其它核心链路
- **P2**：可选增强（性能/完整性/对齐 Kotlin），不建议阻塞 std v1

> “Blockers” 一列优先引用 TODO 任务号（若无明确任务号，则描述所需前置能力）。

| 领域 | Kotlin 代表能力 | 用户价值 | 优先级 | 分类 | Blockers / 依赖 | 备注（对 Scoop 的建议形态） |
|---|---|---:|:---:|:---:|---|---|
| core | `Any`、`Unit`、`Nothing`、`String`、`Bool/Int` 等 | 高 | P0 | pure_scoop_ok | 已有 sysroot 声明 | 继续坚持“声明在 sysroot、布局在编译器/运行时”的分层 |
| properties | `lazy/observable/vetoable`、map-backed delegate | 中 | P0 | pure_scoop_ok | 已完成（T1313） | 早期 lowering 特判可接受，但需在 std 成熟后逐步回到通用机制 |
| collections 基础形态 | `List/MutableList/Set/Map`、iterator | 高 | P0 | pure_scoop_ok（上层） / needs_new_intrinsic（底层疑似） | T1316/T1317；`Array/MutableArray` 真实语义 | 上层 API/算法纯 Scoop；底层 buffer 分配/扩容/搬移可能需要少量 intrinsic（交给 T1017 评估） |
| sequences | `Sequence<T>`、`map/filter/take` | 中 | P0 | pure_scoop_ok | 需要函数值调用/闭包可执行链路（T1307b 等） | Scoop 有代数效果，可把 “lazy sequence” 与 effect/iterator 结合设计 |
| ranges / progressions | `0..n`、`until`、`downTo`、`step`、`for` 迭代 | 中 | P0 | pure_scoop_ok | 需要语法糖/运算符（若已有则无） | 建议把 “progression” 作为可迭代值类型，并让 `for` 降到 iterator 协议 |
| text 基础 | `String` 拼接、`substring`、`startsWith`、`split`（简化） | 高 | P0 | needs_runtime_lib | String 的底层编码/切片约束 | 早期可先限定 UTF-8/byte-index 或提供 “grapheme safe” 明确区分；复杂 Unicode 规则可后置 |
| text 格式化 | `StringBuilder`、`joinToString` | 中 | P1 | pure_scoop_ok / needs_runtime_lib | 依赖 collections/text 基础 | `StringBuilder` 可先用可变字节数组实现，再在 runtime 提供高效拼接 |
| math | `abs/min/max`、三角函数等 | 中 | P1 | needs_runtime_lib | 链接 libc/libm | 纯库封装 + FFI；不建议新增 intrinsic |
| hashing | `hashCode`、常用 hash（Murmur/xxHash） | 中 | P1 | pure_scoop_ok / needs_runtime_lib | 若需要与 runtime/GC 交互则依赖 runtime | 可以先把稳定 hash 算法放 std；对象 identity hash 若需要，可能要 runtime 支持 |
| random | `Random`、`nextInt` | 中 | P1 | pure_scoop_ok / needs_runtime_lib | 种子来源（时间/熵） | PRNG（xorshift/pcg）纯库；默认 seed 可能需要 runtime（time/entropy） |
| time | `Duration`、`TimeSource`/`now()` | 中 | P1 | needs_runtime_lib（或 needs_new_intrinsic 候选） | 平台时钟 API | 推荐先走 runtime lib（C/平台 API），避免 intrinsic；若需编译器内建常量折叠再评估 |
| io（基础） | `println` 已有；`readLine` | 中 | P1 | needs_runtime_lib | 平台 stdin/stdout | 早期以最小 API 形态落地即可（fixture 驱动），复杂 encoding 后置 |
| io（文件/路径） | `File`、`Path`、读写文件 | 中 | P2 | needs_runtime_lib | 平台抽象层 | 建议在 `std` 设计（T1316）里拆分 platform layer |
| concurrency | 线程、mutex、channel、atomics | 中~高 | P2 | needs_new_intrinsic（候选） / needs_runtime_lib | 线程/原子内存序 | 若 Scoop 以 effect/async 为主，可把 “并发原语” 作为后置；但 atomics/线程注册对 GC 影响大，需 gate |
| coroutines | Kotlin `suspend`/Continuation | 高（若对齐 Kotlin） | P2 | 视 Scoop effect 路线而定 | effect runtime 已有基础 | Scoop 的代数效果可作为更统一模型；必要时提供 Kotlin 风格 façade |
| reflection | Kotlin runtime reflection | 低~中 | P2 | pure_scoop_ok（静态/编译期） | T1208/T1209/T1215 等已部分具备 | 建议优先发展 Scoop 的静态反射与 comptime，而非复刻 Kotlin/JVM 反射模型 |

---

## 3. 结论与对后续 TODO 的建议落点

### 3.1 可以直接推进的（不经 T1017 gate）

这些能力应优先进入 **T1315（纯 Scoop 补齐）** 或 **T1316/T1317（std 设计与 v1）**：

- collections：上层抽象（List/Map/Set）与算法（map/filter/fold/join）
- ranges/progressions：基础结构与迭代协议
- sequences：lazy 算法（但依赖可执行的函数值调用链路）
- hashing/random 的纯库部分

### 3.2 需要 runtime lib 支持（但不应新增 intrinsic）

建议作为 `std` 的 platform/runtime layer（T1316）输入：

- text：`String` 的底层表示与常用算法（尤其是 Unicode 相关）
- time：时钟读取/高精度计时
- io：stdin/stdout、文件与路径
- math：libm 封装

### 3.3 “疑似需要新 intrinsic”的候选（必须走 T1017/T1018）

以下条目在 T1314 阶段仅作为“疑似候选”列出，用于提醒 std 设计时避免隐性扩 intrinsic。  
截至 2026-03-30，T1017 已完成审计（见 `RUNTIME_STDLIB_INTRINSIC_AUDIT.md`）：在 `std v1` 与 “Kotlin runtime gap 的纯 Scoop 补齐” 的目标边界下，**暂不需要新增编译器 intrinsic**；这些候选项应优先落到 runtime lib/ABI 层（或纯库层），只有出现“无法表达”的真实 blocker 时才进入 T1018。

- `MutableArray` 的底层 buffer 分配/搬移/扩容：优先通过 runtime lib + type descriptor（或等价元数据）实现，不作为 intrinsic gate 理由。
- 原子与线程相关 primitive：优先通过 runtime lib（C11 原子/平台原子库 + 线程注册）实现；若未来追求“零开销内建原子指令”再评估 intrinsic。
- 对象 identity hash / 对象地址相关能力：属于 GC/对象头语义，优先由 runtime 提供稳定 API。

---

## 4. 附：建议的输出物与维护方式

为避免“审计文档漂移”：

- 当 T1017 得出结论（是否需要 intrinsic）时，应把结论回写到本文的 3.3 小节，并把 “needs_new_intrinsic（候选）” 改为明确结论。
- 当 `std` 设计（T1316）确定模块边界时，应把本表按模块拆分，形成 `core/alloc/std/platform` 的能力映射。
