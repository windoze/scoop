# 当前执行计划

## 约束

- 以 `TODO.md` 为任务排序和完成状态的唯一来源。
- 本次只完成第一个标题未标记 `[DONE]` 的任务，然后停止。
- 不做开放式历史问题清扫；只处理当前任务相关或验证中暴露且未被明确排期的问题。
- 如遇到阻塞当前任务的缺失功能、规格不匹配或失败测试，优先修复；若不能在本次完成，则在 `TODO.md` 中插入最小必要前置任务并提交后停止。
- 只有阶段级计划、依赖或完成标准变化时才更新 `PLAN.md`。

## 步骤

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务，并记录任务要求、依赖和验证要求。
2. 检查最近提交信息，判断是否有与该任务直接相关的未完成事项需要纳入本次任务或作为前置任务记录。
3. 阅读与当前任务相关的代码、测试和文档，确认实现边界。
4. 按任务要求做最小且完整的代码或文档修改，避免绕过规格或只针对夹具的特殊处理。
5. 按要求运行格式化、lint、相关测试；如任务影响编译行为，再运行完整 Rust 测试和夹具套件。
6. 若验证通过，将当前任务标题加上 `[DONE]`，更新完成记录；若出现未排期失败，修复或在 `TODO.md` 插入前置任务。
7. 检查工作区差异，提交本次所有相关修改，提交信息包含任务编号。
8. 停止，不继续处理下一个任务。

## 进度

- 已创建本执行计划，下一步读取 `TODO.md`。
- 已读取 `TODO.md`，第一个未完成任务为 `TC-03：effect 路径语句改 walk LIR`。
- 最近提交为 `f9644fbb [TC-02-R] Review plain LIR codegen`，未直接声明 TC-03 的未完成 blocker；下一步检查 effect-step 语句发射入口和 LIR state body 结构。
- 已检查 `effect_lowered/body`、`effect_lowered/value.rs` 与 `LirExecutableBody`。实施策略：`CallableEmitter` 保存 callable 的 `LirExecutableBody`，slot/used-local 与 state 语句遍历改走 LIR；动态 invoke/class ctor boundary 通过 LIR statement 和分类 anchor 定位，不再回读 MIR source slice 中的 statement。
- 已完成第一轮代码迁移：`body` 目录的验收 grep（`.source_body()` / `.source_slices()` / `LateLoweredSourceBody` / `mir::Rvalue` / `mir::Statement`）已无命中。下一步运行格式化/检查，修复编译与 lint 问题。
- `cargo fmt` 和 `cargo clippy --all-targets -- -D warnings` 已通过；期间删除了已无调用的 MIR local-use 收集函数。下一步执行完整验证基线。
- 完整 fixture 首跑发现 2 个 class init cleanup GC fixture 失败，根因是 LIR class ctor boundary 成功迁移后没有像旧路径一样清理失败构造对象的临时 GC root；已修复 LIR class ctor root 清理，并将 spill root clear store 标记为 volatile。两个失败 fixture 单独重跑已通过，下一步重新跑完整基线。
- 完整验证基线已全部通过：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`。
- 已将 `TODO.md` 中 `TC-03` 标记为 `[DONE]` 并补充完成记录；下一步检查 git 差异并提交。
