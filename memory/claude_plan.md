# 执行计划

说明：我不会记录不可审计的详细内在推理，但会持续维护一份可检查的、可执行的步骤计划与关键进展。

## 当前计划

1. 检查最新提交信息，确认是否明确提到需要先修复的既有问题。
2. 阅读 `TODO.md` 与 `PLAN.md`，确定第一个未完成任务。
3. 如果首个任务过大，先把它拆成更小的子任务，并同步更新 `TODO.md` 与 `PLAN.md`。
4. 实现当前应执行的首个任务，只做这一项。
5. 运行与改动直接相关的测试；若暴露既有问题，先修复该问题或把它登记为前置任务并调整顺序。
6. 完成后更新 `TODO.md` 与 `PLAN.md`，记录结果与剩余依赖。
7. 按仓库提交风格创建一次 git 提交，然后停止。

## 进度记录

- 已创建本计划文件，后续会在关键步骤完成或计划调整时更新。
- 已检查最新提交 `e23657f [T5001d3R] Review explicit frame home-slot coverage`。提交信息本身没有额外声明一个尚未处理、必须先于计划任务修复的遗留 issue。
- 已读取 `TODO.md` 与 `PLAN.md`。当前首个未完成任务是 `T5001e1 收紧 safepoint clobber / reload contract，打通 post-safepoint ref 使用主线`。
- 当前判断：先检查现有 safepoint lowering、runtime helper call、effect/resume 边界与 LLVM 回归，确认是否还存在 post-safepoint 直接复用旧 SSA / register 的路径；若任务范围过大，再先拆分并回写 `TODO.md` / `PLAN.md`。
- 已确认一个需要当前任务修复的真实缺口：`with_conservative_gc_local_root_spills(...)` 已在 safepoint 前后把 relocated roots 写回 explicit frame home slots，但 `codegen_var_ref(...)`、`materialize_deferred_cg_value(...)`、`materialize_deferred_cg_value_for_call_arg_impl(...)` 这类 post-safepoint 消费路径仍优先从旧 local/spill slot reload，而不是从 explicit frame home slot reload。
- 执行调整：
  1. 为“单个 pointer-shaped GC 值”增加统一的 explicit-frame reload helper。
  2. 让普通 local 读取与 deferred scalar GC 值 materialize 优先从 home slot reload。
  3. 补充 LLVM 回归，锁定 direct local return 与 deferred call arg 两条 post-safepoint reload 路径。
  4. 运行相关测试与 lint；若暴露其它既有问题，先修复再继续。
- 已完成代码修改：
  1. `local_ptr_for_use(...)` 在 explicit frame 已启用且值是单槽 GC pointer 时，改为返回 explicit frame home slot。
  2. `materialize_deferred_cg_value(...)` 与 `materialize_deferred_cg_value_for_call_arg_impl(...)` 对同类值改为从 home slot reload。
  3. 新增两条 LLVM 回归，锁定 ordinary safepoint 后 direct local reload 与 deferred call arg 经后续 safepoint 后的 reload source。
- 已完成验证：`cargo test -p scoopc managed_function_reloads_direct_gc_local_from_explicit_frame_after_safepoint`、`cargo test -p scoopc deferred_call_arg_reloads_from_explicit_frame_after_later_safepoint`、`cargo test -p scoopc --lib`、`cargo run -p scoop -- test --fixtures tests/fixtures/build`、`cargo clippy -p scoopc --all-targets -- -D warnings` 均已通过。
- 下一步：检查 git diff / status，更新任务文档后创建一次提交，并停止。
