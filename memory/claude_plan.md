# 本轮执行计划

## 约束与目标

- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后立即停止。
- 在开始任何命令执行前，先把计划写入本文件；后续若计划调整、发现阻塞、完成关键步骤，需要继续更新本文件。
- 若最新提交中提到已有问题，必须先修复这些问题，再继续当前任务。
- 若当前任务依赖缺失特性、现存缺陷或与规范不符的实现，不能绕过，必须先在 `TODO.md` / `PLAN.md` 中补入前置任务、调整顺序、提交后停止。
- 若任务过大，需要先拆分为更小的子任务，并同步更新 `TODO.md` 与 `PLAN.md`，本轮只执行拆分后的第一个子任务。

## 执行步骤

1. 检查最新一次 git 提交：
   - 查看提交说明是否提到已知问题、后续修复项、临时方案或未完成边界。
   - 如果有，需要先定位并修复，再跑相关测试。
2. 读取任务与规划文件：
   - 打开 `TODO.md`，定位第一个未完成任务。
   - 打开 `PLAN.md`，理解该任务的上下文、依赖、历史拆分情况。
3. 评估任务规模与依赖：
   - 判断该任务是否足够明确且能在本轮完整收敛。
   - 如果过大或存在前置缺口，先更新 `TODO.md` / `PLAN.md` 做拆分或重排。
4. 实施代码修改：
   - 在不回退用户现有改动的前提下，只修改与本轮任务相关的文件。
   - 若实现过程中发现规范不匹配或现存缺陷，停止“绕过式实现”，转为补任务和调整计划。
5. 验证：
   - 至少运行与改动直接相关的测试。
   - 若改动影响公共路径，补跑更高层级测试。
   - 按要求检查无告警，优先考虑 `cargo fmt`、相关测试、`cargo clippy --all-targets -- -D warnings`。
6. 文档同步：
   - 更新 `TODO.md`：完成则标记完成；若阻塞则保持 TODO 并挪到正确依赖位置。
   - 更新 `PLAN.md`：记录已完成内容、阻塞原因或任务拆分。
   - 更新本文件：记录关键发现、实际执行结果以及与初始计划不同的地方。
7. 提交：
   - 使用清晰的 git commit message。
   - 完成一个任务后立即停止，不继续下一个任务。

## 预期检查点

- 检查点 A：确认最新提交是否带有必须先修复的问题。
- 检查点 B：确认第一个未完成任务及其依赖是否清晰。
- 检查点 C：确认是否需要先拆任务或补前置缺陷任务。
- 检查点 D：实现完成并通过相关验证。
- 检查点 E：`TODO.md`、`PLAN.md`、本文件已同步，且已提交。

## 风险与处理原则

- 如果工作区已有未提交改动：
  - 先识别是否与本轮任务冲突。
  - 不回退非本人改动；若直接冲突，再根据实际情况调整实现方式。
- 如果测试暴露出前置 bug：
  - 将其视为真实项目问题处理，而不是临时规避。
- 如果 `README.md` 缺失关键用法或与实现明显不一致：
  - 在本轮任务影响范围内一并修正；若超出范围但阻碍任务验收，则纳入前置或并行修复。

## 本轮当前状态

- 已完成：初始化计划文件。
- 已完成：检查最新提交、`TODO.md`、`PLAN.md`，确认首个未完成任务为 `T4003SR`（复审普通顶层 `val` 的运行期读取主线）。
- 已完成：审读 `T4003S` 的主要改动面，覆盖 HIR `top_level_immutable_values` side table、LLVM once-init / backing global、reachability 扫描以及 effect state-machine 中对隐藏初始化边界的识别。
- 已完成：定向 probe 验证普通顶层 `val` 可在普通顶层函数中稳定读取：
  - `/tmp/t4003sr_fun_read.scoop` build 成功；
  - 运行输出为 `seed`、`41`、`41`，说明读取不只在 `main` 路径可用。
- 关键发现：
  - `/tmp/t4003sr_self_cycle.scoop`：
    - 源码：`val x: Int = x + 1`
    - build 成功；
    - 运行输出 `1`，退出码 `1`。
  - 这说明当前普通顶层 `val` 在同线程递归初始化时，会因为 once guard 的“重入直接返回 0 以避免死锁”语义，继续读取尚未完成初始化的 backing global 零值，从而把“未初始化读取”静默伪装成合法结果。

## 修复策略（已更新）

1. 保留当前“普通顶层 `val` 独立 side table + once-init + backing global”的主线，不回退到 `const val` 或静态 `var` 旁路。
2. 在 `codegen_top_level_immutable_value_access` 中：
   - 仍先调用对应的 init function；
   - 随后直接检查该 value guard 的状态是否已经进入 `initialized`；
   - 若 init 返回后 guard 仍处于 `initializing`，则将其视为“同线程递归初始化重入”，直接走稳定失败路径，禁止继续读取 backing global。
3. 新增回归时不用“直接自引用 + main 返回 x”这种会与旧错误语义偶然同码的写法，而是使用“helper 函数间接回读顶层 `val`”：
   - 旧错误行为：会打印/返回错误值并正常退出；
   - 修复后：应在首次读取时直接失败，且不会打印成功路径输出。
4. 修复后重新运行：
   - 顶层 `val` 定向 run-pass 子集；
   - `cargo test --all`；
   - `cargo clippy --all-targets -- -D warnings`。

## 当前下一步

- 已完成：修改 LLVM 顶层 immutable value 访问路径，在 init function 返回后检查 guard 是否真正进入 `initialized`，若仍处于 `initializing` 则以退出码 `1` 终止。
- 已完成：新增回归 `tests/fixtures/run-pass/top_level_val_recursive_init_is_error.scoop`，覆盖 helper 间接回读顶层 `val` 的递归初始化场景。
- 已完成：验证
  - `cargo run -p scoop -- test --fixtures /tmp/t4003sr-run-pass`（`fixtures: ok (2)`）
  - `cargo run -p scoop -- build tests/fixtures/build/top_level_val_read_minimal_ok.scoop -o /tmp/top_level_val_read_minimal_ok.out`
  - `/tmp/top_level_val_read_minimal_ok.out`（退出码 `42`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 当前下一步：检查最终 diff，提交 `T4003SR` 完成结果并停止。
