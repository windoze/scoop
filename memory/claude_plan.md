# 执行计划

## 约束说明
- 不提供逐字内部思维链，但会持续记录可审计的执行计划、关键判断、阻塞信息与已完成步骤。
- 本次调用只处理第一个未完成的详细任务；完成后更新对应 TODO 记录、验证、提交并停止。

## 初始步骤
1. 读取 `TODO.md`，把它当作索引文件使用。
2. 按 `TODO.md` 中引用的顺序读取对应 `TODO-Px.md`，确认第一个标题未带 `[DONE]` 的详细任务。
3. 检查最近一次提交是否明确提到与该任务直接相关但未完成的问题；若是，则把它视为当前任务的一部分或必要前置。
4. 在不做开放式历史问题排查的前提下，阅读当前任务所需的最小范围代码、测试、规格和相关文档。

## 执行策略
1. 严格按当前任务原始要求实现，不因困难主动拆分。
2. 若发现阻塞当前任务、导致规格不成立、或由本次修改引入的回归：
   - 先确认是否必须新增前置任务。
   - 仅在无法正确落地当前任务时，向对应 `TODO-Px.md` 插入最小必要前置任务，并同步 `TODO.md`。
   - 仅当阶段级计划变化时更新 `PLAN.md`。
3. 修改代码时采用最小正确变更，避免引入规避性实现或临时兼容层。

## 验证与收尾
1. 运行与当前任务直接相关的测试。
2. 如任务涉及通用编译/静态检查风险，补充运行 `cargo test --all`、`cargo clippy --all-targets -- -D warnings` 或更小但足够的目标命令。
3. 更新 `TODO-Px.md`：仅在任务真正完成时给标题加 `[DONE]`，并填写完成记录。
4. 若索引状态、标题、顺序或任务集发生变化，同步更新 `TODO.md`。
5. 若阶段计划确实变化，再更新 `PLAN.md`。
6. 检查工作区变更，按要求提交本次任务相关全部未提交内容，然后停止。

## 进度记录
- 已完成：创建本计划文件。
- 已完成：读取 `TODO.md` 与 `TODO-P6-part2.md`，确认首个未完成详细任务为 `P6-T02p`（发布 callable version 选择 contract）。
- 已确认：最新提交标题为 `[P6-T03] Add callable version selection prerequisite`，与当前任务直接相关，说明仓库已把该问题前置到当前执行单元，继续按 `P6-T02p` 落地即可。
- 已完成：审阅未提交 diff，确认当前任务已有中断残留实现，主要内容包括 `body_version_key` 查询、known-instance selector、carrier target contract 与定向测试。
- 已完成：运行 `refactor_llvm_callable_version_query` / `refactor_llvm_known_instance_version_selection` / `refactor_llvm_callable_carrier_version_selection` 定向测试，定位到一个真实回归：carrier target 发布错误地扫描了整个 `class_itables` / `class_vtables` 索引，把未进入 late-lowered program 的 `Hashable.hash` 等槽位也当成必须发布对象。
- 已完成：把 carrier target 发布范围收敛到“当前 late-lowered program 已发布的 callable roots”，保留同 root 多 version 的显式拒绝语义；三组定向测试现已通过。
- 已完成：运行 `cargo fmt --all`、`cargo test -p scoopc refactor_llvm_`、相关 build fixture / dump-effect-lowered 命令与 `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`，全部通过。
- 已完成：更新 `TODO-P6-part2.md` 与 `TODO.md`，将 `P6-T02p` 标记为 `[DONE]` 并补齐完成记录与验证清单。
- 下一步：检查最终 diff，按 `P6-T02p` 主题提交当前全部未提交文件，然后停止。
