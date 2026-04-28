# 执行计划

## 当前目标

按照本次调用要求，只处理 `TODO.md` 中第一个未完成任务；在开始任务前先检查最新提交是否提到已有问题，并优先修复或排入前置任务。完成一个任务后提交 Git commit 并停止。

## 执行原则

- 使用中文记录进度与结果。
- 不采用临时绕过、夹具专用 hack 或弱化规格的实现。
- 若发现已有 bug、规格不匹配、未完成边界或测试暴露的回归，先处理该问题；若无法立即修复，则将其作为前置任务插入 `TODO.md`，更新 `PLAN.md`，提交后停止。
- 每次关键步骤完成或计划改变时更新本文件。
- 只完成一个未完成任务，不继续推进下一个任务。

## 初始步骤

1. 检查最新提交，确认是否提到预存问题或回归。
2. 阅读 `TODO.md`，找出第一个未完成任务。
3. 阅读 `PLAN.md` 和相关源码、测试、规格文件，确认任务边界。
4. 如果任务过大，先拆分任务并更新 `TODO.md` / `PLAN.md`，提交后停止或执行拆出的第一个子任务，按文件中的依赖顺序处理。
5. 实现当前任务或前置修复。
6. 添加或更新最小但充分的测试。
7. 运行相关测试；必要时运行更广的 `cargo test --all` 或指定检查。
8. 更新 `TODO.md` 和 `PLAN.md`，将本次完成的任务标记为完成并记录状态。
9. 检查工作区差异，提交清晰的 Git commit。
10. 停止，不处理后续任务。

## 进度记录

- 已检查最新提交：`05f1e6d9 [T5000i1P3] Fix entry-root MIR request roots`。
- 最新提交已将上一轮 P2 的剩余问题排入 `TODO.md`：
  - `T5000i1P4`：materializer request-root 可达扫描需要使用 MIR reachable-block 过滤；
  - `T5000i1P5`：production LLVM body emission 默认消费 materialized MIR body。
- 已读取任务列表；当前第一个未完成任务是 `T5000i1P4`，本轮只处理该任务。

## T5000i1P4 执行计划

1. 阅读 `TODO.md` / `PLAN.md` 中 `T5000i1P4` 的完整上下文，以及 `ISSUES.md` 中对应 P2 记录。
2. 定位 MIR materializer 中 `scan_reachable_non_generic_fun(...)`、entry-main request seed 过滤、MIR body reachable-block API 与 LLVM 侧已有 reachability 口径。
3. 将 materializer 对 reachable function body 的扫描从遍历 `body.blocks` 改为：
   - 优先使用 `body.reachable_blocks()` 的可达 block 集合；
   - 若 CFG reachable-block 计算失败，则保守回退扫描全部 blocks。
4. 同步收口 entry-main 模式下 initial `MonomorphRequest` seed 的放行粒度：
   - 从“可达函数体 span”缩小到 reachable block / reachable statement 级别；
   - 保持 source-file rooted dump/debug 模式原语义。
5. 添加不可达 block 中 generic direct-call 不应产生额外实例的回归测试。
6. 运行聚焦测试；再根据影响面运行 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
7. 更新 `TODO.md` 标记 `T5000i1P4` 完成，更新 `PLAN.md` 与本文件，提交 `[T5000i1P4] Filter materializer roots by reachable MIR blocks` 后停止。

## T5000i1P4 当前进度

- 已定位实现问题：
  - `scan_reachable_non_generic_fun(...)` 原先遍历 `body.blocks` 全量 block；
  - entry-main initial request seed 还保留按可达函数 span 放行的粗粒度 fallback；
  - request-root caller-side pass candidate rewrite 也会重写全 body，并可能从不可达 block enqueue 泛型调用。
- 已完成代码改动：
  - 新增 `reachable_body_block_indices(...)`，与 LLVM reachability 一样优先使用 `body.reachable_blocks()`，失败时保守回退全 block；
  - request-root 扫描改为只遍历可达 block；
  - entry-main seed fallback 从可达函数 span 改为可达语句 span；
  - request-root candidate rewrite 改为只重写可达 block，避免不可达 block 在 rewrite 阶段绕过扫描过滤 enqueue 泛型实例。
- 全量测试中暴露并已修复既有 MIR CFG 边界问题：
  - `TerminatorKind::Handle` 原先没有把 handler body / arms / finally 暴露为 CFG successor；
  - 这会让 `reachable_blocks()` 把 handle 内部语义上可执行的 block 判为不可达；
  - 现已为 handle terminator 增加保守 successor targets，并更新 `tests/fixtures/mir/handle_perform.mir`。
- 完整 fixture suite 继续暴露并已修复顶层 immutable `val` initializer 可达性缺口：
  - 入口路径读取的顶层值会 runtime lazy init；
  - 其 initializer 中的 generic call 不能被 entry-main request filtering 误删；
  - materializer 现在在可达 MIR `TopLevelRef` 命中顶层 immutable value 时，递归标记该 initializer span 及其引用的顶层值 initializer span。
- 已新增回归：
  - `request_root_scan_ignores_generic_calls_in_unreachable_mir_blocks` 手动向 `main` MIR 追加结构不可达的 `id<Int>` direct-call，并确认不会生成额外实例或 materialized body。
- 已通过：
  - `cargo fmt --all --check`
  - `cargo test -p scoopc request_root_scan_ignores_generic_calls_in_unreachable_mir_blocks -- --nocapture`
- 已继续通过：
  - `cargo test -p scoopc mir::materialize -- --nocapture`
  - `cargo test -p scoop build_frontend_ -- --nocapture`
  - `cargo test -p scoopc production_codegen_suspendability_observes_overridden_pass_summary -- --nocapture`
  - `cargo test -p scoopc mir::tests:: -- --nocapture`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone/cross_file_generic_top_level_val_basic`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已更新：
  - `TODO.md`：`T5000i1P4` 标记为 DONE，并记录实现与验证；
  - `PLAN.md`：补充本条完成记录；
  - `ISSUES.md`：对应 P2 标记为已修复。
- 下一步：最终检查 diff / status，提交 `[T5000i1P4] Filter materializer roots by reachable MIR blocks` 后停止，不处理 `T5000i1P5`。
