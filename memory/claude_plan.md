## 本次执行计划

说明：这里记录可公开的任务执行计划与进度摘要，不包含逐字内部推理。

1. 读取 `TODO.md`，严格按标题是否带有 `[DONE]` 判断完成状态，定位第一个未完成任务。
2. 检查最近一次提交是否直接提到与该任务相关且未完成的问题；若是，则将其视为当前任务的一部分或其前置条件。
3. 阅读与当前任务直接相关的代码、测试、规范与任务说明，确认依赖、约束、验收标准，以及是否存在阻塞问题。
4. 若任务可直接完成：实施代码修改，补充或更新测试，并进行必要验证（至少覆盖任务要求与相关回归）。
5. 若发现阻塞当前任务的真实缺陷、缺失特性或规范不匹配：
   - 不做规避性实现；
   - 在 `TODO.md` 中插入最小必要前置任务并调整顺序/依赖；
   - 仅在阶段计划确实变化时更新 `PLAN.md`；
   - 提交这些变更后停止。
6. 任务完成后：
   - 更新 `memory/claude_plan.md` 记录关键进度与最终结果；
   - 在 `TODO.md` 中把该任务标题标记为 `[DONE]`，并补充完成记录；
   - 若需要，更新 `PLAN.md`；
   - 运行格式化、测试与 `cargo clippy --all-targets -- -D warnings` 等相关验证；
   - 按要求创建一次 git 提交；
   - 停止，不继续下一个任务。

## 进度记录

- 已创建本计划文件。
- 已读取 `TODO.md`，当前首个未完成任务为 `P6-T01R：Review RTTI 与 JSON 收口，确认剩余外部 surface 已只需最终验收`。
- 已查看最近一次提交摘要：`[P6-T01] Unify RTTI helpers and closure env identity`，与当前 review 任务直接相关；下一步需要检查提交正文与当前工作树，确认是否存在必须并入本任务的未完成事项。

## 当前任务细化步骤（P6-T01R）

1. 读取 `P6-T01R`、`PLAN.md` P6、`STABLE_ID.md` 中 RTTI / JSON / shared hash helper 相关要求，整理本次 review 的验收清单。
2. 检查最近一次提交正文与当前工作树状态，确认是否存在直接相关但未完成的问题；若存在且会阻塞 `P6-T01R`，按要求先转化为前置任务或直接修复。
3. 代码复核重点：
   - `closure env canonical name / type_id` 是否完全脱离 `ClosureId`；
   - RTTI / interface id 是否统一复用 shared helper；
   - `.cone` / JSON 基线是否仅做防回归、没有无谓结构 churn；
   - 是否还残留 RTTI identity path 上的旧 helper / 旧字符串来源。
4. 运行验证：
   - `cargo test -p scoopc dump_rtti -- --nocapture`
   - `cargo test -p scoopc path_free -- --nocapture`
   - `cargo test -p scoopc`
   - `cargo clippy -p scoopc --all-targets -- -D warnings`
   - 必要的精确搜索（如 `ClosureId`、`scoop.lambda_env$`、`stable_hash64`、shared helper 使用点）
5. 若 review 发现真实缺陷：优先修复；若无法在本次内闭合，则在 `TODO.md` 插入最小前置任务并停止。
6. 若 review 通过：
   - 在 `TODO.md` 将 `P6-T01R` 标记为 `[DONE]` 并补全完成记录，明确哪些 JSON / cache surface 已证明无需再改；
   - 仅在阶段计划变化时更新 `PLAN.md`；
   - 更新本计划文件的结果摘要；
   - 提交本次变更并停止。

## 当前结果摘要

- 最近一次提交正文未声明与 `P6-T01R` 直接相关的未完成问题；当前工作树在执行前仅有本计划文件变更，无外部阻塞项。
- 代码复核结果：
  - `crates/scoopc/src/rtti/type_desc.rs` 的 closure env RTTI 已通过 `StableClosureKey::env_canonical_name()` + `stable_rtti_type_id(...)` 生成；未发现继续把 `ClosureId` 或 `scoop.lambda_env$<id>` 用作 RTTI identity 输入的生产代码路径。
  - `rtti/mod.rs`、`rtti/type_desc.rs`、`itable.rs`、`llvm/codegen/gc.rs`、`llvm/codegen/mir_body.rs`、`llvm/codegen/mod.rs` 中的 RTTI / interface / runtime-match id 入口均已收口到 `stable_rtti_type_id(...)` / `stable_rtti_interface_id(...)`。
  - `.cone` / JSON 健康基线继续保持“防回归而非重写”：`api.scoopir`、`PRE_SPECIALIZE.json`、`SYMBOL_VISIBILITY.json`、`ANNOTATION_CLASSES.json` 无需在 P6 再做结构改写。
- 验证已完成并通过：
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc dump_rtti -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc path_free -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
- 精确搜索摘要：
  - `ClosureId` 在 `crates/scoopc/src/rtti`、`crates/scoopc/src/itable.rs`、`crates/scoopc/src/llvm/codegen/gc.rs`、`crates/scoopc/src/llvm/codegen/mir_body.rs` 中均为 0 命中。
  - `scoop.lambda_env$` 在生产代码中为 0 命中；仅 `rtti/type_desc.rs` 中保留 1 处负向测试断言。
  - `StableHashScope::RttiV0` 只剩 `crates/scoopc/src/stable_id.rs` 中的 shared helper 与单测命中。
- 已更新 `TODO.md`：`P6-T01R` 已标记为 `[DONE]`，并写入 review 结论与验证记录。
- `PLAN.md` 无需更新。下一步仅剩提交本次变更并停止。
