# 执行计划

## 当前任务
- 已读取 `TODO.md`，当前第一项未完成任务是 `P8-T03R`：review 测试/文档残留清理，确认公开叙述与主测试路径只剩新主线，且不再暴露已删除 `async` / `await` / `Task` surface。
- 最新提交是 `[P8-T03] Clean stale pipeline and async-task residue`；提交信息未声明需要先处理的新 blocker，因此本次直接执行 `P8-T03R` 的复核与验证。

## 执行步骤
1. 读取 `TODO-P8.md` 中 `P8-T03R` 的 review 要求、必须检查的位置和验证命令。
2. 检查 `P8-T03` 的提交结果与当前工作区状态，确认 review 基线是否干净、是否存在未提交续作。
3. 逐项复核 `P8-T03R` 指定位置：
   - `crates/scoop/src/fixtures/**`
   - `crates/scoopc/src/llvm/tests.rs`
   - `tools/scoop_tools/src/fixtures_matrix.rs`
   - README / 开发文档 / fixture 注释
   - 保留的迁移说明与负向删除测试
4. 重新运行 `P8-T03` 要求的测试与命令，并执行 `P8-T03R` 指定的额外搜索，分类判断剩余命中是否仅为历史说明或负向守护。
5. 若复核发现问题，做最小必要修复并重跑相关验证；若没有问题，则仅更新文档记录。
6. 更新 `TODO-P8.md` 与 `TODO.md`，把 `P8-T03R` 标为 `[DONE]` 并写入 review 结论；仅在阶段计划变化时更新 `PLAN.md`。
7. 更新本文件记录关键进展，最后按仓库约定提交一次 git commit，然后停止。

## 记录原则
- 发现 blocker 时，不绕过；若必须新增前置任务，则先改 `TODO.md`，保持 `P8-T03R` 未完成并记录原因。
- 每完成一个关键步骤或调整验证范围，立即回写本文件。

## 进展记录
- 已读取 `TODO.md`，确认当前首个未完成任务为 `P8-T03R`。
- 已读取 `TODO-P8.md` 中 `P8-T03R` / `P8-T04` 条目，明确本次需要复核指定文件、重跑 `P8-T03` 的全部验证，并执行额外 `rg` 搜索分类。
- 已查看最新提交 `1f948efc [P8-T03] Clean stale pipeline and async-task residue`；未发现提交信息中声明的直接相关未完 blocker。
- 已检查工作区状态：当前未提交改动仅为本文件；review 可以在干净基线上继续。
- 已完成首轮定向搜索与文件复核：`crates/scoop/src/fixtures/**`、`crates/scoopc/src/llvm/tests.rs`、`tools/scoop_tools/src/fixtures_matrix.rs` 未再暴露 pipeline selector 或 async/task 现行 surface；`ASYNC_REFACTOR.md` 与 `SCOOP_FULL_SPEC.md` 的相关命中属于历史/移除说明；`crates/scoop/src/commands/build.rs` 的 `legacy` 命中属于 anti-fallback 负向断言。
- 发现 `P8-T03` 的漏网项：`docs/spec/language_spec-part1.md` 仍在目录与关键字列表中写出 `async/await`；`docs/spec/language_spec-part3.md` 仍把 `async` / `await` 描述成现行表达式语法与前缀运算符。这直接违反 `P8-T03R` 的 live-doc review 要求。
- 已完成修正：
  1. `docs/spec/language_spec-part1.md` 已将第 4 部分目录项改回“效果系统与异常语法糖”，并从关键字列表删除 `async await`；
  2. `docs/spec/language_spec-part3.md` 已删除把 `async` 视为现行表达式、把 `async {}` 视为现行块形状、以及把 `await` 视为前缀运算符的描述；
  3. `crates/scoop/tests/p8_docs_cleanup.rs` 已把上述 part1/part3 漏网项纳入负向守护。
- 已完成验证：
  - `cargo fmt`
  - `cargo test -p scoop legacy_pipeline_docs_removed`
  - `cargo test -p scoopc legacy_compare_harness_removed`
  - `cargo clippy --all-targets -- -D warnings`
  - 任务要求搜索：`rg -n -e "--effect-pipeline legacy|--effect-pipeline refactor|legacy pipeline|parallel pipeline|old effect mainline|async fun|Async\.await|Task<|std_task_|async_await_" . --glob '!docs/archive/**' --glob '!target/**'`
  - 追加 live-spec 搜索：`rg -n "async/await|\basync\b|\bawait\b|Task<|Async\.await|std_task_|async_await_" docs/spec crates/scoop/tests/p8_docs_cleanup.rs tools/scoop_tools/src/fixtures_matrix.rs SCOOP_FULL_SPEC.md ASYNC_REFACTOR.md EFFECT_REFACTOR.md HIR_COMPLETENESS_HANDOFF.md MIR_REFACTOR_PHASE_EXIT_AUDIT.md README.md`
- 验证结果：
  - 定向测试与 clippy 均通过。
  - 任务要求搜索的剩余命中集中在历史 `TODO/PLAN` 记录、`SCOOP_FULL_SPEC.md` / `ASYNC_REFACTOR.md` 的删除或设计说明、`crates/scoop/tests/p8_docs_cleanup.rs` 负向守护，以及 `crates/scoop/src/commands/build.rs` anti-fallback 断言。
  - 追加 live-spec 搜索确认：`docs/spec/language_spec-part1.md` / `language_spec-part3.md` 的漏网项已清理；live spec 中仅剩 `language_spec-part4.md` 对“当前版本不定义 async/await”的负向说明。
- 已更新 `TODO-P8.md` 与 `TODO.md`：`P8-T03R` 已标记为 `[DONE]` 并写入复核记录。
- 下一步：检查最终工作区状态，创建 `[P8-T03R]` 提交，然后停止。
