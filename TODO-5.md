# TODO-5：P7 文档、fixtures 收尾与全量回归矩阵

> 索引：[`TODO.md`](./TODO.md)
> 计划基线：[`PLAN.md`](./PLAN.md)
> 覆盖阶段：P7
> 包目标：把 P1-P6 的运行期与编译期行为反映到 runtime 文档、env 旋钮说明、fixtures 与回归矩阵，明确归位 out-of-scope，确保后续不需重新判读新行为。

## P7：Spec / 文档 / fixtures 收尾与回归矩阵

### [TODO] P7-T01：回写 runtime/spec 文档（pacing + immortal）

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
  - （待执行）

### [TODO] P7-T01R：Review 文档回写

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
  - （待执行）

### [TODO] P7-T02：审计需要 `PACING=off` 的测试并注明原因

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
  - （待执行）

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
