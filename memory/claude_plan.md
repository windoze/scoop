# 执行计划

说明：按安全约束，这里记录的是可审阅的高层执行计划、关键判断和进度，不包含逐字内部推理。

## 初始计划

1. 创建本文件，作为本轮工作的计划与进度日志。
2. 检查最新一次 Git 提交，确认是否提到任何已知问题；如果提到了，需要先纳入本轮处理范围。
3. 阅读 `TODO.md`，定位第一个未完成任务。
4. 如该任务过大，拆分为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`，随后只执行拆分后的第一个子任务。
5. 实现当前目标任务，必要时补充或调整测试。
6. 运行相关验证，至少覆盖：
   - 直接相关测试
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   如发现问题，先修复再继续。
7. 更新文档与任务状态：
   - 在 `TODO.md` 中标记当前任务完成，或在受阻时按要求重排任务。
   - 在 `PLAN.md` 中记录当前状态、依赖与后续顺序。
   - 持续更新本文件，记录关键步骤完成情况或计划变化。
8. 使用清晰的 Git 提交信息提交本轮变更。
9. 停止，不继续处理下一个任务。

## 当前状态

- 已创建计划文件。
- 已检查最新提交：`[T4010b1a] Instantiate generic value member access result types`；提交说明未显式要求先修其它 issue，但已把下一项推进点更新为 `T4010R`。
- 已读取 `TODO.md` / `PLAN.md`，确认本轮原始目标为 `T4010R`。
- 在执行 `T4010R` review 时发现新的前置 blocker，当前不能直接完成该 review。

## 关键发现

- 最小 probe `struct Point(var x: Int, val y: Int)` 当前可成功 build 并运行，程序退出码为 `3`。
- 这与规范中“所有 value type 都是 immutable；`var` 只能重绑定槽位、不能让值类型字段可写”的约束冲突。
- 初步根因：
  - parser 已把主构造参数 `val/var` 写入 `ast::Param.kind`。
  - `typecheck::structs::check_one_struct_fields` 仍沿用旧假设，只收字段名、不拒绝 `struct` 主构造参数上的 `var`。
  - `typecheck::expr::collect::collect_member_mutabilities_in_type_decl` 会把这类 ctor `var` 继续记成 mutable member，导致 `p.x = 7` 这类写回在 typecheck 阶段也会漏网，直到 LLVM 才报 `assignment lhs` unsupported。

## 计划调整

1. 在 `TODO.md` 中把该问题前插为新的 blocker 任务 `T4010b1b`，并把 `T4010R` 顺延到其后。
2. 在 `PLAN.md` 中记录发现过程、根因和新的执行顺序。
3. 本轮不继续做生产代码修复；按要求提交“任务重排 + 计划更新”后停止，等待下一轮从 `T4010b1b` 开始。

## 2026-04-20 本轮：T4010b1b

### 本轮目标

1. 完成 `T4010b1b`：禁止 `struct` 主构造参数上的 `var`，并阻止其继续泄漏到 value member mutability。
2. 让最小 probe `struct Point(var x: Int, val y: Int)` 在前端静态阶段失败，而不是继续 build/run 成功。
3. 补充 regression，至少覆盖：
   - `struct` 主构造参数 `var` 的静态报错。
   - 不因该语法漏网而把值类型字段写回错误地留到 LLVM 才失败。
4. 完成相关验证后，更新 `TODO.md` / `PLAN.md` / 本文件并提交一次 Git commit，然后停止。

### 当前判断

- 最新提交 `[T4010b1b] Insert blocker for struct ctor var immutability leak` 只是把 blocker 正式插入 `TODO.md` / `PLAN.md`，没有留下额外待修的历史 issue；因此本轮直接从 `T4010b1b` 开始。
- 当前工作树干净，`TODO.md` 中第一条未完成任务确认为 `T4010b1b`。
- 已知根因来自两处：
  - `typecheck::structs::check_one_struct_fields` 没有拒绝 `struct` 主构造参数上的 `var`。
  - `typecheck::expr::collect::collect_member_mutabilities_in_type_decl` 仍会把这类参数记成 mutable member。

### 执行步骤

1. 阅读 `typecheck/structs.rs`、`typecheck/expr/collect.rs`、相关错误诊断定义和已有 fixture。
2. 在前端统一禁止 `struct` 主构造参数 `var`，并确保 member mutability 收集与该规则一致。
3. 新增或更新 typecheck / run-pass fixture，验证报错位置与字段写回不再漏到 LLVM。
4. 运行定向测试，再跑 `cargo test --all -- --test-threads=1` 与 `cargo clippy --all-targets -- -D warnings`。
5. 完成后更新 `TODO.md`、`PLAN.md` 与本文件的进度记录，并提交 `[T4010b1b] ...`。

### 本轮实际进展

- 已先完成本轮最小验证：
  - `cargo test -q -p scoopc struct_primary_ctor_var_does_not_mark_member_mutable -- --nocapture`：通过。
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck`：通过，`fixtures: ok (348)`。
  - `cargo test --all -- --test-threads=1`：通过。
  - `cargo clippy --all-targets -- -D warnings`：通过。
- 在补跑完整 `cargo run -q -p scoop -- test` 时，发现一个更前置、且会阻断当前任务收口的真实 blocker：
  - 真实 `typecheck_multi` case `tests/fixtures/typecheck_multi/generic_value_member_access_cross_file` 失败，报 `member access（未 resolve）`。
  - 该 case 单独以 `typecheck_multi/<case>/` 目录形态运行时同样失败；之前 `T4010b1a` 使用的命令只是把 case 目录当普通 fixtures root 运行，没有覆盖真实多文件编译单元入口。
  - 根因已缩窄到 `collect_struct_field_types` 的跨文件补全只涵盖 primary ctor 字段，未覆盖定义在另一文件 type body 里的值成员 / getter-only property，因此 `Box(9).readBack` 无法进入 late member resolution。

### 计划变更

1. 当前不提交 `T4010b1b` 的生产代码；已撤回本轮临时实现，避免在 blocker 之前落下未完成任务的部分改动。
2. 在 `TODO.md` 中于 `T4010b1b` 前新增 blocker `T4010b1a1`，先补齐真实 `typecheck_multi` 编译单元下的跨文件值成员解析。
3. 在 `PLAN.md` 中同步记录这次发现、误测原因和新的执行顺序。
4. 本轮仅提交任务重排与计划更新，然后停止；下一轮从 `T4010b1a1` 开始。
