# 执行计划

> 说明：这里记录可检查的执行计划、关键判断和进度更新；不记录私有逐字推理链。

## 初始计划

1. 读取 `TODO.md`，按文件顺序找到第一个标题未带 `[DONE]` 的任务。
2. 如需要，查看最新提交是否明确提到与该任务直接相关的未完成问题；只处理会阻塞当前任务的问题。
3. 阅读当前任务涉及的代码、测试和规格上下文，确认完成条件与验证命令。
4. 以最小正确改动实现当前任务；若发现阻塞性的缺失功能或规格不匹配，则在 `TODO.md` 中插入最小必要前置任务并停止。
5. 运行相关测试；必要时运行更广的验证命令，修复当前任务引入或暴露且会阻塞该任务的问题。
6. 更新 `TODO.md`：完成时在任务标题前加 `[DONE]` 并填写完成记录；仅在阶段计划真实变化时更新 `PLAN.md`。
7. 更新本文件记录关键进度。
8. 按要求提交所有本次任务相关更改，提交信息使用任务编号和简明描述。
9. 完成一个任务后停止，不继续下一个任务。

## 进度日志

- 已读取 `TODO.md`，第一个未完成任务是 `P5-T00：补跑 Linux/amd64 LLVM ABI 矩阵并固化可复现执行环境`。
- 下一步核对最新提交、CI 配置和远端/本地可用验证记录；如果当前 CI run 已覆盖任务要求，则回写 `TODO.md` / 相关文档并提交；如果没有可完成的 Linux/amd64 runner 结果，则只记录阻塞和必要的前置调整。
- 已核对最新提交：`10df6e1 [P5-T00] Extend Linux ABI CI runner`，包含 `.github/workflows/ci.yml` 的 Linux ABI targeted matrix 与 Clippy 步骤；远端 CI run `25952815819` 正在执行。
- 当前执行策略：等待该 CI run 完成，检查 `Platform diagnostics`、完整 cargo/fixture、P5 targeted matrix 与 Clippy 步骤；成功后回写 `TODO.md` / `MANAGED_ABI.md`，失败则按真实失败修复或记录新的前置任务。
- CI run `25952815819` 已成功：`ubuntu-24.04` / `x86_64` / Rust `1.95.0` / LLVM `21.1.8`；完整 Rust suite、fixture smoke、P5 ABI targeted matrix 与 `cargo clippy --all-targets -- -D warnings` 均通过。
- 已把 `P5-T00` 标记为 `[DONE]`，并更新 `TODO.md`、`MANAGED_ABI.md`、`PLAN.md` 记录 Linux/amd64 runner、命令日志 URL、平台诊断与验证结果。
- 已运行 `git diff --check`，无 whitespace error。
- 已提交完成记录：`a1b3bc46 [P5-T00] Record Linux ABI matrix completion`。
- 当前任务已完成；按要求停止，不继续 `P5-T01`。
