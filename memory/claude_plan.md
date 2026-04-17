# 执行计划

## 说明

按要求先记录执行计划。这里记录的是可审查的行动计划、检查项、阶段结论与后续调整，不包含冗长的内部推理原文。

## 当前目标

完成 `TODO.md` 中第一个未完成任务，然后停止。

## 执行步骤

1. 检查最新一次提交：
   - 查看最新提交信息与改动。
   - 判断提交中是否提到已知问题、临时修复、后续遗留项。
   - 如存在提交中明确提到且尚未解决的问题，先修复这些问题。
2. 检查任务列表与计划：
   - 阅读 `TODO.md`，识别第一个未完成任务。
   - 阅读 `PLAN.md`，确认该任务的上下文、依赖与已有拆分。
   - 如任务过大或存在明确前置依赖，先更新 `PLAN.md` 与 `TODO.md`，将任务拆为更小子任务，并把当前要做的子任务放在首个未完成位置。
3. 实施当前任务：
   - 阅读相关源码、测试、规格或文档。
   - 修改实现，确保不引入规避性方案或偏离规格的行为。
   - 若发现规格与实现不一致且阻塞当前任务，则先把该问题转化为新的前置任务，更新 `TODO.md`/`PLAN.md`，提交后停止。
4. 验证：
   - 运行与改动直接相关的测试。
   - 运行必要的全量或工作区检查，至少覆盖编译、测试与 `clippy` 无 warning 的要求。
   - 若验证失败，立即修复后重新验证。
5. 文档与任务状态更新：
   - 在 `TODO.md` 中将已完成任务标记为完成。
   - 在 `PLAN.md` 中更新当前状态、后续顺序与关键说明。
   - 在本文件中补充完成情况与任何计划调整。
6. 提交：
   - 使用清晰提交信息提交本次修改。
   - 完成一个任务后立即停止，不继续处理下一个任务。

## 初始检查清单

- [x] 查看最新提交
- [x] 查看 `TODO.md`
- [x] 查看 `PLAN.md`
- [x] 确认第一个未完成任务
- [x] 判断是否需要拆分
- [x] 实施改动
- [x] 运行验证
- [x] 更新 `TODO.md`
- [x] 更新 `PLAN.md`
- [x] 更新本文件
- [x] 提交并停止

## 进度记录

- 已创建初始执行计划，等待读取仓库状态与任务列表。
- 已检查最新提交 `528f876 [T3012] Rebaseline unified expected-context task scope`：
  - 提交本身只更新 `PLAN.md` / `TODO.md` / `memory/claude_plan.md`，未直接引入新的生产代码缺陷声明。
  - 提交中提到的残留问题已按任务边界重新归类到既有后续任务：`continuation_resume_enum.scoop` 属于 `T3013` / `T3009b`，不是当前 `T3012R` 之前必须额外前插的新 issue。
- 已确认当前首个未完成任务为 `T3012R`：Review：确认 unified path 的 expected context 与 closure 支持已与普通 codegen 对齐。
- 已完成第一轮生产代码审查，重点覆盖：
  - `crates/scoopc/src/llvm/codegen/expr.rs`
  - `crates/scoopc/src/llvm/codegen/control_flow.rs`
  - `crates/scoopc/src/llvm/codegen/stmt.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc/src/llvm/codegen/effect/mod.rs`
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
- 当前审查要点与阶段判断：
  - unified path 中 handle 结果 expected type 来源于 `codegen_handle_expr(..., expected)` 与 `contract.result_ty()`，不是 fixture 名称或源码形状。
  - local initializer 仍复用普通 `codegen_initializer_expr`；call/when/if/print/println 等 expected-context 逻辑仍落在普通 codegen 入口（`codegen_call` / `codegen_if_expr` / `codegen_when_expr` / `codegen_sysroot_print_like`）。
  - unified emitter 未发现按 `single` / `indirect` / `nested` / callee shape 等源码分类分流的生产逻辑回流。
  - 初步未发现必须在 `T3012R` 内直接修复的明确生产 bug；接下来用定向 fixture + 全量验证确认。
- 已完成验证：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_locals.scoop`：通过。
  - `cargo run -p scoop -- run tests/fixtures/run-pass/std_test_assertions_basic.scoop`：通过。
  - 额外手动最小复现：`handle` 返回局部 closure 值，可成功运行，未触发 unified-only closure 占位错误。
  - `cargo test --all`：通过。
  - `cargo clippy --all-targets -- -D warnings`：通过。
  - `cargo run -p scoop --features llvm -- test`：仍停在已跟踪的 stale `EXPECT: fail` `tests/fixtures/run-pass/continuation_resume_continuation.scoop`（`T3017`），未出现新的更早失败点。
- 当前结论：
  - `T3012R` 可按“无新增生产代码修复、review 结论成立”收口。
  - 下一步文档状态更新应为：`TODO.md` 将 `T3012R` 标记完成，`PLAN.md` 推进下一项到 `T3013`，随后提交并停止。
