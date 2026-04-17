# Claude Plan

说明：按当前执行约束，此文件记录“可验证的决策摘要、执行计划、进度更新与变更原因”，不记录逐字内部推理。

## 本轮目标

完成 `TODO.md` 中第一个未完成任务；如果遇到前置缺陷或规格不匹配，先把该问题作为更高优先级任务纳入 `TODO.md` / `PLAN.md`，提交后停止。

## 初始执行计划

1. 检查最新一次 Git 提交，确认是否提到已知问题、遗留修复项或显式 TODO。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对任务上下文、依赖与当前计划。
4. 检查工作树状态，识别是否存在用户未提交改动，避免误覆盖。
5. 评估第一个未完成任务：
   - 如果任务边界清晰且本轮可完成，直接实现。
   - 如果任务过大，先拆分为可执行子任务，更新 `TODO.md` 与 `PLAN.md`，本轮只执行第一个子任务。
   - 如果实现过程中发现更早的规格缺口/已有缺陷，先新增前置任务并重排依赖，再停止。
6. 对已实现变更运行相关验证：
   - 最小必要测试；
   - 如任务影响范围允许，运行更完整的测试/检查（包括 `cargo fmt`、相关 `cargo test`、必要时 `cargo clippy --all-targets -- -D warnings`）。
7. 更新文档状态：
   - 在 `TODO.md` 标记完成或重排任务；
   - 在 `PLAN.md` 记录完成情况、拆分结果或阻塞原因；
   - 在本文件记录关键进度与决策。
8. 提交本轮所有改动，提交后停止，不继续下一个任务。

## 进度日志

- 已创建本轮计划文件，下一步开始检查最新提交与任务列表。
- 已检查最新提交、`TODO.md`、`PLAN.md` 与工作树：当前第一个未完成任务是 `T3009b2aR`。
- 审查 `callee_suspend_state` 生产路径后确认：
  - LLVM 生产发射只在 `Suspend` terminator 中执行一次 `get + clear`，把当前 TLS suspend state 提升进 continuation 字段；
  - runtime 生产路径只在 `scoop_continuation_resume_common()` 中把 continuation 捕获值临时恢复进 TLS，并在 step_fn 返回后恢复 caller 原 TLS；
  - 当前未发现按 callee 名称、fixture 名称或源码形状分流的捕获逻辑。
- 审查同时发现一个需要直接修复的 ABI 边界问题：
  - `runtime/c/scoop_runtime_api.h` 仍把裸 TLS 符号 `__scoop_callee_suspend_state` 作为正式导出符号暴露；
  - `scoop_callee_suspend_state_set` 当前只被运行时测试使用，却仍以通用 runtime API 的形式暴露。
- 下一步修复：
  1. 把 `__scoop_callee_suspend_state` 收紧为 runtime 内部静态 TLS，不再作为 ABI 导出。
  2. 移除通用导出的 `scoop_callee_suspend_state_set`，改为显式 test helper，避免形成生产旁路。
  3. 更新相关测试、allowlist 与任务文档，然后跑定向测试、全量测试与 clippy。
- 已完成 ABI 收紧：
  - `runtime/c/scoop_runtime.c` 中的 `__scoop_callee_suspend_state` 已改为 `static` TLS；
  - 原通用 setter 已改为 test helper `scoop_test_callee_suspend_state_set`；
  - `runtime/c/scoop_runtime_api.h` allowlist 与相关运行时测试已同步更新。
- 已完成验证：
  - `cargo test -p scoop_runtime --test continuation_one_shot`
  - `cargo test -p scoop_runtime --test effect_tls`
  - `cargo test -p scoop_runtime abi_exports_allowlist -- --nocapture`
  - `cargo test -p scoopc suspend_ir_captures_callee_suspend_state_into_continuation -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 本轮结论：
  - `T3009b2aR` 可收口完成；
  - continuation/runtime ABI 现已以 continuation 字段为唯一持久化 owner；
  - TLS 只承担运行期动态范围内的临时寄存职责，不再以裸导出符号形式暴露。
- 下一步：更新 Git 状态，提交本轮变更，停止在 `T3009b2b` 之前。
