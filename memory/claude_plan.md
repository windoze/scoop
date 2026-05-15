## 执行计划

说明：我不会记录逐字的内部推理，但会在此维护可审阅的执行摘要、决策依据和步骤进展。

1. 读取 `TODO.md`，定位第一个标题中未带 `[DONE]` 的任务；同时检查最近一次提交是否直接提到与该任务相关且未完成的问题。
2. 读取该任务对应说明、依赖、验证要求；如有必要再读取 `PLAN.md` 获取阶段级上下文，但不把 `PLAN.md` 当作日常任务台账。
3. 检查工作区当前状态，识别是否已有与当前任务直接相关的未提交更改；若存在，谨慎在其基础上继续，不回退非本人修改。
4. 实现当前任务所要求的代码与测试改动；若遇到阻塞当前任务的真实缺陷或缺失能力，先按要求把最小前置任务写入 `TODO.md`，更新依赖顺序，并停止继续向前实现。
5. 运行任务要求的验证命令，以及必要的格式化、测试、lint/`clippy` 检查，修复发现的问题，直到当前任务满足验收条件。
6. 更新文档记录：
   - 在 `TODO.md` 中把当前任务标题显式改为 `[DONE]`。
   - 补全该任务 completion record。
   - 仅当阶段计划、依赖结构或完成标准变化时更新 `PLAN.md`。
7. 复核 `memory/claude_plan.md`，记录关键结果、阻塞处理或计划调整。
8. 按仓库提交风格创建一次 git 提交，提交本次任务涉及的全部未提交文件，然后停止，不进入下一个任务。

## 当前状态

- 状态：已读取 `TODO.md` 与最近一次提交；首个未完成任务确认为 `P4-T01c`。
- 最近提交：`[P4-T01c-pre1] Add sysroot overlay fixture support`，未发现提交信息中额外声明的、会直接改写当前任务顺序的未完成事项。
- 当前工作区：仅有 `memory/claude_plan.md` 的本次进度记录修改。
- 已识别的直接阻塞：
  - `@Intrinsic` 的 sysroot 免 gate 判定目前主要依赖路径/目录启发式；task-specific overlay `*.sysroot/core.scoop` 需要被稳定视为 sysroot 源，否则 `P4-T01c` 的 overlay fixture 会被误判为用户源码。
  - `typecheck/annotations.rs` 当前仍硬性禁止 intrinsic nominal 的成员函数带 body；这与 `P4-T01c` 目标正面冲突。
- 已采取的修正：
  - 为 `SourceFile` 增加显式来源标记，并让 sysroot/support-source 加载链路保留 `Sysroot` 身份；同时保留旧路径启发式作为回退。
  - 把 intrinsic nominal 的前端门禁从“成员函数必须无 body”改为“允许普通方法 body，但禁止声明字段（含 ctor property field / direct body field）”。
- 后续实现中额外处理的直接问题：
  - zero-field intrinsic struct 最初暴露出 composite transport `size_bytes == 0` gate 误拒绝；最终改为允许 zero-sized nominal payload 保留自身 value-box/type-desc/itable 身份，而不是退回 `BoxedUnit`。
  - class `override` 校验原先只看 superclass、不看 direct interface，导致 generic intrinsic class 的 `override` 被误报“目标不存在”；现已补到 direct interface members。
- 当前结果：`P4-T01c` 已实现完成，并已回写到 `TODO.md` 为 `[DONE]`。
- 已完成验证：
  - 新增 targeted fixtures（run-pass/build/typecheck）全部通过。
  - 新增 LLVM owner tests 全部通过。
  - `cargo fmt --all`、`cargo clippy --all-targets -- -D warnings` 通过。
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 未引入新的 run-pass 回归；仍存在两个既存失败：`extern_native_aggregate_return_direct_indirect_parity.scoop`、`sync_gc_release_task_like_object_basic.scoop`。
  - `cargo test -p scoopc llvm_tests -- --nocapture` 仍是 harness filter `0 passed`，实际 owner coverage 由新增的三条单测承担。
- 下一步：检查 `git status` 后按任务要求创建一次提交并停止。
