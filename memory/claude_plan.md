# Claude 执行计划

## 范围

- 以 `TODO.md` 作为权威任务列表和完成状态来源。
- 本轮只完成第一个标题未带 `[DONE]` 的任务：`TC-04-R：Review TC-04`，完成后停止。
- `TC-04-R` 是审查任务；除非审查发现必须修复的阻塞问题，否则不改实现代码。
- 除非阶段级计划或依赖关系实际变化，否则不更新 `PLAN.md`。

## 当前任务

- 已选择第一个未完成任务：`TC-04-R：Review TC-04`。
- 初始依赖状态：`TC-04-FIX1` 和 `TC-04-FIX2` 在 `TODO.md` 中均已标记 `[DONE]`，因此本轮应重新执行 TC-04 的最终审查确认。
- 审查目标：确认 LLVM codegen 中 callee、符号、布局等 live callable 引用已改为句柄/contract/hash；FQN 仅保留为 LLVM 符号名、诊断、source-signature 文本字段或非 callable 的 nominal/global layout key；不存在 FQN 查找 fallback、句柄转 FQN 再查、LIR→MIR shim。

## 执行步骤

1. 检查最新提交和工作区状态，确认是否有与 `TC-04-R` 直接相关的未完成事项或已有未提交变更。
2. 读取 `TC-04`、`TC-04-FIX1`、`TC-04-FIX2` 的相关完成记录，以及必要的 codegen 文件，建立审查上下文。
3. 运行 `TC-04-R` 指定的静态 grep/rg 验收：旧 root/FQN callable lookup helper、carrier/dispatch target string key、`lir_*_to_mir`、`_fqn` 残留抽样。
4. 对 `_fqn` 残留逐类核对语义，确认它们不是 live callable 查找或 fallback 路径。
5. 若发现真实阻塞缺口，按要求在 `TODO.md` 增加最小前置修复任务，保持 `TC-04-R` 未完成，提交后停止。
6. 若审查通过，按要求运行验证：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`。
7. 审查和验证均通过后，在 `TODO.md` 中将 `TC-04-R` 标题改为 `[DONE]`，更新完成记录。
8. 更新本文件记录关键进度和验证结果。
9. 检查最终差异，提交本轮任务变更，提交信息使用 `TC-04-R` 前缀。

## 进度

- 已读取 `TODO.md` 并确认第一个未完成任务为 `TC-04-R`。
- 已检查工作区状态和最新提交：最新提交为 `41ae9210 [TC-04-FIX2] Remove carrier FQN target selection`，与当前 review 的前置修复直接相关；初始工作区仅有本计划文件变更。
- 已执行 `TC-04-R` / `TC-04-FIX2` 指定的旧 helper、carrier target、physical FQN target、`program.callable`、`lir_*_to_mir` 等静态 grep，均无命中。
- 广义 `_fqn` 抽样和 root-equality 搜索发现新的 review 阻塞：source callable、direct-call ABI/signature 和 layout 查询仍有生产路径按 callable root/FQN 扫描或选择 live callable facts/layout。
- 已在 `TODO.md` 中新增最小前置任务 `TC-04-FIX3：清除 source-callable/direct-call 残留 FQN live lookup`，并把 `TC-04-R` 依赖更新为 `TC-04-FIX1`、`TC-04-FIX2`、`TC-04-FIX3`。
- 按阻塞处理规则，本轮不标记 `TC-04-R` 完成；提交 TODO/计划变更后停止。

## 验证记录

- 已执行静态审查命令：旧 helper / carrier target / physical FQN target / legacy lookup / LIR-to-MIR shim grep 无命中。
- 未运行 `cargo fmt`、`cargo clippy`、Rust 测试或 fixture suite；本轮只登记 review 阻塞和计划记录，无实现代码变更。
