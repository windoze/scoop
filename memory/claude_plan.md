## 2026-04-19 本轮续作计划（T4008b1a）

说明：按要求先记录可公开的执行计划、依据、风险与步骤；这里不写内部私有推理细节，只记录后续可审计的决策与行动。

当前状态：
- 已确认最新提交 `a1ad298 [T4008b] 拆分 continuation resumed-step blocker` 未声明额外需先修复的问题。
- `TODO.md` 当前首个未完成任务是 `T4008b1a`：为 escape continuation binder 补 resumed tail 的 direct boundary effect-row 汇总。
- 上一轮已开始修改 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`，但尚未编译、测试、收尾，也尚未同步 `PLAN.md`。

本轮目标：
1. 使 `state_machine_plan.rs` 的新 direct-step 分析代码可以通过编译。
2. 为 `T4008b1a` 增加最小且有区分度的回归测试，覆盖：
   - resumed tail 会把 escape site 之后的 direct effect 纳入 summary；
   - summary 会在再次触达 escape continuation 边界时停止，不跨到后续 escape site。
3. 运行定向测试、全量测试与 `cargo clippy --all-targets -- -D warnings`。
4. 若通过，则更新 `TODO.md`、`PLAN.md`、本文件，并提交仅与 `T4008b1a` 相关的改动后停止。

已知风险：
- 新增代码可能缺少 `EffectRow` 等导入，或与现有 `HandlePlanBuilder` / resume-tail 重建接口不匹配。
- direct summary 目前只应覆盖 `T4008b1a` 范围：direct `perform` 与 direct effectful call；`arm body` / `finally` / nested handle / hidden boundary 仍保留给 `T4008b1b`。
- 若在实现或测试中发现规范不匹配且不能在本轮无 workaround 地修复，需要先更新 `TODO.md`/`PLAN.md` 记录阻塞，再提交并停止。

执行步骤：
1. 检查当前工作树与相关文件差异，确认未完成编辑点。
2. 修复 `state_machine_plan.rs` 编译错误与接口问题。
3. 编写/调整测试，验证 direct-step effect-row summary。
4. 运行验证命令：
   - `cargo test -p scoopc ...`（定向）
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
5. 更新 `TODO.md`、`PLAN.md`、`memory/claude_plan.md` 并提交。
# 本轮执行计划

## 说明

用户要求先把思路和分步计划写入此文件，再执行任何命令。这里记录的是可公开的执行思路摘要与操作计划，不包含隐藏推理细节。

## 目标

完成 `TODO.md` 中第一个未完成任务，并在完成后立即停止。

## 计划步骤

1. 检查最新一次 Git 提交信息，确认是否提到了需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`、相关规范和实现代码，确认该任务的范围、依赖与现状。
4. 如果任务过大或存在前置缺口：
   - 拆分任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md` 里的任务顺序与依赖；
   - 本轮只处理拆分后的第一个可执行子任务，或在被前置问题阻塞时仅记录并提交计划调整。
5. 如果任务可直接执行：
   - 实现代码；
   - 补充或调整测试；
   - 运行相关验证，至少包括与改动直接相关的测试，并尽量满足 `cargo clippy --all-targets -- -D warnings` 与必要的 `cargo test`。
6. 更新文档与任务状态：
   - 在 `TODO.md` 中标记已完成任务；
   - 在 `PLAN.md` 中记录当前进展与后续状态；
   - 如执行过程中计划变化，及时回写到本文件。
7. 检查工作区变更，整理提交内容，使用清晰的 Git 提交信息提交。
8. 停止，不继续处理下一个任务。

## 当前状态

- 已完成：初始化本轮计划文件。
- 已完成：检查最新提交、`TODO.md` 与 `PLAN.md`；确认最新提交未引入需先修的额外既有 bug。
- 已完成：确认当前首个可执行未完成任务原为 `T4008b1`。
- 已完成：补读 continuation / handle / state-machine 相关实现，确认本轮目标应先沉淀“resumed-step effect-row”分析结果，而不是提前修改 `k` 的外显类型。
- 已完成：进一步确认 `T4008b1` 仍跨越“resumed tail 重建”和“复杂边界语义补齐”两套基础设施；已将其细化为 `T4008b1a -> T4008b1b`。
- 进行中：执行新的首个子任务 `T4008b1a`，实现 resumed-tail 重建与 direct boundary summary API，并补充回归。

## 细化执行方案

1. 在 effect/state-machine 分析相关代码中实现一个内部可复用的 resumed-step effect-row 计算入口：
   - 输入：`handle` 以及 escape continuation arm/站点信息；
   - 输出：每个 escape continuation binder 对应的 step-level `EffectRow`。
2. 本轮先让该分析正确处理 `T4008b1a` 范围内的主线：
   - 从 escape site 之后开始；
   - 不把 site 之前的 prefix effects 算进去；
   - 在再次命中 escape continuation 边界时停止，不把 fresh continuation 之后的 tail 算进去。
   - direct `perform` / direct effectful call 进入 summary；复杂 arm/finally/nested-handle/hidden boundary 语义留给 `T4008b1b`。
3. 增加单测/回归，断言：
   - 最小 probe 中 `Ask.current(), k -> ...` 的 direct summary 包含 `Boom` 而不是 `Pure`；
   - 第一个 escape site 的 direct summary 不会错误吃进第二个 escape site 之后的 tail。
4. 验证相关 Rust 单测、必要的 fixture/typecheck、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
5. 更新 `TODO.md`、`PLAN.md` 与本文件，记录 `T4008b1a` 已完成，再提交并停止。

## 2026-04-19 阶段进度更新

- 已完成 `state_machine_plan.rs` 中 `T4008b1a` 主线实现的首轮收敛：
  - 新增按 escape site 重建 resumed tail 并汇总 direct-step `EffectRow` 的内部 API；
  - direct-call summary 现在会回退读取 handle 内局部函数值声明的精确类型元数据，而不只盯住 `callee.ty`。
- 已新增两条 Rust 单测：
  - direct effectful function-value call 会被计入 resumed-step summary；
  - summary 会在下一次 escape continuation 边界停止，不把后续 site 的 tail 误算到前一个 site。
- 已验证：`cargo test -p scoopc direct_step_` 通过。
- 下一步：跑 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`，随后更新 `PLAN.md` / `TODO.md` 并提交。

## 2026-04-19 最终验证更新

- 全量验证已完成并通过：
  - `cargo test --all`
  - `cargo run -p scoop -- test`（`fixtures: ok (1055)`）
  - `cargo clippy --all-targets -- -D warnings`
- `T4008b1a` 现可正式标记完成；下一个未完成任务已切换为 `T4008b1b`。
- 剩余收尾动作：同步 `TODO.md` / `PLAN.md` 状态后，提交本轮改动并停止。
