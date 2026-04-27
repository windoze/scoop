# 执行计划

## 当前目标

按 `TODO.md` 的顺序完成第一个未完成任务；在此之前，先检查最新提交是否提到既有问题，如有则优先修复。整个过程中把关键决策、发现的问题、计划调整和完成状态持续记录在本文件。

## 初始步骤

1. 查看最新一次 Git 提交信息，确认是否明确提到待修复的既有问题。
2. 阅读 `TODO.md` 与 `PLAN.md`，识别第一个未完成任务及其上下文。
3. 评估该任务是否过大：
   - 如果可直接完成，则继续实现。
   - 如果过大，则先拆分任务，更新 `TODO.md` 与 `PLAN.md`，本次只执行拆出的第一个子任务。
4. 在实现前检查相关代码、测试、规格和最近改动，确认是否存在会阻塞任务的既有缺陷。
5. 实现任务并补充/调整测试。
6. 运行与任务相关的验证：
   - 至少运行定向测试；
   - 若改动影响较广，再运行更大范围测试；
   - 结束前确保 `cargo clippy --all-targets -- -D warnings` 无告警（若范围允许）。
7. 更新 `TODO.md`、`PLAN.md` 和本文件，记录结果与后续状态。
8. 提交 Git commit，然后停止，不继续处理下一个任务。

## 执行原则

- 不用规避方案掩盖真实缺陷；若发现规格不匹配、实现边界缺失或回归，优先修复，或将其作为前置任务插入 `TODO.md` 后停止。
- 不回退用户已有改动；若工作树中存在无关变更，仅在必要范围内协同处理。
- 每完成一个关键节点后更新本文件，包括：
  - 已检查内容；
  - 发现的问题；
  - 当前正在执行的步骤；
  - 是否调整计划。

## 当前进展

- 最新提交检查结果：
  - `git show -1 --format=fuller --no-patch` 显示最新提交为 `9250a0f0 [T5000fR] Fix overloaded summary instance identity`；
  - 提交信息只有标题，没有正文，没有额外标注“后续必须立即修复”的既有问题。
- 第一个未完成任务：
  - `TODO.md` 中首个 `[TODO]` 条目是 `T5000g 在 MIR 层实现通用 devirtualization`；
  - 其要求是在 MIR 层统一处理 `VirtualCall` / `InterfaceCall` 的 target-set shrinking，并在 target singleton 时改写为 `DirectCall`，禁止依赖热点函数名特判。
- 当前工作树状态：
  - 存在大量未提交改动，集中在 `crates/scoopc/src/{hir/lower/*,mir/{lower,materialize}.rs,llvm/*}`、新文件 `crates/scoopc/src/devirtualize.rs`，以及若干 MIR fixtures；
  - 从 diff 观察，这批改动显然是在推进 `T5000g`：新增 dispatch-call side table、known receiver subclass / vtable / itable 事实输入、以及在 MIR materialization 阶段把部分 dispatch 调用改写为 `DirectCall`；
  - 因为这些改动与当前任务直接相关，先不视为无关噪音，而是按“当前任务的未完成状态”继续审查、补完和验证。

## 接下来

1. 详细审读 `devirtualize.rs` 与相关 lowering/materialization 变更，确认设计是否符合 `T5000g` 的边界要求。
2. 运行定向测试或编译检查，找出当前未提交改动中的真实失败点。
3. 若发现既有阻塞缺陷，先修复；若当前 devirtualization 实现不完整，则继续补齐直到 `T5000g` 可验收。
4. 完成后更新 `TODO.md`、`PLAN.md`、本文件，并提交一次 commit。

## 已完成的关键修复

- 编译断点已补齐：
  - `MirLoweringFacts` 增加了可直接从 HIR side tables + resume spans 构造的入口；
  - `pre_specialize` 现在通过新的 `lower_fun_with_type_bindings_and_mir_facts(...)` 保留单函数 lowering 产生的 dispatch/effect/when side tables，再传给 MIR lowering；
  - closure MIR lowering 已补上传递 `source_path`；
  - materializer 相关测试构造点已补齐新的 devirtualization 事实输入。
- devirtualization 主链修正：
  - `collect_hir_direct_call_instance_requests(...)` 不再要求“HIR 里已经 materialize 的 direct-call FQN”与 `TopLevelFunCallBinding.fqn` 完全相等，而是优先按 call-site binding 回查 template；
  - `MirInstanceMaterializer::site_instance_binding_for_callee(...)` 现在会把 `foo::<...>` 视为同一 template 的已物化 callee，直接复用 call-site binding，而不是误判为需要 remap 的另一个 template；
  - `try_devirtualize_dispatch_target(...)` 对 exact receiver 的 class dispatch 增加了非-vtable fallback：当 receiver exact type 与 owner 一致、且没有更具体 target set 时，可直接收缩到 `owner.member`，从而覆盖没有 vtable slot 的非 override/final class member。
- 回归覆盖已补充：
  - exact virtual receiver 会在 monomorphized MIR 中改写为 `DirectCall`；
  - 当 receiver 类型存在已知子类时，`VirtualCall` 会保留；
  - `where T: Interface` 在实例化到 concrete receiver 后，interface dispatch 会在 MIR 中改写为 `DirectCall`。

## 当前状态

- `cargo fmt --all`：已通过。
- `cargo test -p scoopc`：已通过（412 tests）。
- `cargo test --all`：已通过。
- `cargo run -p scoop -- test`：已通过（`fixtures: ok (1201)`）。
- `cargo clippy --all-targets -- -D warnings`：已通过。
- 文档状态：
  - `TODO.md` 已把 `T5000g` 标记为 `[DONE]`，并补充完成记录；
  - `PLAN.md` 已补记 `T5000g` 的实现结果、回归覆盖与验证命令；
  - 下一条待执行任务已切换为 `T5000gR Review：确认 devirtualization 已经是结构驱动而不是热点特判`。

## 剩余收尾

1. 检查最终 diff 与工作树状态，确认本次提交内容。
2. 以 `T5000g` 主题提交一次 commit。
3. 停止，不继续处理 `T5000gR`。
