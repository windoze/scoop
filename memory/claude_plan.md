## 执行计划

1. 检查最新一次提交信息，确认是否提到需要先修复的既有问题；若有，优先处理。
2. 读取 `TODO.md`，定位第一个未完成任务，并结合 `PLAN.md` 判断是否需要拆分为更小子任务。
3. 如果任务过大或存在前置缺口，先更新 `TODO.md` 与 `PLAN.md`，确保依赖顺序正确，然后在本轮只处理新的第一个任务。
4. 实现当前目标任务，尽量做最小且正确的修改，不绕过已有缺陷或规范缺口。
5. 运行与改动相关的测试，以及必要的质量检查；若发现既有问题，立即先修复或把它提升为前置任务。
6. 完成后更新 `memory/claude_plan.md`、`TODO.md`、`PLAN.md`，然后创建一次提交并停止，不继续处理下一个任务。

## 进度记录

- 已写入初始执行计划。
- 已检查最新提交 `9a9bbbb1af058a233d5c6000f253f965db09f0bb`，提交信息未声明需先修复的既有问题。
- 已读取 `TODO.md` 与 `PLAN.md`，当前首个未完成任务为 `T5001fR Review：确认默认 correctness 路线已真正切到 explicit root frame`。
- 当前执行策略：围绕默认 explicit mode 的 stackmap 残留依赖做代码审查与定向验证；若发现真实缺口，先修复缺口，再回写计划与任务文件。
- 审查中发现一处真实缺口：`__scoop_stackmap_statepoint_smoke()` 仍被保留为可选 stackmap smoke helper，但默认去掉所有 `gc "statepoint-example"` 后，它已无法再为调用点产出真实 statepoint/stackmap record。
- 已实施最小修复：仅对显式调用该 helper 的函数恢复 LLVM statepoint GC strategy，避免影响默认 explicit-root-frame 路线；同时补充 LLVM 回归锁定该 opt-in 边界。
- 已完成验证：`cargo test -p scoopc --lib`、`cargo run -p scoop -- test --fixtures tests/fixtures/build`、`cargo clippy -p scoopc --all-targets -- -D warnings` 全部通过。
- 已回写 `TODO.md` 与 `PLAN.md`：`T5001fR` 已标记完成，并记录本次 review 中发现并修复的 opt-in stackmap smoke 回归。
- 下一步：检查工作区差异，准备按 `[T5001fR] ...` 创建提交，然后停止，不继续处理 `T5001g`。
