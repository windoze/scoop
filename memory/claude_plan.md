## 当前执行计划

1. 读取 `TODO.md`，按标题是否带有 `[DONE]` 判定首个未完成任务。
2. 检查最近一次提交是否直接提到与该任务相关且未完成的问题；若该问题构成当前任务前置条件，则先在 `TODO.md` 中显式记录依赖。
3. 阅读当前任务涉及的代码、测试、规范与相关文档，确认实现边界与验证要求。
4. 以最小正确改动完成当前任务；若遇到阻塞当前任务的真实缺口或回归，则先修复该问题，或按要求在 `TODO.md` 中添加最小前置任务并停止。
5. 运行与当前任务直接相关的验证，并补充必要测试；再运行任务要求的更广泛检查，确保无警告。
6. 更新 `memory/claude_plan.md` 记录关键进展与计划变更。
7. 按要求更新 `TODO.md` 的完成状态与完成记录；仅在阶段计划变化时更新 `PLAN.md`。
8. 检查工作区变更，按仓库约定创建一次提交，然后停止，不进入下一个任务。

## 当前任务

- `G2-T03R：Review explicit outcome/transport primitive，确认 contract 已 backend-owned`

## 本轮执行步骤

1. 复核 `TODO.md` 中 `G2-T03` 与 `G2-T03R` 的要求、验证项和完成条件。
2. 检查最近一次提交是否与该 review 任务直接相关，并据此聚焦审阅范围。
3. 阅读以下实现文件，确认 explicit outcome/query/write-back surface 是否真正 backend-owned，且未残留 runtime bridge：
   - `crates/scoopc/src/llvm/codegen/effect_outcome.rs`
   - `crates/scoopc/src/llvm/codegen/runtime_abi.rs`
   - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`
   - `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`
   - `crates/scoopc/src/llvm/codegen/mir_body.rs`
4. 运行 `cargo check -p scoopc` 与针对旧 bridge 名字的 grep，验证当前错误是否确已切换到后续结构性 gap。
5. 若 review 中发现当前任务范围内的真实问题，直接修复并重新验证；若发现阻塞后续但无法在本任务内完成的问题，则按要求在 `TODO.md` 记录前置依赖。
6. 若 review 通过，则更新 `TODO.md` 将 `G2-T03R` 标为 `[DONE]`，补全完成记录，并同步更新本文件。
7. 检查工作区、提交本次变更并停止。

## 进展记录

- 已写入初始计划。
- 已读取 `TODO.md` 并确认首个未完成任务为 `G2-T03R`。
- 已完成首轮代码复核与验证：旧 bridge 名字 grep 为零命中，`cargo check -p scoopc` 的首批错误确已切到 `G4/G6/G7` 等后续结构性 gap。
- review 发现一个需要在本任务内收口的问题：`object_init` / top-level immutable init bridge 仍直接返回 `ScoopEffectOutcome::const_zero()`，应改为统一走 `effect_outcome.rs` 中的 backend-owned outcome builder，避免 outcome write-back surface 再次分散。
- 已将默认 complete outcome 的构造集中到 `effect_outcome.rs::build_zero_complete_effect_outcome(...)`，并让 object/top-level immutable init bridge 复用该 helper。
- 二次验证完成：`cargo fmt` 通过；旧 bridge 名字 grep 仍为零命中；`cargo check -p scoopc` 仍失败，但首批错误继续集中在 `G4/G6/G7` 等后续结构性缺口，未回退到 outcome/transport primitive 或旧 bridge 残余问题。
- 已更新 `TODO.md`：将 `G2-T03R` 标记为 `[DONE]`，并补齐 review 完成记录；下一步只做 git 提交并停止。
