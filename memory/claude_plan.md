## 执行计划

说明：我不会写入私有推理细节，但会在这里持续维护可审阅的执行计划、决策依据摘要、进度和阻塞信息。

1. 读取 `TODO.md`，定位第一个标题未标记 `[DONE]` 的任务。
2. 检查最近提交是否直接提到与该任务相关的未完成问题；如果有且它构成当前任务的直接前提，则把它作为当前任务范围的一部分，或在 `TODO.md` 中补充为前置任务。
3. 阅读当前任务在 `TODO.md` 中的详细要求、依赖、验收和完成记录，并只围绕该任务展开工作，不做开放式问题扫查。
4. 检查相关代码、测试、夹具和文档，确认需要修改的最小范围。
5. 实现当前任务；如果遇到阻塞当前任务的真实缺陷或缺失能力，不采用绕过方案，而是在 `TODO.md` 中插入最小必要前置任务并停止。
6. 运行任务要求的验证，以及必要的构建、测试和 `cargo clippy --all-targets -- -D warnings`；修复由本次任务暴露的相关问题。
7. 更新 `memory/claude_plan.md` 记录关键进展或计划调整。
8. 若任务完成，则在 `TODO.md` 中将任务标题前缀改为 `[DONE]`，补全完成记录；仅在阶段级计划变化时更新 `PLAN.md`。
9. 按要求创建一次 git 提交，提交信息使用当前任务编号，随后停止，不继续下一个任务。

## 当前状态

- 状态：已读取 `TODO.md`，首个未完成任务已确认。
- 当前任务：`P7-T01R Review 全量收口结果，确认 stable-id 方案已闭合且未带来功能漂移`。
- 相关提交：最近一次提交为 `[P7-T01] Close final stable-id audit matrix`；提交信息中未显式记录新的未完成前置问题。
- 阻塞：无。

## 本轮执行细化

1. 复核 `PLAN.md` §5、§6 与 `STABLE_ID.md` §10、§11、§12，提炼 review 签收清单。
2. 先做静态预检查，确认是否还存在直接违反 `STABLE_ID.md` 强制规则的 active production 路径；若存在，则不继续跑全量复核矩阵，而是先把 blocker 写回 `TODO.md`。
3. 若静态预检查无 blocker，再重跑 `P7-T01` 的关键验证矩阵，覆盖：
   - checkout-path 稳定性
   - distinct virtual cones / symbol 冲突防护
   - grep 审计清单
   - `scoop` build fixture / run-pass fixture / spec-fixtures
   - `scoopc`、`scoop_runtime`、workspace 全量测试
   - `cargo clippy --all-targets -- -D warnings`
4. 若验证全部通过，则仅更新 `TODO.md`：
   - 将 `P7-T01R` 标为 `[DONE]`
   - 在完成记录中逐项签收 `PLAN.md` §6 的 8 条完成标准
5. 若验证或静态预检查中发现当前任务范围内的真实缺口：
   - 不做绕过
   - 视是否需要插入前置任务到 `TODO.md`
   - 记录阻塞后提交并停止
6. 最后检查工作区、提交本轮变更并停止。

## 本轮发现

- 已确认 blocker：`crates/scoopc/src/llvm/codegen/object_init.rs` 与 `crates/scoopc/src/llvm/codegen/mod.rs` 仍在 production 路径中使用 legacy object/top-level init private names（如 `__scoop_object_init__...`、`__scoop_top_level_val_init__...`、`__scoop_object_guard__...`、`__scoop_top_level_val__...`）。
- 该问题与 `STABLE_ID.md` §5.1 第 5/7 条、§7.3、§8.5 冲突：compiler-private LLVM function/global/type 名称应统一走 `PrivateSymbolMangler`，`sanitize_llvm_ident()` 只能作为可读前缀，不能继续让旧 raw prefix 充当最终命名。
- 处理决定：不继续执行 `P7-T01R` 的最终签收；已在 `TODO.md` 中插入新的前置任务 `P7-T01A`，等待后续调用先修复该缺口。
