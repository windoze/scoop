## 当前执行计划（初始版）

### 目标
- 按照 `TODO.md` 的顺序完成第一个未完成任务，然后停止。
- 在开始正式实现前，先检查最近一次提交是否提到任何已知问题；如果提到，则先修复该问题。
- 在执行过程中，任何已存在的问题、回归、规格不匹配、实现边界缺失或测试中暴露的问题，都视为当前范围内事项，必须先修复或在 `TODO.md` 中前置建模为依赖任务。

### 约束与原则
- 不采用变通方案、夹层修补、仅针对夹具的特殊处理或偏离规格的实现。
- 只能在完整实现并通过相关验证后，才将任务标记为完成。
- 若当前任务过大，需要先拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，随后只执行拆分后的第一个子任务。
- 完成后必须提交 Git commit，然后停止，不继续下一个任务。

### 预定执行步骤
1. 检查最近一次 Git 提交的提交信息与相关上下文，确认是否显式提到待修复问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认当前计划、依赖和任务上下文。
4. 检查工作区状态，识别是否存在用户未提交修改，避免覆盖。
5. 评估第一个未完成任务的规模与依赖：
   - 若可直接完成，则进入实现。
   - 若过大或被缺失特性/已有缺陷阻塞，则先更新 `TODO.md`/`PLAN.md`，把前置任务排到正确位置，并在本次只处理新的第一项。
6. 实施代码修改，并在关键进展后回写本文件。
7. 运行与变更相关的测试、格式化和质量检查，至少包括必要范围的测试；若任务触及公共行为或核心路径，则补充更广验证。
8. 如验证中发现既有问题：
   - 若可以在本次一并修复，则立即修复并重新验证。
   - 若无法在当前任务内直接完成，则将该问题作为前置任务加入 `TODO.md`，调整顺序，更新 `PLAN.md`，提交后停止。
9. 任务完成后：
   - 更新 `TODO.md` 为已完成状态。
   - 更新 `PLAN.md` 反映当前状态和后续顺序。
   - 回写本文件记录完成情况与已执行验证。
   - 提交 Git commit，提交信息使用任务编号或明确描述。

### 预期检查项
- 最近提交是否带有“已知问题”“follow-up”“fixme”“regression”等信息。
- `TODO.md` 中首个未完成项是否存在隐含依赖。
- 相关实现是否已有未完成边界、注释中的 TODO、禁用测试、临时绕过逻辑。
- 变更后是否满足：
  - `cargo fmt`
  - `cargo clippy --all-targets -- -D warnings`
  - 与任务直接相关的测试
  - 必要时 `cargo test --all`

### 当前进展（已确认）
- 最近一次提交为 `[T4013R] Review inline removal and marker-only @Inline`，提交信息未显式提到需要在本轮优先修复的额外既有问题。
- `TODO.md` 的首个未完成任务已确认是 `T4014a`：明确普通 `@Extern` 不能穿透 effect / continuation / non-local control。
- `PLAN.md` 也将当前主线标记为 `T4014a -> T4014b -> T4014R`。
- 工作区存在用户未提交修改：`run_agent.sh`；后续必须避开，不得覆盖。
- 已盘点到当前实现缺口：
  - `annotations.rs` 目前只检查 `@Extern` ABI 类型必须是 GC-free 值类型，还没有禁止 non-Pure effect row / `eff` 参数。
  - LLVM `codegen_top_level_fun_call(...)` 仍对 `@Extern + call_may_suspend` 安装 `legacy effect boundary`，允许 outward effect 从 native 返回。
  - 仓库中仍有回归 `tests/fixtures/run-pass/extern_native_effect_boundary_raise_try_catch_basic.scoop` 与对应 LLVM 单测，明确验证 “effectful extern 可以工作”，这与 `T4014a` 目标冲突。
- 已完成的代码改动（待验证）：
  - `crates/scoopc/src/typecheck/annotations.rs` 已新增 `extern_fun_effects_not_allowed` / `extern_fun_eff_param_not_allowed` 两个门禁，ordinary `@Extern` 现在会拒绝 non-Pure effect row 与 `eff` 参数。
  - `crates/scoopc/src/llvm/codegen/mod.rs` 已移除 extern call 的 effect-boundary 支持；ordinary `@Extern` 不再在 codegen 里安装/消费显式 outcome。
  - `crates/scoopc/src/llvm/mod.rs` 的 LLVM 单测已改为锁定 “pure extern call 不安装 effect boundary”。
  - `runtime/c/scoop_test.c` 与 `runtime/c/scoop_runtime_api.h` 已删除仅用于 effectful extern outward propagation 的 test-only helper/allowlist。
  - 已新增/替换 typecheck fixtures，删除旧的 effectful extern run-pass fixture，并在 `sysroot/unsafe.scoop` 注释中明确 `FunPtr<F>` 是显式 bridge。

### 风险记录（当前）
- 仓库中的 `memory/claude_plan.md` 原本已有上一轮内容；本轮后续更新时需要保留当前执行轨迹与结论，避免再次覆盖有用上下文。
- 对 “non-local control” 的静态边界还需要谨慎定稿。当前更明确、低歧义的收口点是：普通 `@Extern` 禁止声明 non-Pure effect row 与 `eff` 参数；continuation 继续由现有 GC-free ABI 门禁禁止直接跨越；显式 unsafe bridge（如 `FunPtr<F>`）应继续允许。

### 下一步
- 先运行定向测试，确认新增 diagnostics、fixture 迁移与 LLVM 单测通过。
- 若定向测试通过，再同步 `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`ISSUES.md`、`PLAN.md`、`TODO.md`。
- 最后做格式化、全量相关测试与 `clippy`，确认后再收尾提交。

### 更新规则
- 一旦确认首个未完成任务、发现前置缺陷、决定拆分任务、开始实现、完成验证或调整计划，都要立即更新本文件。

### 最终结果（本轮）
- `T4014a` 已完成，并已在 `TODO.md` 标记为 `[DONE]`；下一次调用应从 `T4014b` 开始。
- 本轮没有发现需要前插到 `TODO.md` 的新 blocker；已有 probing 中暴露的 “effectful extern/native boundary 仍被允许” 已作为 `T4014a` 的核心范围直接修复。
- 代码/文档结果：
  - ordinary `@Extern` 现已在声明层要求 Pure（或省略 effect row）、禁止 `eff` 参数，并继续拒绝 GC-managed control object 直接跨 ABI。
  - extern-native outward-effect lowering、相关 run-pass 与 test-only runtime helper 已删除。
  - `FunPtr<F>` 被保留并明确为 effectful/deferred native capability 的显式 unsafe bridge。
  - `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`ISSUES.md`、`PLAN.md`、`TODO.md`、`sysroot/unsafe.scoop` 已同步到同一叙事。
- 已执行验证：
  - `cargo test -p scoopc --features llvm pure_extern_call_does_not_install_effect_boundary -- --nocapture`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  - `cargo fmt --all`
  - `cargo run -p scoop_tools -- spec-fixtures check`
  - `cargo run -p scoop -- test`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 工作区注意事项：
  - 用户已有未提交改动 `run_agent.sh`，提交时必须排除，不得一并提交。
