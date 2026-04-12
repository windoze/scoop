# 执行计划（本轮）

## 约束说明

- 按用户要求，本文件会在正式实现前先记录执行计划，并在关键步骤完成或计划调整时持续更新。
- “完整思考过程”不在此记录；这里记录的是可审计的高层决策、执行步骤、发现的问题与调整。
- 本轮目标是只完成 `TODO.md` 中**第一个未完成任务**，完成后立即停止。

## 初始执行步骤

1. 检查最新一次 Git 提交信息，确认是否提到已知问题、临时修复或待补项。
2. 若最新提交中存在明确指出但尚未修复的问题，先修复这些问题，并补充测试。
3. 阅读 `TODO.md`，定位第一个未完成任务。
4. 评估该任务是否足够小且可在一轮内完整交付。
5. 若任务过大，则：
   - 在 `PLAN.md` 中拆解为更小的子任务；
   - 在 `TODO.md` 中按依赖顺序插入/替换这些子任务；
   - 选择拆解后的第一个子任务作为本轮执行目标。
6. 读取与目标任务直接相关的代码、规范、测试和文档，确认实现边界。
7. 实现目标任务，避免任何规避式做法、临时兼容层或偏离规范的折中。
8. 运行相关测试；若涉及公共行为或基础设施变更，扩大到必要范围的测试与 `clippy`/格式化检查。
9. 更新 `TODO.md` 与 `PLAN.md`，记录已完成状态或阻塞原因与依赖调整。
10. 使用清晰的 Git 提交信息提交本轮结果，然后停止，不继续处理下一个任务。

## 当前未知项

- 最新提交是否遗留了必须先修复的问题。
- `TODO.md` 中当前第一个未完成任务的内容、范围与依赖。
- 目标任务是否暴露新的规范缺口或实现缺陷，需要先补前置任务。

## 进度记录

- 已创建本计划文件，待下一步读取仓库状态并识别任务。
- 已检查最新一次提交：提交说明为 `[T2003c0c2b3c3] Support no-immediate if-branch indirect escape sites`，未发现提交说明中显式列出的“已知未修复问题”或要求先处理的遗留缺陷。
- 已定位 `TODO.md` 中首个未完成任务为 `T2003c0c2b3c4`：无 immediate-resume 的 `while` body indirect escape sites。
- 已确认该任务不需要继续拆分：`matrix.rs` 已具备 while-indirect replay helper，缺口主要在 `mixed.rs` 的 no-immediate indirect 分流、stmt 分类与 continuation step 接线。
- 已完成实现：
  - 放开 no-immediate indirect 路径对 `while body` resume-path 的门禁；
  - 在 no-immediate indirect 的 site 分类中新增 `while_indirect_site_pc_by_stmt_idx`；
  - 在 initial body 与 continuation step 两处接入 `codegen_mixed_escape_matrix_while_stmt_indirect_site` / `codegen_mixed_escape_matrix_while_tail_after_indirect_site`。
- 已完成局部验证：
  - `cargo build -p scoop --features llvm` 通过；
  - 新增 run-pass 样例 `effect_multi_escape_custom_nonresuming_indirect_while_multi`，手动 `scoop run` 输出与预期一致；
  - 新增 build-fail 样例 `effect_multi_escape_direct_indirect_while_is_error`，手动 `scoop build` 稳定报 `handle multi-arm without immediate-resume (escape site matrix not yet supported)`。
- 待完成：
  - 提交本轮改动。

- 已完成全量验证：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 已同步项目记录：
  - `TODO.md` 已将 `T2003c0c2b3c4` 标记为完成，并记录新增正/负例。
  - `PLAN.md` 已记录实现边界，并把下一步推进到 `T2003c0c2b3d`。

## 本轮任务拆解（T2003c0c2b3c4）

1. 读取 `TODO.md` / `PLAN.md` 中该任务的目标、验收条件与依赖说明。
2. 阅读当前 no-immediate mixed-arm lowering 的相关实现，重点查看：
   - top-level / nested block / if-branch indirect escape 已支持路径；
   - while body indirect 当前的稳定诊断与边界判断；
   - 相关 fixture 与测试组织方式。
3. 判断该任务是否可直接实现；如果范围仍过大，再按用户流程拆为更小子任务并同步更新 `TODO.md` / `PLAN.md`。
4. 实现 while body indirect escape lowering：
   - 扫描 while body 内 indirect site；
   - 正确保存/恢复 continuation step 所需状态；
   - 保持 sibling non-resuming dispatch、handler scope、resume 后 replay 与 loop 迭代语义正确。
5. 补充或调整 run-pass / build-fail fixtures，覆盖 while body indirect 的正向路径与仍不支持的边界。
6. 运行相关测试，再扩大到任务要求的完整验收命令。
7. 更新 `TODO.md` / `PLAN.md`，标记任务完成并记录实现边界。
8. 提交本轮改动并停止。
