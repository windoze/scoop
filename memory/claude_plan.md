## 执行计划

说明：按安全要求，这里记录可审计的高层执行计划、关键判断依据与进度更新，不记录不可审计的内部推理细节。

1. 先读取 `TODO.md`，按标题是否带有 `[DONE]` 确定第一个未完成任务。
2. 检查最近一次提交信息，确认是否存在与该任务直接相关且明确未完成的问题；若存在，则将其视为当前任务的一部分或在 `TODO.md` 中补成前置任务。
3. 阅读当前任务相关条目中的要求、依赖、验证方式与完成记录；只在必要时阅读 `PLAN.md` 了解阶段性依赖，不把 `PLAN.md` 当作日常任务日志。
4. 检查与当前任务直接相关的代码、测试、夹具和文档，确认当前实现状态与缺口。
5. 如可直接完成，则实施最小且正确的代码修改；如遇到阻塞当前任务的真实缺陷或缺失特性，则先修复该阻塞，或将其作为最小前置任务写入 `TODO.md` 后停止。
6. 运行任务要求的验证步骤，以及必要的相关测试、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`，并修复由本次改动暴露的问题。
7. 更新 `memory/claude_plan.md` 记录关键进展；若任务完成，则在 `TODO.md` 中将任务标题标记为 `[DONE]` 并填写/更新完成记录；仅当阶段计划真的变化时才更新 `PLAN.md`。
8. 按仓库提交风格创建一次原子提交，提交信息以任务号开头，然后停止，不继续处理下一个任务。

## 进度

- 已写入初始执行计划。
- 已读取 `TODO.md` 并确认首个未完成任务为 `P2-T01：建立单一 native ABI classifier，统一 direct/indirect declaration 与 call scaffolding`。
- 已检查最近一次提交：`[P1-T02] Tighten FunPtr native-only call handoff`，未发现需要先插入到 `P2-T01` 之前的明确未完成项。
- 已读取 `PLAN.md` / `MANAGED_ABI.md` 对应章节，并检查 `crates/scoopc/src/llvm/codegen/{call/abi.rs,call/lowering.rs,mod.rs,mir_body.rs}`。
- 当前关键结论：
  - direct `@Extern` 已经负责 `enter_native/leave_native`、`gc-leaf-function` 和 native declaration；
  - native `FunPtr` 仍在 HIR/MIR lowering 中各自手写 `hidden_sret = None`、`callconv 0`，且调用路径绕过了 direct extern 的 boundary scaffold；
  - `MANAGED_ABI.md` 的 P2 设计已经要求 direct `@Extern` 与 native `FunPtr` 共用单一 native ABI classifier，并让 C ABI 的 direct/indirect 入口都走同一 boundary 语义；
  - 因此本任务的正确实现应把 `FunPtr` 间接调用也接入统一 native boundary（含 `enter_native/leave_native`），而不是继续保留隐式分裂。
- 接下来的实施步骤：
  1. 在 LLVM codegen 共享层新增 native callable classifier 结构与统一 emit helper。
  2. 让 top-level native declaration 改为使用 classifier 生成 param/return ABI、callconv、gc-leaf。
  3. 让 direct extern call、HIR funptr call、MIR direct native call、MIR funptr call 全部复用该 classifier / emit helper。
  4. 补 direct/indirect parity 回归：至少覆盖 indirect funptr 也插入 `enter_native/leave_native`，以及 direct/indirect aggregate-return parity。
  5. 更新 `MANAGED_ABI.md` / `TODO.md` / 本文件，然后运行格式化、相关回归与 `cargo clippy --all-targets -- -D warnings`。
- 已完成的实现步骤：
  - 已在 `crates/scoopc/src/llvm/codegen/{mod.rs,call/abi.rs}` 中加入 shared native callable classifier 数据结构与统一 native call emit helper。
  - 已让 `declare_top_level_fun*` / `declare_materialized_top_level_fun_with_symbol` 使用 classifier 驱动 native declaration 的 param/return ABI、callconv 与 gc-leaf。
  - 已让 HIR direct native call、HIR `FunPtr` call、MIR direct native call、MIR `FunPtr` call 复用 classifier；`FunPtr` 不再硬编码 `callconv 0`，并改为通过统一 boundary scaffold 插入 `enter_native/leave_native`。
- 已补 runtime/test 承载：导出 direct aggregate helper、增加 `scoop_test_get_gc_collect_in_native_funptr`，并新增 runtime_gc/build/run-pass/LLVM parity 回归。
- 已完成验证与文档回写：
  - `cargo fmt --all`
  - `cargo test -p scoopc native_callable -- --nocapture`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_extern_call_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/funptr_enter_native_roots_gc.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build/funptr_enter_native_no_statepoint_writeback.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/extern_native_aggregate_return_direct_indirect_parity.scoop`
  - `cargo test -p scoopc llvm_tests -- --nocapture`（filter 命中 0 条，因此额外补跑 `cargo test -p scoopc abi_baseline -- --nocapture` 与 `cargo test -p scoopc native_callable -- --nocapture`）
  - `cargo clippy --all-targets -- -D warnings`
  - 已回写 `MANAGED_ABI.md` 与 `TODO.md`，将 `P2-T01` 标记为 `[DONE]`。
- 当前仅剩提交步骤：使用 `[P2-T01] ...` 风格创建原子提交，然后停止。
