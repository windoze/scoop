## 当前执行计划

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务；以该任务为本次唯一执行目标。
2. 读取最近一次提交信息，检查是否存在与该任务直接相关且明确未完成的问题；若存在，将其视为当前任务的一部分或按要求补充为前置任务。
3. 仅围绕该任务读取必要代码与文档，确认需求、依赖、现状和验证要求。
4. 实现任务；若遇到阻塞当前任务且不能按规范继续推进的问题，则先在 `TODO.md` 中增加最小前置任务并停止。
5. 运行该任务要求的验证，以及必要的回归测试、格式化和无警告检查。
6. 更新 `memory/claude_plan.md` 记录关键进展与计划变化。
7. 在 `TODO.md` 中将完成的任务标题显式标记为 `[DONE]`，补全完成记录；仅在阶段计划变化时更新 `PLAN.md`。
8. 按仓库约定创建一次 git 提交，然后停止，不进入下一个任务。

## 记录约定

- 不记录隐式内部推理；这里只保留可审计的决策摘要、执行步骤、发现的问题与验证结果。
- 每完成关键步骤、发现阻塞、或调整计划时，立即更新本文件。

## 当前任务

- 已读取 `TODO.md`，当前首个未完成任务为 `P0-T02R：Review 审计脚手架与测试基线，确认后续任务不会被旧字符串绑定卡住`。
- 已检查最近一次提交：`[P0-T02] Harden stable-id test baselines`。提交信息未显式声明与 `P0-T02R` 直接相关的未完成问题。

## 本轮细化计划

1. 阅读 `P0-T02R` 指定文件与相关 helper，确认 object/symbol 审计入口、旧命名断言迁移情况、以及 `.cone` / JSON 健康基线边界。
2. 复跑 P0-T01 / P0-T02 要求的测试与 grep 审计，核对 review 任务要求的验证入口是否完整可复用。
3. 若 review 发现会阻塞后续 P1-P7 的具体缺口，则在 `TODO.md` 中插入最小前置任务并停止；若未发现阻塞，则补全 `P0-T02R` 完成记录并标记 `[DONE]`。
4. 提交本轮变更并停止。

## 执行记录

- 已审阅 `crates/scoopc/src/llvm/tests.rs` 中 stable-id 审计 helper 与 source inventory，以及四个健康 `.cone` / JSON 基线测试文件。
- 已复跑并通过：
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_audit -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc external_symbol -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_source_inventory -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc path_free -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
- review 发现的具体阻塞：`crates/scoopc/src/llvm/tests.rs` 仍有多处 stable-id 敏感行为测试直接绑定旧 private helper spelling，例如：
  - `default_single_file_ir_helper_lowers_handle_main_without_hir_fallback` 仍命中 `__scoop_refactor_resume__a_main__case0`
  - `direct_call_with_real_outward_effect_uses_step_boundary_and_surface_resume_dispatch` 仍命中 `__scoop_refactor_direct_invoke__a_entry` / `__scoop_refactor_surface_resume_owner_dispatch__...`
  - `closure_call_with_real_outward_effect_uses_explicit_outcome_boundary` 仍命中 `__scoop_refactor_direct_invoke__a_entry__lambda0`
  - `top_level_immutable_init_emits_explicit_root_frame_descriptor` 仍命中 `@__scoop_explicit_root_desc____scoop_top_level_val_init__a_greeting`
  - `effect_state_machine_functions_emit_explicit_root_frame_descriptors` 仍命中 `@__scoop_explicit_root_desc____scoop_refactor_direct_invoke__a_go`
- 结论：当前 `P0-T02R` 不能标记完成；已在 `TODO.md` 中插入最小前置任务 `P0-T02A`，用于清理这些剩余旧 private helper 名字绑定，并将 `P0-T02R` 依赖调整为 `P0-T02A`。
