# 执行计划

说明：我不会把完整的内部推理逐字展开，但会在这里持续维护可检查的执行计划、关键判断依据、当前进度与变更记录。

## 当前目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果在执行前或执行中发现已有问题、回归、规格不匹配或实现边界缺口，则先修复该问题，或者按要求把它整理为前置任务写回 `TODO.md` / `PLAN.md`，然后停止。

## 初始步骤

1. 检查最新一次 Git 提交信息，确认是否提到了待修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解当前计划与任务上下文。
4. 检查工作树状态，识别是否已有未提交改动，避免覆盖用户改动。
5. 评估第一个未完成任务是否可在本轮完整完成。
6. 如果任务过大，则把它拆成更小的子任务，并更新 `TODO.md` / `PLAN.md`。
7. 实现当前应执行的那个任务。
8. 运行相关测试与质量检查；若发现已有问题，立即修复或转化为前置任务。
9. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或阻塞原因。
10. 提交 Git commit，然后停止。

## 进度记录

- 已创建初始计划文件，下一步将检查最新提交与任务列表。
- 已检查最新提交 `3432ab25c7ef3abe890f64d620762685f7a4092f`，提交正文未额外声明需要先修的既有问题。
- 已定位第一个未完成任务为 `T5000e1b0bR Review：确认 type-body generic member fun 已进入 generic MIR template → instance materialization 主线`。
- 正在复核的关键点：
  - `dump-mir` 是否通过 `lowered_hir.member_funs` 发射真实 member fun MIR root；
  - `materialize_for_dump(...)` 的 template catalog / canonical lookup / instance cache 是否把 type-body member fun 与顶层 / extension fun 同层处理；
  - `eff_args` 是否不仅停留在 call binding，而是真正进入 `InstanceKey` 与 concrete instance FQN。
- 复核过程中发现新的前置缺陷（必须先修）：
  - `dump-mir` / `dump-ir` 对 `TypeName.member()` 的 companion member call 仍会在 typecheck 阶段把 receiver 当普通值表达式处理；
  - 复现实例：
    - `/tmp/t5000e1b0br_companion_plain.scoop` 中 `Box.forward()`；
    - `/tmp/t5000e1b0br_companion_member.scoop` 中 `Box.forward<eff E>()`；
  - 当前错误：
    - `scoop::typecheck::unsupported_expr`
    - `暂不支持的表达式类型检查：ident（未 resolve）`
- 该缺陷为何阻塞当前任务：
  - `T5000e1b0b` / `T5000e1b0bR` 的范围明确包含 type-body / companion object 内的 generic member fun；
  - 如果 `TypeName.member()` 这条 companion dispatch 主线在 typed dump 路径本身就失败，就不能声称已完成“generic MIR template -> instance materialization 主线”的 review。
- 修复计划已调整为：
  1. 先修 companion member call 的 typed typecheck / HIR lowering 接线；
  2. 再补 review 所需的 companion generic member 回归测试；
  3. 跑定向验证与全量质量检查；
  4. 通过后再回写 `TODO.md` / `PLAN.md` 完成 `T5000e1b0bR`。
- 已完成修复：
  - `typecheck/expr/call.rs` 已为 unresolved type receiver 恢复 companion object nominal receiver type；
  - `hir/lower/expr.rs` 已把 companion member direct-call 改写为带 companion singleton receiver 的顶层 direct-call；
  - 已新增 typed HIR / MIR / materialize 三条回归测试，覆盖 companion generic member direct-call、generic MIR root 发射与不同 effect-row 实例身份。
- 已完成验证：
  - `cargo test -p scoopc companion -- --nocapture`
  - `cargo run -q -p scoop -- dump-mir /tmp/t5000e1b0br_companion_plain.scoop`
  - `cargo run -q -p scoop -- dump-mir /tmp/t5000e1b0br_companion_member.scoop`
  - `cargo run -q -p scoop -- dump-ir /tmp/t5000e1b0br_companion_member.scoop`
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/mir`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 全部通过。
- 文档/任务状态已更新：
  - `TODO.md` 已将 `T5000e1b0bR` 标记为完成，并记录 review 期间修复的 companion dispatch 前置缺陷；
  - `PLAN.md` 已补记本轮 review 的结论与验证结果；
  - 下一条待执行任务已切换为 `T5000e1bR Review：确认 effect-row 实参已成为 InstanceKey / materializer 的一等维度`。
