# 执行计划与进度记录

## 说明

按照要求，本文件在任何进一步检查或执行前先建立，用于记录本次执行的计划、关键决策和进度更新。

注意：这里记录的是可审计的执行计划、依据和阶段性结论，不包含不可审计的内部隐式推理。

## 初始计划

1. 检查最新一次 Git 提交信息与变更，确认是否提到了需要先处理的既有问题。
2. 阅读 `TODO.md`，找出第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务是否已有现成分解、依赖或上下文。
4. 结合代码现状判断该任务是否可以在本轮完整落地：
   - 如果可以，直接实现、补测试、验证、更新文档与任务状态。
   - 如果过大或存在前置缺口，则先把任务拆分并回写到 `TODO.md` / `PLAN.md`，本轮只执行新的第一个子任务。
5. 在实现过程中，如果发现任何规格不匹配、缺失语言特性、运行时错误或依赖缺口：
   - 停止“绕过式”实现；
   - 在 `TODO.md` 中新增前置修复任务并调整顺序；
   - 在 `PLAN.md` 与本文件中记录阻塞原因；
   - 提交变更后停止。
6. 若任务可完成，则执行相关验证，至少覆盖：
   - 直接相关测试；
   - 必要的回归测试；
   - `cargo fmt`；
   - `cargo clippy --all-targets -- -D warnings`；
   - 相关构建 / 测试命令。
7. 完成后更新：
   - `TODO.md`；
   - `PLAN.md`；
   - 本文件中的进度记录。
8. 提交一次清晰的 Git commit，然后停止，不进入下一个任务。

## 进度记录

- 已创建本文件，等待开始仓库检查。
- 已检查最新 Git 提交：
  - 最新提交为 `011e1b2345d9481291c8a145e90dc1776e40cce9`，标题是 `[T3016jR] Review closure non-resuming return contract`。
  - commit message 本身未声明新的必须先修遗留问题。
- 已检查当前工作区：
  - 当前未提交变更只有本文件 `memory/claude_plan.md`。
- 已初步检查 `TODO.md`：
  - 第一个未完成任务是 `T3017`：回收 `T3006` 暂时 `xfail` fixtures，恢复 effect run-pass 基线。
  - `T3017` 之前的已知前置 blocker（`T3016a`、`T3016b`、`T3016c`、`T3016d`、`T3016e`、`T3016i`、`T3016j`）都已标记完成。
- 下一步：
  - 读取 `T3017` / `T3017R` 的完整内容与 `PLAN.md` 对应段落。
  - 复核当前仓库中剩余 `EXPECT: fail` / `xfail` 基线，确认哪些仍是 stale expectation，哪些是新的真实生产缺口。
  - 决定本轮直接完成 `T3017`，还是先按要求将其拆为更细子任务并回写 `TODO.md` / `PLAN.md`。
- 已读取 `T3017` 的完整描述并核对当前 run-pass 基线：
  - `tests/fixtures/run-pass/**` 中已无 `T3006: 暂时标记为 fail` 注释。
  - run-pass 下剩余 6 个 `EXPECT: fail` fixture，其中 4 个是本来就应失败的负向/诊断夹具，2 个已分别转记到 `T3304` / `T3406`。
- 已执行 `cargo run -p scoop --features llvm -- test` 做 `T3017` 最终验收：
  - runner 暴露新的更前置 pass-fixture 回归：`tests/fixtures/run-pass/effect_raise_trace_hook_basic.scoop`。
  - 失败形态：fixture 期望通过，但 stdout 与 golden 不一致。
- 已做最小复现与定位：
  - 直接运行 `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_raise_trace_hook_basic.scoop`，实际输出为：
    - `0`
    - `0`
  - 对应 golden 为：
    - `16`
    - `5`
  - 说明 `Raise.raise(...)` 的 call-site trace line/col 没有被写入 runtime TLS。
  - runtime 侧 `scoop_effect_set_active_with_trace(uint32_t src_line, uint32_t src_col)` 仍然存在；
    但当前 codegen 中：
    - `runtime_symbols.rs` 只保留了 `SCOOP_EFFECT_SET_ACTIVE`，没有 `*_WITH_TRACE` 符号常量；
    - `runtime_abi.rs` 只声明了 `declare_runtime_effect_set_active()`，没有 `declare_runtime_effect_set_active_with_trace()`；
    - `effect/mod.rs` 的 `codegen_perform_expr()`、`emit_raise_runtime_error_variant()` 都只调用无 trace 的 `set_active()`，其中 `emit_raise_runtime_error_variant()` 还把 `span` 显式标成未使用；
    - `state_machine_emitter.rs` 的 `UnifiedStateTerminator::Suspend` 生产路径同样只调用无 trace 的 `set_active()`。
  - 结论：这是统一 non-resuming effect trace hook 合同的共享生产回归，而不是 fixture/golden 本身的问题。
- 决策：
  - `T3017` 当前被新的更前置生产缺口阻塞，不能在本轮直接完成。
  - 下一步将按阻塞规则在 `T3017` 前新增修复任务与 review 任务，更新 `TODO.md` / `PLAN.md`，然后提交并停止。
- 已完成阻塞重排：
  - `TODO.md` 已在 `T3017` 前新增 `T3016k` / `T3016kR`，并把 `T3017` 的依赖改为 `T3016kR`。
  - `PLAN.md` 已新增本轮阻塞重排记录，并把当前执行顺序改为 `T3016k -> T3016kR -> T3017 -> T3017R`。
- 本轮收尾动作：
  - 复核变更；
  - 提交一次“新增 blocker 任务并顺延 `T3017`”的 commit；
  - 停止，等待下一轮从 `T3016k` 开始。
