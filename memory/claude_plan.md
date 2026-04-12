# 执行计划

1. 先检查最新一次 Git 提交，确认是否提到了需要先修复的既有问题；如果有，优先处理这些问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 结合代码现状评估该任务是否足够小且能在一次迭代内完整交付；如果过大，就把它拆分为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 实现当前应执行的首个任务，只做与该任务直接相关且必要的代码修改。
5. 运行相关测试、格式化与静态检查；若发现问题，继续修复直到结果稳定。
6. 更新 `memory/claude_plan.md` 记录关键进展，更新 `TODO.md`/`PLAN.md` 标记状态与依赖关系。
7. 检查工作区变更，使用清晰的提交信息提交本轮修改。
8. 完成一个任务后立即停止，不继续处理下一个任务。

# 说明

- 这里记录的是可审阅的执行方案与关键决策摘要，不包含逐字的内部推理。
- 如果执行过程中发现规范缺口、已有缺陷或任务依赖未满足，会先补充/重排 `TODO.md` 与 `PLAN.md`，再决定是否继续实现。

## 当前进展（2026-04-13）

- 已检查最新提交 `2aee6aaacec68fab0416f08b1fd527f6e4ec1486`（`[T2003u3a] Add effect mode-specific simplification dump`）。
- 该提交说明中没有显式列出必须先处理的既有问题，因此当前按 `TODO.md` 的任务顺序继续执行。
- 已定位首个未完成任务为 `T2003u3b`：`Effect：用 simplification 收口 codegen_handle_expr 入口选路`。

## T2003u3b 细化执行步骤

1. 阅读 `TODO.md` / `PLAN.md` 中 `T2003u3b` 的目标与验收要求。
2. 检查 `crates/scoopc/src/llvm/codegen/effect/` 下与统一 plan、simplification、`codegen_handle_expr` 入口选路相关的实现。
3. 判断 `T2003u3b` 是否仍然是可在单轮内完整完成的任务；如果范围已膨胀，则先拆分并更新 `TODO.md` / `PLAN.md`。
4. 若任务范围可控，则把 `codegen_handle_expr` 的入口分流改为以 simplification 结果为主，收口遗留的结构性假设。
5. 为代表性的 never-resume / immediate-resume / escape-continuation 路径补或更新 LLVM 级验证。
6. 运行格式化、测试与静态检查，至少覆盖本任务要求的命令；若失败则继续修复。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，随后提交一次 Git commit 并停止。

## 当前实现状态

- 已在 `state_machine_simplify.rs` 中补充 root-arm 汇总与 `codegen_entrypoint` 分类，入口分类现在可区分：
  - no-suspend；
  - single non-resuming / immediate-resume / escape-continuation；
  - multi non-resuming；
  - multiple immediate / multiple escape；
  - immediate+non-resuming、escape+non-resuming、immediate+escape(+non-resuming)；
  - 当前仍未支持的“multiple immediate + escape”与“immediate + multiple escape”混合形态。
- 已把 `codegen_handle_expr` 与 `codegen_handle_expr_multi_arm` 改为消费上述分类来选择现有 specialized lowering，并增加 simplification 分类与 HIR arm 形态的一致性校验。
- 已新增单元测试，直接验证 simplification 对 single-mode 与 mixed representative sample 的入口分类结果。

## 下一步

1. 重新运行 `cargo fmt --all`。
2. 重新跑最相关测试与失败回归：
   - `cargo test -p scoopc simplification_codegen_entrypoint -- --nocapture`
   - `cargo run -p scoop --features llvm -- test`（重点确认 `class_init_raise_cleanup_init_block_gc_basic` 不再回归）
3. 若通过，再跑：
   - `cargo test --all`
   - `cargo clippy --workspace --all-targets -- -D warnings`
4. 最后更新 `TODO.md` / `PLAN.md` 标记 `T2003u3b` 完成并提交。

## 已发现并处理的回归

- 首轮把 `NoSuspendSites` 直接作为 no-perform 早退条件后，`cargo run -p scoop --features llvm -- test` 在
  `tests/fixtures/run-pass/class_init_raise_cleanup_init_block_gc_basic.scoop` 上回归：
  类初始化中的 `Raise.raise` 没有被外层 `try/catch` 捕获，程序提前返回，stdout 变为空。
- 原因分析：
  - 统一 plan/simplification 当前仍未完整覆盖某些“调用点表面纯，但内部会通过 Raise/effect unwinding 逃逸”的路径；
  - 该 fixture 正好命中 class init block 的隐藏 unwind 形态；
  - 因而 `NoSuspendSites` 目前不能独自承担 no-perform 早退判定。
- 修正：
  - 恢复旧的 `block_may_perform` 保守 gate 作为 no-perform 早退前置条件；
  - 当 simplification 仍给出 `NoSuspendSites` 但旧 gate 判断“可能 perform”时，不再早退，而是继续按 simplification 的 arm 模式分类选旧 emitter。

## 当前结论

- 所有本轮验收已通过：
  - `cargo test -p scoopc simplification_codegen_entrypoint -- --nocapture`
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- `TODO.md` / `PLAN.md` 已更新，`T2003u3b` 标记为完成，下一步顺延到 `T2003u4`。
- 剩余动作：
  1. 检查工作区 diff；
  2. 使用 `T2003u3b` 对应的提交信息提交；
  3. 停止，不进入下一任务。
