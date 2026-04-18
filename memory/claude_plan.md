# 本轮执行计划（决策摘要）

## 目标
- 只完成 `TODO.md` 中第一个未完成任务，然后停止。
- 在开始任务前，先检查最新一次提交是否提到任何既有问题；若有，先修复这些问题。
- 全程同步更新本文件，记录计划变化、关键发现和执行进度。

## 约束与执行原则
- 不采用规避方案、兼容性补丁或仅为测试通过而做的夹具式修复。
- 若发现规范不匹配、缺失能力或前置依赖缺口，必须先把问题转成 `TODO.md` 中更靠前的任务，并更新 `PLAN.md`。
- 必须补齐实现、测试、文档更新，并提交 Git commit。
- 本轮只处理一个任务；完成后立即停止，不继续下一个。

## 执行步骤
1. 检查当前工作树状态，避免误覆盖已有修改。
2. 查看最新提交信息，确认是否显式提到需要先修复的既有问题。
3. 阅读 `TODO.md`，定位第一个未完成任务。
4. 阅读 `PLAN.md` 及相关上下文，判断该任务是否可直接完成；若过大，则拆分任务并先更新 `TODO.md`/`PLAN.md`。
5. 实现当前要执行的任务，必要时阅读相关代码、测试和规范文件。
6. 运行针对性测试，再运行必要的全量校验，至少覆盖：
   - 相关测试
   - `cargo test --all`（若与改动相关且成本可接受）
   - `cargo clippy --all-targets -- -D warnings`
7. 更新 `TODO.md`、`PLAN.md` 和本文件的进度记录。
8. 用清晰的提交信息创建一次 Git commit。
9. 停止。

## 进度记录
- 已创建本轮计划文件，尚未开始仓库检查。
- 已检查最新提交、`TODO.md`、`PLAN.md`：
  - 最新提交 `2e82e0ec224d29f204d2d39d5b78c7bb67af2006` 只有 `T3016bR` 复审与计划更新，没有提交说明中显式提到的额外既有问题。
  - 当前第一个未完成任务为 `T3016c`：接回 outer-body `Continuation.resume(...)` 在 `when` / nested handle 场景中的生产 lowering。
  - `PLAN.md` 已把 `T3016c` 列为 `T3016bR` 之后的下一项，因此本轮按该任务推进。
- 当前执行重点：
  1. 复现 `effect_escape_continuation_finally_normal.scoop` 与 `effect_escape_continuation_nested_outer_resume_inner_multi.scoop` 的失败。
  2. 检查 `crates/scoopc/src/llvm/codegen/effect/mod.rs` 与相关 emitter / plan / typecheck side table，确认 outer-body `Continuation.resume(...)` 为什么没有进入 dedicated lowering。
  3. 若根因单一则直接修复；若发现任务实际混合多个独立缺口，再按要求拆分 `TODO.md` / `PLAN.md`。
- 已执行的关键复现与结论：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_finally_normal.scoop`
    - 当前结果：`unsupported_main_body: call callee`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_nested_outer_resume_inner_multi.scoop`
    - 当前结果：`unsupported_main_body: effect frame seed outer-scope local`
  - 通过阅读 `crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/mod.rs`、`crates/scoopc/src/typecheck/expr/stmt.rs`、`crates/scoopc/src/typecheck/expr/call.rs` 确认：
    - 普通 call codegen 只按 `continuation_resume_call_sites` 命中 `Continuation.resume(...)` dedicated lowering。
    - `check_expr_stmt()` 当前不会对 statement-position `Continuation.resume(...)` 运行 builtin typecheck 规则，因此这类 call site 不会写入 `continuation_resume_call_sites`，也不会记录 `Raise<RuntimeError>` required effects。
    - 这与现有 typecheck fixtures `continuation_resume_requires_raise_runtime_error_missing_is_error.scoop` / `continuation_type_and_resume_pure_ok.scoop`，以及 run-pass fixture `effect_escape_continuation_perform_not_first_statement.scoop` 注释中“需要 try/catch 维持 `main` 的 Pure 约束”的既有合同一致冲突。
    - `effect_escape_continuation_finally_normal.scoop` 目前之所以能前进到 codegen，而不是在 typecheck 报 `required_effect_not_declared`，正是因为 statement-position `Continuation.resume(...)` 绕过了 builtin typecheck。
    - `effect_escape_continuation_nested_outer_resume_inner_multi.scoop` 则独立暴露真实生产缺口：nested handle outer-body resume 时，inner frame seeding 仍可能拿不到 enclosing scope local。
- 因此，本轮最终判断：
  - 原 `T3016c` 混合了两个不同层级的问题，不能继续作为单一实现任务推进。
  - 更前置的问题是前端/typecheck 语义缺口：statement-position `Continuation.resume(...)` 没有走统一 builtin 合同。
  - 其后才是 production lowering 缺口：spec-correct outer-body resume 与 nested handle frame seeding。
- 已采取的动作：
  - 将原 `T3016c` 拆分并前移为：
    1. `T3016c0` / `T3016c0R`：先修 statement-position `Continuation.resume(...)` 的 typecheck side-table / required-effects 合同，并重新分类 `effect_escape_continuation_finally_normal.scoop`。
    2. 收窄后的 `T3016c` / `T3016cR`：再处理 spec-correct outer-body lowering 与 nested-handle frame seeding。
  - 已更新 `TODO.md` 与 `PLAN.md` 的任务定义、依赖关系与执行顺序。
  - 已撤回本轮所有临时调试测试改动，当前仅保留文档/记忆文件变更。
  - 已执行 `cargo check -p scoopc`，确认撤回临时代码后仓库仍可编译。
