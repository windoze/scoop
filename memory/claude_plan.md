# 执行计划

## 说明

用户要求先写入“完整思考过程”。我不会记录不可共享的内部推理，但会持续记录可执行计划、关键判断依据、进度状态与后续动作，便于检查执行过程。

## 初始计划

1. 检查最新一次提交的提交说明与改动范围，确认是否提到了尚未修复的问题。
2. 若最新提交暴露了前置问题，先定位并修复这些问题，再继续后续任务。
3. 阅读 `TODO.md`，定位第一个未完成任务。
4. 评估该任务是否能在本轮完整交付。
5. 如果任务过大，则拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，然后执行拆分后的第一个子任务。
6. 实现当前目标任务，确保实现符合规范，不引入规避性方案。
7. 运行相关测试与质量检查，至少覆盖受影响范围；如有必要运行更广泛测试。
8. 更新 `memory/claude_plan.md`、`PLAN.md`、`TODO.md`，记录完成状态或阻塞原因。
9. 使用清晰的提交信息提交本轮改动。
10. 停止，不继续处理下一个任务。

## 进度记录

### 2026-04-12：初始化检查

- 已检查最新提交 `a5b6a35 [T2003c0c2b3c2-3] Refactor effect handler scaffolds`。
- 该提交说明未显式提及额外的 pre-existing issue；当前未发现必须先于 TODO 主线处理的新问题。
- 已检查 `TODO.md` / `PLAN.md`。
- 当前第一个未完成任务为 `T2003c0c2b3c3`：
  `Effect：LLVM 多 arm handle dispatch（无 immediate-resume，if branch indirect escape sites）`。

## 当前状态

- 状态：实现、回归与 lint 已完成，正在更新计划文件并准备提交。
- 已完成事项：
  1. 修改 `mixed.rs`，让 no-immediate indirect 路径支持 if-branch indirect site，并把 initial body / continuation step 都接到现有 if-branch helper。
  2. 新增 run-pass fixture `effect_multi_escape_custom_nonresuming_indirect_if_multi`，覆盖 then/else 两个分支与 sibling non-resuming dispatch。
  3. 删除旧的 if build-fail fixture；确认 while indirect build-fail 仍稳定报错，继续作为下一任务边界。
  4. 已运行并通过：
     - `cargo test --all`
     - `cargo run -p scoop -- test`
     - `cargo run -p scoop --features llvm -- test`
     - `cargo clippy --workspace --all-targets -- -D warnings`
  5. 已更新 `TODO.md` / `PLAN.md`，将 `T2003c0c2b3c3` 标记为完成，并把下一步切换为 `T2003c0c2b3c4`。
- 下一步：
  1. 提交本轮改动并停止。
