# 当前执行计划

## 约束与目标

- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后停止。
- 在开始任何仓库检查与命令执行前，先记录当前计划；后续若计划变化或关键步骤完成，将持续更新此文件。
- 如果最新提交提到预存问题，需要优先修复这些问题，再进入 `TODO.md` 任务。
- 如果首个未完成任务过大，需要先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`。
- 完成任务后需要补测试、更新文档状态，并提交 Git commit。

## 当前已知执行步骤

1. 检查最新一次 Git 提交信息，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`、必要的相关文档与代码，判断该任务是否足够小且可直接实施。
4. 如果任务过大，先拆成更小的子任务，并更新 `PLAN.md` 与 `TODO.md`，然后只执行拆分后的第一个子任务。
5. 实现当前目标任务。
6. 运行相关测试，并补充缺失测试；同时确保 `cargo fmt`、相关测试以及 `cargo clippy --all-targets -- -D warnings` 不出现问题。
7. 更新 `TODO.md`、`PLAN.md`、必要文档（含 `README.md`，若本轮改动需要反映）。
8. 提交本轮改动，提交信息与任务编号或实际改动保持清晰一致。
9. 停止，不继续处理下一个任务。

## 当前风险与待确认事项

- 已检查最新提交：`[T2003c0b2c1a] Support mixed-arm nested-block escape sites`。提交说明未额外列出需要先修复的遗留问题。
- 已定位 `TODO.md` 中首个未完成任务：`T2003c0b2c1b`，主题是 mixed-arm `handle` 中 sibling escape-continuation 在 `if branch` 的 direct site。
- 已完成实现面审计：该任务可以直接实现，不需要继续拆分。
- 计划中的代码改动点：
  1. 扩展 mixed-arm direct-site 扫描路径，让 `if then/else` branch 内的 direct perform site 能被记录为 resume path。
  2. 扩展 mixed-arm body decl 收集、used-after 分析，以及 nested prefix/tail replay helper，使其识别 `if` 分支 tail 与分支后的合流。
  3. 扩展 site-matrix lowering：同一顶层 `if` 语句中允许出现 then/else 两个候选 escape site，并在 `state0`、`state1`、step trampoline 中按运行时条件选择命中的分支。
  4. 保留 while body 与 nested indirect 的稳定诊断，不把范围扩到 `T2003c0b2c2` / `T2003c0b2c3`。
  5. 新增/调整 fixtures：补 pre-immediate 与 post-immediate 的 if-branch run-pass 回归，并把旧的 nested-if 负例改成 while body 负例或等价稳定诊断覆盖。
- 完成代码后将运行格式化、相关测试和 clippy，然后更新 `TODO.md` / `PLAN.md` 并提交。
- 已完成实现：
  - `crates/scoopc/src/llvm/codegen/effect.rs` 已支持 mixed-arm sibling escape-continuation 在 statement-position `if` then/else branch 中的 direct site。
  - step trampoline、pre-immediate `state0`、post-immediate `state1` 已接入共享的 if-branch dispatch helper。
  - 已新增 fixtures：`effect_resume_mixed_escape_pre_immediate_if`、`effect_resume_mixed_escape_post_immediate_if`；旧的 nested-if 负例已替换为 `effect_resume_mixed_escape_while_is_error`。
- 已完成验收：
  - `cargo fmt --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
- 当前剩余动作：检查工作区、提交本轮改动，然后停止。
- 可能存在用户已有未提交改动；执行时需避免覆盖无关修改。
- 任务可能依赖尚未实现的语言特性；若确认缺失，需要按要求调整 `TODO.md` / `PLAN.md` 后提交并停止。
