# Claude Plan

## Constraints
- 不记录或暴露内部私有推理；这里只维护可审阅的执行计划、关键判断与进度。
- 先读取 `TODO.md` 作为索引，再读取对应 `TODO-Px.md` 作为任务真源。
- 本次只完成第一个未完成的详细任务；若遇到真实阻塞，则按要求补充最小前置任务、同步索引并停止。

## Execution Plan
1. 读取 `TODO.md`，按索引顺序定位相关 `TODO-Px.md` 文件。
2. 检查详细任务文件中的完成记录，确定第一个未完成的详细任务。
3. 查看最近提交，判断是否存在与该任务直接相关且未完成的问题需要并入当前任务或作为前置任务。
4. 阅读与当前任务直接相关的代码、测试、规范和任务约束，确认实现边界。
5. 实现当前任务，保持改动最小且符合现有架构。
6. 运行与该任务相关的测试、必要的格式化/静态检查；若出现失败，先修复再继续。
7. 更新对应 `TODO-Px.md` 的完成记录；如任务索引、标题、顺序或依赖变化，同步更新 `TODO.md`；仅在阶段计划变化时更新 `PLAN.md`。
8. 将关键进展与结果补充到本文件。
9. 按仓库约定创建一次 git 提交，然后停止，不进入下一个任务。

## Progress Log
- 已创建初始计划，下一步开始读取任务索引并确定当前执行单元。
- 已读取 `TODO.md`、`TODO-P0.md`、`TODO-P1.md`；确认 P0 全部完成，当前首个未完成详细任务为 `P1-T02`（锁定 continuation/resume 与单一 `Unit` 参数调用的 AST 形状）。
- 已检查最新提交 `29c04a46fc7bea6a8c4559ebd2f090a157f52b45 [P1-T01R] Review AST stage parser neutrality`，未发现与 `P1-T02` 直接相关且需先补做的未完成事项。
- 下一步：审阅 parser 测试、AST dump fixture 结构与现有 `dump-ast` 输出稳定性，决定需要新增的最小 fixtures 与断言位置。
- 已完成实现改动：
  - 在 `crates/scoopc/src/parser/tests.rs` 增加 3 组结构性断言，分别锁定 `k.resume(x)`、`k.resume()`、`f()` / `f(())` 的 AST 形状。
  - 新增 3 个 parse fixtures：`continuation_resume_member_call_basic.scoop`、`continuation_resume_unit_call_basic.scoop`、`unit_single_param_zero_arg_call_basic.scoop`。
  - 已根据实际 `dump-ast` 输出补齐对应 `.ast` golden，当前无需修改 parser/AST 实现本身。
- 下一步：运行 `parser::tests`、legacy/refactor parse fixture、`dump-ast` smoke 与 clippy，确认任务验证矩阵全部通过。
- 验证中发现一个非语义问题：`clippy` 对测试辅助函数提示 `needless_lifetimes`。已改为省略 lifetime，接下来重跑需要的验证命令。
- 复验完成并全部通过：`cargo test -p scoopc --no-default-features parser::tests`、6 组 legacy/refactor parse fixture 命令、2 组 refactor `dump-ast` smoke、`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`。
- 已将 `P1-T02` 完成记录回写到 `TODO-P1.md`。下一步检查工作树差异并按任务号创建一次提交，然后停止。
