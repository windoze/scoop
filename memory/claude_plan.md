# 本轮执行计划

说明：按要求记录可执行计划、关键判断、检查结果与进度更新；不记录逐字内部推理。

## 初始计划

1. 检查最新一次 Git 提交，确认提交信息或提交内容中是否提到需要先修复的既有问题。
2. 阅读 `TODO.md` 与 `PLAN.md`，识别第一个未完成任务，并确认是否需要拆分。
3. 如果任务过大：
   - 在 `PLAN.md` 中补充细化步骤；
   - 在 `TODO.md` 中把该任务拆成更小的子任务，并把当前要执行的第一个子任务放到首个未完成位置。
4. 实现当前应执行的首个任务或子任务。
5. 运行相关检查与测试，至少包括与改动相关的测试；如涉及全局质量约束，补充运行 `cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 或必要子集。
6. 更新文档与计划：
   - 在 `TODO.md` 中标记当前任务完成，或在阻塞时按依赖顺序重排；
   - 在 `PLAN.md` 中记录当前状态、阻塞关系和后续计划；
   - 按需要继续更新本文件。
7. 使用清晰的提交信息提交本轮所有变更，然后停止，不继续处理下一个任务。

## 进度记录

- 已创建本文件，准备开始仓库检查。
- 已检查最新提交 `b05f769fcc3b5849b93227779df27a131f8be47e`（`[T3016dR] Review GC-stress continuation root contract`）。提交信息与变更说明未点名新的“必须先修”的遗留问题；当前已知待办仍以 `TODO.md` / `PLAN.md` 记录为准。
- 已确认首个未完成任务是 `T3017`：回收 `T3006` 暂时 xfail fixtures，恢复 effect run-pass 基线。
- 已复核 `T3017` 的现有说明：此前为执行它而新增的四组前置问题（`T3016a`/`b`/`c`/`d`）都已完成，因此当前可以正式执行该任务。
- 当前执行策略更新：
  1. 先枚举 `tests/fixtures/run-pass` 下仍为 `EXPECT: fail` 的 effect / continuation / GC continuation 相关 fixtures。
  2. 在临时目录中把这些候选 fixtures 改成 `EXPECT: pass`，使用官方 `scoop test --fixtures <tempdir>` 统一验证，识别哪些只是 stale expectation。
  3. 若验证中出现新的真实生产缺口，按规则回写 `TODO.md` / `PLAN.md` 并停止；若没有，则回收仓库内对应 expectation 与过时注释。
  4. 完成后运行任务要求的全量验证，并更新 `TODO.md`、`PLAN.md`、本文件与 Git 提交。
- 已完成临时扫描：
  - 在 `/tmp/scoop_t3017_scan` 中复制了 61 条 effect / continuation 相关 `run-pass` xfail fixture，并统一改成 `EXPECT: pass`。
  - 官方 runner 首轮即在 `effect_handler_stack_nearest_three_levels_and_arm_outside_scope.scoop` 失败；继续直接运行后确认，pass 基线里的 `effect_handler_stack_nearest_and_arm_outside_scope.scoop` 与 `effect_custom_nonresuming_nested_nearest_and_arm_outside_scope.scoop` 也同样失败。
  - 真实现象：inner arm/catch 内再次 `Raise.raise(...)` / `Boom.boom(...)` 后，程序错误继续执行 `unreachable_after_inner` / `unreachable_after_middle`，没有向外层最近 handler/catch 传播。
  - 结论：这不是 stale expectation/golden，而是新的前置生产回归；已决定新增 `T3016e` / `T3016eR`，把 `T3017` 顺延到其后，本轮按阻塞处理收尾。
