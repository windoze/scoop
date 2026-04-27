# 执行计划

## 目标

完成 `TODO.md` 中第一个未完成任务；如果最新提交提到已有问题，则先修复该问题；完成后更新文档、运行测试、提交 Git，然后停止。

## 初始步骤

1. 检查最新一次 Git 提交，确认是否提到了需要先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对现有计划与任务依赖。
4. 如当前任务过大或被既有问题阻塞：
   - 细化为更小的子任务，更新 `PLAN.md`。
   - 按依赖顺序更新 `TODO.md`。
   - 本次只执行调整后排在最前面的那个任务，并在必要时立即停止。

## 执行原则

- 不绕过既有缺陷、规格不匹配或实现边界。
- 发现阻塞当前任务的已有问题时，先修复；若无法当场修复，则把它作为前置任务插入 `TODO.md`，更新 `PLAN.md` 后提交并停止。
- 修改过程中持续更新本文件，记录关键步骤、判断和当前状态。

## 预期收尾

1. 实现当前目标任务。
2. 运行相关测试、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`，并修复暴露的问题。
3. 更新 `TODO.md`、`PLAN.md` 和本文件。
4. 使用清晰的提交信息完成 Git 提交。
5. 停止，不继续下一个任务。

## 当前状态

- 已完成：
  - 检查最新提交标题，未发现直接声明需先修复的未解决既有缺陷。
  - 读取 `TODO.md` / `PLAN.md`，确认首个未完成任务为 `T5000h 在 MIR 层实现 summary-driven inlining`。
  - 核对 `crates/scoopc/src/mir/materialize.rs`、`crates/scoopc/src/mir/summary.rs`、`crates/scoopc/src/hir/lower/mod.rs` 与相关计划记录，确认当前 build/frontend 主路径虽然已切到 MIR instance collection，但仍只消费 `materialized.instance_keys`，不会消费 `MaterializedMir.file` 或 `MaterializedMir.summaries`。

## 新发现的前置缺口

- `MaterializedMir.summaries` 当前只在 `crates/scoopc/src/mir/summary.rs` 自测中被读取；生产代码没有消费者。
- build/frontend 主路径在 `crates/scoopc/src/hir/lower/mod.rs` 中调用 `materialize_compilation_unit_from_typechecked_inputs(...)` 后，只把 `materialized.instance_keys` 与 `materialized.types` 传给 HIR 兼容 lowering；materialized MIR body 与 summary side table 都被丢弃。
- 这意味着如果现在直接实现 `T5000h` 的 MIR 内联：
  - dump/test 路径可能能看到 rewrite；
  - 但 production/codegen 主路径看不到这些 rewrite；
  - 无法满足“codegen 不再承担内联后才能去掉的额外高层调用边界”这一验收要求。

## 调整后的执行判断

- `T5000h` 目前不能按原顺序直接实现，必须先补一个前置任务：
  - 让 build/frontend 主路径消费 materialized MIR body / 后续 MIR pass 产物，而不是仅把 MIR 当作实例集合发现器。
- 已完成：
  - 已更新 `TODO.md`，在 `T5000h` 前插入 `T5000h0` / `T5000h0R`，并把 `T5000h` 依赖改为 `T5000h0R`。
  - 已更新 `PLAN.md`，记录 production 主路径当前仍只消费 `instance_keys` 的阻塞证据，并把高层执行顺序调整为“先接 materialized MIR body，再做 MIR inlining”。
- 下一步：
  1. 复核变更与 Git 状态。
  2. 提交本轮对 `TODO.md` / `PLAN.md` / 本文件的更新。
  3. 停止，等待下一次调用执行新的首个未完成任务 `T5000h0`。
