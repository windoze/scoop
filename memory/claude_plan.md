## 本次执行计划

1. 读取 `TODO.md`，严格按标题是否带有 `[DONE]` 判断首个未完成任务。
2. 检查最近提交信息，确认是否存在与该任务直接相关且明确未完成的问题；若有，按要求并入当前任务范围或作为前置任务写回 `TODO.md`。
3. 阅读当前任务涉及的代码、测试、规范与依赖约束，确认是否能直接完整实现；若存在阻塞，最小化地把前置任务写入 `TODO.md`，并停止在该前置处理上。
4. 实现当前任务所需代码改动，坚持最小正确修改，不采用规避性方案。
5. 运行任务要求的验证，包括相关测试、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`，以及必要的定向命令；若失败，先修复再继续。
6. 更新本文档，记录关键发现、计划调整、已完成步骤与验证结果。
7. 更新 `TODO.md`：将已完成任务标题前缀改为 `[DONE]`，填写或补全 completion record；仅在阶段计划变化时更新 `PLAN.md`。
8. 按仓库提交风格创建一次 git 提交，提交信息包含当前任务编号，然后停止，不进入下一个任务。

## 进度记录

- 已初始化本次执行计划。
- 已读取 `TODO.md`，确认首个未完成任务为 `TODO-P7.md` 中的 `P7-T04R`：`Review P7 阶段退出条件，确认默认主线已切换且 P8 只需删除旧主线并再次 full regression`。
- 已检查最新提交：`[P7-T04] Freeze GC env handoff`。提交正文未记录与 `P7-T04R` 直接相关的额外未完成事项，因此当前按既有 `P7-T04R` 执行。
- 已完成代码/文档复查：默认 selector 与 handoff 关键落点确认在 `crates/scoopc/src/session/mod.rs`、`crates/scoop/src/cli.rs`、`crates/scoopc/src/driver_cli.rs`、`crates/scoop/src/commands/mod.rs`、`crates/scoop/src/fixtures/mod.rs`、`crates/scoop/src/fixtures/run_pass.rs`、`crates/scoopc/src/effect_refactor_pipeline/mod.rs`、`EFFECT_REFACTOR.md` §5.6.6、`TODO-P8.md` 首部与 `P8-T01`/`P8-T02`。
- 已完成搜索审计：实现代码中的 selector/legacy/fallback 命中仅剩显式 compare/rollback 入口、legacy unsupported 诊断和 anti-fallback 断言，未发现 omission 默认回 legacy 或 refactor 失败后 hidden fallback。
- 已完成复验：selector/default 定向测试、default/refactor/legacy smoke、`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check`、`cargo clippy --all-targets -- -D warnings`、`P7-T03S` 的 GC stale-root 守护、`P7-T04` 的 GC env 全量矩阵与最终 default/legacy smoke 全部通过。
- 已更新 `TODO.md` 与 `TODO-P7.md`，将 `P7-T04R` 标记为 `[DONE]` 并写入 completion record。
- 下一步：检查 worktree、按任务编号创建提交，然后停止，不进入 `P8-T01`。

## 说明

- 本文件记录执行计划、关键决策与进展更新，不包含逐字内部推理。
