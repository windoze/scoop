# 执行计划与进度记录

## 说明

按要求在执行任何仓库检查命令前先建立本文件。出于安全限制，这里不会记录逐字内部推理，但会持续维护：

- 当前目标
- 分步执行计划
- 关键决策依据
- 已完成步骤与结果
- 若计划变化时的调整说明

## 当前目标

完成 `TODO.md` 中第一个未完成任务，并在完成后停止。本轮开始前还需要先检查最新提交是否提到已有问题；若提到，则优先修复该问题。

## 初始执行计划

1. 检查最新一次 git 提交的提交信息与变更说明，确认是否显式提到需先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对现有计划与任务依赖关系。
4. 判断该任务是否过大：
   - 如果可直接完成，则进入实现。
   - 如果过大，则先拆解任务，更新 `PLAN.md` 与 `TODO.md`，并把第一个子任务作为当前执行目标。
5. 实现当前目标，同时在探查、测试、审阅过程中留意任何既有缺陷、回归、规格不匹配或临时绕过；若发现，会先修复，或将其作为前置任务加入 `TODO.md` 并调整顺序。
6. 运行与该任务相关的测试，并补充必要测试。
7. 运行质量检查，至少包括：
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 如任务相关，还会运行更小范围或更针对性的命令以提高定位效率。
8. 更新文档与计划：
   - 在 `TODO.md` 中将当前完成任务标记为完成。
   - 在 `PLAN.md` 中反映当前状态、依赖变化和后续影响。
   - 持续更新本文件记录关键进展。
9. 检查工作区变更，确认不误改无关文件。
10. 提交本轮变更，提交信息使用任务编号或明确描述。
11. 停止，不继续处理下一个任务。

## 进度日志

- 已创建计划文件，准备开始检查最新提交与任务列表。
- 已检查最新提交 `f9bdbf575b3a25e7d1f4c0486beadc33031df90d`（`[T4014b] Finalize stable handle FFI contract`）以及其前一提交 `42d41463`；提交信息本身未显式提出必须先修的新既有问题，因此继续按 `TODO.md` 顺序推进。
- 已读取 `TODO.md`、`PLAN.md` 与 `ISSUES.md`，确认第一项未完成任务为 `T4014R`：复审普通 `@Extern` 边界是否仍隐含 GC / effect 语义，并确认 stable handle / `Pinned` 的职责分离已在实现、类型系统、ABI surface 与文档中一致成立。

## 当前执行步骤（T4014R）

1. 复审文档与 sysroot：
   - 检查 `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`sysroot/core.scoop`、`sysroot/unsafe.scoop`、`ISSUES.md` 是否仍保留“普通 `@Extern` 可穿透 effect/continuation”或“`Pinned` 可作为长期 token”这类旧叙事。
2. 复审编译器与 runtime 边界：
   - 检查 `crates/scoopc/src/typecheck/annotations.rs`、`crates/scoopc/src/typecheck/expr/error.rs`、`crates/scoopc/src/llvm/codegen/mod.rs`、相关 LLVM/fixture 测试，确认 ordinary `@Extern` 仍然：
     - 只允许 GC-free 签名；
     - 拒绝非 `Pure` effect row 与 `eff` 参数；
     - 不安装 outward-effect / continuation 边界；
     - 要求长期 identity 走 `GcHandle.raw: UIntPtr`，短时裸地址借出才走 `Pinned`。
3. 运行验证：
   - 先跑 `T4014` 相关定向 tests / spec-fixtures。
   - 再跑 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
4. 根据结果收口：
   - 若 review 暴露既有缺口，则先修复，或在 `TODO.md` / `PLAN.md` 前插 blocker 任务并停止。
   - 若未发现缺口，则把 `T4014R` 标记完成，更新 `PLAN.md` / `TODO.md` / `ISSUES.md` / 本文件并提交。

## T4014R 复审结果

- 静态复审结论：未发现新的 blocker。
- 具体结论：
  - `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`sysroot/core.scoop`、`sysroot/unsafe.scoop` 与 `ISSUES.md` 对 ordinary `@Extern` 的叙事一致：普通 FFI 边界仅承担 `enter_native -> native leaf call -> leave_native`，不再隐含 `EffectCtx` / `EffectOutcome`、continuation replay 或 non-local control 传播。
  - `crates/scoopc/src/typecheck/annotations.rs` 中 ordinary `@Extern` 的类型系统门禁一致成立：非 `Pure` effect row、`eff` 参数与 GC-managed 签名（包括 `Continuation<...>`、`Pinned`）都会被拒绝；长期 token 的推荐桥接路径保持为 `GcHandle.raw: UIntPtr`。
  - `crates/scoopc/src/llvm/codegen/mod.rs` 与 `crates/scoopc/src/llvm/mod.rs` 的 lowering / LLVM 单测一致表明：pure extern call 不再安装 effect boundary，也不再做 TLS active probing；ordinary extern path 只保留 native-roots 暴露与 leaf call。
  - runtime / fixture 注释与回归一致表明：长期 identity / callback token 走 stable handle；`Pinned` 仍只承担 Scoop 侧短时裸地址借出与保活，不再被描述为 ordinary `@Extern` ABI token。

## 已完成验证

- `cargo run -p scoop_tools -- spec-fixtures check`
- `cargo test -p scoopc pure_extern_call_does_not_install_effect_boundary --features llvm`
- `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
- `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`
- `cargo run -p scoop -- test`
- `cargo test --all`
- `cargo clippy --all-targets -- -D warnings`

## 下一步

- 更新 `TODO.md` / `PLAN.md`，将 `T4014R` 标记完成并把主线推进到 `T4015a`。
- 检查变更集，仅包含本轮 review 收口所需文件后提交。

## 最新进展

- 已完成 `TODO.md`、`PLAN.md` 与 `ISSUES.md` 的状态同步：
  - `T4014` / `T4014R` 已标记完成；
  - `PLAN.md` 的当前主线已推进到 `T4015a`；
  - `ISSUES.md` 第 11 条现明确把 ordinary `@Extern` 的 effect-impermeable 边界与 stable handle / `Pinned` token 模型一并记为已收口。
- 当前只剩最后一步：检查工作区、仅暂存本轮相关文件并提交。
