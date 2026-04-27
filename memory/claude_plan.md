# 本轮执行计划（可共享摘要）

## 目标

按照 `TODO.md` 的顺序，只完成第一个未完成任务；如果在执行前或执行中发现最新提交提到的既有问题、测试暴露的既有缺陷、规格不匹配或实现边界缺失，则先修复该问题，或把它作为前置任务插入 `TODO.md` 并更新 `PLAN.md`，然后停止。

## 约束

- 不绕过现有缺陷，不用临时性 workaround。
- 只完成一个任务或一个新拆出的首个子任务。
- 所有分析、执行记录和结论使用中文。
- 在关键步骤完成后持续更新本文件。

## 初始步骤

1. 检查最新一次 Git 提交，确认提交说明是否提到了待修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解该任务的上下文、依赖和预期边界。
4. 检查当前工作树状态，避免覆盖用户已有改动。
5. 评估该任务是否过大：
   - 如果过大，则先拆分任务，更新 `TODO.md` 与 `PLAN.md`，提交后停止。
   - 如果可执行，则直接进入实现。

## 实施步骤

1. 阅读与目标任务直接相关的代码、测试和文档。
2. 实现任务要求的改动，必要时补充或调整测试。
3. 运行与改动直接相关的测试。
4. 运行必要的质量检查，至少覆盖：
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 若任务涉及格式或文档同步，再补充对应命令
5. 若测试或检查暴露既有问题：
   - 判断是否属于当前任务前置问题；
   - 若是，先修复，或写入 `TODO.md` 作为前置任务并停止。

## 收尾步骤

1. 更新 `TODO.md`，把本轮完成的任务标记为已完成。
2. 更新 `PLAN.md`，反映当前状态、实现细节和后续顺序。
3. 更新本文件，记录完成情况与执行结果。
4. 使用清晰的 Git 提交信息提交本轮改动。
5. 停止，不继续处理下一个任务。

## 本轮目标（已确认）

- `TODO.md` 中第一个未完成任务是 `T5000h0aR Review：确认 production frontend 已稳定保留 materialized MIR / summary 产物`。
- 最新提交信息为 `[T5000h0a] Retain materialized MIR in frontend outputs`，提交说明本身未直接声明一个待先修复的既有缺陷，因此当前按 `T5000h0aR` 进入 review。

## 本轮具体执行计划

1. 阅读 `TODO.md` / `PLAN.md` 中 `T5000h0a` 与 `T5000h0aR` 相邻条目，明确 review 验收口径。
2. 查看最新提交的具体 diff，锁定本次需要审查的文件和行为改动。
3. 阅读相关实现与现有测试，重点检查：
   - production frontend 是否真的把 `MaterializedMir` / summaries 保留到后续产物；
   - build 与 single-file 两条路径是否一致；
   - 是否存在只保留 `instance_keys`、丢失 body/summary 视图、或生命周期/所有权错误；
   - 是否缺少覆盖新增行为的测试。
4. 运行与本改动相关的测试与质量检查，必要时补充更小范围命令帮助定位问题。
5. 根据 review 结果执行其一：
   - 若发现既有缺陷：先修复缺陷，补测试，更新 `TODO.md` / `PLAN.md` / 本文件，再提交并停止；
   - 若未发现阻塞问题：将 `T5000h0aR` 标为完成，更新 `TODO.md` / `PLAN.md` / 本文件，提交并停止。

## 当前状态

- 已完成：初始计划写入、最新提交检查、首个未完成任务定位。
- 已完成：读取 `T5000h0a` 改动与 `T5000h0aR` 验收口径。
- 已完成：静态检查 `LoweredHir::materialized_mir` 的赋值点、build/single-file 生产入口调用点与 `LoweredHir` 构造点，暂未发现“入口重新组装后丢失 materialized MIR”的路径。
- 已完成：定向回归测试
  - `cargo test -p scoopc single_file_frontend_keeps_distinct_effect_row_generic_instances -- --nocapture`
  - `cargo test -p scoop build_frontend_keeps_distinct_effect_row_generic_instances -- --nocapture`
  - 结果：均通过。
- 已完成：更大范围验证
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- test`
  - 结果：全部通过，其中 fixture 套件结果为 `fixtures: ok (1201)`。
- 已完成：回写 `TODO.md` / `PLAN.md`，将 `T5000h0aR` 标记为完成，并记录“未发现需要插入的新前置缺陷任务”的 review 结论。
- 待执行收尾：检查工作树、提交本轮改动，然后停止。
