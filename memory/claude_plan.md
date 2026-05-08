## 本轮执行计划

### 当前约束
- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后立即停止。
- `TODO.md` 是任务顺序、依赖、验证要求、完成记录的唯一权威来源。
- 若遇到阻塞当前任务的真实缺陷、规格不匹配或缺失特性，不绕过；必须先修复，或在 `TODO.md` 中插入最小前置任务后停止。
- 仅在阶段计划、依赖结构或完成标准变化时更新 `PLAN.md`。

### 执行步骤
1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务。
2. 查看最近一次提交信息，确认是否存在与该任务直接相关但未完成的事项；若有且确为当前任务前置内容，将其纳入当前任务或写入 `TODO.md`。
3. 阅读当前任务条目中的要求、依赖、验证标准与完成记录，并据此锁定需要修改的代码与测试位置。
4. 实现当前任务，优先采用最小正确改动，不引入规避性实现。
5. 运行该任务要求的验证命令，以及必要的回归测试；若失败，先修复再重试。
6. 更新 `memory/claude_plan.md` 记录关键进展与任何计划变更。
7. 将当前任务在 `TODO.md` 中标记为 `[DONE]`，补全完成记录；只有标题显式带 `[DONE]` 才算完成。
8. 若阶段计划被影响，再更新 `PLAN.md`；否则不改。
9. 按仓库提交规范创建一次 git 提交，包含本轮相关未提交文件。

### 进度记录
- 已开始：写入本计划文件。
- 已完成：读取 `TODO.md` 与最近一次提交，确认首个未完成任务为 `CG-T07S0a`。
- 观察：最近一次提交为 `[CG-T07S0a24] Route infer fixtures through typed HIR`，属于 `CG-T07S0a` 已登记前置任务之一；当前未发现需要在执行 `CG-T07S0a` 之前再新增前置项的直接证据。
- 已完成：`effect_handle_top_level_val_pattern_access_basic.scoop` 的单 fixture build/test 通过，说明任务原始 `top-level value ref` 故障已不再复现。
- 新观察：默认 `cargo run -p scoop -- test` 在用户中断前未暴露新的单 fixture 失败；根据用户反馈，当前阻塞转为“全部用例执行完成后命令挂起不退出”。
- 已缩小范围：用户确认挂起更可能位于 `scoop test --fixtures tests/fixtures/runtime_gc`。
- 计划调整：优先定位 `runtime_gc` phase 中具体导致收尾挂起的 fixture/runner 路径，并判断它是当前任务的未追踪前置 blocker，还是可直接在当前修复的 runner/runtime 缺陷。
- 已定位具体 fixture：`tests/fixtures/runtime_gc/gc_stw_cross_thread_roots_basic.scoop`。
- 关键证据 1：`tools/run_fixture_scan.sh --no-build --timeout-secs 20 tests/fixtures/runtime_gc` 显示该 fixture 是 `runtime_gc` 组唯一失败项，其余 24 个通过。
- 关键证据 2：`sample` 表明顶层 `scoop test` 卡在 `run_pass::run_command_collect_output()` 的 stdout/stderr reader `join()`；超时后只杀了外层 `scoop run`，后代 `a.out` 仍存活并继承 pipe，导致 runner 假性挂起。
- 关键证据 3：导出的 `/tmp/gc_stw_cross_thread_roots_basic.ll` 中 `@__scoop_top_level_var__fixtures.codegen.ready` / `proceed` 只有普通 `load`，没有对应 atomic store/load；worker/main 因此永远观察不到共享状态变化，程序本体卡住在 `waitWorkerReady()` / allocation loop。
- 结论：这是阻塞 `CG-T07S0a` full-suite 验收的未追踪前置缺陷，涉及 refactor LLVM 对 top-level `@Global __AtomicInt` lvalue 的 lowering 漂移，以及 run-pass timeout 未清理后代进程导致的假性 hang。
- 决策：按用户给定工作流，不在本轮继续修复代码；先在 `TODO.md` 前插最小 prerequisite 任务、把 `CG-T07S0a` 依赖更新为显式依赖该 blocker，然后提交并停止。
