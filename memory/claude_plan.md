# 执行计划

## 当前目标

本轮只完成 `TODO.md` 中第一项未完成任务；如果发现前置缺陷、规范不匹配或任务过大，需要先调整 `TODO.md` / `PLAN.md`，提交后停止。

## 已知约束

- 先检查最新一次 Git 提交，确认是否提到需要先修复的既有问题；若有，优先修复。
- 只能处理一项任务（或拆分后的第一项子任务），完成后必须测试、更新文档并提交 Git。
- 不能通过变通方案绕过规范缺口；如果存在缺失能力或错误实现，必须先把修复任务写入 `TODO.md` 并调整依赖顺序。
- 过程中需要持续更新本文件，记录关键判断、计划调整和完成状态。

## 执行步骤

1. 检查最新提交内容，确认是否存在提交中明确提到的遗留问题需要先修复。
2. 阅读 `TODO.md`，定位第一项未完成任务。
3. 评估该任务是否可在本轮完整落地：
   - 若可完成，直接实现。
   - 若过大或被前置问题阻塞，拆分任务并更新 `TODO.md` / `PLAN.md`，然后执行新的第一项子任务；若仍因前置缺陷无法继续，则只记录依赖调整并提交。
4. 实现任务时同步检查相关规范、现有实现与测试覆盖，避免引入临时性兼容方案。
5. 运行相关验证：
   - 至少执行与改动直接相关的测试。
   - 若改动影响通用编译/静态检查，补跑 `cargo test --all`、`cargo clippy --all-targets -- -D warnings` 或更精确但充分的等价检查。
6. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成情况或阻塞原因。
7. 使用清晰的提交信息创建 Git 提交，然后停止，不继续处理下一项任务。

## 进度记录

- 已创建初始执行计划，下一步检查最新提交与任务列表。
- 已检查最新提交 `93a52648d9fea6de59d762a922702a8ed869a26c`，提交说明未显式提到需要先修复的遗留问题；未发现必须先于 `TODO.md` 执行的提交内问题。
- 已定位 `TODO.md` 第一项未完成任务为 `T2003r3b3`：让 unified emitter 接管 `MultiNonResuming` 并退化旧 non-resuming specialized 主入口。
- 已阅读 `TODO.md` / `PLAN.md` 与当前 LLVM effect lowering 代码：
  - `NoSuspendSites` 与 `SingleNonResuming` 已通过 `UnifiedNoContinuationEntrypoint` 进入统一主线；
  - `MultiNonResuming` 仍从 `codegen_handle_expr_multi_arm(...)` 进入旧的 `codegen_handle_expr_nonresuming_multi_arm(...)` specialized 主路径；
  - 现有结构允许沿用 `T2003r3b1/b2` 的模式，把 `MultiNonResuming` 纳入 unified no-continuation 主入口，同时把旧 multi non-resuming lowering 收口为 leaf helper。
- 当前执行方案：
  1. 扩展 unified no-continuation 入口与 plan contract 校验，纳入 `MultiNonResuming`。
  2. 调整 `codegen_handle_expr` 选路，让 pure multi non-resuming handle 优先走 unified emitter 主线。
  3. 将旧 multi non-resuming lowering 明确退化为 leaf helper，避免继续作为主路由入口。
  4. 新增定向单测与 run-pass fixture，覆盖 multi non-resuming + `finally` + nested handle representative sample。
  5. 跑相关测试与 lint，更新 `TODO.md` / `PLAN.md` / 本文件后提交。
- 代码实现已完成：
  - `UnifiedNoContinuationEntrypoint` 已新增 `MultiNonResuming`；
  - unified 入口已新增 multi non-resuming plan contract 校验，并在 root handle 上优先接管 pure multi non-resuming 主路由；
  - 原 pure multi non-resuming lowering 已更名并收口为 `codegen_handle_expr_multi_nonresuming_leaf`，保留为局部 helper；
  - `multi_escape.rs` 的 no-escape fallback 已同步改调新 helper，避免旧命名残留。
- 回归已补齐：
  - 定向单测：`unified_no_continuation_entrypoint_marks_multi_nonresuming_finally_nested_handle_sample`
  - LLVM run-pass fixture：`tests/fixtures/run-pass/effect_multi_nonresuming_finally_nested_handle.scoop`
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc unified_no_continuation_entrypoint_marks_multi_nonresuming_finally_nested_handle_sample -- --nocapture`
  - `cargo test -p scoopc llvm::codegen::effect::tests:: -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_nonresuming_finally_nested_handle.scoop`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo test --all`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 文档状态已完成：
  - `TODO.md` 已将 `T2003r3b3` 标记为完成并补充完成说明；
  - `PLAN.md` 已记录本轮迁移、验证结果，并把当前下一步更新为 `T2003r3c`。
- 当前收尾步骤：
  - 检查 diff 后创建 Git 提交；
  - 本轮到此停止，不进入 `T2003r3c`。
