# 本轮执行计划

## 目标

按 `TODO.md` 的顺序处理第一个未完成任务；但在此之前，先检查最新一次提交是否提到任何既有问题，并优先修复这些问题。若当前首个未完成任务过大，则先把它拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，然后只执行拆分后的第一个子任务。

## 约束与原则

- 全程使用中文记录与沟通。
- 任何代码或行为都必须符合规范，不接受临时绕过、fixture-only hack、兼容性垫片式完成。
- 若发现规范缺口、实现边界、语言特性缺失或已有 bug 阻塞当前任务，必须先在 `TODO.md` / `PLAN.md` 中显式建模该依赖，再提交并停止。
- 本轮最多完成一个任务（或拆分后的第一个子任务），完成后立即停止。
- 在执行过程中，如计划发生变化或关键步骤完成，需要继续更新本文件。

## 执行步骤

1. 查看最新一次 git 提交，确认是否提到待修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解当前计划与任务上下文。
4. 如首个未完成任务过大或存在前置依赖，先在 `PLAN.md` / `TODO.md` 中完成拆分或重排。
5. 阅读与该任务直接相关的代码、测试、规范或文档，确认当前实现状态。
6. 实现任务所需代码修改。
7. 运行相关测试，并补充必要测试；随后运行至少覆盖质量门禁所需的检查，优先包括：
   - `cargo fmt --check`（必要时先 `cargo fmt`）
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 与当前任务直接相关的更小范围命令（若能更快暴露问题）
8. 若测试或检查失败，先修复失败项，再重新验证。
9. 更新文档与跟踪文件：
   - 在 `TODO.md` 中标记任务完成，或在阻塞场景下重排任务并保留 `[TODO]`
   - 在 `PLAN.md` 中记录当前状态、完成情况、依赖调整或阻塞原因
   - 按需补充 `README.md`、代码注释或相关文档
   - 更新本文件，记录关键决策和执行结果
10. 查看工作区变更，确保只包含本轮应提交内容。
11. 使用清晰的提交信息创建 git commit。
12. 停止，不继续下一个任务。

## 当前已知未知项

- 最新提交是否声明了尚未修复的问题。
- `TODO.md` 中首个未完成任务的内容、规模与依赖。
- 当前代码库是否已有未提交改动，需要在不回退用户改动的前提下协作处理。

## 变更记录

- 初始化本轮计划，待读取提交记录与任务列表后补充更具体内容。
- 已检查最新提交 `020649b [T4008c0] Fix mixed replay pending continuation publication`：提交信息本身未声明新的待修复遗留项，因此继续按 `TODO.md` 顺序推进。
- 已定位首个未完成任务为 `T4008c1`：收口多 effect type params 的 effect op call / handle arm 实例化。
- 针对 `T4008c1` 的细化执行步骤：
  1. 阅读 `TODO.md` / `PLAN.md` / `ISSUES.md` 中与 `T4008c1`、effect、continuation、handler instantiation 相关的条目，提炼验收标准与已知缺口。
  2. 搜索 effect op call、handle arm、effect type params、continuation instantiation 相关实现位置，确认 parser / typecheck / lowering / LLVM 当前边界。
  3. 先构造或运行最小复现，确认当前失败形态，判断任务是否需要进一步拆分。
  4. 若任务规模可控，则直接实现：
     - effect op call 对多 effect type params 的实参/类型参数绑定；
     - handler arm 对 effect 泛型实例化后的参数、返回值与 continuation 类型对齐；
     - 必要的 HIR / LLVM / runtime 侧配套调整。
  5. 增加或更新最小但充分的回归测试，至少覆盖 typecheck、run-pass，以及必要时的 lowering / runtime 路径。
  6. 运行定向测试后，再跑全量质量门禁：`cargo test --all`、`cargo clippy --all-targets -- -D warnings`，必要时补 `cargo fmt`。
  7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或阻塞依赖。
  8. 提交本轮修改并停止。
- 复现结论：
  - `effect Pair<A, B> { fun left(value: A): Int }` 一类程序在调用点会报 `effect op call（multiple effect type params）`。
  - 不含 performed effect 的 `handle` 最小 probe 会在 arm head 报 `handle arm（multiple effect type params）`。
  - `handle` arm head 的显式 `Pair<String, Int>.op(...)` 语法当前 parser 仍不可达，因此本任务回归采用“单 payload tuple + binder 注解 / body-performed 唯一候选推断”这两条现有主线验证完整 effect instance。
- 已完成实现：
  - `crates/scoopc/src/typecheck/expr/call.rs` 已移除多 effect type params early gate，并把全部 effect type params 追加到 effect op 可实例化签名中。
  - `crates/scoopc/src/typecheck/expr/infer.rs` 已对 handler arm 做同样调整；handled effect 可通过 binder 注解或 body 内唯一 performed effect 反推出完整实例。
  - 已新增回归：
    - `tests/fixtures/typecheck/effect_multi_type_params_tuple_payload_ok.scoop`
    - `tests/fixtures/run-pass/effect_multi_type_params_dispatch_basic.scoop`
    - `tests/fixtures/run-pass/effect_multi_type_params_dispatch_basic.stdout`
- 已完成验证：
  - `cargo fmt --check`
  - `cargo run -q -p scoop -- run tests/fixtures/run-pass/effect_multi_type_params_dispatch_basic.scoop`
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (332)`）
  - `cargo run -q -p scoop -- test`（最终单独重跑为 `fixtures: ok (1060)`；首次并行运行时因 `target/debug/scoop` 被其它 cargo 进程重建而失效，已排除）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 当前状态：
  - `T4008c1` 已完成，待同步提交 `TODO.md` / `PLAN.md` / 本文件后创建 commit。
