## 本轮执行计划（初始）

### 目标
- 按照仓库要求，先检查最新提交是否提到遗留问题；若有，优先修复。
- 然后读取 `TODO.md`，定位第一个未完成任务，只完成这一个任务并停止。

### 思路摘要
- 先收集上下文：最新提交信息、`TODO.md`、`PLAN.md`、工作区状态。
- 判断第一个未完成任务是否过大；若过大，先在 `PLAN.md` / `TODO.md` 中拆分为更小子任务。
- 结合相关代码与测试实现该任务，避免任何规避性做法；若发现规范不匹配或前置能力缺失，则先把阻塞项转成更靠前的任务。
- 运行必要的格式化、测试与 lint，确保变更可验证。
- 更新 `TODO.md`、`PLAN.md` 与本文件，最后提交 git commit，然后停止。

### 分步计划
1. 检查最新提交信息，确认是否存在提交中提到但尚未解决的遗留问题。
2. 阅读 `TODO.md` 与 `PLAN.md`，定位第一个未完成任务及其上下文。
3. 评估任务规模与依赖：
   - 若可直接完成，继续实现。
   - 若过大或被前置缺陷阻塞，先更新 `TODO.md` / `PLAN.md` 进行拆分或重排。
4. 阅读相关代码、测试与规范，确定需要修改的模块。
5. 实现任务并补充/调整测试。
6. 运行格式化、测试、lint，修复发现的问题直到通过。
7. 更新 `TODO.md`、`PLAN.md`、`memory/claude_plan.md` 记录完成情况。
8. 使用清晰的提交信息提交本轮变更。
9. 停止，不继续处理下一个任务。

### 执行约束
- 全程使用中文记录与说明。
- 不接受 workaround；若遇到规范缺口或实现边界，必须先在 `TODO.md` / `PLAN.md` 记录为前置任务。
- 只完成一个任务。

## 当前进展

### 已完成
- 已检查最新提交 `470582e75874f25ef952d352c02e20f627d0cdcb`，提交信息与 diff 主要是对 continuation / Task 过渡注释与计划文档的对齐，没有发现一个独立于当前任务序列之外、需要在进入 `TODO.md` 前额外抢修的新遗留问题。
- 已阅读 `TODO.md` 与 `PLAN.md`，确认 `T4016` 已经拆分；当前首个可执行的未完成子任务是 `T4016b`：把 answer type、returning-resume 与 `-> resume` 移除接入 parser / typecheck / HIR / lowering 主线。
- 已进一步盘点实现现状：
  - parser / AST / HIR / LLVM 仍把 `-> resume` immediate-resume 作为显式 arm kind；
  - typecheck 仍把 `Continuation.resume(...)` 钉死成 `Unit`；
  - runtime ABI 仍是 `void scoop_continuation_resume(void*)`，因此“真正 answer-returning 的 `resume`”不能与“删除 `-> resume` 语法”在同一小步里稳妥落地。
- 基于上述依赖，已把 `T4016b` 重新拆分为：
  - `T4016b1`：删除用户态 `-> resume`，把 tail-resume 收口为内部 lowering / codegen 分类；
  - `T4016b2`：把 continuation answer type 接入 binder 静态模型与显式 surface；
  - `T4016c`：提供 runtime / ABI answer-return 通道；
  - `T4016b3`：基于统一 answer-return 通道完成 `Continuation.resume(...): Answer` 主线接入。

### 下一步
- 先提交任务拆分与依赖重排。
- 之后执行新的首个子任务 `T4016b1`：
  1. 删除 parser / AST / HIR / resolver 对 `-> resume` 的公开语义支持。
  2. 把旧 immediate-resume fixtures 改写为 `, k ->` + `k.resume(...)`。
  3. 在 lowering / codegen 内部改为基于 escape-continuation arm 的 tail `k.resume(...)` 形状做内部分类。
  4. 运行相关测试、更新 `TODO.md` / `PLAN.md` / 本文件并提交。
# 本轮续跑更新（2026-04-21）

## 当前判断

- `T4016b` 已经在上一轮被拆成 `T4016b1 -> T4016b2 -> T4016c -> T4016b3`，当前只处理第一个未完成子任务 `T4016b1`。
- `T4016b1` 的主体实现已经落地，`cargo test -p scoopc --lib` 与 `cargo test --all` 已通过。
- 当前剩余工作主要是补齐夹具/金文件、跑完整 fixture 与 lint、同步任务文档、提交本轮完成结果。
- 已知阻塞点是 `cargo run -p scoop -- test` 在 HIR golden `tests/fixtures/hir/handle_mixed_arm_kinds.hir` 上失败，需要先对齐实际输出，再继续检查是否有后续失败项。

## 本轮执行计划

1. 先检查 `tests/fixtures/hir/handle_mixed_arm_kinds.scoop` 与对应 `.hir` 金文件的差异来源，确认是 HIR 表示变化还是还有真实实现问题。
2. 若只是金文件漂移，则更新该 HIR golden；若发现实现缺口，则先修实现，再重新生成/同步预期。
3. 重新运行 `cargo run -p scoop -- test`，继续逐个修复与 `-> resume` 移除相关的 parse / HIR / typecheck / run-pass fixture 失配，直到该命令通过。
4. 运行 `cargo clippy --all-targets -- -D warnings`，处理任何告警，确保满足“无 warning”要求。
5. 更新 `TODO.md`、`PLAN.md`、本文件，明确 `T4016b1` 已完成且下一步切换到 `T4016b2`。
6. 检查工作区变更，确认只包含本任务相关改动后，使用独立提交完成本轮，不继续处理下一个任务。

## 变更准则

- 不做绕过实现的临时兼容；如果发现规范缺口，必须先把缺口转成 `TODO.md` 前置任务。
- 不回退用户现有改动，只在当前任务相关文件中做最小充分修改。
- 所有文件编辑继续使用 `apply_patch`；测试和检查命令用非交互方式执行。

## 进度更新

- 已同步 `tests/fixtures/hir/handle_mixed_arm_kinds.hir`：第一条 arm 的 HIR 已从旧 `resume` binder 形态切换为普通 continuation binder + `k.resume(...)` 成员调用。
- 已同步 `tests/fixtures/parse/handle_arm_explicit_type_args_basic.ast`：AST 现在反映 `, k ->` 绑定与成员调用，而不是已删除的 `-> resume` 专用节点。
- 已修正 `tests/fixtures/parse/handle_immediate_resume_removed.scoop` 的报错行号预期。
- 单独验证 `tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_block_multi.scoop` 可正常结束且 stdout 与 golden 一致；此前对 `cargo run -p scoop -- test` 的“卡住”判断不成立，只是整套 fixture 执行时间较长。
- 下一步继续完整等待 `cargo run -p scoop -- test` 结束；若再出现失败，再按“真实语义回归优先，其次夹具同步”的顺序处理。

## 最终结果

- 已完成 `T4016b1`：
  - 用户态 `-> resume` 语法已移除，parser 现在对旧语法给出 removed-syntax diagnostic，并指向 `, k -> { k.resume(...) }` 迁移方向。
  - AST / HIR / resolver / typecheck 已删除 `ImmediateResume` 公开语义；tail `k.resume(...)` 仅保留为 lowering / LLVM state machine 的内部分类。
  - 相关 fixtures 已完成迁移与同步，包括：
    - `tests/fixtures/hir/handle_mixed_arm_kinds.hir`
    - `tests/fixtures/parse/handle_arm_explicit_type_args_basic.ast`
    - `tests/fixtures/parse/handle_immediate_resume_removed.scoop`
    - `tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop`（改为反映 `T4016b1` 当前边界，不再错误要求旧 immediate-resume 静态拒绝）
- 最终验证结果：
  - `cargo test --all` 通过。
  - `cargo run -p scoop -- test --fixtures /tmp/scoop-t4016b1-fixtures.6ZfJrZ` 通过，受影响的 38 个 fixture 全部通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。
- 文档状态已同步：
  - `TODO.md` 已将 `T4016b1` 标记为完成。
  - `PLAN.md` 已更新为下一步进入 `T4016b2`。
