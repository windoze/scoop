# 本轮执行计划

## 目标

完成 `TODO.md` 中第一个未完成任务；如果存在更基础的既有问题或规范缺口，先修复或将其前置为任务，再停止。

## 约束与执行原则

- 先检查最新提交是否提到遗留问题；若有，优先修复。
- 只处理一个任务（或当前任务拆分后的第一个子任务）。
- 不接受规避方案；若发现规范不匹配，必须在 `TODO.md` / `PLAN.md` 中显式建模依赖关系。
- 完成后必须更新 `TODO.md`、`PLAN.md`，运行相关测试，并提交 git commit。
- 变更过程中如果计划发生调整，会继续更新本文件。

## 初始步骤

1. 查看最新一次 git 提交信息，确认是否提到需要先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对当前任务背景、依赖和已有拆分。
4. 结合相关代码与测试评估任务复杂度。
5. 若任务过大，先拆分任务并更新 `TODO.md` / `PLAN.md`，本轮只执行拆分后的第一个子任务。
6. 实现当前目标任务。
7. 运行必要的格式化、测试、lint（至少覆盖受影响范围，并满足无 warning 约束）。
8. 更新 `TODO.md`、`PLAN.md`、本文件，记录已完成内容或阻塞原因。
9. 使用清晰的提交信息创建 git commit，然后停止。

## 预期检查项

- 最新提交是否提到 bug / follow-up / known issue。
- 当前首个未完成任务是否依赖尚未实现的语言特性、运行时能力或规范修复。
- 相关模块是否已有测试覆盖；若没有，需要补充测试。
- 是否需要同步更新根 `README.md` 或内联注释以保持文档完整性。

## 当前判断

- 最新提交 `6d4d15c44759b45e087f571fe2d1ed5e607727ee` 的 commit message 未显式声明新的 follow-up issue；当前没有发现必须先于 `TODO.md` 主线处理的“提交中自带遗留问题”。
- `TODO.md` 中首个未完成任务是 `T2003r1c`：`Effect：segmenting 覆盖 nested-while / richer matrix，并冻结 builder 输入契约`。
- 结合 `PLAN.md` 与现有代码，`T2003r1c` 当前更像“补全 segment 阶段覆盖面并显式锁定 segment list 不变量”，而不是继续扩旧 shape-based lowering，因此本轮可以直接实现，不需要再拆子任务。

## 本轮实施方案

1. 审阅 `crates/scoopc/src/llvm/codegen/effect/state_machine_segments.rs` 与 `state_machine_plan_tests.rs`，确认现有 segment dump 已覆盖的场景与仍缺的 nested-while / richer mixed representative samples。
2. 在 `state_machine_segments.rs` 中补充“builder 输入契约”层面的显式校验逻辑，确保下一阶段 builder 可只依赖 `HandleSegmentList` 的 segment id / edge / suspend / dispatch / cleanup / arm-body 关系，而不需要回看源码形状。
3. 在 `state_machine_plan_tests.rs` 中新增 segment dump / contract 回归：
   - nested-while representative sample；
   - richer mixed direct/indirect representative sample；
   - 必要时增加 contract 校验断言，锁定 builder-only input 约束。
4. 运行定向测试与 lint，至少覆盖：
   - `cargo test -p scoopc segment_dump_`
   - 视新增命名再补 `cargo test -p scoopc segment_contract_` 或更具体过滤
   - `cargo test -p scoopc plan_dump_`
   - `cargo clippy --workspace --all-targets -- -D warnings`
5. 任务完成后更新 `TODO.md`、`PLAN.md`、本文件，并创建对应 git commit，然后停止。

## 状态

- 已完成 `T2003r1c` 代码修改：
  - 在 `state_machine_segments.rs` 中新增 `HandleSegmentList::validate_builder_contract`，显式校验 segment / edge / suspend-site / dispatch-entry / arm-body / cleanup-scope 的引用完整性与上下文一致性。
  - 在 `state_machine_plan.rs` 的 `build_handle_state_machine_plan` 中加入 debug 校验，确保统一 segment projection 在 ground-up rewrite 期间持续满足 builder-only 输入契约。
  - 在 `state_machine_plan_tests.rs` 中新增 nested-while 与 richer mixed while（direct + indirect）segment dump 回归，并让 `build_segment_dump` 统一执行 contract 校验。
- 已完成验证：
  - `cargo test -p scoopc segment_dump_`
  - `cargo test -p scoopc plan_dump_`
  - `cargo fmt --all`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 已完成状态同步：`TODO.md` / `PLAN.md` 已将 `T2003r1c` 标记为完成，并记录新增的 builder contract 校验与回归测试。
- 下一步：检查最终 diff，提交 git commit，然后停止。
