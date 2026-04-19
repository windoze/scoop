# 本轮执行计划与决策记录

更新时间：2026-04-20

## 说明

本文件记录本轮执行的目标、依据、步骤、关键发现与变更决策，作为可检查的工作日志使用。
我不会在这里写原始的逐字内部思维，而是记录足够详细的分析摘要、执行计划、判断依据和进度更新。

## 当前目标

按 `TODO.md` 的优先级，仅完成“第一个未完成任务”，并在完成后停止。

## 初始执行计划

1. 检查最新一次 Git 提交，确认提交信息或关联改动中是否提到尚未修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认该任务的背景、依赖、验收标准与已有拆分。
4. 如果该任务过大或存在前置缺口：
   - 将任务拆分为可单次完成的子任务。
   - 更新 `PLAN.md`。
   - 更新 `TODO.md`，把新的前置子任务放到正确顺序。
   - 本轮只执行拆分后的第一个子任务，或在存在阻塞时只提交任务重排与计划更新。
5. 若任务可直接执行：
   - 阅读相关代码与测试。
   - 实现任务，避免任何规避式 workaround。
   - 补充或调整测试。
   - 运行相关验证，至少覆盖该任务影响面，并在可能情况下执行更高强度检查。
6. 更新文档状态：
   - 在 `TODO.md` 中标记本轮完成项。
   - 在 `PLAN.md` 中记录当前状态、已完成内容和后续影响。
   - 在本文件中补充结果与决策摘要。
7. 使用清晰的 Git 提交信息提交本轮所有改动，然后停止。

## 当前状态

- 已建立本轮执行记录文件。
- 已检查最新提交：`d7a166a1b33d86a6040e1b2831b9399bde54a765`，提交信息为 `Update plan`，未在提交说明中直接声明新的待修复 issue；本轮仍按 `TODO.md` 顺序推进。
- 已读取 `TODO.md` 与 `PLAN.md`，当前首个未完成任务为 `T4009a1`：统一 `async {}` / `async fun` 为 `Task<T>` sugar。

## 当前任务判断

- 任务编号：`T4009a1`
- 任务标题：统一 `async {}` / `async fun` 为 `Task<T>` sugar
- 关键验收：
  - `async {}` / `async fun` 对调用者都稳定暴露 `Task<T>`。
  - typecheck / HIR / lowering / 文档对 async surface 的叙事一致。
  - 不再保留“吞掉内部 `Async` effect 并直接返回 `T`”的旁路表述。

## 针对当前任务的细化计划

1. 搜索 `async {}`、`async fun`、`Task`、`Executor`、`spawn`、`join` 在 parser / typecheck / HIR / LLVM / sysroot / stdlib / runtime / 规范文档中的现状。
2. 找出现有 async surface 是否已经在某些路径上暴露 `Task<T>`、但在其它路径上仍直接暴露 `T` 或依赖 executor ABI。
3. 若发现当前任务被更前置的实现缺口阻塞：
   - 先精确定义 blocker；
   - 更新 `TODO.md` / `PLAN.md`；
   - 提交任务重排并停止。
4. 若可直接实现：
   - 收口 parser/typecheck/HIR/lowering 与 sysroot surface，使 `async {}` / `async fun` 统一成为 `Task<T>` sugar。
   - 补充或修正相关测试与规范文档。
   - 运行定向测试、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
   - 更新 `TODO.md` / `PLAN.md` / 本文件，提交后停止。

## 决策原则

- 若发现规范与实现不一致，必须先把该缺口转化为 `TODO.md` 中更靠前的显式任务，不能绕过。
- 若当前首个未完成任务无法在本轮完整、正确地完成，则优先进行任务拆分或依赖重排，并提交这些计划性改动后停止。
- 不回退或覆盖不属于本轮任务的用户现有修改。
# 2026-04-20 接续执行记录（本轮）

## 当前判断

- 当前首个未完成任务仍是 `T4009a1`，上一轮已经完成绝大部分实现，但还没有完成收尾验证、任务状态回写和提交。
- 已知存在一个待修正的 typecheck fixture 问题：部分测试仍按旧语义把 `Task<T>` 直接传给 `println`，这会因为 `ToString` 约束失败而报错。
- 目前没有发现必须先插入的新 blocker 任务；优先修正现有 fixture，并重新执行完整验证链。

## 本轮执行计划

1. 先核对 `TODO.md`、`PLAN.md`、最新提交信息，确认当前仍应继续 `T4009a1`，且没有新增前置问题。
2. 修正剩余使用旧 async 结果语义的 typecheck fixture，使其改为仅做类型断言，不再把 `Task<T>` 传入 `println`。
3. 运行与 `T4009a1` 直接相关的测试，至少包括：
   - `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck`
   - 必要的 run-pass fixture 或定向测试
4. 若定向测试通过，再跑更高置信度验证：
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 如有必要，`cargo run -q -p scoop -- test`
5. 若验证中出现真实实现缺口或规格不匹配：
   - 先修复；
   - 若本轮无法直接完成，则按要求更新 `TODO.md` / `PLAN.md` 以反映依赖与阻塞关系，但不把当前任务标成完成。
6. 如果验证全部通过：
   - 更新 `TODO.md`，将 `T4009a1` 标记完成；
   - 更新 `PLAN.md` 记录完成情况与后续任务状态；
   - 再次更新本文件记录结果；
   - 提交本轮改动并停止，不继续下一个任务。

## 当前进展

- 已复查最新提交、`TODO.md`、`PLAN.md` 与工作区状态，确认当前仍应继续 `T4009a1`，没有新增必须前插的 blocker。
- 已修正两个残留 typecheck fixture 的旧语义断言：
  - `tests/fixtures/typecheck/async_fun_returns_task_ok.scoop`
  - `tests/fixtures/typecheck/entry_point_main_spawn_join_async_ok.scoop`
- 修正内容为：不再把 `Task<T>` 直接传给 `println`，改为仅通过局部变量声明断言 `async fun` / `async {}` 对外类型稳定为 `Task<T>`。
- `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck` 已通过（`fixtures: ok (335)`）。
- `cargo test --all` 已通过。
- `cargo clippy --all-targets -- -D warnings` 已通过。
- `cargo run -q -p scoop -- test` 已通过（`fixtures: ok (1073)`）。

## 收尾结果

- `T4009a1` 现在可以判定为完成：`async {}` / `async fun` 对调用者都稳定暴露 `Task<T>`，并共享同一条 lazy `taskCreate` lowering 主线。
- 期间没有发现新的前置 blocker；本轮不需要再拆任务。
- `TODO.md` 与 `PLAN.md` 已更新：`T4009a1` 已标记完成，下一项已切换到 `T4009a2`。
- `cargo fmt --all --check` 也已通过。
- 接下来只剩两步：
  - 检查工作区 diff。
  - 提交本轮改动并停止。

## 约束与注意事项

- 不接受 workaround；若暴露出真正的语言/运行时缺口，必须转成明确任务后再停下。
- 编辑继续使用 `apply_patch`。
- 本轮只完成一个任务并停止。
