## 工作计划

说明：我不会记录或暴露私有推理细节，但会在这里维护可执行的步骤计划、关键判断依据与进度更新，便于你随时查看。

### 初始计划

1. 检查最新一次 git 提交信息，确认是否显式提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认当前项目计划与该任务的上下文。
4. 如首个未完成任务过大，先拆分任务，并更新 `TODO.md` 与 `PLAN.md`。
5. 实现当前应执行的首个任务。
6. 运行相关测试、格式化、lint；若发现既有问题，立即优先修复或把它作为前置任务写回 `TODO.md`。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况。
8. 按仓库约定创建一次 git 提交，然后停止。

### 进度

- 已创建计划文件，准备开始仓库检查。
- 已检查最新提交：`[T5000j3R] Review higher-order and init backend boundary`，提交说明未显式声明需要先修复的新既有缺陷。
- 已读取 `TODO.md` / `PLAN.md`，定位首个未完成任务为 `T5000j4 建立 safepoint 数量 / roots 压力的可复验跟踪基线`。
- 当前执行计划更新：
  1. 调查仓库里现有的 safepoint / roots / GC 压力观测入口、脚本、文档与测试载体。
  2. 选择最小但可复验的 workload 与观测口径，并实现文档/脚本/测试支撑。
  3. 运行相关验证，若暴露既有问题则先修复。
  4. 回写 `TODO.md`、`PLAN.md` 与本文件，提交后停止。
- 已完成 `T5000j4` 实现：
  1. 新增 `cargo run -p scoop_tools -- safepoint-baseline`，自动构建内置 workload 并统计 `statepoints` / `gc-live` roots。
  2. 新增两个 build workload fixture，并复用现有 `task_step_cross_thread_sequential_handoff_gc_stress.scoop` 作为高压样本。
  3. 已把方法与当前快照固化到 `docs/safepoint_baseline.md`，并更新 `README.md` / `tools/README.md`。
  4. 已完成验证：`cargo fmt --all`、`cargo test -p scoop_tools`、`cargo run -p scoop_tools -- safepoint-baseline`、`cargo run -p scoop -- test --fixtures tests/fixtures/build`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
- 当前结论：
  - 小 direct-call wrapper 与 non-escaping closure 在 `O2` 下明显减少 safepoint 与 roots 压力；
  - task/effect/thread handoff 高压路径虽然降低了总 roots / 峰值 roots，但绝对压力仍高；
  - 因而当前更值得继续减少调用边界，而不是把 `mem2reg` / register-root 提升为下一优先级。
- 待做：按用户要求仅完成一个任务后停止；下一次调用应从 `T5000j4R` 开始。
