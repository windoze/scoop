# 当前轮执行记录

## 说明

应要求先记录计划。这里写入的是可公开的决策摘要、执行步骤和进度日志，便于随时检查；不包含逐字逐句的内部推理。

## 初始目标

完成 `TODO.md` 中第一个未完成任务，并在完成后停止。本轮开始前还需要先检查最近一次提交是否提到任何既有问题；如果有，这些问题优先进入本轮范围并先修复。

## 执行计划

1. 检查最近一次提交信息与改动摘要，确认是否存在提交中明确提到但尚未解决的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`、相关代码与测试，确认任务边界、依赖关系和是否存在阻塞项。
4. 如果任务过大，先把任务拆分为更小的子任务，并同步更新 `TODO.md` 与 `PLAN.md`，随后仅执行拆分后的第一个子任务。
5. 实现当前目标任务，必要时同时修复执行过程中发现且阻塞规格正确性的既有缺陷。
6. 运行相关验证：
   - 先跑与修改范围直接相关的测试；
   - 再根据需要跑更高层级的测试或检查；
   - 最后执行格式化、`clippy` 或其他必要质量检查，确保没有新警告。
7. 更新文档与任务状态：
   - 在 `TODO.md` 标记当前任务完成；
   - 在 `PLAN.md` 记录当前状态、调整和后续顺序；
   - 持续更新本文件的进度日志。
8. 使用清晰的提交信息创建一次 Git 提交。
9. 停止，不继续处理下一个任务。

## 进度日志

- 已创建本文件并写入初始计划。
- 已检查最近一次提交：提交信息未显式提到尚未修复的既有问题，因此不需要在任务开始前额外插入“修提交备注问题”的修复项。
- 已读取 `TODO.md` / `PLAN.md`，确认当前最前面的未完成任务是 `T2003c0c2b3b`。
- 已审计 `crates/scoopc/src/llvm/codegen/effect.rs`：
  - `scan_mixed_escape_direct_sites` 与 `MixedEscapeDirectFrame` 已能识别 nested block / if / while direct site，并已有路径合法性 helper。
  - `codegen_handle_expr_escape_with_nonresuming_siblings_direct` 目前只接受 `resume_path.is_empty()`，也就是仅支持 top-level direct sites。
  - mixed-arm site-matrix 已有 block / if / while 的 replay helper，可为后续 no-immediate direct 扩展提供参考。
- 计划调整：
  - 原 `T2003c0c2b3b` 同时覆盖 block / if / while 三类 nested direct replay，跨度过大，准备先拆成三个子任务：
    1. `T2003c0c2b3b1`：nested block direct sites；
    2. `T2003c0c2b3b2`：if branch direct sites；
    3. `T2003c0c2b3b3`：while body direct sites。
  - 接下来先更新 `TODO.md` / `PLAN.md` 完成拆分，再执行第一个子任务 `T2003c0c2b3b1`。
- 已完成计划拆分：
  - `TODO.md` 已把 `T2003c0c2b3b` 替换为 `T2003c0c2b3b1` / `b2` / `b3`。
  - `PLAN.md` 已同步记录拆分原因，并把当前轮目标切换到 `T2003c0c2b3b1`。
- 已完成实现：
  - `codegen_handle_expr_escape_with_nonresuming_siblings` 现已把“无 immediate-resume + direct-only”的 multi-arm 组合分流到 no-immediate direct lowering。
  - no-immediate direct lowering 现已接受 statement-position nested block direct site，并在初次执行与 continuation step 中复用 nested block prefix / tail replay helper。
  - body-lift 分析已改为递归收集 nested block 声明，确保 suspension 前的 block locals 在 replay 后继续使用时可正确 capture / restore。
- 已新增回归：
  - run-pass：`effect_multi_escape_custom_nonresuming_direct_block_multi`
  - build-fail：`effect_multi_escape_direct_if_is_error`
- 已完成验收：
  - 定向 LLVM fixture 根目录验证通过。
  - `cargo fmt --all` 通过。
  - `cargo test --all` 通过。
  - `cargo run -p scoop -- test` 通过。
  - `cargo run -p scoop --features llvm -- test` 通过。
  - `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- 已完成文档状态同步：
  - `TODO.md` 已将 `T2003c0c2b3b1` 标记为完成并写入完成说明。
  - `PLAN.md` 已记录本轮实现结果，并将下一步更新为 `T2003c0c2b3b2`。
- 当前待完成的最后步骤：
  1. 查看最终 diff。
  2. 创建本轮提交。
  3. 停止，等待下一轮调用。
