# 本轮执行计划

## 目标

本轮只完成 `TODO.md` 中当前排在最前面的未完成任务；如果在执行前或执行中发现更早存在的问题、回归、规范不匹配、实现边界缺失或最新提交明确提到的待修问题，则先处理这些问题，再决定是否继续当前任务。

## 执行原则

- 先看最新提交，确认是否显式提到未修复问题或已知缺陷。
- 再读 `TODO.md`、`PLAN.md`，定位首个未完成任务与其依赖关系。
- 若任务过大，先拆分任务并更新 `TODO.md`/`PLAN.md`，然后只执行拆分后的第一个子任务。
- 任何已有问题一旦确认阻塞规范正确实现，就必须先修复，或把修复任务插入到被阻塞任务之前，然后提交并停止。
- 完成任务后必须更新 `TODO.md`、`PLAN.md`、本文件，并提交一次 git commit，然后停止。

## 初始步骤

1. 查看最新一次提交的提交信息和改动上下文，确认是否包含待修问题说明。
2. 检查工作区状态，避免误覆盖已有未提交修改。
3. 阅读 `TODO.md` 与 `PLAN.md`，识别当前首个未完成任务。
4. 如有必要，补充阅读与该任务相关的代码、测试、规范或 README。
5. 实施修改并补充/调整测试。
6. 运行与任务相关的测试、`cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`（若时间或环境限制导致无法完整执行，会在本文件与最终说明中明确记录）。
7. 更新 `TODO.md`、`PLAN.md`、本文件中的进度记录，提交本轮改动。

## 进度记录

- 已创建本文件，准备开始仓库检查与任务定位。
- 已检查最新提交：`0bfad5e6bbc1ec6b930fa6226b385daba987e0c3 [T5000e2b] Materialize owner-specialized MIR instances`。
  - 提交信息本身未显式声明“已知待修 bug”。
- 已定位 `TODO.md` 当前首个未完成任务为 `T5000e2bR Review：确认 owner/nominal specialization 已进入 MIR instance collection 语义`。
- 当前 review 计划：
  1. 审阅 `crates/scoopc/src/mir/materialize.rs`、`typecheck/expr/call.rs`、`typecheck/expr/member.rs`、`typecheck/lower.rs`、`monomorph/mod.rs`，核对 owner-specialized member/getter 的实例身份是否已进入 MIR request/materialization 主线。
  2. 搜索 HIR 侧 `collect_generic_member_fun_instantiations(...)` 等残留入口，确认它是否仍承担 MIR collection 的主发现职责，还是仅为后续 `T5000e2c` 尚未切换的 frontend/build 路径保留。
  3. 运行与 owner-specialized materialization 相关的针对性测试；若 review 暴露真实回退，则先修复问题并补测试。
  4. 若未发现阻塞问题，则更新 `TODO.md`、`PLAN.md` 与本文件，记录 review 结论并提交。
- review 实际发现的缺口：
  - `mir/materialize.rs` 原先只会扫描请求源文件中的非泛型 root，以及后续真正被 materialize 的 generic family。
  - 如果 owner-specialized getter/member 只出现在跨文件非泛型 helper 中，或 generic instance 再调用非泛型 helper 后才触发，则该实例不会进入 MIR request 集合；这不满足 `reachable-driven / on-demand` 验收。
- 已完成修复：
  - 在 `crates/scoopc/src/mir/materialize.rs` 中新增 `CallableBodyInfo`、完整 direct-call binding lookup、可达 non-generic body 扫描与递归 helper 路径发现；
  - request-root seeding 与 generic instance rewrite 现在都会沿 direct-call 可达图继续发现非泛型 helper 内部触发的 generic owner member/getter 实例；
  - 已新增两条回归测试，覆盖“请求根跨文件 helper”和“generic instance 经由非泛型 helper”两条路径。
- 已完成验证：
  - `cargo test -p scoopc typechecked_compilation_unit_materialization_reaches_owner_specialized_getter_through_cross_file_non_generic_helper -- --nocapture`
  - `cargo test -p scoopc typechecked_compilation_unit_materialization_reaches_owner_specialized_getter_through_non_generic_helper_called_by_generic_instance -- --nocapture`
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 以上均已通过。
- 当前收尾状态：
  - `TODO.md` / `PLAN.md` 已更新为 `T5000e2bR` 完成；
  - 下一条待执行任务已切换为 `T5000e2c`；
  - 下一步只剩检查工作区、提交本轮改动并停止。
