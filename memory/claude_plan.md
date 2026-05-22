执行计划（2026-05-22）

说明：本文件记录可公开的执行计划、决策摘要和进度，不包含隐藏推理链。

1. 读取 `TODO.md`，按文件顺序找到第一个标题未以 `[DONE]` 标记的任务，并确认其要求、依赖和验证标准。
2. 查看最近提交信息，只在其明确提到与当前任务直接相关的未完成问题时，将其纳入当前任务或作为前置项记录到 `TODO.md`。
3. 根据当前任务定位相关代码、测试和夹具，优先采用最小正确修改完成任务，不做规避实现或规格降级。
4. 若发现阻塞当前任务的规格不匹配、缺失语言特性或实现边界，先在 `TODO.md` 中插入最小必要前置任务，保持当前任务未完成，提交后停止。
5. 若可直接完成任务，实施代码或文档修改，并为行为变化补充或更新相关测试/fixture。
6. 运行当前任务要求的验证命令，以及必要的定向测试；若出现失败，修复后重跑验证。
7. 任务完成后，在 `TODO.md` 的任务标题前加 `[DONE]` 并更新完成记录；仅在阶段级计划发生变化时更新 `PLAN.md`。
8. 检查工作区差异，确认只提交本次任务相关文件；如存在需纳入的恢复性未提交文件，按用户要求一并提交。
9. 使用清晰的任务编号提交信息提交更改，然后停止，不继续处理下一个任务。

当前状态：已确认首个未完成任务为 `P6-T04R：Review P6 全包完成度`。最新提交 `[P6-T04] Complete P6 cleanup audit` 直接属于本 review 范围。

进度记录：已复审 P6 主要边界。`cone_init.rs` 仍按 LIR facts 构造并执行 per-cone eager roots；top-level `val` 普通访问只做 initialized guard 检查并读取 backing storage；`scoop_once_begin/end` 的实际 codegen 调用仍限定在 `object_init.rs`。LLVM 中 `top_level_vars`、`top_level_immutable_values`、`object_inits`、`extern_globals` 等 HIR scaffold 读取仍存在，但 `TODO-6.md` 的 `P7-T01` / `P7-T02` / `P7-T03` 已明确登记为 P7 residual，不把它们描述为 P6 已完成范围。下一步运行 P6-T04R 验证矩阵。

验证记录：`cargo fmt`、`cargo test -p scoopc_lir_facts`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`、`cargo clippy --all-targets -- -D warnings`、`git diff --check` 均通过。完整 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 已重跑，P6/global-init/thread-init 相关 fixtures 通过；全量仍有 7 个既有非 P6 baseline 失败，与 `P6-T04` 记录一致。

当前状态：`TODO.md` / `TODO-6.md` 已更新为 `P6-T04R` 完成。下一步提交 `[P6-T04R] Review P6 completion`，然后停止。
