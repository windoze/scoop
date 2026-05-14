## 当前执行计划

说明：按安全与保密要求，这里记录可审计的执行计划与关键决策摘要，不记录内部推理细节。

1. 读取 `TODO.md`，识别第一个标题未带 `[DONE]` 的任务，并确认它就是本次唯一执行单元。
2. 检查最近提交信息是否直接提到与该任务相关且未完成的问题；若存在且它构成当前任务前置条件，则先在 `TODO.md` 中反映该依赖关系。
3. 阅读任务相关代码、测试、规格与必要上下文，仅围绕当前任务收集实现所需信息，避免开放式排查。
4. 实现当前任务要求的代码改动；若遇到阻塞当前任务的真实缺陷或缺失能力，则先修复，或在 `TODO.md` 中最小化新增前置任务并停止。
5. 运行任务要求的验证与相关测试，并修复实现过程中暴露的问题；同时确保 `cargo clippy --all-targets -- -D warnings` 不报错（若其适用于当前改动范围）。
6. 更新 `memory/claude_plan.md`，记录关键进展、计划变更、验证结果与是否存在阻塞。
7. 在任务真正完成后，更新 `TODO.md`：将任务标题显式标记为 `[DONE]`，补全 completion record；仅在阶段计划实际变化时更新 `PLAN.md`。
8. 按仓库约定创建一次 git 提交，提交信息使用当前任务号，随后停止，不继续处理下一个任务。

## 当前任务识别

- 已读取 `TODO.md`。
- 当前第一个未完成任务：`P2-T02`《收口 native surface gate 与诊断，统一 `@Extern` / native `FunPtr` contract`》。
- 最近提交：`[P2-T01] Establish unified native callable ABI classifier`。
- 对最近提交的判断：它直接完成了 `P2-T01` 的 classifier 收口，但未显示引入额外未跟踪前置任务；`TODO.md` 中已明确 `P2-T02` 负责 gate / diagnostics 收口，因此当前先按 `P2-T02` 执行。

## 当前执行细化

1. 阅读 `P2-T02` 涉及的 typecheck、diagnostic、测试与设计文档，确认当前 `@Extern` 与 `FunPtr` 的分裂点。
2. 判断现状是否允许保留 aggregate native surface；若可保留，则实现统一 contract 与诊断，并补 direct vs indirect parity 覆盖。
3. 若发现无法按现有任务直接完成的真实前置缺口，则在 `TODO.md` 中最小化引入前置任务并停止。
4. 完成实现后运行任务要求的 fixtures、相关单测、`cargo clippy --all-targets -- -D warnings`。
5. 任务完成后更新 `TODO.md` completion record，并创建一次提交后停止。

## 当前进展

- 已实现统一 native surface gate：`@Extern` 不再只用 `GC-free` 近似；native `FunPtr` 也不再只检查“是不是 pure 函数类型”。
- 当前 front-end contract：允许标量、`UIntPtr`、`Ptr<T>`、纯 `FunPtr<F>` token、tuple、以及 `@CLayout` struct；拒绝普通非 `@CLayout` nominal aggregate、GC ref、`Pinned`、`Continuation`、`Option` 等未固定 native value layout 的类型。
- 已更新：
  - `crates/scoopc/src/typecheck/{lower.rs,annotations.rs,expr/call.rs}`
  - `sysroot/unsafe.scoop`
  - `MANAGED_ABI.md`
  - 多个 typecheck fixture（含新增 `extern_fun_signature_with_{clayout_struct_ok,plain_struct_is_error}` 与 `uintptr_to_funptr_plain_struct_type_arg_is_error`）
- 已完成首轮验证：
  - `cargo fmt --all`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_signature_with_plain_struct_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_signature_with_clayout_struct_ok.scoop`
- 尚未发现需要把当前任务拆成新的前置任务的阻塞项。

## 验证收尾

- 已补完任务所需验证：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_signature_with_gc_ref_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_signature_with_continuation_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_signature_with_pinned_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_effectful_funptr_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/uintptr_to_funptr_effectful_type_arg_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/uintptr_to_funptr_plain_struct_type_arg_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_extern_call_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/extern_native_aggregate_return_direct_indirect_parity.scoop`
  - `cargo test -p scoopc llvm_tests -- --nocapture`（filter 返回 0 条命中）
  - `cargo test -p scoopc native_callable -- --nocapture`
  - `cargo test -p scoopc abi_baseline_native_funptr_aggregate_return_uses_native_result_abi -- --nocapture`
  - `cargo clippy --all-targets -- -D warnings`
- 已回写 `TODO.md`：`P2-T02` 现已标记为 `[DONE]`，并补全 completion record。
- 未发现新的阻塞前置任务；下一次调用应从 `P3-T01` 开始。
