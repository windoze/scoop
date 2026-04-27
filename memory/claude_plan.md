# 执行计划

## 当前目标

- 按照用户要求，本轮只处理 `TODO.md` 中第一个未完成任务。
- 在开始任何仓库检查和代码执行前，先记录本文件，后续如计划变化或关键步骤完成，会继续更新本文件。
- 输出和进度记录使用中文。

## 约束与优先级

- 先检查最新提交是否提到已有问题；如果存在已有问题，必须先修复或把必要前置任务加入 `TODO.md` 后停止。
- 再读取 `TODO.md`，定位第一个未完成任务。
- 如第一个任务过大，先拆分为可执行子任务，更新 `PLAN.md` 和 `TODO.md`，提交后停止或执行当前拆分出的第一个子任务，取决于拆分后的任务是否已足够明确。
- 遇到任何规格不符、已知缺陷、测试暴露的问题或实现边界，不能绕过；必须修复，或新增并排序前置任务后提交停止。
- 本轮最多完成一个任务，完成后必须更新文档、运行相关测试并提交 Git。
- 不回退用户已有改动，不使用破坏性 Git 命令。

## 步骤计划

1. 查看工作区状态，确认是否已有未提交改动需要避让。
2. 查看最新提交信息和补丁，判断是否提到或留下需要优先处理的已有问题。
3. 读取 `TODO.md` 和 `PLAN.md`，确认第一个未完成任务及其上下文。
4. 如果发现最新提交中的已有问题，先处理该问题；否则执行第一个未完成任务。
5. 依据任务范围读取相关源码、测试和规范文档，定位最小正确实现边界。
6. 如任务可直接实现，修改源码和必要测试；如任务被缺失特性阻塞，更新 `TODO.md` 与 `PLAN.md` 记录前置任务并停止。
7. 运行相关测试；根据改动风险扩大到必要的 `cargo test` / fixture 测试 / clippy 检查。
8. 更新 `TODO.md` 标记完成，更新 `PLAN.md` 记录状态变化。
9. 检查 `git diff`，确认只包含本轮必要改动。
10. 使用清晰任务标签提交。
11. 停止，不处理下一个任务。

## 当前状态

- 已写入初始计划。
- 已检查工作区状态：当前只有本轮 `memory/claude_plan.md` 改动。
- 已检查最新提交：`[T5000h0e2] Lower pass-rewritten MIR bodies in production LLVM`，提交信息未明确提到需要优先修复的既有问题。
- 已读取 `TODO.md` 并定位第一个未完成任务：
  - `T5000h0eR Review：确认 production codegen 已真正切到 pass-rewritten callable body / summary 输入面`。
- 本轮任务性质是 review。执行重点：
  1. 复核 `MaterializedMirPassView` / pass artifacts 是否能区分 raw materialized body 与 pass-overridden body / summary。
  2. 复核 production LLVM reachability、body emission、effect/suspend summary 查询是否都优先观察 pass view。
  3. 复核 `T5000h0e2` 新增 MIR body lowering 是否只对显式 pass-overridden callable 生效，且 unsupported MIR 节点不会静默回退到 HIR body。
  4. 运行定向测试与必要的全量验证。
  5. 若发现真实问题，先修复或新增前置任务；若未发现阻塞问题，更新 `TODO.md` / `PLAN.md` 标记 review 完成并提交。
- Review 过程中已复核：
  - `emit.rs` 会把 `materialized_pass_view` 传入 reachability，并在 reachable body emission 时对显式 overridden callable 调用 `codegen_top_level_mir_fun(...)`；
  - `reachability.rs` 对 pass-visible callable 会扫描 canonical MIR body；
  - `effect_state_machine_analysis.rs` 只消费 pass 显式 override 的 summary，避免 raw summary 抢占 HIR/effect 分析；
  - 代表性虚调用 / 接口调用 fixture 的 production build 和运行通过。
- 已发现并修复一个 review 暴露的边界不一致：
  - `codegen_top_level_mir_fun(...)` 原先在调用 `build_fun_callee_suspend_plan(...)` 后才切换 `current_source_id`；
  - HIR lowering 是先切到当前函数源文件再做 suspend-plan 分析；
  - 已将 MIR lowering 的 `source_id_for_path(...)` 提前到 suspend-plan 检查之前，避免跨文件 pass-overridden callable 的 effect/suspend 分析使用入口源文件上下文。
- 已完成第一轮验证：
  - `cargo fmt --all`：通过；
  - `cargo test -p scoopc production_codegen_ -- --nocapture`：4 个 production pass-view/codegen 回归通过；
  - `cargo test -p scoopc mir::pass_view -- --nocapture`：3 个 pass-view 回归通过；
  - `cargo run -p scoop -- build tests/fixtures/run-pass/member_call_virtual_dispatch_override_basic.scoop -o /tmp/scoop_virtual_test`：通过，运行输出 `Derived.ping / after / 2`；
  - `cargo run -p scoop -- build tests/fixtures/run-pass/member_call_interface_dispatch_basic.scoop -o /tmp/scoop_interface_test`：通过，运行输出 `Impl.foo / after / 42`。
- 已完成全量验证：
  - `cargo test -p scoopc --no-default-features`：通过；
  - `cargo test --all`：通过；
  - `cargo run -p scoop -- test`：通过，`fixtures: ok (1201)`；
  - `cargo clippy --all-targets -- -D warnings`：通过。
- 已更新 `TODO.md` 与 `PLAN.md` 标记 `T5000h0eR` 完成，并完成最终 diff 检查。
- 下一步：提交本轮 review 修复与任务记录，然后停止。
