# 执行计划

## 当前目标
- 先读取 `TODO.md`，确定第一个未完成且标题未带 `[DONE]` 的任务。
- 仅处理这一个任务；若遇到阻塞当前任务的真实前置问题，则先把该前置问题按最小必要粒度写入 `TODO.md`，更新依赖顺序后停止。

## 执行步骤
1. 读取 `TODO.md`，识别当前应执行的首个未完成任务。
2. 检查最近提交是否直接说明了与该任务相关但尚未完成的问题；若有，把它视为当前任务的一部分或写成新的前置任务。
3. 阅读与该任务直接相关的代码、测试、文档与规范，确认实现边界、依赖关系和验收条件。
4. 以最小正确改动实现任务；如果发现当前任务被真实缺陷或缺失特性阻塞，不做绕过方案，而是把该问题写入 `TODO.md` 作为前置任务，并在必要时更新 `PLAN.md`。
5. 运行该任务要求的验证命令，以及必要的回归测试、格式化、lint、构建检查，直到结果稳定。
6. 更新 `TODO.md`：若任务完成，则给任务标题加上 `[DONE]` 并填写/更新完成记录；若阻塞，则保持该任务未完成并写清新增前置任务与依赖。
7. 仅在阶段计划、依赖结构或完成标准发生变化时更新 `PLAN.md`。
8. 检查工作区改动，确保不回滚他人改动；按要求创建一次 git 提交，提交信息使用当前任务号。
9. 停止，不继续处理下一个任务。

## 约束
- 不跳过 review 型任务。
- 不因为任务偏大而默认拆分。
- 不用 workaround、fixture hack、缩小范围或改模型来规避缺陷。
- 只在确有必要时新增最小数量的前置任务。
- 若本次是在继续一个未提交的同一任务收尾，则提交时包含当前所有未提交文件。

## 进度记录
- 已读取 `TODO.md`，当前首个未完成任务为 `P3-T01：扩展 @Extern 语法与 HIR，正式支持 abi = "scoop"`。
- 已检查最新提交摘要：`[P2-T02] Unify native surface gate diagnostics`，未发现提交信息中直接声明的、会前置阻塞 `P3-T01` 的未完成事项。
- 已完成首轮实现：
  - `ExternAbi` 已扩展为 `C` / `Scoop`，并加入共享 ABI 名解析；
  - `@Extern(..., abi = "scoop")` 已能在 HIR side table 产出 `ExternAbi::Scoop`；
  - typecheck 已补：无效 ABI、重复 `abi`、`abi` 与异常参数形状、`@CallingConvention` 组合、以及 `abi = "scoop"` 的 v1 前端限制（顶层/无 receiver、无泛型、无 function value / continuation surface）。
- 已新增回归：
  - parser fixture：`tests/fixtures/parse/extern_fun_scoop_abi_basic.*`
  - typecheck fixtures：`tests/fixtures/typecheck/extern_fun_scoop_abi_*`
  - HIR 单测：`refactor_hir_collects_scoop_extern_abi_metadata`
- 已完成定向验证：
  - `cargo test -p scoopc refactor_hir_collects_scoop_extern_abi_metadata -- --nocapture`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/parse/extern_fun_scoop_abi_basic.scoop`
  - 全部新增 `extern_fun_scoop_abi_*` typecheck fixtures
- 已完成最终验证：
  - `cargo fmt --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已回写 `TODO.md`：`P3-T01` 已标记为 `[DONE]` 并补全 completion record。
- 下一步：检查工作区、创建 `[P3-T01]` 提交，然后停止。
