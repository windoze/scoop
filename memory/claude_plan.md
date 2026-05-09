# 执行计划

## 当前任务
- 已读取 `TODO.md`，当前第一项未完成任务是 `P8-T03`：清理 tests / fixtures / docs 中的 legacy 主线与已删除 async/Task surface 残留，并把 compare 型资产改写为纯新主线回归。
- 下一步先补读 `TODO-P8.md` 中 `P8-T03` 的完整要求与验证项，再检查最新提交是否带有与该任务直接相关的未完事项。

## 执行步骤
1. 读取 `TODO-P8.md` 中 `P8-T03` / `P8-T03R` 的完整定义、验证要求与完成条件。
2. 查看最新提交信息，判断是否明确提到与 `P8-T03` 直接相关但尚未完成的收尾项；若有，纳入本任务范围或按要求写回 `TODO.md` 作为前置。
3. 针对 `tests`、`fixtures`、`docs`、`README`、`tools` 和相关测试 helper 做定向搜索，找出以下残留：
   - 把 legacy 主线当作可执行入口或 compare 对象的文本、命令、fixture/harness。
   - 把 `async` / `await` / `Task` 当作现行 surface、现行标准库入口或主路径分类的文本、fixture、测试命名。
4. 逐项修改最小必要文件：
   - 将 compare/rollback 资产改写为纯新主线路径回归，或在对新主线无价值时删除。
   - 清理 README / 文档 / 注释 / 帮助文本中的过时叙述。
   - 更新或新增定向守护测试，确保仓库主路径只剩新主线，且已删除 surface 不再被当作现行能力。
5. 运行本任务要求的定向验证；如改动影响面需要，再补充必要的 smoke 或相关测试。
6. 若任务完成，则更新 `TODO-P8.md` 与 `TODO.md`，把 `P8-T03` 标为 `[DONE]` 并补全完成记录；只有阶段计划变化时才修改 `PLAN.md`。
7. 检查工作区状态，按仓库约定创建一次提交，然后停止，不进入下一任务。

## 记录原则
- 发现 blocker 时，不绕过；若必须新增前置任务，则先改 `TODO.md`，保持当前任务未完成并记录原因。
- 每完成一个关键步骤，或计划因新发现而调整，立即回写本文件。

## 当前已知信息
- `TODO.md` 已显示 `P8-T01`、`P8-T01R`、`P8-T02`、`P8-T02R` 均完成，因此本次不能回头重做前序任务，只能围绕 `P8-T03` 执行。
- 目前尚未核对 `P8-T03` 的完整验证列表，也尚未检查最新提交是否提到直接相关的未完事项；这两步优先于广泛改动。

## 进展记录
- 已读取 `TODO.md` 并确认本次目标任务为 `P8-T03`。
- 已读取 `memory/claude_plan.md` 旧内容并重写为本次任务计划，避免继续沿用上一次 `P8-T02R` 的记录。
- 已补读 `TODO-P8.md` 中 `P8-T03` / `P8-T03R` 的完整要求；最新提交仅为 `[P8-T02R] Review legacy backend removal`，未声明与 `P8-T03` 直接相关的未完 blocker。
- 已完成首次残留盘点：当前主路径中的实质问题集中在 `docs/spec/language_spec-part1.md`、`docs/spec/language_spec-part4.md`、`EFFECT_REFACTOR.md`、`HIR_COMPLETENESS_HANDOFF.md`、`MIR_REFACTOR_PHASE_EXIT_AUDIT.md`、`tools/scoop_tools/src/fixtures_matrix.rs`，以及 `crates/scoopc/src/llvm/tests.rs` 中若干仍把 direct-HIR compare 路径称为 `legacy` 的 helper / 测试名。
- 已判定 `PLAN.md` / `PLAN-pipeline-gaps-*.md` / `ASYNC_REFACTOR.md` / `SCOOP_FULL_SPEC.md` 当前命中属于阶段计划或历史/移除说明，不作为本任务的首批强制改动对象；完成记录中需对它们做分类解释。
- 下一步将执行最小编辑：
  1. 更新上述 live docs，去掉把 selector 当作现行命令或把 async/Task 当作现行 surface 的表述；
  2. 从 stdlib fixture matrix 中删除 `Task / Executor (async)` 域并同步测试；
  3. 把 `llvm/tests.rs` 中误导性的 `legacy` compare 命名改为中立 direct-HIR 命名，并新增 `legacy_compare_harness_removed_*` 守护；
  4. 新增 `legacy_pipeline_docs_removed_*` 守护测试覆盖主文档/工具索引残留。
- 已完成上述编辑：`docs/spec/language_spec-part1.md` / `language_spec-part4.md` 已去掉把 async/task 当作现行 surface 的叙述；`EFFECT_REFACTOR.md`、`HIR_COMPLETENESS_HANDOFF.md`、`MIR_REFACTOR_PHASE_EXIT_AUDIT.md` 已改为只描述当前单一路径命令；`tools/scoop_tools/src/fixtures_matrix.rs` 已删除 `Task / Executor (async)` 域并同步域编号/单测；`crates/scoopc/src/llvm/tests.rs` 已把 direct-HIR compare 命名从 `legacy` 改为中立表达，并新增窄的 source guard；`crates/scoop/tests/p8_docs_cleanup.rs` 已新增主文档/工具索引负向守护。
- 已完成定向验证：
  - `cargo fmt`
  - `cargo test -p scoop legacy_pipeline_docs_removed`
  - `cargo test -p scoopc legacy_compare_harness_removed`
  - `cargo clippy --all-targets -- -D warnings`
- 已完成两轮搜索核对：
  - 聚焦已改文件的搜索已确认不再残留 `--effect-pipeline legacy|refactor`、`Task<`、`Async.await`、`std_task_` 等现行误导文本。
  - 扩展搜索（排除 `docs/archive/**`、`target/**`、`TODO*.md`、`PLAN*.md`、`memory/**`、`.git/**`）后，剩余命中仅有：`SCOOP_FULL_SPEC.md` 的删除说明、`ASYNC_REFACTOR.md` 的历史设计文档、`crates/scoop/src/commands/build.rs` 的 anti-fallback 断言文本，以及本次新增 `crates/scoop/tests/p8_docs_cleanup.rs` 负向守护；未发现主文档/主测试/fixture helper 继续把 legacy 或 async/task surface 当作现行入口。
- 已更新 `TODO-P8.md` 与 `TODO.md`：`P8-T03` 已标记为 `[DONE]`，并补齐完成记录、验证命令与搜索分类摘要。
- 下一步：检查最终工作区状态，按任务要求创建提交，然后停止。
