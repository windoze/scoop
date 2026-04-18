# 执行计划

说明：我不会记录不可公开的内部逐字思维链，但会在此文件持续维护可审计的执行计划、决策依据摘要、关键进展与变更。

## 初始计划

1. 检查最近一次提交，确认提交说明中是否提到已有问题；如果提到，先定位并修复这些问题。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 评估该任务规模：
   - 如果任务足够小，直接实现。
   - 如果任务过大或存在前置依赖，先更新 `PLAN.md` 与 `TODO.md`，拆成更小子任务，并只执行拆分后的第一个子任务。
4. 实现当前目标任务，必要时同步更新此文件记录关键步骤与决策。
5. 运行与该任务相关的测试，以及必要的格式化、lint、无警告检查。
6. 更新 `TODO.md` 与 `PLAN.md`，反映已完成内容或阻塞关系调整。
7. 提交 git commit，然后停止，不继续处理下一个任务。

## 当前状态

- 已检查最近提交：`9f2abc3 [T3016h] Fix outer-slot seeding for closure binders`。提交说明没有留下需先单独处理的未修复既有问题。
- 已读取 `TODO.md`，当前首个未完成任务为 `T3016hR`。
- 已判断 `T3016hR` 规模适中，不需要拆分 `PLAN.md` / `TODO.md`。

## 当前任务：T3016hR

目标：复审 stdlib/task adapter 路径上的 outer-local seeding 修复，确认它仍然严格依赖统一 frame metadata / slot seeding 合同，而不是为 `Executor.await`、`assertTrue`、`require` 或具体 fixture 增加 helper-specific 特判。

### 已完成的审查

1. 检查 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`：
   - `collect_outer_scope_slots()` 统一通过“已声明局部集合 declared”与“已使用局部集合 used”的差集生成 `seed_from_outer_scope` slot。
   - `collect_declared_local_ids_in_closure()` 会把 closure 显式参数，以及 resolver 注入的隐式单参 lambda binder `it` 一并记为 closure 内声明局部，然后继续递归 closure body。
2. 检查 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`：
   - `seed_outer_scope_frame_slots()` 与 `write_back_outer_scope_frame_slots()` 只读取统一 contract/frame metadata 中的 `seed_from_outer_scope`、`mutable`、`owner_arm` 等通用信息。
   - 当前未发现基于 stdlib helper、Task adapter、fixture 名称或源码形状的分支。
3. 定向检索生产代码：
   - `Executor.await`、`assertTrue`、`require`、`std_task_async_adapters_basic`、`stdlib_smoke_test_and_preconditions` 这些名称未出现在 effect 生产 lowering 代码里。
4. 检查新增结构测试：
   - `state_machine_transform.rs` 中已有针对“显式 closure 参数不应被当作 outer-scope seed slot”以及“隐式 it binder 不应被当作 outer-scope seed slot”的测试。

### 下一步

1. 更新后的状态已记录到 `TODO.md` 与 `PLAN.md`，接下来只剩确认工作区差异并提交。
2. 本轮完成后停止，不继续处理 `T3017`。

## 验证结果

已完成并通过：

1. `cargo test -p scoopc handle_outer_scope_seeding -- --nocapture`
2. `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/std_task_async_adapters_basic.scoop`
3. `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/stdlib_smoke_test_and_preconditions.scoop`
4. `cargo fmt --check`
5. `cargo test --all`
6. `cargo clippy --all-targets -- -D warnings`

## 当前结论

- `T3016hR` 复审通过。
- 未发现需要新增或前置的生产 blocker。
- `TODO.md` 已将 `T3016hR` 标记完成；`PLAN.md` 已把下一项推进到 `T3017`。
