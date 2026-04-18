# 执行计划与进度记录

## 说明

按要求先记录一份可审阅的计划与进度日志。这里保留的是执行思路摘要、决策依据、步骤分解与后续变更，不包含逐词内部推理。

## 本轮目标

完成 `TODO.md` 中第一个未完成任务，然后停止。

## 初始步骤

1. 检查最新一次提交，确认是否提到了尚未修复的已知问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 如该任务过大，则拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 实现当前应执行的任务。
5. 运行相关测试与必要的质量检查，至少覆盖本次改动影响范围；如任务要求或仓库状态允许，再补充更全面检查。
6. 更新 `TODO.md`、`PLAN.md` 与本文件的进度记录。
7. 提交 git commit，并停止，不继续下一个任务。

## 风险与约束

- 如果发现最新提交遗留问题，需要先修复，再继续当前任务流。
- 如果当前任务依赖尚未实现的语言特性、运行时能力或其他前置修复，不能绕过；必须先把依赖问题写入 `TODO.md`/`PLAN.md`，提交后停止。
- 不回退或覆盖仓库中与当前任务无关的现有改动。

## 进度日志

- 已创建本文件并写入初始计划，下一步开始检查最新提交与任务列表。
- 已检查最新提交 `2a705d8bc448ebbb9c875c1036c58b8a8bac3eaf`；提交说明未额外挂出新的“提交内遗留问题”，因此继续按 `TODO.md` 首个未完成项推进。
- 已定位当前首个未完成任务为 `T3016h`：修正 unified effect frame seeding 在 stdlib/task adapter 路径上遗漏 outer-scope local 的生产缺口。
- 已复现目标失败：`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/std_task_async_adapters_basic.scoop` 仍报 `unsupported_main_body: effect frame seed outer-scope local`。
- 已完成根因定位摘要：
  - 问题不在 stdlib helper 名称本身，而在统一 frame slot 收集逻辑。
  - `collect_local_refs_in_expr()` 会递归进入 closure body，把 closure 内部对局部名的引用都算进外层 handle/try 的 “used locals”。
  - 但 `collect_declared_local_ids_in_expr()` 对 `ExprKind::Closure` 只递归 body，没有把 closure 显式参数以及隐式 `it` 视为 closure 内声明。
  - 结果是外层 handle/try 会把 `v`、`acc`、`x`、隐式 `it` 等 lambda 形参误判成 outer-scope slots；后续 emitter 在 `seed_outer_scope_frame_slots()` 里去当前函数 `env` 查这些 symbol 时失败，从而报 `effect frame seed outer-scope local`。
- 已完成代码修复：
  - 在 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 中补齐 closure declared-local 收集，显式 lambda 参数与 synthetic `it` binder 都会先写入 declared set，再递归 closure body。
  - 在 `crates/scoopc/src/llvm/codegen/effect/state_machine_transform.rs` 中新增两条结构回归测试：
    - `handle_outer_scope_seeding_excludes_explicit_closure_params`
    - `handle_outer_scope_seeding_excludes_implicit_it_lambda_binder`
- 已完成验证：
  - `cargo test -p scoopc handle_outer_scope_seeding_ -- --nocapture`
  - `cargo test -p scoopc nested_handle_outer_scope_seeding_marks_only_real_outer_slots -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/std_task_async_adapters_basic.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/stdlib_smoke_test_and_preconditions.scoop`
  - `cargo fmt --check`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已完成文档同步：
  - `TODO.md` 中 `T3016h` 已标记为 `[DONE]` 并补记实现/验证进展。
  - `PLAN.md` 已记录本轮完成摘要，下一项推进到 `T3016hR`。
  - 两条目标 run-pass fixture 已从 stale `EXPECT: fail` 恢复为 `EXPECT: pass`。
- 下一步：
  1. 检查工作区变更。
  2. 以 `T3016h` 语义提交本轮改动。
  3. 停止，不进入下一项任务。
