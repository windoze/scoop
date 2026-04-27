# 当前执行计划

## 任务边界

- 本轮只处理 `TODO.md` 中第一个未完成任务。
- 在读取和执行任务前，先检查最新提交是否提到已有问题；若发现已有问题，优先修复或按要求加入前置任务并停止。
- 遇到任何现有 bug、规格不一致、回归、未完成边界或临时绕过，均视为本轮优先事项，不通过缩小范围或替代实现绕过。
- 完成后必须测试、更新 `TODO.md` 和 `PLAN.md`，并提交 Git commit，然后停止。

## 执行步骤

1. 检查仓库状态和最新提交信息，确认是否有前置问题或未提交变更需要避让。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取相关 `PLAN.md`、源码、测试和规范上下文，判断任务是否可直接完成，或是否需要拆分成更小任务。
4. 若发现任务依赖缺失特性或已有缺陷，更新 `TODO.md` 和 `PLAN.md` 记录前置任务，提交后停止。
5. 若任务可完成，按仓库既有结构实现，保持改动范围最小且符合规格。
6. 添加或更新聚焦测试，运行相关测试；若改动风险较高，再运行更广的测试或检查。
7. 更新 `TODO.md` 标记本任务完成，并同步更新 `PLAN.md`。
8. 检查最终 diff，提交清晰的 Git commit。

## 当前状态

- 已写入初始计划。
- 已检查 Git 状态：当前工作区只有本计划文件改动。
- 已检查最新提交：`[T5000hR] Review summary-driven MIR inlining boundaries`，提交说明中未发现明确要求先修复的前置问题。
- 已定位 `TODO.md` 中第一个未完成任务：`T5000i 加入 continuation / closure escaping analysis，并把 effect/state-machine planning 迁到正确边界`。
- 已判断 `T5000i` 单轮过大，并已在 `TODO.md` / `PLAN.md` 中拆成 `T5000i1`～`T5000i4`。
- 本轮执行的第一子任务：`T5000i1 建立 MIR-level closure / continuation escape facts side table`。
- 已新增 MIR escape analysis 模块草案，挂接 `MaterializedMirPassArtifacts` / `MaterializedMirPassView`，并加入 `OptLevel::enables_mir_escape_analysis()` gate。
- 已运行 `cargo fmt --all`。
- 已运行 `cargo test -p scoopc mir::escape -- --nocapture`，5 个 escape 相关测试通过。
- 已运行 `cargo test -p scoopc mir:: -- --nocapture`，43 个 MIR 测试通过。
- 已运行 `cargo test -p scoopc llvm::tests -- --nocapture`，61 个 LLVM production 相关测试通过。
- 已运行 `cargo test -p scoopc --no-default-features`，280 个无默认特性测试通过。
- 已运行 `cargo test --all`，workspace 测试通过。
- 已运行 `cargo clippy --all-targets -- -D warnings`，无 warning。
- 已运行 `cargo run -p scoop -- test`，结果为 `fixtures: ok (1201)`。
- 已将 `TODO.md` 中 `T5000i1` 标记为完成，并在 `PLAN.md` 追加拆分与完成记录。
- 最终清理后补跑 `cargo test -p scoopc mir::escape -- --nocapture` 与 `cargo clippy --all-targets -- -D warnings`，均通过。
- 下一步：最终 diff 检查，然后提交 `[T5000i1] Add MIR escape facts side table`。
