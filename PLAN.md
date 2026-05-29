# Scoop：GC Pacing + Immortal Objects 落地计划

> 生成时间：2026-05-29
> 设计基线：[`GC_PACING.md`](./GC_PACING.md)、[`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md)
> 格式参考：[`docs/archive/plans/PLAN-spec-fix-overload.md`](./docs/archive/plans/PLAN-spec-fix-overload.md)、[`docs/archive/plans/PLAN-managed-abi.md`](./docs/archive/plans/PLAN-managed-abi.md)
> 当前状态：两份设计文档均为 P0 design / design-only。运行期今天没有任何按压力触发的 collect（堆单调增长直到 OOM），编译期常量值（String literal / `__type_name` / `Platform`）每次求值都在 GC 堆上分配 wrapper。
> 行号说明：下文以当前文件路径和符号名为准；后续若行号漂移，优先按文件路径、符号名和 fixture / 测试名定位。

## 0. 工作原则

- [`GC_PACING.md`](./GC_PACING.md) 与 [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) 是本轮设计基线。若实现中发现必须改变其中任何运行期或语言决议，必须先回写对应设计文档，再继续改代码。
- 当前活跃计划文档是根目录 [`PLAN.md`](./PLAN.md)；`docs/archive/plans/**` 仅作历史和格式参考，不再回写。
- **两条线的优先级是明确的**：Pacing 决定“长程序能不能跑起来”（correctness / 防 OOM），Immortal 只是“减少 GC 压力”（优化）。Pacing 先行，Immortal 其后。
- 两条线在运行期之外几乎正交：Pacing 全部落在 `runtime/c/`；Immortal 跨 sysroot / typecheck / HIR→MIR lowering / codegen，且只在运行期加一个 `SCOOP_GC_FLAG_IMMORTAL` 标志位。可并行推进，但本计划按“Pacing 优先”排序。
- Immortal 的核心不变式必须守干净：**immortal 对象永不被写、永不被 trace**。任何“可写或可能需要 trace”的对象（`.data` 静态、含可变托管引用的全局）一律不进 immortal 轨道，留作独立后续工作。
- “是否常量化”是由**类型的传递不可变性**决定的通用决策，不是为 String / Platform 开的特判；“是否 dedup”是正交的、且仅对 String 开的内容池行为。实现不得退回“逐类型特判”。
- Pacing 必须 **on-by-default**。现状是无条件无界增长，这不是“在 ergonomics 与正确性之间权衡”，默认开启是底线；只保留 `SCOOP_GC_PACING=off` 给需要确定性堆计数的测试。
- 所有 runtime 改动必须保持现有 backend 分层（`immix` / `hosted` / `minimal`）可编译可回归；阈值比较本身 backend 无关。
- 验收面：`cargo test --all --all-targets`、`python3 tools/run_fixtures.py`、`python3 tools/spec_fixtures.py check`，以及 runtime C 单元测试与长程序回归。

## 1. 当前判断

### Pacing（来自 `GC_PACING.md`，已核对）

- 生产路径唯一的 collect 触发点是 `SCOOP_GC_STRESS` 测试钩子（`runtime/c/scoop_runtime.c:502-507`）；其余都是 public API 或测试代码。没有“堆增长到 X 就收集”的机制。
- block pool 耗尽时无条件 `posix_memalign` 新块（`runtime/c/scoop_gc_immix_internal.h:548-575`），没有“先尝试回收再增长”的回退。
- nursery 满时静默回退到 old space（`runtime/c/scoop_runtime.c:563-567`），**不触发 minor GC**；分代退化成单代。
- `bytes_allocated` 只是一个计数器（`scoop_gc_backend_immix.c:78-79,2416,2524,5382-5388`），从不与任何阈值比较。
- 没有 `SCOOP_GC_HEAP_TARGET` / `TRIGGER_BYTES` / growth-factor / hard cap 任何 env 旋钮。pacing 是真缺失，不是没调好。

### Immortal（来自 `GC_IMMORTAL_FIX.md`，已核对）

- String literal 每次求值都 `scoop_alloc_typed` 一个 `ScoopString` wrapper（`scoopc_codegen_llvm/src/llvm/codegen/main/literal.rs:133-197`），字节负载已在 `.rodata`，但 wrapper 是堆对象。
- `TypeMetadataLiteral::TypeNameString` 复用同一路径，继承 wrapper 分配（`.../mir_body/transport.rs:186-201`）。
- `Platform` 每次读取分配 5 个 `ScoopString` 再 SSA insert 成结构体（`.../mir_body/transport.rs:203-292`）。
- `ScoopGcObjectHeader` 的 `mark` 被 marker 无条件写（`runtime/c/scoop_gc_backend_immix.c:2719-2737`），所以朴素 `.rodata` 拷贝会在首次 trace 时打只读页 fault；但 slot 扫描已通过 heap-membership 过滤堆外指针（`:2739-2760`），这是 immortal 透明性的支点。
- `scoop.unsafe.__AtomicInt` 当前是 `typealias = Int`（`sysroot/lib/scoop.unsafe/src/unsafe.scoop:163`），在类型层被擦除成 `Int`。它是 interior-mutable（经 unsafe 原子 intrinsic 原地写）但在类型层不可见——这是常量化谓词的隐藏陷阱：藏在 `val` 字段后会被误判可变性、误常量化进 `.rodata`、原子写 fault。
- Scoop 无任何 reference-identity 运算符（`docs/spec/language_spec-part3.md:66` 仅 `==`/`!=`），所以**对象身份对任何 ref 类型都不可观测**；这是“不可变即可常量化”和“dedup 仅 String 安全”的根。

## 2. Gap 覆盖矩阵

| Gap | 当前状态 | 本轮动作 | 归属阶段 |
|---|---|---|---|
| Pacing 软触发（堆增长阈值） | `bytes_allocated` 从不比较 | alloc 后比较 `next_gc`，超过则 safepoint 处请求 collect | P1 |
| `next_gc` / `request_collect` / safepoint 集成 | 无 | `ScoopGcHeap` 加 `next_gc`，请求标志由 `scoop_gc_safepoint_poll` 消费，cycle 末 `next_gc = max(min, live*factor)` | P1 |
| `SCOOP_GC_PACING` 等 env 旋钮 | 无 | 加 `PACING` / `GROWTH_FACTOR` / `MIN_THRESHOLD_BYTES` / `MAX_HEAP_BYTES`，默认 on | P1 |
| nursery 满 ⇒ minor GC | 静默回退 old space | 满则 minor GC 再重试，仍满才落 old | P2 |
| block pool 耗尽 ⇒ full GC | 无条件 `posix_memalign` | 两表空时先 full GC，取到块再用，取不到才增长 | P2 |
| hard cap OOM 防御 | 无 cap | `MAX_HEAP_BYTES`，post-GC 重试仍超则 `scoop_alloc` 返回 NULL | P2 |
| backend parity（hosted/minimal） | 仅 immix 关注 | 阈值比较 backend 无关，hosted/minimal 也尊重旋钮 | P2 |
| `__AtomicInt` typealias → marked struct | `typealias = Int`，类型层擦除 | 升为 `@InteriorMutable struct __AtomicInt { val raw: Int }`，5 处擦除点改“类型相异、布局=Int” | P3 |
| `@InteriorMutable` 注解 | 无 | 新增注解，metadata-only，`is_immutable` 读它即否决 | P3 |
| `SCOOP_GC_FLAG_IMMORTAL` + marker 短路 | header 无 immortal 概念 | 加两个 sentinel，`scoop_gc_mark_object_if_needed` 加 flag 短路 | P4 |
| byte 数组 content-hash 键 + `unnamed_addr` | span-key，无 dedup | 改 content-hash，加 `set_unnamed_addr(true)`，跨 TU 折叠 | P4 |
| `is_immutable(T)` 谓词 | 无 | 传递不可变 + `@InteriorMutable` 否决，值/ref 双层 | P5 |
| `try_emit_immortal` 通用折叠器 | 三个手写路径 | 标量/字符串/纯数据聚合统一折叠器，提升门=全 Const+boxing none+kind(+ref 加 is_immutable) | P5 |
| String literal 走 immortal | 每次 `scoop_alloc_typed` | `codegen_string_literal_from_bytes` 经折叠器，零分配 | P5 |
| dedup（仅 String 内容池） | 无 | String content-pool；其它可常量化 ref 类型 per-site 一份 | P5 |
| `Platform` → MIR StructLit | codegen 动态拼 5 alloc | lowering 成 5 字段 `SynthString` StructLit，折叠器自动吃；删 `codegen_platform_literal` | P6 |
| `TypeMetadataLiteral` 审计 | 走 string 路径 | 确认消费者不 mutate，断言两次 `__type_name(T)` 指针相等 | P6 |

## 3. 代码入口总表

| 主题 | 入口文件 / 符号 | 当前问题 | 目标状态 |
|---|---|---|---|
| alloc 快路径 / safepoint | `runtime/c/scoop_runtime.c::scoop_alloc`（`:498-499` poll，`:502-507` stress，`:563-567` nursery 回退，`:514` OOM⇒NULL） | 无 pacing 触发；nursery 满静默落 old | alloc 后阈值比较请求 collect；nursery 满先 minor GC 再重试 |
| 堆状态 / 计数 | `runtime/c/scoop_gc_backend_immix.c:78-79,2416,2524,5382-5388`、`ScoopGcHeap` 结构 | `bytes_allocated` 只计数不比较 | 加 `next_gc`，cycle 末按 `live*factor` 更新，alloc 比较 |
| block pool | `runtime/c/scoop_gc_immix_internal.h:548-575::scoop_gc_immix_state_take_block`、`:283-299` `block_alloc_new` | 两表空无条件 `posix_memalign` | 先 full GC 回收，取不到才增长，可选 hard cap |
| marker 写 mark | `runtime/c/scoop_gc_backend_immix.c:2719-2737::scoop_gc_mark_object_if_needed`、`:2739-2760` visitor、`:5177/5185` pinned | 无条件写 `mark`，朴素 .rodata 会 fault | flag 短路 immortal；membership 已过滤堆外 |
| GC header / sentinel | `runtime/c/scoop_gc.h:210-244`、`scoop_runtime_api.h:37-38` | header 无 immortal 概念 | 加 `SCOOP_GC_FLAG_IMMORTAL` / `SCOOP_GC_MARK_IMMORTAL` |
| backend parity | `runtime/c/scoop_gc_backend_hosted.c`、`scoop_gc_backend_minimal.c` | 只 immix 有 pacing | 三 backend 均尊重 pacing 旋钮 |
| `__AtomicInt` 声明 | `sysroot/lib/scoop.unsafe/src/unsafe.scoop:163` | `typealias = Int`，标记无处可挂 | `@InteriorMutable struct __AtomicInt { val raw: Int }` |
| `__AtomicInt` 擦除点 | `crates/scoopc_hir/src/typecheck/lower.rs:2662,3522`、`scoopc_codegen_llvm/.../mir_body/types.rs:436`、`scoopc_hir/src/hir/lower/util/generic_layouts.rs:89`、`scoopc_hir/src/hir/lower/main/impl_lowering.rs:1724` | 类型=Int，原子性消失 | 类型=`__AtomicInt` nominal，布局/ABI=Int word |
| atomics 构造 | `sysroot/lib/scoop.core/src/core.scoop`（`AtomicInt`/`AtomicBool` 的 `var raw`） | 直接 `= initial`（Int→__AtomicInt 隐式） | `= __AtomicInt(initial)` 显式构造；无隐式 coerce |
| `@InteriorMutable` 注解 | `crates/scoopc_hir/src/typecheck/{annotations.rs,builtin_annotations.rs}` | 无该注解 | 新增 metadata-only 注解，供谓词读取 |
| 不可变性谓词 / 折叠器 | `scoopc_codegen_llvm/src/llvm/codegen/mir_body/{const_pat.rs,terminator.rs}`（`codegen_mir_const`、`codegen_mir_rvalue` 的 StructLit/MakeTuple arm）；新增 `is_immutable`/`try_emit_immortal` | 三个手写 immortal 路径 | 一个谓词 + 一个折叠器，按类型特征决策 |
| String literal lowering | `scoopc_codegen_llvm/src/llvm/codegen/main/literal.rs:133-197`、`alloca.rs:56-72`（`get_or_create_global_bytes`） | 每次 `scoop_alloc_typed`；byte 数组 span-key | 经折叠器零分配；byte 数组 content-hash + `unnamed_addr` |
| TypeMetadata / Platform | `scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:186-201,203-292`；HIR→MIR `TypedIntrinsicKind::Platform` lowering | Platform 在 codegen 动态拼 | Platform lower 成 StructLit；删 `codegen_platform_literal` |
| 运行期 / fixtures 验收 | `tests/`、`runtime/c/` C 单元测试、`tools/run_fixtures.py`、`tools/spec_fixtures.py` | 无 pacing / immortal 回归 | 长程序堆有界、零分配断言、immortal header 不被写 |

## 4. 顺序总览

1. P0：核对并冻结当前运行期/编译期行为，建立长程序与分配计数的最小回归基线。
2. P1：Pacing 核心——`next_gc` + 软触发 + safepoint 集成 + `SCOOP_GC_PACING` 旋钮。最紧急，单独可用。
3. P2：Pacing 分代与 OOM 防御——nursery-full minor GC、block-pool 回退、hard cap、backend parity。
4. P3：`__AtomicInt` 升为 `@InteriorMutable struct`，引入 `@InteriorMutable` 注解。Immortal 的 sysroot/类型前置。
5. P4：Immortal 运行期——`SCOOP_GC_FLAG_IMMORTAL` + marker 短路；codegen byte 数组 content-hash 键 + `unnamed_addr`。
6. P5：通用 `is_immutable` 谓词 + `try_emit_immortal` 折叠器，String literal 走 immortal，String 内容池 dedup。
7. P6：`Platform` lower 成 StructLit、删除专用 codegen、`TypeMetadataLiteral` 审计。
8. P7：spec / 文档 / fixtures 收尾与全量回归矩阵。

依赖说明：

- P0 先于所有实现阶段：两份设计的“current behavior”都需要一个可复用的 baseline（长程序增长曲线、分配计数、immortal header 写检测），否则后续 agent 重复核对。
- P1 必须先于 P2：分代触发与 OOM 回退都建立在 pacing 核心的 `next_gc` / `request_collect` / safepoint 之上。
- Pacing 线（P1-P2）与 Immortal 线（P3-P6）正交，可并行；但 Pacing 更紧急，故排在前。若并行，唯一交点是 `runtime/c/` 同文件编辑冲突，需协调 P2 与 P4 的 runtime PR 顺序。
- P3 必须先于 P5：`is_immutable` 谓词读 `@InteriorMutable`，且 String/聚合常量化依赖类型层不再把 `__AtomicInt` 当 Int。
- P4 必须先于 P5：codegen 发射 ref-tier immortal `ScoopString` 时引用 `SCOOP_GC_FLAG_IMMORTAL`，运行期常量必须先存在；content-hash 键也是 dedup 的前提。
- P5 必须先于 P6：Platform 折叠依赖通用折叠器与 String immortal 已就位。
- P7 之前不算完成：只让少量测试通过但 spec / 文档仍描述旧行为，不代表闭环。

## 5. 分阶段计划

### P0. 冻结当前行为与最小回归基线

参考：
- [`GC_PACING.md`](./GC_PACING.md) “Current behavior (verified)”
- [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Current behavior (verified)”
- `runtime/c/scoop_runtime.c`、`scoop_gc_backend_immix.c`、`scoop_gc_immix_internal.h`、`scoop_gc.h`
- `tools/run_fixtures.py`、`tests/fixtures/**`

目标：

- 把 pacing 缺失点与 immortal 分配点的当前行为核实成可复用基线，避免每个阶段重新核对行号与路径。
- 建立两个最小度量：长程序的 `bytes_allocated` 增长曲线，以及 String/Platform literal 的 `scoop_alloc_typed` 计数。

必须实现的内容：

1. 核对并记录 pacing 侧关键行号：stress 触发、block pool 增长、nursery 回退、`bytes_allocated` 计数点、env 旋钮集合。
2. 核对并记录 immortal 侧关键行号：String literal lowering、TypeMetadataLiteral、Platform、marker 写、membership 过滤、`__AtomicInt` typealias 与 5 处擦除点。
3. 新增最小度量工具/测试（可先用 `scoop_gc_debug_*` 与现有 fixtures）：
   - 一个 10M 小对象分配循环，记录峰值堆（baseline 下应无界增长）；
   - 一个只含 String literal / `Platform` 读取的函数，断言其 IR 中 `scoop_alloc_typed` 出现次数（baseline 下 >0）。
4. 记录 P0 期间允许保留的偏离，后续阶段逐项关闭。

必须遵从的约束：

- P0 不改变运行期或语言行为；只做核对、度量与记录。
- 不得删除任何现有 GC 测试来掩盖当前无界增长。

阶段输出：

- pacing / immortal 双侧的 verified 行号与路径基线。
- 长程序增长与分配计数两个最小度量，供后续阶段做前后对比。

验证：

1. `cargo test --all --all-targets`
2. `python3 tools/run_fixtures.py`
3. 新增度量测试在 baseline 下表现符合预期（增长无界、分配计数 >0）。

完成条件：

- 后续阶段不需要重新判读当前 GC 行为，可直接引用基线。

### P1. Pacing 核心：堆增长阈值触发

参考：
- [`GC_PACING.md`](./GC_PACING.md) “Pacing model”“Three trigger points”(1)、“Why a flag”、“Concurrency”、“Env knobs”、Phasing 1
- `runtime/c/scoop_runtime.c::scoop_alloc`、`scoop_gc_safepoint_poll`
- `runtime/c/scoop_gc_backend_immix.c`（`bytes_allocated` 计数与 cycle 末）
- `ScoopGcHeap` 结构定义

目标：

- 把无条件无界增长改成 `target = max(min_threshold, live * growth_factor)` 的按压力触发，默认开启。

必须实现的内容：

1. `ScoopGcHeap` 新增 `next_gc`（初值 = `min_threshold`，默认 4 MB）与 `request_collect` 标志/计数。
2. 每个 GC cycle 末（sweep 后，持 GC 锁）设置 `next_gc = max(min_threshold, live * growth_factor)`，`growth_factor` 默认 1.5。
3. alloc 快路径：`bytes_allocated_add` 后用 relaxed load 比较 `next_gc`，超过则 `request_collect`（幂等，置标志）。
4. `scoop_gc_safepoint_poll` 消费标志：在下一次 alloc 的 poll 处运行 collect，再分配——遵循“先 poll 后 alloc”的 root publication 纪律，不在 alloc 内同步 collect。
5. 新增 env 旋钮：`SCOOP_GC_PACING`（默认 on）、`SCOOP_GC_HEAP_TARGET_GROWTH_FACTOR`（1.5）、`SCOOP_GC_HEAP_MIN_THRESHOLD_BYTES`（4 MB）。`SCOOP_GC_STRESS` 激活时旁路 pacing。

必须遵从的约束：

- 默认 on；`off` 仅供需要确定性堆计数的测试。
- 触发只能经 safepoint，不在 `scoop_alloc` 内同步 collect（root publication / reentrancy）。
- `next_gc` 仅在 cycle 末更新；hot path 用 relaxed 原子，允许轻微 overshoot。

阶段输出：

- 长程序在默认配置下堆有界（约 `growth_factor * peak_live` + 一块 slop）。
- `SCOOP_GC_PACING=off` 保持旧的无界行为以供对照。

验证：

1. P0 的 10M 分配循环：默认配置下峰值堆有界；`PACING=off` 仍无界（证明 pacing 生效）。
2. `cargo test --all --all-targets`
3. 多线程并发分配：`request_collect` 不死锁，over-allocation 有界。

完成条件：

- 无 env 旋钮的默认运行不再无界增长。

### P2. Pacing 分代触发、OOM 防御与 backend parity

参考：
- [`GC_PACING.md`](./GC_PACING.md) “Three trigger points”(2)(3)、Phasing 2-5
- `runtime/c/scoop_runtime.c:563-567`（nursery 回退）
- `runtime/c/scoop_gc_immix_internal.h:548-575`（`scoop_gc_immix_state_take_block`）
- `runtime/c/scoop_gc_backend_hosted.c`、`scoop_gc_backend_minimal.c`

目标：

- 恢复分代实际收益，加上 block-pool 耗尽与 hard cap 的兜底，并让三个 backend 一致尊重 pacing。

必须实现的内容：

1. nursery 满 ⇒ minor GC：把 `scoop_runtime.c:563-567` 的静默回退改成“先 minor GC 再重试 nursery alloc；仍满才落 old space”（避免单对象大于 nursery 时死循环）。
2. block pool 耗尽 ⇒ full GC：`scoop_gc_immix_state_take_block` 两表空时先 full GC，取到 reusable/free 块则用，取不到才 `posix_memalign`。
3. hard cap：`SCOOP_GC_MAX_HEAP_BYTES`（默认 0=无 cap）。post-GC 重试仍超 cap，则 `scoop_alloc` 返回 NULL（上游已文档化 OOM⇒NULL，仅让其可达）。
4. backend parity：`hosted` / `minimal` 也读取并尊重 pacing 旋钮；阈值比较 backend 无关，即使其 collect 更受限。

必须遵从的约束：

- minor-GC-then-retry 不得在 nursery 真比单对象小的情况下死循环。
- hard cap 只在 post-GC 重试后才 OOM，不得在尚可回收时提前失败。
- 不改变 `scoop_gc_collect()` 手动调用与 `SCOOP_GC_STRESS` 语义。

阶段输出：

- 固定小 nursery 时不再永久卡满；分代分配模式恢复。
- 接近 hard cap 的程序仍能靠回收推进；真 OOM 干净返回 NULL。

验证：

1. 固定 `SCOOP_GC_IMMIX_NURSERY_BLOCKS=4` 的混合 live/dead workload：`gc_cycles` 递增、nursery 不永久满、`bytes_freed` 增长。
2. 紧 `SCOOP_GC_MAX_HEAP_BYTES`：贴 cap 分配仍成功（回收生效），超出干净返回 NULL。
3. `cargo test --all --all-targets`；三 backend 均编译可回归。

完成条件：

- pacing 的三层触发（软/分代/硬）与 hard cap 全部生效，长程序在受限堆下可持续运行。

### P3. `__AtomicInt` 升为 `@InteriorMutable struct`

参考：
- [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Interior mutability”“`scoop.unsafe.__AtomicInt`: typealias → marked struct”
- `sysroot/lib/scoop.unsafe/src/unsafe.scoop:163`、`sysroot/lib/scoop.core/src/core.scoop`（atomics）
- `crates/scoopc_hir/src/typecheck/lower.rs:2662,3522`、`.../typecheck/{annotations.rs,builtin_annotations.rs}`
- `crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/types.rs:436`、`crates/scoopc_hir/src/hir/lower/util/generic_layouts.rs:89`、`.../hir/lower/main/impl_lowering.rs:1724`

目标：

- 把 interior mutability 表达成一个**抗 aliasing、可被谓词读取的类型特征**，为 P5 的常量化谓词扫清隐藏陷阱。

必须实现的内容：

1. 新增 `@InteriorMutable` 注解（metadata-only，无 codegen），typecheck 识别并挂在 nominal 上。
2. `sysroot/lib/scoop.unsafe/src/unsafe.scoop`：`typealias __AtomicInt = Int` 改为
   ```scoop
   @InteriorMutable
   public struct __AtomicInt { val raw: Int }
   ```
   依赖普通单字段 struct 的派生布局（word）与派生构造器 `__AtomicInt(initial)`，不写专用 codegen。
3. 5 处擦除点从“类型 = `Int`”改为“类型 = `__AtomicInt` nominal、布局/ABI = `Int` word”；`__atomicIntLoad/Store/CompareExchange` 签名不变（按 lvalue 取存储当 i64 操作）。
4. `core.scoop` atomics 构造改显式：`var raw: __AtomicInt = __AtomicInt(initial)`、`= __AtomicInt(__atomicBoolToInt(initial))`；无隐式 Int↔__AtomicInt coerce。

必须遵从的约束：

- `__AtomicInt` 必须是与 `Int` 类型相异、布局相同的 nominal；aliases 解析回它时标记不丢。
- struct 只能 `val` 字段（语言刻意不放开 `var` struct 字段），`@InteriorMutable` 因此是 load-bearing。
- 不引入隐式 coerce；构造/load/store 三个面都显式。

阶段输出：

- `__AtomicInt` 是带 `@InteriorMutable` 的相异 struct，原子操作纪律落进类型。
- 现有 atomic 测试在显式构造下不变通过。

验证：

1. `cargo test --all --all-targets`
2. targeted：现有 atomics fixtures / 单元测试（`AtomicInt`/`AtomicBool`/`Atomic`）回归。
3. 类型层不再把 `__AtomicInt` 等同 `Int`（可加断言/测试）。

完成条件：

- P5 谓词可凭 `@InteriorMutable` 安全否决，无需名字匹配，且 `__AtomicInt` 不会被误当 Int 常量化。

### P4. Immortal 运行期支持与 content-hash 键

参考：
- [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Runtime change”“Cache and dedup keys”、Phasing 2-3
- `runtime/c/scoop_gc.h:210-244`、`scoop_gc_backend_immix.c:2719-2737,2739-2760,5177/5185`
- `crates/scoopc_codegen_llvm/src/llvm/codegen/main/alloca.rs:56-72`

目标：

- 让运行期能把带 immortal header 的对象视为透明（永不写、永不 trace），并把 byte 数组改成可跨 TU 折叠的 content-hash 键。这两步独立、无行为变化风险，先行落地。

必须实现的内容：

1. `scoop_gc.h` 新增 `#define SCOOP_GC_FLAG_IMMORTAL 0x80000000u` 与 `#define SCOOP_GC_MARK_IMMORTAL 0xFFFFFFFFu`。
2. `scoop_gc_mark_object_if_needed` 开头加 `if ((obj->flags & SCOOP_GC_FLAG_IMMORTAL) != 0) return;`，覆盖 pinned 扫描等不经 membership 预检的入口。slot visitor 不改（membership 已过滤堆外）。
3. `get_or_create_global_bytes`（`alloca.rs:56-72`）键从 `__scoop_str_data_{span.start}_{span.end}` 改为 `base16(SHA-256(bytes)[..16])`，并 `set_unnamed_addr(true)`。

必须遵从的约束：

- 运行期改动仅作用于带 flag 的对象；普通堆对象 marker 行为不变（flag-gated，非 blanket）。
- content-hash 改键不得改变现有字符串语义；只影响全局名与去重。

阶段输出：

- 运行期可安全承载 immortal ref 对象。
- byte 数组按内容去重、可被 linker 折叠。

验证：

1. runtime C 单元测试：构造一个带 `SCOOP_GC_FLAG_IMMORTAL` 的栈上 header，推上 mark stack，断言 `mark`/`flags` 字节不变（ASan 下）；同测一个堆 header 断言 `mark` 被更新。
2. golden-file：相同字面量在多处只产生一个 `__scoop_str_data_<hash>`。
3. `cargo test --all --all-targets`

完成条件：

- immortal flag 短路正确且 flag-gated；byte 数组 content-hash 键稳定。

### P5. 通用 `is_immutable` 谓词、`try_emit_immortal` 折叠器与 String immortal

参考：
- [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “The constantization predicate”“The generic immortal folder”“Emission shapes”“Deduplication”、Phasing 4
- `crates/scoopc_codegen_llvm/src/llvm/codegen/main/literal.rs:133-197`
- `crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/{const_pat.rs,terminator.rs}`
- `crates/scoopc_mir/src/mir/transport.rs`（`AggregateTransportMetadata` / `MirBoxingIntent`）
- `crates/scoopc_hir/src/hir/mod.rs:205`（`FieldDecl.mutable`）

目标：

- 用一个由类型特征驱动的通用决策替换三个手写 immortal 路径，让 String literal 零分配，并对 String 开内容池 dedup。

必须实现的内容：

1. `is_immutable(T)`（结构、递归、可 memo）：
   - 带 `@InteriorMutable` → false；
   - 值标量 → true；
   - 值 struct / tuple → 所有字段类型 `is_immutable`（struct 字段必 `val`，无需查可变性）；
   - ref class → 所有字段 `val` 且 所有字段类型 `is_immutable`。
2. `try_emit_immortal(value) -> Option<GlobalValue>`（content-hash 缓存）：
   - 标量 `ConstValue::*` → LLVM 标量常量；
   - `ConstValue::String/SynthString` 与 `TypeMetadataLiteral::TypeNameString` → immortal `ScoopString` 全局（带 header + `SCOOP_GC_FLAG_IMMORTAL` + `SCOOP_GC_MARK_IMMORTAL`，`next=null`）；
   - `Rvalue::StructLit/MakeTuple` 过提升门则发射常量聚合全局（值类型层无 header；ref 类型层带 header），否则 `None` 回退动态路径。
3. 提升门：① 字段全 `Operand::Const`；② 每字段 `transport.boxing.is_none()`；③ `transport.kind` 为 `Tuple`/`Struct`；④ ref 类型聚合再加 `is_immutable(aggregate_ty)`。
4. `codegen_string_literal_from_bytes` 与 `const_pat.rs` 的 String/SynthString 分支改走折叠器；`terminator.rs` 的 `StructLit`/`MakeTuple` arm 先试折叠器再回退。
5. dedup：String 走 content-pool（`__scoop_str_lit_<hash>`）；其它可常量化 ref 类型每个 literal site 一份全局，不跨站合并。

必须遵从的约束：

- 决策由类型特征驱动，不得退回逐类型特判或维护类型白名单。
- 折叠器遇到非平凡 transport（boxing / value-erasure）或非 `Const` 字段必须安全回退 `None`。
- 本版不追 `Local` → 定义它的 StructLit（嵌套聚合回退）；不含 `EnumVariant`。
- dedup 仅对 String；其它 ref 类型 per-site，保持身份观测不变。

阶段输出：

- String literal / `__type_name` 零 `scoop_alloc_typed`。
- `is_immutable` 正确区分 String / 全-val class（可常量化）与 atomics / `RefCell` / `__AtomicInt`（否决）。

验证：

1. codegen 单元：`codegen_string_literal_from_bytes("hello")` 零 `scoop_alloc_typed`；同函数两个 `"hello"` 引用同一全局。
2. `is_immutable` 单元：String 与合成全-val class 为 true；`RefCell`/`AtomicInt`（var）与 `__AtomicInt`（`@InteriorMutable`）为 false。
3. 集成：literal 在 10M 循环里打印，首个 cycle 后 `bytes_allocated` 零增长（除 print 自身）。
4. `cargo test --all --all-targets`、`python3 tools/run_fixtures.py`

完成条件：

- String 走通用 immortal 折叠器、零堆分配、按内容 dedup；谓词由类型特征驱动。

### P6. `Platform` 折叠与 `TypeMetadataLiteral` 审计

参考：
- [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Consumers (recast)”、Phasing 5-6
- `crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:186-201,203-292`
- HIR→MIR `TypedIntrinsicKind::Platform` lowering
- `sysroot/lib/scoop.core/src/core.scoop`（`Platform` struct）

目标：

- 让 `Platform` 作为通用机制的消费者自动落入折叠器，删除一切 Platform 专用 codegen；确认 `TypeMetadataLiteral` 消费者不 mutate。

必须实现的内容：

1. 在 HIR→MIR 把 `TypedIntrinsicKind::Platform` lower 成 `Rvalue::StructLit`，5 个字段为 `Operand::Const(ConstValue::SynthString(...))`，transport kind = `Struct`、各字段 `boxing: None`。
2. 删除 `codegen_platform_literal`（`transport.rs:203-292`）及任何 `get_or_create_immortal_platform_global` 专用 helper——不留 Platform 专用代码。
3. `Platform` 是值类型 struct，折叠器走值类型层（无 header），5 个字段引用 ref 层 immortal `ScoopString`。
4. `TypeMetadataLiteral` 审计：确认无消费者 mutate 其结果；新增断言两次 `__type_name(T)` 读返回指针相等的 `ScoopString`（dedup 后成立）。

必须遵从的约束：

- 不得保留 Platform 专用常量化路径；它必须与普通值 struct 常量化同路。
- Platform 聚合本身无 GC header（值类型），不进 ref-tier。

阶段输出：

- `Platform` 读取零 `scoop_alloc_typed`，由通用折叠器处理。
- `TypeMetadataLiteral` 不可变性经审计确认。

验证：

1. codegen 单元：`Platform` 访问零 `scoop_alloc_typed`。
2. `__type_name(T)` 两次读指针相等。
3. 集成：`Platform.os` 读 10M 次不触发 GC（与 P5 的零增长一致）。
4. `cargo test --all --all-targets`、`python3 tools/run_fixtures.py`

完成条件：

- Platform / TypeMetadata 完全收敛到通用机制，无任何专用常量化代码。

### P7. Spec / 文档 / fixtures 收尾与全量回归矩阵

参考：
- [`GC_PACING.md`](./GC_PACING.md) “Test plan”“Out of scope”
- [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Test plan”“Out of scope”
- `SCOOP_RUNTIME.md`、`docs/spec/**`、`tools/run_fixtures.py`、`tools/spec_fixtures.py`

目标：

- 把 P1-P6 的运行期与编译期行为反映到 runtime 文档、env 旋钮说明、fixtures 与回归矩阵，确保后续不需重新判读新行为。

必须实现的内容：

1. 更新 `SCOOP_RUNTIME.md`（及相关 spec 段）描述 pacing 模型、三层触发、env 旋钮（`SCOOP_GC_PACING` / `GROWTH_FACTOR` / `MIN_THRESHOLD_BYTES` / `MAX_HEAP_BYTES`）与默认 on 的姿态。
2. 记录 immortal 概念：值/ref 双层、`is_immutable` 谓词、`@InteriorMutable`、dedup 仅 String。
3. 审计哪些既有测试断言精确堆计数，给它们显式 `SCOOP_GC_PACING=off` 并注明原因（这同时是 pacing 的审计面）；确认 immortal 测试**不**需要 pacing off（immortal 不进堆）。
4. 全量验证：`cargo fmt`、`cargo test --all --all-targets`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`、runtime C 单元测试与长程序回归。
5. 明确归位 out-of-scope（不做、且为何不做）：incremental/concurrent GC、time-budget pacing、`.data` 单实例静态初始化与 static rooting、嵌套聚合 / `EnumVariant` 常量、跨类型 dedup、跨 `.cone` 字面量 dedup、嵌入式 tier 提示。

必须遵从的约束：

- 不得把未完成行为简单记成 future work；剩余项必须是明确超出两份设计文档的 v2+ 扩展。
- 需要 `PACING=off` 的测试必须注明 why。

阶段输出：

- runtime 文档、env 说明、fixtures 与 compiler/runtime 行为一致。
- 完整回归矩阵通过。

验证：

1. `cargo fmt`
2. `cargo test --all --all-targets`
3. `python3 tools/spec_fixtures.py check`
4. `python3 tools/run_fixtures.py`

完成条件：

- `GC_PACING.md` 与 `GC_IMMORTAL_FIX.md` 的目标行为成为运行期与编译期的实际 contract；旧的无界增长与 per-use wrapper 分配只存在于 `PACING=off` 对照与 design history 中。

## 6. 预期收口状态

- 默认配置下，长程序堆有界（`target = max(min_threshold, live * growth_factor)`），不再单调增长到 OOM；nursery 满触发 minor GC，block pool 耗尽先 full GC 再增长，`SCOOP_GC_MAX_HEAP_BYTES` 提供 hard cap。
- pacing 在 `immix` / `hosted` / `minimal` 三 backend 一致生效，默认 on，`SCOOP_GC_STRESS` 与手动 `scoop_gc_collect()` 语义不变。
- String literal、`__type_name(T)`、`Platform` 读取均不再在 GC 堆上分配 wrapper；它们走由 `is_immutable(T)` 驱动的通用常量化路径。
- “是否常量化”由类型传递不可变性决定，String / `Platform` / 用户全-val 不可变类型一视同仁；atomics / `RefCell` / `__AtomicInt` 由 `var` 字段或 `@InteriorMutable` 自动排除。
- dedup 仅作用于 String（内容池），其它可常量化 ref 类型 per-site 一份，身份观测不变；运行期 immortal 不变式为“永不写、永不 trace”。
- `scoop.unsafe.__AtomicInt` 是带 `@InteriorMutable` 的相异 struct（布局=Int），interior mutability 成为抗 aliasing 的类型特征，原子访问纪律落进类型系统。
- 不存在任何 Platform / String 专用常量化代码；新可常量化类型无需额外 compiler 改动即可获得提升。
- 测试矩阵覆盖：长程序堆有界、nursery/blockpool/hardcap 触发、immortal header 不被写、String/Platform 零分配与 dedup 指针相等。
