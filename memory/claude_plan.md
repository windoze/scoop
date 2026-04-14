# 执行计划

说明：我不会写出逐字逐句的内部推理，但会持续在这里维护可审计的执行计划、关键判断依据、进度与调整。

## 初始目标

本次调用只完成 `TODO.md` 中第一个未完成任务，然后停止。

## 既定执行步骤

1. 检查最新一次 Git 提交，确认提交说明里是否提到任何已知遗留问题。
2. 如果最新提交提到需要先修复的遗留问题，优先定位、修复、测试，并在必要时更新 `TODO.md` / `PLAN.md`。
3. 阅读 `TODO.md`，定位第一个未完成任务。
4. 判断该任务是否过大：
   - 如果可直接完成，则继续实现。
   - 如果过大，则拆分为更小子任务，更新 `PLAN.md` 与 `TODO.md`，并只执行拆分后的第一个子任务。
5. 阅读相关代码、规格、测试和计划文件，确认实现边界与依赖。
6. 实现该任务，避免引入规避性方案；如果发现规范不匹配或前置缺失，则先把问题转化为更前置的 `TODO.md` 任务并更新 `PLAN.md`，随后提交并停止。
7. 运行必要的格式化、测试与质量检查，至少覆盖：
   - 相关定向测试
   - 必要时运行更大范围测试
   - `cargo clippy --all-targets -- -D warnings`
8. 更新文档与计划状态：
   - 在 `TODO.md` 中标记本次完成的任务
   - 在 `PLAN.md` 中记录当前状态和后续影响
   - 持续同步本文件，记录关键进展和计划变化
9. 使用清晰的 Git 提交信息提交变更。
10. 停止，不继续下一个任务。

## 待确认信息

- 该任务是否依赖尚未实现的语言特性、运行时能力或规范修复。

## 进度记录

- 已创建本计划文件，准备开始仓库检查。
- 已检查最新提交 `067b797f4db47e0794747b295cb2f4d1948f3db1`，提交信息为 `[T2003r3d3a] Reconnect mixed sibling nonresuming leaf`，提交说明中未额外提到需要先修复的遗留问题。
- 已读取 `TODO.md` / `PLAN.md`，确认当前第一个未完成任务是 `T2003r3d3b`：推广 unified multi-escape leaf 到 direct source-path matrix。
- 当前判断：先不拆任务，先审计 `multi_resuming_heap.rs`、相关 unified plan metadata、现有 LLVM 定向测试与 representative fixtures；若发现任务实际仍跨多个独立缺口，再回写 `TODO.md` / `PLAN.md` 做进一步拆分。

## 当前执行计划

1. 阅读 `multi_resuming_heap.rs`、`shared.rs`、`state_machine_plan.rs` 与相关测试，定位当前 top-level-only gate 的具体位置。
2. 对照已有 single-arm / mixed unified leaf 的 source-path replay 做法，确认 pure multi-escape direct nested path 所需的最小共享 helper 与 metadata。
3. 实现 `T2003r3d3b`，要求：
   - pure multi-escape direct site 支持 legal nested source-path；
   - 不新增按源码形状分流的 emitter 主路径；
   - sibling non-resuming / `finally` 继续复用统一 contract。
4. 补定向 LLVM 单测与 representative run-pass fixtures，覆盖 direct source-path matrix。
5. 运行格式化、定向测试、fixture 运行与 `cargo clippy --workspace --all-targets -- -D warnings`。
6. 更新 `TODO.md`、`PLAN.md`、本文件并提交。

## 已完成关键步骤

- 已把 `multi_resuming_heap.rs` 的 pure multi-escape leaf 从 top-level direct-only 扩展为基于 unified source-path 的递归拦截 / replay：
  - direct site 不再要求 `resume_path.is_empty()`；
  - main body 与 step trampoline 都改为消费 unified plan 的 while / if / block source-path；
  - 不新增任何按 block / if / while 单独分流的 emitter 主路径。
- 已新增 plan / LLVM 定向单测：
  - `resolve_mixed_escape_direct_sites_from_plan_recovers_nested_source_path_matrix`
  - `unified_multi_resuming_codegen_emits_heap_continuation_direct_source_path_matrix_sample`
- 已新增 representative run-pass fixture：
  - `tests/fixtures/run-pass/effect_multi_escape_direct_source_path_matrix.scoop`
- 已完成验证：
  - `cargo check -p scoopc --features llvm`
  - `cargo test -p scoopc llvm::codegen::effect::tests:: -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_escape_direct_source_path_matrix.scoop`
  - `cargo fmt --all`
  - `cargo clippy --workspace --all-targets -- -D warnings`

## 待收尾

1. 更新 `TODO.md` 与 `PLAN.md` 的任务状态和完成说明。
2. 检查工作树 diff 与状态。
3. 提交本次变更并停止。
