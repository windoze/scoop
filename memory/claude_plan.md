# 执行计划与进度记录

## 说明

按要求先记录执行计划与后续进度。这里记录的是可审计的计划、判断依据和关键决策，不包含不可审计的内部推理细节。

## 初始计划

1. 检查最近一次 Git 提交，确认提交说明里是否提到已有但未修复的问题。
2. 如最近一次提交暴露出仍未处理的问题，先定位并修复这些问题，再继续后续步骤。
3. 读取 `TODO.md`，找出第一个未完成任务。
4. 判断该任务是否足够小且可以在本轮完整交付。
5. 如果任务过大，则把它拆成更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，随后只执行拆分后的第一个子任务。
6. 实现当前目标任务。
7. 运行与改动相关的验证：
   - 最小必要测试
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 如任务涉及格式或夹具，再运行相应命令
8. 更新文档与任务状态：
   - 更新 `TODO.md`
   - 更新 `PLAN.md`
   - 必要时继续更新本文件中的进度记录
9. 检查工作区变更，确保没有误改或遗漏。
10. 使用清晰的提交信息提交本轮改动，然后停止，不进入下一项任务。

## 进度记录

- 已创建本文件并写入初始计划。
- 已检查最近一次提交 `d995dabe42ddbd9abe7baeb5fa3bd0795fb3b4cf`，提交说明仅为 `[T3009b0a] Front-load outer-slot writeback blocker`，未附带额外“已知未修复问题”说明，因此当前无需在任务外先修新的提交遗留问题。
- 已读取 `TODO.md` 与 `PLAN.md`，确认第一个未完成任务为 `T3009b0a`：把 unified handle frame 中 outer-scope `var` slot 的写回接回 enclosing locals。
- 当前判断：先不继续拆分 `T3009b0a`。需要先阅读 `state_machine_emitter`、frame slot metadata 与相关 fixture，确认缺口是否集中在“handle 完成后的统一写回路径”；若实现面超出单轮闭环，再回头拆分并同步更新 `PLAN.md` / `TODO.md`。
- 已完成实现面定位：
  - `seed_outer_scope_frame_slots` 只负责把 outer locals/params 拷入 handle frame。
  - `handle_done` 当前只清理 TLS / handler stack 并读取结果，没有把 frame 中被修改的 outer mutable slot 写回 enclosing locals。
  - `handle_propagate` 当前会直接 outward propagate，也没有写回；若不在这里同步，则 finally/arm 对 outer `var` 的改动会在 outward propagation 场景下丢失或只停留在 frame 副本里。
- 当前实施方案：
  1. 在 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 中新增统一 helper，按 frame metadata 遍历 slot，只同步 `seed_from_outer_scope && mutable && owner_arm == None` 的 slot。
  2. 在 `handle_done` 与 `handle_propagate` 两个出口都调用该 helper，确保普通 body、arm body、finally 都经过同一条 authoritative writeback 路径。
  3. 先跑 `effect_escape_continuation_resume_unit.scoop` / `effect_escape_continuation_resume_string.scoop` 验证 blocker 是否解除；若这些 fixture 因修复而转绿，再决定是否同步移除对应的 stale xfail 标记。
- 新发现的关键事实：
  - 仅补 emitter writeback 还不够。复跑 `effect_escape_continuation_resume_unit.scoop` / `..._string.scoop` 后，输出仍然是 `missing`。
  - 根因进一步定位到 plan builder：`HandleStateMachinePlan::build()` 当前只对 `handle.body.stmts` 调用 `collect_outer_scope_slots(...)`，没有把 arm body / `finally` 中引用的 outer locals 纳入 outer seeded slots。
  - 因而 `saved` 这类“只在 escape arm 中赋值”的外层 `var` 根本没有进入 frame authoritative slot，后续 writeback helper 自然无从同步。
- 计划调整（仍在 `T3009b0a` 范围内，不单独拆任务）：
  1. 扩展 outer-scope slot 收集范围，从仅 `handle.body` 改为覆盖整个 handle：body、arms、finally。
  2. 显式排除 handle 内声明的局部（body locals、arm binder / resume / continuation locals、finally locals），避免把 handle 内部局部误标为 outer seeded slot。
  3. 保留已接上的统一 writeback helper，再次验证最小 repro。
- 当前实现进度：
  - 已在 `state_machine_emitter.rs` 中新增 metadata 驱动的 outer-slot writeback helper，并接到 `handle_done` / `handle_propagate` 两个出口。
  - 已在 `state_machine_plan.rs` 中把 outer-scope slot 收集范围扩展到整个 `handle`（body、arms、finally），并显式排除 arm binder / resume / continuation locals 以及 handle 内局部。
  - 已新增结构测试 `handle_outer_scope_seeding_includes_arm_and_finally_locals`，锁定“只在 arm/finally 中引用的 outer local 也必须 seed；`k` 之类 arm 局部不得误入 outer seeded slot”。
  - 已新增 focused fixture `effect_escape_continuation_outer_var_writeback_basic.scoop`，覆盖 body/arm/finally 三类 outer `var` 写回；该 fixture 当前通过。
  - 复跑 `effect_escape_continuation_resume_unit.scoop` / `..._string.scoop` 后，`saved` 不再丢失，输出已从 `missing` 推进到 resumed body 执行阶段；剩余“`resume(...)` 返回后未继续执行 caller tail”的问题属于下一任务 `T3009b0` 的 dedicated resume lowering / return path 范围。
- 已完成验证：
  - `cargo test -p scoopc handle_outer_scope_seeding_includes_arm_and_finally_locals -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_outer_var_writeback_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已完成文档更新：
  - `TODO.md` 已将 `T3009b0a` 标记为完成，并记录 focused fixture 与剩余 blocker 归属 `T3009b0`。
  - `PLAN.md` 已记录本轮完成情况，并把下一步执行顺序推进到 `T3009b0aR -> T3009b0 -> ...`。
