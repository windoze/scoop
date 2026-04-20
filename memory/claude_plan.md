# 当前执行计划

## 约束与执行边界

- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后立即停止。
- 在开始具体实现前，先检查最新提交是否提到现存问题；若有，先修复这些问题。
- 若首个未完成任务过大，先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`。
- 任何发现的规范不一致、实现缺口、测试绕过或依赖缺失，都必须先转化为前置任务，更新 `TODO.md` / `PLAN.md`，提交后停止，不能带着 workaround 继续。
- 所有说明与工作记录使用中文。

## 初始步骤

1. 查看最新一次提交信息，判断是否提及尚未解决的问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 结合 `PLAN.md`、相关代码和规范，判断该任务是否可以在本轮完整落地。
4. 如果任务过大或存在前置依赖缺口：
   - 拆分为更小的子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md` 的顺序与依赖；
   - 本轮只执行拆分后的第一个子任务，或在被阻塞时只提交任务调整。

## 实施步骤

1. 阅读与目标任务直接相关的代码、测试、文档。
2. 实现任务，避免引入临时兼容层、跳过分支或仅针对 fixture 的补丁。
3. 补充或调整测试，确保行为与规范一致。
4. 运行相关验证：
   - 至少运行与改动直接相关的测试；
   - 如改动影响面较广，补充运行更大范围测试；
   - 最终检查 `cargo clippy --all-targets -- -D warnings` 是否通过（如果适用于本次改动范围）。
5. 更新文档状态：
   - 在 `TODO.md` 标记该任务完成，或在阻塞时调整任务顺序；
   - 在 `PLAN.md` 记录当前状态、后续依赖与变更原因；
   - 持续更新本文件，记录关键进展和计划变化。
6. 使用清晰的提交信息提交本轮变更。

## 进度记录

- 已创建本轮计划文件。
- 已检查最新提交 `497f0af [T4009c] Defer spawn and join surface`：提交说明本身未额外引入需先处理的新遗留 issue。
- 已读取 `TODO.md` / `PLAN.md` / `ISSUES.md`，确认首个未完成任务为 `T4009h`。
- 已核对现状：
  - runtime stable handle API（`scoop_handle_new/get/drop`）与低层 Rust 单测 `crates/scoop_runtime/tests/stable_handle.rs` 已存在；
  - `SCOOP_FULL_SPEC.md` / `SCOOP_RUNTIME.md` / `sysroot/core.scoop` 已说明“长期 token 用 handle、短时裸地址借出用 pin”；
  - 仍缺少两类产出：一是 native 侧长期保存 `GcHandle.raw` 并回传给 Scoop 的回归；二是 stale token / cancelled registration / lookup failure 的高层合同文字。
- 已确认当前任务无需再拆子任务，本轮计划直接完成：
  1. 为测试辅助层补一个最小 handle-slot extern helper，模拟 reactor/callback 持有并回传 `GcHandle.raw`；
  2. 新增正向 runtime_gc fixture，覆盖 handle token 经 native round-trip 后仍能定位对象；
  3. 新增失败 fixture，覆盖“取消/释放后，晚到的 stale token lookup 会失败”；
  4. 同步更新 `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`sysroot/core.scoop`，明确 ownership / round-trip / stale token / pin 职责边界；
  5. 更新 `TODO.md` / `PLAN.md`，运行定向验证与全量要求中的关键命令。
- 已完成实现：
  - `runtime/c/scoop_test.c` 新增 `scoop_test_handle_token_slot_reset/store/take`，可模拟 native 长期保存 `GcHandle.raw` 并在稍后回传；
  - `runtime/c/scoop_runtime_api.h` / `runtime/c/scoop_gc.h` 同步补齐导出与底层合同注释；
  - 新增 `tests/fixtures/runtime_gc/gc_handle_token_roundtrip_callback_basic.scoop` 与 `tests/fixtures/runtime_gc/gc_handle_stale_callback_token_is_error.scoop`；
  - 已同步更新 `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`sysroot/core.scoop`。
- 执行中发现并处理的一个实现细节：
  - `GcHandle` 在 Scoop 侧应通过 struct literal `GcHandle { raw: raw }` 重建，而不是写成可调用构造 `GcHandle(raw)`；这不是新的 blocker，而是当前语言对 struct 的既有构造规则，已在 fixture 与文档中统一改正。
- 已完成验证：
  - `cargo run -q -p scoop -- run tests/fixtures/runtime_gc/gc_handle_token_roundtrip_callback_basic.scoop` → stdout `wake 7`
  - `cargo run -q -p scoop -- run tests/fixtures/runtime_gc/gc_handle_stale_callback_token_is_error.scoop` → 退出码 `3`
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/runtime_gc` → `fixtures: ok (19)`
  - `cargo test -q -p scoop_runtime --test stable_handle` → 通过
  - `cargo run -q -p scoop_tools -- spec-fixtures check` → `spec fixtures: ok (1)`
  - `cargo run -q -p scoop -- test` → `fixtures: ok (1074)`
  - `cargo test --all` → 通过
  - `cargo clippy --all-targets -- -D warnings` → 通过
- 已同步状态：
  - `TODO.md` 已将 `T4009h` 标记为完成，并写入实现摘要与验证命令；
  - `PLAN.md` 已把 P6 / issue 跟踪推进到 `T4009R`；
  - 下一次调用应从 `T4009R` 开始。

## 记录原则

- 这里记录的是执行计划、关键决策、进度与外显依据，不记录不可复现的私有推理。
