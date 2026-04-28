# Claude Plan

## 约束说明

- 本文件记录可共享的执行计划、检查点、关键发现和进度更新。
- 不记录隐藏推理过程；后续如果计划变化或关键步骤完成，会继续更新本文件。
- 本轮只处理 `TODO.md` 中第一个未完成任务；完成后提交 Git commit 并停止。

## 初始执行计划

1. 检查最新 Git commit，确认提交信息或变更中是否提到已有问题、回归、规格不匹配或临时处理。
2. 如最新 commit 暴露已有问题，优先修复该问题；否则继续读取 `TODO.md`。
3. 读取 `TODO.md`，定位第一个未完成任务。
4. 评估该任务复杂度：
   - 若可直接完成，则实现该任务。
   - 若任务过大，则将其拆分为更小子任务，更新 `PLAN.md` 和 `TODO.md`，提交后停止。
5. 实现第一个任务或子任务，严格避免 workaround、fixture-only hack 或规格偏离。
6. 运行相关测试，并根据改动风险扩大到必要的测试范围；若发现已有问题或规格缺口，按要求优先修复或插入前置任务。
7. 更新 `TODO.md` 将本轮任务标记为完成，并更新 `PLAN.md` 记录进度。
8. 运行格式化/检查；目标是无编译和 lint 警告。
9. 提交所有本轮相关变更，提交信息使用清晰任务标签。
10. 停止，不继续处理下一个任务。

## 当前状态

- 已写入初始计划。
- 已检查最新 commit：`36c43f5b [T5000i1P4] Filter materializer roots by reachable MIR blocks`。
- commit message 本身没有额外问题说明；最新提交改动的 `ISSUES.md` 记录了 P2：production LLVM body emission 仍默认走 HIR 兼容 body，只有 pass override 时走 MIR body。
- 已读取 `TODO.md`，第一条未完成任务为 `T5000i1P5 让 production LLVM body emission 默认消费 materialized MIR body`，与最新 issue 记录一致，作为本轮执行目标。

## T5000i1P5 执行计划

1. 阅读 `TODO.md` 中 `T5000i1P5` 的范围/验收，以及 `PLAN.md` 中对应进度说明。
2. 阅读 LLVM production emit/body emission 相关代码，重点检查：
   - `crates/scoopc/src/llvm/emit.rs`
   - `crates/scoopc/src/llvm/codegen/mir_body.rs`
   - `MaterializedMirPassView` / callable body 查询接口
   - 现有 production LLVM 回归测试
3. 明确当前 `pass_view.callable_body_is_overridden(...)` gate 的行为，并改为 production 默认优先消费 materialized/pass-visible MIR body；仅在缺失 MIR body 或暂未支持的结构化边界下使用明确诊断，避免静默 HIR workaround。
4. 补充或调整回归测试，覆盖“未 pass override 的 materialized MIR body 也被 production LLVM body emission 消费”。
5. 运行聚焦测试；根据风险继续运行 `cargo fmt --all`、相关 crate 测试、必要 fixture，以及 `cargo clippy --all-targets -- -D warnings`。
6. 更新 `TODO.md` 标记 `T5000i1P5` 完成，并更新 `PLAN.md` / `ISSUES.md` 对 P2 的状态记录。
7. 提交本轮变更并停止。

## T5000i1P5 进度更新

- 已定位 production body emission 的旧 gate：`crates/scoopc/src/llvm/emit.rs` 只有 `callable_body_is_overridden(...)` 为真时才走 `codegen_top_level_mir_fun(...)`。
- 已完成第一版切换：对于 pass view 中存在 canonical callable body 的 materialized instance，production body emission 默认走 MIR bridge；pass view 明确移除 body 时继续不发射；没有 pass-visible body 的非泛型边界仍走 HIR 兼容路径。
- 已新增回归 `production_codegen_lowers_raw_materialized_mir_body_without_pass_override`，覆盖 O0 下未被 pass override 的 raw materialized `wrap::<Int>` 也通过 MIR bridge 发射。
- 扩大到 `production_codegen` 测试时暴露 effect/state-machine raw MIR body 的已知 bridge 边界；已收口为：未被 pass override 的 raw materialized effect/state-machine body 继续走现有 HIR effect lowering，显式 pass override 仍不静默回退。
- 已验证：
  - `cargo fmt --all --check`
  - `cargo test -p scoopc production_codegen -- --nocapture`
  - `cargo test -p scoop build_frontend_ -- --nocapture`
- 更大范围验证中发现 raw materialized body 还会暴露函数值 `TopLevelRef`、async/task MIR rvalue 等当前 bridge 不支持的形状；这些属于未被 pass override 的 raw body 兼容边界，已新增结构预检，确保只有 bridge 已支持形状默认走 MIR，unsupported raw body 保守走 HIR，显式 pass override 仍严格走 MIR。
- 已更新 `TODO.md` 标记 `T5000i1P5` 完成，并更新 `PLAN.md` / `ISSUES.md`。
- 最终已验证：
  - `cargo fmt --all --check`
  - `cargo test -p scoopc production_codegen -- --nocapture`
  - `cargo test -p scoopc llvm::tests -- --nocapture`
  - `cargo test -p scoopc mir::materialize -- --nocapture`
  - `cargo test -p scoop build_frontend_ -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 下一步：检查 git diff/status，然后提交本轮变更。
