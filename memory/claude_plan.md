## 当前执行计划

说明：出于安全与协作边界，这里记录可执行计划、关键判断依据与进度，不记录不可验证的私有推理细节。

1. 先读取 `TODO.md`，确认第一个标题未带 `[DONE]` 的任务；该任务是本次唯一执行目标。
2. 检查最近一次提交信息，判断是否存在与该任务直接相关且明确未完成的问题；若存在，则将其视为当前任务的一部分或在 `TODO.md` 中补成前置依赖。
3. 阅读当前任务在 `TODO.md` 中的完整要求、依赖、验证标准，再只针对该任务所涉及的代码与文档做最小范围检查。
4. 实现任务；若遇到阻塞当前任务且不能绕过的规格缺口或缺陷，则在 `TODO.md` 中增加最小必要前置任务，保持当前任务未完成，并停止继续推进后续任务。
5. 运行与该任务相关的验证；至少覆盖任务要求中的指定验证，并补充必要的回归验证。若本次修改触及通用构建/质量门槛，则执行相应检查。
6. 更新 `memory/claude_plan.md` 记录关键进展与计划变化。
7. 若任务完成：在 `TODO.md` 中将该任务标题改为 `[DONE]`，补全完成记录；仅在阶段计划发生变化时更新 `PLAN.md`。
8. 按要求提交本次变更，提交信息以任务号为前缀，然后停止，不继续下一个任务。

## 当前任务

- 已确认第一个未完成任务：`P6-T01：统一 RTTI / interface hash helper，并修复 closure env identity 来源`。
- 最近一次提交：`[P5-T02R] Review dump and fixture migration`。
- 判断：最近提交没有明确提到与 `P6-T01` 直接相关且仍未完成的问题，因此不额外插入前置任务，先按 `P6-T01` 原任务执行。

## 针对 P6-T01 的执行计划

1. 阅读 `TODO.md` 中 `P6-T01` 条目对应的要求，并补充读取 `PLAN.md` / `STABLE_ID.md` 的 P6 / RTTI / closure env 相关段落，确认验收边界。
2. 定位并阅读以下实现与测试入口：
   - `crates/scoopc/src/rtti/type_desc.rs`
   - `crates/scoopc/src/rtti/mod.rs`
   - `crates/scoopc/src/itable.rs`
   - 必要时补充 `stable_id`、closure stable key、`dump_rtti_*` 相关测试与调用链。
3. 确认 closure env 当前 canonical name / `type_id` 是否仍由 `ClosureId` 或 `scoop.lambda_env$` 族名驱动；同时梳理 RTTI / interface hash helper 是否仍存在分叉输入前缀或局部实现。
4. 以最小但完整的改动完成：
   - RTTI / interface / type identity 收口到共享 stable-id helper；
   - closure env canonical name 改为基于 `StableClosureKey` 的 authoritative 来源；
   - 保持 `.cone` / JSON 健康 schema 仅做审计，不做结构重写。
5. 补齐或更新定向测试，覆盖：
   - `dump-rtti` / closure env identity
   - RTTI / interface hash helper 统一入口
   - 必要的 source inventory / grep 防回流
6. 运行任务要求的验证，至少包括 `cargo test -p scoopc`、相关 RTTI 定向测试，以及 `clippy -D warnings`。
7. 若任务完成：
   - 更新 `TODO.md`：将 `P6-T01` 标题改为 `[DONE]` 并填写完成记录；
   - 如 phase 计划未变，不改 `PLAN.md`；
   - 提交本次改动并停止。
8. 若发现无法按现有任务直接完成的真实阻塞：
   - 在 `TODO.md` 中插入最小前置任务；
   - 记录 blocker；
   - 保持 `P6-T01` 未完成并提交后停止。

## 进度记录

- 已创建计划文件。
- 已读取 `TODO.md` 并识别第一个未完成任务为 `P6-T01`。
- 已检查最近一次提交信息；未发现需要先插入的直接相关 unfinished issue。
- 已读取 `PLAN.md` / `STABLE_ID.md` 的相关段落，并确认 `P6-T01` 的边界：closure env 必须改为 `StableClosureKey -> canonical name -> shared RTTI hash helper`，RTTI / interface/runtime-match id 需要统一 helper。
- 已完成中的关键实现：
  - 抽出共享 HIR closure lexical-path helper，避免 LLVM 与 RTTI 各自维护一套 `$lambdaN` 路径恢复逻辑。
  - `dump-rtti` closure env 名字与 `type_id` 已切到 `StableClosureKey::env_canonical_name()` + shared RTTI helper。
  - RTTI / interface/runtime-match 相关 `stable_hash64(RttiV0, ...)` 生产代码调用点，已开始统一替换为 shared helper（覆盖 `rtti` / `itable` / LLVM sibling case）。
- 已完成验证：
  - `cargo fmt`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc dump_rtti -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc path_free -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
  - 精确搜索：`fn stable_hash64` 仅剩 `crates/scoopc/src/stable_id.rs`；`ClosureId|scoop.lambda_env$` 在 `crates/scoopc/src/rtti`、`itable.rs`、`llvm/codegen/gc.rs`、`llvm/codegen/mir_body.rs` 中为 0 命中。
- 待执行：更新 `TODO.md` 的完成记录，复查工作区后提交本次任务并停止。
