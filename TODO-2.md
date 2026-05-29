# TODO-2：P2 Pacing 分代触发、OOM 防御与 backend parity

> 索引：[`TODO.md`](./TODO.md)
> 计划基线：[`PLAN.md`](./PLAN.md)
> 覆盖阶段：P2
> 包目标：恢复分代实际收益，加上 block-pool 耗尽与 hard cap 兜底，并让三个 backend 一致尊重 pacing。

## P2：分代触发、OOM 防御与 backend parity

### [DONE] P2-T01：nursery 满触发 minor GC 再重试

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P2
  - [`GC_PACING.md`](./GC_PACING.md) “Three trigger points”(2)、Phasing 2
- 目标：
  - 把 nursery 满时的静默回退改成“先 minor GC 再重试 nursery alloc”，恢复分代收益。
- 必须修改的文件/位置：
  - `runtime/c/scoop_runtime.c:563-567`（nursery 满回退点）
  - `runtime/c/scoop_runtime.c:252-323`（`scoop_gc_immix_nursery_*`）
- 必须实现的内容：
  1. nursery 满（take_block 返回 NULL）时运行一次 minor collection，再重试 nursery alloc。
  2. 若 minor GC 后仍满，才回退 old-space（避免单对象大于 nursery 时死循环）。
  3. 保证 minor GC 经既有 STW/safepoint 机制，遵循 root publication 纪律。
- 必须遵从的约束：
  - 不得在 nursery 真小于单对象时死循环。
  - 不得改变 old-space 正确性；minor GC 仅收集 nursery。
- 验证：
  1. 固定 `SCOOP_GC_IMMIX_NURSERY_BLOCKS=4` 的混合 live/dead workload：`gc_cycles` 递增、nursery 不永久满、`bytes_freed` 增长。
  2. `cargo test --all --all-targets`
- 完成条件：
  - 分代分配模式恢复，nursery 不再一次满后永久满。
- 依赖：P1-T02R
- 完成记录：
  - 2026-05-29：已完成。
  - 实现：`scoop_alloc` 在 Immix nursery 分配失败后触发一次 `scoop_gc_collect_minor()`，随后重试 nursery 分配；若仍失败，才沿用 old-space fallback，避免大对象或无法恢复场景死循环。
  - 修正：minor GC 在移除 dead nursery 对象时调用 release callback 并累计 `bytes_freed`，保证回收统计与 pacing live-bytes 观测一致。
  - 测试：更新 `gc_immix_nursery` 覆盖 `SCOOP_GC_IMMIX_NURSERY_BLOCKS=4` 的 mixed live/dead workload，断言 `gc_cycles` 递增、`bytes_freed` 增长、nursery 自动 minor 后仍可继续分配；调整 `gc_immix_write_barrier` 以适配 nursery-full 不再静默 fallback 的行为。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test -p scoop_runtime --test gc_immix_nursery -- --nocapture`；`cargo test -p scoop_runtime --test gc_immix_write_barrier -- --nocapture`；`cargo test -p scoop_runtime --test gc_immix_minor_collect -- --nocapture`；`cargo test --all --all-targets`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] P2-T01R：Review nursery-full minor GC

- 参考：
  - P2-T01 完成记录
  - [`GC_PACING.md`](./GC_PACING.md) “Three trigger points”(2)
- 目标：
  - 复核 minor-GC-then-retry 是否正确、不死循环。
- 必须检查的文件/位置：
  - P2-T01 对 nursery 回退路径的改动
- 必须实现的内容：
  1. 确认单对象大于 nursery 时正确回退 old-space，不死循环。
  2. 确认 minor GC 不破坏 old-space。
  3. 运行固定小 nursery 的回归确认 nursery 周期性释放。
- 必须遵从的约束：
  - 若存在死循环或 old-space 破坏风险，必须修正后才进入 P2-T02。
- 验证：
  1. `cargo test --all --all-targets`
- 完成条件：
  - 分代触发正确。
- 依赖：P2-T01
- 完成记录：
  - 2026-05-29：已完成。
  - Review 结论：P2-T01 的 nursery-full minor-GC-then-retry 路径不死循环；单对象大于 Immix block payload 时走 large/fallback old-space 分配；minor GC 不再破坏 old-space block free/reusable 链表。
  - 修正：write barrier 的 old→nursery 判定改为基于 heap membership 与 Immix `all_blocks` containment，避免对 large/fallback malloc 对象使用地址掩码反推 block；old/fallback 对象写入 nursery 引用时会晋升 nursery block。
  - 修正：promote-on-store 与 pinned nursery promotion 会递归晋升 nursery 引用闭包，维持 minor GC “不扫描 old-space 字段”所依赖的无 old→nursery 边不变式。
  - 修正：minor reset 仅清理 nursery blocks 的 `next_free`，不再清空 old-space block lists；major mark/sweep/compaction 与 reserved-bytes 统计改用 Immix state containment，避免 large/fallback 对象误判或崩溃。
  - 测试：新增 `gc_immix_minor_old_edges` 覆盖 large/fallback old object store、跨 nursery block 引用闭包晋升以及后续 minor 不回收被 old object 引用的对象；保留并运行 fixed small nursery 回归。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test -p scoop_runtime --test gc_immix_minor_old_edges -- --test-threads=1 --nocapture`；`cargo test -p scoop_runtime --test gc_immix_nursery -- --test-threads=1 --nocapture`；`cargo test -p scoop_runtime --test gc_immix_write_barrier -- --test-threads=1 --nocapture`；`cargo test -p scoop_runtime --test gc_immix_minor_collect -- --test-threads=1 --nocapture`；`cargo test --all --all-targets`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] P2-T02：block pool 耗尽先 full GC 再增长

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P2
  - [`GC_PACING.md`](./GC_PACING.md) “Three trigger points”(3)、Phasing 3
- 目标：
  - block pool 两表空时先 full GC 回收，取不到块才 `posix_memalign` 增长。
- 必须修改的文件/位置：
  - `runtime/c/scoop_gc_immix_internal.h:548-575`（`scoop_gc_immix_state_take_block`）
- 必须实现的内容：
  1. `reusable_blocks` 与 `free_blocks` 均空时，先运行 full collection。
  2. 回收后若有 reusable/free 块则取用；仍无才 `scoop_gc_immix_block_alloc_new`。
  3. 保证该路径在 collection reentrancy 下安全（collection 自身分配辅助结构）。
- 必须遵从的约束：
  - 不得在尚可回收时提前增长；也不得在确实需要增长时拒绝。
- 验证：
  1. 紧堆 workload：贴近可回收上限时分配仍成功（回收生效）。
  2. `cargo test --all --all-targets`
- 完成条件：
  - block pool 耗尽有 full-GC 回退，不再无条件增长。
- 依赖：P2-T01R
- 完成记录：
  - 2026-05-29：已完成。
  - 实现：`scoop_gc_immix_state_take_block` 拆出 reusable/free 取块重试逻辑；当 block pool 两表均空时，持锁路径会先释放 Immix lock、运行一次 full GC、重新加锁并重试 reusable/free blocks，仍无可用块时才 `scoop_gc_immix_block_alloc_new` 增长。
  - Reentrancy：新增 `collection_depth` 标记 major/minor collector 持有 heap 的阶段，collector 内部分配路径不会递归触发 full GC；`scoop_alloc` 的 TLS cache refill 通过 `did_block_pool_collect` 保证单次分配最多触发一次 block-pool full GC。
  - 测试：新增 `gc_immix_block_pool` 覆盖紧堆 old-space workload，断言 block-pool exhaustion 会增加 `gc_cycles` 且不增加 reserved bytes；同步修正受 hard trigger 影响的 STW/native-blocking 测试、pacing-env 断言与同进程 runtime-global 测试隔离，使其遵守 P2 hard-trigger contract。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test -p scoop_runtime --test gc_immix_block_pool -- --test-threads=1 --nocapture`；`cargo test -p scoop_runtime --test gc_enter_native -- --test-threads=1 --nocapture`；`cargo test -p scoop_runtime --test gc_immix_mark_region -- --test-threads=1 --nocapture`；`cargo test -p scoop_runtime --test gc_immix_parallel_mark -- --test-threads=1 --nocapture`；`cargo test -p scoop_runtime --test gc_immix_parallel_mark_sweep_stress -- --test-threads=1 --nocapture`；`cargo test -p scoop_runtime --test gc_immix_allocator -- --test-threads=1 --nocapture`；`cargo test -p scoop_runtime --test gc_pacing_env -- --test-threads=1 --nocapture`；`cargo test -p scoop_runtime --test gc_stackmap_multiframe_keepalive -- --test-threads=1 --nocapture`；`cargo test -p scoop_runtime --test gc_stop_the_world -- --test-threads=1 --nocapture`；`cargo test -p scoop_runtime --test mutable_array_runtime -- --nocapture`；`cargo test --all --all-targets`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] P2-T02R：Review block pool 回退

- 参考：
  - P2-T02 完成记录
- 目标：
  - 复核 full-GC-before-grow 的正确性与 reentrancy 安全。
- 必须检查的文件/位置：
  - P2-T02 对 `scoop_gc_immix_state_take_block` 的改动
- 必须实现的内容：
  1. 确认 collection reentrancy 下不递归崩溃。
  2. 确认回收无效时仍能正确增长。
- 必须遵从的约束：
  - 若回退路径有 reentrancy 或正确性风险，必须修正后才进入 P2-T03。
- 验证：
  1. `cargo test --all --all-targets`
- 完成条件：
  - block pool 回退正确。
- 依赖：P2-T02
- 完成记录：
  - 2026-05-29：已完成。
  - Review 结论：P2-T02 的 block-pool fallback 在 reusable/free 两表耗尽时先释放 Immix lock 并触发一次 full GC，collector 持有 heap 时通过 `collection_depth` 跳过递归触发，单次 mutator 分配通过 `did_block_pool_collect` 避免重复 full GC。
  - 覆盖补强：新增 `immix_block_pool_exhaustion_grows_when_collection_cannot_reclaim`，用 pinned live old-space blocks 证明 full GC 无法回收时 allocation 仍会在一次 GC 后增长，而不是误报 OOM 或递归崩溃；保留原紧堆 reclaim workload 证明可回收时不提前增长。
  - 验证：`cargo fmt`；`cargo test -p scoop_runtime --test gc_immix_block_pool --features gc-immix -- --test-threads=1 --nocapture`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] P2-T03：接入 hard cap 与 OOM 返回

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P2
  - [`GC_PACING.md`](./GC_PACING.md) “Three trigger points”(3)、Phasing 4
- 目标：
  - 接入 `SCOOP_GC_MAX_HEAP_BYTES`，post-GC 重试仍超 cap 时让 `scoop_alloc` 返回 NULL。
- 必须修改的文件/位置：
  - `runtime/c/scoop_gc_immix_internal.h:548-575`（增长前的 cap 检查）
  - `runtime/c/scoop_runtime.c::scoop_alloc`（`:514` OOM⇒NULL 路径）
- 必须实现的内容：
  1. 接入 `SCOOP_GC_MAX_HEAP_BYTES`（默认 0=无 cap）。
  2. 在 P2-T02 的 post-GC 重试之后，若仍超 cap 则不增长，`scoop_alloc` 返回 NULL。
  3. 确认上游 OOM⇒NULL 行为不变，仅让其可达。
- 必须遵从的约束：
  - hard cap 只在 post-GC 重试后才 OOM，不得在尚可回收时提前失败。
- 验证：
  1. 紧 `SCOOP_GC_MAX_HEAP_BYTES`：贴 cap 分配靠回收成功，超出干净返回 NULL。
  2. `cargo test --all --all-targets`
- 完成条件：
  - hard cap 生效，真 OOM 干净返回 NULL。
- 依赖：P2-T02R
- 完成记录：
  - 2026-05-29：已完成。
  - 实现：新增 `SCOOP_GC_MAX_HEAP_BYTES` runtime env 解析并写入 heap 配置，默认 `0` 表示无 cap；Immix block-pool 在 P2-T02 的 full-GC retry 后若仍需增长且会超过 cap，则拒绝 `posix_memalign` 新 block，让 `scoop_alloc` 走既有 `NULL` OOM 返回路径。
  - 完整性：reserved heap 统计同时计入 Immix blocks 与 large/fallback malloc 对象；large-object malloc fallback 也会先 full GC 再判定 hard cap，避免绕过 `SCOOP_GC_MAX_HEAP_BYTES`。
  - 测试：新增 `gc_immix_hard_cap`，覆盖 tight cap 下 unrooted block 经 full GC 回收后分配成功、live/pinned block 超 cap 时干净返回 `NULL`，以及 large-object fallback 遵守 hard cap。
  - 验证：`cargo fmt`；`cargo test -p scoop_runtime --test gc_immix_hard_cap --features gc-immix -- --test-threads=1 --nocapture`；`cargo test -p scoop_runtime --test gc_immix_block_pool --features gc-immix -- --test-threads=1 --nocapture`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`；`cargo test -p scoop_runtime --no-default-features --features gc-baseline`；`cargo test -p scoop_runtime --no-default-features --features gc-minimal`；`cargo test -p scoop_runtime --no-default-features --features gc-hosted`。

### [TODO] P2-T03R：Review hard cap

- 参考：
  - P2-T03 完成记录
- 目标：
  - 复核 hard cap 时序（仅 post-GC 重试后 OOM）与 NULL 返回路径。
- 必须检查的文件/位置：
  - P2-T03 的 cap 检查与 OOM 返回改动
- 必须实现的内容：
  1. 确认 cap 不在尚可回收时提前触发。
  2. 确认 OOM 返回 NULL 后上游处理无 UB。
- 必须遵从的约束：
  - 若 cap 提前失败或 NULL 路径有 UB，必须修正后才进入 P2-T04。
- 验证：
  1. `cargo test --all --all-targets`
- 完成条件：
  - hard cap 行为正确。
- 依赖：P2-T03
- 完成记录：
  - （待执行）

### [TODO] P2-T04：hosted/minimal backend pacing parity

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P2
  - [`GC_PACING.md`](./GC_PACING.md) Phasing 5
- 目标：
  - 让 `hosted` / `minimal` backend 也读取并尊重 pacing 旋钮（阈值比较 backend 无关）。
- 必须修改的文件/位置：
  - `runtime/c/scoop_gc_backend_hosted.c`、`runtime/c/scoop_gc_backend_minimal.c`
- 必须实现的内容：
  1. 在两个 backend 接入 pacing 旋钮与阈值比较；即使其 `scoop_gc_collect` 更受限（如 hosted 多线程下 no-op）也保持旋钮一致。
  2. 确认三 backend 的 env 旋钮语义一致。
- 必须遵从的约束：
  - 不得让某 backend 因 pacing 旋钮而崩溃或回归；阈值比较与触发请求 backend 无关。
- 验证：
  1. 三 backend 均编译可回归。
  2. `cargo test --all --all-targets`
- 完成条件：
  - pacing 在三 backend 一致生效，pacing 线收口。
- 依赖：P2-T03R
- 完成记录：
  - （待执行）

### [TODO] P2-T04R：Review backend parity

- 参考：
  - P2-T04 完成记录
- 目标：
  - 复核三 backend pacing 行为一致、无回归。
- 必须检查的文件/位置：
  - P2-T04 对 hosted/minimal backend 的改动
- 必须实现的内容：
  1. 确认旋钮语义在三 backend 一致。
  2. 确认无 backend 因改动崩溃或回归。
- 必须遵从的约束：
  - 若任一 backend 回归，必须修正后才进入 TODO-3。
- 验证：
  1. `cargo test --all --all-targets`
- 完成条件：
  - pacing 全线收口（P1-P2 完成）。
- 依赖：P2-T04
- 完成记录：
  - （待执行）
