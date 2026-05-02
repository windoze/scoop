## 当前执行计划

1. 读取 `TODO.md`，确认索引指向哪些详细任务文件。
2. 按索引顺序检查对应的 `TODO-Px.md`，定位第一个标题未带 `[DONE]` 的详细任务。
3. 阅读该任务的要求、约束、依赖、验证方式，并检查最近一次提交是否存在与该任务直接相关的未完成事项。
4. 实现该任务；如果遇到阻塞当前任务的真实前置问题，则先按要求在对应 `TODO-Px.md` / `TODO.md` 中补充最小前置任务并停止。
5. 运行与该任务直接相关的测试与必要的质量检查，修复发现的问题。
6. 更新 `memory/claude_plan.md` 记录进展，更新对应 `TODO-Px.md` 的完成记录，并在需要时同步 `TODO.md` / `PLAN.md`。
7. 按任务编号创建一次 git 提交，然后停止，不继续下一个任务。

## 进度记录

- 已写入初始执行计划，下一步开始读取 `TODO.md` 与详细任务文件。
- 已确认首个未完成详细任务为 `P5-T04b`（`TODO-P5.md`）。
- 已检查最近一次提交 `[P4-T05b] Separate continuation surface_ty from step upper bound`，它与当前任务直接相关，但提交本身没有遗留新的未完成 prerequisite。
- 当前实现缺口：`crates/scoopc/src/effect_lowered/` 里的 builder/IR/dump 主要只保留了 internal step schema 视角，尚未把 `ContinuationSchema.surface_ty` 作为 source-visible contract 明确留在 P5 shell/dump 中，也没有通过 `ContinuationSchema.out_step_schema()` 显式锁定 resume interface 的返回 step contract。
- 下一步：
  1. 修改 `effect_lowered` IR/builder/dump，使 continuation shell 同时显式记录 `surface_ty` 与 `out_step_schema`，并由 builder 校验 internal contract 来自 `ContinuationSchema`。
  2. 新增/更新定向测试，覆盖“`surface_ty` 为 `Pure`/`Boom` 但 `out_step_schema` 仍带 runtime-error case”的样本。
  3. 若需要，补充 `TODO-P5.md` 中 `P5-T05R` 的 review 重点，明确后续 review 也要检查这层边界。
  4. 跑任务要求的测试与 `clippy`，然后更新 TODO 完成记录并提交。
- 已完成代码修改：
  - `crates/scoopc/src/effect_lowered/ir.rs` 新增 `LateLoweredContinuationContract`，在 step case / resume method / continuation method 中显式保留 `resume_tuple_ty`、`answer_ty`、`out_step_schema`、`surface_ty`。
  - `crates/scoopc/src/effect_lowered/builder.rs` 现在统一从 `ContinuationSchema` 构造上述 contract，并校验 `out_step_schema` 与 `answer_ty` 不会和当前 step return contract 漂移。
  - `crates/scoopc/src/effect_lowered/dump.rs` 已把 `surface_ty` / `out_step_schema` / `answer_ty` 写入 stable dump。
- 已完成验证：`cargo fmt --all`、`cargo test -p scoopc --no-default-features refactor_resume_interface`、`cargo test -p scoopc --no-default-features refactor_continuation_object`、`cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`、`cargo test -p scoopc --no-default-features refactor_late_lowered_ir`、`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。
- 已完成任务文档回写：`TODO-P5.md` 已把 `P5-T04b` 标为 `[DONE]` 并补齐完成记录，同时加强了 `P5-T05R` 的 review 重点；`TODO.md` 已同步索引状态。
- 下一步只剩检查工作区、创建本次任务提交，然后停止。
