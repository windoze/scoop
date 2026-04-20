# 执行计划

说明：我会把可公开的执行计划与进度记录在这里；不会写入内部私有推理细节，但会完整记录步骤、决策结果、阻塞点与后续动作。

## 初始计划

1. 检查最新一次 Git 提交的信息，确认是否提到已知遗留问题。
2. 如果最新提交提到需要先修复的遗留问题，优先定位并修复这些问题，再继续后续任务流。
3. 读取 `TODO.md`，识别第一个未完成任务。
4. 判断该任务是否过大：
   - 如果可直接完成，则进入实现；
   - 如果过大，则在 `PLAN.md` 中拆分为更小的子任务，并同步更新 `TODO.md`，本次只执行拆分后的第一个子任务。
5. 阅读与该任务相关的代码、测试、规范和计划文件，确认实现边界与依赖。
6. 按规格实现该任务；如果遇到任何会导致“绕过实现缺口”的情况，停止采用变通方案，转而把真实缺口整理为新的前置任务，并更新 `TODO.md` / `PLAN.md`。
7. 运行相关验证：
   - 至少运行与修改相关的测试；
   - 如适用，运行 `cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 或足以证明本次修改正确性的更小范围命令。
8. 更新文档与任务状态：
   - 在 `TODO.md` 中标记本次完成的任务；
   - 在 `PLAN.md` 中记录当前状态、后续依赖与必要调整；
   - 持续更新本文件记录关键进展。
9. 使用清晰的提交信息提交本次变更。
10. 完成首个未完成任务后立即停止，不继续处理下一个任务。

## 进度记录

- 已创建计划文件，等待开始检查最新提交与任务列表。
- 已检查最新提交 `f521c9edec8f573a26bf4b200431a7278b08ecf9`，提交说明为“`[T4010b1a1] Insert blocker for true typecheck_multi value member resolution`”。
- 已读取 `TODO.md` / `PLAN.md`，确认当前第一个未完成任务是 `T4010b1a1`：补齐真实 `typecheck_multi` 编译单元下的跨文件值成员解析。
- 当前判断：该任务边界清晰，先不再拆分；本轮直接以修复真实跨文件 generic value member access 为目标推进。
- 下一步：
  1. 读取 `T4010b1a1` 的任务说明与失败 fixture；
  2. 排查 `typecheck_multi` 编译单元下 member candidate 收集与 late resolve 路径；
  3. 实现修复并补充/更新必要回归；
  4. 跑定向验证、全量测试、`clippy`；
  5. 更新 `TODO.md` / `PLAN.md`，提交本轮变更后停止。
- 已完成实现：
  1. 扩展 `collect_struct_field_types`，让它在真实多文件编译单元中收集 foreign AST 里的 ctor 字段、body property 与 getter-only property，并对 generic owner 保留可后续具体化的占位类型。
  2. 调整 member late resolve 的存在性判断，改为基于 `Index.by_fqn` 的 value symbol + 可见性，而不是把“能否晚解析”绑定在字段类型表是否已收集。
  3. 更新真实回归 fixture `tests/fixtures/typecheck_multi/generic_value_member_access_cross_file`，补上 body property 覆盖。
  4. 为 fixtures driver 增加回归保护：当 `--fixtures` 直接指向 `typecheck_multi/<case>` 目录时，按多文件 case 执行而不是退回 parse。
- 已完成验证：
  - `cargo test -p scoopc collect_struct_field_types_includes_foreign_body_properties -- --nocapture`
  - `cargo test -p scoop run_all_treats_typecheck_multi_case_root_as_single_case -- --nocapture`
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck_multi/generic_value_member_access_cross_file`
  - `cargo run -q -p scoop -- test`
  - `cargo test --all -- --test-threads=1`
  - `cargo clippy --all-targets -- -D warnings`
- 当前结果：`T4010b1a1` 已完成；下一项应为 `T4010b1b`。本轮接下来只更新任务状态并提交，不继续进入下一任务实现。
