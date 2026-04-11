# 当前执行计划

## 说明

用户要求先把计划写入本文件，再开始执行任何命令。这里记录的是可审计的执行计划、关键判断和进度摘要，不包含不可验证的内部推理细节。

## 总目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。

## 执行步骤

1. 检查最新一次 Git 提交信息，确认是否提到了已知问题、遗留缺陷或需要先处理的事项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务的上下文、依赖与当前项目阶段。
4. 如果该任务过大，则把它拆分为更小的可执行子任务，并同步更新 `PLAN.md` 与 `TODO.md`，本轮仅执行拆分后的第一个子任务。
5. 调研并阅读与目标任务直接相关的代码、测试、文档和已有实现。
6. 实现任务，必要时补充或重构代码，保持改动范围尽量聚焦。
7. 运行格式化、静态检查和相关测试，至少覆盖：
   - `cargo fmt --check` 或等效格式化检查
   - `cargo clippy --all-targets -- -D warnings`
   - 与本任务相关的测试
   - 如有必要，再运行更大范围测试
8. 修复执行过程中发现的相关问题，直到任务完成且测试通过，且不引入新的警告。
9. 更新文档与项目计划：
   - 在 `TODO.md` 中将本轮完成任务标记为已完成
   - 在 `PLAN.md` 中记录当前状态和后续影响
   - 继续更新本文件，记录关键进展或计划调整
10. 检查工作区改动，确认不误伤用户已有改动。
11. 提交 Git commit，提交信息聚焦本轮完成的任务。
12. 停止，不继续处理下一个任务。

## 当前状态

- 已完成：计划文件初始化
- 已完成：检查最新提交，未发现提交信息中额外声明需优先修复的遗留问题
- 已完成：识别首个未完成任务为 `T2002b`
- 已完成：确认 `T2002b` 无需继续拆分，可在本轮直接实现
- 已完成：完成 `CalleeSuspendState` 双通道 payload ABI 改造，覆盖 top-level function / closure 的恢复值解码，以及间接 perform step 对 callee state 的写回
- 已完成：新增两组 `T2002b` run-pass 回归：
  - `effect_escape_continuation_indirect_perform_resume_string`
  - `effect_escape_continuation_indirect_perform_resume_struct_with_ref`
- 已完成：单独编译并运行上述新 fixture，输出符合预期
- 已完成：测试与静态检查通过
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 已完成：更新 `TODO.md` 与 `PLAN.md`，将 `T2002b` 标记为完成并记录后续转向 `T2003`
- 进行中：检查工作区改动并准备提交

## 当前任务判断

- 当前首个未完成任务：`T2002b`
- 任务主题：泛化 escape continuation / `CalleeSuspendState` 的恢复值 ABI，使间接 `perform` + `resume(...)` 路径支持 `String`、引用类型和聚合值。
- 已确认缺口：
  - `CalleeSuspendState` 仍只有 `resume_word`
  - top-level / closure resume path 仍直接按标量恢复值解码
  - 间接 perform 的 escape continuation step 只把 `resume_word` 写回 callee state
- 已实施修复方向：
  - 统一 `CalleeSuspendState` 为 `resume_word + resume_gc_ref + locals...`
  - top-level / closure resume path 复用统一 `decode_abi_payload_transport`
  - 间接 perform step 把双通道 payload 写回 callee state，并对 GC ref 写槽走写屏障

## 进度更新规则

- 在开始关键阶段前更新本文件
- 如果计划发生变化，及时补充原因
- 在完成实现、测试、文档更新、提交后分别记录结果
