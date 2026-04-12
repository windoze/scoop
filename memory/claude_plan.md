# 本轮执行计划

## 约束说明

- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后立即停止。
- 在开始实现任务前，先检查最新提交是否提到既有问题；若存在，优先修复。
- 若首个未完成任务过大，则先拆分任务并更新 `PLAN.md` 与 `TODO.md`，随后只执行拆分后的第一个子任务。
- 在执行过程中持续更新本文件，记录关键步骤、计划变更、阻塞和完成状态。

## 初始步骤

1. 查看最新一次 git 提交信息，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解当前项目计划与任务上下文。
4. 如有必要，检查相关代码、测试、规范文档与最近变更，判断该任务是否可直接完成，或是否需要拆分/前置修复。

## 执行策略

1. 若发现最新提交中提到的既有问题，先复现并修复该问题，再继续主任务判定。
2. 若首个未完成任务需要拆分：
   - 更新 `PLAN.md`，写明拆分理由与子任务顺序。
   - 更新 `TODO.md`，将原任务替换或扩展为更细的子任务，并确保依赖顺序正确。
   - 只执行新的第一个子任务。
3. 对当前要执行的任务进行实现，避免规避规范缺口；若发现规范不匹配，必须把修复缺口作为前置任务写入 `TODO.md`/`PLAN.md`，提交后停止。
4. 完成实现后运行相关验证：
   - 至少运行与改动直接相关的测试。
   - 若改动影响通用编译路径，补充运行 `cargo test --all`、`cargo clippy --all-targets -- -D warnings` 或更小但足够覆盖的命令。
5. 更新文档与计划：
   - 在 `TODO.md` 标记该任务完成。
   - 在 `PLAN.md` 反映当前进展与后续顺序。
   - 在本文件补充执行结果与关键决策。
6. 使用清晰的提交信息提交本轮所有变更，然后停止。

## 当前状态

- 已创建本计划文件。
- 已检查最新提交：`ff67b7da4bd526c5f3c4dfc8e44fb71fd3912248`，提交信息为 `Update plan`，未在提交信息中提到需要先修的既有缺陷。
- 已读取 `TODO.md` / `PLAN.md`。
- 已定位首个未完成任务：`T2003c0c2b3d2`，内容为“无 immediate-resume，nested block direct + indirect same-stmt mixed”。

## 当前任务理解

- 现状并不是 parser/typecheck 缺口，而是 LLVM no-immediate mixed lowering 的分流和 continuation step 仍只支持：
  - top-level direct + indirect mixed；
  - direct-only nested block / if / while；
  - indirect-only nested block / if / while。
- 对于“同一个 statement-position nested block 内 direct / indirect 共存”的 mixed 路径，当前仍在入口报：
  - `handle multi-arm without immediate-resume (escape site matrix not yet supported)`
- 已做最小复现：
  - 临时文件：`/tmp/effect_multi_escape_custom_nonresuming_direct_indirect_block_multi.scoop`
  - 复现命令：`cargo run -p scoop --features llvm -- build ...`
  - 结果：稳定复现上述 `unsupported_main_body` 报错。

## 细化执行步骤

1. 修改 `crates/scoopc/src/llvm/codegen/effect/mixed.rs` 的 no-immediate mixed 分流：
   - 保留现有 top-level mixed 路径；
   - 放开并接入“top-level 或 statement-position nested block”这一级 mixed 子集；
   - 继续让 if / while mixed 保持稳定拒绝。
2. 扩展 no-immediate mixed lowering 的状态机：
   - 为 nested block mixed 记录同 statement 的前后 site 关系；
   - 在 initial body 与 continuation step 中接入 block prefix / next-site replay / indirect-site continue helper；
   - 确保 block locals 的 capture / restore 与 top-level tail 继续执行正确。
3. 新增回归 fixtures：
   - run-pass：nested block direct + indirect same-stmt mixed；
   - build：至少一个仍未支持的 if 或 while mixed 边界。
4. 运行验证：
   - `cargo fmt --all`
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `cargo run -p scoop --features llvm -- test`
   - `cargo clippy --workspace --all-targets -- -D warnings`
5. 若通过：
   - 更新 `TODO.md` / `PLAN.md` / 本文件；
   - 提交 git commit；
   - 停止。

## 执行结果

- 已完成 `T2003c0c2b3d2`。
- 已修改 `crates/scoopc/src/llvm/codegen/effect/mixed.rs`：
  - no-immediate mixed 分流不再只接受 top-level direct+indirect mixed；
  - 现已支持 statement-position nested block 的 direct + indirect same-stmt mixed；
  - initial body / continuation step 都已接入 block prefix、same-block next-site replay 与 indirect tail replay；
  - 修复了 second indirect step replay 时未补回 block scope、导致外层 handle-body 局部（例如 `prefix`）在 replay 中触发 `unknown local value` 的问题。
- 已新增回归：
  - run-pass：`tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_indirect_block_multi.scoop`
  - build：`tests/fixtures/build/effect_multi_escape_direct_indirect_if_is_error.scoop`
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 下一步应切换到 `T2003c0c2b3d3`（if branch direct + indirect same-stmt mixed）。
