# 本轮执行计划

## 约束与工作方式
- 本轮只处理 `TODO.md` 中第一个未完成任务；如果发现其前置缺陷或规范不匹配，优先修复缺陷或把缺陷改写成更靠前的前置任务，然后停止。
- 在执行过程中持续更新本文件，记录当前结论、关键决策、阻塞关系、已完成步骤和测试结果。
- 这里记录的是对外可审阅的执行理由与步骤摘要，不包含逐字内部推理。

## 初始步骤计划
1. 检查最新一次 Git 提交，确认提交信息或相关改动是否提到了尚未解决的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对现有计划与 `TODO.md` 是否一致。
4. 结合任务与最近提交内容评估是否存在必须先修复的前置问题。
5. 如果首个未完成任务过大，则把它拆成更小的子任务，并同步更新 `TODO.md` 与 `PLAN.md`。
6. 实现本轮要执行的唯一任务。
7. 运行相关测试，并补充必要测试；如果发现既有问题，立即转为当前优先事项处理。
8. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成状态或新的依赖关系。
9. 提交本轮改动，随后停止。

## 当前状态
- 已完成：创建本计划文件并写入初始执行步骤。
- 已完成：检查最新提交、`TODO.md` 与 `PLAN.md`。
- 已完成：执行 `T5000b4b 拆出 MainCodegen 的 function/body 级上下文`。
- 当前目标：同步更新任务/计划记录并准备提交。

## 已确认结论
- 最新提交为 `20c5af549131eedd381555b24a5f0cf05e4f7f25`，提交主题是 `[T5000b4aR] Review shared codegen cache separation`。
- 该提交以及 `TODO.md` / `PLAN.md` 中对应 review 记录均表明：在 `T5000b4aR` 复核后，未发现必须插入到 `T5000b4b` 之前的新前置缺陷任务。
- 因此本轮无需先新增 prerequisite task，可直接进入 `T5000b4b` 实现。

## 针对当前任务的细化执行步骤
1. 盘点 `MainCodegen` 中 function/body 生命周期字段及其所有读写点。已完成。
2. 识别 child-codegen、nested lowering、函数入口/出口与顶层常量求值路径如何创建、重置或保存这些状态。已完成。
3. 设计并引入独立的 function/body 上下文类型，确保边界清晰且不改变现有 lowering 语义。已完成。
4. 更新相关 helper 与调用点，使其通过明确上下文访问函数级状态。已完成。
5. 运行格式化、相关测试、全量测试与 `clippy -D warnings`。已完成。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，记录本轮实现与验证结果。进行中。
7. 提交改动并停止。待执行。

## 实现摘要
- 在 `crates/scoopc/src/llvm/codegen/mod.rs` 中新增 `FunctionBodyCodegenCx`，集中承接 `env`、`extra_gc_root_slots` / `next_extra_gc_root_slot_id`、`loop_context_stack`、`return_context`、`current_fun_return_ty`、`current_sret_return_ptr`、`top_level_const_eval_stack`。
- `MainCodegen` 当前改为持有 `function_cx`，并通过 `take_function_body_cx()` / `restore_function_body_cx()` 提供整组函数级状态交换能力。
- `call/resume.rs` 与 `effect/state_machine_emitter.rs` 的关键入口已改为整体交换 `function_cx`，不再继续手动保存/恢复一串普通函数级字段。
- 相关 lowering 模块已统一经 `self.function_cx` 访问函数级状态。

## 验证结果
- `cargo check -p scoopc --all-targets`：通过。
- `cargo fmt --all`：通过。
- `cargo test -p scoopc llvm::`：通过。
- `cargo test --all`：通过。
- `cargo clippy --all-targets -- -D warnings`：通过。

## 待补充信息
- 是否在提交前还需要补充额外说明或小修。
- 最终提交信息。
