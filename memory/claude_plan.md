## 本轮执行计划

### 约束与执行原则
- 本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。
- 在真正实现任务前，先检查最新提交中是否提到了需要先修复的既有问题；若有，则这些问题优先处理。
- 任何与规范不一致、需要新增前置依赖、或当前任务过大无法一次完成的情况，都必须先更新 `TODO.md` 与 `PLAN.md`，并据此调整执行顺序。
- 在执行过程中持续更新本文件，记录关键决策、已完成步骤、测试结果与计划变更。

### 初始步骤
1. 查看最新提交信息，确认是否提到已有问题、待修复事项或需要优先处理的回归。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 阅读 `PLAN.md`，确认当前计划与 `TODO.md` 是否一致。
4. 如首个未完成任务过大，则将其拆分为更小的可执行子任务，并更新 `PLAN.md` / `TODO.md`。

### 实施步骤
1. 分析目标任务涉及的代码路径、规范、测试位置与潜在依赖。
2. 实现任务，必要时补充或重构代码以保持模块清晰、无临时性绕过方案。
3. 运行相关测试，随后运行更广覆盖的校验命令，至少包括与本任务相关的测试，以及在可行情况下执行格式化、lint 和必要的全量测试。
4. 若发现规范缺口、语言特性缺失或既有 bug 阻塞当前任务：
   - 精确记录阻塞点；
   - 在 `TODO.md` 中添加或重排前置任务；
   - 更新 `PLAN.md` 说明阻塞原因与依赖关系；
   - 本轮只提交这些计划调整并停止。

### 收尾步骤
1. 更新 `TODO.md`，将本轮完成的任务标记为已完成。
2. 更新 `PLAN.md`，反映当前状态、后续任务与任何新增依赖。
3. 更新本文件，记录最终执行结果与验证情况。
4. 使用清晰的提交信息创建 git commit。
5. 停止，不进入下一个任务。

### 已完成的关键检查
- 已查看最新提交：`bcf8c43 [T2003r3c] Route single resuming handles through unified emitter`。提交信息本身没有额外点名“必须先修的既有问题”。
- 已读取 `TODO.md` / `PLAN.md`，确认首个未完成任务原本是 `T2003r3d`。
- 已进一步审计 effect lowering 主入口与现有 fixture：
  - `codegen_handle_expr` 目前已统一接管 `NoSuspendSites`、`SingleNonResuming`、`MultiNonResuming`、`SingleImmediateResume`、`SingleEscapeContinuation`；
  - multi-site / mixed-resuming 仍通过 `codegen_handle_expr_multi_arm(...)` 落回旧分流；
  - `tests/fixtures/build/effect_resume_mixed_escape_while_indirect_is_error.scoop` 仍暴露合法但未完成的 mixed nested-while indirect lowering；
  - simplification 仍显式保留 `UnsupportedMixedMultipleEscapeWithImmediate` 与 `UnsupportedMixedMultipleImmediateWithEscape` 两类未实现合法组合。

### 计划调整
- 结论：原始 `T2003r3d` 范围过大，必须拆分。
- 已决定把 `T2003r3d` 拆为四个子任务：
  1. `T2003r3d1`：统一 multi-resuming 主入口，先接管当前已支持的 legal shapes。
  2. `T2003r3d2`：补 immediate+escape mixed 在 nested `while` deeper indirect site 的合法 lowering。
  3. `T2003r3d3`：补“一个 immediate + 多个 escape arms” mixed-resuming。
  4. `T2003r3d4`：补“多个 immediate arms + 一个 escape”，并清空剩余已知合法 mixed lowering 缺口。
- 当前本轮执行目标切换为：完成 `T2003r3d1`。

### 当前执行步骤
1. 更新 `TODO.md` / `PLAN.md`，把 `T2003r3d` 拆为 `T2003r3d1`～`T2003r3d4`。已完成。
2. 实现 `T2003r3d1`：为 multi-site / mixed-resuming 增加 unified 主入口分类、contract 校验与 root 路由切换。已完成。
3. 补充对应单测与 representative LLVM fixture 验证。已完成。
4. 运行相关测试与 `clippy`。已完成。
5. 更新 `TODO.md` / `PLAN.md` / 本文件并提交。进行中。

### 本轮实现结果
- 已新增 `UnifiedMultiResumingEntrypoint`，并把 root `codegen_handle_expr` 对以下类别的主选路统一收口到 `codegen_handle_expr_unified_multi_resuming(...)`：
  - `MultipleImmediateResumeTopLevel`
  - `MultipleEscapeTopLevelDirect`
  - `ImmediateResumeWithNonResumingSiblings`
  - `EscapeContinuationWithNonResumingSiblings`
  - `ImmediateResumeWithEscapeSibling`
  - `ImmediateResumeWithEscapeAndNonResumingSiblings`
- `codegen_handle_expr_multi_arm(...)` 仍作为 leaf helper 保留，但 root 不再直接依赖它做这些 legal 组合的主选路。
- unified multi-resuming 入口现已在进入 leaf helper 前校验 plan 中各 arm 的 `resume_mode` / `body_exit` 契约；对于尚未实现的 “1 immediate + N escape” / “N immediate + 1 escape” 组合，继续在 unified 入口下给出显式 `Unsupported*` 路由。

### 本轮验证结果
- `cargo fmt --all`：通过。
- `cargo test -p scoopc unified_multi_resuming_entrypoint_ -- --nocapture`：通过，6 个新增入口分类测试全部通过。
- `cargo test -p scoopc llvm::codegen::effect::tests:: -- --nocapture`：通过。
- representative LLVM fixtures：
  - `effect_resume_multi_immediate_top_level.scoop`：通过。
  - `effect_multi_escape_multi_arm_with_nonresuming.scoop`：通过。
  - `effect_resume_mixed_escape_post_immediate_if_direct_indirect_custom_nonresuming.scoop`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --all`：最终通过。
  - 第一次全套运行曾在 `commands::build::tests::build_produces_executable_and_it_runs` 上出现一次性失败（链接阶段找不到临时 `main.o`）。
  - 随后单独重跑该测试通过，再次执行 `cargo test --all` 也全部通过，因此本轮未对该测试额外改动。

### 下一步
- 当前第一个未完成任务已变为 `T2003r3d2`：补 immediate+escape mixed 在 nested `while` deeper indirect site 的合法 lowering。
