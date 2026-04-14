# 执行计划

## 约束说明

- 按要求先更新本文件，再执行任何命令。
- 这里只记录可审计的执行计划、关键判断依据和进度，不记录不可审计的内部推理细节。
- 本轮目标：先处理最新提交中明确提到的遗留问题；随后读取 `TODO.md`，完成第一个未完成任务；完成后测试、更新文档、提交并停止。

## 初始步骤

1. 检查最新一次 Git 提交的提交信息与变更内容，确认是否明确提到已知问题、后续修复项或待处理缺陷。
2. 读取 `TODO.md` 与 `PLAN.md`，定位第一个未完成任务，并核对是否已经有依赖说明或拆分计划。
3. 如首个未完成任务过大，则先把任务拆分为更小子任务，并同步更新 `TODO.md` 与 `PLAN.md`，本轮只执行拆分后的第一个子任务。
4. 实施任务所需代码修改，遵守仓库规范，不引入规避式实现。
5. 运行与改动直接相关的测试；若任务影响范围较大，再补充工作区要求的格式化、lint 或更广测试。
6. 更新 `TODO.md`、`PLAN.md`、本文件中的进度记录。
7. 提交本轮变更，提交后停止，不继续做下一个任务。

## 进度记录

- 2026-04-14：已写入初始计划，尚未开始仓库检查。
- 2026-04-14：已检查最新提交 `f2b2119aa9aed4d38951ad9f406039c67db20e8e`，提交信息未显式标注新的遗留问题；接下来通过 `TODO.md`/`PLAN.md` 核对是否已有对应后续修复项。
- 2026-04-14：已定位首个未完成任务为 `T2003r3d3`。结合 `multi_resuming_heap.rs` 与 `multi_resuming_mixed.rs` 的现状，确认该任务同时包含两个独立 unified leaf（pure multi-escape / `1 immediate + 1 escape` mixed）的不同缺口，不适合整包在一轮内推进。
- 2026-04-14：决定先将 `T2003r3d3` 拆分为更小子任务，并先执行第一个子任务：接回 unified `1 immediate + 1 escape` mixed leaf 的 sibling non-resuming / cleanup contract，清掉当前显式的 `sibling non-resuming not yet connected` gate。
- 2026-04-14：已完成 `T2003r3d3a` 实现。`nonresuming.rs` 已允许 `1 immediate + 1 escape + sibling non-resuming` 进入 unified mixed leaf；`multi_resuming_mixed.rs` 已接回 sibling dispatch / cleanup contract，并补了 LLVM 单测与 representative fixture `effect_resume_mixed_escape_abort_finally`。
- 2026-04-14：已完成定向验证：`cargo fmt --all`、`cargo test -p scoopc unified_multi_resuming_codegen_emits_single_immediate_single_escape_with_nonresuming_sibling -- --nocapture`、`cargo test -p scoopc llvm::codegen::effect::tests:: -- --nocapture`、`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_mixed_escape_abort_finally.scoop`、`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_mixed_escape_direct_finally.scoop`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。
