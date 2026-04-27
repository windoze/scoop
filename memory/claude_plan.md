# 本轮执行计划

## 说明

按要求，本文件先于仓库探查与命令执行创建，用于记录本轮的执行计划、关键决策、阻塞点与进度更新。
出于协作与安全边界，这里记录的是可审计的任务计划、检查项与结论摘要，不写入内部私有推理全文。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果在执行过程中发现更早应修复的既有问题，则先修复该问题，或将其作为前置任务插入 `TODO.md` 后停止。

## 执行步骤

1. 检查最新一次 Git 提交信息，确认是否提到需要优先修复的既有问题。
2. 阅读 `TODO.md`，找出第一个未完成任务。
3. 阅读 `PLAN.md`，确认该任务上下文、依赖与当前计划是否一致。
4. 如果任务过大：
   - 将任务拆分为更小的子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md`，把新的前置子任务插入到正确位置；
   - 本轮只执行拆出的第一个子任务。
5. 实现目标任务，并在实现过程中同步记录关键变更与发现。
6. 运行相关验证：
   - 至少运行与改动直接相关的测试；
   - 如有必要，运行更广泛测试；
   - 运行 `cargo fmt`；
   - 运行 `cargo clippy --all-targets -- -D warnings`，确保无警告。
7. 如果发现既有 bug、回归、规格不匹配或实现边界缺口：
   - 立即优先修复；或
   - 若无法在本轮直接修复，则把它作为当前任务的前置任务插入 `TODO.md`，更新 `PLAN.md`，提交后停止。
8. 完成后更新：
   - `TODO.md` 中将该任务标记完成；
   - `PLAN.md` 反映当前状态与后续顺序；
   - 本文件记录结果与验证情况。
9. 提交 Git，提交信息使用清晰描述并尽量带任务号。
10. 停止，不继续下一个任务。

## 当前状态

- 状态：已完成初始化与任务定位，当前任务锁定为 `T5000h0dR Review`。
- 已完成：
  - 检查最新提交：`[T5000h0d] Add canonical materialized MIR pass artifacts`，提交标题未声明新的待优先修复既有问题；
  - 读取 `TODO.md` / `PLAN.md`，确认第一个未完成任务是 `T5000h0dR`；
  - 开始审阅 `crates/scoopc/src/mir/pass_view.rs`、`materialize.rs`、`callables.rs`、`summary.rs`、`hir/lower/types.rs` 及 production 接线位置。
- 当前 review 关注点：
  - pass view 是否已经脱离 raw `MaterializedMir` 的薄包装；
  - rewritten callable body / summary / family 映射是否有稳定查询入口；
  - production 侧是否只是“携带 pass view”，还是已经把它明确当作 canonical pass 输出边界保留下来。
- 下一步：
  - 继续核对 `llvm/emit.rs`、`llvm/codegen/mod.rs`、frontend/build 接线；
  - 运行针对性测试与全量检查；
  - 若未发现前置缺陷，则把 `T5000h0dR` 标记完成并更新 `PLAN.md` / `TODO.md` 后提交。

## 执行中发现与修复

- review 过程中发现一个既有一致性问题：
  - `crates/scoopc/src/mir/callables.rs` 中 `MaterializedCallableFamilies::replace_family(...)` 之前只在 debug 下通过 `debug_assert!` 假设 callable 不会跨实例 family 迁移；
  - 但在 release 构建里，如果 pass 把某个 callable 重挂到另一个 family，旧 family 的 `callable_fqns` 会静默残留该 symbol，导致同一 callable 同时出现在两个 family。
- 已实施修复：
  - `replace_family(...)` 现在会在重写 family 时同步从旧 owner 的 `callable_fqns` 中移除迁出的 symbol；
  - 对输入 `callable_fqns` 增加稳定去重，避免 pass side table 自身引入重复 callable 成员。
- 已新增回归测试：
  - `mir::pass_view::tests::pass_view_rehomes_callable_across_families_without_leaving_duplicate_membership`

## 当前结论

- `MaterializedMirPassView` 已经建立在独立的 `MaterializedMirPassArtifacts` 之上，不再只是 raw `MaterializedMir` 的薄包装；
- rewritten callable body / summary / family 映射现在都有稳定查询入口；
- 未发现需要插入到 `T5000h0e` 之前的新前置任务；下一步可直接进入 production codegen 真正消费 pass-rewritten callable body / summary 的接线任务。

## 验证状态

- 已通过：
  - `cargo fmt --all`
  - `cargo test -p scoopc mir::pass_view -- --nocapture`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo clippy --all-targets -- -D warnings`
- 当前剩余动作：
  - `TODO.md` / `PLAN.md` / 本文件已更新完成；
  - 检查 diff；
  - 以 `T5000h0dR` 对应提交信息提交；
  - 停止，不继续下一个任务。
