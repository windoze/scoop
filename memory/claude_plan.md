# Claude Plan

## 约束与工作方式

- 本轮目标：只完成 `TODO.md` 中第一个未完成任务，然后停止。
- 在开始任何命令执行前，先建立本文件，记录执行计划与后续决策。
- 我会在本文件中持续更新“决策摘要、已完成步骤、阻塞点、计划变更”，而不是写入不可核查的原始思维流。
- 若发现最新提交提到的遗留问题，需先修复这些问题，再处理 `TODO.md` 任务。
- 若当前首个未完成任务过大，需要先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`，然后只执行拆分后的第一个子任务。
- 若遇到任何与规范不符但当前任务依赖的问题，必须先把该问题前置为 `TODO.md` 任务，更新 `PLAN.md`，提交后停止，不能以规避方案继续。

## 初始执行计划

1. 检查最新一次提交信息，确认是否提到了需要先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`、相关说明文档与实现代码，确认任务上下文、依赖与现状。
4. 判断任务是否足够小且可直接实现；若过大，则拆分任务并更新 `PLAN.md` / `TODO.md`。
5. 实现本轮目标任务，必要时补充或重构测试。
6. 运行格式化、测试、`clippy` 等验证，修复发现的问题直到通过。
7. 更新 `TODO.md` 与 `PLAN.md`，记录本轮完成情况或依赖调整。
8. 提交所有变更，使用清晰提交信息，然后停止。

## 当前状态

- 已完成：
- 建立计划文件。
- 检查最新提交：最新提交 `1e9b853 [T3009b2R] Front-load multi-site callee resume blocker` 只做任务重排与计划更新，未在提交说明里引入额外需要先修复的独立遗留 bug。
- 读取 `TODO.md` / `PLAN.md` 并定位首个未完成任务。
- 当前识别到的首个未完成任务：`T3009b2c`。

## 当前任务识别

- 任务编号：`T3009b2c`
- 任务标题：收口 ordinary indirect callee 多 suspend-site 的 resumed-body caller-tail 合同
- 初步理解：当前 ordinary indirect callee 在“同一 callee 内存在多个 suspend site”时，resume 后会把 payload 误当作整次调用结果，跳过 callee 自身 post-suspend body；需要把 multi-site 情况接回统一 resumed-body caller-tail 语义。

## 下一步计划

1. 读取 `TODO.md` / `PLAN.md` 中 `T3009b2c` 与其前置任务附近的完整描述。
2. 检查相关实现与已有 reproducer / fixture / 定向测试，确认当前失败面是否只覆盖 multi-site ordinary indirect callee。
3. 判断 `T3009b2c` 是否足够聚焦；若仍过大，则先拆分并更新 `TODO.md` / `PLAN.md`。
4. 若无需拆分，则直接实现、补测试、跑验证。

## 当前发现

- `build_ordinary_callee_suspend_plan_from_unified_contract()` 当前在 `builder.suspend_sites.len() != 1` 时直接返回 `None`，这是明确的单 suspend-site 前提。
- `CalleeSuspendPlan` 当前只有单个 `resume_slot` 与单个 `resume_tail`，`codegen_top_level_fun()` / `codegen_closure_fun_body()` 的 resume 入口也只会执行一次固定的 `plan.resume_tail`。
- caller 侧 `emit_resume_after_call_site()` 已经能区分“callee 是否存在 suspend state”，但 callee 自己的 resume 入口没有办法根据“是哪个 site 被恢复”做选择。
- 初步判断：`T3009b2c` 仍然是一个可实现的单任务，不需要再拆分；但实现上预计至少会涉及 `CalleeSuspendPlan` 数据结构、callee suspend-state ABI 字段，以及 top-level/closure resume 入口分派。

## 已确认复现

- 已用最小复现 `/tmp/t3009b2c_repro.scoop` 验证当前失败面：
  - 预期：`resume("if")` 后应回到 `viaIf` 的 resumed body，继续打印 `if_resume`、`if_after`、`I:if`，然后 outer caller 再打印 `after_if`。
  - 实际：输出直接变成 `after_handle -> fallback -> after_if -> if -> after_resume1`，缺失 `viaIf` 自身的 resumed body 输出。
- 结论：当前确实是 ordinary indirect callee 的 resumed-body caller-tail 被跳过，根因与 `T3009b2c` 描述一致。

## 本轮实现结果

- 已将 `CalleeSuspendPlan` 从 single-site 扩成 multi-site：
  - 每个 suspend site 单独记录 `resume_slot`、`resume_tail` 与 site-local `saved_locals`。
  - 整个 plan 额外保留 union locals，作为 ordinary callee suspend-state 堆对象的统一布局。
- 已修改 ordinary callee suspend-state ABI：
  - 在 `crates/scoopc/src/llvm/codegen/effect/mod.rs` 中加入 `site_tag` 字段。
  - fresh path 保存 suspend state 时写入当前 site tag。
  - resume path 先读取 `site_tag`，再 dispatch 到对应 `resume_site*` block。
- 已修改 ordinary callee codegen 入口：
  - `codegen_top_level_fun()` 与 `codegen_closure_fun_body()` 现在共用 multi-site resume dispatch helper，不再只执行单一 `plan.resume_tail`。
- 已补测试资产：
  - 新增 focused run-pass fixture `tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_multi_site_callee_branch.scoop`。
  - 新增 IR 定向测试 `ordinary_multi_site_callee_materializes_resume_site_dispatch`。

## 当前验证结果

- 最小复现 `/tmp/t3009b2c_repro.scoop` 已转正：原先缺失的 `if_resume` / `if_after` / `I:if` 已恢复。
- 已通过：
  - `cargo fmt`
  - `cargo check -p scoopc`
  - `cargo test -p scoopc ordinary_multi_site_callee_materializes_resume_site_dispatch -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_multi_site_callee_branch.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_statement_container_matrix.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

## 待收尾步骤

1. 提交变更并停止。

## 收尾状态

- `TODO.md` 已更新：`T3009b2c` 标记为完成，首个未完成任务已切换为 `T3009b2cR`。
- `PLAN.md` 已更新：记录了 multi-site ordinary callee 修复、focused fixture、IR 定向测试与全量验证结果。
- `git diff --check` 已通过。
