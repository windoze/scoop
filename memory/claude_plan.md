# 执行计划与进度

## 当前约束
- 以 `TODO.md` 为任务顺序和完成状态的唯一来源。
- 只处理第一个未在标题中标记 `[DONE]` 的任务，完成后停止。
- 若遇到当前任务的真实前置阻塞问题，先修复；若无法在本轮完成，则把最小必要前置任务写入 `TODO.md` 并停止。
- 不使用规避实现、不弱化测试、不更改任务范围来绕过缺陷。
- 完成后更新 `TODO.md`，运行相关验证，提交 Git commit。

## 初始执行计划
1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务，并记录任务编号、要求、依赖和验证标准。
2. 检查最近提交信息是否明确提到与该任务直接相关的未完成问题；如有，将其纳入当前任务或作为前置项写入 `TODO.md`。
3. 针对当前任务阅读相关代码、文档和测试，明确最小正确改动范围。
4. 实现当前任务，优先做符合规格的通用修复，不做 fixture-only 或局部绕过。
5. 添加或更新最小相关测试/fixture，确保行为被覆盖。
6. 运行任务要求的验证命令及必要的补充验证；若失败，修复后重跑。
7. 更新 `TODO.md`：在完成任务标题前加 `[DONE]`，补全完成记录和验证记录；仅在阶段计划变化时更新 `PLAN.md`。
8. 检查 `git status`、`git diff`、最近提交，确认只提交本轮相关文件；若存在本轮应纳入的未提交恢复状态，一并提交。
9. 使用清晰的任务编号 commit message 提交，然后停止，不处理后续任务。

## 进度日志
- 已创建本轮执行计划；下一步读取 `TODO.md` 定位第一个未完成任务。
- 已定位当前任务：`P7-T01`（迁移 LLVM entry/global 查询到 LIR facts）。最近提交为 `P6-T04R` review 完成提交，未发现需要在本任务前新增的直接未完成前置项。
- 当前实现重点：检查 LLVM emit/codegen entry/global 路径，找出仍读取 HIR side tables 的 entry selection、global/root inventory、extern/global/top-level/object 查询；若 LIR facts 缺字段，补到 `scoopc_lir_facts` 或 builder，而不是保留 fallback。
- 已确认需要补 facts owner：entry selection 需要 LIR callable 的 source-level 签名与 top-level 候选分类；extern global physicalization 需要 LIR extern linkage。下一步先扩展 `scoopc_lir_facts` contract/builder/verifier，再切换 LLVM entry/global 查询。
- 已完成实现：LIR facts 发布 entry/global 所需合同；LLVM entry selection、extern global、top-level var/val、object singleton inventory 查询已切到 LIR facts；`CompilationUnitCodegenCx` 不再携带 HIR `extern_globals` side table。
- 已完成验证：`cargo fmt`；`cargo test -p scoopc_lir_facts`；`cargo test -p scoopc --no-default-features llvm_entry_global`；`cargo test -p scoopc llvm_entry_global`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
- 已运行完整 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`，本任务相关路径通过，全量仍有 7 个既有非本任务失败，已记录到 `TODO-6.md` 完成记录。
