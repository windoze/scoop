## 当前执行计划

说明：这里记录的是可审阅的执行计划与进度，不包含内部推理细节。

1. 读取 `TODO.md`，确认它作为索引列出的详细任务文件与顺序。
2. 按索引顺序检查对应的 `TODO-Px.md`，定位第一个未完成的详细任务。
3. 如有必要，检查最近提交是否直接提到与该任务相关且未完成的问题；若该问题阻塞当前任务，则先按要求在详细 TODO 中补充前置任务并同步索引。
4. 阅读当前任务涉及的实现、约束、依赖与验证要求，确认需要修改的代码、测试与文档位置。
5. 实现当前任务，保持变更尽量小且符合既有结构；如遇到真正阻塞该任务的缺失能力或规格偏差，则新增最小前置任务并停止在该处。
6. 运行与当前任务直接相关的验证：至少包括针对性测试，并视需要运行更广的测试、格式化、`clippy` 等检查，修复发现的问题。
7. 更新任务记录：在对应 `TODO-Px.md` 标记完成；若任务索引、顺序或标题发生变化，则同步更新 `TODO.md`；仅在阶段计划发生变化时更新 `PLAN.md`。
8. 将本次关键进展补记到本文件。
9. 按仓库约定创建一次 git 提交，然后停止，不继续下一个任务。

## 进度记录

- 已创建计划文件，下一步开始读取 `TODO.md` 与对应详细任务文件，定位首个未完成任务。
- 已读取 `TODO.md` 与 `TODO-P0.md`，确认首个未完成详细任务为 `P0-T04`（建立 P0 baseline parity 验证矩阵）。
- 已检查最近一次提交：`[P0-T03R] Review effect refactor boundary inventory`，未发现提交信息中存在需要先处理的、与 `P0-T04` 直接相关的未完成事项。
- 下一步：阅读当前 CLI / dispatcher / 测试实现，确定复用哪套测试入口来落地自动化 parity 验证。
- 已完成实现：
  - 为 `dump-ast` / `dump-hir` / `dump-mir` / `dump-ir` 抽出可复用的字符串渲染辅助函数，避免 parity 测试只能走黑盒 stdout 捕获。
  - 新增 `crates/scoop/src/commands/parity.rs`，覆盖 AST / HIR / MIR / IR 四类样本的 legacy/refactor 自动化 parity 验证。
  - `dump-ir` 额外提供仅供测试使用的稳定化 parity 视图：保留 materialized file 与 instance family/summary 投影，规避原始 `Debug` 中 `TypeStore` 和 hash-backed side table 的跨进程不稳定顺序。
- 已完成验证：
  - `cargo test -p scoop --no-default-features parity`
  - `cargo test -p scoop --no-default-features cli`
  - `cargo test -p scoopc --no-default-features session`
  - `cargo test -p scoop build_emit_llvm_cli_parity_matches_legacy_and_refactor`
  - `cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`
  - CLI smoke：`dump-ast`、`dump-hir`、`dump-mir` 已人工比较 legacy/refactor 一致；`dump-ir` 已人工确认两种 mode 均可成功运行，具体输出一致性由自动化正规化 parity 测试覆盖。
- 已把 `P0-T04` 完成记录回写到 `TODO-P0.md`；`TODO.md` 与 `PLAN.md` 无需同步，因为任务 id / 顺序 / phase plan 未变化。
- 下一步：检查工作区 diff，按仓库约定创建 `P0-T04` 提交，然后停止本轮执行。
