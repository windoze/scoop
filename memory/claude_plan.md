## 当前执行计划

1. 先检查最新一次提交，确认提交信息里是否提到任何已知遗留问题；若有，优先修复这些问题。
2. 阅读 `TODO.md`，定位第一个未完成任务，并确认是否存在依赖关系或阻塞项。
3. 如首个未完成任务过大，拆分为可执行子任务，并同步更新 `PLAN.md` 与 `TODO.md`，本轮只执行拆分后的第一个子任务。
4. 在实现前阅读相关代码、测试、规范或文档，明确影响范围。
5. 实现当前目标任务，保证改动符合规范，不引入临时性绕过方案。
6. 运行相关测试与必要的质量检查；若出现失败或规范不一致，先修复问题或将真正的前置任务加入 `TODO.md`。
7. 更新 `memory/claude_plan.md` 记录进展，随后更新 `TODO.md` 与 `PLAN.md`。
8. 使用清晰的提交信息提交本轮改动，然后停止，不继续处理下一个任务。

## 进度记录

- 已创建本计划文件，下一步检查最新提交与任务列表。
- 已确认最新提交未额外标注遗留问题；当前首个未完成任务是 `T2003u4b2`。
- 任务判断：暂不再拆分，按“single-arm escape-continuation 主 emitter 切到统一状态机输入”直接推进。
- 当前实施方案：
  1. 扩展 unified plan 的 source-path 元数据，使 single-arm escape 的 direct/indirect 站点都能从 plan 恢复回旧 emitter 需要的源码位置。
  2. 把 single-arm escape 入口改为显式接收 `HandleStateMachinePlan` 与 arm id，不再直接依赖旧扫描器做终态站点发现。
  3. 在 escape emitter 内新增 plan-driven resolver：从 unified plan 解析 direct perform 站点、indirect call 站点，以及基于 plan capture set 派生 body lift / outer capture。
  4. 补充单元测试，锁定 direct/indirect source-path 与 plan-driven 解析；随后运行相关测试与质量检查。
# 本轮执行计划（更新于 2026-04-13）

## 当前任务理解

- 目标仍然是 `TODO.md` 中首个未完成任务 `T2003u4b2`：将 single-arm escape-continuation 主 emitter 切换到 unified state-machine 输入。
- 上一轮已经把 direct / indirect suspend site 发现和 capture 集合改为从 unified state-machine plan 解析，相关单测与大部分回归已通过。
- 当前已知剩余问题是 `tests/fixtures/run-pass/continuation_resume_ref_class.scoop` 失败，症状为 `unsupported_main_body` / `unknown local value`。
- 从已有单测看，更像是 single-arm escape emitter 的某条恢复/续执行路径没有正确把 lifted local 放回 codegen 环境，而不是 unified plan 漏算 capture。

## 已知约束

- 必须先检查最新提交是否提到 pre-existing issue；若有，需优先修复。
- 只完成 `TODO.md` 的首个未完成任务，不继续做后续任务。
- 不能用 workaround；若发现真实实现缺口，需要在 `TODO.md` / `PLAN.md` 中显式建依赖任务并停止。
- 完成后必须测试、更新 `TODO.md` / `PLAN.md`、提交 git commit，然后停止。

## 执行步骤

1. 检查最新提交内容，确认是否存在提交中明确提及但未修复的问题。
2. 重新确认 `TODO.md` 的首个未完成任务仍是 `T2003u4b2`，同时查看 `PLAN.md` 了解当前分解状态。
3. 复现 `continuation_resume_ref_class.scoop` 失败，并定位 `unknown local value` 对应的具体 symbol / 控制流路径。
4. 重点审查 `crates/scoopc/src/llvm/codegen/effect/escape_continuation.rs` 中 single-arm direct escape 的：
   - lifted local 恢复逻辑
   - perform 后续执行路径
   - top-level tail / nested tail 对 body lift 的特殊分支
   - 恢复后 `cg.env` / local slot 的接线
5. 如定位明确，修复 emitter 仍残留的旧 scanner / 旧 env 假设，使 single-arm escape 主路径完全依赖 unified plan 输入。
6. 补充或调整最小必要单测，覆盖本次回归场景。
7. 运行相关验证：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `cargo run -p scoop --features llvm -- test`
   - `cargo clippy --workspace --all-targets -- -D warnings`
8. 若以上全部通过，更新 `TODO.md` 和 `PLAN.md`，将 `T2003u4b2` 标记完成并记录结果。
9. 提交本轮变更，提交后停止。

## 当前判断

- 已通过临时诊断定位到真实根因：失败的不是 `b1` / `b2` 这类 body lift，而是外层局部 `cell`（`SymbolId=1`）。
- 具体表现为：第二次 direct perform 在 step trampoline 中再次进入 escape arm body 时，`cell.k = Some(k)` 这类“arm body 读取外层局部”的路径没有被纳入当前 capture 集合；step `cg.env` 里只有 resumed body 所需 capture / lifts，没有 arm body 的 outer locals，因此在 codegen arm body 时触发 `unknown local value`。
- 修复方向调整为：
  1. 给 unified state-machine plan 增加 single-arm escape arm body 的 free-local capture 元数据；
  2. direct / indirect single-arm escape emitter 在构造 `outer_captures` 时，除了 site capture 集合外，还要并入 arm body 的 outer captures；
  3. 移除临时调试打印，补最小单测 / 回归验证。

## 完成状态

- `T2003u4b2` 已完成。
- 实际修复分成两部分：
  1. unified plan 现在会为 escape arm 记录 free-local capture 集合，single-arm escape direct/indirect emitter 会把这组 arm-body outer captures 与 suspend-site capture 集合合并，避免第二次及后续 perform 在 step trampoline 中重新进入 arm body 时丢失外层局部（已修复 `continuation_resume_ref_class.scoop` 中的 `cell` 丢失）。
  2. unified plan 的 outer-scope slot / declared-local 收集现在会穿过 nested handle，同时排除 inner handle 的 binders、`resume` / `k` 符号和内部 `val`；这样外层 continuation step 能保留“只在 nested handle 中使用”的外层局部（已修复 `std_task_async_adapters_basic.scoop` 中 `Task.andThen` 嵌套 handle 丢失 `resultTask` 的问题）。
- 新增单测：
  - `escape_arm_capture_locals_include_outer_scope_reads`
  - `resolve_escape_direct_sites_from_plan_captures_outer_local_used_only_in_nested_handle`
- 本轮最终验证已全部通过：
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 下一轮应从 `T2003u4c` 继续：迁移 mixed-arm / site-matrix / multiple-resuming 主 emitter 到统一状态机输入。
