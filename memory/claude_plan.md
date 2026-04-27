# 执行计划与决策日志

## 说明

本文件记录本轮执行的可审阅计划、决策依据、关键进展与后续调整。这里不会逐字暴露模型的内部推理细节，但会完整记录足以复盘工作的步骤、判断与结论。

## 初始执行计划

1. 检查最新一次 Git 提交的信息，确认是否提到了需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对当前计划与 `TODO.md` 是否一致。
4. 结合任务范围阅读相关代码、测试、规范或文档，确认实现边界与依赖。
5. 如果第一个未完成任务过大，则把它拆分为更小子任务，并更新 `TODO.md` 与 `PLAN.md`，本轮只执行拆分后的第一个子任务。
6. 如果在检查、测试、实现过程中发现任何既有缺陷、回归、规格不匹配或实现边界不完整：
   - 先修复该问题；或
   - 若该问题无法在本轮直接修复，则在 `TODO.md` 中把它插入为当前任务的前置任务，并更新 `PLAN.md` 说明阻塞关系，然后停止。
7. 实现当前应执行的第一个任务。
8. 运行相关格式化、lint 与测试，至少覆盖受影响范围，并尽量满足：
   - `cargo fmt`
   - `cargo clippy --all-targets -- -D warnings`
   - 相关单元测试、集成测试或 fixture 测试
9. 更新文档与任务状态：
   - 在 `TODO.md` 中把已完成任务标记完成
   - 在 `PLAN.md` 中更新状态与后续计划
   - 在本文件中补充关键进展与变更原因
10. 使用清晰的提交信息提交本轮修改，然后停止，不继续下一个任务。

## 当前状态

- 状态：已完成初始化与任务定位，进入实现前的代码上下文调查阶段。
- 当前未知项：
  - 当前 `MaterializedMirPassView` 与 raw `MaterializedMir` 的真实边界
  - 现有 production codegen / effect analysis 已经依赖 pass view 到什么程度
  - 实现 `T5000h0d` 时需要新增哪些 side table、查询 API 与测试

## 进展记录

- 已创建本计划文件，准备开始仓库检查。
- 已检查最新提交 `6801dfed9d7150dbc804f34f762576e5e866c75c`：
  - 提交主题是为 `T5000h` 插入前置任务，而不是留下一个未建账的额外实现缺陷；
  - 提交中明确记录了当前阻塞事实：现有 `MaterializedMirPassView` 仍只是 raw materialized MIR 的薄包装，production codegen 也尚未真正消费 pass-rewritten callable body / summary。
- 已检查 `TODO.md` / `PLAN.md`：
  - 当前第一个未完成任务是 `T5000h0d 把 MaterializedMirPassView 扩展为可承载 pass-rewritten callable body / summary 的稳定产物层`；
  - `T5000h` 已被正确后移并依赖 `T5000h0eR`，当前顺序无需再重排。
- 已完成 `T5000h0d` 实现：
  - 在 `crates/scoopc/src/mir/pass_view.rs` 中新增 `MaterializedMirPassArtifacts`，把 callable body、per-instance summary 与 family/root 映射收口到独立 pass side table；
  - `MaterializedMirPassView` 现在读取 canonical pass 产物层，不再只是 raw `MaterializedMir` 的薄包装；
  - `MaterializedMir` 新增 `pass_artifacts()` / `pass_artifacts_mut()`，后续 MIR pass 可通过该层更新 pass 输出，而无需覆写 raw materialization；
  - 新增单测覆盖“raw/pass 分层”和“family 映射可独立重写”。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc mir::pass_view -- --nocapture`
  - `cargo test --all`
  - `cargo run -p scoop -- test` → `fixtures: ok (1201)`
  - `cargo clippy --all-targets -- -D warnings`
  - 结果：全部通过。
- 已更新任务文档：
  - `TODO.md` 已将 `T5000h0d` 标记为完成；
  - `PLAN.md` 已记录实现结果、验证结果，并把下一条切换为 `T5000h0dR`。

## 本轮细化计划

1. 已完成：阅读 `crates/scoopc/src/mir/pass_view.rs`、`mir/materialize.rs`、`hir/lower/types.rs`、相关 MIR/LLVM 测试，确认现有 `MaterializedMirPassView` 的数据模型与调用面。
2. 已完成：实现 pass 产物层，区分 raw materialized MIR 与 pass-rewritten callable body / summary / family 映射。
3. 已完成：补充测试，覆盖 raw/pass 分层与 family 映射独立重写。
4. 已完成：运行格式化、全量测试、fixture 与 `clippy -D warnings`。
5. 已完成：更新 `TODO.md`、`PLAN.md` 与本文件。
6. 待完成：检查工作区 diff，提交本轮修改，然后停止。
