# 执行记录与计划

说明：本文件记录可执行计划、关键决策、进度与变更；不记录冗长的内部推理草稿。

## 初始计划
1. 检查最新一次 Git 提交，确认提交信息中是否提到已知问题、遗留修复或必须先处理的事项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 结合代码库现状评估该任务复杂度：
   - 如果任务足够清晰且可在本轮完整交付，直接实现。
   - 如果任务过大或前置条件不满足，则拆分为更小子任务，更新 `PLAN.md` 与 `TODO.md`，并执行拆分后的第一个子任务。
4. 在实现前阅读相关代码、测试、文档与计划文件，避免与现有结构冲突。
5. 完成实现后运行相关测试；如果任务影响范围较大，补充运行更广泛的校验，包括格式化、测试、`clippy` 零警告检查。
6. 更新文档与计划记录：
   - 在 `TODO.md` 中标记当前任务完成，或在阻塞情况下重新排序并注明依赖。
   - 在 `PLAN.md` 中记录当前状态、拆分结果、风险与下一步。
   - 视需要补充 `README.md` 或代码注释。
7. 使用清晰的提交信息提交本轮变更，然后停止，不继续处理下一个任务。

## 进度日志
- 已创建初始计划，等待开始仓库检查。
- 已检查最新提交 `b36ddc46da9b149612aee136cfa86018941859b1`（`[T2003c0b2c2a] Support mixed-arm while-body flat escape sites`），提交信息未额外声明必须先修的遗留问题。
- 已读取 `TODO.md` / `PLAN.md` / `README.md`，确认当前第一个未完成任务为 `T2003c0b2c2b`：`Effect：LLVM 多 arm handle dispatch（sibling escape-continuation，while body 的 nested direct site）`。
- 当前执行重点：
  1. 阅读 `crates/scoopc/src/llvm/codegen/effect.rs` 中 mixed-arm while-body 相关 lowering。
  2. 阅读现有 `tests/fixtures/run-pass/effect_resume_mixed_escape_*while*.scoop` 与失败夹具，确认当前仅支持 flat site 的边界。
  3. 判断 `T2003c0b2c2b` 是否可直接在本轮完成；若范围仍过大，则按要求继续拆分并更新 `PLAN.md` / `TODO.md`。
- 已确认工作区存在与当前任务直接相关的未提交改动：`crates/scoopc/src/llvm/codegen/effect.rs`、两个新的 while-nested run-pass fixtures，以及文档/注释更新。当前策略是先审读这些现有改动，判断其完成度，再在其基础上补齐实现与验证，不回退已有工作。

## 当前实现方案
结论：`T2003c0b2c2b` 当前范围可在本轮直接完成，不再继续拆分。

计划中的代码改动：
1. 调整 mixed-arm direct-site 扫描后的校验逻辑：
   - 去掉“while body 中只要 `resume_path.len() > 1` 就统一拒绝”的门禁。
   - 把本轮支持范围收口为：`while` 外层 + 单个 nested `block` 或 `if` direct site。
   - 对更深层形状继续给出稳定诊断，避免半支持状态。
2. 扩展 while direct-site lowering：
   - 让 `codegen_mixed_escape_matrix_while_stmt_direct_site` 支持命中 while body 内的 nested `block` / `if` 路径。
   - 让 `codegen_mixed_escape_matrix_while_tail_after_site` 在 `resume(...)` 后先 replay 命中的 nested path 尾部，再执行当前迭代余下语句、loop condition 与后续迭代。
3. 视需要抽取小型 helper，减少 while 初次命中与 tail replay 的重复逻辑。
4. 更新 fixtures：
   - 新增 while body nested `block` / nested `if` 的 run-pass 回归。
   - 把现有 while 负例改成“更深层 nested 形状仍不支持”的稳定诊断。
5. 验证：
   - `cargo fmt --all`
   - `cargo test --all`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo run -p scoop -- test`
   - `cargo run -p scoop --features llvm -- test`

## 本轮补充计划
1. 审读现有未提交 diff，确认 `effect.rs` 与新增 fixtures 已经覆盖了哪些 while-nested 形状。
2. 若实现只差收口或修补，则直接补齐代码与 diagnostics；若仍存在未覆盖的 nested replay 路径，再最小化扩展 helper。
3. 用新增/现有 fixtures 先做 LLVM 定向验证，再决定是否补跑更大范围测试。
4. 完成后更新 `TODO.md` / `PLAN.md` 的完成说明，并保持 `T2004` 继续留作后续前端任务。

## 本轮结果
- 已确认并保留现有 `effect.rs` 草稿实现，在其基础上完成 while-body nested direct site 支持：
  - 允许的最小路径为 `while -> block -> block*` 与 `while -> if-branch -> block*`；
  - `resume(...)` 后会先 replay nested path 尾部，再继续 while 当前迭代尾部与 loop re-entry；
  - 更深层 `while -> block -> if` / nested while 继续报稳定诊断。
- 已验证新增 run-pass：
  - `effect_resume_mixed_escape_pre_immediate_while_nested_block`
  - `effect_resume_mixed_escape_post_immediate_while_nested_if`
- 已验证 build-fail：
  - `effect_resume_mixed_escape_while_is_error`
- 全量验证通过：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
- 已更新 `TODO.md` / `PLAN.md`：`T2003c0b2c2b` 标记完成，下一步推进到 `T2003c0b2c3`；`T2004` 保留为后续前端裸 block 语法任务。
