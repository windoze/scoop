# 本轮执行计划

说明：这里记录可公开的执行计划、依据和进展摘要，不包含内部保密推理细节。

## 目标

按 `TODO.md` 的顺序处理第一个未完成任务，但在此之前先检查最新提交是否提到了需要先修复的既有问题；如果发现任何已存在的问题、回归、规范不匹配或实现边界不完整，会先修复它们，或将其作为前置任务加入 `TODO.md` 并停止。

## 初始步骤

1. 查看最新一次 Git 提交的提交信息与改动说明，确认是否明确提到待修复的既有问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，理解现有计划、依赖关系与任务背景。
4. 如首个未完成任务过大，则将其拆分为可执行子任务，并同步更新 `PLAN.md` 与 `TODO.md`。

## 执行策略

1. 先做最小必要的代码库勘查，定位与当前任务相关的实现、测试、规范和已有回归。
2. 若勘查或测试中发现任何既有问题，立即转为优先修复，禁止绕过。
3. 对当前任务进行完整实现。
4. 运行相关测试；如有必要，补充或调整测试以覆盖修复/实现内容。
5. 运行质量检查，至少包括：
   - 相关测试
   - `cargo fmt --check`（必要时先 `cargo fmt`）
   - `cargo clippy --all-targets -- -D warnings`
6. 更新文档与跟踪文件：
   - 在 `TODO.md` 标记完成，或在受阻时重排前置任务
   - 更新 `PLAN.md`
   - 持续更新本文件记录关键进展
7. 用清晰的提交信息提交本轮改动，然后停止。

## 风险与约束

- 不允许通过变更表示方式、缩窄测试形状、加入特判或 fixture hack 来绕过缺陷。
- 若任务依赖缺失能力，必须先将缺失能力作为前置任务写入 `TODO.md`，更新 `PLAN.md` 后提交并停止。
- 不回退或覆盖与当前任务无关的现有改动。

## 进展记录

- 已创建本计划文件，并完成初始仓库勘查：
  - 已检查最新提交 `e1fd061 [T4015c] Clarify const fun Pure contract`；
  - 已读取 `TODO.md` / `PLAN.md` / `ISSUES.md`，定位当前首个未完成任务为 `T4015R`。
- 已进入 `T4015R` 复审，并完成静态核对：
  - `eval_const_bindings_in_compilation_unit(...)` 会先跑 compilation-unit resolve/typecheck，再把 `TopLevelFunCallBinding` / `TypeStore` 交给解释器；
  - `call_fun_or_intrinsic(...)` 仍保留“缺少绑定时回退到 `call_const_fun(...)` simple-name + arity”的旧兜底；
  - `trim_package_level_comptime_ifs(source, file)` 正是仍会落到该兜底的 pre-typecheck 路径。
- 已完成定向 probe，确认这是当前真实 blocker：
  - package-level `comptime if (describe(1)) { ... }`，同文件定义 `describe(Int)` / `describe(String)` 时，当前稳定报 `scoop::comptime::const_fun_ambiguous`；
  - package-level `comptime if (truthy<Int>(1)) { ... }` 当前稳定报 `scoop::comptime::unsupported_const_fun_signature`（`explicit type args`）。
- 计划已调整：
  1. 在 `TODO.md` 中前插 `T1220b`，把 package-level `comptime if` 条件绑定问题列为 `T4015R` 的前置任务。
  2. 更新 `PLAN.md` / `ISSUES.md`，记录该 blocker 与新的执行顺序 `T1220b -> T4015R`。
  3. 本轮按“受阻重排”提交这些跟踪文件改动后停止，等待下一轮先实现 `T1220b`。
