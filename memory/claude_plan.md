# 当前执行计划

## 约束与执行原则

- 先读取 `TODO.md` 作为索引，再按索引打开对应的 `TODO-Px.md`，以详细任务文件为准识别第一个标题未带 `[DONE]` 的任务。
- 本轮只完成第一个未完成的详细任务；完成后更新详细 TODO、必要时同步 `TODO.md`，提交 Git，然后停止。
- 若发现当前任务被具体实现缺口阻塞，不绕过、不降级；只添加最小必要前置任务并同步索引，提交后停止。
- 不回滚或覆盖用户已有改动；若遇到冲突性改动，先记录并询问。
- 本文件记录可审计的计划、关键发现和进度更新；不记录隐藏推理链。

## 步骤计划

1. 读取 `TODO.md`，确认索引顺序和引用的详细任务文件。
2. 按顺序读取相关 `TODO-Px.md` 文件，找到第一个标题未带 `[DONE]` 的详细任务。
3. 检查最新提交是否明确提到与该任务直接相关的未完成问题；若相关，将其纳入当前任务或作为前置任务记录。
4. 阅读当前任务涉及的代码、测试和规范，确定最小正确实现范围。
5. 实现当前任务；若必须修改计划或发现阻塞，立即更新本文件。
6. 运行相关测试；根据失败结果修复问题并重新验证。
7. 更新对应 `TODO-Px.md` 的任务标题为 `[DONE]` 并填写完成记录；如索引项状态或任务顺序变化，同步 `TODO.md`。
8. 运行必要的最终检查。
9. 检查 Git 状态与差异，提交所有本轮相关改动。
10. 停止，不进入下一项任务。

## 当前状态

- 已读取 `TODO.md`，第一个未完成索引项为 `P6-T03c`。
- 已读取 `TODO-P6-part3.md`，确认本轮详细任务是 `P6-T03c：实现 refactor pure statement lowering，停止调用 legacy statement-level lowering`。
- 已检查最新提交：`[P6-T03b] Publish source slice statement classification`，未发现直接声明的未完成 blocker。
- 初始验证发现 `effect_refactor_dynamic_entry_publication_emit_llvm.scoop` 当前失败在 `ClassCtor` 被分类为 unsupported；同一 source slice 还包含 pure direct call 与 closure materialization，需要由 `P6-T03c` 闭合。
- 执行方向：更新 source-slice classification，使 class ctor / closure carrier / pure call 进入 refactor pure statement lowering；把 `value.rs` 的 statement lowering 改为本地分派，不再调用 `codegen_mir_effect_neutral_statement`；pure direct call 使用 refactor callable layout 调 direct entry 并提取 `Step_F::Complete` payload，effectful/dynamic call 继续 fail fast。
- 已实现：`value.rs` 本地 lower refactor pure statements；`ClassCtor` / `MakeClosure` / pure direct `Call` 进入 effect-neutral classification；旧 `codegen_mir_effect_neutral_statement` 已移除。
- 已实现：pure direct call 使用 refactor callable direct entry，按 published source ABI pack args，并从 returned `Step_F::Complete` 提取 payload；effectful、dynamic、virtual/interface/resume call 仍 fail fast。
- 已修正：body emitter 按 callable `body_version_key` 消费 ABI visibility layout 的 step/frame schema，避免 `--opt-level 0` primary program 的局部 `StepSchemaId` 与 ABI visibility program 漂移导致 completion binding 取错。
- 已通过：`cargo test -p scoopc refactor_llvm_pure_statement_lowering`；`cargo test -p scoopc refactor_llvm_member_read_write_lowering`；`cargo test -p scoopc refactor_effect_lowered_source_slice_classification`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`。
- 已通过：`cargo clippy --all-targets -- -D warnings`。
- 已更新：`TODO-P6-part3.md` 中 `P6-T03c` 标记为 `[DONE]` 并填写完成记录；`TODO.md` 索引已同步 `[DONE]`。
- 下一步：检查工作区差异并提交本轮所有相关改动。
