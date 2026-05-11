# Claude Plan

## 约束
- 先以 `TODO.md` 为唯一任务排序与完成状态来源，定位第一个未标记 `[DONE]` 的任务。
- 在确认当前任务前，不做开放式问题排查；仅检查与当前任务直接相关的最新提交说明与阻塞项。
- 本次调用只完成一个任务；若遇到真实阻塞，则在 `TODO.md` 中插入最小必要前置任务并停止。
- 任务完成后必须更新 `TODO.md` 的标题为 `[DONE]`，补全完成记录，按需更新 `PLAN.md`，执行验证，并提交 git commit。
- 执行过程中若计划变化或关键步骤完成，及时更新本文件。

## 初始执行计划
1. 读取 `TODO.md`，确定第一个未完成任务及其要求、依赖、验证标准。
2. 查看最近提交，确认是否存在与该任务直接相关且未完成的问题需要一并处理或前置登记。
3. 阅读当前任务涉及的代码、测试、文档位置，确认实现边界与现状。
4. 实施最小正确修改，必要时同步补充或调整测试。
5. 运行任务要求的验证，以及相关最小测试集；若需要，补跑 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings` 等。
6. 更新 `memory/claude_plan.md` 记录结果；更新 `TODO.md`（和仅在阶段计划变化时更新 `PLAN.md`）。
7. 检查工作区状态并按仓库约定创建一次提交，然后停止。

## 当前任务
- `G8-T10：完整扫描所有 fixture 并建立失败清单`

## 当前任务执行计划
1. 检查最近提交，确认是否存在与 `G8-T10` 直接相关且尚未收口的问题需要并入当前任务或登记前置依赖。
2. 复核 `G8-T10` 需要产出的 failure inventory 形态，并在仓库中找到合适的记录位置。
3. 运行完整 fixture sweep：
   - `cargo run -p scoop -- test`
   - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test`
4. 若本地与 CI 不一致，查询最新失败的 GitHub Actions run，抓取失败 job / step / 首条失败信息。
5. 整理 failure inventory：按 build、run-pass、snapshot、runtime/GC-only、仅 GC-env 暴露、CI-only/local-only 分类，并记录每项的 fixture、失败阶段、首个报错、直接相关 owner 文件/函数。
6. 更新 `TODO.md`：将 `G8-T10` 标记为 `[DONE]`，写入改动范围、核心决策、验证结果与对应 gap。
7. 视需要更新 `PLAN.md`（仅当阶段计划变化），检查工作区，提交本次任务。

## 进度记录
- 已写入初始计划。
- 已读取 `TODO.md` 并锁定当前任务为 `G8-T10`。
- 已运行一次默认环境 `cargo run -p scoop -- test`；当前观察到 harness 在首个失败 `tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop` 处停止，首条报错为：`LLVM stage handoff 缺少 reachable callable 'fixtures.build.Base.ping' 的 published late-lowered body`。
- 计划调整：先确认 `scoop test` 是否存在 continue/keep-going 选项；若没有，则改为按 fixture 枚举并逐个调用同一 harness，以满足“完整 sweep”要求。
