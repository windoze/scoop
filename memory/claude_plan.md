## 当前执行计划

1. 记录本轮目标与约束，只处理 `TODO.md` 中第一个未完成任务。
2. 检查最新一次提交，确认是否提到需要先修复的既有问题；若有，优先处理。
3. 阅读 `TODO.md` 与 `PLAN.md`，定位第一个未完成任务，并判断是否需要拆分。
4. 如任务过大，则先更新 `PLAN.md` 与 `TODO.md`，把任务拆成更小的前置子任务；本轮仅执行拆分后的第一个子任务。
5. 实现当前目标，必要时补充或调整测试。
6. 运行相关验证命令；若发现既有缺陷、回归或规格不匹配，优先修复或把其作为前置任务写入 `TODO.md` 后停止。
7. 完成后更新 `memory/claude_plan.md`、`PLAN.md`、`TODO.md`，然后按仓库约定提交一次 git commit，并停止。

## 进度记录

- 已创建本计划文件。
- 已检查最新提交 `69b6b65c9de84fb059b1f9b219930a774410d48b`，提交信息未声明需要先修复的既有问题。
- 已读取 `TODO.md` 与 `PLAN.md`，确认首个未完成任务是 `T5001d1R Review`，当前无需先拆分任务。
- 已完成首轮审查，确认两个阻塞 `T5001d2` 的缺口：
  - 顶层不可变值初始化函数设置了 GC 策略，但没有开启/结束 explicit frame layout 规划，因此不会发 descriptor。
  - effect state-machine 的 `step/dispatch` 托管函数同样缺少 layout 规划；其函数级返回槽位与 `handle_result_slot` 也绕过了 tracked entry alloca 路径。
- 已修复上述缺口，并补充 LLVM 回归测试覆盖顶层值初始化函数与 effect state-machine 托管函数的 descriptor 发射。
- 已完成验证：`cargo test -p scoopc --lib`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test --fixtures tests/fixtures/build` 全部通过。
- 下一步：回写 `TODO.md` / `PLAN.md` 后整理提交，提交完成即停止本轮。
