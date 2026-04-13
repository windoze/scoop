# Claude Plan

更新时间：2026-04-13

说明：按要求在任何代码或命令执行前先记录计划。这里记录的是可审阅的高层分析、执行步骤、关键决策和进度，不包含内部私有推理细节。

## 当前目标

完成 `TODO.md` 中第一个未完成任务，并在完成后停止。如果发现前置问题、实现缺口或任务过大，则先调整 `TODO.md`/`PLAN.md`，提交后停止。

## 初始执行计划

1. 检查最近一次提交信息与变更，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务的上下文、依赖与项目阶段。
4. 如果任务过大：
   - 将任务拆分为更小子任务。
   - 更新 `PLAN.md`。
   - 更新 `TODO.md`，使第一个子任务成为新的首个未完成任务。
   - 本次只执行新的第一个子任务。
5. 实现该任务，过程中如果发现任何与规范不一致的缺口：
   - 先确认是 bug、缺失特性还是实现边界。
   - 在 `TODO.md` 中添加并前移必要前置任务。
   - 在 `PLAN.md` 和本文件中记录阻塞原因。
   - 仅提交计划调整并停止。
6. 为实现补充或更新测试，并运行相关验证：
   - 任务相关测试。
   - 至少执行必要的格式化、静态检查和无警告构建/检查流程。
7. 更新文档状态：
   - 在 `TODO.md` 标记任务完成。
   - 在 `PLAN.md` 记录进度和后续顺序。
   - 视需要更新 `README.md` 或内联注释。
8. 检查工作区改动，确保未误改无关内容。
9. 提交本次变更，提交信息聚焦当前任务。
10. 停止，不继续下一个任务。

## 进度记录

- 2026-04-13：已创建初始计划文件，下一步开始检查最近提交与任务列表。
- 2026-04-13：已检查最新提交 `842f2614393c0504ca9e6bff03e1795f39e2c9e5`。提交信息本身没有额外正文或显式要求先修复的遗留问题。
- 2026-04-13：已定位 `TODO.md` 中首个未完成任务为 `T2003u5c`：`Effect：no-immediate multiple-escape 的 while direct/indirect mixed replay`。
- 2026-04-13：下一步将阅读 `T2003u5c` 任务定义、`PLAN.md` 上下文以及对应 LLVM effect lowering / fixture，确认需要补齐的具体路径与回归范围。
- 2026-04-13：已确认缺口位于 `crates/scoopc/src/llvm/codegen/effect/mixed.rs` 的 `codegen_handle_expr_escape_with_nonresuming_siblings_top_level_mixed`：
  - 当前 while mixed 只允许“同一条 statement 内的 direct + indirect 共存”。
  - 但该文件和 `matrix.rs` 中其实已经存在可复用的 while direct-site / indirect-site emitter helper，说明主要缺的是 site 分类与两处语句分发接线。
- 2026-04-13：当前实施计划：
  1. 先按顺序将原 `T2003u5c` 拆成 `T2003u5c1` / `T2003u5c2`，显式追踪 `direct -> indirect` 与 `indirect -> direct` 两种 while separate-stmt mixed replay。
  2. 本轮只实现新的首个子任务 `T2003u5c1`：支持 `direct -> indirect`。
  3. 代码层面优先扩展 `mixed.rs` / `matrix.rs` 里 direct-first 所需 helper：
     - initial while mixed 入口允许 `direct` 先于 `indirect` 的 separate-stmt 排序；
     - continuation step 可从 direct site 继续 replay 到后续 indirect site；
     - indirect site 完成当前迭代后，future iteration 重新命中 earlier direct site。
  4. 将 `tests/fixtures/build/effect_multi_escape_direct_indirect_while_is_error.scoop` 转为 run-pass 回归。
  5. 运行格式化、测试、LLVM fixture 与 clippy，无警告后更新文档状态并提交。
- 2026-04-13：已完成实现：
  - `scan.rs` 新增 while separate-stmt 排序 helper，用于识别 `direct -> indirect` 的合法 while mixed 子集。
  - `mixed.rs` 的 no-immediate top-level mixed 分类现已接受 while body 中 one direct + one indirect 的 direct-first separate-stmt 顺序。
  - `matrix.rs` 已补 direct-first 所需的 replay helper：从 direct site 继续到后续 indirect site，以及 indirect site 完成当前迭代后回到 earlier direct site。
  - 已将原 build-fail `effect_multi_escape_direct_indirect_while_is_error` 转成 run-pass `effect_multi_escape_direct_indirect_while`。
- 2026-04-13：已完成验证：
  - `cargo test -q`
  - `cargo run -q -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_escape_direct_indirect_while.scoop`
  - `cargo run -q -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 2026-04-13：输出核对结果：
  - 新 run-pass 程序 stdout 与新增 golden 一致。
  - 全量 LLVM fixtures 通过：`fixtures: ok (995)`。
