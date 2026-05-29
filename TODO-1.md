# TODO-1：P0-P1 行为基线冻结与 pacing 核心

> 索引：[`TODO.md`](./TODO.md)
> 计划基线：[`PLAN.md`](./PLAN.md)
> 覆盖阶段：P0-P1
> 包目标：核对并冻结 pacing/immortal 当前行为与度量基线，并落地 pacing 核心触发，使长程序在默认配置下堆有界。

## P0：冻结当前行为与最小回归基线

### [DONE] P0-T01：核对并冻结 pacing 当前行为基线

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P0
  - [`GC_PACING.md`](./GC_PACING.md) “Current behavior (verified)”
- 目标：
  - 在改运行期行为前，固定 pacing 缺失点的当前路径与行号，供 P1-P2 直接引用，不再重复全仓核对。
- 必须检查的文件/位置：
  - `runtime/c/scoop_runtime.c`（`scoop_alloc`、`scoop_gc_safepoint_poll`、`:502-507` stress、`:563-567` nursery 回退、`:514` OOM⇒NULL）
  - `runtime/c/scoop_gc_immix_internal.h:548-575`（`scoop_gc_immix_state_take_block`）、`:283-299`（`block_alloc_new`）
  - `runtime/c/scoop_gc_backend_immix.c:78-79,2416,2524,5382-5388`（`bytes_allocated` 计数点）
  - `runtime/c/scoop_runtime_api.h:37-38`（`scoop_gc_collect` API）
  - `ScoopGcHeap` 结构定义
  - `grep getenv runtime/c/` 的 env 旋钮全集
- 必须实现的内容：
  1. 在本条完成记录写出 pacing 侧关键行号与行为摘要：唯一生产触发（stress）、block pool 无条件增长、nursery 静默回退、`bytes_allocated` 只计数不比较、现有 env 旋钮集合。
  2. 确认 `ScoopGcHeap` 当前没有 `next_gc` / 目标堆字段，记录 P1 应新增字段的落点。
  3. 记录 safepoint poll 当前在 `scoop_alloc` 中的位置与时序，作为 P1 触发挂载点。
- 必须遵从的约束：
  - 本任务不改运行期行为；只做核对与记录。
  - 不得删除或弱化现有 GC 测试来掩盖当前无界增长。
- 验证：
  1. `cargo test --all --all-targets`
  2. 对完成记录中的行号/路径做人工抽样复核。
- 完成条件：
  - P1-P2 可直接从本条记录读取 pacing 触发挂载点与现状，无需重新核对。
- 依赖：无
- 完成记录：
  - 2026-05-29：已核对并冻结 pacing 当前行为基线；本任务只更新记录与当前行为文档，未修改运行期行为。
  - 生产触发面：`runtime/c/scoop_runtime_api.h:37-38` 只把 `scoop_gc_collect` / `scoop_gc_collect_minor` 暴露为 public C API；排除 `scoop_test_*` smoke/test helper 后，唯一自动触发 collection 的生产分配路径是 `runtime/c/scoop_runtime.c:501-507` 的 `SCOOP_GC_STRESS` 测试开关，在分配前按计数调用 `scoop_gc_collect()`。
  - safepoint poll 挂载点：`runtime/c/scoop_runtime.c:483-499` 中 `scoop_alloc` 先保证 runtime init 与线程注册，再在 `:493-499` 调用 `scoop_gc_safepoint_poll()`；该 poll 位于 stress 触发、size 规范化、底层分配和 heap 登记之前。P1 的 requested-collect 消费点应挂在 `scoop_gc_safepoint_poll` 语义上，使下一次 allocation 在分配新对象前收集；不得在对象尚未发布到 roots 的 alloc 内同步 collect。
  - block pool 当前无回收回退：`runtime/c/scoop_runtime.c:224-240` 的 TLS cache refill 持 `state->lock` 调 `scoop_gc_immix_state_take_block`，分配路径在 `runtime/c/scoop_runtime.c:574-584` cache 空时直接 refill；`runtime/c/scoop_gc_immix_internal.h:548-575` 依次取 `reusable_blocks`、`free_blocks`，两者为空时在 `:565` 调 `scoop_gc_immix_block_alloc_new()`，而该函数在 `:283-299` 直接 `posix_memalign` 新 32KB block。当前没有 “pool 空先 full GC 再增长” 的路径。
  - nursery 当前静默回退：`runtime/c/scoop_runtime.c:252-323` 的 nursery allocator 在 `state->nursery_blocks >= state->nursery_max_blocks` 时返回 NULL；调用方 `runtime/c/scoop_runtime.c:562-567` 只尝试 nursery alloc，若 `p == 0` 会继续走 `:569-603` 的 old-space block allocator。当前没有 nursery-full 触发 minor GC、retry，再决定回退 old 的路径。
  - `bytes_allocated` 当前只是观测计数：Immix backend 在 `runtime/c/scoop_gc_backend_immix.c:78-79` 以 relaxed atomic add 递增，唯一分配登记调用点是 `:2409-2417` 的 `scoop_gc_heap_register_object`；`runtime/c/scoop_gc_backend_immix.c:2473-2527` heap init 时初始化为 0，debug 读取点是 `:5382-5389`。`grep bytes_allocated runtime/c/` 未发现任何阈值比较或 pacing 触发字段；当前也没有 cycle 末根据 live/allocated 更新 `next_gc` 的逻辑。
  - `ScoopGcHeap` 当前字段：`runtime/c/scoop_gc.h:270-277` 只有 `objects`、`free_list`、`bytes_allocated`、`bytes_freed`、`gc_cycles`；没有 `next_gc`、target heap、growth factor 或 `request_collect` 字段。P1 应在该结构附近新增 pacing 目标与幂等 request 标志，并在 `scoop_gc_heap_init` 初始化默认值。
  - 现有 env 旋钮集合（按 `grep getenv runtime/c/` 与调用点核对）：`SCOOP_GC_STRESS`（`runtime/c/scoop_runtime.c:135-159`）；Immix nursery `SCOOP_GC_IMMIX_NURSERY_BYTES` / `SCOOP_GC_IMMIX_NURSERY_BLOCKS`（`runtime/c/scoop_gc_backend_immix.c:2448-2470`）；Immix parallel mark/sweep `SCOOP_GC_IMMIX_PARALLEL_MARK`（`:3022-3050`）与 `SCOOP_GC_IMMIX_PARALLEL_SWEEP`（`:3053-3081`）；GC diagnostics `SCOOP_GC_VERIFY_ROOTS`（Immix `:3084-3113`，baseline `runtime/c/scoop_gc.c:1361-1394`）；baseline moving `SCOOP_GC_MOVE`（`runtime/c/scoop_gc.c:1388-1390`）；stackmap strict `SCOOP_STACKMAP_STRICT`（`runtime/c/scoop_stackmap.c:550-570`）。`runtime/c/platform/platform_posix.c:30` 是通用 platform env wrapper，不是固定 GC pacing knob。当前没有 `SCOOP_GC_PACING`、heap target、growth factor、min-threshold 或 max-heap hard cap 旋钮。
  - 人工抽样复核：已读取并核对 `scoop_alloc` poll/stress/nursery fallback、`scoop_gc_immix_state_take_block`、`scoop_gc_immix_block_alloc_new`、`scoop_gc_heap_register_object`、`ScoopGcHeap` 定义、`scoop_gc_debug_heap_bytes_allocated` 与 `getenv` 命中。P1-P2 可直接引用上述挂载点与缺失点。
  - 验证：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`python3 tools/run_fixtures.py` 均已通过；fixture 汇总 `fixtures: ok (1607)`。

### [TODO] P0-T01R：Review pacing 行为基线

- 参考：
  - P0-T01 完成记录
  - [`GC_PACING.md`](./GC_PACING.md) “Current behavior (verified)”
- 目标：
  - 独立复核 pacing 行为基线是否准确、可执行，足以支撑 P1-P2。
- 必须检查的文件/位置：
  - P0-T01 完成记录中的所有路径与行号
- 必须实现的内容：
  1. 抽样复核行号是否仍指向描述的代码。
  2. 确认 P1 的触发挂载点（safepoint poll、cycle 末更新点、alloc 计数点）已被准确标定。
  3. 如发现漏项或行号漂移，直接修正记录。
- 必须遵从的约束：
  - Review 不得只看格式；必须复核基线可执行性。若 P0-T01 未达目标，阻塞 P0-T02。
- 验证：
  1. `cargo test --all --all-targets`
- 完成条件：
  - pacing 行为基线准确可用。
- 依赖：P0-T01
- 完成记录：
  - （待执行）

### [TODO] P0-T02：核对并冻结 immortal 当前行为基线

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P0
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Current behavior (verified)”“Interior mutability”
- 目标：
  - 固定 immortal 分配点、marker 写路径、`__AtomicInt` typealias 与 5 处擦除点的现状，供 P3-P6 引用。
- 必须检查的文件/位置：
  - `crates/scoopc_codegen_llvm/src/llvm/codegen/main/literal.rs:133-197`、`alloca.rs:56-72`
  - `crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:186-201,203-292`
  - `runtime/c/scoop_gc.h:210-244`、`scoop_gc_backend_immix.c:2719-2737,2739-2760,5177/5185`
  - `sysroot/lib/scoop.unsafe/src/unsafe.scoop:163`、`sysroot/lib/scoop.core/src/core.scoop`（atomics）
  - 擦除点：`crates/scoopc_hir/src/typecheck/lower.rs:2662,3522`、`scoopc_codegen_llvm/.../mir_body/types.rs:436`、`scoopc_hir/src/hir/lower/util/generic_layouts.rs:89`、`.../hir/lower/main/impl_lowering.rs:1724`
- 必须实现的内容：
  1. 记录 String literal / TypeMetadataLiteral / Platform 当前各自的分配路径与 `scoop_alloc_typed` 调用点。
  2. 记录 marker 无条件写 `mark` 与 heap-membership 过滤的位置（immortal 透明性支点）。
  3. 记录 `__AtomicInt` typealias 与全部擦除点，标注 P3 要改成“类型相异、布局=Int”的具体落点。
  4. 记录 `core.scoop` 中 atomics 的 `var raw` 构造点，标注 P3 要改成显式构造的位置。
- 必须遵从的约束：
  - 本任务不改任何行为；只核对记录。
- 验证：
  1. `cargo test --all --all-targets`
  2. 对完成记录中的行号/路径做人工抽样复核。
- 完成条件：
  - P3-P6 可直接读取 immortal 现状与落点。
- 依赖：P0-T01R
- 完成记录：
  - （待执行）

### [TODO] P0-T02R：Review immortal 行为基线

- 参考：
  - P0-T02 完成记录
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Current behavior (verified)”
- 目标：
  - 复核 immortal 基线是否覆盖三类分配点、marker 路径与全部 `__AtomicInt` 擦除点。
- 必须检查的文件/位置：
  - P0-T02 完成记录中的所有路径与行号
- 必须实现的内容：
  1. 反向 grep 确认 `__AtomicInt` 的擦除点没有遗漏（5 处之外是否还有别处把它当 Int 处理）。
  2. 确认 marker 写与 membership 过滤的描述准确。
  3. 如有漏项直接补记录。
- 必须遵从的约束：
  - 若 P0-T02 未达目标，阻塞 P0-T03。
- 验证：
  1. `cargo test --all --all-targets`
- 完成条件：
  - immortal 基线完整可用。
- 依赖：P0-T02
- 完成记录：
  - （待执行）

### [TODO] P0-T03：建立堆增长与字面量分配计数度量

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P0
  - [`GC_PACING.md`](./GC_PACING.md) “Test plan”、[`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Test plan”
- 目标：
  - 建立两个可复用度量：长程序 `bytes_allocated` 峰值曲线，以及 String/Platform literal 的 `scoop_alloc_typed` 计数；作为后续阶段前后对比基线。
- 必须检查的文件/位置：
  - `runtime/c/` 现有 `scoop_gc_debug_*`（如 `scoop_gc_debug_heap_bytes_allocated`）
  - `tests/`、`runtime/c/` 现有 GC 单元测试组织方式
  - `tools/run_fixtures.py` 的 fixture 表达能力
- 必须实现的内容：
  1. 新增/接入一个长程序度量：10M 小对象分配循环，记录峰值堆。baseline 下应观察到无界增长（用于 P1 前后对比，不要求此时收敛）。
  2. 新增一个分配计数度量：仅含 String literal / `Platform` 读取的函数，统计其 IR 中 `scoop_alloc_typed` 次数（baseline 下 >0）。
  3. 在完成记录中写明这两个度量的运行方式与 baseline 数值。
- 必须遵从的约束：
  - 度量不得改变运行期/编译期行为；若引入测试，baseline 下不能让全量 suite 失败（必要时标注为度量/诊断用途，不计入 pass/fail 断言）。
- 验证：
  1. `cargo test --all --all-targets`
  2. 两个度量在 baseline 下表现符合预期（增长无界、计数 >0）。
- 完成条件：
  - 后续 P1（堆有界）与 P5/P6（零分配）可直接复用这两个度量做前后对比。
- 依赖：P0-T02R
- 完成记录：
  - （待执行）

### [TODO] P0-T03R：Review 度量基线

- 参考：
  - P0-T03 完成记录
- 目标：
  - 复核两个度量是否真实反映“无界增长”和“per-use 分配”，可作为后续阶段的客观对比。
- 必须检查的文件/位置：
  - P0-T03 新增/接入的度量代码与运行方式
- 必须实现的内容：
  1. 实际运行两个度量，确认 baseline 数值与记录一致。
  2. 确认度量不会在 baseline 下破坏全量 suite。
- 必须遵从的约束：
  - 若度量不可复现或不反映目标现象，阻塞 P1-T01。
- 验证：
  1. `cargo test --all --all-targets`
- 完成条件：
  - 度量可复现、可作为前后对比基线。
- 依赖：P0-T03
- 完成记录：
  - （待执行）

## P1：Pacing 核心：堆增长阈值触发

### [TODO] P1-T01：实现 pacing 核心 next_gc + request_collect + safepoint + 阈值

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P1
  - [`GC_PACING.md`](./GC_PACING.md) “Pacing model”“Three trigger points”(1)、“Why a flag”、“Concurrency”、Phasing 1
- 目标：
  - 把无条件无界增长改为 `target = max(min_threshold, live * growth_factor)` 的按压力触发，经 safepoint 落地。
- 必须修改的文件/位置：
  - `ScoopGcHeap` 结构（新增 `next_gc`、`request_collect` 标志/计数）
  - `runtime/c/scoop_runtime.c::scoop_alloc`、`scoop_gc_safepoint_poll`
  - `runtime/c/scoop_gc_backend_immix.c`（cycle 末 sweep 后更新 `next_gc`、alloc 计数点）
- 必须实现的内容：
  1. `ScoopGcHeap` 新增 `next_gc`（初值 = `min_threshold`，默认 4 MB）与幂等 `request_collect` 标志。
  2. 每个 GC cycle 末（sweep 后、持 GC 锁）设 `next_gc = max(min_threshold, live * growth_factor)`，`growth_factor` 默认 1.5。
  3. alloc 快路径：`bytes_allocated_add` 后用 relaxed load 比较 `next_gc`，超过则 `request_collect`（置标志，不在 alloc 内同步 collect）。
  4. `scoop_gc_safepoint_poll` 消费标志：在下一次 alloc 的 poll 处运行 collect 再分配，遵循“先 poll 后 alloc”的 root publication 纪律。
- 必须遵从的约束：
  - 触发只能经 safepoint；不得在 `scoop_alloc` 内同步 collect（root publication / reentrancy）。
  - `next_gc` 仅在 cycle 末更新；hot path 用 relaxed 原子，允许一个对象的轻微 overshoot。
  - `request_collect` 幂等，collection 进行中为 no-op。
- 验证：
  1. P0-T03 的 10M 循环度量：默认配置下峰值堆有界（约 `growth_factor * peak_live` + 一块 slop）。
  2. `cargo test --all --all-targets`
  3. 多线程并发分配不死锁，over-allocation 有界。
- 完成条件：
  - 默认运行不再无界增长（env 旋钮在 P1-T02 接入，本任务可临时硬编码默认值）。
- 依赖：P0-T03R
- 完成记录：
  - （待执行）

### [TODO] P1-T01R：Review pacing 核心

- 参考：
  - P1-T01 完成记录
  - [`GC_PACING.md`](./GC_PACING.md) “Why a flag”、“Concurrency”
- 目标：
  - 复核触发是否经 safepoint、root publication 是否安全、并发是否正确。
- 必须检查的文件/位置：
  - P1-T01 对 `ScoopGcHeap`、`scoop_alloc`、`scoop_gc_safepoint_poll`、cycle 末更新点的改动
- 必须实现的内容：
  1. 确认没有在 alloc 内同步 collect；新对象的 root publication 不被破坏。
  2. 确认 `next_gc` 仅 cycle 末更新、并发读为 relaxed 且安全。
  3. 运行长程序度量确认堆有界，`PACING` 临时关闭路径仍可无界（证明 pacing 生效）。
- 必须遵从的约束：
  - 若触发路径破坏 root publication 或 reentrancy，必须修正后才进入 P1-T02。
- 验证：
  1. `cargo test --all --all-targets`
- 完成条件：
  - pacing 核心正确且默认堆有界。
- 依赖：P1-T01
- 完成记录：
  - （待执行）

### [TODO] P1-T02：接入 pacing env 旋钮与默认 on，并加长程序有界回归

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P1
  - [`GC_PACING.md`](./GC_PACING.md) “Env knobs”、Phasing 1
- 目标：
  - 把 pacing 参数暴露为 env 旋钮（默认 on），并固化长程序有界回归测试。
- 必须修改的文件/位置：
  - `runtime/c/scoop_runtime.c`（env 读取，复用现有 `getenv` 模式）
  - GC 单元测试目录
- 必须实现的内容：
  1. 接入 `SCOOP_GC_PACING`（默认 on）、`SCOOP_GC_HEAP_TARGET_GROWTH_FACTOR`（1.5）、`SCOOP_GC_HEAP_MIN_THRESHOLD_BYTES`（4 MB）。
  2. `SCOOP_GC_STRESS` 激活时旁路 pacing（stress 已比 pacing 收集更频繁）。
  3. 新增长程序回归：默认配置下峰值堆有界；`SCOOP_GC_PACING=off` 下保持旧的无界行为（作为对照与确定性堆计数测试的出口）。
- 必须遵从的约束：
  - 默认 on；`off` 仅供测试，且后续（P7）需对用到它的测试注明 why。
  - 不改变 `scoop_gc_collect()` 手动调用与 `SCOOP_GC_STRESS` 语义。
- 验证：
  1. 新增长程序回归：on 有界、off 无界。
  2. `cargo test --all --all-targets`
- 完成条件：
  - pacing 旋钮齐备、默认 on，长程序有界有回归覆盖。
- 依赖：P1-T01R
- 完成记录：
  - （待执行）

### [TODO] P1-T02R：Review pacing env 旋钮与有界回归

- 参考：
  - P1-T02 完成记录
  - [`GC_PACING.md`](./GC_PACING.md) “Env knobs”
- 目标：
  - 复核旋钮默认值、stress 旁路与有界回归的有效性。
- 必须检查的文件/位置：
  - P1-T02 的 env 读取与新增回归测试
- 必须实现的内容：
  1. 确认默认 on、默认值与设计一致，`off` 能恢复无界行为。
  2. 确认 stress 激活时 pacing 被旁路。
  3. 确认长程序回归在 on/off 下表现符合预期。
- 必须遵从的约束：
  - 若默认未 on 或旋钮语义偏离设计，必须修正后才进入 TODO-2。
- 验证：
  1. `cargo test --all --all-targets`
- 完成条件：
  - pacing 线核心收口，长程序在默认配置下可持续运行。
- 依赖：P1-T02
- 完成记录：
  - （待执行）
