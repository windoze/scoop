## 本轮执行计划

1. 读取 `TODO.md`，按标题是否带有 `[DONE]` 判断完成状态，锁定第一个未完成任务。
2. 查看最近一次 git 提交信息，确认是否存在与该任务直接相关且明确标注未完成的事项；如果存在，将其作为当前任务的一部分或在 `TODO.md` 中登记为前置依赖。
3. 阅读当前任务涉及的代码、测试、规范记录（必要时包含 `PLAN.md` 中的阶段背景，但不把它当作任务真源）。
4. 在不规避规范缺口的前提下，直接实现该任务；如果发现阻塞当前任务的真实缺陷或缺失特性，则先在 `TODO.md` 中按依赖顺序补充最小必要前置任务，并停止继续推进当前任务。
5. 运行与当前任务直接相关的验证，再运行要求的质量检查（至少包含相关测试，以及可行时的 `cargo clippy --all-targets -- -D warnings`）。
6. 完成后更新 `memory/claude_plan.md`、`TODO.md` 的完成记录与 `[DONE]` 前缀；仅在阶段计划发生变化时更新 `PLAN.md`。
7. 按仓库约定创建一次 git 提交，只提交当前任务相关的最终结果，然后停止，不进入下一个任务。

## 当前任务

- 首个未完成任务：`P4-T01f`。
- 最近一次提交标题是 `[P4-T01f] Record scalar String bridge prerequisite`，说明它只是把该任务前置条件写入 `TODO.md`，当前仍需实际落地 bridge。

## 已确认实现方案

1. 保持 `check_extern_fun_signature_matches_native_abi()` 不变，继续拒绝 native `@Extern` 的 `String` / managed ref surface。
2. 复用现有 named intrinsic `RuntimeCall` 审计表，不新增任何“native `@Extern` 放宽”路径：
   - 为 `scoop_char_to_string` / `scoop_int_to_string` / `scoop_float32_to_string` / `scoop_float64_to_string` 增加显式 named intrinsic entry；
   - 在审计表里写清 runtime symbol、精确签名和 why-runtime 理由，明确它们是 substrate bridge，而不是新的用户态 FFI 能力。
3. 新增一个可编译 sysroot 文件，定义 ordinary managed helper（带 body），由 helper body 调用上述 named intrinsic bridge；这样后续 `P4-T01` 可以直接用 ordinary managed call 引用这些 helper，而不必借旧 `toString` 按名特判自举。
4. 把该 sysroot 文件加入“始终进入完整编译管线”的 support source 列表，确保 helper body 会被真正编译进模块。
5. 补两类回归：
   - typecheck fixture：native `@Extern` 继续拒绝 `String` 返回；
   - LLVM / fixture：compiled sysroot helper 会编进模块，并通过 ordinary managed 路径返回 `String`，其 helper body 内部再调用 runtime substrate symbol。
6. 若实现过程中发现 named intrinsic runtime 签名层无法准确表达 `i32/f32/f64`，则补最小必要的共享签名类型支持；这是 bridge 的直接前置，不属于额外拆任务。

## 进展更新

- 已完成：为 named intrinsic runtime 签名补充 `I32/I64/Float32/Float64/StringRef` 精确类型，并新增四个 `scalar_*_to_string_bridge` 审计条目，分别绑定现有 runtime substrate symbol。
- 已完成：新增 `sysroot/scalar_string_bridge.scoop`，在可编译 sysroot 中暴露 ordinary managed helper：`scoopAbiCharToString`、`scoopAbiIntToString`、`scoopAbiFloat32ToString`、`scoopAbiFloat64ToString`。
- 已完成：把 `scalar_string_bridge.scoop` 加入始终参与完整编译管线的 sysroot support source 列表。
- 已完成：补回归与验证入口：
  - `tests/fixtures/typecheck/extern_fun_signature_with_string_return_is_error.scoop`
  - `tests/fixtures/run-pass/sysroot_scalar_string_bridge_basic.scoop`
  - `crates/scoopc/src/llvm/tests.rs::compiled_sysroot_scalar_string_bridge_helpers_stay_in_module`
- 已完成的验证：
  - `cargo test -p scoopc compiled_sysroot_scalar_string_bridge_helpers_stay_in_module -- --nocapture`
  - `cargo test -p scoopc named_intrinsic -- --nocapture`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_signature_with_string_return_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_signature_with_gc_ref_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/sysroot_scalar_string_bridge_basic.scoop`
  - `cargo test -p scoopc llvm_tests -- --nocapture`（当前 harness 仍为 `0 passed; 861 filtered out`，因此实际 owner coverage 以上述定向 LLVM test 为准）
  - `cargo clippy --all-targets -- -D warnings`

## 剩余步骤

1. 回写 `TODO.md` 的完成记录并加上 `[DONE]`。
2. 评估是否需要同步 `MANAGED_ABI.md` 的 bridge 叙事；若需要则补最小文档回写。
3. 检查 `git status`，按任务约定提交本轮结果并停止。

## 执行约束

- 只处理 `TODO.md` 中顺序上的第一个未完成任务。
- 不用变通方案绕过规范、实现缺口或测试问题。
- 若发现阻塞项，先把阻塞项显式写入 `TODO.md` 并按依赖排序，然后提交并停止。
- 执行过程中若计划变化或关键步骤完成，会继续更新本文件。
