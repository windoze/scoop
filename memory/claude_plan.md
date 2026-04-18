## 当前执行计划

说明：我不会写入逐字的内部推理细节，但会持续记录可公开的判断依据、执行步骤、进度和计划变更，便于检查当前状态。

### 初始步骤

1. 检查最新一次 Git 提交信息，确认是否提到尚未修复的既有问题；如果有，优先修复。
2. 查看 `TODO.md`，找出第一个未完成任务。
3. 查看 `PLAN.md`，确认当前计划和任务依赖是否一致。
4. 评估第一个未完成任务的范围：
   - 如果任务足够小，直接实现。
   - 如果任务过大或存在前置依赖，先细分任务并更新 `TODO.md` / `PLAN.md`。
5. 实现当前应执行的第一个任务。
6. 运行相关测试，并补充必要测试，确保实现正确且无回归。
7. 运行质量检查，至少包括与改动相关的测试，以及尽可能完整的格式化/编译/Clippy 检查。
8. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成情况或阻塞原因。
9. 提交本次改动，完成后停止，不继续处理下一个任务。

### 当前假设

- 需要先读取仓库现状后，才能确定本轮真正要执行的具体任务。
- 如果发现规范不匹配、缺失特性或已有回归，需要先把问题显式写入 `TODO.md` 并调整依赖，再决定是否继续实现原任务。

### 进度

- 已创建本计划文件。
- 已检查最新提交：`1d312c6c99ddd9086af05acfddaea8508c90725c`，提交说明为 `[T3016f] Fix top-level multi-site escape replay`。提交信息本身未额外声明需要先行修复的独立既有问题；计划文件与任务清单均把后续动作指向 `T3016fR`。
- 已检查 `TODO.md` / `PLAN.md`：当前第一个未完成任务是 `T3016fR Review：确认 top-level multi-escape replay 已回到统一 resume-path 合同`。
- 已完成 `T3016fR` 的生产代码复审：`attach_escape_resume_targets()` 的修复只使用既有 suspend-site 元数据 `source_path.top_level_stmt_idx` 来裁剪 replay action；未发现按 helper 名称、fixture 名称、direct/indirect 顺序或其他源码形状新增的 fallback。
- 已确认 `escape_resume_target`、`source_path`、`resume_path` 继续只作为统一元数据在 plan → segments → unified machine 间 round-trip；emitter 侧公开访问面仍只有 `escape_resume_state()`，没有重新按源码形状做 replay 决策。
- 已完成验证：
  - 结构测试：`source_plan_preserves_same_statement_escape_replay_prefix_for_nested_block_call_site`
  - 结构测试：`source_plan_trims_escape_replay_to_current_top_level_statement`
  - 结构测试：`resume_path_is_preserved_from_plan_to_segments_to_unified_machine`
  - 运行时 fixture：`effect_multi_escape_custom_nonresuming_direct_indirect_multi.scoop`
  - 运行时 fixture：`effect_multi_escape_custom_nonresuming_indirect_multi.scoop`
  - 运行时 fixture：`effect_multi_escape_indirect_multi.scoop`
  - 运行时 fixture：`effect_resume_mixed_multi_escape_direct_indirect.scoop`
  - 回归 fixture：`effect_multi_escape_custom_nonresuming_direct_block_multi.scoop`
  - 全量：`cargo test --all`
  - 质量门槛：`cargo clippy --all-targets -- -D warnings`
- 结果：未发现需要在 `T3016fR` 内直接补修的生产问题。下一步是更新 `TODO.md` / `PLAN.md` 标记完成并提交；完成后停止，本轮不进入 `T3016g`。
