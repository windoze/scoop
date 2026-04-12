# 当前执行计划

## 约束说明
- 本文件记录可审计的执行计划、决策、进度与变更。
- 不记录冗长的内部推理细节；仅记录高层步骤、依据与结果。
- 按用户要求，在任何进一步代码/命令执行前先初始化本文件。

## 初始步骤
1. 检查最新一次 git 提交信息，确认是否提到了需要先修复的既有问题。
2. 读取 `TODO.md`，识别第一个未完成任务。
3. 读取 `PLAN.md`，对照当前任务与整体计划。
4. 如果首个未完成任务过大，则拆分为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`；本次只执行拆分后的第一个子任务。
5. 实现该任务，必要时补充或调整相关代码结构与注释。
6. 运行相关格式化、lint 与测试，至少覆盖受影响范围；若发现既有问题且属于当前范围，立即修复。
7. 更新 `TODO.md` / `PLAN.md` / 本文件，记录完成情况或阻塞原因。
8. 生成一次 git 提交，提交信息对应当前完成的任务，然后停止。

## 进度记录
- 已初始化本文件，等待仓库检查结果。
- 已检查最新提交 `9d67972e75ff96701f4985ff623ce871694e54ef`；提交信息未显式留下需先修的遗留 issue。
- 已定位首个未完成任务为原 `T2003u4b`，并完成范围审计。
- 审计结论：`T2003u4b` 原范围过大。当前 unified plan 已能决定入口分类，但尚未携带 single-arm emitter 复用旧 replay/capture helper 所需的源码路径元数据；`immediate_resume.rs` 与 `escape_continuation.rs` 仍各自依赖 scanner 重新发现 site。
- 已决定拆分任务：
  1. `T2003u4b1`：先补 unified plan 的 single-site/source-path 元数据，并迁移 single-arm immediate-resume 主 emitter。
  2. `T2003u4b2`：再迁移 single-arm escape-continuation 主 emitter。
- 当前执行目标已切换为 `T2003u4b1`。
- 已完成代码实现：
  - 为 unified plan 的 direct perform site 增加 single-site/source-path 元数据，记录 top-level stmt 与 `block` / `if` / `while` 路径。
  - `codegen_handle_expr_immediate_resume` 已改为从 unified plan 解析 perform-site，不再调用 `scan_immediate_resume_site`。
  - 补回 `while body` nested perform 的既有稳定诊断，避免 build-fail fixture 漂移。
- 已完成验证：
  - targeted fixtures：`effect_resume_yield_int_basic`、`effect_resume_nested_block_single_perform`、`effect_resume_if_then_branch_single_perform`、`effect_resume_if_else_branch_single_perform`、`effect_resume_while_body_single_perform`、`effect_resume_finally_normal`。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 当前任务 `T2003u4b1` 已完成；下一轮应从 `T2003u4b2` 开始。
