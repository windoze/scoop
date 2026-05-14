# 执行计划

注意：我不会写出逐字的内部私有推理，但会持续在这里维护可审查的高层计划、关键判断、阻塞项与进度更新。

## 初始计划

1. 读取 `TODO.md`，定位第一个标题未标记为 `[DONE]` 的任务，并确认其依赖、验收标准与完成记录要求。
2. 检查最近提交信息，判断是否存在与该任务直接相关且明确未完成的问题；若存在，将其视为当前任务的一部分或按要求补录为前置任务。
3. 阅读与当前任务直接相关的代码、测试、文档与计划文件，仅收集完成该任务所需的上下文，避免无关范围扩张。
4. 如任务可直接完成：实施最小且正确的修改，补充或更新测试，并运行任务要求的验证命令。
5. 如遇到阻塞当前任务的真实缺陷、缺失特性或规范不匹配：先精确定义问题，再按要求更新 `TODO.md`（必要时也更新 `PLAN.md`），把阻塞项作为最小前置任务插入正确顺序，然后停止。
6. 任务完成后：
   - 更新 `memory/claude_plan.md` 记录结果与验证情况。
   - 在 `TODO.md` 中将当前任务标题显式改为 `[DONE]`，并完善完成记录。
   - 仅在阶段级计划确实变化时更新 `PLAN.md`。
   - 进行 git 提交，提交信息遵循仓库风格并包含任务号。
7. 完成一个任务后立即停止，不继续下一个任务。

## 执行约束

- 以 `TODO.md` 作为任务顺序与完成状态的唯一事实来源。
- 不以变通方案、夹具特判或缩小范围来绕过规范缺口。
- 若发现阻塞问题，只添加最少必要的前置任务，不为方便而拆分当前任务。
- 代码修改尽量小而正确；测试和验证必须覆盖当前任务的真实行为。
- 不回退或覆盖我未创建的现有改动。

## 进度

- 已创建初始计划文件。
- 已读取 `TODO.md`，确认首个未完成任务为 `P1-T01`：建立 callable ABI identity，并让 `ExternFun.abi` 真正进入 lowering 的 source of truth。
- 已检查最近提交：最近正式提交停在 `[P0-T01]`，未发现提交说明中直接点名的 `P1-T01` 未完成项。
- 已确认工作树存在大量与 `P1-T01` 相关的未提交改动，需按“上次执行中断后的续作”处理：理解现状、补完缺口、完成验证后原子提交全部未提交文件。
- 已审阅关键入口，当前工作树中已出现 `P1-T01` 主体结构：
  - `hir::CallableAbiIdentity` 已新增到共享层；
  - `ExternFun.abi -> callable_abi_identity()` 已进入 `hir_stage` 与 LLVM codegen；
  - typed call contract 已为 direct/member/extension/fun value/funptr/closure 携带 `abi_identity`；
  - declaration path 与 direct call / MIR direct call 已开始按 `abi_identity` 决定 native/ordinary/effect-step 路径。
- 待确认项：
  - 当前实现是否已完整满足 `P1-T01` 验收而无需继续补丁；
  - 是否仍存在阻碍本任务闭合的残余 ABI 猜测路径；
  - 验证命令是否全部通过，尤其是 `refactor_hir_call_contracts_surface_ok`、`unsafe_funptr_extern_call_basic`、`cargo test -p scoopc llvm_tests -- --nocapture`。
- 验证结果：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/refactor_hir_call_contracts_surface_ok.scoop` 通过。
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_extern_call_basic.scoop` 通过。
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_effectful_funptr_is_error.scoop` 通过。
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/uintptr_to_funptr_effectful_type_arg_is_error.scoop` 通过。
  - `cargo test -p scoopc llvm_tests -- --nocapture` 在当前 harness 下未命中任何测试（`0 passed; 841 filtered out`），因此补跑了：
    - `cargo test -p scoopc refactor_hir_call_contracts_record_callable_provenance -- --nocapture`
    - `cargo test -p scoopc refactor_hir_rejects_effectful_funptr_signature_before_hir -- --nocapture`
    - `cargo test -p scoopc abi_baseline_native_funptr_aggregate_return_uses_native_result_abi -- --nocapture`
    三条均通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。
- 已将 `TODO.md` 中的 `P1-T01` 标记为 `[DONE]` 并补写完成记录。
- 下一步：检查最终 diff / git status，随后以 `P1-T01` 任务号提交当前全部未提交文件并停止。
