# 执行计划记录

说明：按安全与协作要求，这里记录可审计的执行计划、决策摘要和进度更新，不记录不可验证的内部推理细节。

## 初始计划

1. 检查最新一次 Git 提交，确认提交说明中是否提到已知问题、回归、临时修复或待补修项。
2. 如果最新提交提到前置问题，先定位并修复该问题，再继续后续步骤。
3. 读取 `TODO.md`，识别第一个未完成任务。
4. 读取 `PLAN.md`，确认该任务的上下文、约束和依赖。
5. 判断该任务是否过大：
   - 如果可直接完成，则进入实现。
   - 如果过大，则先拆分为更小子任务，更新 `PLAN.md` 与 `TODO.md`，然后执行拆分后的第一个子任务。
6. 实现当前目标任务，并在实现过程中检查是否暴露出既有缺陷、规格不匹配或实现边界问题。
7. 对任何发现的既有问题：
   - 若可直接修复，则先修复再继续当前任务。
   - 若构成前置依赖且本轮不宜直接完成，则在 `TODO.md` 中前置新增任务、在 `PLAN.md` 中记录原因，并按要求停止。
8. 运行相关验证，至少覆盖：
   - 针对变更的测试；
   - 必要的工作区测试；
   - `cargo clippy --all-targets -- -D warnings`（若与本次改动相关且可执行）。
9. 更新文档与任务状态：
   - 在 `TODO.md` 中标记完成；
   - 在 `PLAN.md` 中更新当前状态与后续顺序；
   - 在本文件记录已完成步骤与任何计划调整。
10. 生成一次 Git 提交，提交信息与任务对应，然后停止，不进入下一个任务。

## 进度日志

- 已创建本文件并写入初始执行计划。
- 已检查最新提交 `e34593163e945367f1e2b9c5b4ac5250d9e35e24 (Update plan)`：
  - 提交说明本身未声明新的既有缺陷需要优先修复；
  - 提交 diff 主要是把后续若干条目显式标记为 `[TODO]`，未引入新的“先修 bug”说明。
- 已读取 `TODO.md` 与 `PLAN.md`，确认当前第一个未完成任务为 `T5000j1b 收口 user-defined compareTo 比较 target，并删除剩余 struct member eager inclusion`。
- 当前执行焦点：
  1. 盘点 `< <= > >=` 经 `compareTo` 的 typed HIR / MIR / materialization / LLVM codegen 路径；
  2. 找出 `llvm/emit.rs` 中仍因 operator overload 保留的 struct member eager inclusion；
  3. 在不引入 workaround 的前提下完成 `T5000j1b`，然后运行验证、更新 `TODO.md` / `PLAN.md` 并提交。
- 已完成实现策略判断：`T5000j1b` 规模可直接在本轮完成，无需再拆子任务。
- 当前实现方案：
  1. 在 `typecheck/expr/ops.rs` 为 user-defined `compareTo` 比较路径补齐与普通 operator direct-call 一致的 overload 选择、effect 记录、monomorph request 与 `TopLevelFunCallBinding` 写回；
  2. 在 `hir/lower/expr.rs` 将 `< <= > >=` 的 user-defined `compareTo` 站点降为“显式 direct-call + `SynthInt(0)` 的普通整数比较”；
  3. 删除 `llvm/emit.rs` 中仅为 `compareTo` 保留的 struct member eager inclusion；
  4. 增加回归，覆盖 typed HIR / production LLVM / monomorph binding 三个层面；
  5. 跑格式化、测试、clippy 与 fixture，再更新 `TODO.md` / `PLAN.md` 并提交。
- 已完成代码修改：
  - `typecheck/expr/ops.rs`：`compareTo` 比较现在会记录 direct-call binding / monomorph request，而不是只在 typecheck 阶段返回 `Bool`；
  - `hir/lower/expr.rs`：`< <= > >=` 的 user-defined `compareTo` 站点现在会降为“direct-call + `SynthInt(0)` 比较”；
  - `llvm/emit.rs`：已删除仅为 `compareTo` 保留的 struct member eager inclusion；
  - 新增 LLVM / materialization 回归，覆盖 compareTo 的 direct-call 形状、IR reachability 以及 owner specialization / eff-arg 保留。
- 已完成第一轮针对性验证：
  - `cargo fmt --all`：通过；
  - `cargo test -p scoopc compare_to -- --nocapture`：通过。
- 在继续验证时暴露并修复了一个既有问题：
  - `typecheck/expr/stmt.rs` 的 `check_if_expr_stmt` 之前不会对条件表达式做 `infer`；
  - 这会导致 statement-position `if (lhs < rhs)` 里的 compareTo 站点不写回 typed side table，进而让 dump-mir / generic MIR 看不到 direct-call target；
  - 现已在该入口补齐条件表达式推导，并新增 `dump_mir_lowers_compare_to_in_if_condition_as_direct_call` 回归覆盖该真实路径。
- 已完成最终验证：
  - `cargo fmt --all`：通过；
  - `cargo test -p scoopc typed_hir_fixture_preserves_compare_to_direct_call_binding -- --nocapture`：通过；
  - `cargo test -p scoopc dump_mir_lowers_compare_to_in_if_condition_as_direct_call -- --nocapture`：通过；
  - `cargo test -p scoopc compare_to -- --nocapture`：通过；
  - `cargo test -p scoopc frontend_codegen_consumes_compare_to_direct_calls_without_eager_member_inclusion -- --nocapture`：通过；
  - `cargo test -p scoopc`：通过；
  - `cargo test --all`：通过；
  - `cargo clippy --all-targets -- -D warnings`：通过；
  - `cargo run -p scoop -- test`：通过（`fixtures: ok (1202)`）。
- 当前剩余步骤：
  1. 复核最终 diff；
  2. 生成与 `T5000j1b` 对应的 Git 提交并停止。
