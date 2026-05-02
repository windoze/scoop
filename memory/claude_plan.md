# Claude Plan

## Constraints
- 不记录或暴露内部私有推理，仅记录可执行计划、决策依据摘要、进度与验证结果。
- 先读取 `TODO.md` 作为索引，再按顺序检查对应 `TODO-Px.md` 中的完成状态，定位第一个未完成的详细任务。
- 默认完成该任务本身；若遇到阻塞当前任务的真实前置问题，则在对应 `TODO-Px.md` 中补充最小必要前置任务，并同步 `TODO.md`，然后提交并停止。
- 在执行过程中持续更新本文件，记录计划变更、关键发现、实现步骤、验证结果与提交信息。

## Initial Execution Plan
1. 读取 `TODO.md`，确认详细任务文件索引和顺序。
2. 按索引顺序读取相关 `TODO-Px.md`，定位第一个未明确记录为完成的详细任务。
3. 查看最近提交信息，确认是否存在与该任务直接相关且未完成的问题，需要并入当前任务或登记为前置。
4. 阅读该任务涉及的代码、文档、测试与规范约束，确定最小正确改动范围。
5. 实现任务；若发现阻塞性规格缺口或回归，先修复该问题，或在详细 TODO 中插入前置任务并同步索引。
6. 运行与该任务相关的测试、必要的 `cargo test` 子集，以及质量检查；若改动范围需要，则补充运行 `cargo clippy --all-targets -- -D warnings`。
7. 更新对应 `TODO-Px.md` 的完成记录；若任务索引、标题、顺序或依赖变化，同步更新 `TODO.md`；仅在阶段计划变化时更新 `PLAN.md`。
8. 检查工作区状态，整理提交内容，使用符合仓库风格的提交信息提交本次变更。
9. 停止，不继续下一个任务。

## Progress Log
- 已创建初始计划，下一步开始读取任务索引并定位当前应执行的首个未完成详细任务。
- 已读取 `TODO.md`、`TODO-P0.md`、`TODO-P1.md`、`TODO-P2.md`；确认 `P0`、`P1` 全部条目已在详细文件中记录完成。
- 当前首个未完成详细任务：`P2-T02`（`TODO-P2.md`），目标是对齐 `Continuation` surface contract，并把一般性的单一 `Unit` 参数零参 sugar 落到 refactor typed 阶段。
- 当前执行子计划：
  1. 检查最近提交是否留下与 `P2-T02` 直接相关的未完事项。
  2. 阅读 `sysroot/core.scoop`、refactor `hir_stage`/typecheck 入口、现有 `Continuation.resume` 相关 legacy 逻辑，以及当前 HIR/typecheck fixtures。
  3. 判定是否存在阻塞 `P2-T02` 的真实前置缺口；若无，则直接实现最小正确改动。
  4. 补齐/更新 fixture、HIR dump 与单元测试，验证 refactor 路径上的 typed sugar 与 `Continuation` surface contract。
  5. 回写 `TODO-P2.md` 完成记录，必要时同步 `TODO.md`/`PLAN.md`，然后提交并停止。
- 已检查最新提交：`[P2-T01R] Review typed HIR stage split`，未发现与 `P2-T02` 直接相关且需要先补的新前置事项。
- 关键发现：工作区中的现有改动已经接通了 `Continuation` surface 与大部分 typed `Unit` sugar 主线，但重载决议和 extension/member-call 还缺少“exact-arity 优先，失败后才回退到 `Unit` sugar”这一步，因此继续补齐 `typecheck/expr/call.rs` 的候选选择逻辑。
- 已完成实现：
  1. `sysroot/core.scoop` 已把 `Continuation` 固定为 compiler-owned `interface` surface，并去掉 tuple payload 扁平传参与旧 resume 特例叙述。
  2. `typecheck/expr/call.rs` 现已把 typed `Unit` sugar 收口到一般 callable 规则，并覆盖 direct-call、extension/member-call、function value、funptr、effect-op 与 `Continuation.resume(...)`。
  3. 已补齐 exact-arity 优先级：若 `f()` 存在真实零参候选，则不会误回退到 `f(())`；只有零参匹配失败时才启用 `Unit` sugar fallback。
  4. `ast` side table / typed HIR canonicalization 已保持 AST 原始零参 surface 不变，同时在 typed HIR 中把命中的 sugar 调用统一落成显式 `UnitLit` 参数。
  5. 已新增 extension/overload 定向单元测试与 typecheck fixtures，并补齐 HIR/typecheck 样本。
- 已完成验证：
  1. `cargo test -p scoopc --no-default-features continuation_resume`
  2. `cargo test -p scoopc --no-default-features unit_single_param_zero_arg`
  3. `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/continuation_resume_answer_expression_ok.scoop`
  4. `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/continuation_resume_unit_sugar_ok.scoop`
  5. `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/unit_single_param_zero_arg_call_ok.scoop`
  6. `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/unit_single_param_zero_arg_extension_call_ok.scoop`
  7. `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/unit_single_param_zero_arg_overload_prefers_exact_zero_arg_ok.scoop`
  8. `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/continuation_resume_tuple_requires_single_tuple_arg_is_error.scoop`
  9. `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/hir/continuation_resume_surface_named_tuple_and_unit_basic.scoop`
  10. `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-hir tests/fixtures/hir/continuation_resume_surface_named_tuple_and_unit_basic.scoop`
  11. `cargo test -p scoop --no-default-features parity`
  12. `cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`
- 下一步：检查最终 diff，创建 `P2-T02` 提交并停止。
