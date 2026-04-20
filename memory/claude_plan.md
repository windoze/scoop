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
