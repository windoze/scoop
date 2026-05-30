# TODO-5：P7 文档、fixtures 收尾与全量回归矩阵

> 索引：[`TODO.md`](./TODO.md)
> 计划基线：[`PLAN.md`](./PLAN.md)
> 覆盖阶段：P7
> 包目标：把 P1-P6 的运行期与编译期行为反映到 runtime 文档、env 旋钮说明、fixtures 与回归矩阵，明确归位 out-of-scope，确保后续不需重新判读新行为。

## P7：Spec / 文档 / fixtures 收尾与回归矩阵

### [DONE] P7-T00：增强 STW 健壮性避免僵尸线程卡死

- 参考：
  - `runtime/c/scoop_gc_backend_immix.c`：`scoop_gc_stop_the_world_begin_prepare_unlocked` / `scoop_gc_stop_the_world_begin_unlocked` / `scoop_gc_thread_unregister` / `scoop_gc_safepoint_common`
  - 现象记录（`gc-fix` 分支调查；复现该现象的 `gc_immix_parallel_mark_sweep_stress` 测试因属于 native 调用方无法安全保活 fresh 分配的不可修场景，已在本分支删除，仅保留以下文字记录）：某 worker 断言 panic 后未调用 `scoop_thread_unregister` 即退出，其线程记录仍以 `Running` 留在 `scoop_gc_threads`，导致后续 STW 永久等待该“僵尸线程” park（STW 诊断 dump：`waiting for park: ... state=Running last_epoch=旧 parked_epoch=0`）。
- 目标：
  - 已注册线程异常终止（panic / 未 `scoop_thread_unregister` 就退出）后，stop-the-world 不得被这个不会再到达 safepoint 的线程永久卡住。
- 必须修改的文件/位置：
  - `runtime/c/scoop_gc_backend_immix.c`（STW 等待循环与线程状态机；如需可配合 TLS/线程退出 hook）
- 必须实现的内容：
  1. STW 等待 `parked_count >= need_to_park` 时，需对“记录仍在表中、状态 Running、但线程实际已退出”的情形给出确定性兜底，保证 STW 一定能在有界时间内推进。
  2. 优先方案：提供线程退出时可靠注销的 hook（例如 TLS 析构 / 注册退出回调），从根因上保证线程消失即从 `scoop_gc_threads` 移除；若无法完全覆盖异常退出，再补 STW 侧的僵尸检测/回收兜底。
  3. 不得削弱正常路径正确性：仍在运行的 mutator 必须真正 park 后才允许 GC 扫描其 roots。
- 必须遵从的约束：
  - 兜底不得把“仍存活的线程”误判为僵尸而提前扫描/回收其 roots（否则会引入 use-after-free / 漏标）。
  - 可移植性优先：避免依赖平台特定的“线程存活探测”，除非作为可降级的增强（参考 portability-first 取向）。
- 验证：
  1. 新增一个聚焦的回归用例：构造“已注册线程未 `scoop_thread_unregister` 即退出”的场景，断言后续 STW 能在有界时间内完成（不依赖已删除的旧 stress 测试）。
  2. `cargo test --all --all-targets` 多次连续运行不挂死。
- 完成条件：
  - 即使存在异常终止/未注销的线程，STW 也能在有界时间内完成，不再死锁。
- 依赖：P6-T02R
- 完成记录：
  - 2026-05-30：已完成。
  - 实现：`scoop_thread_register` 现在安装 pthread TLS 退出析构 hook；线程异常返回、panic unwind 或其它漏掉显式 `scoop_thread_unregister` 的正常线程退出路径，会在 pthread TSD teardown 中强制移除当前线程的 GC 记录，避免 STW 后续等待已退出线程 park。
  - 正确性约束：没有加入“存活探测”或把 Running 线程猜成僵尸的 STW 侧捷径；仍在运行的 mutator 必须通过 safepoint/`InNative` 协议就绪后才允许扫描 roots。hook 初始化失败时 fail-fast，避免静默进入可能卡死的注册状态。
  - 清理：显式 unregister 与退出 hook 共用 TLS 指针注销 helper；Immix 线程记录释放前销毁保存的 `stack_walking_ctx`，避免异常/正常注销路径泄漏 unwind ctx。
  - 回归：新增 `scoop_test_gc_registered_thread_exit_without_unregister` 和 `crates/scoop_runtime/tests/gc_stw_thread_exit.rs`，构造 worker 注册后不显式 unregister 即退出，再用 500ms 有界 STW probe 断言后续 STW 不会被 stale Running 记录卡住，并在成功后执行一次正常 `scoop_gc_collect()`。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test -p scoop_runtime --test gc_stw_thread_exit`；`cargo test --all --all-targets` 连续运行两次，均通过且未挂死。

### [DONE] P7-T00b：补 Scoop 语言级并发/GC 应用测试

- 参考：
  - P7-T00 完成记录（STW 健壮性修复）
  - 已删除的 `gc_immix_parallel_mark_sweep_stress.rs`（手写 C-API mutator 无法安全保活 fresh 分配，属于不可修场景，故删除）
- 目标：
  - 用 **Scoop 语言级**的实际程序覆盖“多线程并发分配 + 跨线程引用 + 触发 GC”这类场景，替代被删除的手写 C-API stress 测试。Scoop codegen 会为活跃局部产出 explicit root frame，因此不存在 native 调用方的 alloc→root 窗口，能写出确定性、不 flaky 的并发 GC 测试。
- 必须修改/新增的文件/位置：
  - `tests/fixtures/**`（新增 Scoop fixture）或 `crates/**` 下对应的端到端测试入口。
- 必须实现的内容：
  1. 至少新增几个 Scoop 程序：覆盖并发分配、对象图跨“线程/作用域”引用、循环中制造垃圾以驱动 mark/sweep 与（如适用）minor/晋升路径。
  2. 断言可观察的正确性（结果值、存活/回收计数等），保证在默认 pacing on 下稳定通过、单用例 < 1 分钟。
- 必须遵从的约束：
  - 不得回退到手写 C-API mutator 去模拟“无 root 兜底”的不可修场景。
- 验证：
  1. `python3 tools/run_fixtures.py`（新增 fixture 通过）
  2. `cargo test --all --all-targets`
- 完成条件：
  - 并发/GC 行为由 Scoop 语言级测试覆盖，且稳定不 flaky。
- 依赖：P7-T00
- 完成记录：
  - 2026-05-30：已完成。
  - 新增 `tests/fixtures/runtime_gc/gc_language_parallel_alloc_shared_roots.scoop`：两个 Scoop worker 在 main 触发 STW GC 时持续语言级分配，并通过 worker 局部 roots 保活跨线程发布的对象；stdout 断言最终对象仍可读取。
  - 新增 `tests/fixtures/runtime_gc/gc_language_cross_thread_ref_handoff.scoop`：producer 线程发布含跨对象引用的 `Payload` 图，main 在线程交接前触发 GC，consumer 线程在 GC 后读取并计算确定性结果。
  - 新增 `tests/fixtures/runtime_gc/gc_language_repeated_collect_shared_chain.scoop`：两个 worker 分阶段构造共享链表，每阶段 main 触发 GC，最终跨线程发布链头并在 join 后再次 GC 验证可追踪；阶段等待使用 `sleepMillis(1)` 进入 native wait，避免 NoGC 自旋阻塞 STW。
  - 约束：全部用 Scoop 语言级 `scoop.thread` / `scoop.sync` / `scoop.runtime.test` 覆盖，不恢复手写 C-API mutator，不设置 `SCOOP_GC_PACING=off`。
  - 验证：三个新增 fixture 单独运行均通过；`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（1625 checks）全部通过。

### [DONE] P7-T01：回写 runtime/spec 文档（pacing + immortal）

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P7
  - [`GC_PACING.md`](./GC_PACING.md) “Proposed design”“Env knobs”、[`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Proposed design”
- 目标：
  - 把 pacing 模型与 immortal 概念写进 runtime 文档/相关 spec，作为长期 contract。
- 必须修改的文件/位置：
  - `SCOOP_RUNTIME.md`（及相关 `docs/spec/**` 段，如涉及）
- 必须实现的内容：
  1. 记录 pacing：`target = max(min_threshold, live*factor)`、三层触发（软/分代/硬）、env 旋钮（`SCOOP_GC_PACING`/`GROWTH_FACTOR`/`MIN_THRESHOLD_BYTES`/`MAX_HEAP_BYTES`）、默认 on 姿态、stress 旁路。
  2. 记录 immortal：值/ref 双层、`is_immutable` 谓词、`@InteriorMutable`、dedup 仅 String、immortal 不变式“永不写、永不 trace”。
  3. 同步任何引用旧 GC 行为（无界增长 / per-use wrapper）的文档表述。
- 必须遵从的约束：
  - 文档必须与 P1-P6 实际行为一致，不得描述未实现的旋钮或语义。
- 验证：
  1. `python3 tools/spec_fixtures.py check`
  2. 人工复核文档与实现一致。
- 完成条件：
  - runtime/spec 文档反映 pacing + immortal 实际行为。
- 依赖：P6-T02R
- 完成记录：
  - 2026-05-30：已完成。
  - Runtime 文档：`SCOOP_RUNTIME.md` 新增 pacing contract，记录 `live` / `target_live` / 累计 `next_gc` 公式、软触发（alloc 后 request、下一 safepoint collect）、nursery full minor-GC retry、Immix block-pool/full-GC/hard-cap 路径、`SCOOP_GC_PACING` / `SCOOP_GC_HEAP_TARGET_GROWTH_FACTOR` / `SCOOP_GC_HEAP_MIN_THRESHOLD_BYTES` / `SCOOP_GC_MAX_HEAP_BYTES` / `SCOOP_GC_STRESS` 的默认值与语义，以及 soft pacing 在 `immix` / `hosted` / `minimal` 的 backend parity。
  - Immortal 文档：`SCOOP_RUNTIME.md` 新增 immortal ref header contract（`SCOOP_GC_FLAG_IMMORTAL`、`SCOOP_GC_MARK_IMMORTAL`、`next=null`、永不写/永不 trace），记录 value/ref 双层、`is_immutable(T)` 结构谓词、metadata-only `@InteriorMutable`、`__AtomicInt` nominal struct 化、String 内容池 dedup 与其它 ref-tier 常量 site-keyed 策略。
  - Spec 同步：`SCOOP_FULL_SPEC.md` 与 `docs/spec/language_spec-part1.md` / `part2.md` / `part5.md` / `part6.md` 记录未插值 String literal 可池化为 immortal、`Platform` 是值类型编译期常量、`@InteriorMutable` 内建注解语义、internal atomic value types 必须用 marker 表达背后可变性。
  - 旧行为归位：`GC_PACING.md` / `GC_IMMORTAL_FIX.md` 状态改为 implemented design history，`PLAN.md` 顶部“当前状态”改为 P0 baseline 说明；旧无界增长与 per-use wrapper 分配不再被描述为当前 runtime/spec contract。同步更新 `runtime/c/scoop_gc.h` 的手动 GC API 注释，避免仍声称无自动触发策略。
  - 一致性复核：文档中的 env 名称、默认值和 false/off 解析对照 `runtime/c/scoop_runtime.c` / `runtime/c/scoop_gc.h`；immortal flag/mark、String wrapper 全局、dedup/site-key 策略对照 `runtime/c/scoop_gc.h` 与 `crates/scoopc_codegen_llvm/src/llvm/codegen/main/immortal.rs`；`is_immutable(T)` 规则对照 `immutability.rs`。
  - 验证：`python3 tools/spec_fixtures.py check`（`spec fixtures: ok (1)`）。`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`python3 tools/run_fixtures.py` 未运行，因为本任务只修改 Markdown 文档和一处 C 注释，不改变编译输出；沿用 P7-T00b 完成记录中的最近全量绿灯。

### [DONE] P7-T01R：Review 文档回写

- 参考：
  - P7-T01 完成记录
- 目标：
  - 复核文档与实现一致、无遗漏旋钮或概念。
- 必须检查的文件/位置：
  - P7-T01 文档改动
- 必须实现的内容：
  1. 逐条核对旋钮默认值、触发层次、immortal 概念与实现一致。
  2. 确认无残留的旧 GC 行为描述。
- 必须遵从的约束：
  - 若文档与实现不一致，必须修正后才进入 P7-T02。
- 验证：
  1. `python3 tools/spec_fixtures.py check`
- 完成条件：
  - 文档准确。
- 依赖：P7-T01
- 完成记录：
  - 2026-05-30：已完成。
  - 复核范围：逐条对照 P7-T01 文档回写与实现，覆盖 pacing 默认 on、`SCOOP_GC_PACING` false/off 解析、`SCOOP_GC_HEAP_TARGET_GROWTH_FACTOR` / `SCOOP_GC_HEAP_MIN_THRESHOLD_BYTES` 默认与非法值回退、`SCOOP_GC_MAX_HEAP_BYTES` Immix hard-cap、`SCOOP_GC_STRESS` 分配前触发/旁路 pacing、soft request 在 allocation/write-barrier safepoint 消费、hosted/minimal soft pacing parity，以及 immortal header flag/mark、`is_immutable(T)`、`@InteriorMutable`、String 内容池 dedup、Platform value-tier 常量与 ref-tier site-keyed 策略。
  - 修正：`SCOOP_RUNTIME.md` 明确 soft trigger 在 allocation 或 write-barrier safepoint 消费，且只在 allocation path 上发生“下一次分配前”collect；补全 `SCOOP_GC_STRESS` 的 unset/off/0/false/no、数字间隔与其它非空值语义。
  - 旧行为归位：`GC_PACING.md` / `GC_IMMORTAL_FIX.md` 将 “Today/currently” 旧行为语句改为 P0/P5 baseline/design-history 表述；`PLAN.md` / `TODO.md` 不再把旧无界增长描述成当前状态；`PLAN.md` 同步累计 `next_gc = bytes_freed + target_live` 公式。
  - 验证：`python3 tools/spec_fixtures.py check`（`spec fixtures: ok (1)`）。`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`python3 tools/run_fixtures.py` 未运行，因为本 review 只修改 Markdown 文档、任务记录和 `memory/claude_plan.md`，不改变编译输出；沿用 P7-T00b / P7-T01 记录中的最近完整绿灯。

### [DONE] P7-T02：审计需要 `PACING=off` 的测试并注明原因

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P7
  - [`GC_PACING.md`](./GC_PACING.md) “Test plan — Integration”
- 目标：
  - 找出所有断言精确堆计数的测试，给它们显式 `SCOOP_GC_PACING=off` 并注明 why；确认 immortal 测试不需 off。
- 必须检查的文件/位置：
  - `tests/`、`runtime/c/` 中断言堆对象数/分配数的测试
  - GC smoke 测试（如 `runtime/c/scoop_gc.c` 中精确计数断言）
- 必须实现的内容：
  1. 审计所有依赖确定性堆计数的测试，逐个加 `SCOOP_GC_PACING=off` 并在注释/记录写明原因（这同时是 pacing 的影响面审计）。
  2. 确认 immortal-fix 相关测试**不**需要 `PACING=off`（immortal 不进堆）。
- 必须遵从的约束：
  - 每个用到 `PACING=off` 的测试必须注明 why；不得无理由关闭 pacing 掩盖问题。
- 验证：
  1. `cargo test --all --all-targets`
  2. `python3 tools/run_fixtures.py`
- 完成条件：
  - 需要确定性计数的测试已显式 opt-out 并注明原因。
- 依赖：P7-T01R
- 完成记录：
  - 2026-05-30：已完成。
  - 精确 heap object-count fixture：为 `gc_trace_struct_string_field_basic.scoop`、`gc_mark_sweep_basic.scoop`、`gc_pin_unpin_basic.scoop`、`effect_handle_return_from_function_any_boxing.scoop`、`class_init_raise_cleanup_property_init_gc_basic.scoop`、`class_init_raise_cleanup_init_block_gc_basic.scoop`、`gc_trace_class_ref_field_basic.scoop`、`effect_raise_cleanup_gc_basic.scoop` 以及 retired-ledger mirror `umb_fix/B-29-gc-intrinsics/pos_gc_pin_handle_boundary.scoop` 增加 `SCOOP_GC_PACING=off`，并逐个添加 `P7-T02 why` 注释，说明 stdout / source fixture 依赖确定性 heap object-count。
  - 运行期 integration tests：补充 `gc_pacing_env.rs`、`gc_immix_block_pool.rs`、`gc_immix_hard_cap.rs`、`gc_immix_hard_cap_nursery.rs` 中既有 `SCOOP_GC_PACING=off` 的 `P7-T02 why` 注释；为 `gc_stackmap_multiframe_keepalive.rs` 设置 `SCOOP_GC_PACING=off`，因为 native helper 在 `runtime/c` 中断言 manual GC 后的精确 heap object-count。
  - Opt-out 收窄：移除 `gc_hard_cap_codegen_oom_trap.scoop` 中不需要的 `SCOOP_GC_PACING=off`；该 fixture 只验证 hard-cap OOM fatal trap，不断言确定性 heap 计数。
  - Immortal 审计：`string_literal_immortal_no_alloc_loop.scoop` 与 `platform_immortal_no_alloc_loop.scoop` 保持默认 pacing on，并补注释说明 immortal / Platform folding 路径不应进堆，零 allocation 断言不需要 `SCOOP_GC_PACING=off`。
  - 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（`fixtures: ok (1625)`）。

### [TODO] P7-T02R：Review `PACING=off` 审计

- 参考：
  - P7-T02 完成记录
- 目标：
  - 复核每个 `PACING=off` 都有正当理由，immortal 测试未误关。
- 必须检查的文件/位置：
  - P7-T02 标注的所有 `PACING=off` 用例
- 必须实现的内容：
  1. 逐个确认 why 成立（确实需要确定性计数）。
  2. 确认 immortal 测试在 pacing on 下也通过。
- 必须遵从的约束：
  - 若有无理由的 `PACING=off`，必须改回 on 或补理由后才进入 P7-T03。
- 验证：
  1. `cargo test --all --all-targets`
- 完成条件：
  - pacing opt-out 面清晰可审计。
- 依赖：P7-T02
- 完成记录：
  - （待执行）

### [TODO] P7-T03：全量测试矩阵、out-of-scope 归位与收口

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P7、§6
  - [`GC_PACING.md`](./GC_PACING.md) “Out of scope”、[`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Out of scope”
- 目标：
  - 跑通全量回归矩阵，明确归位 out-of-scope，并写最终收口记录。
- 必须检查的文件/位置：
  - `tests/fixtures/**`、`runtime/c/` 单元测试、长程序回归
  - `PLAN.md` §6 预期收口状态
- 必须实现的内容：
  1. 全量验证：`cargo fmt`、`cargo test --all --all-targets`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`、runtime C 单元测试与长程序回归。
  2. 明确归位 out-of-scope（不做且为何不做）：incremental/concurrent GC、time-budget pacing、`.data` 单实例静态初始化与 static rooting、嵌套聚合 / `EnumVariant` 常量、跨类型 dedup、跨 `.cone` 字面量 dedup、嵌入式 tier 提示、allocation-rate 自适应、pause-time tuning。
  3. 写最终收口记录，逐项对照 `PLAN.md` §6 预期收口状态。
- 必须遵从的约束：
  - 不得把未完成行为简单记成 future work；剩余项必须是明确超出两份设计文档的 v2+ 扩展。
- 验证：
  1. `cargo fmt`
  2. `cargo test --all --all-targets`
  3. `python3 tools/spec_fixtures.py check`
  4. `python3 tools/run_fixtures.py`
- 完成条件：
  - `GC_PACING.md` 与 `GC_IMMORTAL_FIX.md` 的目标行为成为运行期与编译期实际 contract；旧行为只存在于 `PACING=off` 对照与 design history。
- 依赖：P7-T02R
- 完成记录：
  - （待执行）

### [TODO] P7-T03R：Review 最终收口质量

- 参考：
  - P7-T03 完成记录
  - [`PLAN.md`](./PLAN.md) §6
- 目标：
  - 复核全量矩阵通过、out-of-scope 归位合理、收口记录逐项对应 §6。
- 必须检查的文件/位置：
  - P7-T03 收口记录与全量验证输出
- 必须实现的内容：
  1. 复跑关键验证命令确认通过。
  2. 逐项核对 §6 预期收口状态是否达成。
  3. 确认 out-of-scope 项都是明确的 v2+ 扩展，而非未完成的本轮工作。
- 必须遵从的约束：
  - 若任一 §6 项未达成或被错误记为 future work，阻塞收口并补做。
- 验证：
  1. `cargo fmt`
  2. `cargo test --all --all-targets`
  3. `python3 tools/spec_fixtures.py check`
  4. `python3 tools/run_fixtures.py`
- 完成条件：
  - 两份设计文档的目标行为完整闭环。
- 依赖：P7-T03
- 完成记录：
  - （待执行）
