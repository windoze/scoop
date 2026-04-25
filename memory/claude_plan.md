# 本轮执行计划（初始）

## 目标

本轮只完成一项工作：先检查最新提交是否提到需要先修复的既有问题；若有，则优先修复这些问题。随后读取 `TODO.md`，定位第一项未完成任务，并在必要时将其拆分为更小的子任务，更新 `PLAN.md` 与 `TODO.md`。然后只执行排在最前面的那一项，完成实现、测试、文档更新与提交，最后停止。

## 已知约束

- 所有输出与记录使用中文。
- 在执行任何实际代码修改或命令前，先把计划写入本文件。
- 发现任何既有问题、规格不一致、实现边界缺失、回归或测试暴露的问题，都必须立即纳入本轮范围优先处理。
- 不能通过变通、缩小范围、替换表示方式、弱化测试形状等方式绕过问题。
- 本轮至多完成 `TODO.md` 中当前排在最前面的一个未完成任务；完成后必须停止。
- 修改计划、发现阻塞、完成关键步骤时，要及时更新本文件。

## 执行步骤

1. 查看最新一次 Git 提交，确认是否提到需要先修复的既有问题。
2. 读取 `TODO.md`、`PLAN.md`，确认当前第一项未完成任务及上下文。
3. 结合代码与测试现状评估该任务是否足够小：
   - 若任务过大，先拆分为更小的可执行子任务；
   - 更新 `TODO.md` 与 `PLAN.md`，并将第一项子任务作为本轮执行目标。
4. 在开始实现前，检查工作区状态，避免覆盖已有未提交改动。
5. 实现当前目标任务，必要时先修复在探查、编译、测试中发现的既有问题。
6. 运行相关验证：
   - 至少运行与修改直接相关的测试；
   - 若变更范围需要，运行更广的测试；
   - 最终运行 `cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`（若时间和变更范围允许且与本任务相关）。
7. 更新文档与任务状态：
   - 在 `TODO.md` 中将本轮完成的任务标记为完成；
   - 在 `PLAN.md` 中记录当前状态、后续依赖或调整；
   - 如计划发生变化，同步更新本文件。
8. 检查变更，使用清晰的 Git 提交信息提交本轮工作。
9. 提交后停止，不继续处理下一项任务。

## 风险与决策原则

- 如果最新提交或当前代码状态暴露出先于任务的缺陷，先修复缺陷，再决定是否还来得及完成当前任务。
- 如果任务依赖缺失特性或错误实现，不能绕过；必须把该依赖作为前置任务写入 `TODO.md`，更新 `PLAN.md`，提交后停止。
- 如果测试暴露出与当前任务无关但已存在的真实缺陷，只要是在当前探查路径中发现，也要按要求优先处理。

## 进度记录

- 初始计划已写入，尚未开始仓库检查。
- 已检查最新提交 `2ddbc085653f799f58c23b7de99be2256685c9eb`，提交信息仅为 `[T5000b3c] Split closure and class ctor lowering modules`，未额外提及需要先修复的既有问题。
- 已读取 `TODO.md` 与 `PLAN.md`，确认当前首个未完成任务为 `T5000b3cR Review：确认 closure/ 与 class_ctor.rs 主题边界成立`。
- 当前 review 重点：
  - 检查 closure env/body lowering 是否仍散落在 `call/`、`effect/`、`intrinsics/` 或 `codegen/mod.rs`；
  - 检查 class ctor 选择、实参求值、super/delegation、init-step lowering 是否已经集中到 `class_ctor.rs`；
  - 检查这些主题与 `call/`、`intrinsics/` 的接口是否保持为窄委托，而不是相互夹杂主体实现。
- review 过程中发现并修复了一个既有文档问题：
  - `crates/scoopc/src/llvm/codegen/class_ctor.rs` 顶部注释仍写着“仅支持 positional args，不支持 named/default args”；
  - 但实现已经通过 `CtorCallInfo::arg_mapping` 与 `default_value` 支持 named/default args；
  - 已修正注释，使其准确描述“优先使用 `CtorCallInfo`，无 side-table 的内部路径才退回 positional-only”。
- 已完成 `T5000b3cR` 的 review 结论整理，并同步更新 `TODO.md` / `PLAN.md`：
  - closure 主题边界已稳定收口到 `crates/scoopc/src/llvm/codegen/closure/mod.rs`；
  - class ctor 主题边界已稳定收口到 `crates/scoopc/src/llvm/codegen/class_ctor.rs`；
  - `call/dispatch.rs`、`expr.rs`、`effect/mod.rs`、`intrinsics/{sync,thread}.rs` 当前只保留窄委托接口；
  - 下一条待执行任务已切换为 `T5000b3d`。
- 验证结果：
  - `cargo fmt --all`
  - `cargo test -p scoopc llvm::`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 全部通过。
