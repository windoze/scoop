# 当前回合执行计划

## 约束与执行边界

- 本回合只处理 `TODO.md` 中第一个未完成任务；如果发现它过大或被前置问题阻塞，会先更新 `PLAN.md` 与 `TODO.md`，然后只执行拆分后的第一个子任务或记录阻塞调整后停止。
- 在开始实现当前任务前，先检查最近一次提交是否提到已知问题；若有，先修复这些问题并验证，再继续当前任务。
- 所有实现必须符合规范，不能依赖临时绕过方案；一旦发现规范不匹配或缺失能力，需要先把问题前移登记到 `TODO.md`/`PLAN.md`。
- 需要在过程中持续更新本文件，记录计划变化、关键发现、已完成步骤和待验证事项。

## 可共享的思路摘要

- 先收集事实，避免在不了解代码库当前状态时直接改代码。
- 优先确认“最新提交中是否已经暴露出必须先修的遗留问题”，因为这是用户给出的最高优先级前置要求。
- 再定位 `TODO.md` 里的首个未完成任务，并评估它是否可在一个提交中完整落地。
- 如果任务边界清晰，则直接实现并补齐测试；如果任务边界过大，则把它拆成更小、可验证、可提交的子任务，并只完成第一个。
- 实现后用与改动最相关的测试、全量必要检查、以及无 warning 的构建/静态检查来收尾。
- 最后同步 `TODO.md`、`PLAN.md`、本文件，并创建一次清晰的 Git 提交后停止。

## 具体步骤

1. 查看最近一次 Git 提交的说明与改动上下文，确认是否提到了尚未修复的问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md` 与相关代码，判断该任务的范围、依赖和验收方式。
4. 如任务过大或被缺陷/缺失特性阻塞：
   - 更新 `PLAN.md`，细化子任务或记录阻塞原因。
   - 调整 `TODO.md` 顺序与依赖，使第一个未完成项反映真实优先级。
   - 若本回合只能完成计划调整，则提交这些文档变更并停止。
5. 如任务可执行：
   - 修改实现代码。
   - 为改动补充或调整测试。
   - 运行相关测试，再运行必要的工作区检查（至少覆盖构建、测试、`clippy -D warnings`）。
6. 将任务状态同步到 `TODO.md` 和 `PLAN.md`。
7. 复查工作区变更，确保没有误改或漏改。
8. 提交本回合所有变更，提交信息聚焦本次完成的任务。

## 进度记录

- 2026-04-19: 已创建本文件并写入初始计划；尚未开始读取仓库状态文件。
- 2026-04-19: 已检查最新提交 `53ab29854fa4023f09bb960845ee632a4fa819fe`，提交说明未额外声明新的遗留问题；当前工作区仅有本文件修改。
- 2026-04-19: 已读取 `TODO.md` 与 `PLAN.md`，确认首个未完成任务为 `T4006U`：修复 full fixture suite 中 `top_level_val_recursive_init_is_error` 的顺序相关 stdout mismatch。`T4006V` 与 `T4006R` 依赖它，因此本回合目标锁定为该任务。
- 2026-04-19: 当前执行策略更新为：
  1. 读取 `top_level_val_recursive_init_is_error` fixture 与 test runner 实现。
  2. 先复现“单独运行通过、全量运行失败”的差异。
  3. 定位是 fixture harness 状态污染、编译缓存/产物复用问题，还是运行时状态未清理。
  4. 修复后补充或调整回归验证，确保全量 `cargo run -p scoop -- test` 不再在该 fixture 上失败。
- 2026-04-19: 复现后修正了最初假设。关键事实如下：
  - `target/debug/scoop run tests/fixtures/run-pass/top_level_val_recursive_init_is_error.scoop` 的实际行为稳定为：退出码 `1`，stdout/stderr 都为空。
  - `target/debug/scoop test --fixtures tests/fixtures/run-pass` 与“仅含该 fixture 的临时 root”都会失败在同一处，因此问题不是全量顺序污染，而是该 fixture 自身期望与当前实现不一致。
  - `tests/fixtures/run-pass/top_level_val_recursive_init_is_error.stdout` 实际包含 1 个字节（单独一个 `\n`），这与“递归初始化在进入 `println(x)` 前即终止”的当前语义冲突。
  - 该 golden 文件由历史提交 `[T4003SR] 修复顶层 val 递归初始化读取` 引入，当前判断为陈旧 golden，而非新的 codegen/runtime 回退。
- 2026-04-19: 计划调整为：
  1. 将 `top_level_val_recursive_init_is_error.stdout` 改为空文件，使其与当前退出前无 stdout 的语义一致。
  2. 验证单 fixture、`tests/fixtures/run-pass` 子集，以及全量 `cargo run -p scoop -- test` 已越过该失败点。
  3. 复跑 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
  4. 根据验证结果更新 `TODO.md` / `PLAN.md`，记录 `T4006U` 的真实根因与完成状态。
- 2026-04-19: 已完成代码/fixture 修改：
  - `tests/fixtures/run-pass/top_level_val_recursive_init_is_error.stdout` 已改为空文件。
  - `tests/fixtures/run-pass/top_level_val_recursive_init_is_error.scoop` 已补注释，明确程序会在进入 `main` 前终止，因此 stdout 为空。
- 2026-04-19: 已完成验证：
  - 单 fixture 临时 root：`fixtures: ok (1)`。
  - `target/debug/scoop test --fixtures tests/fixtures/run-pass`：`fixtures: ok (346)`。
  - `cargo run -p scoop -- test`：`fixtures: ok (1051)`。
  - `cargo test --all`：通过。
  - `cargo clippy --all-targets -- -D warnings`：通过。
- 2026-04-19: 已更新 `TODO.md` / `PLAN.md`，将 `T4006U` 标记为完成，并把下一项切换为 `T4006V`。剩余待做仅为整理工作区、提交本回合变更并停止。
