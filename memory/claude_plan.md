# 当前执行计划

说明：本文件记录可审查的执行计划、关键决策和进度更新；不记录逐字内部思维链。

## 初始计划

1. 读取 `TODO.md`，按文件顺序识别第一个标题未带 `[DONE]` 的任务。
2. 检查该任务的具体要求、依赖、验证条件和完成记录；只处理该任务，不做开放式历史问题扫描。
3. 如最新提交明确提到与该任务直接相关的未完成问题，则纳入当前任务范围或作为必要前置项记录到 `TODO.md`。
4. 阅读与该任务相关的最小代码、测试、规格或夹具上下文。
5. 按任务要求实施最小正确修改，避免绕过规格或降低测试覆盖。
6. 运行相关测试和必要的质量检查；若失败，修复由当前任务引入或阻塞当前任务的问题。
7. 更新 `TODO.md`：将完成的任务标题加 `[DONE]`，并补充完成记录；仅在阶段计划真实变化时更新 `PLAN.md`。
8. 再次更新本文件，记录完成情况、测试结果和任何偏差。
9. 按要求提交所有本次任务相关变更，然后停止，不进入下一任务。

## 当前状态

- 已建立初始执行计划。
- 已读取 `TODO.md`，第一个未完成任务为 `C2-T01B：删除 MIR lowering 中的隐式 CaptureBox 生成与读写`。
- 已检查最新提交 `e9ed994d [C2-T01A] Remove MIR CaptureBox core model`，没有记录与 `C2-T01B` 直接相关的未完成问题。
- 已确认 `crates/scoopc/src/mir/lower` 中不再有 `boxed_symbols`、`CaptureBox*` 或 `__CaptureBox` 残留；更新了 HIR capture mutability 注释，删除旧 box/alias 语义表述。
- 已完成验证：`cargo build -p scoopc` 通过；CaptureBox lowering 残留搜索无输出；两个定向 Rust 测试通过；`cargo clippy -p scoopc --all-targets -- -D warnings` 通过。
- 已将 `TODO.md` 中 `C2-T01B` 标记为 `[DONE]` 并填写完成记录；未更新 `PLAN.md`，因为阶段级计划未变化。

## C2-T01B 执行计划

1. 检查最新提交是否包含与 `C2-T01B` 直接相关的未完成问题。
2. 阅读 `C2-T01B` 涉及的 MIR lowering 与 HIR capture 注释位置，确认当前代码是否仍有 `boxed_symbols` / `CaptureBox` lowering 残留。
3. 删除或整理 MIR lowering 中所有隐式 CaptureBox 生成、读取、写入与相关 helper；保留普通 closure env transport 和 `ClosureCaptureTransportMetadata.mutable`。
4. 更新 HIR `Capture.mutable` 注释，明确该字段用于 closure body per-call local mutability。
5. 运行任务指定验证：`cargo build -p scoopc`、相关 `rg` 搜索、两个定向 Rust 测试；必要时运行更窄的补充测试。
6. 更新 `TODO.md` 中 `C2-T01B` 标题为 `[DONE]` 并填写完成记录；如阶段计划未变化，不更新 `PLAN.md`。
7. 最后提交本任务相关所有变更并停止。
