# 当前执行计划

## 约束

- `TODO.md` 是任务顺序、完成状态和验收要求的唯一权威来源。
- 本次只完成第一个标题未带 `[DONE]` 的任务，完成后提交并停止。
- 不跳过 review 任务，不因任务较大而拆分；仅在存在真实前置阻塞时最小化新增前置任务。
- 如发现未排期的测试或 fixture 失败，必须修复或把最小前置任务加入 `TODO.md`，不能把当前任务标记完成。
- 计划文件只记录可公开的执行计划和进度，不记录隐藏推理过程。

## 步骤

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务，并记录任务 ID、要求、依赖和验证命令。
2. 检查最近提交是否明确提到与该任务直接相关的未完成事项；若相关，将其纳入当前任务或作为前置项更新 `TODO.md`。
3. 阅读与当前任务直接相关的代码、测试、fixture 和文档，避免开放式历史问题扫查。
4. 如任务可直接执行，进行最小且完整的实现；如遇真实前置阻塞，更新 `TODO.md` 插入最小前置任务并停止。
5. 按要求运行格式化、lint、相关测试以及必要的完整测试/fixture 验证；发现未排期失败则修复或排期。
6. 更新 `TODO.md`，给完成任务标题加 `[DONE]`，补充完成记录；仅当阶段计划变化时更新 `PLAN.md`。
7. 提交本次所有相关更改，提交信息包含任务 ID，并在提交后停止。

## 进度记录

- 已创建初始执行计划。
- 已读取 `TODO.md` 与 `TODO-3.md`，确认本次第一个未完成任务为 `T3-04J`：删除 source-payload class ctor fallback 与 gate 残余缺口。
- 已检查最近提交，`ec566011 [T3-04R] Schedule tenth review follow-up` 与 `T3-04J` 直接相关。
- 已发现工作区存在与本任务相关的未提交中间改动：LIR class ctor source contract 结构、materialized backend contracts 的 ctor call-site 输入尚未完成接线。
- 具体执行方向：保留并补全上游发布的 class ctor source contract，P6 HIR/source class ctor lowering 改为只按该 contract 消费；删除 `select_class_ctor_from_source_payload`、`class_ctor_init_body_for_source_selection`、`same_span_class_ctor_init_body` 以及 result-type / unresolved-ident 触发的 ctor fallback；补齐 dependency gate 对当前 helper 名称和等价模式的守卫。
- 已完成主体编辑：LIR init body 会收集 source-payload class ctor contract；LLVM class ctor source lowering 只消费该 contract；精确 init-key 查询已删除 span 后缀恢复；direct call lowering 已删除 result-type / unresolved-ident 构造器 fallback；dependency gate 已补实际 helper 名称守卫。
- 完整 fixture 初跑暴露 9 个 source-payload/global/object/class-init 相关失败；已修复两个根因：generic class init 收集改为 fixed point，LIR program 发布全局 source class ctor contracts，P6 source lowering 只消费这些 contract。
- 已针对失败项重跑：`sysroot_atomic_basic`、`cross_file_ctor_named_default_basic`、`gc_module_global_roots_move_basic`、`gc_continuation_multi_thread_concurrent_alloc_resume` 和 5 个 `std_sync_backend_parity_*` 均已恢复通过。
- 已完成最终全量验证：`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`（1664 checks）均通过。
- 已更新 `TODO-3.md`，将 `T3-04J` 标为 `[DONE]` 并填写完成记录；已更新 `TODO.md` 当前活跃任务为下一项 `T3-04R`。
- 下一步检查 git diff/status，提交本次任务变更后停止。
