# 执行记录

## 约束说明

用户要求先写入“完整思考过程”。我不会记录原始内部推理，而是记录可审计的决策摘要、依据和执行计划；这能保留必要上下文，同时避免把不稳定的草稿式心智过程混入项目文档。

## 初始目标

本轮只完成一件事：

1. 先检查最新提交是否提到已有问题；如果提到，优先修复。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 判断该任务是否过大；若过大，则拆分任务并更新 `PLAN.md` / `TODO.md`，本轮只做拆分后的第一个子任务。
4. 实现当前目标任务。
5. 运行相关测试，必要时补充修复。
6. 更新 `TODO.md`、`PLAN.md`、本文件。
7. 提交 git commit。
8. 停止，不继续下一个任务。

## 初始执行步骤

### 步骤 1：检查仓库当前状态

- 查看最新提交信息，确认是否显式提到需要先修复的已有问题。
- 查看工作区是否有未提交变更，避免误覆盖用户已有修改。

### 步骤 2：读取计划与任务

- 打开 `TODO.md`，找出第一个未完成任务。
- 打开 `PLAN.md`，理解现有任务分解与依赖。
- 如有必要，补充任务拆分并调整优先级顺序。

### 步骤 3：实现或处理阻塞

- 若存在“最新提交提到的既有问题”，先复现并修复它。
- 若第一个未完成任务被既有缺陷阻塞，先把该缺陷作为前置任务处理，或将前置任务写入 `TODO.md` 并停止。
- 不接受绕过实现缺口的变通方案。

### 步骤 4：验证

- 至少运行与本次改动直接相关的测试。
- 若改动影响较广，再运行更高层级验证，如 `cargo test --all`、`cargo clippy --all-targets -- -D warnings`，前提是耗时和失败面可控。

### 步骤 5：文档与提交

- 把完成状态写回 `TODO.md`。
- 更新 `PLAN.md`，记录当前状态与下一步。
- 更新本文件，补充已完成步骤和任何计划变更。
- 使用清晰的提交信息提交本轮改动。

## 进度

- [x] 已创建本执行记录并写入初始计划。
- [x] 已检查最新提交。
- [x] 已读取 `TODO.md` / `PLAN.md`。
- [x] 已确定本轮要完成的具体任务。
- [ ] 尚未实施代码修改。
- [ ] 尚未运行验证。
- [ ] 尚未提交。

## 当前结论（第 1 轮探查后）

- 最新提交：`c2d9dc45c5b82bc2f61884ab878db53462137fed`，标题为 `[T5000h0bR] Review canonical materialized callable view boundary`。
- 结合提交标题、`TODO.md` / `PLAN.md` 当前记录，可确认：
  - 该 review 未留下“需要先插入到下一条任务之前修复”的新增前置缺陷；
  - 当前顺序上的第一个未完成任务是 `T5000h0c 让 LLVM build / single-file entry 显式接入 materialized body / pass 视图`。

## 当前执行计划（围绕 T5000h0c）

1. 阅读 `T5000h0c` 相关代码边界：
   - `crates/scoopc/src/llvm/frontend.rs`
   - `crates/scoop/src/commands/build.rs`
   - `crates/scoopc/src/hir/lower/types.rs`
   - `crates/scoopc/src/mir/callables.rs`
   - 以及 LLVM entry / emit 主路径中当前仍直接消费 HIR 兼容 body 的位置
2. 判断 production/codegen 目前是否只是“保留了 materialized MIR”，但还没有把它作为显式输入边界接到 LLVM 主路径。
3. 实现显式接线：
   - 让 build / single-file 入口明确携带 canonical materialized callable body / summary / pass 视图；
   - 给后续 MIR rewrite 预留稳定插入点，而不是让消费侧继续隐式回退到 HIR body。
4. 运行针对性测试，再按影响面补充更高层验证。
5. 更新 `TODO.md`、`PLAN.md`、本文件并提交。

## 当前状态（实现与验证完成）

- 已完成目标任务：`T5000h0c 让 LLVM build / single-file entry 显式接入 materialized body / pass 视图`。
- 已完成的核心改动：
  1. 新增 `crates/scoopc/src/mir/pass_view.rs`，抽出 `MaterializedMirPassView`，并通过 `MaterializedMir::pass_view()` / `LoweredHir::materialized_pass_view()` 暴露 canonical production pass 输入面。
  2. `crates/scoopc/src/llvm/emit.rs` 新增 production-only LLVM entry，显式要求 `LoweredHir` 携带 `materialized_pass_view()`；缺失时返回 `MissingMaterializedPassView`，不再静默回退。
  3. build 主路径与 single-file LLVM 路径都已切到新的 production entry。
  4. `CompilationUnitCodegenCx` 已开始显式保留 `materialized_pass_view`，为后续 MIR rewrite / inlining 留出 codegen 接缝。
- 已完成验证：
  - `cargo test -p scoopc llvm::tests -- --nocapture`
  - `cargo test -p scoop build_ -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- test`
  - 结果：全部通过；fixture suite 输出 `fixtures: ok (1201)`。
- 当前剩余动作：
  - [x] 实现代码修改
  - [x] 运行验证
  - [x] 更新 `TODO.md` / `PLAN.md`
  - [ ] 提交 git commit
