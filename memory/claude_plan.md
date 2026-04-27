# 执行计划

## 当前约束

- 输出使用中文。
- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后停止。
- 在推进任务前，先检查最新提交是否提到既有问题；若有，优先修复。
- 任何执行中发现的既有 bug、回归、规格不匹配、未完成边界或 workaround 都立即纳入范围。
- 不通过削弱规格、调整测试形状或引入临时绕路来完成任务。
- 完成后需要更新 `TODO.md`、`PLAN.md`，运行相关测试并提交 Git commit。

## 步骤计划

1. 查看最新提交信息，确认是否提到需要优先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md` 以及与该任务相关的源码、测试和规格说明，确认正确实现边界。
4. 如果任务过大或被前置问题阻塞，则更新 `TODO.md` 和 `PLAN.md`，提交这些计划调整后停止。
5. 若任务可直接完成，则按现有项目结构实现最小但完整的修复或功能。
6. 添加或更新最小相关测试，避免 fixture-only hack 或任务私有特殊分支。
7. 运行相关测试；若改动影响面较大，再运行更宽范围测试。
8. 处理测试、编译或 lint 中暴露的真实问题；若发现新的前置规格问题，转为 TODO 前置任务并停止。
9. 将本任务在 `TODO.md` 标记完成，并更新 `PLAN.md` 的当前状态。
10. 复查 `git diff`，确认没有无关回退或用户改动被覆盖。
11. 使用清晰任务标签提交本轮改动。

## 进度记录

- 已创建本执行计划，下一步检查最新提交和任务列表。
- 已确认最新提交 `[T5000h1] Implement summary-driven MIR direct-call inlining` 未在提交信息中声明需要优先修复的既有问题。
- 已定位第一个未完成任务为 `T5000h2`。
- 当前实现计划：
  1. 在 materialized MIR 中保留 request-root 可达的 non-generic caller MIR body，作为 pass 的私有候选输入，但不默认写入 pass view。
  2. 扩展 summary-driven inlining，使其可以改写这些 caller 候选；只有实际改写且 body 仍属于当前 production MIR lowering 支持子集时，才写入 pass artifacts。
  3. 让 LLVM reachability / body emission 识别没有 instance owner 的显式 pass body override，避免 caller rewrite 后仍扫描旧 HIR body。
  4. 增加 MIR pass 与 production LLVM 回归，分别验证 non-generic caller rewrite、生效后的 LLVM body，以及未改写 non-generic body 不进入 pass view。
- 已完成初版代码修改：
  - `MaterializedMir` 现在保留 caller-side pass 候选 body；
  - inlining pass 会改写 request-root 可达的 non-generic caller，但只在实际变化且结构属于可发布子集时写入 pass artifacts；
  - LLVM reachability / emission 已识别无 instance owner 的显式 body override；
  - 已添加 MIR 与 LLVM 回归测试。
- 针对测试暴露的问题已调整实现：non-generic caller 候选在记录前会复用 materializer 的 site binding / instance FQN 重写逻辑，避免 raw caller body 仍指向模板 FQN 而无法匹配 pass-visible callee instance。
- 当前针对性验证已通过：
  - `cargo test -p scoopc mir::inline -- --nocapture`
  - `cargo test -p scoopc production_codegen_observes_caller_side_mir_inlining_for_non_generic_body -- --nocapture`
  - `cargo test -p scoopc production_reachability_scans_overridden_non_generic_pass_body -- --nocapture`
  - `cargo test -p scoopc mir::pass_view -- --nocapture`
- `cargo test -p scoopc llvm::tests -- --nocapture` 曾暴露 entry `main` 边界：entry main 仍由 HIR 专用路径降低，不能发布 pass MIR override。已将 `main` 排除在当前 caller-side 可发布 body 子集之外，并确认此前失败的两个 LLVM 回归已单独通过。
- 已重新验证：
  - `cargo test -p scoopc llvm::tests -- --nocapture`
  - `cargo test -p scoopc mir:: -- --nocapture`
- 收尾验证已通过：
  - `cargo test -p scoopc --no-default-features`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- test`（`fixtures: ok (1201)`）
- 已更新 `TODO.md` / `PLAN.md`，将 `T5000h2` 标记完成；下一步复查 diff 并提交。
