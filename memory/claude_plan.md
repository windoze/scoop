## 当前执行计划

说明：我不会写入逐字逐句的内部推理，但会在此维护完整的可执行计划、关键判断依据摘要、进度更新和阻塞信息。

1. 读取 `TODO.md`，定位第一个标题未标记 `[DONE]` 的任务。
2. 读取与该任务直接相关的说明文件（必要时包含 `PLAN.md`、相关源码/测试/夹具），确认范围、依赖和验证要求。
3. 检查最近提交是否直接提到与当前任务相关且未完成的问题；若该问题阻塞当前任务，则先在 `TODO.md` 中作为前置任务显式记录。
4. 实施当前任务要求的最小正确改动，不做规避性实现，不偏离规范。
5. 运行与任务直接相关的验证；若任务涉及通用质量门禁，则补充运行格式化、测试与 `cargo clippy --all-targets -- -D warnings`。
6. 如遇阻塞：不把当前任务标记完成；在 `TODO.md` 中插入最小必要前置任务并更新依赖/顺序；仅在阶段计划变化时更新 `PLAN.md`。
7. 若任务完成：把该任务标题改为 `[DONE]`，补全完成记录；必要时更新 `PLAN.md`（仅限阶段级变化）。
8. 检查工作区变更，按任务要求创建一次原子提交，然后停止，不继续下一个任务。

## 进度日志

- 已初始化计划文件，下一步读取 `TODO.md` 并锁定首个未完成任务。
- 已读取 `TODO.md`，首个未完成任务为 `P1-T02`：收紧 `FunPtr<F>` 合同，明确其为 pure-only native surface。
- 下一步：检查最近提交是否存在与 `P1-T02` 直接相关且未完成的问题；随后读取 `PLAN.md` / `MANAGED_ABI.md` 对应章节和实现入口文件，确认改动范围与验证口径。
- 已确认最新提交 `[P1-T01]` 没有额外声明与 `P1-T02` 直接相关的未完成事项。
- 当前代码现状摘要：
  - 前端已经通过 `check_funptr_signature_contract` 拒绝 effectful `FunPtr<F>`；
  - `sysroot/unsafe.scoop` 也已把 `FunPtr` 文档写成 pure-only native leaf token；
  - 但 MIR/handoff 仍主要把 `FunPtr` 调用压成 `CallKind::FunValue`，effect-facts / effect-lowered / LLVM funptr lowering 继续以“兼容 effect/state-machine 的通用 callable-value 形状”消费它；
  - `codegen_funptr_value_call_impl` / `codegen_mir_funptr_value_call` 仍保留 `call_may_suspend` 参数与 explicit effect hidden ABI 分支，虽然当前总是传 `false`。
- 计划中的实现收口：
  1. 把 `FunPtr` 调用在 MIR/handoff 上显式化，避免继续仅靠 `callee_ty`/carrier source type 临场识别；
  2. 删除 funptr lowering 上无实际意义的 effect hidden ABI / suspend 路径，使其 API 只表达 native-only 调用；
  3. 视需要补强 typed contract / MIR 测试，明确 `FunPtr` 只发布 native ABI family；
  4. 完成后运行 `P1-T02` 指定 fixture，并补充相关单测/质量门禁。
- 已完成的关键实现：
  - 新增 MIR `CallKind::FunPtr` 与 effect-facts `CallSiteKind::FunPtr`，把 native `FunPtr` 调用从 generic `FunValue` 路线中显式拆出；
  - 新增 `FunPtrCallSpec`，删除 LLVM funptr lowering 上遗留的 `call_may_suspend` / explicit effect hidden ABI / outcome-slot 分支；
  - 更新 HIR stable dump、MIR lowering、effect-facts、effect-lowered 与 raw/refactor LLVM call path 的相应消费者；
  - 新增 MIR 单测与 effect-facts 单测，固定 `FunPtr` native-only handoff。
- 已完成验证：
  - `cargo test -p scoopc funptr -- --nocapture`
  - `cargo test -p scoopc refactor_hir_call_contracts_record_callable_provenance -- --nocapture`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_effectful_funptr_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/uintptr_to_funptr_effectful_type_arg_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_extern_call_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/top_level_callable_value_call_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_receiver_call_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`
  - `cargo clippy --all-targets -- -D warnings`
- 当前剩余步骤：
  1. 检查 git diff / status，整理提交内容；
  2. 回写 `TODO.md` 完成记录（已完成）；
  3. 创建 `P1-T02` 提交并停止。
