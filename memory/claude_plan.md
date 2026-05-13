## 当前执行计划

1. 读取 `TODO.md`，按标题是否带有 `[DONE]` 判断首个未完成任务；不依据完成记录文字跳过任务。
2. 检查最近的提交信息，确认是否存在与该任务直接相关且明确未完成的事项；若有，则将其视为当前任务范围或在 `TODO.md` 中补充为前置任务。
3. 阅读当前任务涉及的代码、测试、规范与依赖，确认需要修改的最小范围，并识别是否存在阻塞当前任务的真实缺口。
4. 若无阻塞：直接实现当前任务，优先采用最小且正确的改动；若存在真实阻塞：先在 `TODO.md` 中加入最小前置任务并调整依赖顺序，然后停止本次执行。
5. 运行与当前任务直接相关的验证；在可行范围内补充必要测试，并执行任务要求的构建/测试/静态检查命令。
6. 更新 `memory/claude_plan.md` 记录关键进展与计划变化；将完成的任务在 `TODO.md` 标题前加上 `[DONE]`，并补全 completion record；仅在阶段计划真正变化时更新 `PLAN.md`。
7. 检查工作区中与本任务相关的改动，使用符合仓库风格的提交信息创建一次提交；完成后停止，不继续处理下一个任务。

## 计划记录约定

- 这里记录执行步骤、关键决策、阻塞与验证结果。
- 这里不会记录逐字逐句的内部推理，而是记录可审计的外部执行计划与进展。

## 当前任务识别（已更新）

- `TODO.md` 中首个未完成任务为 `P3-T02：用 stable schema key / canonical type hash 替换 effect helper 与 transport type 的 private naming`。
- 最近提交 `13be4d69 [P3-T01] Stabilize closure private naming` 与当前阶段直接相邻，但提交标题本身未显式声明未完成的附属问题；仍需继续读取任务正文与相邻完成记录，确认是否存在需要并入本任务的直接遗留项。

## 面向 P3-T02 的执行计划

1. 读取 `P3-T01` 与 `P3-T02` 的任务正文、完成记录，以及 `PLAN.md` / `STABLE_ID.md` 中 P3 对应要求。
2. 搜索 effect helper、transport type、schema key、canonical type hash、private mangling 当前实现位置，明确还在使用 dense id、旧 schema/id 文本或 ad hoc naming 的代码路径与测试。
3. 判断是否存在阻塞 `P3-T02` 的真实缺口；若存在且未被 `TODO.md` 跟踪，则最小化新增前置任务并停止；若不存在，则直接在当前任务内完成迁移。
4. 实现 `P3-T02` 所需的 naming 来源替换与相关测试迁移，优先复用现有 `stable_id` authoritative API，不新增平行命名/哈希逻辑。
5. 运行任务要求的定向测试、全量 `scoopc` 测试与 `clippy -D warnings`，必要时修复验证中暴露的问题。
6. 更新 `memory/claude_plan.md`、`TODO.md` 的完成记录并提交一次 git commit，然后停止。

## 当前进展

- 已确认生产代码中当前任务的旧 private naming 入口至少包括：
  - `effect_lowered/layout.rs`：`refactor_resume`、`surface_resume`、`dynamic_invoke`、`direct_invoke`、`surface_resume_owner_dispatch`、carrier dynamic entry shell。
  - `effect_lowered/body.rs`：`surface_resume_outcome`、`continuation_drive_outcome`、`task_transport_resume`、effect transport box/type。
  - `effect_lowered/value.rs`：`plain_adapter`、`closure_step_adapter`、`thread_resume_u64`、`thread_resume_transport`。
- 其中 `value.rs` 的 adapter/thread-resume helper 虽未在 `TODO.md` 条目正文逐项列出，但与当前任务同属“private effect helper naming 仍受 schema/continuation/type id 控制”的同根问题，按类修复要求并入本任务一起处理。
- 当前实现策略：新增 `effect_lowered` 内部稳定命名 helper，基于现有 `stable_id` canonical encoder / `PrivateSymbolMangler` 统一生成：
  - callable version 语义 key
  - effect/continuation schema key
  - transport type canonical key
  并让 `layout.rs`、`body.rs`、`value.rs` 的 helper/type 名统一改走这条路径。

## 已完成的关键步骤

1. 新增 `crates/scoopc/src/llvm/codegen/effect_lowered/stable_naming.rs`，集中构造：
   - callable version key text
   - effect schema key text
   - continuation schema key text
   - transport box/type stable name
   - 基于 canonical text 的 `PrivateSymbolMangler` 包装
2. 把以下旧命名入口迁移到 stable key / canonical type hash：
   - `effect_lowered/layout.rs`：`refactor_resume`、`surface_resume`、`dynamic_invoke`、`direct_invoke`、`surface_resume_owner_dispatch`、closure/vtable/itable dynamic entry
   - `effect_lowered/body.rs`：`surface_resume__outcome`、`surface_resume_owner__outcome`、`surface_resume_owner__core`、`continuation_drive__outcome`、`continuation__step`、`task_transport_resume`、effect transport box/type
   - `effect_lowered/value.rs`：`plain_adapter`、`closure_step_adapter`、`thread_resume_u64`、`thread_resume_transport`
3. 为了让 `body.rs` / `value.rs` 在缺少完整上下文时仍能复用 authoritative identity，给 ABI layout 结构补充了 stable key text：
   - `RefactorStepLayout`
   - `RefactorCallableLayout`
   - `RefactorPlainCallableLayout`
   - `RefactorContinuationSurfaceResumeLayout`
   - `RefactorContinuationSurfaceResumeOwnerTrampolineLayout`
4. 迁移了 3 条被新 naming 触发的 LLVM 测试定位逻辑，改为按 private role / step type / helper family 定位，而不再假设 effect helper 带 FQN 或 `kN`。

## 验证结果

- `cargo fmt`
- `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_ -- --nocapture`
- `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc surface_resume -- --nocapture`
- `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc closure_call_with_real_outward_effect_uses_explicit_outcome_boundary -- --nocapture`
- `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc composed_continuation_resume_publishes_internal_outcome_surface_and_owner_core -- --nocapture`
- `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc external_symbol_audit_closure_effect_and_hidden_init_helpers_smoke -- --nocapture`
- `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
- `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
- 精确 grep（`crates/scoopc/src/llvm/codegen`）结果：
  - `__schema[0-9]+`：0 命中
  - `__k[0-9]+`：0 命中
  - `t[0-9]+__`：0 命中
