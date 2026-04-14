# 执行计划与进度记录

说明：我不会写入不可公开的内部推理细节，但会持续记录可审计的执行计划、决策依据、关键发现与进度。

## 当前目标

按 `TODO.md` 的优先顺序完成第一个未完成任务；如果发现前置缺陷或规格不匹配，先修复或把它们整理为更高优先级任务，并更新 `PLAN.md` / `TODO.md` 后提交。

## 初始执行计划

1. 检查最新一次 Git 提交信息，确认是否提到需要先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认该任务的上下文、依赖与已有拆分。
4. 如任务过大，先将其拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
5. 实现当前应执行的第一个任务。
6. 运行相关格式化、静态检查与测试，至少覆盖：
   - `cargo fmt --check`
   - `cargo clippy --all-targets -- -D warnings`
   - 与当前任务直接相关的测试命令
7. 更新 `TODO.md` 与 `PLAN.md`，记录完成情况或阻塞原因。
8. 使用清晰的提交信息创建 Git 提交，然后停止。

## 当前状态

- 已创建计划文件。
- 已检查最新提交、`TODO.md`、`PLAN.md`。
- 最新提交 `53ef769e5bd0ac2088d4a81b157de2ecd8e4079b` 的提交信息未额外声明需先修的遗留问题；当前仍按 `TODO.md` 顺序执行。
- 当前首个未完成任务：`T2003r3d3c`，目标是把 unified pure multi-escape leaf 从 direct source-path 扩展到 indirect / callee-suspend matrix。

## 当前任务理解

- 现状：`multi_resuming_heap.rs` 已经能消费 unified plan 的 direct source-path matrix，但对 `resolve_mixed_escape_indirect_sites_from_plan(...)` 解析出的 indirect / callee-suspend suspend sites 直接报：
  - `handle multi-resuming heap-continuation-only (indirect call site not yet supported)`
- 关键观察：
  - unified plan / resolver 已能恢复 indirect site 的 source path 与 capture ids，说明主要缺口在 emitter 接线，而不是 plan 表达层。
  - 仓库中已有 single-arm escape-continuation 的 indirect call-site / callee-suspend lowering，可复用其 resume payload 写回 TLS callee suspend state、重新调用 callee、再继续 tail replay 的 contract。
  - 当前任务范围看起来仍可在一轮内完成，暂不需要继续拆分 `TODO.md` / `PLAN.md`。

## 本轮细化计划

1. 继续阅读 `multi_resuming_heap.rs`、single-arm indirect escape lowering 与共享 helper，明确可复用的 callee-suspend / resume replay 机制。
2. 修改 unified multi-escape heap leaf，使其能接入 indirect / callee-suspend site：
   - 让 site 解析结果进入 unified heap leaf，而不是直接 early reject。
   - 复用 unified plan 的 source-path / capture metadata，避免引入专用 indirect main route。
   - 保持 sibling non-resuming / `finally` 与 heap continuation contract 不分叉。
3. 补 LLVM 定向单测，覆盖 pure multi-escape indirect / callee-suspend representative sample。
4. 新增或更新 run-pass fixture，覆盖 indirect / callee-suspend matrix 的代表性路径。
5. 运行最小验证集并修复问题：
   - 相关 `cargo test -p scoopc ...`
   - 相关 `cargo run -p scoop --features llvm -- run ...`
   - `cargo clippy --workspace --all-targets -- -D warnings`
6. 更新 `TODO.md` / `PLAN.md` / 本文件并提交。

## 进度日志

- 2026-04-14：初始化计划文件，准备开始仓库检查。
- 2026-04-14：已确认首个未完成任务为 `T2003r3d3c`，并完成首轮代码路径审计；当前判断该任务可直接实现，无需先拆分子任务。
## 2026-04-14 续做计划（本轮）

### 当前目标
- 继续完成 `TODO.md` 中第一个未完成任务 `T2003r3d3c`：`Effect：推广 unified multi-escape leaf 到 indirect / callee-suspend matrix`。
- 本轮只处理这一项；完成后更新 `TODO.md`、`PLAN.md`、`memory/claude_plan.md`，补充测试并提交一次 git commit，然后停止。

### 已知前置结论
- 已检查最新提交 `53ef769e5bd0ac2088d4a81b157de2ecd8e4079b`，提交信息未声明额外必须先修的问题。
- 现有未提交实现已经把 unified multi-resuming heap leaf 扩展到 direct + indirect site 的大部分接线。
- 当前主要失败点在新单测：恢复 indirect call site 时，重新计算 `site.init` 触发 `UnsupportedMainBody { kind: "unknown local value" }`。

### 本轮执行计划
1. 复查失败用例和相关代码，确认报错对应的源码位置与缺失的 local。
2. 检查 unified plan / capture / step 恢复链路，确定是 capture 集不完整还是恢复时未重新放回 env。
3. 修复 indirect site 恢复逻辑，必要时补 plan/capture 计算。
4. 跑最小相关测试：
   - 新增/现有单测
   - 相关 LLVM codegen effect 测试
   - 新 run-pass fixture
   - `cargo clippy --workspace --all-targets -- -D warnings`
5. 若实现中发现真实 spec 缺口，按要求先更新 `TODO.md` / `PLAN.md` 记录依赖并停止；否则在任务完成后更新文档并提交。

### 本轮记录约定
- 关键结论、计划变更、测试结果会持续回写到本文件。

## 2026-04-14 本轮执行结果

### 关键发现
- `resolve_mixed_escape_indirect_sites_from_plan(...)` 已能恢复 indirect / callee-suspend site 的 `capture_ids`，但 `multi_resuming_heap.rs` 在组装 pure multi-escape unified leaf 时最初把这部分 capture 聚合丢掉了，导致 step trampoline 重新求值 `site.init` 时丢失 `counter` 等局部函数值。
- 真实 representative sample 里还存在第二个缺口：planner 只靠 `known_local_fun_effects` / `callee.ty` 识别 local function-value call，无法稳定识别 handle body 内刚声明出来的 effectful function value，因此 `counter()` 一度没有进入 unified suspend-site 列表，运行时直接漏回 outer sibling dispatch。

### 已完成修改
- `crates/scoopc/src/llvm/codegen/effect/multi_resuming_heap.rs`
  - 让 indirect site 的 `capture_ids` 并入 multi-escape leaf 的统一 capture 聚合。
  - 修正 indirect dispatch helper 的 builder 收尾：构建 dispatch/catch 后恢复到正常 continuation block，避免遗留无 terminator 的 `effect_unwind_cont`。
- `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`
  - `classify_suspend_call(...)` 现会按当前已绑定 local slot 类型识别 handle body 内的 effectful function value call，不再遗漏 `counter()` 一类 indirect site。
- `crates/scoopc/src/llvm/codegen/effect/state_machine_plan_tests.rs`
  - 新增两个定向 plan 回归：
    - `resolve_mixed_escape_indirect_sites_from_plan_captures_local_function_value`
    - `resolve_mixed_escape_indirect_sites_from_plan_keeps_callee_suspend_and_local_function_sites`
  - 保留并跑通 LLVM 定向样例：
    - `unified_multi_resuming_codegen_emits_heap_continuation_indirect_callee_suspend_matrix_sample`
- `tests/fixtures/run-pass/effect_multi_escape_indirect_callee_suspend_matrix.scoop`
  - 已补对应 stdout fixture：`effect_multi_escape_indirect_callee_suspend_matrix.stdout`

### 验证结果
- 通过：`cargo test -p scoopc resolve_mixed_escape_indirect_sites_from_plan_captures_local_function_value -- --nocapture`
- 通过：`cargo test -p scoopc resolve_mixed_escape_indirect_sites_from_plan_keeps_callee_suspend_and_local_function_sites -- --nocapture`
- 通过：`cargo test -p scoopc unified_multi_resuming_codegen_emits_heap_continuation_indirect_callee_suspend_matrix_sample -- --nocapture`
- 通过：`cargo test -p scoopc llvm::codegen::effect::tests:: -- --nocapture`
- 通过：`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_escape_indirect_callee_suspend_matrix.scoop`
- 通过：`cargo clippy --workspace --all-targets -- -D warnings`

### 当前状态
- `T2003r3d3c` 可标记完成。
- 下一轮应继续 `T2003r3d3d`。
