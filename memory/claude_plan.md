# 当前执行计划

说明：本文件记录可执行计划、关键进度与计划变更；不包含隐藏推理过程。

## 初始计划

1. 读取 `TODO.md`，按文件顺序识别第一个标题未以 `[DONE]` 开头的任务。
2. 检查最新提交是否明确提到与该任务直接相关的未完成事项；若有，将其纳入该任务或作为前置项记录到 `TODO.md`。
3. 阅读该任务涉及的最小必要代码、测试与规范上下文，避免无关历史问题排查。
4. 若任务可直接完成，实施最小正确改动，并补充或调整对应测试/fixture。
5. 运行任务要求的验证命令以及必要的相关测试；若验证失败，修复当前任务范围内的问题并重跑。
6. 若发现阻塞当前任务的真实前置缺口，不做 workaround；在 `TODO.md` 中插入最小必要前置任务，更新依赖说明，提交后停止。
7. 完成任务后，将该任务标题加上 `[DONE]`，更新 completion record；仅在阶段计划真实改变时更新 `PLAN.md`。
8. 提交所有本次任务相关变更，提交信息使用任务编号与简明说明，然后停止，不继续下一个任务。

## 本次调用计划

任务：`CG-T02：收口 runtime type/value primitive LLVM lowering`。

1. 核对最新提交，确认是否有直接关联 `CG-T02` 的未完成事项。
2. 检查 `CG-T02` 涉及的现有 lowering、backend gate、runtime type descriptor、fixture 与测试位置。
3. 实现 refactor LLVM path 中 `TypeCheck`、pattern `is Type`、`CastOp::As`、`CastOp::AsQ` 与 `!!` 所需的最小正确 lowering 或确认 frontend reject 策略。
4. 补充 `refactor_llvm_runtime_type_primitives` 定向测试和必要 run-pass / negative fixture，不使用 fixture-only workaround。
5. 运行 `CG-T02` 要求的验证命令及必要的相关回归；失败则修复当前任务范围内问题后重跑。
6. 更新 `TODO.md`：将 `CG-T02` 标题加 `[DONE]`，补完成记录和实际验证命令；仅在阶段计划变化时修改 `PLAN.md`。
7. 提交本次任务所有相关变更，提交信息以 `[CG-T02]` 开头，然后停止。

## 进度记录

- 已创建初始执行计划，下一步读取 `TODO.md` 确认首个未完成任务。
- 已确认首个未完成任务为 `CG-T02：收口 runtime type/value primitive LLVM lowering`；最新提交 `[CG-T01R] Review raw MIR route gate` 未提到直接相关未完成事项。
- 当前执行范围限定为 `is` / `!is` / `as` / `as?` / `!!` / pattern type test 的 refactor LLVM lowering 或明确 frontend reject，以及对应定向测试和完成记录。
- 已定位现状：HIR/legacy helper 已有 runtime descriptor/type-desc/itable 检查；refactor raw MIR rvalue 仍拒绝 `TypeCheck` / `Cast`，pattern `is Type` 已有直接 lowering；`!!` 已在 MIR 中展开成 Option pattern + `Raise<RuntimeError>`。
- 实施方案：在 MIR rvalue lowering 中新增 `TypeCheck`、`CastOp::As`、`CastOp::AsQ` 复用 helper；让 support gate 接受已可 lower 的 runtime primitive；补 `refactor_llvm_runtime_type_primitives` IR 测试，并将 `not_null_assert_basic.scoop` 从 expected-fail 回收为 run-pass。
- 2026-05-07 本次调用：已重新读取 `TODO.md`，确认首个未完成任务仍为 `CG-T02`；下一步按本次调用计划核对最新提交和当前代码状态。
- 2026-05-07：最新提交为 `e1780fd0 [CG-T01R] Review raw MIR route gate`，未明确提到与 `CG-T02` 直接相关的未完成事项；当前未提交变更仅有本计划文件。
- 2026-05-07：已开始实现 `CG-T02`：`TypeCheck` / `Cast` 不再由 raw backend gate 统一拦截，MIR lowering 新增 runtime type-test、`as` failure raise boundary、`as?` Option construction 的初版实现。
- 2026-05-07：已补 `refactor_llvm_runtime_type_primitives` 定向 IR 测试，并将 `tests/fixtures/run-pass/not_null_assert_basic.scoop` 从 expected-fail 回收为 expected-pass。
- 2026-05-07：首次运行定向测试暴露 `as` failure effect-exit 的 dead block 缺 terminator；已在 MIR/HIR cast failure lowering 的 effect-exit 后补 `unreachable`。
- 2026-05-07：`type_check_cast_is_as_asq_basic` 运行暴露 `as Base` 后 final method `Base.baseTag` 被错误保留为 virtual call；已修正 devirtualization，使不在 owner vtable 的 final class member 发布为 direct target，不在 backend 猜 vtable slot。
- 2026-05-07：同一 fixture 继续暴露 `as Other` failure 未进入本地 `try/catch`；已将 MIR `as` lowering 拆为 `TypeCheck` 成功分支与 `Raise.raise(ClassCastFailed)` Perform 失败分支，成功分支保留已验证 `CastOp::As` 投影。
- 2026-05-07 本次恢复：重新读取 `TODO.md`，首个未完成任务仍为 `CG-T02`。本次将先检查最新提交与工作区现状，确认是否存在前次未提交实现，然后继续完成 `CG-T02` 的实现、验证、`TODO.md` 完成记录和 `[CG-T02]` 提交。
- 2026-05-07：最新提交为 `e1780fd0 [CG-T01R] Review raw MIR route gate`，未提到与 `CG-T02` 直接相关的未完成事项；工作区已有 `CG-T02` 相关未提交改动，下一步审阅 diff 后继续验证和修复。
- 2026-05-07：已审阅现有 diff，改动集中在 MIR runtime type/cast lowering、`as` failure Raise 边界、`as?` Option 构造、enum unit variant 引用、`!!` fixture 回收和 gap gate 放开；下一步运行 `CG-T02` 定向验证。
- 2026-05-07：`refactor_llvm_runtime_type_primitives` 和 `type_check_cast_is_as_asq_basic.scoop` 已通过；`not_null_assert_basic.scoop` 失败于 late-lowering boundary contract，根因是 `!!` 合成的 `RuntimeError.NullAssertionFailed` HIR 表达式仍标为 `Any`，需要改为显式 `RuntimeError`。
- 2026-05-07：已将 `!!` 合成的 `RuntimeError.NullAssertionFailed` 标为显式 `RuntimeError`；`not_null_assert_basic.scoop` 现已通过，function type cast negative diagnostics 三个 fixture 也已通过。
- 2026-05-07：`cargo clippy --all-targets -- -D warnings` 首次失败仅剩两个新增 codegen helper 的 `too_many_arguments` lint；按现有同类 helper 风格添加局部 allow 后重跑。
- 2026-05-07：已格式化代码并重跑 `cargo clippy --all-targets -- -D warnings`，通过。
- 2026-05-07：已将 `TODO.md` 中 `CG-T02` 索引和标题标记为 `[DONE]`，并写入实现摘要与实际验证命令；下一步执行提交前 git 审阅并提交 `[CG-T02]`。
- 2026-05-07：提交前同步了既有 `CG-T01R` 索引行的 `[DONE]` 标记，使索引与其已完成任务标题一致；不改变任务顺序或阶段计划。
