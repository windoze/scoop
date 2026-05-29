# 当前执行计划

## 约束说明

- 该文件记录可检查的执行计划、关键决策和进度更新。
- 不记录逐字内部推理链；仅记录任务选择依据、计划步骤、验证结果和阻塞事项。
- 本次调用只完成 `TODO.md` 中第一个未标记 `[DONE]` 的任务，然后提交并停止。

## 初始计划

1. 阅读 `TODO.md`，按文档顺序识别第一个标题未带 `[DONE]` 的任务。
2. 检查最近提交是否明确提到与该任务直接相关的未完成事项；如相关，将其纳入当前任务或作为前置任务写入 `TODO.md`。
3. 阅读该任务涉及的规格、源码、测试或 fixture，确认验收条件和依赖。
4. 实施最小正确修改；若遇到阻塞性缺失功能或规格不匹配，不绕过，改为在 `TODO.md` 插入最小前置任务并停止。
5. 按要求运行格式化、lint、相关测试；若代码变更影响编译输出，则依次运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、完整 Rust 测试和 fixture 套件，必要时修复失败。
6. 更新 `TODO.md`：在已完成任务标题前加 `[DONE]`，补充完成记录和验证记录。仅当阶段计划实际变化时更新 `PLAN.md`。
7. 检查 git 状态和 diff，提交本次任务相关的全部变更。
8. 停止，不继续处理下一个任务。

## 当前状态

- 计划已初始化。
- 已读取 `TODO.md`。
- 当前第一个未完成任务：`P5-T04R`，目标是 review `P5-T04` 的 selected callable identity 贯通。
- 已读取 `TODO-5.md` 中 `P5-T04` / `P5-T04R` 详情。
- 最近提交 `75ada465 [P5-T04] Thread selected callable identity` 直接对应本 review，没有发现需先另行登记的无关历史事项。
- Review 计划：检查 P5-T04 修改的 HIR call binding、HIR lowering、MIR materialization、MIR lowering、LLVM call codegen，以及三个 overload run-pass baseline fixture 是否启用并通过。
- 初步 review 发现需修复的直接缺口：
  1. HIR lowering 汇总 synthetic named intrinsic call sites 时可能用 bare-FQN 生成的 synthetic binding 覆盖 typecheck 写入的 selected binding。
  2. LLVM direct-call codegen 在 exact LIR callable/signature 查找失败时仍存在 bare-FQN / ABI-signature 推断 fallback。
- 已修复：synthetic call-site binding 现在只补缺不覆盖 typecheck selected binding；MIR LLVM direct-call codegen 现在要求 exact concrete callable 与 exact signature facts，不再按 bare FQN/ABI signature 推断；HIR stage ABI identity 对 typechecked binding 优先按 selected decl file/span 查找。
- 复核后补充修复：HIR stage ABI identity 的 selected decl lookup 现在接受 typecheck name span 被 HIR declaration span 包含；exact miss 时只允许 same-FQN 唯一候选 fallback，否则报 typed HIR contract error，避免 overloaded FQN 静默回退。
- 已执行：`cargo fmt`；`cargo clippy --all-targets -- -D warnings` 通过。第一次 targeted fixture 多路径调用因脚本只接受单个 positional path 而用法错误，随后逐个运行三个 targeted overload fixtures 均通过；ABI 修复后将重新运行 targeted fixtures。
- ABI 修复后重新运行三个 targeted overload fixtures 均通过。
- 完整 `cargo test --all --all-targets` 发现本次 ABI identity 修复过严：sysroot/intrinsic bindings（如 `scoop.core.Int.plus`、`scoop.core.println`、`scoop.core.mutableArrayNew`、`scoop.core.panic`）不一定存在于当前 lowered HIR 中，直接报 selected declaration missing 会误伤合法代码。
- 已调整 ABI fallback 策略：仅当当前 lowered HIR 内存在同名多候选且 selected decl 仍无法匹配时拒绝 bare-FQN fallback；对当前 HIR 不承载的 sysroot/imported binding 保留既有 ABI 分类。
- 已重新执行：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test -p scoopc --lib`，均通过。
- 已重新执行并通过：完整 `cargo test --all --all-targets`；三个 P5-T04R targeted overload run-pass fixtures；`python3 tools/spec_fixtures.py check`；完整 `python3 tools/run_fixtures.py`。
- 已更新 `TODO.md` / `TODO-5.md`：`P5-T04R` 已标记 `[DONE]` 并补充完成记录。
- 下一步：检查 git status / diff / recent log，提交本轮任务相关变更。
