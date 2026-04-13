# 本轮执行计划

更新时间：2026-04-13

## 目标

完成 `TODO.md` 中第一个未完成任务，并在完成后停止。

## 初始计划

1. 检查最新一次 Git 提交，确认是否提到了需要先修复的既有问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，对照当前计划与任务依赖关系。
4. 如果首个未完成任务过大或存在前置依赖缺口：
   - 将任务拆分为更小的子任务。
   - 更新 `PLAN.md`。
   - 更新 `TODO.md`，把新的前置子任务排到正确顺序。
   - 本轮只执行拆分后排在最前面的那个任务。
5. 实施任务所需的代码修改。
6. 运行相关验证：
   - 至少运行与变更直接相关的测试。
   - 如有必要，运行更广范围的回归测试。
   - 检查格式、编译、lint，尽量满足无警告要求。
7. 更新文档与任务状态：
   - 在 `TODO.md` 中标记本轮完成的任务。
   - 在 `PLAN.md` 中反映当前状态、后续依赖或新增问题。
   - 在本文件中补充关键进展与计划变更。
8. 提交 Git commit，提交信息明确描述本轮完成内容。
9. 停止，不继续处理下一个任务。

## 执行原则

- 不使用规避性实现，不以临时兼容或仅测试夹具通过作为完成标准。
- 如果发现规范不匹配、缺失语言特性、已有缺陷或前置依赖缺口：
  - 先把该问题转化为 `TODO.md` 中更靠前的任务。
  - 更新 `PLAN.md` 与本文件说明阻塞原因。
  - 提交这些计划调整后停止。
- 不回退或覆盖我未创建的现有修改。

## 进展记录

- 已创建本文件并写入初始计划。
- 已检查最新提交、`TODO.md`、`PLAN.md`。
- 最新提交未显式引入一个必须先单独修复的遗留问题；当前首个未完成任务原本是 `T2003u5`。
- 已确认 `T2003u5` 过大，不能在一轮内稳妥完成；现已将其拆成 `T2003u5a`～`T2003u5d`，分别处理：
  - `multiple escape arms + sibling non-resuming + finally`
  - single-arm immediate-resume 的 while-nested replay
  - no-immediate multiple-escape 的 while direct/indirect separate-stmt mixed replay
  - immediate+escape mixed-arm 的 while richer matrix replay
- 本轮实际执行目标已收口为 `T2003u5a`：打通 top-level direct single-site 的 `multiple escape-continuation arms + sibling non-resuming + finally`。
- 已完成 `T2003u5a` 实现：
  - `crates/scoopc/src/llvm/codegen/effect/multi_escape.rs` 中已移除显式门禁。
  - main body 的 no-match dispatch、sibling catch body 的成功/向外传播路径、escape arm unwind 路径都已接到 `finally` 收口。
  - continuation step 保持既有语义，不会在 `resume(...)` replay 中重复执行 `finally`。
- 已完成回归迁移：
  - 删除 build-fail `effect_multi_escape_multi_arm_with_nonresuming_finally_is_error.scoop`。
  - 新增 run-pass `effect_multi_escape_multi_arm_with_nonresuming_finally.scoop`。
  - 新增 run-pass `effect_multi_escape_multi_arm_with_nonresuming_finally_raise.scoop`。
- 已完成验证：
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 下一次调用应从 `T2003u5b` 开始。

## 当前实现计划

1. 修改 `TODO.md` / `PLAN.md`，把 `T2003u5` 拆成子任务并把 `T2003u5a` 置于当前执行位置。
2. 审计 `crates/scoopc/src/llvm/codegen/effect/multi_escape.rs` 中该组合的显式门禁与 cleanup 路径。
3. 实现 `finally` 与 sibling non-resuming 共存时的主路径、catch 路径、arm unwind 路径收口，确保 `finally` 只在离开 source-handle 时执行一次。
4. 把现有 build-fail `effect_multi_escape_multi_arm_with_nonresuming_finally_is_error` 转成 run-pass 或等价正向回归。
5. 运行相关测试与 lint。
6. 完成后更新 `TODO.md` / `PLAN.md` / 本文件，并提交 commit。

## 本轮结果

- `T2003u5a` 已完成。
- 已更新 `TODO.md` / `PLAN.md`，使下一未完成任务变为 `T2003u5b`。
